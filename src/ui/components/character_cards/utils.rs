//! Shared rendering helpers for the character-cards sub-system.
//!
//! This module collects small, stateless utilities that are used across
//! multiple character-card sub-modules (`card`, `stats`, `skills`, `gear`):
//!
//! - [`log_type_icon`] — maps an `account_log` type string to a display icon and colour.
//! - [`normalise_code`] — converts snake_case item codes to title-case display strings.
//! - [`truncate`] — Unicode-aware string truncation with ellipsis.
//! - [`CharacterExt`] — extension trait that infers the active skill from a character's
//!   current task and last action.

use crate::core::game::CharacterState;
use ratatui::style::Color;

/// Map an `account_log` type string to a `(icon, colour)` pair.
///
/// Returns `("·", DarkGray)` for unrecognised log types.
pub(crate) fn log_type_icon(log_type: &str) -> (&'static str, Color) {
    match log_type {
        "fight" | "multi_fight" => ("✕", Color::Red),
        "gathering" => ("⌃", Color::Green),
        "crafting" => ("◈", Color::Yellow),
        "movement" => ("→", Color::Cyan),
        "rest" => ("♥", Color::Blue),
        "task_completed" => ("✓", Color::Magenta),
        "new_task" => ("+", Color::Magenta),
        "recycling" => ("♻", Color::LightYellow),
        "buy_ge" | "sell_ge" | "create_buy_order_ge" | "fill_buy_order_ge" => ("$", Color::Cyan),
        "deposit_item" | "deposit_gold" => ("▼", Color::Gray),
        "withdraw_item" | "withdraw_gold" => ("▲", Color::Gray),
        _ => ("·", Color::DarkGray),
    }
}

/// Convert a code like `wooden_staff` → `Wooden Staff`.
pub(crate) fn normalise_code(code: &str) -> String {
    code.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Truncate `s` to at most `max` Unicode scalar values, appending `…` if cut.
///
/// Returns an empty string when `max` is 0.  Does not split multi-byte
/// characters because it operates on `char` boundaries, not byte boundaries.
pub(crate) fn truncate(s: &str, max: usize) -> String {
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

// ── Component impl ────────────────────────────────────────────────────────────

/// Extension trait for [`CharacterState`] providing UI-layer helpers.
pub(crate) trait CharacterExt {
    /// Infer the skill name that the character is currently exercising, if any.
    ///
    /// Returns `Some("Mining")`, `Some("Cooking")`, etc. when the character's
    /// `last_action` is `"gathering"` or `"crafting"` and the `task_type` maps
    /// to a known skill name.  Returns `None` for all other action types or
    /// unrecognised task types.
    fn activity_skill(&self) -> Option<&'static str>;
}

impl CharacterExt for CharacterState {
    fn activity_skill(&self) -> Option<&'static str> {
        match self.last_action.as_str() {
            "gathering" => match self.task_type.as_str() {
                "mining" => Some("Mining"),
                "woodcutting" => Some("Woodcutting"),
                "fishing" => Some("Fishing"),
                _ => None,
            },
            "crafting" => match self.task_type.as_str() {
                "weaponcrafting" => Some("Weaponcrafting"),
                "gearcrafting" => Some("Gearcrafting"),
                "jewelrycrafting" => Some("Jewelrycrafting"),
                "cooking" => Some("Cooking"),
                "alchemy" => Some("Alchemy"),
                _ => None,
            },
            _ => None,
        }
    }
}

// ── Border Animation helper ───────────────────────────────────────────────────
