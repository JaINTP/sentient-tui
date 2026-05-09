//! Footer log panel component — scrolling list of game action events.
//!
//! Shows the global action log (fights, crafting, trading, etc.) with color-coded tags.
//! The newest entry briefly glitches when it arrives.
//!
//! Boot animation: 0–150 ms border only (heavy glitch), 150–350 ms content visible (light glitch).

use std::sync::{Arc, RwLock};
use std::time::Instant;

use crossterm::event::{MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use tachyonfx::fx::Glitch;
use tachyonfx::{Duration as FxDuration, Effect, EffectManager};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{
    core::action::Action,
    core::config::Config,
    core::game::{FocusedPanel, GameState, LogEntry},
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

    // ── Filtering state ───────────────────────────────────────────────────
    /// When true, only entries whose character matches the selected bot are shown.
    pub filter_active: bool,

    // ── Scroll state ──────────────────────────────────────────────────────
    /// Rows scrolled back from the bottom (0 = pinned to most recent).
    log_scroll: usize,
    /// Last rendered area — stored so the mouse handler can check containment.
    log_area: Rect,

    // ── Boot animation ────────────────────────────────────────────────────
    /// Timestamp of component creation — drives boot animation timing.
    boot_at: Instant,
    /// Separate effect manager for the boot animation (independent from entry glitches).
    boot_glitch: EffectManager<&'static str>,
}

impl LogPanel {
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
            filter_active: false,
            log_scroll: 0,
            log_area: Rect::default(),
            boot_at: Instant::now(),
            boot_glitch,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

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

    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> color_eyre::Result<Option<Action>> {
        let inside = mouse.column >= self.log_area.x
            && mouse.column < self.log_area.x + self.log_area.width
            && mouse.row >= self.log_area.y
            && mouse.row < self.log_area.y + self.log_area.height;
        if !inside {
            return Ok(None);
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.log_scroll = self.log_scroll.saturating_add(3);
                Ok(Some(Action::Tick))
            }
            MouseEventKind::ScrollDown => {
                self.log_scroll = self.log_scroll.saturating_sub(3);
                Ok(Some(Action::Tick))
            }
            _ => Ok(None),
        }
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::ToggleLog => self.visible = !self.visible,
            Action::FilterLog => self.filter_active = !self.filter_active,
            Action::FocusNext => {
                if self.game_state.read().unwrap().focused_panel == FocusedPanel::LogPanel {
                    self.log_scroll = self.log_scroll.saturating_sub(1);
                }
            }
            Action::FocusPrev => {
                if self.game_state.read().unwrap().focused_panel == FocusedPanel::LogPanel {
                    self.log_scroll = self.log_scroll.saturating_add(1);
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        if !self.visible {
            return Ok(());
        }

        self.log_area = area;

        let boot_elapsed_ms = self.boot_at.elapsed().as_millis() as u64;
        let elapsed = self.last_tick.elapsed();
        self.last_tick = Instant::now();
        let elapsed_ms = elapsed.as_millis() as u32;

        // ── Single snapshot read: entries + filter name + focus state ─────
        let (entries, filter_name, focused) = {
            let gs = self.game_state.read().unwrap();
            let focused = gs.focused_panel == FocusedPanel::LogPanel;
            let filter_name = if self.filter_active {
                let idx = gs.selected_character.min(gs.characters.len().saturating_sub(1));
                gs.characters.get(idx).map(|c| c.name.clone())
            } else {
                None
            };
            let entries: Vec<LogEntry> = gs.log_entries.iter().cloned().collect();
            (entries, filter_name, focused)
        };

        // ── Build title and block ─────────────────────────────────────────
        let title_text = match filter_name.as_deref() {
            Some(name) => format!(" Log — {name} [L/F] "),
            None if self.filter_active => " Log [L/F] ".to_string(),
            _ => " Log [L] [F] ".to_string(),
        };
        let border_color = if focused { Color::Cyan } else { Color::DarkGray };
        let block = Block::default()
            .title(Span::styled(title_text, Style::default().fg(Color::DarkGray)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
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

        // Detect new arrivals on the unfiltered total to arm the glitch.
        if entries.len() > self.last_entry_count {
            self.arm_glitch();
        }
        self.last_entry_count = entries.len();

        // ── Scroll clamping ───────────────────────────────────────────────
        let passes = |e: &&LogEntry| {
            filter_name
                .as_deref()
                .map_or(true, |name| e.character.is_empty() || e.character == name)
        };
        let filtered_count = entries.iter().filter(passes).count();
        let visible_rows = inner.height as usize;
        let max_scroll = filtered_count.saturating_sub(visible_rows);
        self.log_scroll = self.log_scroll.min(max_scroll);

        // Reserve one column for the scrollbar when it is visible.
        let content_width = inner.width.saturating_sub(if max_scroll > 0 { 1 } else { 0 });

        // ── Build list items (most recent at bottom) ──────────────────────
        let items: Vec<ListItem> = entries
            .iter()
            .rev()
            .filter(passes)
            .skip(self.log_scroll)
            .take(visible_rows)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|e| render_entry(e, content_width))
            .collect();

        frame.render_widget(List::new(items).block(block), area);

        // ── Scrollbar ─────────────────────────────────────────────────────
        if max_scroll > 0 {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"));
            let mut scrollbar_state = ScrollbarState::default()
                .content_length(max_scroll)
                .position(self.log_scroll);
            frame.render_stateful_widget(scrollbar, inner, &mut scrollbar_state);
        }

        // ── Boot phase 2 ──────────────────────────────────────────────────
        if boot_elapsed_ms < LOG_BOOT_TOTAL_MS && area.width > 0 && area.height > 0 {
            let dur = FxDuration::from_millis(elapsed_ms.max(1));
            self.boot_glitch
                .process_effects(dur, frame.buffer_mut(), area);
        }

        // ── Per-entry glitch on the newest (bottom-most) visible line ─────
        if self.glitch_timer_ms > 0 && !entries.is_empty() {
            let shown = filtered_count.min(visible_rows);
            let newest_y = inner.y + (shown as u16).saturating_sub(1);
            if newest_y < inner.y + inner.height {
                let row = Rect::new(inner.x, newest_y, inner.width, 1);
                let dur = FxDuration::from_millis(elapsed_ms);
                self.fx_manager
                    .process_effects(dur, frame.buffer_mut(), row);
            }
            self.glitch_timer_ms = self.glitch_timer_ms.saturating_sub(elapsed_ms);
            if self.glitch_timer_ms == 0 {
                self.fx_manager = EffectManager::default();
            }
        }

        Ok(())
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

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
