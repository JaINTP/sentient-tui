//! Logging subsystem initialisation.
//!
//! Configures a file-based [`tracing_subscriber`] that writes structured log
//! records to `<data_dir>/<binary_name>.log`.  ANSI colour codes are stripped
//! so the log file is readable by standard text tools.
//!
//! ## Log-level resolution (highest priority wins)
//!
//! 1. `RUST_LOG` environment variable — standard Rust log filter string.
//! 2. `SENTIENT_TUI_LOG_LEVEL` environment variable (application-specific).
//! 3. Hard-coded default: `INFO`.
//!
//! Source file names and line numbers are included on every record.

use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::core::config;

lazy_static::lazy_static! {
    /// Environment variable name used to configure the log level for this application.
    ///
    /// Resolved to `<PROJECT_NAME>_LOG_LEVEL` (e.g. `SENTIENT_TUI_LOG_LEVEL`).
    /// Takes effect only when `RUST_LOG` is not set.
    pub static ref LOG_ENV: String = format!("{}_LOG_LEVEL", config::PROJECT_NAME.clone());

    /// Log file name — `<binary_name>.log` — written inside the data directory.
    pub static ref LOG_FILE: String = format!("{}.log", env!("CARGO_PKG_NAME"));
}

/// Initialise the tracing subscriber and wire it into the global registry.
///
/// Creates the data directory if it does not already exist, then opens (or
/// truncates) the log file.  A structured [`tracing_subscriber::fmt`] layer
/// with source-location info is registered alongside [`ErrorLayer`] for
/// `color_eyre` span-trace capture.
///
/// # Errors
///
/// Returns an error if the data directory cannot be created, the log file
/// cannot be opened, the `LOG_ENV` variable contains an invalid filter string,
/// or the global subscriber has already been set.
pub fn init() -> color_eyre::Result<()> {
    let directory = config::get_data_dir();
    std::fs::create_dir_all(directory.clone())?;
    let log_path = directory.join(LOG_FILE.clone());
    let log_file = std::fs::File::create(log_path)?;

    // Build the filter: prefer RUST_LOG, fall back to LOG_ENV, then INFO.
    let env_filter = EnvFilter::builder().with_default_directive(tracing::Level::INFO.into());
    let env_filter = env_filter.try_from_env().or_else(|_| {
        env_filter
            .with_env_var(LOG_ENV.clone())
            .from_env()
    })?;

    let file_subscriber = fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_writer(log_file)
        .with_target(false)
        .with_ansi(false) // plain text — no terminal colour codes in the log file
        .with_filter(env_filter);

    tracing_subscriber::registry()
        .with(file_subscriber)
        .with(ErrorLayer::default())
        .try_init()?;

    Ok(())
}
