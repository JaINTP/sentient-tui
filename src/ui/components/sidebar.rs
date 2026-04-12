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

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use tachyonfx::fx::Glitch;
use tachyonfx::{Duration as FxDuration, Effect, EffectManager};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{
    core::action::Action,
    core::config::Config,
    core::game::{GEFeedEntry, GameState, WsStatus},
    ui::image_cache::SharedImageCache,
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

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let elapsed_ms = self.born_at.elapsed().as_millis() as u64;
        let delta_ms = self.last_tick.elapsed().as_millis() as u32;
        self.last_tick = Instant::now();

        // ── Outer block ───────────────────────────────────────────────────
        let version = env!("CARGO_PKG_VERSION");
        let block = Block::default()
            .title(format!(" Info v{} ", version))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
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
        let (ws_status, total_gold, gold_per_hr, events, ge_feed, sel_char, map_tiles, char_layer) = {
            let gs = self.game_state.read().unwrap();
            let total_gold: u64 = gs
                .characters
                .iter()
                .map(|c| c.gold as u64)
                .sum();
            let gold_per_hr = gs.gold_per_hour();
            let events = gs.world_events.clone();
            let ge_feed: Vec<GEFeedEntry> = gs.ge_feed.iter().cloned().collect();
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
            (ws_status, total_gold, gold_per_hr, events, ge_feed, sel_char, map_tiles, char_layer)
        };

        // ── Layout: status(1) + economy(2) + events(dynamic) + ge(compact) + minimap(fill) ──
        let events_rows = (events.len().min(3) as u16).max(1) + 1; // +1 for header
        let has_events = !events.is_empty();
        // Show minimap whenever we have a character and enough room (≥12 rows).
        let has_minimap = sel_char.is_some() && inner.height >= 12;
        // Compact GE feed: only show when there's a minimap; a small fixed window.
        let ge_rows: u16 = if has_minimap {
            5
        } else {
            0
        };

        let [
            status_area,
            economy_area,
            events_area,
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
            Constraint::Length(ge_rows),
            Constraint::Fill(1),
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

        // ── GE feed (compact, above minimap) ─────────────────────────────
        if ge_rows > 0 && ge_area.height > 0 {
            let visible = ge_area.height as usize;
            let items: Vec<ListItem> = ge_feed
                .iter()
                .rev()
                .take(visible)
                .map(|entry| ge_feed_item(entry, ge_area.width))
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();

            let ge_block = Block::default()
                .title(Span::styled("GE Feed", Style::default().fg(Color::DarkGray)))
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray));

            frame.render_widget(List::new(items).block(ge_block), ge_area);
        }

        // ── Minimap (selected character, square area) ─────────────────────
        if has_minimap && let Some(ref ch) = sel_char {
            // Compute height so the minimap is square in PIXEL space.
            // Terminal characters are typically ~2× taller than wide, so a
            // character-square area would be a tall rectangle in pixels.
            let (fw, fh) = self.minimap.font_size();
            let sq_h = if fw > 0 && fh > 0 {
                // pixel_width = minimap_area.width * fw
                // pixel_height = sq_h * fh
                // square: pixel_width == pixel_height → sq_h = width * fw / fh
                let sq_h_px = (minimap_area.width as u32 * fw as u32) / fh as u32;
                (sq_h_px as u16).min(minimap_area.height)
            } else {
                // Fallback: use half of width as a 2:1 heuristic.
                (minimap_area.width / 2).min(minimap_area.height)
            };
            let sq_h = sq_h.max(4);
            // Stick to the bottom of the panel by offsetting y.
            let sq_y = minimap_area.y + minimap_area.height.saturating_sub(sq_h);
            let square_area = Rect::new(minimap_area.x, sq_y, minimap_area.width, sq_h);

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
