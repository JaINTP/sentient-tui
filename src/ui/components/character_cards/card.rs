use super::animation::{
    CARD_BORDER_PHASE_MS, EL_BAG, EL_CD, EL_GEAR, EL_GOAL, EL_HISTORY, EL_HP, EL_PORTRAIT,
    EL_SIGNAL, EL_SKILLS, EL_STATS, EL_XP, element_reveal_ms,
};
use super::gear::draw_gear_table;
use super::skills::draw_skills_grid;
use super::stats::draw_stats_grid;
use super::utils::{log_type_icon, truncate};
use super::{CharacterCards, PORTRAIT_COL_W, PORTRAIT_MIN_WIDTH};
use crate::{
    core::game::{ActionRecord, CharacterState},
    ui::image_cache::ImageCache,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
};
use std::collections::VecDeque;

impl CharacterCards {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_card(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        char: &CharacterState,
        history: &VecDeque<ActionRecord>,
        cooldown_remaining: f64,
        cooldown_total: f64,
        heartbeat: bool,
        is_selected: bool,
        // Milliseconds elapsed since this card's boot animation started.
        // Pass `u64::MAX` to skip animation (card fully loaded).
        elapsed_ms: u64,
    ) {
        // ── Border colour based on activity state ─────────────────────────
        let border_color = match char.last_action.as_str() {
            "fight" | "multi_fight" => Color::Red,
            "gathering" => Color::Green,
            "movement" => Color::Blue,
            "crafting" => Color::Yellow,
            _ => {
                if is_selected {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }
            }
        };

        let hb = if heartbeat {
            "♥ "
        } else {
            "· "
        };
        let level_s = if char.level > 0 {
            format!("Lv{} ", char.level)
        } else {
            String::new()
        };
        let gold_s = if char.gold > 0 {
            format!("  {}g", char.gold)
        } else {
            String::new()
        };
        let pos_s = format!(" [{},{}]", char.x, char.y);

        let block = Block::default()
            .title(format!(" {hb}{}{}{}{} ", level_s, char.name, pos_s, gold_s))
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(border_color)
                    .add_modifier(if is_selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            );

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // ── Border-only phase: content is blank; glitch applied by draw() ─
        if elapsed_ms < CARD_BORDER_PHASE_MS {
            return;
        }

        if inner.height < 5 || inner.width < 10 {
            return;
        }

        // ── Optionally split inner area horizontally for portrait ─────────
        let has_portrait_col = inner.width >= PORTRAIT_MIN_WIDTH && !char.skin.is_empty();
        let gauge_top_h: u16 = 5; // HP + XP + Bag + Cooldown + blank

        // Split inner vertically: [gauge_top | rest of card]
        let [
            gauge_top_area,
            rest_area,
        ] = Layout::vertical([
            Constraint::Length(gauge_top_h),
            Constraint::Min(0),
        ])
        .areas(inner);

        let (portrait_col, gauge_top_inner) = if has_portrait_col {
            let [left, right] = Layout::horizontal([
                Constraint::Length(PORTRAIT_COL_W),
                Constraint::Min(0),
            ])
            .areas(gauge_top_area);
            (Some(left), right)
        } else {
            (None, gauge_top_area)
        };

        let h = rest_area.height + gauge_top_h;
        let has_skills = h >= 12;
        let has_stats = h >= 21;
        let has_gear = h >= 22;

        // Compute overhead for the gauge column layout
        let mut overhead: u16 = 4 + 1 + 1 + 1; // 4 gauges + blank + status + goal
        let skill_rows: u16 = if has_skills {
            4 + 1
        } else {
            0
        };
        let stats_rows: u16 = if has_stats {
            9 + 1 // 5 data rows + 4 section label rows
        } else {
            0
        };
        let gear_rows: u16 = if has_gear {
            8 + 1
        } else {
            0
        };
        overhead += skill_rows + stats_rows + gear_rows;

        let history_rows = h.saturating_sub(overhead).min(6);

        let mut constraints = vec![];
        if has_skills {
            constraints.push(Constraint::Length(skill_rows - 1));
            constraints.push(Constraint::Length(1));
        }
        if has_stats {
            constraints.push(Constraint::Length(stats_rows - 1));
            constraints.push(Constraint::Length(1));
        }
        if has_gear {
            constraints.push(Constraint::Length(gear_rows - 1));
            constraints.push(Constraint::Length(1));
        }
        constraints.push(Constraint::Length(history_rows));
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Min(0));

        let splits = Layout::vertical(constraints).split(rest_area);

        let [
            hp_area,
            xp_area,
            bag_area,
            cd_area,
            _blank,
        ] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(gauge_top_inner);

        let mut i = 0usize;

        let sk_area = if has_skills {
            let a = splits[i];
            i += 2;
            Some(a)
        } else {
            None
        };
        let stats_area = if has_stats {
            let a = splits[i];
            i += 2;
            Some(a)
        } else {
            None
        };
        let eq_area = if has_gear {
            let a = splits[i];
            i += 2;
            Some(a)
        } else {
            None
        };

        let hist_area = splits[i];
        i += 1;
        let signal_area = splits[i];
        i += 1;
        let goal_area = splits[i];

        // Helper: has this element passed its reveal deadline?
        let revealed = |idx: u8| elapsed_ms >= element_reveal_ms(&char.name, idx);

        // ── Portrait (character skin image) ───────────────────────────────
        if let Some(pcol) = portrait_col
            && revealed(EL_PORTRAIT)
        {
            let portrait_key = format!("characters/{}", char.skin);

            // Try to get image from cache; fires download if not yet available.
            if let Some(img) = ImageCache::get_or_fetch(&self.image_cache, "characters", &char.skin)
            {
                self.portraits
                    .ensure(&portrait_key, &img);
            }

            // Portrait spans the gauge rows height (4 gauges + 1 blank = 5 rows)
            let portrait_rect = Rect {
                x: pcol.x,
                y: pcol.y,
                width: pcol.width,
                height: (5).min(pcol.height),
            };

            if self.portraits.has(&portrait_key) {
                self.portraits
                    .render(&portrait_key, frame, portrait_rect);
            } else {
                // Placeholder: character initial while loading
                let init = char
                    .name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_string();
                frame.render_widget(
                    Paragraph::new(init)
                        .style(
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::BOLD),
                        )
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(Color::DarkGray)),
                        ),
                    portrait_rect,
                );
            }
        }

        // ── HP gauge ──────────────────────────────────────────────────────
        if revealed(EL_HP) {
            if char.max_hp > 0 {
                let ratio = char.hp_ratio().clamp(0.0, 1.0);
                let col = if ratio > 0.6 {
                    Color::Rgb(40, 120, 70) // Forest Green
                } else if ratio > 0.3 {
                    Color::Rgb(180, 90, 30) // Dark Orange
                } else {
                    Color::Rgb(150, 40, 40) // Crimson
                };
                let label = Span::styled(
                    format!("HP {}/{}", char.hp, char.max_hp),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                );
                frame.render_widget(
                    Gauge::default()
                        .label(label)
                        .ratio(ratio)
                        .gauge_style(Style::default().fg(col)),
                    hp_area,
                );
            } else {
                frame.render_widget(
                    Paragraph::new("HP  —").style(Style::default().fg(Color::DarkGray)),
                    hp_area,
                );
            }
        }

        // ── XP gauge ──────────────────────────────────────────────────────
        if revealed(EL_XP) {
            let xp_ratio = char.xp_ratio().clamp(0.0, 1.0);
            let label = Span::styled(
                format!("XP {}/{}", char.xp, char.max_xp),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );
            frame.render_widget(
                Gauge::default()
                    .label(label)
                    .ratio(xp_ratio)
                    .gauge_style(Style::default().fg(Color::Rgb(120, 60, 180))), // Deep Purple
                xp_area,
            );
        }

        // ── Bag (inventory) gauge ─────────────────────────────────────────
        if revealed(EL_BAG) {
            let item_count: u32 = char
                .inventory
                .iter()
                .map(|s| s.quantity)
                .sum();
            let bag_max = char.inventory_max_items.max(1);
            let bag_ratio = (item_count as f64 / bag_max as f64).clamp(0.0, 1.0);
            let bag_col = if bag_ratio > 0.9 {
                Color::Rgb(150, 40, 40) // Crimson
            } else if bag_ratio > 0.7 {
                Color::Rgb(180, 90, 30) // Dark Orange
            } else {
                Color::Rgb(60, 110, 160) // Slate Blue
            };
            let label = Span::styled(
                format!("Bag {}/{}", item_count, char.inventory_max_items),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );
            frame.render_widget(
                Gauge::default()
                    .label(label)
                    .ratio(bag_ratio)
                    .gauge_style(Style::default().fg(bag_col)),
                bag_area,
            );
        }

        // ── Cooldown gauge ────────────────────────────────────────────────
        if revealed(EL_CD) {
            let cd_ratio = (cooldown_remaining / cooldown_total.max(1.0)).clamp(0.0, 1.0);
            let label = Span::styled(
                format!("CD {:.1}/{:.0}s", cooldown_remaining, cooldown_total),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );
            frame.render_widget(
                Gauge::default()
                    .label(label)
                    .ratio(cd_ratio)
                    .gauge_style(Style::default().fg(Color::Rgb(150, 40, 40))), // Crimson
                cd_area,
            );
        }

        // ── Skills grid ───────────────────────────────────────────────────
        if let Some(sk_rect) = sk_area
            && revealed(EL_SKILLS)
        {
            draw_skills_grid(frame, sk_rect, char, &self.image_cache, &mut self.skill_icons);
        }

        // ── Combat stats ─────────────────────────────────────────────────
        if let Some(st_rect) = stats_area
            && revealed(EL_STATS)
        {
            draw_stats_grid(frame, st_rect, char, &self.image_cache, &mut self.stat_icons);
        }

        // ── Gear table (with item icons when wide enough) ─────────────────
        if let Some(eq_rect) = eq_area
            && revealed(EL_GEAR)
        {
            draw_gear_table(frame, eq_rect, char, &self.image_cache, &mut self.item_icons);
        }

        // ── Action history ────────────────────────────────────────────────
        if revealed(EL_HISTORY) && history_rows > 0 {
            let w = hist_area.width as usize;
            let lines: Vec<Line> = history
                .iter()
                .rev()
                .take(history_rows as usize)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|rec| {
                    let (icon, col) = log_type_icon(&rec.log_type);
                    let desc_max = w.saturating_sub(icon.len() + 1);
                    Line::from(vec![
                        Span::styled(icon, Style::default().fg(col)),
                        Span::raw(" "),
                        Span::styled(
                            truncate(&rec.description, desc_max),
                            Style::default().fg(Color::Gray),
                        ),
                    ])
                })
                .collect();
            frame.render_widget(Paragraph::new(lines), hist_area);
        }

        // ── Signal (cooldown / last action) ──────────────────────────────
        if revealed(EL_SIGNAL) {
            let signal_text = if cooldown_remaining > 0.0 {
                format!("⏱ {:.1}s", cooldown_remaining)
            } else if !char.last_description.is_empty() {
                char.last_description.clone()
            } else {
                "—".to_string()
            };
            let sig_col = if cooldown_remaining > 0.0 {
                Color::Yellow
            } else {
                Color::DarkGray
            };
            let sig_block = Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray));
            let sig_inner = sig_block.inner(signal_area);
            frame.render_widget(sig_block, signal_area);
            frame.render_widget(
                Paragraph::new(truncate(&signal_text, sig_inner.width as usize))
                    .style(Style::default().fg(sig_col)),
                sig_inner,
            );
        }

        // ── Goal / task ───────────────────────────────────────────────────
        if revealed(EL_GOAL) {
            let accent = char.activity_color();
            let goal_lines: Vec<Line> = if let Some(task_str) = char.task_display() {
                vec![
                    Line::from(Span::styled(
                        char.activity_label(),
                        Style::default()
                            .fg(accent)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(vec![
                        Span::styled("Task: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            truncate(&task_str, (goal_area.width as usize).saturating_sub(6)),
                            Style::default().fg(Color::Magenta),
                        ),
                    ]),
                ]
            } else {
                let act = char.activity_label();
                let desc = if char.last_description.is_empty() {
                    act.to_string()
                } else {
                    format!(
                        "{}: {}",
                        act,
                        truncate(
                            &char.last_description,
                            (goal_area.width as usize).saturating_sub(act.len() + 2)
                        )
                    )
                };
                vec![Line::from(
                    Span::styled(
                        desc,
                        Style::default()
                            .fg(accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                )]
            };
            frame.render_widget(
                Paragraph::new(goal_lines).wrap(Wrap {
                    trim: true,
                }),
                goal_area,
            );
        }
    }
}
