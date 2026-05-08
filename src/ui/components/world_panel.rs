//! World panel component — standalone view of world events and Grand Exchange activity.
//!
//! Provides a self-contained split layout showing:
//! - **Active world events** — monsters or hazards currently spawned on the map.
//! - **Grand Exchange feed** — a rolling history of recent orders and completed trades.
//!
//! This component maintains its own local copy of events and GE entries,
//! updated via the [`Action`] bus.  It is currently not wired into the primary
//! layout (the sidebar handles the same data in the main view), but is available
//! as a drop-in replacement panel when a full-screen world overview is needed.

use std::collections::VecDeque;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{
    core::action::Action,
    core::config::Config,
    core::game::{GEOrder, GETransaction, WorldEvent},
};

/// Maximum number of world events retained before the oldest is evicted.
const MAX_EVENTS: usize = 20;
/// Maximum number of GE feed entries retained before the oldest is evicted.
const MAX_GE: usize = 30;

/// Standalone world-events and Grand Exchange panel.
///
/// Tracks its own state rather than reading from [`crate::core::game::GameState`],
/// making it suitable for use in an independent pane without holding a read lock
/// during rendering.
pub struct WorldPanel {
    /// Action bus sender — stored for future event dispatch.
    command_tx: Option<UnboundedSender<Action>>,
    /// Global configuration — keybindings and styles.
    config: Config,
    /// Currently active world events.  Entries are added on `EventSpawn` and
    /// removed on `EventRemoved`.
    active_events: Vec<WorldEvent>,
    /// Rolling Grand Exchange feed — orders and transactions, newest at the back.
    ge_feed: VecDeque<GEFeedEntry>,
    /// Current WebSocket connection state, used to render a status indicator.
    ws_status: WsStatus,
}

/// Local copy of WebSocket connection state for the panel's status badge.
#[derive(Debug, Clone)]
enum WsStatus {
    /// Awaiting initial connection or reconnecting after a drop.
    Connecting,
    /// Authenticated and receiving events.
    Connected,
    /// Connection lost; contains the disconnect reason string.
    #[allow(dead_code)]
    Disconnected(String),
}

/// A single Grand Exchange feed entry — either a new order or a completed trade.
#[derive(Debug, Clone)]
enum GEFeedEntry {
    /// A buy or sell order was posted to the Grand Exchange.
    Order(GEOrder),
    /// A Grand Exchange order was fully or partially filled.
    Transaction(GETransaction),
}

impl Default for WorldPanel {
    fn default() -> Self {
        Self {
            command_tx: None,
            config: Config::default(),
            active_events: Vec::new(),
            ge_feed: VecDeque::with_capacity(MAX_GE),
            ws_status: WsStatus::Connecting,
        }
    }
}

impl WorldPanel {
    /// Create a new [`WorldPanel`] in the connecting state with empty feeds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a GE feed entry, evicting the oldest when the buffer is full.
    fn push_ge(&mut self, entry: GEFeedEntry) {
        if self.ge_feed.len() >= MAX_GE {
            self.ge_feed.pop_front();
        }
        self.ge_feed.push_back(entry);
    }
}

impl Component for WorldPanel {
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
            Action::WsConnected => {
                self.ws_status = WsStatus::Connected;
            }
            Action::WsDisconnected(ref reason) => {
                self.ws_status = WsStatus::Disconnected(reason.clone());
            }
            Action::WsConnect => {
                self.ws_status = WsStatus::Connecting;
            }
            Action::EventSpawn(ref evt) => {
                if self.active_events.len() >= MAX_EVENTS {
                    self.active_events.remove(0);
                }
                self.active_events.push(evt.clone());
            }
            Action::EventRemoved(ref code) => {
                self.active_events
                    .retain(|e| e.code != *code);
            }
            Action::GEOrderCreated(ref order) => {
                self.push_ge(GEFeedEntry::Order(order.clone()));
            }
            Action::GETransactionCompleted(ref txn) => {
                self.push_ge(GEFeedEntry::Transaction(txn.clone()));
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        // ── Status indicator + outer block ────────────────────────────────
        let (status_text, status_color) = match &self.ws_status {
            WsStatus::Connected => ("● connected", Color::Green),
            WsStatus::Connecting => ("○ connecting…", Color::Yellow),
            WsStatus::Disconnected(_) => ("✕ disconnected", Color::Red),
        };

        let outer = Block::default()
            .title(format!(" World  {} ", status_text))
            .title_style(Style::default().fg(status_color))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        // Split: top = active events, bottom = GE feed
        let event_rows = (self.active_events.len() as u16 + 1).min(8); // header + entries
        let [events_area, ge_area] = Layout::vertical([
            Constraint::Length(event_rows + 1),
            Constraint::Min(0),
        ])
        .areas(inner);

        // ── Active events ─────────────────────────────────────────────────
        let events_block = Block::default()
            .title(" Events ")
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray));
        let events_inner = events_block.inner(events_area);
        frame.render_widget(events_block, events_area);

        if self.active_events.is_empty() {
            frame.render_widget(
                Paragraph::new("no active events").style(Style::default().fg(Color::DarkGray)),
                events_inner,
            );
        } else {
            let visible = events_inner.height as usize;
            let lines: Vec<Line> = self
                .active_events
                .iter()
                .rev()
                .take(visible)
                .map(|e| {
                    Line::from(vec![
                        Span::styled("⚑ ", Style::default().fg(Color::LightGreen)),
                        Span::styled(e.name.clone(), Style::default().fg(Color::White)),
                        Span::styled(
                            format!(" ({})", e.code),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ])
                })
                .collect();
            frame.render_widget(Paragraph::new(lines), events_inner);
        }

        // ── GE feed ───────────────────────────────────────────────────────
        let ge_block = Block::default()
            .title(" Grand Exchange ")
            .borders(Borders::NONE)
            .border_style(Style::default().fg(Color::DarkGray));
        let ge_inner = ge_block.inner(ge_area);
        frame.render_widget(ge_block, ge_area);

        if self.ge_feed.is_empty() {
            frame.render_widget(
                Paragraph::new("no GE activity yet").style(Style::default().fg(Color::DarkGray)),
                ge_inner,
            );
        } else {
            let visible = ge_inner.height as usize;
            let lines: Vec<Line> = self
                .ge_feed
                .iter()
                .rev()
                .take(visible)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|entry| match entry {
                    GEFeedEntry::Order(o) => {
                        let (col, verb) = if o.order_type == "sell" {
                            (Color::LightRed, "SELL")
                        } else {
                            (Color::LightGreen, "BUY ")
                        };
                        Line::from(vec![
                            Span::styled(
                                format!("{verb} "),
                                Style::default()
                                    .fg(col)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(o.code.clone(), Style::default().fg(Color::White)),
                            Span::styled(
                                format!(" ×{} @{}g", o.quantity, o.price),
                                Style::default().fg(Color::Yellow),
                            ),
                            o.account
                                .as_deref()
                                .map(|a| {
                                    Span::styled(
                                        format!(" {}", a),
                                        Style::default().fg(Color::DarkGray),
                                    )
                                })
                                .unwrap_or_else(|| Span::raw("")),
                        ])
                    }
                    GEFeedEntry::Transaction(t) => {
                        let (col, verb) = if t.order_type == "sell" {
                            (Color::Green, "SOLD")
                        } else {
                            (Color::Cyan, "BGHT")
                        };
                        Line::from(vec![
                            Span::styled(
                                format!("{verb} "),
                                Style::default()
                                    .fg(col)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(t.code.clone(), Style::default().fg(Color::White)),
                            Span::styled(
                                format!(" ×{}={}g", t.quantity, t.total_price),
                                Style::default().fg(Color::Yellow),
                            ),
                        ])
                    }
                })
                .collect();
            frame.render_widget(Paragraph::new(lines), ge_inner);
        }

        Ok(())
    }
}
