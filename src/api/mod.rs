//! Network and REST API integration layer.
//!
//! This module contains all outbound network communication for sentient-tui:
//!
//! - [`network`] — long-lived WebSocket connection to `wss://realtime.artifactsmmo.com`
//!   for real-time game event notifications.
//! - [`rest`] — one-shot startup fetches against `https://api.artifactsmmo.com` to
//!   pre-populate character data and map tiles before the first WebSocket event arrives.
//! - [`bot`] — optional integration with a local Bot Control API (`BOT_CONTROL_API_URL`)
//!   for swarm management and manual command dispatch.
//!
//! All three modules communicate with the rest of the application exclusively through
//! the [`crate::core::action::Action`] bus (an `mpsc` channel), keeping I/O concerns
//! cleanly separated from UI state.

pub mod bot;
pub mod network;
pub mod rest;
