use crossterm::event::{KeyCode, KeyEvent};

use crate::app::overlay::{OverlayAction, OverlayKind};
use crate::app::overlay::{OverlayNavAction, map_close_key, map_navigation_key};
use crate::app::view::components::approvals::ApprovalsState;

pub fn handle_approvals_input(overlay: &mut ApprovalsState, key: KeyEvent) -> Vec<OverlayAction> {
    let mut actions = Vec::new();

    if let Some(action) = map_close_key(key, OverlayKind::Approvals) {
        actions.push(action);
        return actions;
    }

    if let Some(nav) = map_navigation_key(key) {
        match nav {
            OverlayNavAction::Up => overlay.select_prev(),
            OverlayNavAction::Down => overlay.select_next(),
            OverlayNavAction::PageUp => overlay.scroll_up(10),
            OverlayNavAction::PageDown => overlay.scroll_down(10),
            OverlayNavAction::Home => overlay.scroll_y = 0,
            // Clamp-to-end happens during render based on actual content height.
            OverlayNavAction::End => overlay.scroll_y = u16::MAX,
            _ => {}
        }
        return actions;
    }

    match key.code {
        KeyCode::Enter | KeyCode::Char('y') => {
            actions.push(OverlayAction::ApproveSelectedProposal);
        }
        KeyCode::Char('n') | KeyCode::Char('d') => {
            actions.push(OverlayAction::DenySelectedProposal);
        }
        KeyCode::Char('?') => {
            overlay.help_visible = !overlay.help_visible;
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            overlay.increase_view_lines();
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            overlay.decrease_view_lines();
        }
        KeyCode::Char('u') => {
            overlay.toggle_unlimited();
        }
        KeyCode::Char('f') => {
            overlay.cycle_filter();
        }
        KeyCode::Char('v') => {
            overlay.toggle_diff_view();
        }
        KeyCode::Char('o') => {
            actions.push(OverlayAction::OpenSelectedProposalInEditor);
        }
        _ => {}
    }
    actions
}
