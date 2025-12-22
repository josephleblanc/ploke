use crossterm::event::{KeyCode, KeyEvent};

use crate::app::view::components::config_overlay::{ConfigOverlayPane, ConfigOverlayState};

pub fn handle_config_overlay_input(overlay: &mut ConfigOverlayState, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            return true;
        }
        KeyCode::Char('?') => {
            overlay.help_visible = !overlay.help_visible;
        }
        KeyCode::Tab => {
            overlay.next_pane();
        }
        KeyCode::BackTab => {
            overlay.prev_pane();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            overlay.prev_pane();
        }
        KeyCode::Right | KeyCode::Char('l') => {
            overlay.next_pane();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            overlay.move_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            overlay.move_down();
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            overlay.activate();
        }
        KeyCode::Char('c') => {
            overlay.pane = ConfigOverlayPane::Categories;
            overlay.normalize_indices();
        }
        KeyCode::Char('s') => {
            overlay.pane = ConfigOverlayPane::Items;
            overlay.normalize_indices();
        }
        KeyCode::Char('v') => {
            overlay.pane = ConfigOverlayPane::Values;
            overlay.normalize_indices();
        }
        _ => {}
    }
    false
}
