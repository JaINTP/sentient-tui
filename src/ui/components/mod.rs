//! UI components — modular reusable elements (character cards, sidebar, log panel, etc.).
//!
//! All components implement the `Component` trait for unified event handling and rendering.

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

/// Trait for visual and interactive UI components.
///
/// All components implement this trait to provide a uniform interface for
/// registration, event handling, state updates, and rendering.
pub trait Component {
    /// Register the action bus sender so the component can dispatch events.
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        let _ = tx;
        Ok(())
    }

    /// Register the global config so the component can access keybindings and styles.
    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        let _ = config;
        Ok(())
    }

    /// Initialize the component with the terminal size.
    ///
    /// Called once at startup after all registrations are complete.
    fn init(&mut self, area: ratatui::layout::Size) -> color_eyre::Result<()> {
        let _ = area;
        Ok(())
    }

    /// Handle a TUI event (keyboard, mouse, resize, etc.).
    ///
    /// Default implementation dispatches to specialized handlers.
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

    /// Update component state in response to an action.
    ///
    /// Called every frame for actions that pass through the action bus.
    /// Return Some(action) to dispatch a follow-up action.
    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        let _ = action;
        Ok(None)
    }

    /// Render the component into the given frame area.
    ///
    /// Called every frame (60 FPS by default). Must be implemented by all components.
    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()>;
}
