/// REST API client for ArtifactsMMO.
///
/// Two one-shot startup fetches:
///   1. `GET /my/characters`  — pre-populate character cards with full stat data.
///   2. `GET /maps` (paginated) — populate the minimap tile cache.
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, info, warn};

use crate::{
    core::action::Action,
    core::game::{CharacterState, GameState, MapTile},
    ui::image_cache::{ImageCache, SharedImageCache},
};

const BASE_URL: &str = "https://api.artifactsmmo.com";

// ── Characters ────────────────────────────────────────────────────────────────

/// Fetch `GET /my/characters` and dispatch `Action::CharactersFetched`.
pub async fn fetch_my_characters(token: String, bot_control_url: Option<String>, tx: UnboundedSender<Action>) {
    let client = match build_client() {
        Ok(c) => c,
        Err(e) => {
            error!("REST: client build failed: {e}");
            return;
        }
    };

    if let Some(control_url) = bot_control_url {
        info!("REST: fetching bots from {control_url}");
        match crate::api::bot::fetch_bot_summaries(&client, &control_url).await {
            Ok(summaries) => {
                let mut characters = Vec::with_capacity(summaries.len());
                for sum in summaries {
                    match crate::api::bot::fetch_bot_detail(&client, &control_url, &sum.name).await {
                        Ok(detail) => {
                            let mut cs = CharacterState {
                                name: sum.name.clone(),
                                ..Default::default()
                            };
                            cs.apply_full_schema(&detail);
                            characters.push(cs);
                        }
                        Err(e) => warn!("REST: failed to fetch details for {}: {e}", sum.name),
                    }
                }
                if !characters.is_empty() {
                    info!("REST: dispatching CharactersFetched ({} bots)", characters.len());
                    let _ = tx.send(Action::CharactersFetched(characters));
                }
            }
            Err(e) => warn!("REST: /bots request failed: {e}"),
        }
        return;
    }

    let url = format!("{BASE_URL}/my/characters");
    info!("REST: fetching {url}");



    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .send()
        .await;

    let resp = match resp {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            warn!("REST: /my/characters returned {}", r.status());
            return;
        }
        Err(e) => {
            warn!("REST: /my/characters request failed: {e}");
            return;
        }
    };

    let json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            warn!("REST: failed to parse /my/characters: {e}");
            return;
        }
    };

    let data = match json
        .get("data")
        .and_then(|v| v.as_array())
    {
        Some(arr) => arr,
        None => {
            warn!("REST: /my/characters missing 'data' array");
            return;
        }
    };

    let mut characters: Vec<CharacterState> = Vec::with_capacity(data.len());
    for node in data {
        let name = node
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let account = node
            .get("account")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        let mut cs = CharacterState {
            name: name.clone(),
            account,
            ..Default::default()
        };
        cs.apply_full_schema(node);
        info!("REST: loaded character '{name}'");
        characters.push(cs);
    }

    if !characters.is_empty() {
        info!("REST: dispatching CharactersFetched ({} chars)", characters.len());
        let _ = tx.send(Action::CharactersFetched(characters));
    }
}

/// Spawn a one-shot tokio task for the character fetch.
pub fn spawn_character_fetch(token: String, bot_control_url: Option<String>, tx: UnboundedSender<Action>) {
    tokio::spawn(async move {
        fetch_my_characters(token, bot_control_url, tx).await;
    });
}

/// Spawn a background task to periodically fetch swarm demand from the local Bot Control API.
pub fn spawn_demand_poll(bot_control_url: String, tx: UnboundedSender<Action>) {
    tokio::spawn(async move {
        let client = match build_client() {
            Ok(c) => c,
            Err(e) => {
                error!("REST: client build failed for demand poll: {e}");
                return;
            }
        };

        loop {
            match crate::api::bot::fetch_swarm_demand(&client, &bot_control_url).await {
                Ok(demand_map) => {
                    let mut demand: Vec<(String, u32)> = demand_map.into_iter().collect();
                    // Sort by quantity descending
                    demand.sort_by(|a, b| b.1.cmp(&a.1));
                    let _ = tx.send(Action::SwarmDemandFetched(demand));
                }
                Err(e) => {
                    warn!("REST: failed to fetch swarm demand: {e}");
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

// ── Map tiles ─────────────────────────────────────────────────────────────────

/// Paginate through `GET /maps`, populate `GameState::map_tiles`, then
/// prefetch every unique tile sprite from the image cache so they are
/// ready before the minimap needs to render them.
pub async fn fetch_all_maps(
    token: String,
    bot_sync_url: Option<String>,
    tx: UnboundedSender<Action>,
    game_state: Arc<RwLock<GameState>>,
    image_cache: SharedImageCache,
) {
    let client = match build_client() {
        Ok(c) => c,
        Err(e) => {
            error!("REST: client build failed for maps: {e}");
            return;
        }
    };

    let base = bot_sync_url.unwrap_or_else(|| BASE_URL.to_string());
    let mut page = 1u32;
    let page_size = 10000u32;
    let mut total_fetched = 0usize;

    loop {
        let url = format!("{base}/maps?page={page}&size={page_size}");
        info!("REST: fetching {url}");

        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json")
            .send()
            .await;

        let resp = match resp {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                warn!("REST: /maps page {page} returned {}", r.status());
                break;
            }
            Err(e) => {
                warn!("REST: /maps request failed: {e}");
                break;
            }
        };

        let json: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                warn!("REST: failed to parse /maps: {e}");
                break;
            }
        };

        let (items, total_pages) = extract_page(&json);

        {
            let mut gs = match game_state.write() {
                Ok(g) => g,
                Err(_) => {
                    error!("REST: map state lock poisoned");
                    break;
                }
            };

            for tile_val in &items {
                let x = tile_val
                    .get("x")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                let y = tile_val
                    .get("y")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                let map_id = tile_val
                    .get("map_id")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                let layer = tile_val
                    .get("layer")
                    .and_then(|v| v.as_str())
                    .unwrap_or("overworld")
                    .to_string();
                let skin = tile_val
                    .get("skin")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = tile_val
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let (content_type, content_code) = extract_content(tile_val);

                if map_id != 0 {
                    gs.map_id_to_layer
                        .insert(map_id, layer.clone());
                }
                gs.map_tiles.insert(
                    (x, y, layer.clone()),
                    MapTile {
                        x,
                        y,
                        map_id,
                        layer,
                        skin,
                        name,
                        content_type,
                        content_code,
                    },
                );
            }
        } // write guard released

        total_fetched += items.len();
        info!("REST: /maps page {page}/{total_pages} — {total_fetched} tiles");

        if page >= total_pages || items.is_empty() {
            break;
        }
        page += 1;

        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    info!("REST: map fetch complete — {total_fetched} tiles cached");

    // ── Prefetch every unique tile sprite ─────────────────────────────────────
    // Collect all distinct non-empty skins, then fire a background download for
    // each.  The ImageCache deduplicates concurrent requests automatically.
    let unique_skins: HashSet<String> = {
        let gs = game_state
            .read()
            .unwrap_or_else(|e| e.into_inner());
        gs.map_tiles
            .values()
            .filter(|t| !t.skin.is_empty())
            .map(|t| t.skin.clone())
            .collect()
    };

    info!("REST: prefetching {} unique map sprites", unique_skins.len());
    for skin in &unique_skins {
        ImageCache::prefetch(&image_cache, "maps", skin);
        // Tiny yield so we don't hammer the tokio task queue all at once.
        tokio::task::yield_now().await;
    }

    // Wait for every map sprite to finish (downloaded, disk-cached, or failed)
    // before signalling MapsFetched.  This guarantees the loading screen only
    // transitions once all tile images are actually available.
    loop {
        let all_settled = unique_skins
            .iter()
            .all(|skin| ImageCache::is_settled(&image_cache, "maps", skin));
        if all_settled {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let _ = tx.send(Action::MapsFetched);
}

/// Spawn a one-shot tokio task for the map fetch.
pub fn spawn_map_fetch(
    token: String,
    bot_sync_url: Option<String>,
    tx: UnboundedSender<Action>,
    game_state: Arc<RwLock<GameState>>,
    image_cache: SharedImageCache,
) {
    tokio::spawn(async move {
        fetch_all_maps(token, bot_sync_url, tx, game_state, image_cache).await;
    });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
}

/// Extract the items array and total_pages from a paginated API response.
///
/// ArtifactsMMO uses two response shapes:
///   A) `{ "data": { "items": [...], "pages": N } }` — wrapped
///   B) `{ "data": [...], "pages": N, "total": T }` — flat (most endpoints)
fn extract_page(json: &serde_json::Value) -> (Vec<serde_json::Value>, u32) {
    let data = json.get("data").unwrap_or(json);
    if let Some(items) = data
        .get("items")
        .and_then(|v| v.as_array())
    {
        // Shape A: pages lives inside the data object.
        let pages = data
            .get("pages")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;
        (items.clone(), pages)
    } else if let Some(arr) = data.as_array() {
        // Shape B: data IS the array; pages lives at the top level of the response.
        let pages = json
            .get("pages")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;
        (arr.clone(), pages)
    } else {
        (Vec::new(), 1)
    }
}

/// Extract content type and code from a tile JSON value.
///
/// The API schema has changed over time:
///   - New (current): `{ "interactions": { "content": { "type": "...", "code": "..." } } }`
///   - Old (fallback): `{ "content": { "type": "...", "code": "..." } }`
fn extract_content(tile: &serde_json::Value) -> (String, String) {
    // Try new schema first: interactions.content
    let content = tile
        .get("interactions")
        .and_then(|v| v.get("content"))
        .filter(|v| v.is_object())
        // Fallback to old flat schema: content at top level
        .or_else(|| {
            tile.get("content")
                .filter(|v| v.is_object())
        });

    if let Some(c) = content {
        let ct = c
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let cc = c
            .get("code")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        (ct, cc)
    } else {
        (String::new(), String::new())
    }
}
