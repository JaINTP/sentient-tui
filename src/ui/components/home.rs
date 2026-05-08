//! Home screen component — placeholder root view.
//!
//! This component acts as the default root-level view and is reserved for
//! future top-level layout composition.  The active main screen is rendered
//! by [`super::character_cards::CharacterCards`], [`super::sidebar::Sidebar`],
//! and [`super::log_panel::LogPanel`] directly from [`crate::app::App`].

use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{core::action::Action, core::config::Config};

/// Placeholder home-screen component.
///
/// Currently renders a static test string and is not wired into the active
/// layout.  Extend this to build a top-level routing layer if the application
/// grows to support multiple primary views.
#[derive(Default)]
pub struct Home {
    /// Action bus sender — stored for future use.
    command_tx: Option<UnboundedSender<Action>>,
    /// Global configuration — keybindings and styles.
    config: Config,
}

impl Home {
    /// Create a new [`Home`] component with default state.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Component for Home {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::Tick | Action::Render => {}
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        frame.render_widget(Paragraph::new("hello world"), area);
        Ok(())
    }
}
