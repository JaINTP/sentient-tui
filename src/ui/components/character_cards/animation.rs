//! Boot animation primitives for individual character cards.
//!
//! Each card runs a two-phase boot sequence when it first appears:
//!
//! 1. **Border phase** (`0` → [`CARD_BORDER_PHASE_MS`] ms): the card's border
//!    is drawn with a full-card tachyonfx [`Glitch`] effect; inner content is
//!    blank.
//! 2. **Reveal phase** ([`CARD_CONTENT_START_MS`] → [`CARD_CONTENT_END_MS`] ms):
//!    individual content elements fade in at pseudo-random offsets derived from
//!    the character name (see [`element_reveal_ms`]).
//!
//! After [`CARD_BOOT_TOTAL_MS`] the animation completes and the card renders
//! normally on every frame.
//!
//! The animated border effect ([`apply_animated_border`]) is a "shooting star"
//! pulse that continues running indefinitely after boot, driven by the
//! elapsed-milliseconds clock.

use ratatui::{layout::Rect, style::Color};
use tachyonfx::{Effect, EffectManager, fx::Glitch};

/// Total duration of the card boot sequence in milliseconds.
pub(crate) const CARD_BOOT_TOTAL_MS: u64 = 580;

/// Border-only phase: inner content is blank, full-card glitch is active.
pub(crate) const CARD_BORDER_PHASE_MS: u64 = 240;

/// Earliest time at which a content element may reveal itself during boot.
pub(crate) const CARD_CONTENT_START_MS: u64 = 220;

/// Latest time at which a content element reveals itself during boot.
pub(crate) const CARD_CONTENT_END_MS: u64 = 560;

// ── Element indices for reveal ordering ───────────────────────────────────────
// These constants are passed to `element_reveal_ms` to produce per-element,
// per-character reveal times that are consistent across frames but vary between
// cards so they don't all appear simultaneously.

/// Element index: character portrait image.
pub(crate) const EL_PORTRAIT: u8 = 0;
/// Element index: HP bar.
pub(crate) const EL_HP: u8 = 1;
/// Element index: XP bar.
pub(crate) const EL_XP: u8 = 2;
/// Element index: inventory / bag row.
pub(crate) const EL_BAG: u8 = 3;
/// Element index: cooldown countdown.
pub(crate) const EL_CD: u8 = 4;
/// Element index: skills sub-panel.
pub(crate) const EL_SKILLS: u8 = 5;
/// Element index: stats sub-panel.
pub(crate) const EL_STATS: u8 = 6;
/// Element index: gear/equipment sub-panel.
pub(crate) const EL_GEAR: u8 = 7;
/// Element index: action history feed.
pub(crate) const EL_HISTORY: u8 = 8;
/// Element index: WebSocket signal indicator.
pub(crate) const EL_SIGNAL: u8 = 9;
/// Element index: current goal / task line.
pub(crate) const EL_GOAL: u8 = 10;

/// Compute the reveal timestamp (in milliseconds since card boot) for element
/// `idx` on a card belonging to `char_name`.
///
/// The result is deterministic for a given `(char_name, idx)` pair so the
/// animation is stable across re-renders, but varies between characters and
/// between element types so no two elements appear at exactly the same moment.
///
/// The returned value is always in the range
/// `[CARD_CONTENT_START_MS, CARD_CONTENT_END_MS)`.
pub(crate) fn element_reveal_ms(char_name: &str, idx: u8) -> u64 {
    let name_hash = char_name.bytes().fold(0u64, |h, b| {
        h.wrapping_mul(31)
            .wrapping_add(b as u64)
    });
    let combined = name_hash.wrapping_add((idx as u64).wrapping_mul(2_654_435_761u64));
    let r = combined.wrapping_mul(0x9e37_79b9_7f4a_7c15u64) >> 33;
    CARD_CONTENT_START_MS + r % (CARD_CONTENT_END_MS - CARD_CONTENT_START_MS)
}

/// Create a fresh [`EffectManager`] pre-loaded with the card boot glitch effect.
///
/// The returned manager holds a single `"boot"` effect — a tachyonfx [`Glitch`]
/// configured to affect ~55 % of cells with randomised start delays and
/// durations.  Call `mgr.process_effects(dt, frame, area)` on every tick
/// while the card is in the border phase.
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

/// Paint an animated "shooting star" border pulse onto an existing buffer.
///
/// Walks the perimeter of `area` (top → right → bottom ← left ←) and
/// recolours only box-drawing characters (`U+2500`–`U+257F`) so the effect
/// cannot bleed over title text or content cells.  The colour sequence — bright
/// head, core body, fading tail, long dark gap — is repeated around the entire
/// perimeter and advanced each call based on `elapsed_ms` and a fixed speed of
/// 14 cells per second.
///
/// # Parameters
///
/// - `buf` — the ratatui render buffer to mutate.
/// - `area` — the full card bounding box (border cells are at the edges).
/// - `base_color` — the card's theme colour; bright/dark variants are derived
///   from its HSL representation.
/// - `elapsed_ms` — milliseconds since the card was created, used to advance
///   the pulse position.
///
/// Does nothing when `area` is smaller than 2×2.
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
