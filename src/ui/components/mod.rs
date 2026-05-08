//! UI components — modular, reusable rendering and interaction units.
//!
//! Every component implements the [`Component`] trait which provides a unified
//! lifecycle for registration, event handling, state updates, and rendering.
//!
//! ## Available components
//!
//! | Module | Status | Purpose |
//! |--------|--------|---------|
//! | [`character_cards`] | Active | 3-column animated grid of character status cards |
//! | [`loading_screen`] | Active | Boot splash with image-download progress bar |
//! | [`log_panel`] | Active | Scrollable system and action log footer |
//! | [`fps`] | Active | Minimal FPS counter overlay |
//! | [`sidebar`] | Inactive (unused) | Character detail + minimap sidebar |
//! | [`focus_panel`] | Inactive (unused) | Focused single-character detail view |
//! | [`home`] | Inactive (unused) | Placeholder root view |
//! | [`world_panel`] | Inactive (unused) | World events + Grand Exchange feed |
//!
//! "Inactive" components compile and are tested but are not wired into the
//! primary layout.  They are available as drop-in replacements.

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};
use tokio::sync::mpsc::UnboundedSender;

use crate::{core::action::Action, core::config::Config, ui::tui::Event};

pub mod character_cards;
#[allow(dead_code)]
pub mod focus_panel;
pub mod fps;
#[allow(dead_code)]
pub mod home;
pub mod loading_screen;
pub mod log_panel;
#[allow(dead_code)]
pub mod sidebar;
pub mod world_panel;

/// Lifecycle and rendering contract for all UI components.
///
/// The trait mirrors a simplified ECS component model: `App` calls the
/// appropriate method on every registered component once per event or frame.
///
/// ## Implementation notes
///
/// - All methods have default no-op implementations except [`draw`], which
///   **must** be provided.
/// - Methods returning `color_eyre::Result<Option<Action>>` may emit a
///   follow-up [`Action`] that will be re-dispatched by `App::handle_actions`.
///   Return `Ok(None)` when no follow-up is needed.
///
/// [`draw`]: Component::draw
pub trait Component {
    /// Register the action bus sender so the component can dispatch [`Action`]s.
    ///
    /// Called once at startup, before the first [`draw`][Self::draw] call.
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        let _ = tx;
        Ok(())
    }

    /// Register the global configuration.
    ///
    /// Called once at startup after [`register_action_handler`][Self::register_action_handler].
    /// Use to cache keybinding or style settings from [`Config`].
    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        let _ = config;
        Ok(())
    }

    /// One-shot initialisation called after all registrations are complete.
    ///
    /// `area` is the initial terminal size.  Override to perform setup that
    /// requires knowing the viewport dimensions (e.g. pre-computing layouts).
    fn init(&mut self, area: ratatui::layout::Size) -> color_eyre::Result<()> {
        let _ = area;
        Ok(())
    }

    /// Dispatch a raw TUI event to the appropriate specialised handler.
    ///
    /// The default implementation routes [`Event::Key`] to
    /// [`handle_key_event`][Self::handle_key_event] and [`Event::Mouse`] to
    /// [`handle_mouse_event`][Self::handle_mouse_event]; all other event types
    /// are silently ignored.
    fn handle_events(&mut self, event: Option<Event>) -> color_eyre::Result<Option<Action>> {
        let action = match event {
            Some(Event::Key(key_event)) => self.handle_key_event(key_event)?,
            Some(Event::Mouse(mouse_event)) => self.handle_mouse_event(mouse_event)?,
            _ => None,
        };
        Ok(action)
    }

    /// Handle a keyboard key event. Override to implement custom keybindings.
    fn handle_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<Option<Action>> {
        let _ = key;
        Ok(None)
    }

    /// Handle a mouse event (click, scroll, move, etc.). Override for mouse support.
    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> color_eyre::Result<Option<Action>> {
        let _ = mouse;
        Ok(None)
    }

    /// Update component state in response to an [`Action`].
    ///
    /// Called by `App::handle_actions` for every action that passes through
    /// the action bus.  Returns `Some(action)` to emit a follow-up action that
    /// will be re-dispatched in the same iteration; returns `Ok(None)` when no
    /// follow-up is needed.
    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        let _ = action;
        Ok(None)
    }

    /// Render the component into `area` on the current `frame`.
    ///
    /// Called on every [`Event::Render`] (60 FPS by default).  This is the only
    /// method without a default implementation and **must** be provided by every
    /// concrete component.
    ///
    /// # Errors
    ///
    /// Return an error only for unrecoverable rendering failures.  Transient
    /// issues (e.g. a temporarily unavailable image) should be handled
    /// gracefully by rendering a placeholder rather than propagating an error.
    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()>;
}
