//! Skills sub-panel for a character card.
//!
//! Renders an 4-row × 2-column grid of XP gauges — one per ArtifactsMMO skill
//! (Mining, Woodcutting, Fishing, Weaponcrafting, Gearcrafting, Jewelrycrafting,
//! Cooking, Alchemy).  Each cell shows a ratatui [`Gauge`] filled to the
//! character's XP progress within the current level, with the skill icon on
//! the left and the skill name + level on the gauge label.
//!
//! The cell corresponding to the character's current activity is highlighted
//! in bold so it is easy to identify at a glance.

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

/// Render the full 4-row × 2-column skills grid for `char` into `area`.
///
/// Skills are paired left-right as follows:
///
/// | Row | Left | Right |
/// |-----|------|-------|
/// | 0 | Mining | Woodcutting |
/// | 1 | Fishing | Weaponcrafting |
/// | 2 | Gearcrafting | Jewelrycrafting |
/// | 3 | Cooking | Alchemy |
///
/// The cell for the character's currently active skill (if determinable from
/// `char.last_action` and `char.task_type`) is rendered in bold.
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

/// Render a single skill cell: optional icon + XP gauge.
///
/// # Parameters
///
/// - `area` — bounding box for this cell.
/// - `name` — display name of the skill (e.g. `"Mining"`), also used to
///   derive the icon image key.
/// - `skill` — current level and XP data for the skill.
/// - `is_active` — when `true` the gauge label is rendered in bold to
///   indicate this is the character's current activity.
/// - `image_cache` / `icon_cache` — shared image store and per-card protocol
///   cache; the icon is fetched asynchronously on first call.
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
        if let Some(img) =
            ImageCache::get_or_fetch_from(image_cache, PLAY_BASE_URL, "skills", &code)
        {
            icon_cache.ensure(&key, &img);
        }
        if icon_cache.has(&key) {
            icon_cache.render(&key, frame, ic);
        } else {
            use ratatui::widgets::Paragraph;
            frame
                .render_widget(Paragraph::new("·").style(Style::default().fg(Color::DarkGray)), ic);
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

/// Map a skill level to a gauge fill colour.
///
/// | Range | Colour |
/// |-------|--------|
/// | 0 | Dark grey (unlevelled) |
/// | 1–9 | Slate blue |
/// | 10–19 | Rust / dark orange |
/// | 20–29 | Forest green |
/// | 30+ | Deep purple |
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
