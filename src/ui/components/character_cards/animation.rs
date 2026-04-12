use ratatui::{layout::Rect, style::Color};
use tachyonfx::{Effect, EffectManager, fx::Glitch};

pub(crate) const CARD_BOOT_TOTAL_MS: u64 = 580;
/// Border-only phase: inner content is blank, full-card glitch is active.
pub(crate) const CARD_BORDER_PHASE_MS: u64 = 240;
/// Earliest time at which a content element may reveal itself.
pub(crate) const CARD_CONTENT_START_MS: u64 = 220;
/// Latest time at which a content element reveals itself.
pub(crate) const CARD_CONTENT_END_MS: u64 = 560;

// ── Element indices for reveal ordering ───────────────────────────────────────
pub(crate) const EL_PORTRAIT: u8 = 0;
pub(crate) const EL_HP: u8 = 1;
pub(crate) const EL_XP: u8 = 2;
pub(crate) const EL_BAG: u8 = 3;
pub(crate) const EL_CD: u8 = 4;
pub(crate) const EL_SKILLS: u8 = 5;
pub(crate) const EL_STATS: u8 = 6;
pub(crate) const EL_GEAR: u8 = 7;
pub(crate) const EL_HISTORY: u8 = 8;
pub(crate) const EL_SIGNAL: u8 = 9;
pub(crate) const EL_GOAL: u8 = 10;

pub(crate) fn element_reveal_ms(char_name: &str, idx: u8) -> u64 {
    let name_hash = char_name.bytes().fold(0u64, |h, b| {
        h.wrapping_mul(31)
            .wrapping_add(b as u64)
    });
    let combined = name_hash.wrapping_add((idx as u64).wrapping_mul(2_654_435_761u64));
    let r = combined.wrapping_mul(0x9e37_79b9_7f4a_7c15u64) >> 33;
    CARD_CONTENT_START_MS + r % (CARD_CONTENT_END_MS - CARD_CONTENT_START_MS)
}

/// Create a fresh glitch EffectManager for the card boot animation.
pub(crate) fn arm_card_glitch() -> EffectManager<&'static str> {
    let mut mgr = EffectManager::default();
    mgr.add_unique_effect(
        "boot",
        Effect::new(
            Glitch::builder()
                .cell_glitch_ratio(0.55)
                .action_start_delay_ms(0..52)
                .action_ms(31..100)
                .build(),
        ),
    );
    mgr
}

// ── Sub-widget helpers ────────────────────────────────────────────────────────

/// 4-row × 2-col skills grid with XP gauges.
pub(crate) fn apply_animated_border(
    buf: &mut ratatui::buffer::Buffer,
    area: Rect,
    base_color: Color,
    elapsed_ms: u64,
) {
    if area.width < 2 || area.height < 2 {
        return;
    }

    let (h, s, l) = tachyonfx::color_to_hsl(&base_color);

    // Create a clean "shooting star" pulse strictly adhering to the base hue
    let bright = tachyonfx::color_from_hsl(h, s, (l + 25.0).min(85.0));
    let dark = tachyonfx::color_from_hsl(h, s, (l - 15.0).max(25.0));
    let darkest = tachyonfx::color_from_hsl(h, s, (l - 30.0).max(15.0));

    let seq = [
        (2, bright),     // Bright leading head
        (3, base_color), // Core body
        (4, dark),       // Fading tail
        (15, darkest),   // Long dark gap
    ];

    let mut expanded = Vec::new();
    for (count, c) in seq {
        for _ in 0..count {
            expanded.push(c);
        }
    }
    let total = expanded.len();
    if total == 0 {
        return;
    }

    // Controls animation speed (cells per second). Decrease this number to make it slower!
    let speed_cells_per_second = 14.0;
    let idx_offset = (elapsed_ms as f64 / 1000.0 * speed_cells_per_second) as usize;

    let mut update_cell = |x: u16, y: u16, idx: usize| {
        if let Some(cell) = buf.cell_mut((x, y)) {
            // Only update the color if it's a box-drawing character,
            // this prevents the effect from bleeding over the title text!
            let is_box_char = cell
                .symbol()
                .chars()
                .next()
                .map(|c| ('\u{2500}'..='\u{257F}').contains(&c))
                .unwrap_or(false);

            if is_box_char {
                cell.set_fg(expanded[idx % total]);
            }
        }
    };

    let mut c_idx = idx_offset;
    for x in area.left()..area.right() {
        update_cell(x, area.top(), c_idx);
        c_idx += 1;
    }
    for y in area.top() + 1..area.bottom() - 1 {
        update_cell(area.right() - 1, y, c_idx);
        c_idx += 1;
    }
    for x in (area.left()..area.right()).rev() {
        update_cell(x, area.bottom() - 1, c_idx);
        c_idx += 1;
    }
    for y in (area.top() + 1..area.bottom() - 1).rev() {
        update_cell(area.left(), y, c_idx);
        c_idx += 1;
    }
}
