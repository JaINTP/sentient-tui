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
//! | GET | `/bank` | Retrieve the bank details |

use reqwest::Client;
use serde::{Deserialize, Serialize};

/// A single inventory slot as returned by the bot control server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventorySlot {
    pub slot: u32,
    pub code: String,
    pub quantity: u32,
}

/// Bot snapshot returned by `GET /bots` and `GET /bots/{name}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotSummary {
    pub name: String,
    pub level: u32,
    pub hp: i32,
    pub max_hp: i32,
    pub xp: u32,
    pub max_xp: u32,
    pub gold: u64,
    pub skin: String,
    pub status: String,
    pub running: bool,
    pub paused: bool,
}

/// Aggregate swarm status returned by `GET /status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStatus {
    pub bot_count: usize,
    pub active_units: usize,
    pub status: String,
    pub summaries: Vec<BotSummary>,
    pub bots: Vec<serde_json::Value>,
}

/// Bank details returned by `GET /bank`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankDetails {
    pub items: Vec<InventorySlot>,
    pub total_gold: u64,
}

/// A single entry from the swarm's decision log (`GET /decision-monitor`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEntry {
    pub name: String,
    pub current_goal: Option<String>,
    pub action: String,
    pub reasoning: String,
    /// `[x, y, layer]` position tuple.
    pub position: [serde_json::Value; 3],
    pub hp_pct: f32,
    pub cooldown_remaining: u32,
    pub paused: bool,
    pub level: u32,
    pub gold: u64,
    pub skin: String,
    pub inventory_count: usize,
    pub inventory_max: u32,
}

/// Task specification payload for `POST /command`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTaskSpec {
    pub action: String,
    pub code: Option<String>,
    pub quantity: Option<u32>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub slot: Option<String>,
}

/// Request body for `POST /command`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    /// Name of the bot to command. `None` broadcasts to all bots.
    pub bot_name: Option<String>,
    pub task: CommandTaskSpec,
}

/// Fetch the summary list of all registered bots from `GET /bots`.
pub async fn fetch_bot_summaries(
    client: &Client,
    control_url: &str,
) -> reqwest::Result<Vec<BotSummary>> {
    let url = format!("{}/bots", control_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?;
    resp.json().await
}

/// Fetch full detail for a single bot from `GET /bots/{name}`.
pub async fn fetch_bot_detail(
    client: &Client,
    control_url: &str,
    name: &str,
) -> reqwest::Result<serde_json::Value> {
    let url = format!("{}/bots/{}", control_url.trim_end_matches('/'), name);
    let resp = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?;
    resp.json().await
}

/// Dispatch a manual task command to `POST /command`.
pub async fn send_command(
    client: &Client,
    control_url: &str,
    req: CommandRequest,
) -> reqwest::Result<()> {
    let url = format!("{}/command", control_url.trim_end_matches('/'));
    client
        .post(&url)
        .json(&req)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// Pause a bot via `POST /bots/{name}/pause`.
pub async fn pause_bot(client: &Client, control_url: &str, name: &str) -> reqwest::Result<()> {
    let url = format!("{}/bots/{}/pause", control_url.trim_end_matches('/'), name);
    client
        .post(&url)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// Resume a paused bot via `POST /bots/{name}/resume`.
pub async fn resume_bot(client: &Client, control_url: &str, name: &str) -> reqwest::Result<()> {
    let url = format!("{}/bots/{}/resume", control_url.trim_end_matches('/'), name);
    client
        .post(&url)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// Force a bot to rest via `POST /bots/{name}/rest`.
pub async fn rest_bot(client: &Client, control_url: &str, name: &str) -> reqwest::Result<()> {
    let url = format!("{}/bots/{}/rest", control_url.trim_end_matches('/'), name);
    client
        .post(&url)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// Retrieve the swarm demand board from `GET /swarm/demand`.
pub async fn fetch_swarm_demand(
    client: &Client,
    control_url: &str,
) -> reqwest::Result<std::collections::HashMap<String, u32>> {
    let url = format!("{}/swarm/demand", control_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?;
    resp.json().await
}

/// Retrieve bank contents from `GET /bank`.
pub async fn fetch_bank(client: &Client, control_url: &str) -> reqwest::Result<BankDetails> {
    let url = format!("{}/bank", control_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?;
    resp.json().await
}

/// Blacklist an item from automated demands via `POST /demand/ignore`.
pub async fn dismiss_demand(
    client: &Client,
    control_url: &str,
    item_code: &str,
) -> reqwest::Result<()> {
    let url = format!("{}/demand/ignore", control_url.trim_end_matches('/'));
    client
        .post(&url)
        .json(&serde_json::json!({ "item_code": item_code }))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
