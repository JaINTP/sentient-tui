//! # Sentient TUI — Terminal Client for ArtifactsMMO
//!
//! A high-performance terminal UI for the ArtifactsMMO game, built with ratatui, tokio, and tachyonfx.
//!
//! ## Architecture
//!
//! - **Real-time WebSocket**: Connects to `wss://realtime.artifactsmmo.com` for live game events
//! - **REST API**: Fetches character and map data from `https://api.artifactsmmo.com` at startup
//! - **Action Bus**: All UI events flow through an mpsc channel (`Action` enum) to a central dispatcher
//! - **Shared State**: `Arc<RwLock<GameState>>` holds character data, world events, and GE feed
//! - **Image Cache**: Background downloads + disk caching of sprites via `Arc<Mutex<ImageCache>>`
//!
//! ## Key Components
//!
//! - **CharacterCards**: 3-column grid of animated character status cards with portraits, stats, skills, gear
//! - **Sidebar**: Real-time info panel showing WS status, economy, world events, GE feed
//! - **LogPanel**: Footer log of all in-game actions (fights, crafting, trades, etc.)
//! - **FpsCounter**: Performance metrics overlay (ticks/sec, frames/sec)
//! - **LoadingScreen**: Initial splash screen with asset download progress bar

use clap::Parser;
use cli::Cli;

use crate::app::App;

pub mod api;
mod app;
mod cli;
pub mod core;
pub mod logging;
pub mod ui;

/// Entry point — parses CLI args, initializes subsystems, and runs the TUI event loop.
#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    // Load .env from the current directory (or any parent). Silently ignored if
    // no .env file is found — env vars set in the shell still take precedence.
    let _ = dotenvy::dotenv();

    crate::core::errors::init()?;
    crate::logging::init()?;

    let args = Cli::parse();
    args.apply();
    let mut app = App::new(args.tick_rate, args.frame_rate)?;
    app.run().await?;
    Ok(())
}
