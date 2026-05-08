//! Build script — injects compile-time version metadata via `vergen-gix`.
//!
//! Emits the following `cargo:rustc-env` variables for use with `env!()`:
//!
//! - `VERGEN_BUILD_*` — build timestamp and host information.
//! - `VERGEN_GIT_*` — git branch, commit SHA, and dirty status from the
//!   working tree (via `gix`).
//! - `VERGEN_CARGO_*` — Cargo target triple and feature flags.
//!
//! These values are surfaced in the `--version` output and can be displayed
//! in the loading screen or `--version` flag output.

use anyhow::Result;
use vergen_gix::{BuildBuilder, CargoBuilder, Emitter, GixBuilder};

/// Inject compile-time build metadata using `vergen-gix`.
///
/// # Errors
///
/// Returns an error if any of the `vergen-gix` builders fail to collect
/// metadata or if the [`Emitter`] cannot write the generated `cargo:rustc-env`
/// lines to stdout.
fn main() -> Result<()> {
    let build = BuildBuilder::all_build()?;
    let gix = GixBuilder::all_git()?;
    let cargo = CargoBuilder::all_cargo()?;
    Emitter::default()
        .add_instructions(&build)?
        .add_instructions(&gix)?
        .add_instructions(&cargo)?
        .emit()
}
