use crate::app_state::core::{DiffPreview, EditProposal, EditProposalStatus};
use crate::rag::editing::{approve_pending_edits, deny_pending_edits};
use crate::test_utils::new_test_harness::AppHarness;
use ploke_core::ArcStr;
use std::path::PathBuf;
use uuid::Uuid;

fn make_pending_proposal(request_id: Uuid, proposed_at_ms: i64, file_path: &str) -> EditProposal {
    EditProposal {
        request_id,
        parent_id: Uuid::new_v4(),
        call_id: ArcStr::from("test_call_id"),
        proposed_at_ms,
        edits: Vec::new(),
        files: vec![PathBuf::from(file_path)],
        edits_ns: Vec::new(),
        preview: DiffPreview::UnifiedDiff {
            text: String::new(),
        },
        status: EditProposalStatus::Pending,
        is_semantic: true,
    }
}

#[tokio::test]
#[cfg(feature = "test_harness")]
async fn approve_pending_edits_applies_newest_and_marks_overlap_stale() {
    let harness = AppHarness::spawn().await.expect("spawn harness");

    let new_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    let old_id = Uuid::new_v4();

    let newer = make_pending_proposal(new_id, 2000, "tests/fixture_crates/a.rs");
    let other = make_pending_proposal(other_id, 1500, "tests/fixture_crates/b.rs");
    let older = make_pending_proposal(old_id, 1000, "tests/fixture_crates/a.rs");

    {
        let mut reg = harness.state.proposals.write().await;
        reg.insert(new_id, newer);
        reg.insert(other_id, other);
        reg.insert(old_id, older);
    }

    approve_pending_edits(&harness.state, &harness.event_bus).await;

    let reg = harness.state.proposals.read().await;
    assert!(
        matches!(reg.get(&new_id).unwrap().status, EditProposalStatus::Applied),
        "newest overlapping proposal should be applied"
    );
    assert!(
        matches!(reg.get(&other_id).unwrap().status, EditProposalStatus::Applied),
        "non-overlapping proposal should be applied"
    );
    assert!(
        matches!(reg.get(&old_id).unwrap().status, EditProposalStatus::Stale(_)),
        "older overlapping proposal should be marked stale"
    );
}

#[tokio::test]
#[cfg(feature = "test_harness")]
async fn deny_pending_edits_marks_all_pending_denied() {
    let harness = AppHarness::spawn().await.expect("spawn harness");

    let first_id = Uuid::new_v4();
    let second_id = Uuid::new_v4();

    let first = make_pending_proposal(first_id, 1000, "tests/fixture_crates/a.rs");
    let second = make_pending_proposal(second_id, 1100, "tests/fixture_crates/b.rs");

    {
        let mut reg = harness.state.proposals.write().await;
        reg.insert(first_id, first);
        reg.insert(second_id, second);
    }

    deny_pending_edits(&harness.state, &harness.event_bus).await;

    let reg = harness.state.proposals.read().await;
    assert!(
        matches!(reg.get(&first_id).unwrap().status, EditProposalStatus::Denied),
        "pending proposals should be denied"
    );
    assert!(
        matches!(reg.get(&second_id).unwrap().status, EditProposalStatus::Denied),
        "pending proposals should be denied"
    );
}
