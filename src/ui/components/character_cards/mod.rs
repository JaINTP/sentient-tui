//! Character card grid component — 3-column layout of animated character status panels.
//!
//! Each card shows portrait, HP/XP/bag gauges, cooldown timer, skills grid,
//! combat stats, equipment table, action history, and a goal/task line.  Cards
//! are arranged in a [`N_COLS`]-column responsive grid that expands sections as
//! vertical space permits.
//!
//! ## Boot animation
//!
//! When a character card first appears it runs a ~580 ms two-phase sequence:
//!
//! 1. **Border phase** (0–240 ms) — only the outer border is drawn; a
//!    tachyonfx [`Glitch`] effect covers the full card area.
//! 2. **Reveal phase** (240–580 ms) — individual content elements fade in at
//!    pseudo-random offsets computed from the character name
//!    (see [`element_reveal_ms`]).
//!
//! ## Border colours
//!
//! The card border colour reflects the character's `last_action`:
//! Fighting → Red | Gathering → Green | Moving → Blue | Crafting → Yellow |
//! Idle/selected → Cyan | Idle/unselected → DarkGray.
//!
//! The selected card additionally has an animated "shooting star" pulse drawn
//! on top of its border via [`apply_animated_border`].
//!
//! [`Glitch`]: tachyonfx::fx::Glitch
//! [`element_reveal_ms`]: animation::element_reveal_ms

pub mod animation;
pub mod card;
pub mod gear;
pub mod skills;
pub mod stats;
pub mod utils;

use animation::{CARD_BOOT_TOTAL_MS, apply_animated_border, arm_card_glitch};

/// 3-column grid of animated character status cards.
///
/// Layout per card (adaptive, tallest sections shown when space allows):
///   Header    : ♥ Name  Lv N  [x,y]  Ng
///   Portrait  : character skin image (left column, always — fades in when downloaded)
///   Gauges    : HP / XP / Bag  (always, to the right of portrait when wide enough)
///   Minimap   : ratatui-image tile view around character position (≥ 14 rows)
///   Skills    : 4-row × 2-col grid with XP gauges  (≥ 20 rows)
///   Gear      : 2-col equipment table with item icons when downloaded  (≥ 28 rows)
///   Status    : signal line + task/goal (always)
///
/// Border colour reflects character state:
///   Fighting → Red  |  Gathering → Green  |  Moving → Blue
///   Crafting → Yellow  |  Idle/default → DarkGray
///
/// Image integration:
///   - Character skin:  fetched as `characters/{skin}.png`, shown as portrait column
///   - Item icons:      fetched as `items/{code}.png`, shown as 3-col thumbnails in gear
///
/// Boot animation:
///   Each card runs a ~580 ms boot sequence when it first appears:
///     Phase 1 (0–240 ms)  — border only, heavy glitch on full card area
///     Phase 2 (240–580 ms)— elements reveal in random order through lighter inner glitch
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};
use tachyonfx::{Duration as FxDuration, EffectManager};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{
    core::action::Action,
    core::config::Config,
    core::game::{FocusedPanel, GameState},
    ui::image_cache::{ProtocolCache, SharedImageCache},
};

/// Number of card columns in the grid.
const N_COLS: usize = 3;

/// Minimum card width (in terminal columns) required to show the portrait sidebar.
const PORTRAIT_MIN_WIDTH: u16 = 20;

/// Width of the portrait column in terminal characters.
const PORTRAIT_COL_W: u16 = 7;

/// Width of the icon thumbnail column used in the stats, skills, and gear sub-panels.
const ICON_COL_W: u16 = 3;

/// Character card grid component — displays up to 6 characters in 3 columns.
///
/// Each card shows portrait, HP/XP/bag gauges, cooldown timer, skills, equipment,
/// and action history. Cards animate on first appearance with a glitch effect.
pub struct CharacterCards {
    /// Action bus sender — passed to cards during event handling.
    command_tx: Option<UnboundedSender<Action>>,
    /// Global configuration — keybindings and styles.
    config: Config,
    /// Shared game state — read to get character data.
    game_state: Arc<RwLock<GameState>>,
    /// Shared image cache for character skins and item icons.
    image_cache: SharedImageCache,
    /// Terminal rendering protocol cache for character portrait images.
    portraits: ProtocolCache,
    /// Terminal rendering protocol cache for item icon images.
    item_icons: ProtocolCache,
    /// Terminal rendering protocol cache for stat effect images.
    stat_icons: ProtocolCache,
    /// Terminal rendering protocol cache for skill icon images.
    skill_icons: ProtocolCache,
    /// Index of currently selected character (0-based, for border highlighting).
    selected: usize,
    /// Whether the selected character card is maximized.
    pub maximized: bool,

    // ── Boot animation state ──────────────────────────────────────────────
    /// Timestamp when each character card first appeared (for boot animation timing).
    card_born: HashMap<String, Instant>,
    /// Per-card glitch effect manager (keyed by character name).
    card_glitch: HashMap<String, EffectManager<&'static str>>,
    /// Timestamp of the last render call — used to compute per-frame delta.
    last_render_tick: Instant,
    /// Timestamp of component creation — absolute reference for persistent animations.
    app_start: Instant,
}

impl CharacterCards {
    /// Create a new [`CharacterCards`] component.
    ///
    /// - `game_state` — shared game state; the component takes a read lock
    ///   on each render frame to snapshot character data.
    /// - `image_cache` — shared image store used for portrait, item, stat,
    ///   and skill icon downloads.
    pub fn new(game_state: Arc<RwLock<GameState>>, image_cache: SharedImageCache) -> Self {
        Self {
            command_tx: None,
            config: Config::default(),
            game_state,
            image_cache,
            portraits: ProtocolCache::new(),
            item_icons: ProtocolCache::new(),
            stat_icons: ProtocolCache::new(),
            skill_icons: ProtocolCache::new(),
            selected: 0,
            maximized: false,
            card_born: HashMap::new(),
            card_glitch: HashMap::new(),
            last_render_tick: Instant::now(),
            app_start: Instant::now(),
        }
    }
}

impl Component for CharacterCards {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }
    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::FocusNext => {
                let gs = self.game_state.read().unwrap();
                if gs.focused_panel == FocusedPanel::CharGrid && !gs.characters.is_empty() {
                    self.selected = (self.selected + 1) % gs.characters.len();
                    drop(gs);
                    self.game_state.write().unwrap().selected_character = self.selected;
                }
            }
            Action::FocusPrev => {
                let gs = self.game_state.read().unwrap();
                if gs.focused_panel == FocusedPanel::CharGrid && !gs.characters.is_empty() {
                    self.selected = if self.selected == 0 {
                        gs.characters.len() - 1
                    } else {
                        self.selected - 1
                    };
                    drop(gs);
                    self.game_state.write().unwrap().selected_character = self.selected;
                }
            }
            Action::MaximizeCharacter => {
                self.maximized = !self.maximized;
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        // ── Brief read snapshot ───────────────────────────────────────────
        let (characters, cooldown_expires, cooldown_totals, action_history, heartbeat) = {
            let gs = self.game_state.read().unwrap();
            (
                gs.characters.clone(),
                gs.cooldown_expires.clone(),
                gs.cooldown_totals.clone(),
                gs.action_history.clone(),
                gs.heartbeat,
            )
        };

        if characters.is_empty() {
            frame.render_widget(
                Paragraph::new("Waiting for character data…\nMake sure ARTIFACTS_TOKEN is set.")
                    .block(
                        Block::default()
                            .title(" Characters ")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    )
                    .style(Style::default().fg(Color::DarkGray)),
                area,
            );
            return Ok(());
        }

        let selected = self
            .selected
            .min(characters.len().saturating_sub(1));

        // ── Per-frame timing (used to advance glitch effects) ─────────────
        let delta_ms = self
            .last_render_tick
            .elapsed()
            .as_millis() as u32;
        self.last_render_tick = Instant::now();

        // Register any new characters into the boot-animation tracking maps.
        for char in &characters {
            if !self.card_born.contains_key(&char.name) {
                self.card_born
                    .insert(char.name.clone(), Instant::now());
                self.card_glitch
                    .insert(char.name.clone(), arm_card_glitch());
            }
        }

        let selected_idx = self
            .selected
            .min(characters.len().saturating_sub(1));

        let char = &characters[selected_idx];

        let cd_rem = cooldown_expires
            .get(&char.name)
            .map(|exp| {
                let now = Instant::now();
                if *exp > now {
                    (*exp - now).as_secs_f64()
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        let cd_tot = cooldown_totals
            .get(&char.name)
            .copied()
            .unwrap_or(1.0)
            .max(1.0);

        if self.maximized {
            // bypass grid logic entirely
            let elapsed_ms = self
                .card_born
                .get(&char.name)
                .map(|t| t.elapsed().as_millis() as u64)
                .unwrap_or(u64::MAX);

            let history = action_history
                .get(&char.name)
                .cloned()
                .unwrap_or_default();

            // draw just the one card using the FULL area
            self.draw_card(
                frame, area, char, &history, cd_rem, cd_tot, heartbeat, true, elapsed_ms,
            );

            // don't forget to apply the animated border to the full area too!
            return Ok(());
        }

        // ── Compute 3-column grid layout ──────────────────────────────────
        let n = characters.len();
        let n_rows = n.div_ceil(N_COLS);

        let row_constraints: Vec<Constraint> = (0..n_rows)
            .map(|_| Constraint::Ratio(1, n_rows as u32))
            .collect();
        let row_areas = Layout::vertical(row_constraints).split(area);

        // Collect (char_idx, card_rect, elapsed_ms) for each visible card.
        let mut card_layout: Vec<(usize, Rect, u64)> = Vec::with_capacity(n);

        for row_idx in 0..n_rows {
            let chars_in_row = ((row_idx + 1) * N_COLS).min(n) - row_idx * N_COLS;
            let col_constraints: Vec<Constraint> = (0..chars_in_row)
                .map(|_| Constraint::Ratio(1, chars_in_row as u32))
                .collect();
            let col_areas = Layout::horizontal(col_constraints).split(row_areas[row_idx]);

            for col_idx in 0..chars_in_row {
                let char_idx = row_idx * N_COLS + col_idx;
                if char_idx >= n {
                    break;
                }
                let elapsed_ms = self
                    .card_born
                    .get(&characters[char_idx].name)
                    .map(|t| t.elapsed().as_millis() as u64)
                    .unwrap_or(u64::MAX);
                card_layout.push((char_idx, col_areas[col_idx], elapsed_ms));
            }
        }

        // ── Draw each card ────────────────────────────────────────────────
        for &(char_idx, card_area, elapsed_ms) in &card_layout {
            let char = &characters[char_idx];
            let history = action_history
                .get(&char.name)
                .cloned()
                .unwrap_or_default();

            self.draw_card(
                frame,
                card_area,
                char,
                &history,
                cd_rem,
                cd_tot,
                heartbeat,
                char_idx == selected,
                elapsed_ms,
            );
        }

        // ── Apply per-card glitch effects (post-render pass) ──────────────
        // Take the map out so we can borrow frame.buffer_mut() simultaneously.
        let mut card_glitch = std::mem::take(&mut self.card_glitch);
        let fx_dur = FxDuration::from_millis(delta_ms);

        for &(char_idx, card_area, elapsed_ms) in &card_layout {
            let char_name = &characters[char_idx].name;
            if elapsed_ms >= CARD_BOOT_TOTAL_MS {
                continue; // animation complete
            }
            if let Some(fx) = card_glitch.get_mut(char_name) {
                // Always use card_area (the same rect for both phases).
                // tachyonfx's Glitch stores cell_idx values generated against
                // area.width.  If we switched to a smaller inner rect in phase 2,
                // old indices would compute y = cell_idx / inner.width that
                // exceeds inner.height, making area.y + y fall outside the
                // terminal buffer and causing a cell_mut().unwrap() panic.
                if card_area.width > 0 && card_area.height > 0 {
                    fx.process_effects(fx_dur, frame.buffer_mut(), card_area);
                }
            }
        }

        self.card_glitch = card_glitch;

        // ── Apply animated border to selected card ────────────────────────
        let total_elapsed_ms = self.app_start.elapsed().as_millis() as u64;
        for &(char_idx, card_area, _) in &card_layout {
            if char_idx == selected {
                let char = &characters[char_idx];
                let border_color = match char.last_action.as_str() {
                    "fight" | "multi_fight" => Color::Red,
                    "gathering" => Color::Green,
                    "movement" => Color::Blue,
                    "crafting" => Color::Yellow,
                    _ => Color::Cyan, // default base colour for selection
                };
                apply_animated_border(
                    frame.buffer_mut(),
                    card_area,
                    border_color,
                    total_elapsed_ms,
                );
            }
        }

        Ok(())
    }
}

// ── CharacterState helpers ────────────────────────────────────────────────────
