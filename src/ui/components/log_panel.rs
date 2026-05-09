//! Footer log panel component — scrolling list of game action events.
//!
//! Shows the global action log (fights, crafting, trading, etc.) with color-coded tags.
//! The newest entry briefly glitches when it arrives.
//!
//! Boot animation: 0–150 ms border only (heavy glitch), 150–350 ms content visible (light glitch).

use std::sync::{Arc, RwLock};
use std::time::Instant;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};
use tachyonfx::fx::Glitch;
use tachyonfx::{Duration as FxDuration, Effect, EffectManager};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{
    core::action::Action,
    core::config::Config,
    core::game::{GameState, LogEntry},
};

/// Duration the glitch effect runs on newly arrived entries (ms).
const GLITCH_DURATION_MS: u32 = 350;

/// Boot animation duration (ms).
const LOG_BOOT_TOTAL_MS: u64 = 350;
/// Border-only phase during boot (inner hidden, heavy glitch).
const LOG_BORDER_PHASE_MS: u64 = 150;

/// Footer log panel — scrolling list of all game action events.
pub struct LogPanel {
    /// Action bus sender.
    command_tx: Option<UnboundedSender<Action>>,
    /// Global configuration.
    config: Config,
    /// Shared game state — read to get log entries.
    game_state: Arc<RwLock<GameState>>,
    /// Panel visibility flag (can be toggled).
    pub visible: bool,
    /// Milliseconds remaining on the newest-entry glitch effect (0 = inactive).
    glitch_timer_ms: u32,
    /// Glitch effect manager for per-entry arrivals.
    fx_manager: EffectManager<&'static str>,
    /// Timestamp of last render — used to compute per-frame delta.
    last_tick: Instant,
    /// Log entry count from last draw — used to detect new arrivals.
    last_entry_count: usize,

    // ── Boot animation ────────────────────────────────────────────────────
    /// Timestamp of component creation — drives boot animation timing.
    boot_at: Instant,
    /// Separate effect manager for the boot animation (independent from entry glitches).
    boot_glitch: EffectManager<&'static str>,
}

impl LogPanel {
    /// Create a new [`LogPanel`] bound to `game_state`.
    ///
    /// The panel starts visible, with its boot animation beginning immediately.
    /// Pass the shared [`GameState`] that is written by the WebSocket handler.
    pub fn new(game_state: Arc<RwLock<GameState>>) -> Self {
        let mut boot_glitch = EffectManager::default();
        boot_glitch.add_unique_effect(
            "boot",
            Effect::new(
                Glitch::builder()
                    .cell_glitch_ratio(0.65)
                    .action_start_delay_ms(0..15)
                    .action_ms(12..65)
                    .build(),
            ),
        );
        Self {
            command_tx: None,
            config: Config::default(),
            game_state,
            visible: true,
            glitch_timer_ms: 0,
            fx_manager: EffectManager::default(),
            last_tick: Instant::now(),
            last_entry_count: 0,
            boot_at: Instant::now(),
            boot_glitch,
        }
    }

    /// Return `true` if the panel is currently set to render.
    ///
    /// Toggled by [`Action::ToggleLog`].
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Prime the per-entry glitch effect for the next [`GLITCH_DURATION_MS`] milliseconds.
    ///
    /// Called whenever new log entries arrive.  The glitch is applied only to
    /// the bottom-most visible row (the most recently arrived entry).
    fn arm_glitch(&mut self) {
        self.glitch_timer_ms = GLITCH_DURATION_MS;
        let glitch = Glitch::builder()
            .cell_glitch_ratio(0.35)
            .action_start_delay_ms(0..15)
            .action_ms(25..120)
            .build();
        self.fx_manager
            .add_unique_effect("newest_line", Effect::new(glitch));
    }
}

impl Component for LogPanel {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        if let Action::ToggleLog = action {
            self.visible = !self.visible;
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        if !self.visible {
            return Ok(());
        }

        let boot_elapsed_ms = self.boot_at.elapsed().as_millis() as u64;

        // ── Timing delta (shared between boot and entry glitch) ───────────
        let elapsed = self.last_tick.elapsed();
        self.last_tick = Instant::now();
        let elapsed_ms = elapsed.as_millis() as u32;

        // ── Block + inner ─────────────────────────────────────────────────
        let block = Block::default()
            .title(Span::styled(" Log [L] ", Style::default().fg(Color::DarkGray)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);

        // ── Phase 1: border-only flicker ──────────────────────────────────
        if boot_elapsed_ms < LOG_BORDER_PHASE_MS {
            frame.render_widget(block, area);
            if area.width > 0 && area.height > 0 {
                let dur = FxDuration::from_millis(elapsed_ms.max(1));
                self.boot_glitch
                    .process_effects(dur, frame.buffer_mut(), area);
            }
            return Ok(());
        }

        // ── Brief read snapshot ───────────────────────────────────────────
        let entries: Vec<LogEntry> = {
            let gs = self.game_state.read().unwrap();
            gs.log_entries.iter().cloned().collect()
        };

        // Detect new arrivals to arm the per-entry glitch
        if entries.len() > self.last_entry_count {
            self.arm_glitch();
        }
        self.last_entry_count = entries.len();

        // ── Build list items (most recent at bottom) ──────────────────────
        let visible_rows = inner.height as usize;
        let items: Vec<ListItem> = entries
            .iter()
            .rev()
            .take(visible_rows)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|e| render_entry(e, inner.width))
            .collect();

        frame.render_widget(List::new(items).block(block), area);

        // ── Boot phase 2: glitch on the same area as phase 1 ────────────────
        // IMPORTANT: must use `area` (not `inner`) so stored cell_idx values
        // from phase 1 are decoded against the same width and never produce
        // y-offsets that land outside the terminal buffer.
        if boot_elapsed_ms < LOG_BOOT_TOTAL_MS && area.width > 0 && area.height > 0 {
            let dur = FxDuration::from_millis(elapsed_ms.max(1));
            self.boot_glitch
                .process_effects(dur, frame.buffer_mut(), area);
        }

        // ── Per-entry glitch on the newest (bottom-most) visible line ─────
        if self.glitch_timer_ms > 0 && !entries.is_empty() {
            let newest_y = inner.y + (visible_rows.min(entries.len()) as u16).saturating_sub(1);
            if newest_y < inner.y + inner.height {
                let row = Rect::new(inner.x, newest_y, inner.width, 1);
                let dur = FxDuration::from_millis(elapsed_ms);
                self.fx_manager
                    .process_effects(dur, frame.buffer_mut(), row);
            }
            self.glitch_timer_ms = self
                .glitch_timer_ms
                .saturating_sub(elapsed_ms);
            if self.glitch_timer_ms == 0 {
                self.fx_manager = EffectManager::default();
            }
        }

        Ok(())
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Convert a [`LogEntry`] to a coloured ratatui [`ListItem`].
///
/// Format: `<tag> <character>: <message>` where `tag` is bold and coloured
/// according to [`LogEntry::tag_color`], the character name is bold white, and
/// the message is grey.  The message is truncated to fit within `width`.
fn render_entry(entry: &LogEntry, width: u16) -> ListItem<'static> {
    let tag = entry.tag;
    let tag_color = entry.tag_color;
    let char_part = if entry.character.is_empty() {
        String::new()
    } else {
        format!("{}: ", entry.character)
    };
    let max_msg = (width as usize).saturating_sub(tag.len() + 1 + char_part.len());

    let line = Line::from(vec![
        Span::styled(
            format!("{tag} "),
            Style::default()
                .fg(tag_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            char_part,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(truncate(&entry.message, max_msg), Style::default().fg(Color::Gray)),
    ]);
    ListItem::new(line)
}

/// Truncate `s` to at most `max` Unicode scalar values, appending `…` if cut.
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
