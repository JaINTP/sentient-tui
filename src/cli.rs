//! Command-line argument parsing and configuration.
//!
//! Handles CLI flags for tick rate, frame rate, and cache refresh.
//! Also provides version information including build date and git SHA.

use std::path::PathBuf;

use clap::Parser;
use directories::BaseDirs;
use tracing::info;

use crate::core::config::{get_config_dir, get_data_dir};

/// CLI arguments for the Sentient TUI.
///
/// Parsed from `--tick-rate`, `--frame-rate`, and `--refresh-cache` flags.
/// Also provides custom version string with build metadata.
#[derive(Parser, Debug)]
#[command(author, version = version(), about)]
pub struct Cli {
    /// Tick rate — number of game logic ticks per second (default: 4.0).
    ///
    /// Controls how often the action bus is polled for game state updates.
    #[arg(short, long, value_name = "FLOAT", default_value_t = 4.0)]
    pub tick_rate: f64,

    /// Frame rate — number of screen renders per second (default: 60.0).
    ///
    /// Controls terminal refresh frequency. Higher values consume more CPU.
    #[arg(short, long, value_name = "FLOAT", default_value_t = 60.0)]
    pub frame_rate: f64,

    /// Wipe the local image cache before starting.
    ///
    /// Forces all sprite assets (characters, items, maps) to be re-downloaded
    /// from the CDN on the next run instead of loading from disk cache.
    #[arg(long, default_value_t = false)]
    pub refresh_cache: bool,
}

impl Cli {
    /// Perform any pre-startup side-effects dictated by CLI flags.
    ///
    /// Currently handles `--refresh-cache` by removing the image cache directory.
    /// Call this immediately after parsing, before building `App`.
    pub fn apply(&self) {
        if self.refresh_cache {
            let cache_dir: PathBuf = BaseDirs::new()
                .map(|bd| {
                    bd.cache_dir()
                        .join("sentient-tui")
                        .join("images")
                })
                .unwrap_or_else(|| PathBuf::from(".cache/images"));

            if cache_dir.exists() {
                match std::fs::remove_dir_all(&cache_dir) {
                    Ok(()) => info!("--refresh-cache: cleared {}", cache_dir.display()),
                    Err(e) => eprintln!("--refresh-cache: failed to clear cache: {e}"),
                }
            } else {
                info!("--refresh-cache: cache dir does not exist, nothing to clear");
            }
        }
    }
}

const VERSION_MESSAGE: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "-",
    env!("VERGEN_GIT_DESCRIBE"),
    " (",
    env!("VERGEN_BUILD_DATE"),
    ")"
);

/// Generate the version string shown in `--version` output.
///
/// Includes version, build date, git SHA, and paths to config/data directories.
pub fn version() -> String {
    let author = clap::crate_authors!();

    // let current_exe_path = PathBuf::from(clap::crate_name!()).display().to_string();
    let config_dir_path = get_config_dir().display().to_string();
    let data_dir_path = get_data_dir().display().to_string();

    format!(
        "\
{VERSION_MESSAGE}

Authors: {author}

Config directory: {config_dir_path}
Data directory: {data_dir_path}"
    )
}
