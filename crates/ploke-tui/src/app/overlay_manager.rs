use crossterm::event::KeyEvent;
use ratatui::prelude::Frame;

use crate::app::input;
use crate::app::overlay::OverlayAction;
use crate::app::view::components::config_overlay::{
    ConfigOverlayState, render_config_overlay,
};

#[derive(Debug)]
pub enum ActiveOverlay {
    Config(ConfigOverlayState),
}

#[derive(Debug, Default)]
pub struct OverlayManager {
    active: Option<ActiveOverlay>,
}

impl OverlayManager {
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn is_config_open(&self) -> bool {
        matches!(self.active, Some(ActiveOverlay::Config(_)))
    }

    pub fn open_config(&mut self, state: ConfigOverlayState) {
        self.active = Some(ActiveOverlay::Config(state));
    }

    pub fn close_active(&mut self) {
        self.active = None;
    }

    pub fn handle_input(&mut self, key: KeyEvent) -> Vec<OverlayAction> {
        let actions = Vec::new();
        let Some(active) = self.active.as_mut() else {
            return actions;
        };

        let close = match active {
            ActiveOverlay::Config(state) => {
                input::config_overlay::handle_config_overlay_input(state, key)
            }
        };

        if close {
            self.active = None;
        }

        actions
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        if let Some(active) = self.active.as_mut() {
            match active {
                ActiveOverlay::Config(state) => {
                    render_config_overlay(frame, state);
                }
            }
        }
    }
}
