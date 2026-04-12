use super::ICON_COL_W;
use crate::{
    core::game::CharacterState,
    ui::image_cache::{ImageCache, ProtocolCache, SharedImageCache},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub(crate) fn draw_stats_grid(
    frame: &mut Frame,
    area: Rect,
    char: &CharacterState,
    image_cache: &SharedImageCache,
    icon_cache: &mut ProtocolCache,
) {
    let rows = Layout::vertical([
        Constraint::Length(1), // ── Core ──
        Constraint::Length(1), // general attrs
        Constraint::Length(1), // combat attrs
        Constraint::Length(1), // ── Attack ──
        Constraint::Length(1), // elemental attack
        Constraint::Length(1), // ── Damage ──
        Constraint::Length(1), // elemental damage %
        Constraint::Length(1), // ── Resist ──
        Constraint::Length(1), // elemental resistances
    ])
    .split(area);

    // Section separator: Core
    render_section_label(frame, rows[0], "Core", Color::White);

    // Row 1: General attributes
    render_stat_row(
        frame,
        rows[1],
        &[
            ("haste", "Haste", char.haste, Color::LightCyan),
            ("critical_strike", "Crit", char.critical_strike, Color::LightYellow),
            ("wisdom", "Wisdom", char.wisdom, Color::Magenta),
            ("prospecting", "Prosp", char.prospecting, Color::Green),
        ],
        image_cache,
        icon_cache,
    );
    // Row 2: Combat attributes
    render_stat_row(
        frame,
        rows[2],
        &[
            ("initiative", "Init", char.initiative, Color::LightBlue),
            ("threat", "Threat", char.threat, Color::Red),
            ("dmg", "Dmg", char.dmg, Color::LightRed),
        ],
        image_cache,
        icon_cache,
    );

    // Section separator: Attack
    render_section_label(frame, rows[3], "Attack", Color::Red);

    // Row 4: Elemental attack
    render_stat_row(
        frame,
        rows[4],
        &[
            ("attack_fire", "Fire", char.attack_fire, Color::Red),
            ("attack_earth", "Earth", char.attack_earth, Color::Green),
            ("attack_water", "Water", char.attack_water, Color::Blue),
            ("attack_air", "Air", char.attack_air, Color::Cyan),
        ],
        image_cache,
        icon_cache,
    );

    // Section separator: Damage
    render_section_label(frame, rows[5], "Damage", Color::LightRed);

    // Row 6: Elemental damage %
    render_stat_row(
        frame,
        rows[6],
        &[
            ("dmg_fire", "Fire", char.dmg_fire, Color::Red),
            ("dmg_earth", "Earth", char.dmg_earth, Color::Green),
            ("dmg_water", "Water", char.dmg_water, Color::Blue),
            ("dmg_air", "Air", char.dmg_air, Color::Cyan),
        ],
        image_cache,
        icon_cache,
    );

    // Section separator: Resist
    render_section_label(frame, rows[7], "Resist", Color::Blue);

    // Row 8: Elemental resistances
    render_stat_row(
        frame,
        rows[8],
        &[
            ("res_fire", "Fire", char.res_fire, Color::Red),
            ("res_earth", "Earth", char.res_earth, Color::Green),
            ("res_water", "Water", char.res_water, Color::Blue),
            ("res_air", "Air", char.res_air, Color::Cyan),
        ],
        image_cache,
        icon_cache,
    );
}

fn render_section_label(frame: &mut Frame, area: Rect, label: &str, color: Color) {
    let w = area.width as usize;
    let label_part = format!("─ {label} ");
    let dashes = "─".repeat(w.saturating_sub(label_part.chars().count()));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(label_part, Style::default().fg(color)),
            Span::styled(dashes, Style::default().fg(Color::DarkGray)),
        ])),
        area,
    );
}

pub(crate) fn render_stat_row(
    frame: &mut Frame,
    area: Rect,
    stats: &[(&str, &str, i32, Color)],
    image_cache: &SharedImageCache,
    icon_cache: &mut ProtocolCache,
) {
    let n = stats.len() as u32;
    if n == 0 {
        return;
    }
    let cols = Layout::horizontal(
        (0..n)
            .map(|_| Constraint::Ratio(1, n))
            .collect::<Vec<_>>(),
    )
    .spacing(1)
    .split(area);

    for (i, (effect_code, label, value, color)) in stats.iter().enumerate() {
        let cell = cols[i];
        let val_color = if *value > 0 {
            *color
        } else {
            Color::DarkGray
        };

        if cell.width <= ICON_COL_W {
            frame.render_widget(
                Paragraph::new(value.to_string()).style(Style::default().fg(val_color)),
                cell,
            );
            continue;
        }

        let [icon_area, text_area] = Layout::horizontal([
            Constraint::Length(ICON_COL_W),
            Constraint::Min(0),
        ])
        .areas(cell);

        // Fetch / render effect icon
        let key = format!("effects/{effect_code}");
        if let Some(img) = ImageCache::get_or_fetch(image_cache, "effects", effect_code) {
            icon_cache.ensure(&key, &img);
        }
        if icon_cache.has(&key) {
            icon_cache.render(&key, frame, icon_area);
        } else {
            frame.render_widget(
                Paragraph::new("·").style(Style::default().fg(Color::DarkGray)),
                icon_area,
            );
        }

        // Label (dim) + value (right-aligned, coloured)
        let w = text_area.width as usize;
        let val_str = value.to_string();
        let val_w = val_str.len();
        let label_w = w.saturating_sub(val_w + 1);
        let label_truncated: String = label.chars().take(label_w).collect();
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{label_truncated:<label_w$}"),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(" "),
                Span::styled(val_str, Style::default().fg(val_color)),
            ])),
            text_area,
        );
    }
}
