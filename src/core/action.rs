//! Action bus — all UI events and game notifications route through this enum.
//!
//! Actions are sent by event handlers, WebSocket listeners, and REST clients
//! through an `mpsc` channel. The main app loop receives and dispatches them
//! to update `GameState` and notify components.

use serde::{Deserialize, Serialize};
use strum::Display;

use crate::core::game::{AccountLogEntry, CharacterState, GEOrder, GETransaction, WorldEvent};

/// All possible UI and game events in the application.
///
/// Sent through an `mpsc` channel from event handlers, WebSocket, REST clients,
/// and background tasks. The main app loop processes these and updates `GameState`.
#[derive(Debug, Clone, PartialEq, Display, Serialize, Deserialize)]
pub enum Action {
    // ── UI lifecycle ─────────────────────────────────────────────────────────

    /// Game logic tick — process cooldowns, animations, periodic tasks (4/sec default).
    Tick,
    /// Render frame — redraw all components to the terminal.
    Render,
    /// Terminal was resized to `(width, height)` in character cells.
    Resize(u16, u16),
    /// Suspend the application (Ctrl+Z) — enter background.
    Suspend,
    /// Resume after suspend.
    Resume,
    /// Quit the application (Ctrl+C, 'q', etc.).
    Quit,
    /// Clear the terminal screen.
    ClearScreen,
    /// Display an error message in the TUI.
    Error(String),
    /// Show help/usage information.
    Help,

    // ── Navigation ───────────────────────────────────────────────────────────

    /// Move focus to the next component/element.
    FocusNext,
    /// Move focus to the previous component/element.
    FocusPrev,
    /// Toggle the footer log panel visibility.
    ToggleLog,

    // ── WebSocket lifecycle ──────────────────────────────────────────────────

    /// Attempt to connect to the WebSocket (triggered on disconnect).
    WsConnect,
    /// WebSocket connection successful and authenticated.
    WsConnected,
    /// WebSocket disconnected with reason message.
    WsDisconnected(String),
    /// Attempt to reconnect to WebSocket after a delay.
    WsReconnect,

    // ── Real-time WebSocket notifications ────────────────────────────────────
    // These come from `wss://realtime.artifactsmmo.com`

    /// account_log notification — a character performed an action.
    ///
    /// Contains the full LogSchema payload: action type, description, cooldown,
    /// and character data (e.g., new HP, XP, position after the action).
    AccountLog(AccountLogEntry),

    /// online_characters snapshot — periodic list of currently online players.
    ///
    /// Used to update character positions in the sidebar minimap.
    OnlineCharacters(Vec<CharacterState>),

    /// event_spawn notification — a world event appeared on the map.
    EventSpawn(WorldEvent),

    /// event_removed notification — a world event expired or was defeated.
    EventRemoved(String),

    /// grandexchange_sell_order or grandexchange_buy_order notification.
    ///
    /// A player posted a new Grand Exchange order.
    GEOrderCreated(GEOrder),

    /// grandexchange_buy or grandexchange_sell notification.
    ///
    /// A Grand Exchange transaction completed.
    GETransactionCompleted(GETransaction),

    /// achievement_unlocked notification — a character earned an achievement.
    AchievementUnlocked {
        /// Character name that unlocked the achievement.
        character: String,
        /// Achievement name.
        achievement_name: String,
    },

    /// announcement notification — server-wide message from admins.
    Announcement(String),

    /// Maps fully fetched and tile sprites settled (download complete or failed).
    ///
    /// Marks the end of map tile preload during loading screen.
    MapsFetched,

    /// Characters fetched from REST API on startup (`GET /my/characters`).
    ///
    /// Pre-populates character cards with full stat data before WebSocket events arrive.
    CharactersFetched(Vec<CharacterState>),

    // ── Debugging ────────────────────────────────────────────────────────────

    /// Raw WebSocket message (JSON string) for debugging purposes.
    RawWsMessage(String),

    /// System log event from background tasks (image downloads, REST requests, etc.).
    ///
    /// `tag` is a short bracket label like "[IMG↓]" (downloading) or "[IMG✓]" (success).
    /// `message` is the detail (filename, error reason, etc.).
    SystemLog {
        /// Log event tag/category (e.g., "[IMG↓]", "[IMG✓]", "[IMG✗]").
        tag: String,
        /// Log message detail.
        message: String,
    },
}
