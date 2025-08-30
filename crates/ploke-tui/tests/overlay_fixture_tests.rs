//! Overlay + fixture-backed render tests using the global TEST_APP.
//! This leverages the lazy-static app to avoid repeated heavy setup across tests.

use ratatui::{backend::TestBackend, Terminal};
use ratatui::layout::Rect;

use ploke_tui::app::view::components::approvals::{render_approvals_overlay, ApprovalsState};
use ploke_tui::app_state::core::{EditProposal, EditProposalStatus, DiffPreview};

fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
    let buf = term.backend().buffer();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let ch = buf.cell((x, y)).expect("cell").symbol().chars().next().unwrap_or(' ');
            out.push(ch);
        }
        out.push('\n');
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_harness")]
async fn overlay_renders_with_fixture_app_state() {
    let state = ploke_tui::test_harness::get_state().await;
    // Clear any existing proposals to avoid interference
    {
        let mut reg = state.proposals.write().await;
        reg.clear();
    }

    // Stage a simple proposal into the shared AppState
    let req_id = uuid::Uuid::from_u128(0x11111111_2222_3333_4444_555555555555);
    {
        let mut reg = state.proposals.write().await;
        reg.insert(
            req_id,
            EditProposal {
                request_id: req_id,
                parent_id: uuid::Uuid::new_v4(),
                call_id: "call-fixture".into(),
                proposed_at_ms: chrono::Utc::now().timestamp_millis(),
                edits: vec![],
                files: vec![std::env::current_dir().unwrap().join("Cargo.toml")],
                preview: DiffPreview::UnifiedDiff { text: "sample diff".into() },
                status: EditProposalStatus::Pending,
            },
        );
    }

    // Render via the component using the App's shared state
    let mut term = Terminal::new(TestBackend::new(80, 24)).expect("terminal");
    let ui = ApprovalsState::default();
    tokio::task::block_in_place(|| {
        term.draw(|f| {
            let area = Rect::new(0, 0, 80, 24);
            let _ = render_approvals_overlay(f, area, &state, &ui);
        })
        .expect("draw");
    });
    let text = buffer_to_string(&term);
    assert!(text.contains(" Approvals "));
    assert!(text.contains(" Pending Proposals "));
}
