use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ploke_tui::app::input::{approvals, context_browser};
use ploke_tui::app::overlay::{OverlayAction, OverlayKind};
use ploke_tui::app::view::components::approvals::ApprovalsState;
use ploke_tui::app::view::components::context_browser::{ContextBrowserMode, ContextSearchState};

#[test]
fn context_browser_quits_with_close_action() {
    let mut overlay = ContextSearchState::with_items("query".to_string(), Vec::new());
    overlay.mode = ContextBrowserMode::Normal;

    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let actions = context_browser::handle_context_browser_input(&mut overlay, key);

    assert_eq!(actions.len(), 1);
    assert!(matches!(
        actions[0],
        OverlayAction::CloseOverlay(OverlayKind::ContextBrowser)
    ));
}

#[test]
fn approvals_enter_emits_approve_action() {
    let mut overlay = ApprovalsState::default();
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let actions = approvals::handle_approvals_input(&mut overlay, key);

    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], OverlayAction::ApproveSelectedProposal));
}
