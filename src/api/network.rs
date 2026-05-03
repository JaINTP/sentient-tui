//! WebSocket connection to ArtifactsMMO realtime server.
//!
//! Handles authentication, message parsing, and automatic reconnection.
//! All notifications are dispatched as Actions through the app's action bus.

use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::core::action::Action;
use crate::core::game::{AccountLogEntry, CharacterState, GEOrder, GETransaction, WorldEvent};

/// Real-time WebSocket server endpoint.
const WS_URL: &str = "wss://realtime.artifactsmmo.com";
/// Delay before reconnect attempt after disconnect (5 seconds).
const RECONNECT_DELAY: Duration = Duration::from_secs(5);
/// Interval between ping messages to keep connection alive (30 seconds).
const PING_INTERVAL: Duration = Duration::from_secs(30);

/// Spawn the WebSocket listener as a background task.
///
/// Connects to the realtime server, authenticates with the token, and dispatches
/// all notifications as Actions through `action_tx`. Handles reconnection automatically
/// on disconnect. The `cancel` token allows graceful shutdown from the main loop.
///
/// Authentication: sends `{"token": "<ARTIFACTS_TOKEN>"}` after connection.
/// Omitting `subscriptions` subscribes to all notification types by default.
pub fn spawn_ws_listener(
    token: String,
    action_tx: UnboundedSender<Action>,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if cancel.is_cancelled() {
                break;
            }

            let ws_endpoint = WS_URL.to_string();

            info!("connecting to websocket: {}", ws_endpoint);
            let _ = action_tx.send(Action::WsConnect);

            match connect_async(&ws_endpoint).await {
                Ok((ws_stream, _response)) => {
                    info!("websocket connected, authenticating");

                    let (mut sink, mut stream) = ws_stream.split();

                    // Send auth message immediately after connecting.
                    let auth = serde_json::json!({ "token": token });
                    if sink
                        .send(Message::Text(auth.to_string().into()))
                        .await
                        .is_err()
                    {
                        error!("failed to send auth message");
                        let _ = action_tx.send(Action::WsDisconnected("auth failed".into()));
                        continue;
                    }

                    let _ = action_tx.send(Action::WsConnected);
                    info!("websocket authenticated");

                    let mut ping_interval = tokio::time::interval(PING_INTERVAL);

                    loop {
                        tokio::select! {
                            _ = cancel.cancelled() => {
                                let _ = sink.close().await;
                                return;
                            }
                            _ = ping_interval.tick() => {
                                if sink.send(Message::Ping(vec![].into())).await.is_err() {
                                    warn!("ping failed, reconnecting");
                                    break;
                                }
                            }
                            msg = stream.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        let text_str = text.to_string();
                                        let _ = action_tx.send(Action::RawWsMessage(text_str.clone()));
                                        parse_and_dispatch(&text_str, &action_tx);
                                    }
                                    Some(Ok(Message::Pong(_))) => {}
                                    Some(Ok(Message::Close(_))) => {
                                        info!("server sent close frame");
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        error!("websocket error: {e}");
                                        break;
                                    }
                                    None => {
                                        info!("websocket stream ended");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("websocket connection failed: {e}");
                }
            }

            let _ = action_tx.send(Action::WsDisconnected("connection lost".into()));
            info!("reconnecting in {}s", RECONNECT_DELAY.as_secs());

            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(RECONNECT_DELAY) => {}
            }
        }
    })
}

/// Parse a raw JSON WebSocket notification and dispatch typed Actions.
///
/// Matches on the `type` field and extracts the `data` payload to construct
/// specific Action variants. Unrecognized notification types are silently ignored.
///
/// Real notification types from `wss://realtime.artifactsmmo.com`:
///   - account_log — character action (fight, gather, craft, etc.)
///   - online_characters — current online player list
///   - event_spawn — world event appeared
///   - event_removed — world event removed
///   - grandexchange_sell_order / grandexchange_buy_order — GE order posted
///   - grandexchange_buy / grandexchange_sell — GE transaction completed
///   - achievement_unlocked — character earned achievement
///   - announcement — server-wide message
///   - pending_item_received, version, test — no special action needed
fn parse_and_dispatch(raw: &str, tx: &UnboundedSender<Action>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        warn!("received non-json ws message");
        return;
    };

    let Some(notification_type) = value
        .get("type")
        .and_then(|v| v.as_str())
    else {
        return;
    };

    let data = &value["data"];

    match notification_type {
        // ── Per-character action log ────────────────────────────────────────
        "account_log" => {
            if let Some(entry) = parse_account_log(data) {
                let _ = tx.send(Action::AccountLog(entry));
            }
        }

        // ── Online character list ───────────────────────────────────────────
        "online_characters" => {
            let chars = parse_online_characters(data);
            if !chars.is_empty() {
                let _ = tx.send(Action::OnlineCharacters(chars));
            }
        }

        // ── World events ────────────────────────────────────────────────────
        "event_spawn" => {
            if let Some(evt) = parse_world_event(data) {
                let _ = tx.send(Action::EventSpawn(evt));
            }
        }
        "event_removed" => {
            let code = json_str(data, "code");
            let _ = tx.send(Action::EventRemoved(code));
        }

        // ── Grand Exchange orders ───────────────────────────────────────────
        "grandexchange_sell_order" => {
            if let Some(order) = parse_ge_order(data, "sell") {
                let _ = tx.send(Action::GEOrderCreated(order));
            }
        }
        "grandexchange_buy_order" => {
            if let Some(order) = parse_ge_order(data, "buy") {
                let _ = tx.send(Action::GEOrderCreated(order));
            }
        }

        // ── Grand Exchange transactions ─────────────────────────────────────
        "grandexchange_sell" => {
            if let Some(txn) = parse_ge_transaction(data, "sell") {
                let _ = tx.send(Action::GETransactionCompleted(txn));
            }
        }
        "grandexchange_buy" => {
            if let Some(txn) = parse_ge_transaction(data, "buy") {
                let _ = tx.send(Action::GETransactionCompleted(txn));
            }
        }

        // ── Achievements ────────────────────────────────────────────────────
        "achievement_unlocked" => {
            let character = json_str(data, "character");
            // The achievement name may live at data.achievement.name or data.name
            let achievement_name = data
                .get("achievement")
                .and_then(|a| a.get("name"))
                .and_then(|v| v.as_str())
                .or_else(|| {
                    data.get("name")
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("unknown achievement")
                .to_string();
            let _ = tx.send(Action::AchievementUnlocked {
                character,
                achievement_name,
            });
        }

        // ── Server announcement ─────────────────────────────────────────────
        "announcement" => {
            let text = data
                .get("message")
                .or_else(|| data.get("text"))
                .or_else(|| data.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or(raw)
                .to_string();
            let _ = tx.send(Action::Announcement(text));
        }

        // pending_item_received, version, test — no special action needed
        _ => {}
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Parsers
// ────────────────────────────────────────────────────────────────────────────

fn parse_account_log(data: &serde_json::Value) -> Option<AccountLogEntry> {
    Some(AccountLogEntry {
        character: json_str(data, "character"),
        account: json_str(data, "account"),
        log_type: json_str(data, "type"),
        description: json_str(data, "description"),
        cooldown: data
            .get("cooldown")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32,
        content: data
            .get("content")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    })
}

/// Handles both a bare array and the paginated `{"items": [...], ...}` envelope.
fn parse_online_characters(data: &serde_json::Value) -> Vec<CharacterState> {
    let arr = if let Some(items) = data
        .get("items")
        .and_then(|v| v.as_array())
    {
        items.as_slice()
    } else if let Some(arr) = data.as_array() {
        arr.as_slice()
    } else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|c| {
            let name = json_str(c, "name");
            if name.is_empty() {
                return None;
            }
            Some(CharacterState {
                name,
                account: json_str(c, "account"),
                skin: json_str(c, "skin"),
                x: c.get("x")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32,
                y: c.get("y")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32,
                // Position snapshots don't carry action info; use defaults
                ..Default::default()
            })
        })
        .collect()
}

fn parse_world_event(data: &serde_json::Value) -> Option<WorldEvent> {
    let name = json_str(data, "name");
    if name.is_empty() {
        return None;
    }
    let expiration = data
        .get("expiration")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Some(WorldEvent {
        name,
        code: json_str(data, "code"),
        expiration,
    })
}

fn parse_ge_order(data: &serde_json::Value, order_type: &str) -> Option<GEOrder> {
    let code = json_str(data, "code");
    if code.is_empty() {
        return None;
    }
    Some(GEOrder {
        order_type: order_type.to_string(),
        code,
        quantity: data
            .get("quantity")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        price: data
            .get("price")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        account: data
            .get("account")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
    })
}

fn parse_ge_transaction(data: &serde_json::Value, order_type: &str) -> Option<GETransaction> {
    let code = json_str(data, "code");
    if code.is_empty() {
        return None;
    }
    Some(GETransaction {
        order_type: order_type.to_string(),
        code,
        quantity: data
            .get("quantity")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        total_price: data
            .get("total_price")
            .or_else(|| data.get("price"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
    })
}

fn json_str(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}
