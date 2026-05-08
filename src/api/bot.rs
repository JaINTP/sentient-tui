//! Local Bot Control API client.
//!
//! Provides typed request/response models and async helper functions for
//! interacting with the local bot control server (if configured via
//! `BOT_CONTROL_API_URL`).  This is an optional integration layer — the
//! standard ArtifactsMMO REST and WebSocket connections work independently.
//!
//! ## Endpoints used
//!
//! | Method | Path | Purpose |
//! |--------|------|---------|
//! | GET | `/bots` | List all bot summaries |
//! | GET | `/bots/{name}` | Full detail for one bot |
//! | POST | `/command` | Dispatch a manual task |
//! | POST | `/bots/{name}/pause` | Pause a running bot |
//! | POST | `/bots/{name}/resume` | Resume a paused bot |
//! | POST | `/bots/{name}/rest` | Force a bot to rest immediately |
//! | GET | `/swarm/demand` | Retrieve the swarm demand board |

use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Lightweight bot snapshot returned by `GET /bots`.
///
/// Contains enough information to populate the character card grid without
/// needing a full detail fetch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotSummary {
    /// Unique bot name (also used as the URL path segment).
    pub name: String,
    /// Current character level.
    pub level: u32,
    /// Current hit points.
    pub hp: i32,
    /// Maximum hit points at full health.
    pub max_hp: i32,
    /// Map X coordinate.
    pub x: i32,
    /// Map Y coordinate.
    pub y: i32,
    /// Map layer the bot is currently on (e.g. `"overworld"`).
    pub layer: String,
    /// Human-readable description of the bot's active task, if any.
    pub current_task: Option<String>,
    /// Whether the bot is currently paused and awaiting a resume command.
    pub paused: bool,
}

/// Aggregate status of the entire bot swarm returned by the control API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStatus {
    /// Total number of registered bots.
    pub bot_count: usize,
    /// Total number of distinct item types currently held in the shared bank.
    pub bank_items: usize,
    /// Per-bot summaries.
    pub bots: Vec<BotSummary>,
}

/// A single entry from the swarm's decision log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEntry {
    /// Bot name.
    pub name: String,
    /// Human-readable description of the bot's current objective.
    pub current_goal: Option<String>,
    /// `[x, y, layer]` position tuple as returned by the control API.
    pub position: [serde_json::Value; 3],
    /// Hit point percentage (0.0–100.0).
    pub hp_pct: f32,
    /// Remaining cooldown in seconds.
    pub cooldown_remaining: u32,
    /// Whether the bot is paused.
    pub paused: bool,
}

/// Task specification payload for a manual command dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTaskSpec {
    /// Action verb (e.g. `"move"`, `"fight"`, `"gather"`, `"craft"`).
    pub action: String,
    /// Resource, monster, or item code targeted by the action.
    pub code: Option<String>,
    /// Quantity for crafting or gathering actions.
    pub quantity: Option<u32>,
    /// Destination X coordinate for movement actions.
    pub x: Option<i32>,
    /// Destination Y coordinate for movement actions.
    pub y: Option<i32>,
}

/// Top-level request body sent to `POST /command`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    /// Name of the specific bot to command.  `None` broadcasts to all bots.
    pub bot_name: Option<String>,
    /// Task to execute.
    pub task: CommandTaskSpec,
}

/// Fetch the lightweight summary of all registered bots from `GET /bots`.
///
/// # Errors
///
/// Returns a [`reqwest::Error`] if the request fails or the server returns a
/// non-2xx status code.
pub async fn fetch_bot_summaries(
    client: &Client,
    control_url: &str,
) -> reqwest::Result<Vec<BotSummary>> {
    let url = format!("{}/bots", control_url.trim_end_matches('/'));
    let resp = client.get(&url).send().await?.error_for_status()?;
    resp.json().await
}

/// Fetch the full JSON detail object for a single bot from `GET /bots/{name}`.
///
/// Returns a raw [`serde_json::Value`] so callers can use
/// [`CharacterState::apply_full_schema`] without duplicating schema definitions.
///
/// # Errors
///
/// Returns a [`reqwest::Error`] if the request fails, the bot name is not
/// found (404), or the server returns a non-2xx status.
pub async fn fetch_bot_detail(
    client: &Client,
    control_url: &str,
    name: &str,
) -> reqwest::Result<serde_json::Value> {
    let url = format!("{}/bots/{}", control_url.trim_end_matches('/'), name);
    let resp = client.get(&url).send().await?.error_for_status()?;
    resp.json().await
}

/// Dispatch a manual task command to `POST /command`.
///
/// Use `req.bot_name = None` to broadcast the command to all bots in the swarm.
///
/// # Errors
///
/// Returns a [`reqwest::Error`] if the request fails or the server returns a
/// non-2xx status code.
pub async fn send_command(
    client: &Client,
    control_url: &str,
    req: CommandRequest,
) -> reqwest::Result<()> {
    let url = format!("{}/command", control_url.trim_end_matches('/'));
    client.post(&url).json(&req).send().await?.error_for_status()?;
    Ok(())
}

/// Pause a bot via `POST /bots/{name}/pause`.
///
/// The bot will finish its current action and then stop until
/// [`resume_bot`] is called.
///
/// # Errors
///
/// Returns a [`reqwest::Error`] if the request fails or the server returns a
/// non-2xx status code.
pub async fn pause_bot(client: &Client, control_url: &str, name: &str) -> reqwest::Result<()> {
    let url = format!("{}/bots/{}/pause", control_url.trim_end_matches('/'), name);
    client.post(&url).send().await?.error_for_status()?;
    Ok(())
}

/// Resume a paused bot via `POST /bots/{name}/resume`.
///
/// The bot will continue executing its task queue from where it left off.
///
/// # Errors
///
/// Returns a [`reqwest::Error`] if the request fails or the server returns a
/// non-2xx status code.
pub async fn resume_bot(client: &Client, control_url: &str, name: &str) -> reqwest::Result<()> {
    let url = format!("{}/bots/{}/resume", control_url.trim_end_matches('/'), name);
    client.post(&url).send().await?.error_for_status()?;
    Ok(())
}

/// Force a bot to immediately execute a rest action via `POST /bots/{name}/rest`.
///
/// Useful when a bot is low on HP and needs to recover before continuing.
///
/// # Errors
///
/// Returns a [`reqwest::Error`] if the request fails or the server returns a
/// non-2xx status code.
pub async fn rest_bot(client: &Client, control_url: &str, name: &str) -> reqwest::Result<()> {
    let url = format!("{}/bots/{}/rest", control_url.trim_end_matches('/'), name);
    client.post(&url).send().await?.error_for_status()?;
    Ok(())
}

/// Retrieve the swarm's current item demand board from `GET /swarm/demand`.
///
/// Returns a map of item code → required quantity, sorted by the REST layer
/// before dispatch.  The UI sidebar displays this list to help coordinate
/// manual bot tasking.
///
/// # Errors
///
/// Returns a [`reqwest::Error`] if the request fails or the server returns a
/// non-2xx status code.
pub async fn fetch_swarm_demand(
    client: &Client,
    control_url: &str,
) -> reqwest::Result<std::collections::HashMap<String, u32>> {
    let url = format!("{}/swarm/demand", control_url.trim_end_matches('/'));
    let resp = client.get(&url).send().await?.error_for_status()?;
    resp.json().await
}
