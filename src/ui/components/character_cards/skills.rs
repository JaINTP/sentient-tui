use super::ICON_COL_W;
use super::utils::CharacterExt;
use super::utils::truncate;
use crate::{
    core::game::{CharacterState, SkillInfo},
    ui::image_cache::{ImageCache, PLAY_BASE_URL, ProtocolCache, SharedImageCache},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::Gauge,
};

pub(crate) fn draw_skills_grid(
    frame: &mut Frame,
    area: Rect,
    char: &CharacterState,
    image_cache: &SharedImageCache,
    icon_cache: &mut ProtocolCache,
) {
    let pairs: [(&str, &SkillInfo, &str, &SkillInfo); 4] = [
        ("Mining", &char.mining, "Woodcutting", &char.woodcutting),
        ("Fishing", &char.fishing, "Weaponcrafting", &char.weaponcrafting),
        ("Gearcrafting", &char.gearcrafting, "Jewelrycrafting", &char.jewelrycrafting),
        ("Cooking", &char.cooking, "Alchemy", &char.alchemy),
    ];
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    let active_skill = char.activity_skill();

    for (row_idx, (ln, ls, rn, rs)) in pairs.iter().enumerate() {
        if row_idx >= rows.len() {
            break;
        }
        let [left, right] = Layout::horizontal([
            Constraint::Ratio(1, 2),
            Constraint::Ratio(1, 2),
        ])
        .spacing(1)
        .areas(rows[row_idx]);
        render_skill_cell(frame, left, ln, ls, active_skill == Some(*ln), image_cache, icon_cache);
        render_skill_cell(frame, right, rn, rs, active_skill == Some(*rn), image_cache, icon_cache);
    }
}

pub(crate) fn render_skill_cell(
    frame: &mut Frame,
    area: Rect,
    name: &str,
    skill: &SkillInfo,
    is_active: bool,
    image_cache: &SharedImageCache,
    icon_cache: &mut ProtocolCache,
) {
    if area.width == 0 {
        return;
    }

    // Split off icon column if wide enough
    let (icon_area, gauge_area) = if area.width > ICON_COL_W + 4 {
        let [ic, ga] = Layout::horizontal([
            Constraint::Length(ICON_COL_W),
            Constraint::Min(0),
        ])
        .areas(area);
        (Some(ic), ga)
    } else {
        (None, area)
    };

    // Render skill icon
    if let Some(ic) = icon_area {
        let code = name.to_lowercase();
        let key = format!("skills/{code}");
        if let Some(img) = ImageCache::get_or_fetch_from(image_cache, PLAY_BASE_URL, "skills", &code) {
            icon_cache.ensure(&key, &img);
        }
        if icon_cache.has(&key) {
            icon_cache.render(&key, frame, ic);
        } else {
            use ratatui::widgets::Paragraph;
            frame.render_widget(
                Paragraph::new("·").style(Style::default().fg(Color::DarkGray)),
                ic,
            );
        }
    }

    // Render XP gauge in the remaining area
    let w = gauge_area.width as usize;
    if w == 0 {
        return;
    }
    let level_str = skill.level.to_string();
    let level_str_len = level_str.len();
    let name_max = w.saturating_sub(level_str.len() + 1);
    let name_part = truncate(name, name_max);
    let pad = w.saturating_sub(name_part.chars().count() + level_str.len());
    let text = format!(
        "{}{:>pad$}{:>level_str_len$}",
        name_part,
        "",
        level_str,
        pad = pad,
        level_str_len = level_str_len
    );

    let base_col = skill_level_color(skill.level);
    let ratio = if skill.max_xp > 0 {
        (skill.xp as f64 / skill.max_xp as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let fill_style = if is_active {
        Style::default()
            .fg(base_col)
            .add_modifier(Modifier::BOLD)
    } else if skill.level == 0 {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(base_col)
    };

    let label_style = if skill.level == 0 {
        Style::default().fg(Color::Gray)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    let label = Span::styled(text, label_style);
    frame.render_widget(
        Gauge::default()
            .label(label)
            .ratio(ratio)
            .gauge_style(fill_style),
        gauge_area,
    );
}

/// 4-row combat stats grid: attributes, attack elements, damage %, resistances.
pub(crate) fn skill_level_color(level: u32) -> Color {
    if level >= 30 {
        Color::Rgb(120, 60, 180) // Deep Purple
    } else if level >= 20 {
        Color::Rgb(40, 120, 70) // Forest Green
    } else if level >= 10 {
        Color::Rgb(180, 90, 30) // Rust / Dark Orange
    } else if level > 0 {
        Color::Rgb(60, 110, 160) // Slate Blue
    } else {
        Color::DarkGray // Level 0
    }
}
