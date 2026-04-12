//! Main application event loop and state management.
//!
//! The `App` struct orchestrates the TUI lifecycle: event polling, action dispatching,
//! component rendering, and integration with the WebSocket/REST clients.

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout, Rect};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::{
    api::{network, rest},
    core::action::Action,
    core::config::Config,
    core::game::{
        AccountLogEntry, ActionRecord, CharacterState, GEFeedEntry, GameState, LogEntry, WsStatus,
    },
    ui::components::{
        Component, character_cards::CharacterCards, fps::FpsCounter, loading_screen::LoadingScreen,
        log_panel::LogPanel, sidebar::Sidebar,
    },
    ui::image_cache::{ImageCache, SharedImageCache},
    ui::tui::{Event, Tui},
};

/// Maximum entries kept in the footer log.
const MAX_LOG: usize = 500;
/// Maximum GE feed entries in sidebar.
const MAX_GE: usize = 60;
/// Maximum active world events displayed.
const MAX_EVENTS: usize = 20;
/// Maximum action history rows per character.
const MAX_HISTORY: usize = 8;
/// Gold snapshot interval (take a snapshot every N ticks ~= 1 second).
const GOLD_SNAPSHOT_TICKS: u64 = 60;

/// Main application state and event loop controller.
///
/// Manages:
/// - TUI terminal and event polling
/// - Central `GameState` shared with all components
/// - Image cache for sprite downloads
/// - Action bus (mpsc channel) for inter-component communication
/// - WebSocket and REST client lifecycle
/// - Component registration and frame rendering
pub struct App {
    /// Parsed keybindings and styling from config files.
    config: Config,
    /// Target ticks per second (default 4).
    tick_rate: f64,
    /// Target frames per second (default 60).
    frame_rate: f64,
    /// Set to true when user requests quit (Ctrl+C, 'q', etc.).
    should_quit: bool,
    /// Set to true on suspend signal (Ctrl+Z); app enters background.
    should_suspend: bool,
    /// Current UI mode (Loading or Home).
    mode: Mode,
    /// Accumulated key events for multi-key chord detection (e.g. 'g' then 'g').
    last_tick_key_events: Vec<KeyEvent>,
    /// Sender half of the action bus — used by event handlers and background tasks.
    action_tx: mpsc::UnboundedSender<Action>,
    /// Receiver half of the action bus — polled in the main loop.
    action_rx: mpsc::UnboundedReceiver<Action>,

    /// Central shared game state — written by this loop, read by components during draw.
    /// Contains characters, world events, GE feed, map tiles, and log entries.
    game_state: Arc<RwLock<GameState>>,

    /// Shared image download + disk cache (character skins, items, maps, effects, etc.).
    /// Wraps concurrent downloads with deduplication and disk persistence.
    image_cache: SharedImageCache,

    /// Character card grid component (3 columns of animated status cards).
    character_cards: CharacterCards,
    /// Right sidebar showing WS status, economy, world events, GE feed.
    sidebar: Sidebar,
    /// Footer log panel showing all action events.
    log_panel: LogPanel,
    /// FPS/TPS counter overlay (top-right).
    fps_counter: FpsCounter,
    /// Loading screen with progress bar (shown while assets download).
    loading_screen: LoadingScreen,

    /// True once `CharactersFetched` action received.
    characters_fetched: bool,
    /// True once `MapsFetched` action received (all tile sprites settled).
    maps_fetched: bool,

    /// Cancellation token for the WebSocket listener task.
    ws_cancel: CancellationToken,

    /// Counter incremented on each `Action::Tick` — used for periodic tasks.
    tick_count: u64,
}

/// Application display mode.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    /// Loading screen with progress bar — shown while assets download.
    #[default]
    Loading,
    /// Home screen with character cards, sidebar, and log panel.
    Home,
}

impl App {
    /// Create a new App instance with the given tick and frame rates.
    ///
    /// Initializes all components, the shared game state, the image cache,
    /// and the action bus. Does not start the WebSocket or event loop yet.
    pub fn new(tick_rate: f64, frame_rate: f64) -> color_eyre::Result<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let game_state = Arc::new(RwLock::new(GameState::default()));
        let image_cache = ImageCache::new_shared();

        Ok(Self {
            tick_rate,
            frame_rate,
            should_quit: false,
            should_suspend: false,
            config: Config::new()?,
            mode: Mode::Loading,
            last_tick_key_events: Vec::new(),
            action_tx,
            action_rx,
            character_cards: CharacterCards::new(Arc::clone(&game_state), Arc::clone(&image_cache)),
            sidebar: Sidebar::new(Arc::clone(&game_state), Arc::clone(&image_cache)),
            log_panel: LogPanel::new(Arc::clone(&game_state)),
            fps_counter: FpsCounter::default(),
            loading_screen: LoadingScreen::new(Arc::clone(&image_cache)),
            characters_fetched: false,
            maps_fetched: false,
            game_state,
            image_cache,
            ws_cancel: CancellationToken::new(),
            tick_count: 0,
        })
    }

    pub async fn run(&mut self) -> color_eyre::Result<()> {
        let mut tui = Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        let tx = self.action_tx.clone();
        let config = self.config.clone();
        for component in self.components_mut() {
            component.register_action_handler(tx.clone())?;
            component.register_config_handler(config.clone())?;
        }

        // Route image download events to the TUI footer log
        ImageCache::set_log_tx(&self.image_cache, self.action_tx.clone());

        let size = tui.size()?;
        for component in self.components_mut() {
            component.init(size)?;
        }

        // Spawn WebSocket listener + one-shot REST character + map fetch
        let token = std::env::var("ARTIFACTS_TOKEN").unwrap_or_default();
        if !token.is_empty() {
            self.ws_cancel = CancellationToken::new();
            network::spawn_ws_listener(
                token.clone(),
                self.action_tx.clone(),
                self.ws_cancel.clone(),
            );
            info!("websocket listener spawned");

            rest::spawn_character_fetch(token.clone(), self.action_tx.clone());
            rest::spawn_map_fetch(
                token,
                self.action_tx.clone(),
                Arc::clone(&self.game_state),
                Arc::clone(&self.image_cache),
            );
            info!("REST fetches spawned");
        } else {
            info!("ARTIFACTS_TOKEN not set — skipping loading screen");
            let _ = self
                .action_tx
                .send(Action::WsDisconnected("ARTIFACTS_TOKEN not set".into()));
            // No network work will ever complete, go straight to Home.
            self.mode = Mode::Home;
        }

        let action_tx = self.action_tx.clone();
        loop {
            self.handle_events(&mut tui).await?;
            self.handle_actions(&mut tui)?;
            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                action_tx.send(Action::ClearScreen)?;
                tui.enter()?;
            } else if self.should_quit {
                self.ws_cancel.cancel();
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }

    async fn handle_events(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        let Some(event) = tui.next_event().await else {
            return Ok(());
        };
        let action_tx = self.action_tx.clone();
        match event {
            Event::Quit => action_tx.send(Action::Quit)?,
            Event::Tick => action_tx.send(Action::Tick)?,
            Event::Render => action_tx.send(Action::Render)?,
            Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
            Event::Key(key) => self.handle_key_event(key)?,
            _ => {}
        }
        for component in self.components_mut() {
            if let Some(action) = component.handle_events(Some(event.clone()))? {
                action_tx.send(action)?;
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<()> {
        let action_tx = self.action_tx.clone();
        let Some(keymap) = self
            .config
            .keybindings
            .0
            .get(&self.mode)
        else {
            return Ok(());
        };
        match keymap.get(&vec![key]) {
            Some(action) => {
                info!("Got action: {action:?}");
                action_tx.send(action.clone())?;
            }
            _ => {
                self.last_tick_key_events.push(key);
                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                    info!("Got action: {action:?}");
                    action_tx.send(action.clone())?;
                }
            }
        }
        Ok(())
    }

    fn handle_actions(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        while let Ok(action) = self.action_rx.try_recv() {
            if action != Action::Tick && action != Action::Render {
                debug!("{action:?}");
            }
            match &action {
                Action::Tick => {
                    self.last_tick_key_events.drain(..);
                    self.tick_count += 1;

                    if self.mode == Mode::Loading {
                        // MapsFetched is only sent after all map tile images have settled
                        // (downloaded, disk-cached, or permanently failed), so no stats
                        // comparison is needed here.
                        if self.characters_fetched && self.maps_fetched {
                            self.mode = Mode::Home;
                            let _ = tui.terminal.clear();
                        }
                    }

                    // Toggle heartbeat animation
                    if let Ok(mut gs) = self.game_state.write() {
                        gs.heartbeat = !gs.heartbeat;
                        // Periodically snapshot gold for gold/hr
                        if self
                            .tick_count
                            .is_multiple_of(GOLD_SNAPSHOT_TICKS)
                        {
                            gs.snapshot_gold();
                        }
                    }
                }
                Action::Quit => self.should_quit = true,
                Action::Suspend => self.should_suspend = true,
                Action::Resume => self.should_suspend = false,
                Action::ClearScreen => tui.terminal.clear()?,
                Action::Resize(w, h) => self.handle_resize(tui, *w, *h)?,
                Action::Render => self.render(tui)?,

                // ── WebSocket status ───────────────────────────────────────
                Action::WsConnected => {
                    let mut gs = self.game_state.write().unwrap();
                    gs.ws_status = WsStatus::Connected;
                    push_log(
                        &mut gs.log_entries,
                        "[SYS]",
                        ratatui::style::Color::Blue,
                        "",
                        "websocket connected",
                    );
                }
                Action::WsConnect | Action::WsReconnect => {
                    self.game_state
                        .write()
                        .unwrap()
                        .ws_status = WsStatus::Connecting;
                }
                Action::WsDisconnected(reason) => {
                    let mut gs = self.game_state.write().unwrap();
                    gs.ws_status = WsStatus::Disconnected(reason.clone());
                    push_log(
                        &mut gs.log_entries,
                        "[SYS]",
                        ratatui::style::Color::Red,
                        "",
                        &format!("disconnected: {reason}"),
                    );
                }

                // ── Character data ────────────────────────────────────────
                Action::CharactersFetched(chars) => {
                    self.characters_fetched = true;
                    {
                        let mut gs = self.game_state.write().unwrap();
                        for incoming in chars {
                            upsert_character_full(&mut gs, incoming);
                        }
                    }
                    // Prefetch character skins and equipped item icons so they're
                    // ready by the time the first frame draws.
                    let gs = self.game_state.read().unwrap();
                    for ch in &gs.characters {
                        prefetch_character_images(&self.image_cache, ch);
                    }
                }
                Action::MapsFetched => {
                    self.maps_fetched = true;
                }
                Action::AccountLog(entry) => {
                    let mut gs = self.game_state.write().unwrap();
                    apply_account_log(&mut gs, entry);
                }
                Action::OnlineCharacters(chars) => {
                    let mut gs = self.game_state.write().unwrap();
                    for incoming in chars {
                        if let Some(existing) = gs
                            .characters
                            .iter_mut()
                            .find(|c| c.name == incoming.name)
                        {
                            existing.x = incoming.x;
                            existing.y = incoming.y;
                            if existing.skin.is_empty() {
                                existing.skin = incoming.skin.clone();
                            }
                        }
                    }
                }

                // ── World events ──────────────────────────────────────────
                Action::EventSpawn(evt) => {
                    let mut gs = self.game_state.write().unwrap();
                    if gs.world_events.len() >= MAX_EVENTS {
                        gs.world_events.remove(0);
                    }
                    gs.world_events.push(evt.clone());
                    push_log(
                        &mut gs.log_entries,
                        "[EVT+]",
                        ratatui::style::Color::LightGreen,
                        "",
                        &evt.name,
                    );
                }
                Action::EventRemoved(code) => {
                    let mut gs = self.game_state.write().unwrap();
                    gs.world_events
                        .retain(|e| e.code != *code);
                    push_log(
                        &mut gs.log_entries,
                        "[EVT-]",
                        ratatui::style::Color::DarkGray,
                        "",
                        code,
                    );
                }

                // ── Grand Exchange ────────────────────────────────────────
                Action::GEOrderCreated(order) => {
                    let mut gs = self.game_state.write().unwrap();
                    push_ge_entry(&mut gs.ge_feed, GEFeedEntry::Order(order.clone()));
                    let verb = if order.order_type == "sell" {
                        "SELL"
                    } else {
                        "BUY "
                    };
                    push_log(
                        &mut gs.log_entries,
                        "[GE]",
                        ratatui::style::Color::Cyan,
                        "",
                        &format!("{verb} {} ×{}@{}g", order.code, order.quantity, order.price),
                    );
                }
                Action::GETransactionCompleted(txn) => {
                    let mut gs = self.game_state.write().unwrap();
                    push_ge_entry(&mut gs.ge_feed, GEFeedEntry::Transaction(txn.clone()));
                    let verb = if txn.order_type == "sell" {
                        "SOLD"
                    } else {
                        "BGHT"
                    };
                    push_log(
                        &mut gs.log_entries,
                        "[GE]",
                        ratatui::style::Color::Yellow,
                        "",
                        &format!("{verb} {} ×{}={}g", txn.code, txn.quantity, txn.total_price),
                    );
                }

                // ── Misc ──────────────────────────────────────────────────
                Action::AchievementUnlocked {
                    character,
                    achievement_name,
                } => {
                    let mut gs = self.game_state.write().unwrap();
                    push_log(
                        &mut gs.log_entries,
                        "[ACHV]",
                        ratatui::style::Color::LightYellow,
                        character,
                        achievement_name,
                    );
                }
                Action::Announcement(text) => {
                    let mut gs = self.game_state.write().unwrap();
                    push_log(
                        &mut gs.log_entries,
                        "[ANON]",
                        ratatui::style::Color::Magenta,
                        "",
                        text,
                    );
                }

                // ── Image download events ─────────────────────────────────
                Action::SystemLog {
                    tag,
                    message,
                } => {
                    let color = system_log_color(tag);
                    // Map the owned tag string to a static str so LogEntry is happy.
                    let static_tag: &'static str = match tag.as_str() {
                        "[IMG↓]" => "[IMG↓]",
                        "[IMG✓]" => "[IMG✓]",
                        "[IMG✗]" => "[IMG✗]",
                        "[IMG◈]" => "[IMG◈]",
                        _ => "[IMG]  ",
                    };
                    let mut gs = self.game_state.write().unwrap();
                    if gs.log_entries.len() >= MAX_LOG {
                        gs.log_entries.pop_front();
                    }
                    gs.log_entries
                        .push_back(crate::core::game::LogEntry {
                            tag: static_tag,
                            tag_color: color,
                            character: String::new(),
                            message: message.clone(),
                        });
                }

                _ => {}
            }

            // UI-local actions still forwarded to components (FocusNext, ToggleLog, etc.)
            let mut follow_up = Vec::new();
            for component in self.components_mut() {
                if let Some(a) = component.update(action.clone())? {
                    follow_up.push(a);
                }
            }
            for a in follow_up {
                self.action_tx.send(a)?;
            }
        }
        Ok(())
    }

    fn handle_resize(&mut self, tui: &mut Tui, w: u16, h: u16) -> color_eyre::Result<()> {
        tui.resize(Rect::new(0, 0, w, h))?;
        self.render(tui)?;
        Ok(())
    }

    fn render(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        tui.draw(|frame| {
            let area = frame.area();

            if self.mode == Mode::Loading {
                let _ = self.loading_screen.draw(frame, area);
                return;
            }

            // ── Top-level split: [main 85%] / [footer 15%] ───────────────
            let [
                main_area,
                footer_area,
            ] = Layout::vertical([
                Constraint::Percentage(85),
                Constraint::Percentage(15),
            ])
            .areas(area);

            // ── Main: [character grid 80%] | [sidebar 20%] ───────────────
            let [
                grid_area,
                sidebar_area,
            ] = Layout::horizontal([
                Constraint::Percentage(80),
                Constraint::Percentage(20),
            ])
            .areas(main_area);

            // Character grid
            if let Err(e) = self
                .character_cards
                .draw(frame, grid_area)
            {
                let _ = self
                    .action_tx
                    .send(Action::Error(format!("cards: {e}")));
            }
            // Sidebar
            if let Err(e) = self.sidebar.draw(frame, sidebar_area) {
                let _ = self
                    .action_tx
                    .send(Action::Error(format!("sidebar: {e}")));
            }
            // Footer log
            if let Err(e) = self.log_panel.draw(frame, footer_area) {
                let _ = self
                    .action_tx
                    .send(Action::Error(format!("log: {e}")));
            }
            // FPS overlay (top-right)
            if let Err(e) = self.fps_counter.draw(frame, area) {
                let _ = self
                    .action_tx
                    .send(Action::Error(format!("fps: {e}")));
            }
        })?;
        Ok(())
    }

    fn components_mut(&mut self) -> Vec<&mut dyn Component> {
        vec![
            &mut self.character_cards,
            &mut self.sidebar,
            &mut self.log_panel,
            &mut self.fps_counter,
            &mut self.loading_screen,
        ]
    }
}

// ── GameState mutation helpers (called from the main action loop) ─────────────

fn push_log(
    entries: &mut VecDeque<LogEntry>,
    tag: &'static str,
    tag_color: ratatui::style::Color,
    character: &str,
    message: &str,
) {
    if entries.len() >= MAX_LOG {
        entries.pop_front();
    }
    entries.push_back(LogEntry {
        tag,
        tag_color,
        character: character.to_string(),
        message: message.to_string(),
    });
}

fn push_ge_entry(feed: &mut VecDeque<GEFeedEntry>, entry: GEFeedEntry) {
    if feed.len() >= MAX_GE {
        feed.pop_front();
    }
    feed.push_back(entry);
}

fn upsert_character_full(gs: &mut GameState, incoming: &CharacterState) {
    if let Some(existing) = gs
        .characters
        .iter_mut()
        .find(|c| c.name == incoming.name)
    {
        let la = existing.last_action.clone();
        let ld = existing.last_description.clone();
        *existing = incoming.clone();
        if !la.is_empty() && la != "idle" {
            existing.last_action = la;
            existing.last_description = ld;
        }
    } else {
        gs.characters.push(incoming.clone());
    }
    if incoming.cooldown_secs > 0 {
        gs.cooldown_expires.insert(
            incoming.name.clone(),
            Instant::now() + Duration::from_secs(incoming.cooldown_secs as u64),
        );
        gs.cooldown_totals
            .insert(incoming.name.clone(), incoming.cooldown_secs as f64);
    }
}

fn apply_account_log(gs: &mut GameState, entry: &AccountLogEntry) {
    let name = &entry.character;

    // Upsert character
    if let Some(c) = gs
        .characters
        .iter_mut()
        .find(|c| &c.name == name)
    {
        c.last_action = entry.log_type.clone();
        c.last_description = entry.description.clone();
        if c.account.is_empty() {
            c.account = entry.account.clone();
        }
        if let Some(node) = entry.character_node() {
            c.apply_char_node(node);
        }
    } else {
        let mut c = CharacterState {
            name: name.clone(),
            account: entry.account.clone(),
            last_action: entry.log_type.clone(),
            last_description: entry.description.clone(),
            ..Default::default()
        };
        if let Some(node) = entry.character_node() {
            c.apply_char_node(node);
        }
        gs.characters.push(c);
    }

    // Cooldown
    let remaining = entry.cooldown_remaining_secs();
    let total = entry.cooldown_total_secs();
    if remaining > 0.0 {
        gs.cooldown_expires
            .insert(name.clone(), Instant::now() + Duration::from_secs_f64(remaining));
        gs.cooldown_totals
            .insert(name.clone(), total.max(remaining));
    }

    // History ring-buffer
    let buf = gs
        .action_history
        .entry(name.clone())
        .or_default();
    if buf.len() >= MAX_HISTORY {
        buf.pop_front();
    }
    buf.push_back(ActionRecord {
        log_type: entry.log_type.clone(),
        description: entry.description.clone(),
    });

    // Footer log entry
    let (tag, tag_color) = log_type_tag(&entry.log_type);
    push_log(&mut gs.log_entries, tag, tag_color, name, &entry.description);
}

/// Prefetch all images related to a character: skin, equipped items, task item.
fn prefetch_character_images(cache: &SharedImageCache, ch: &CharacterState) {
    // Character skin portrait
    if !ch.skin.is_empty() {
        ImageCache::prefetch(cache, "characters", &ch.skin);
    }
    // Equipment slots
    let slots = [
        &ch.weapon_slot,
        &ch.shield_slot,
        &ch.helmet_slot,
        &ch.body_armor_slot,
        &ch.leg_armor_slot,
        &ch.boots_slot,
        &ch.ring1_slot,
        &ch.ring2_slot,
        &ch.amulet_slot,
        &ch.rune_slot,
        &ch.artifact1_slot,
        &ch.artifact2_slot,
        &ch.artifact3_slot,
        &ch.bag_slot,
        &ch.utility1_slot,
        &ch.utility2_slot,
    ];
    for slot in slots {
        if !slot.is_empty() {
            ImageCache::prefetch(cache, "items", slot);
        }
    }
    // Current task — use task_type to pick the correct image category.
    // task_type is "monsters" for kill tasks, "items" for gather/craft tasks.
    if !ch.task.is_empty() && !ch.task_type.is_empty() {
        let category = match ch.task_type.as_str() {
            "monsters" => "monsters",
            "items" => "items",
            "resources" => "resources",
            _ => "",
        };
        if !category.is_empty() {
            ImageCache::prefetch(cache, category, &ch.task);
        }
    }
}

/// Map an image log tag to a display color.
fn system_log_color(tag: &str) -> ratatui::style::Color {
    use ratatui::style::Color;
    if tag.contains('✓') || tag.contains("◈") {
        Color::Green
    } else if tag.contains('✗') {
        Color::Red
    } else {
        Color::Cyan
    } // [IMG↓] downloading
}

fn log_type_tag(log_type: &str) -> (&'static str, ratatui::style::Color) {
    use ratatui::style::Color;
    match log_type {
        "fight" | "multi_fight" => ("[FIGHT] ", Color::Red),
        "gathering" => ("[GATHER]", Color::Green),
        "crafting" => ("[CRAFT] ", Color::Yellow),
        "movement" => ("[MOVE]  ", Color::Cyan),
        "rest" => ("[REST]  ", Color::Blue),
        "task_completed" => ("[TASK✓] ", Color::Magenta),
        "new_task" => ("[TASK+] ", Color::Magenta),
        "task_exchange" | "task_cancelled" => ("[TASK]  ", Color::DarkGray),
        "recycling" => ("[RECYCLE", Color::LightYellow),
        "buy_ge" | "create_buy_order_ge" | "fill_buy_order_ge" => ("[GE BUY]", Color::Cyan),
        "sell_ge" => ("[GE SEL]", Color::LightRed),
        "deposit_item" | "deposit_gold" => ("[BANK↓] ", Color::Gray),
        "withdraw_item" | "withdraw_gold" => ("[BANK↑] ", Color::Gray),
        "equip" | "unequip" => ("[EQUIP] ", Color::LightCyan),
        _ => ("[LOG]   ", Color::DarkGray),
    }
}
