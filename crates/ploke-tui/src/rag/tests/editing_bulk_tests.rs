use crate::app_state::core::{
    DiffPreview, EditProposal, EditProposalStatus, derive_edit_proposal_id,
};
use crate::app_state::events::SystemEvent;
use crate::rag::editing::{approve_edits, approve_pending_edits, deny_pending_edits};
use crate::test_utils::new_test_harness::AppHarness;
use crate::{AppEvent, EventPriority};
use ploke_core::file_hash::{FileHash, LargeFilePolicy};
use ploke_core::{ArcStr, PROJECT_NAMESPACE_UUID, WriteSnippetData};
use ploke_io::read::read_and_compute_filehash;
use ploke_io::{Diff, NsWriteSnippetData, PatchApplyOptions};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::tempdir;
use tokio::time::{Duration, timeout};
use uuid::Uuid;

const SIMPLE_NS_PATCH_DIFF: &str = r#"--- a/notes.txt
+++ b/notes.txt
@@ -1,3 +1,3 @@
 alpha
-beta
+delta
 gamma
"#;

const TODO_NS_PATCH_DIFF: &str = r#"--- a/todo.txt
+++ b/todo.txt
@@ -1,3 +1,3 @@
 alpha
-beta
+delta
 gamma
"#;

async fn index_temp_crate(harness: &AppHarness, root: &Path, lib_source: &str) {
    fs::create_dir_all(root.join("src")).expect("create temp crate src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"editing-bulk-test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write temp crate manifest");
    fs::write(root.join("src").join("lib.rs"), lib_source).expect("write temp crate lib");
    crate::app_state::handlers::indexing::index_workspace(
        &harness.state,
        &harness.event_bus,
        Some(crate::app_state::IndexTargetDir::new(root.to_path_buf())),
        true,
    )
    .await;
    let parse_failure = harness
        .state
        .with_system_read(|sys| sys.last_parse_failure().cloned())
        .await;
    assert!(
        parse_failure.is_none(),
        "temp crate should index before edit approval: {parse_failure:?}"
    );
}

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
    let call_id = ArcStr::from("test_call_id");
    EditProposal {
        proposal_id: derive_edit_proposal_id(request_id, &call_id),
        request_id,
        parent_id: Uuid::new_v4(),
        call_id,
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
    let harness = AppHarness::spawn_fresh_fixture_nodes()
        .await
        .expect("spawn harness");
    let tmp = tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src");
    fs::create_dir_all(&src_dir).expect("create temp crate src");
    let a_path = src_dir.join("a.rs");
    let b_path = src_dir.join("b.rs");
    fs::write(&a_path, "fn original_a() {}\n").expect("write a.rs");
    fs::write(&b_path, "fn original_b() {}\n").expect("write b.rs");
    index_temp_crate(&harness, tmp.path(), "mod a;\nmod b;\n").await;

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
        reg.insert(newer.proposal_id, newer);
        reg.insert(other.proposal_id, other);
        reg.insert(older.proposal_id, older);
    }

    approve_pending_edits(&harness.state, &harness.event_bus).await;

    let reg = harness.state.proposals.read().await;
    assert!(
        matches!(
            reg.get(&derive_edit_proposal_id(
                new_id,
                &ArcStr::from("test_call_id")
            ))
            .unwrap()
            .status,
            EditProposalStatus::Applied
        ),
        "newest overlapping proposal should be applied"
    );
    assert!(
        matches!(
            reg.get(&derive_edit_proposal_id(
                other_id,
                &ArcStr::from("test_call_id")
            ))
            .unwrap()
            .status,
            EditProposalStatus::Applied
        ),
        "non-overlapping proposal should be applied"
    );
    assert!(
        matches!(
            reg.get(&derive_edit_proposal_id(
                old_id,
                &ArcStr::from("test_call_id")
            ))
            .unwrap()
            .status,
            EditProposalStatus::Stale(_)
        ),
        "older overlapping proposal should be marked stale"
    );
}

#[tokio::test]
#[cfg(feature = "test_harness")]
async fn semantic_batch_results_follow_edit_paths() {
    let harness = AppHarness::spawn_fresh_fixture_nodes()
        .await
        .expect("spawn harness");
    let tmp = tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src");
    fs::create_dir_all(&src_dir).expect("create temp crate src");
    let a_path = src_dir.join("a.rs");
    let b_path = src_dir.join("b.rs");
    let a_source = "fn original_a() {}\n";
    let b_source = "fn original_b() {}\n";
    fs::write(&a_path, a_source).expect("write a.rs");
    fs::write(&b_path, b_source).expect("write b.rs");
    index_temp_crate(&harness, tmp.path(), "mod a;\nmod b;\n").await;

    let a_hash = read_and_compute_filehash(&a_path, PROJECT_NAMESPACE_UUID)
        .await
        .expect("compute a.rs hash");
    let b_hash = read_and_compute_filehash(&b_path, PROJECT_NAMESPACE_UUID)
        .await
        .expect("compute b.rs hash");
    fs::write(&a_path, "fn changed_before_apply() {}\n").expect("make a.rs stale");

    let request_id = Uuid::new_v4();
    let call_id = ArcStr::from("semantic_path_receipts");
    let proposal_id = derive_edit_proposal_id(request_id, &call_id);
    {
        let mut proposals = harness.state.proposals.write().await;
        proposals.insert(
            proposal_id,
            EditProposal {
                proposal_id,
                request_id,
                parent_id: Uuid::new_v4(),
                call_id: call_id.clone(),
                proposed_at_ms: chrono::Utc::now().timestamp_millis(),
                edits: vec![
                    WriteSnippetData {
                        id: Uuid::new_v4(),
                        name: "edit_b".to_string(),
                        file_path: b_path.clone(),
                        expected_file_hash: b_hash.hash,
                        start_byte: 0,
                        end_byte: b_source.len(),
                        replacement: "fn applied_b() {}\n".to_string(),
                        namespace: PROJECT_NAMESPACE_UUID,
                    },
                    WriteSnippetData {
                        id: Uuid::new_v4(),
                        name: "edit_a".to_string(),
                        file_path: a_path.clone(),
                        expected_file_hash: a_hash.hash,
                        start_byte: 0,
                        end_byte: a_source.len(),
                        replacement: "fn should_not_apply_a() {}\n".to_string(),
                        namespace: PROJECT_NAMESPACE_UUID,
                    },
                ],
                files: vec![a_path.clone(), b_path.clone()],
                edits_ns: Vec::new(),
                preview: DiffPreview::UnifiedDiff {
                    text: String::new(),
                },
                status: EditProposalStatus::Pending,
                is_semantic: true,
            },
        );
    }

    let mut event_rx = harness.event_bus.subscribe(EventPriority::Realtime);
    approve_edits(&harness.state, &harness.event_bus, proposal_id).await;
    let payload = timeout(Duration::from_secs(2), async {
        loop {
            if let AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id: observed_id,
                call_id: observed_call,
                content,
                ..
            }) = event_rx.recv().await.expect("semantic event bus dropped")
                && observed_id == request_id
                && observed_call == call_id
            {
                break serde_json::from_str::<serde_json::Value>(&content)
                    .expect("parse semantic completion");
            }
        }
    })
    .await
    .expect("semantic batch should settle");

    let results = payload["results"]
        .as_array()
        .expect("semantic completion results");
    let result_for = |path: &Path| {
        results
            .iter()
            .find(|result| result["file_path"] == path.display().to_string())
            .unwrap_or_else(|| panic!("missing result for {}", path.display()))
    };
    assert!(
        result_for(&b_path).get("new_file_hash").is_some(),
        "successful write receipt should remain paired with b.rs"
    );
    assert!(
        result_for(&a_path).get("error").is_some(),
        "stale write receipt should remain paired with a.rs"
    );
    assert_eq!(
        fs::read_to_string(&b_path).expect("read b.rs"),
        "fn applied_b() {}\n"
    );
    assert_eq!(
        fs::read_to_string(&a_path).expect("read a.rs"),
        "fn changed_before_apply() {}\n"
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
        reg.insert(first.proposal_id, first);
        reg.insert(second.proposal_id, second);
    }

    deny_pending_edits(&harness.state, &harness.event_bus).await;

    let reg = harness.state.proposals.read().await;
    assert!(
        matches!(
            reg.get(&derive_edit_proposal_id(
                first_id,
                &ArcStr::from("test_call_id")
            ))
            .unwrap()
            .status,
            EditProposalStatus::Denied
        ),
        "pending proposals should be denied"
    );
    assert!(
        matches!(
            reg.get(&derive_edit_proposal_id(
                second_id,
                &ArcStr::from("test_call_id")
            ))
            .unwrap()
            .status,
            EditProposalStatus::Denied
        ),
        "pending proposals should be denied"
    );
}

#[tokio::test]
#[cfg(feature = "test_harness")]
async fn approve_edits_marks_mixed_result_ns_batch_as_partially_applied() {
    let harness = AppHarness::spawn_fresh_fixture_nodes()
        .await
        .expect("spawn harness");
    let tmp = tempdir().expect("tempdir");
    let ok_path = tmp.path().join("notes.txt");
    let stale_path = tmp.path().join("todo.txt");
    let initial = "alpha\nbeta\ngamma\n";
    fs::write(&ok_path, initial).expect("write notes.txt");
    fs::write(&stale_path, initial).expect("write todo.txt");
    index_temp_crate(&harness, tmp.path(), "pub fn fixture() {}\n").await;

    let ok_hash = FileHash::from_bytes(initial.as_bytes());
    let stale_hash = FileHash::from_bytes(initial.as_bytes());

    // Force the second edit to fail after staging while keeping the first valid.
    fs::write(&stale_path, "alpha\nbeta\ngamma\nextra\n").expect("mutate stale file");

    let request_id = Uuid::new_v4();
    let call_id = ArcStr::from("test_ns_call_id");
    let proposal_id = derive_edit_proposal_id(request_id, &call_id);
    {
        let mut proposals = harness.state.proposals.write().await;
        proposals.insert(
            proposal_id,
            EditProposal {
                proposal_id,
                request_id,
                parent_id: Uuid::new_v4(),
                call_id: call_id.clone(),
                proposed_at_ms: chrono::Utc::now().timestamp_millis(),
                edits: vec![],
                edits_ns: vec![
                    NsWriteSnippetData {
                        id: Uuid::new_v4(),
                        file_path: ok_path.clone(),
                        expected_file_hash: Some(ok_hash),
                        namespace: PROJECT_NAMESPACE_UUID,
                        diff: Diff::from(SIMPLE_NS_PATCH_DIFF.to_string()),
                        options: PatchApplyOptions::default(),
                        large_file_policy: LargeFilePolicy::Skip,
                    },
                    NsWriteSnippetData {
                        id: Uuid::new_v4(),
                        file_path: stale_path.clone(),
                        expected_file_hash: Some(stale_hash),
                        namespace: PROJECT_NAMESPACE_UUID,
                        diff: Diff::from(TODO_NS_PATCH_DIFF.to_string()),
                        options: PatchApplyOptions::default(),
                        large_file_policy: LargeFilePolicy::Skip,
                    },
                ],
                files: vec![ok_path.clone(), stale_path.clone()],
                preview: DiffPreview::UnifiedDiff {
                    text: String::new(),
                },
                status: EditProposalStatus::Pending,
                is_semantic: false,
            },
        );
    }

    let mut event_rx = harness.event_bus.subscribe(EventPriority::Realtime);
    approve_edits(&harness.state, &harness.event_bus, proposal_id).await;

    let mut completed_payload = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        match timeout(Duration::from_millis(50), event_rx.recv()).await {
            Ok(Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id: event_request_id,
                call_id: event_call_id,
                content,
                ..
            }))) if event_request_id == request_id && event_call_id == call_id => {
                completed_payload = Some(
                    serde_json::from_str::<serde_json::Value>(&content)
                        .expect("parse ToolCallCompleted payload"),
                );
                break;
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => {}
        }
    }

    let completed_payload =
        completed_payload.expect("expected ToolCallCompleted payload for ns apply");
    assert_eq!(completed_payload["ok"], false);
    assert_eq!(completed_payload["applied"], 1);
    assert_eq!(completed_payload["partial"], true);
    assert_eq!(
        completed_payload["results"]
            .as_array()
            .expect("results array")
            .iter()
            .filter(|entry| entry.get("error").is_some())
            .count(),
        1,
        "payload should include one failed file result"
    );

    let proposals = harness.state.proposals.read().await;
    let proposal = proposals
        .get(&proposal_id)
        .expect("proposal should still exist after apply");
    assert!(
        matches!(proposal.status, EditProposalStatus::PartiallyApplied(_)),
        "mixed-result ns batches should be surfaced as PartiallyApplied; got {:?}",
        proposal.status
    );

    assert_eq!(
        fs::read_to_string(&ok_path).expect("read notes.txt"),
        "alpha\ndelta\ngamma\n",
        "the successful file should be updated"
    );
    assert_eq!(
        fs::read_to_string(&stale_path).expect("read todo.txt"),
        "alpha\nbeta\ngamma\nextra\n",
        "the stale-hash file should remain unchanged by the failed edit"
    );
}
