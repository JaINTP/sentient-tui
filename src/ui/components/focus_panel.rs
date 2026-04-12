use std::collections::{HashMap, VecDeque};

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{
    core::action::Action,
    core::config::Config,
    core::game::{AccountLogEntry, CharacterState},
};

const MAX_HISTORY: usize = 100;

#[derive(Default)]
pub struct FocusPanel {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    /// The currently-selected character (populated from sidebar sync).
    character: Option<CharacterState>,
    /// Per-character recent account_log history (ring buffer).
    history: HashMap<String, VecDeque<AccountLogEntry>>,
}

impl FocusPanel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_character(&mut self, character: Option<CharacterState>) {
        self.character = character;
    }

    fn push_log(&mut self, entry: AccountLogEntry) {
        let buf = self
            .history
            .entry(entry.character.clone())
            .or_default();
        if buf.len() >= MAX_HISTORY {
            buf.pop_front();
        }
        buf.push_back(entry);
    }

    fn render_log_line(entry: &AccountLogEntry) -> Line<'static> {
        let color = log_type_color(&entry.log_type);
        let label = log_type_label(&entry.log_type);
        let cd = if entry.cooldown > 0 {
            format!("  cd {}s", entry.cooldown)
        } else {
            String::new()
        };
        Line::from(vec![
            Span::styled(
                format!("{} ", label),
                Style::default()
                    .fg(color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(truncate(&entry.description, 60), Style::default().fg(Color::Gray)),
            Span::styled(cd, Style::default().fg(Color::DarkGray)),
        ])
    }
}

fn log_type_color(log_type: &str) -> Color {
    match log_type {
        "fight" | "multi_fight" => Color::Red,
        "gathering" => Color::Green,
        "crafting" => Color::Yellow,
        "movement" => Color::Cyan,
        "rest" => Color::Blue,
        "task_completed" => Color::Magenta,
        "new_task" | "task_exchange" | "task_cancelled" => Color::LightMagenta,
        "recycling" => Color::LightYellow,
        "buy_ge" | "sell_ge" | "create_buy_order_ge" | "fill_buy_order_ge" => Color::LightCyan,
        "deposit_item" | "deposit_gold" | "withdraw_item" | "withdraw_gold" => Color::Gray,
        _ => Color::DarkGray,
    }
}

fn log_type_label(log_type: &str) -> &'static str {
    match log_type {
        "fight" => "⚔",
        "multi_fight" => "⚔⚔",
        "gathering" => "⛏",
        "crafting" => "🔨",
        "movement" => "→",
        "rest" => "♥",
        "task_completed" => "✓",
        "new_task" => "+",
        "task_exchange" | "task_cancelled" => "~",
        "recycling" => "♻",
        "buy_ge" | "create_buy_order_ge" | "fill_buy_order_ge" => "↑",
        "sell_ge" => "↓",
        "deposit_item" | "deposit_gold" => "↓B",
        "withdraw_item" | "withdraw_gold" => "↑B",
        _ => "·",
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

impl Component for FocusPanel {
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
            Action::AccountLog(entry) => {
                // Keep the selected character's state in sync
                if self
                    .character
                    .as_ref()
                    .is_some_and(|c| c.name == entry.character)
                    && let Some(c) = self.character.as_mut()
                {
                    c.last_action = entry.log_type.clone();
                    c.last_description = entry.description.clone();
                }
                // Always store in history
                self.push_log(entry);
            }
            Action::OnlineCharacters(ref chars) => {
                // Update position of the focused character if it appears
                if let Some(focused) = self.character.as_mut()
                    && let Some(fresh) = chars
                        .iter()
                        .find(|c| c.name == focused.name)
                {
                    focused.x = fresh.x;
                    focused.y = fresh.y;
                    if focused.skin.is_empty() {
                        focused.skin = fresh.skin.clone();
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let Some(character) = &self.character else {
            let block = Block::default()
                .title(" Focus ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray));
            frame.render_widget(
                Paragraph::new("select a character with j/k")
                    .block(block)
                    .style(Style::default().fg(Color::DarkGray)),
                area,
            );
            return Ok(());
        };

        let block = Block::default()
            .title(format!(" {} ", character.name))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let [
            info_area,
            history_area,
        ] = Layout::vertical([
            Constraint::Length(5),
            Constraint::Min(0),
        ])
        .areas(inner);

        // ── Character info strip ──────────────────────────────────────────
        let activity_color = character.activity_color();
        let info_lines = vec![
            Line::from(vec![
                Span::styled("Account  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    if character.account.is_empty() {
                        "—".to_string()
                    } else {
                        character.account.clone()
                    },
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("Position ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("({}, {})", character.x, character.y),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("Status   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    character.activity_label(),
                    Style::default()
                        .fg(activity_color)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Cooldown ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}s", character.last_action.len()), // placeholder — CD tracked in CharacterCards
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ];
        frame.render_widget(Paragraph::new(info_lines), info_area);

        // ── Recent action history ────────────────────────────────────────
        let hist_block = Block::default()
            .title(" Recent Actions ")
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray));

        let hist_inner = hist_block.inner(history_area);
        frame.render_widget(hist_block, history_area);

        let visible_lines = hist_inner.height as usize;
        let empty: VecDeque<AccountLogEntry> = VecDeque::new();
        let history = self
            .history
            .get(&character.name)
            .unwrap_or(&empty);

        if history.is_empty() {
            frame.render_widget(
                Paragraph::new("no actions yet…").style(Style::default().fg(Color::DarkGray)),
                hist_inner,
            );
        } else {
            let lines: Vec<Line> = history
                .iter()
                .rev()
                .take(visible_lines)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(Self::render_log_line)
                .collect();
            frame.render_widget(
                Paragraph::new(lines).wrap(Wrap {
                    trim: true,
                }),
                hist_inner,
            );
        }

        Ok(())
    }
}
