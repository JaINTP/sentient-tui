use crate::core::game::CharacterState;
use ratatui::style::Color;

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

/// Convert an item code like `wooden_staff` → `Wooden Staff`.
pub(crate) fn pretty_item(code: &str) -> String {
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

pub(crate) trait CharacterExt {
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
