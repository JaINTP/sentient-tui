//! Sidebar component — right panel showing status, economy, world events, and GE feed.
//!
//! Layout (top to bottom):
//! - Status strip (WS connection indicator)
//! - Economy section (total gold, gold/hour rate)
//! - World Events list (up to 3 visible)
//! - Grand Exchange feed (most recent at bottom)
//!
//! Includes boot animation with glitch effects (0–450 ms on startup).

use std::sync::{Arc, RwLock};
use std::time::Instant;

use crossterm::event::{MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};
use tachyonfx::fx::Glitch;
use tachyonfx::{Duration as FxDuration, Effect, EffectManager};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{
    core::action::Action,
    core::config::Config,
    core::game::{FocusedPanel, GEFeedEntry, GameState, WsStatus},
    ui::components::character_cards::utils::normalise_code,
    ui::image_cache::{ImageCache, ProtocolCache, SharedImageCache},
    ui::minimap::MinimapCache,
};

/// Boot animation duration (ms).
const SIDEBAR_BOOT_TOTAL_MS: u64 = 450;
/// Border-only phase duration (inner hidden, heavy glitch).
const SIDEBAR_BORDER_PHASE_MS: u64 = 180;

/// Right sidebar component — shows WS status, economy, world events, GE feed.
pub struct Sidebar {
    /// Action bus sender.
    command_tx: Option<UnboundedSender<Action>>,
    /// Global configuration.
    config: Config,
    /// Shared game state — read to get WS status, events, GE feed, character data.
    game_state: Arc<RwLock<GameState>>,
    /// Shared image cache for world event and item icons.
    image_cache: SharedImageCache,
    /// Minimap renderer for the selected character's tile area.
    minimap: MinimapCache,
    /// Timestamp of component creation — drives boot animation timing.
    born_at: Instant,
    /// Glitch effect manager for the boot animation.
    boot_glitch: EffectManager<&'static str>,
    /// Timestamp of last render — used to compute per-frame delta.
    last_tick: Instant,
    /// ProtocolCache for rendering image icons in the sidebar (like Swarm Demand).
    icon_cache: ProtocolCache,
    /// Current scroll offset for the demand list.
    demand_scroll: usize,
    /// Render area of the demand block (to detect scroll events).
    demand_area: Rect,
    /// Total number of demands (to compute max scroll).
    demand_count: usize,
    /// Current scroll offset for the GE feed (0 = most recent at bottom).
    ge_scroll: usize,
    /// Render area of the GE block (to detect scroll events).
    ge_area: Rect,
    /// Total number of GE entries (to compute max scroll).
    ge_count: usize,
}

impl Sidebar {
    pub fn new(game_state: Arc<RwLock<GameState>>, image_cache: SharedImageCache) -> Self {
        let mut boot_glitch = EffectManager::default();
        boot_glitch.add_unique_effect(
            "boot",
            Effect::new(
                Glitch::builder()
                    .cell_glitch_ratio(0.60)
                    .action_start_delay_ms(0..20)
                    .action_ms(15..70)
                    .build(),
            ),
        );
        Self {
            command_tx: None,
            config: Config::default(),
            game_state,
            image_cache,
            minimap: MinimapCache::new(),
            born_at: Instant::now(),
            boot_glitch,
            last_tick: Instant::now(),
            icon_cache: ProtocolCache::new(),
            demand_scroll: 0,
            demand_area: Rect::default(),
            demand_count: 0,
            ge_scroll: 0,
            ge_area: Rect::default(),
            ge_count: 0,
        }
    }
}

impl Component for Sidebar {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        if self
            .game_state
            .read()
            .unwrap()
            .focused_panel
            != FocusedPanel::Sidebar
        {
            return Ok(None);
        }
        match action {
            Action::FocusNext => {
                let max = self.demand_count.saturating_sub(
                    self.demand_area
                        .height
                        .saturating_sub(2) as usize,
                );
                self.demand_scroll = self
                    .demand_scroll
                    .saturating_add(1)
                    .min(max);
            }
            Action::FocusPrev => {
                self.demand_scroll = self.demand_scroll.saturating_sub(1);
            }
            _ => {}
        }
        Ok(None)
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> color_eyre::Result<Option<Action>> {
        let in_rect = |r: Rect| {
            mouse.column >= r.x
                && mouse.column < r.x + r.width
                && mouse.row >= r.y
                && mouse.row < r.y + r.height
        };

        if in_rect(self.demand_area) {
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    let max = self.demand_count.saturating_sub(
                        self.demand_area
                            .height
                            .saturating_sub(2) as usize,
                    );
                    self.demand_scroll = self
                        .demand_scroll
                        .saturating_add(1)
                        .min(max);
                    return Ok(Some(Action::Tick));
                }
                MouseEventKind::ScrollUp => {
                    self.demand_scroll = self.demand_scroll.saturating_sub(1);
                    return Ok(Some(Action::Tick));
                }
                _ => {}
            }
        }

        if in_rect(self.ge_area) {
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    self.ge_scroll = self.ge_scroll.saturating_sub(1);
                    return Ok(Some(Action::Tick));
                }
                MouseEventKind::ScrollUp => {
                    let max = self
                        .ge_count
                        .saturating_sub(self.ge_area.height.saturating_sub(1) as usize);
                    self.ge_scroll = self
                        .ge_scroll
                        .saturating_add(1)
                        .min(max);
                    return Ok(Some(Action::Tick));
                }
                _ => {}
            }
        }

        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let elapsed_ms = self.born_at.elapsed().as_millis() as u64;
        let delta_ms = self.last_tick.elapsed().as_millis() as u32;
        self.last_tick = Instant::now();

        // ── Outer block ───────────────────────────────────────────────────
        let version = env!("CARGO_PKG_VERSION");
        let focused = self
            .game_state
            .read()
            .unwrap()
            .focused_panel
            == FocusedPanel::Sidebar;
        let border_color = if focused {
            Color::Cyan
        } else {
            Color::DarkGray
        };
        let block = Block::default()
            .title(format!(" Info v{} ", version))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        // ── Phase 1: border-only flicker ──────────────────────────────────
        if elapsed_ms < SIDEBAR_BORDER_PHASE_MS {
            if area.width > 0 && area.height > 0 {
                let dur = FxDuration::from_millis(delta_ms.max(1));
                self.boot_glitch
                    .process_effects(dur, frame.buffer_mut(), area);
            }
            return Ok(());
        }

        if inner.height < 4 {
            return Ok(());
        }

        // ── Brief read snapshot ───────────────────────────────────────────
        let (
            ws_status,
            total_gold,
            gold_per_hr,
            events,
            ge_feed,
            sel_char,
            map_tiles,
            char_layer,
            demand,
        ) = {
            let gs = self.game_state.read().unwrap();
            let total_gold: u64 = gs
                .characters
                .iter()
                .map(|c| c.gold as u64)
                .sum();
            let gold_per_hr = gs.gold_per_hour();
            let events = gs.world_events.clone();
            let ge_feed: Vec<GEFeedEntry> = gs.ge_feed.iter().cloned().collect();
            let demand = gs.swarm_demand.clone();
            let ws_status = gs.ws_status.clone();
            let sel_idx = gs
                .selected_character
                .min(gs.characters.len().saturating_sub(1));
            let sel_char = gs.characters.get(sel_idx).cloned();
            let map_tiles = gs.map_tiles.clone();
            let char_layer = sel_char
                .as_ref()
                .and_then(|ch| {
                    gs.map_id_to_layer
                        .get(&ch.map_id)
                        .cloned()
                })
                .unwrap_or_else(|| "overworld".to_string());
            (
                ws_status,
                total_gold,
                gold_per_hr,
                events,
                ge_feed,
                sel_char,
                map_tiles,
                char_layer,
                demand,
            )
        };

        // ── Layout: status(1) + economy(2) + events(dynamic) + demand(dynamic) + ge(compact) + minimap(fill) ──
        let events_rows = (events.len().min(3) as u16).max(1) + 1; // +1 for header
        let has_events = !events.is_empty();

        let has_demand = !demand.is_empty();

        // Show minimap whenever we have a character and enough room (≥12 rows).
        let has_minimap = sel_char.is_some() && inner.height >= 12;

        // Compute minimap target height before layout to guarantee it gets space
        let sq_h = if has_minimap {
            let (fw, fh) = self.minimap.font_size();
            let h = if fw > 0 && fh > 0 {
                ((inner.width as u32 * fw as u32) / fh as u32) as u16
            } else {
                inner.width / 2
            };
            h.max(4)
        } else {
            0
        };

        let [
            status_area,
            economy_area,
            events_area,
            demand_area,
            ge_area,
            minimap_area,
        ] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(if has_events {
                events_rows
            } else {
                0
            }),
            Constraint::Length(if has_demand {
                (demand.len() as u16 + 1).min(26)
            } else {
                2 // header + "None"
            }),
            Constraint::Min(0),
            Constraint::Length(sq_h),
        ])
        .areas(inner);

        // ── WS status strip ───────────────────────────────────────────────
        let (ws_icon, ws_color, ws_label) = match &ws_status {
            WsStatus::Connected => ("●", Color::Green, "live"),
            WsStatus::Connecting => ("○", Color::Yellow, "connecting…"),
            WsStatus::Disconnected(_) => ("✗", Color::Red, "offline"),
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(ws_icon, Style::default().fg(ws_color)),
                Span::styled(format!(" {ws_label}"), Style::default().fg(Color::DarkGray)),
            ])),
            status_area,
        );

        // ── Economy strip ─────────────────────────────────────────────────
        let gold_str = format_gold(total_gold);
        let rate_str = match gold_per_hr {
            Some(r) if r >= 0.0 => format!("+{}/hr", format_gold(r as u64)),
            Some(r) => format!("{}/hr", format_gold(r.abs() as u64)),
            None => "—/hr".to_string(),
        };
        let rate_color = match gold_per_hr {
            Some(r) if r > 0.0 => Color::LightGreen,
            Some(r) if r < 0.0 => Color::LightRed,
            _ => Color::DarkGray,
        };

        let [gold_row, rate_row] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(economy_area);

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("◈ ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    gold_str,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ])),
            gold_row,
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(rate_str, Style::default().fg(rate_color)),
            ])),
            rate_row,
        );

        // ── World events section ──────────────────────────────────────────
        if has_events && events_area.height > 0 {
            let items: Vec<ListItem> = events
                .iter()
                .take(3)
                .map(|evt| {
                    let exp = if evt.expiration.len() > 5 {
                        &evt.expiration[..16.min(evt.expiration.len())]
                    } else {
                        &evt.expiration
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled("⚡ ", Style::default().fg(Color::LightYellow)),
                        Span::styled(
                            truncate(&evt.name, (events_area.width as usize).saturating_sub(6)),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(format!(" {exp}"), Style::default().fg(Color::DarkGray)),
                    ]))
                })
                .collect();

            let events_block = Block::default()
                .title(Span::styled("Events", Style::default().fg(Color::DarkGray)))
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray));

            frame.render_widget(List::new(items).block(events_block), events_area);
        }

        // ── Swarm Demand section ──────────────────────────────────────────
        self.demand_count = demand.len();
        self.demand_area = demand_area;

        if demand_area.height > 0 {
            let demand_block = Block::default()
                .title(Span::styled("Swarm Demand", Style::default().fg(Color::DarkGray)))
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray));
            let inner_area = demand_block.inner(demand_area);
            frame.render_widget(demand_block, demand_area);

            if !has_demand {
                if inner_area.height > 0 {
                    frame.render_widget(
                        Paragraph::new("None").style(Style::default().fg(Color::DarkGray)),
                        inner_area,
                    );
                }
            } else if inner_area.height > 0 {
                let visible_count = inner_area.height as usize;

                // Adjust scroll if terminal resizes
                let max_scroll = self
                    .demand_count
                    .saturating_sub(visible_count);
                self.demand_scroll = self.demand_scroll.min(max_scroll);

                let rows = Layout::vertical(
                    (0..visible_count)
                        .map(|_| Constraint::Length(1))
                        .collect::<Vec<_>>(),
                )
                .split(inner_area);

                for (i, (code, qty)) in demand
                    .iter()
                    .skip(self.demand_scroll)
                    .take(visible_count)
                    .enumerate()
                {
                    let row_area = rows[i];
                    let [
                        icon_area,
                        qty_area,
                        name_area,
                    ] = Layout::horizontal([
                        Constraint::Length(3), // ICON_COL_W equivalent
                        Constraint::Length(6), // e.g. " 10x  "
                        Constraint::Min(0),
                    ])
                    .areas(row_area);

                    // Fetch and render icon
                    let key = format!("items/{code}");
                    if let Some(img) = ImageCache::get_or_fetch(&self.image_cache, "items", code) {
                        self.icon_cache.ensure(&key, &img);
                    }
                    if self.icon_cache.has(&key) {
                        self.icon_cache
                            .render(&key, frame, icon_area);
                    } else {
                        frame.render_widget(
                            Paragraph::new("·").style(Style::default().fg(Color::DarkGray)),
                            icon_area,
                        );
                    }

                    // Quantity
                    frame.render_widget(
                        Paragraph::new(format!("{}x ", qty))
                            .style(Style::default().fg(Color::Yellow)),
                        qty_area,
                    );

                    // Normalised Name
                    let name = normalise_code(code);
                    frame.render_widget(
                        Paragraph::new(truncate(&name, name_area.width.saturating_sub(2) as usize))
                            .style(Style::default().fg(Color::White)),
                        name_area,
                    );
                }

                if max_scroll > 0 {
                    let scrollbar = Scrollbar::default()
                        .orientation(ScrollbarOrientation::VerticalRight)
                        .begin_symbol(Some("▲"))
                        .end_symbol(Some("▼"));
                    let mut scrollbar_state = ScrollbarState::default()
                        .content_length(max_scroll)
                        .position(self.demand_scroll);
                    let scrollbar_area = Rect::new(
                        demand_area.x,
                        demand_area.y + 1,
                        demand_area.width,
                        demand_area.height.saturating_sub(1),
                    );
                    frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
                }
            }
        }

        // ── GE feed (compact, above minimap) ─────────────────────────────
        self.ge_count = ge_feed.len();
        self.ge_area = ge_area;

        if ge_area.height > 0 {
            let ge_block = Block::default()
                .title(Span::styled("GE Feed", Style::default().fg(Color::DarkGray)))
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray));
            let ge_inner = ge_block.inner(ge_area);
            frame.render_widget(ge_block, ge_area);

            if ge_inner.height > 0 {
                if ge_feed.is_empty() {
                    frame.render_widget(
                        Paragraph::new("None").style(Style::default().fg(Color::DarkGray)),
                        ge_inner,
                    );
                } else {
                    let visible = ge_inner.height as usize;
                    let max_scroll = self.ge_count.saturating_sub(visible);
                    self.ge_scroll = self.ge_scroll.min(max_scroll);

                    let items: Vec<ListItem> = ge_feed
                        .iter()
                        .rev()
                        .skip(self.ge_scroll)
                        .take(visible)
                        .map(|entry| {
                            ge_feed_item(
                                entry,
                                ge_inner
                                    .width
                                    .saturating_sub(if max_scroll > 0 {
                                        1
                                    } else {
                                        0
                                    }),
                            )
                        })
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();

                    frame.render_widget(List::new(items), ge_inner);

                    if max_scroll > 0 {
                        let scrollbar = Scrollbar::default()
                            .orientation(ScrollbarOrientation::VerticalRight)
                            .begin_symbol(Some("▲"))
                            .end_symbol(Some("▼"));
                        let mut scrollbar_state = ScrollbarState::default()
                            .content_length(max_scroll)
                            .position(self.ge_scroll);
                        frame.render_stateful_widget(scrollbar, ge_inner, &mut scrollbar_state);
                    }
                }
            }
        }

        // ── Minimap (selected character, square area) ─────────────────────
        if has_minimap && let Some(ref ch) = sel_char {
            // Space has already been allocated precisely in minimap_area.
            let square_area = minimap_area;

            let map_block = Block::default()
                .title(Span::styled(
                    format!(" {} [{},{}] ", ch.name, ch.x, ch.y),
                    Style::default().fg(Color::DarkGray),
                ))
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray));
            let map_inner = map_block.inner(square_area);
            frame.render_widget(map_block, square_area);
            self.minimap.render(
                ch.x,
                ch.y,
                &char_layer,
                &ch.skin,
                &map_tiles,
                Some(&self.image_cache),
                frame,
                map_inner,
            );
        }

        // ── Phase 2: glitch on the same area as phase 1 ──────────────────
        // Must use `area` (not `inner`) so stored cell_idx values from
        // phase 1 are decoded against the same width.
        if elapsed_ms < SIDEBAR_BOOT_TOTAL_MS && area.width > 0 && area.height > 0 {
            let dur = FxDuration::from_millis(delta_ms.max(1));
            self.boot_glitch
                .process_effects(dur, frame.buffer_mut(), area);
        }

        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn ge_feed_item(entry: &GEFeedEntry, width: u16) -> ListItem<'static> {
    match entry {
        GEFeedEntry::Order(order) => {
            let (color, verb) = if order.order_type == "sell" {
                (Color::LightRed, "SELL")
            } else {
                (Color::LightGreen, "BUY ")
            };
            let max_code = (width as usize).saturating_sub(16);
            ListItem::new(Line::from(vec![
                Span::styled(format!("[{verb}] "), Style::default().fg(color)),
                Span::styled(truncate(&order.code, max_code), Style::default().fg(Color::White)),
                Span::styled(
                    format!(" ×{}@{}g", order.quantity, order.price),
                    Style::default().fg(Color::Yellow),
                ),
            ]))
        }
        GEFeedEntry::Transaction(txn) => {
            let (color, verb) = if txn.order_type == "sell" {
                (Color::Green, "SOLD")
            } else {
                (Color::Cyan, "BGHT")
            };
            let max_code = (width as usize).saturating_sub(16);
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("[{verb}] "),
                    Style::default()
                        .fg(color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(truncate(&txn.code, max_code), Style::default().fg(Color::White)),
                Span::styled(
                    format!(" ×{}={}g", txn.quantity, txn.total_price),
                    Style::default().fg(Color::Yellow),
                ),
            ]))
        }
    }
}

/// Format a gold value with thousands separators: e.g. 1_234_567 → "1,234,567".
fn format_gold(g: u64) -> String {
    let s = g.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        chars[..max.saturating_sub(1)]
            .iter()
            .collect::<String>()
            + "…"
    }
}
