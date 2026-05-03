use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotSummary {
    pub name: String,
    pub level: u32,
    pub hp: i32,
    pub max_hp: i32,
    pub x: i32,
    pub y: i32,
    pub layer: String,
    pub current_task: Option<String>,
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStatus {
    pub bot_count: usize,
    pub bank_items: usize,
    pub bots: Vec<BotSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEntry {
    pub name: String,
    pub current_goal: Option<String>,
    pub position: [serde_json::Value; 3],
    pub hp_pct: f32,
    pub cooldown_remaining: u32,
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTaskSpec {
    pub action: String,
    pub code: Option<String>,
    pub quantity: Option<u32>,
    pub x: Option<i32>,
    pub y: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub bot_name: Option<String>,
    pub task: CommandTaskSpec,
}

/// Fetch the summary of all bots
pub async fn fetch_bot_summaries(
    client: &Client,
    control_url: &str,
) -> reqwest::Result<Vec<BotSummary>> {
    let url = format!("{}/bots", control_url.trim_end_matches('/'));
    let resp = client.get(&url).send().await?.error_for_status()?;
    resp.json().await
}

/// Fetch the full detailed info for a specific bot
pub async fn fetch_bot_detail(
    client: &Client,
    control_url: &str,
    name: &str,
) -> reqwest::Result<serde_json::Value> {
    let url = format!("{}/bots/{}", control_url.trim_end_matches('/'), name);
    let resp = client.get(&url).send().await?.error_for_status()?;
    resp.json().await
}

/// Send a manual task command
pub async fn send_command(
    client: &Client,
    control_url: &str,
    req: CommandRequest,
) -> reqwest::Result<()> {
    let url = format!("{}/command", control_url.trim_end_matches('/'));
    client.post(&url).json(&req).send().await?.error_for_status()?;
    Ok(())
}

/// Pause a bot
pub async fn pause_bot(client: &Client, control_url: &str, name: &str) -> reqwest::Result<()> {
    let url = format!("{}/bots/{}/pause", control_url.trim_end_matches('/'), name);
    client.post(&url).send().await?.error_for_status()?;
    Ok(())
}

/// Resume a bot
pub async fn resume_bot(client: &Client, control_url: &str, name: &str) -> reqwest::Result<()> {
    let url = format!("{}/bots/{}/resume", control_url.trim_end_matches('/'), name);
    client.post(&url).send().await?.error_for_status()?;
    Ok(())
}

/// Rest a bot
pub async fn rest_bot(client: &Client, control_url: &str, name: &str) -> reqwest::Result<()> {
    let url = format!("{}/bots/{}/rest", control_url.trim_end_matches('/'), name);
    client.post(&url).send().await?.error_for_status()?;
    Ok(())
}

/// Fetch the swarm's current demand board
pub async fn fetch_swarm_demand(
    client: &Client,
    control_url: &str,
) -> reqwest::Result<std::collections::HashMap<String, u32>> {
    let url = format!("{}/swarm/demand", control_url.trim_end_matches('/'));
    let resp = client.get(&url).send().await?.error_for_status()?;
    resp.json().await
}
