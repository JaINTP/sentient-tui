//! Equipment / gear sub-panel for a character card.
//!
//! Renders a "Gear" section separator followed by an 8-row × 2-column table
//! of equipment slots.  Each slot cell shows a small item thumbnail (fetched
//! from `ImageCache`) alongside a truncated slot label and the item's
//! title-cased display name.  Empty slots are indicated with an em dash.

use super::ICON_COL_W;
use super::utils::{normalise_code, truncate};
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

/// Render the full gear table for `char` into `area`.
///
/// The table begins with a `"─ Gear ───"` section separator then renders 8
/// rows of paired equipment slots:
///
/// | Row | Left | Right |
/// |-----|------|-------|
/// | 0 | Weapon | Shield |
/// | 1 | Helmet | Body |
/// | 2 | Legs | Boots |
/// | 3 | Ring 1 | Ring 2 |
/// | 4 | Amulet | Rune |
/// | 5 | Artifact 1 | Artifact 2 |
/// | 6 | Artifact 3 | Bag |
/// | 7 | Utility 1 | Utility 2 |
pub(crate) fn draw_gear_table(
    frame: &mut Frame,
    area: Rect,
    char: &CharacterState,
    image_cache: &SharedImageCache,
    icon_cache: &mut ProtocolCache,
) {
    let pairs: [(&str, &str, &str, &str); 8] = [
        ("Weapon", &char.weapon_slot, "Shield", &char.shield_slot),
        ("Helmet", &char.helmet_slot, "Body", &char.body_armor_slot),
        ("Legs", &char.leg_armor_slot, "Boots", &char.boots_slot),
        ("Ring 1", &char.ring1_slot, "Ring 2", &char.ring2_slot),
        ("Amulet", &char.amulet_slot, "Rune", &char.rune_slot),
        ("Artifact 1", &char.artifact1_slot, "Artifact 2", &char.artifact2_slot),
        ("Artifact 3", &char.artifact3_slot, "Bag", &char.bag_slot),
        ("Utility 1", &char.utility1_slot, "Utility 2", &char.utility2_slot),
    ];

    let [sep_area, table_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);

    render_section_label(frame, sep_area, "Gear", Color::Yellow);

    // ── Custom Rect-based layout with icon thumbnails ─────────────────
    let row_areas = Layout::vertical(
        (0..8)
            .map(|_| Constraint::Length(1))
            .collect::<Vec<_>>(),
    )
    .split(table_area);

    for (row_idx, (ll, lc, rl, rc)) in pairs.iter().enumerate() {
        if row_idx >= row_areas.len() {
            break;
        }
        let row = row_areas[row_idx];
        let [
            left_half,
            right_half,
        ] = Layout::horizontal([
            Constraint::Ratio(1, 2),
            Constraint::Ratio(1, 2),
        ])
        .areas(row);

        render_gear_slot_with_icon(frame, left_half, ll, lc, image_cache, icon_cache);
        render_gear_slot_with_icon(frame, right_half, rl, rc, image_cache, icon_cache);
    }
}

/// Render a section separator line: `"─ {label} ───────────"`.
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

/// Render one gear slot with a small icon thumbnail on the left.
///
/// Layout: [icon ICON_COL_W] [label short] [item code rest]
pub(crate) fn render_gear_slot_with_icon(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    code: &str,
    image_cache: &SharedImageCache,
    icon_cache: &mut ProtocolCache,
) {
    if area.width < ICON_COL_W + 4 {
        return;
    }

    let [icon_area, text_area] = Layout::horizontal([
        Constraint::Length(ICON_COL_W),
        Constraint::Min(0),
    ])
    .areas(area);

    // Text portion: label + code
    let lbl_w = (text_area.width / 3).max(4) as usize;
    let item_w = (text_area.width as usize).saturating_sub(lbl_w + 1);
    let [lbl_area, code_area] = Layout::horizontal([
        Constraint::Length(lbl_w as u16),
        Constraint::Min(0),
    ])
    .areas(text_area);

    frame.render_widget(
        Paragraph::new(truncate(label, lbl_w)).style(Style::default().fg(Color::DarkGray)),
        lbl_area,
    );
    frame.render_widget(
        Paragraph::new(if code.is_empty() {
            "—".to_string()
        } else {
            truncate(&normalise_code(code), item_w)
        })
        .style(Style::default().fg(if code.is_empty() {
            Color::DarkGray
        } else {
            Color::Gray
        })),
        code_area,
    );

    // Icon
    if code.is_empty() {
        return;
    }
    let key = format!("items/{code}");
    if let Some(img) = ImageCache::get_or_fetch(image_cache, "items", code) {
        icon_cache.ensure(&key, &img);
    }
    if icon_cache.has(&key) {
        icon_cache.render(&key, frame, icon_area);
    } else {
        // Tiny placeholder square while the image loads
        frame.render_widget(
            Paragraph::new("·").style(Style::default().fg(Color::DarkGray)),
            icon_area,
        );
    }
}
