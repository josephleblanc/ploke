use crossterm::event::{KeyCode, KeyEvent};

use crate::app::overlay::{OverlayAction, OverlayKind};
use crate::app::view::components::approvals::ApprovalsState;

pub fn handle_approvals_input(
    overlay: &mut ApprovalsState,
    key: KeyEvent,
) -> Vec<OverlayAction> {
    let mut actions = Vec::new();
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            actions.push(OverlayAction::CloseOverlay(OverlayKind::Approvals));
            return actions;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            overlay.select_prev();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            overlay.select_next();
        }
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
        KeyCode::Char('o') => {
            actions.push(OverlayAction::OpenSelectedProposalInEditor);
        }
        _ => {}
    }
    actions
}
