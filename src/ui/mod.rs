//! Terminal UI layer — rendering, component management, and event handling.
//!
//! ## Sub-modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`tui`] | Crossterm backend, alternate-screen lifecycle, and the event-polling loop |
//! | [`image_cache`] | Shared HTTP image downloader with on-disk cache and [`ProtocolCache`] renderer |
//! | [`minimap`] | 3×3 tile-sprite minimap renderer used inside the sidebar |
//! | [`components`] | All ratatui [`Component`] implementations (cards, sidebar, panels, etc.) |
//!
//! The UI is built on [ratatui](https://github.com/ratatui-org/ratatui) for
//! layout and drawing, with [tachyonfx](https://github.com/junkdog/tachyonfx)
//! driving the per-card boot animations.
//!
//! [`ProtocolCache`]: image_cache::ProtocolCache
//! [`Component`]: components::Component

pub mod components;
pub mod image_cache;
pub mod minimap;
pub mod tui;
