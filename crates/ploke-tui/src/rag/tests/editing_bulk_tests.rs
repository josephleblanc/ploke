use crate::app_state::core::{DiffPreview, EditProposal, EditProposalStatus};
use crate::rag::editing::{approve_pending_edits, deny_pending_edits};
use crate::test_utils::new_test_harness::AppHarness;
use ploke_core::{ArcStr, PROJECT_NAMESPACE_UUID, WriteSnippetData};
use ploke_io::read::read_and_compute_filehash;
use std::{fs, path::PathBuf};
use tempfile::tempdir;
use uuid::Uuid;

async fn make_pending_proposal(
    request_id: Uuid,
    proposed_at_ms: i64,
    file_path: PathBuf,
    start_byte: usize,
    end_byte: usize,
    replacement: &str,
) -> EditProposal {
    let file_hash = read_and_compute_filehash(&file_path, PROJECT_NAMESPACE_UUID)
        .await
        .expect("compute file hash");
    EditProposal {
        request_id,
        parent_id: Uuid::new_v4(),
        call_id: ArcStr::from("test_call_id"),
        proposed_at_ms,
        edits: vec![WriteSnippetData {
            id: Uuid::new_v4(),
            name: "test_edit".to_string(),
            file_path: file_path.clone(),
            expected_file_hash: file_hash.hash,
            start_byte,
            end_byte,
            replacement: replacement.to_string(),
            namespace: PROJECT_NAMESPACE_UUID,
        }],
        files: vec![file_path],
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
    let tmp = tempdir().expect("tempdir");
    let a_path = tmp.path().join("a.rs");
    let b_path = tmp.path().join("b.rs");
    fs::write(&a_path, "fn original_a() {}\n").expect("write a.rs");
    fs::write(&b_path, "fn original_b() {}\n").expect("write b.rs");

    let new_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    let old_id = Uuid::new_v4();

    let newer = make_pending_proposal(
        new_id,
        2000,
        a_path.clone(),
        0,
        "fn original_a() {}\n".len(),
        "fn newer_a() {}\n",
    )
    .await;
    let other = make_pending_proposal(
        other_id,
        1500,
        b_path.clone(),
        0,
        "fn original_b() {}\n".len(),
        "fn other_b() {}\n",
    )
    .await;
    let older = make_pending_proposal(
        old_id,
        1000,
        a_path.clone(),
        0,
        "fn original_a() {}\n".len(),
        "fn older_a() {}\n",
    )
    .await;

    {
        let mut reg = harness.state.proposals.write().await;
        reg.insert(new_id, newer);
        reg.insert(other_id, other);
        reg.insert(old_id, older);
    }

    approve_pending_edits(&harness.state, &harness.event_bus).await;

    let reg = harness.state.proposals.read().await;
    assert!(
        matches!(
            reg.get(&new_id).unwrap().status,
            EditProposalStatus::Applied
        ),
        "newest overlapping proposal should be applied"
    );
    assert!(
        matches!(
            reg.get(&other_id).unwrap().status,
            EditProposalStatus::Applied
        ),
        "non-overlapping proposal should be applied"
    );
    assert!(
        matches!(
            reg.get(&old_id).unwrap().status,
            EditProposalStatus::Stale(_)
        ),
        "older overlapping proposal should be marked stale"
    );
}

#[tokio::test]
#[cfg(feature = "test_harness")]
async fn deny_pending_edits_marks_all_pending_denied() {
    let harness = AppHarness::spawn().await.expect("spawn harness");
    let tmp = tempdir().expect("tempdir");
    let a_path = tmp.path().join("a.rs");
    let b_path = tmp.path().join("b.rs");
    fs::write(&a_path, "fn deny_a() {}\n").expect("write a.rs");
    fs::write(&b_path, "fn deny_b() {}\n").expect("write b.rs");

    let first_id = Uuid::new_v4();
    let second_id = Uuid::new_v4();

    let first = make_pending_proposal(
        first_id,
        1000,
        a_path,
        0,
        "fn deny_a() {}\n".len(),
        "fn deny_a_changed() {}\n",
    )
    .await;
    let second = make_pending_proposal(
        second_id,
        1100,
        b_path,
        0,
        "fn deny_b() {}\n".len(),
        "fn deny_b_changed() {}\n",
    )
    .await;

    {
        let mut reg = harness.state.proposals.write().await;
        reg.insert(first_id, first);
        reg.insert(second_id, second);
    }

    deny_pending_edits(&harness.state, &harness.event_bus).await;

    let reg = harness.state.proposals.read().await;
    assert!(
        matches!(
            reg.get(&first_id).unwrap().status,
            EditProposalStatus::Denied
        ),
        "pending proposals should be denied"
    );
    assert!(
        matches!(
            reg.get(&second_id).unwrap().status,
            EditProposalStatus::Denied
        ),
        "pending proposals should be denied"
    );
}
