use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ploke_core::ArcStr;
use ploke_core::PROJECT_NAMESPACE_UUID;
use ploke_core::file_hash::{FileHash, LargeFilePolicy};
use ploke_core::rag_types::ApplyCodeEditResult;
use ploke_core::tool_types::{FunctionMarker, ToolName};
use ploke_io::{Diff, NsWriteSnippetData, PatchApplyOptions};
use ploke_llm::response::{FunctionCall, ToolCall};
use ploke_test_utils::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};
use tempfile::tempdir;
use tokio::time::{Instant, timeout};
use uuid::Uuid;

use crate::app::commands::harness::{TestAppAccessor, TestRuntime};
use crate::app_state::commands::StateCommand;
use crate::app_state::core::{
    DiffPreview, EditProposal, EditProposalStatus, derive_edit_proposal_id,
};
use crate::app_state::events::SystemEvent;
use crate::{AppEvent, EventPriority, emit_app_event};

const FIRST_SAME_FILE_DIFF: &str = r#"--- a/notes.txt
+++ b/notes.txt
@@ -1,4 +1,4 @@
 alpha
-beta
+beta-one
 gamma
 delta
"#;

const SECOND_SAME_FILE_DIFF: &str = r#"--- a/notes.txt
+++ b/notes.txt
@@ -1,4 +1,4 @@
alpha
beta
-gamma
+gamma-two
delta
"#;

fn make_ns_batch_proposal(
    request_id: Uuid,
    parent_id: Uuid,
    call_id: ArcStr,
    file_path: &Path,
    expected_file_hash: FileHash,
    diffs: &[&str],
) -> EditProposal {
    let proposal_id = derive_edit_proposal_id(request_id, &call_id);
    EditProposal {
        proposal_id,
        request_id,
        parent_id,
        call_id,
        proposed_at_ms: chrono::Utc::now().timestamp_millis(),
        edits: vec![],
        files: vec![file_path.to_path_buf(); diffs.len()],
        edits_ns: diffs
            .iter()
            .map(|diff| NsWriteSnippetData {
                id: Uuid::new_v4(),
                file_path: file_path.to_path_buf(),
                expected_file_hash: Some(expected_file_hash),
                namespace: PROJECT_NAMESPACE_UUID,
                diff: Diff::from((*diff).to_string()),
                options: PatchApplyOptions::default(),
                large_file_policy: LargeFilePolicy::Skip,
            })
            .collect(),
        preview: DiffPreview::UnifiedDiff {
            text: diffs.join("\n"),
        },
        status: EditProposalStatus::Pending,
        is_semantic: false,
    }
}

async fn configure_temp_workspace(state: &Arc<crate::app_state::AppState>, workspace_root: &Path) {
    let workspace_root = workspace_root.to_path_buf();
    let _ = state
        .with_system_txn(|txn| {
            txn.set_loaded_workspace(
                workspace_root.clone(),
                vec![workspace_root.clone()],
                Some(workspace_root.clone()),
            );
            txn.set_pwd(workspace_root.clone());
        })
        .await;

    let policy = state
        .with_system_read(|sys| sys.derive_path_policy(&[]).expect("path policy after load"))
        .await;

    state
        .io_handle
        .update_roots(Some(policy.roots.clone()), Some(policy.symlink_policy))
        .await;
}

fn write_named_fixture(workspace_root: &Path, relative_path: &str, contents: &str) -> PathBuf {
    let file_path = workspace_root.join(relative_path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).expect("create fixture parent");
    }
    fs::write(&file_path, contents).expect("write named fixture");
    file_path
}

fn same_file_tool_call(call_id: &str, diff: &str, reasoning: &str) -> ToolCall {
    ToolCall {
        call_id: ArcStr::from(call_id),
        call_type: FunctionMarker,
        function: FunctionCall {
            name: ToolName::NsPatch,
            arguments: serde_json::json!({
                "patches": [{
                    "file": "notes.txt",
                    "diff": diff,
                    "reasoning": reasoning,
                }]
            })
            .to_string(),
        },
    }
}

async fn stage_tool_call_via_llm_manager(
    request_id: Uuid,
    parent_id: Uuid,
    tool_call: ToolCall,
    realtime_rx: &mut tokio::sync::broadcast::Receiver<AppEvent>,
) -> ApplyCodeEditResult {
    let expected_call_id = tool_call.call_id.clone();
    emit_app_event(AppEvent::System(SystemEvent::ToolCallRequested {
        tool_call,
        request_id,
        parent_id,
    }))
    .await;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match timeout(Duration::from_millis(100), realtime_rx.recv()).await {
            Ok(Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id: event_request_id,
                call_id: event_call_id,
                content,
                ..
            }))) if event_request_id == request_id && event_call_id == expected_call_id => {
                return serde_json::from_str(&content)
                    .expect("parse ToolCallCompleted payload for staged ns_patch");
            }
            Ok(Ok(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id: event_request_id,
                call_id: event_call_id,
                error,
                ..
            }))) if event_request_id == request_id && event_call_id == expected_call_id => {
                panic!("ns_patch unexpectedly failed while staging: {error}");
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => {}
        }
    }

    panic!("timed out waiting for ToolCallCompleted while staging ns_patch");
}

async fn wait_for_proposal_status(
    state: &Arc<crate::app_state::AppState>,
    proposal_id: Uuid,
    predicate: impl Fn(&EditProposalStatus) -> bool,
) -> EditProposalStatus {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = {
            let guard = state.proposals.read().await;
            guard
                .get(&proposal_id)
                .map(|proposal| proposal.status.clone())
                .expect("proposal should exist while waiting for status")
        };
        if predicate(&status) {
            return status;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for proposal status transition: {status:?}");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ns_patch_same_file_staged_siblings_fail_second_approval_due_to_stale_anchor() {
    let fixture_db =
        Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));
    let rt = TestRuntime::new(&fixture_db)
        .spawn_state_manager()
        .spawn_event_bus()
        .spawn_llm_manager();

    let state = rt.state_arc();
    let events = rt.events_builder().build_event_bus_only();
    let mut realtime_rx = events.event_bus_events.realtime_tx_rx;

    let temp_dir = tempdir().expect("temp workspace");
    let workspace_root = temp_dir.path().join("same-file-siblings");
    let fixture_path =
        write_named_fixture(&workspace_root, "notes.txt", "alpha\nbeta\ngamma\ndelta\n");
    configure_temp_workspace(&state, &workspace_root).await;

    let app = rt.into_app_with_state_pwd(workspace_root.clone()).await;
    let cmd_tx = app.state_cmd_tx();

    tokio::time::sleep(Duration::from_millis(50)).await;

    let first_request_id = Uuid::new_v4();
    let second_request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let first_call = same_file_tool_call(
        "ns-patch-same-file-first",
        FIRST_SAME_FILE_DIFF,
        "Stage first same-file sibling patch",
    );
    let second_call = same_file_tool_call(
        "ns-patch-same-file-second",
        SECOND_SAME_FILE_DIFF,
        "Stage second same-file sibling patch",
    );

    let first_stage = stage_tool_call_via_llm_manager(
        first_request_id,
        parent_id,
        first_call.clone(),
        &mut realtime_rx,
    )
    .await;
    assert!(
        first_stage.ok,
        "first staged ns_patch should complete successfully"
    );
    assert_eq!(
        first_stage.staged, 1,
        "first request should stage exactly one edit"
    );

    let second_stage = stage_tool_call_via_llm_manager(
        second_request_id,
        parent_id,
        second_call.clone(),
        &mut realtime_rx,
    )
    .await;
    assert!(
        second_stage.ok,
        "second staged ns_patch should complete successfully"
    );
    assert_eq!(
        second_stage.staged, 1,
        "second request should stage exactly one edit"
    );

    let first_proposal_id = derive_edit_proposal_id(first_request_id, &first_call.call_id);
    let second_proposal_id = derive_edit_proposal_id(second_request_id, &second_call.call_id);

    let (first_hash, second_hash) = {
        let guard = state.proposals.read().await;
        let first = guard
            .get(&first_proposal_id)
            .expect("first staged proposal should exist");
        let second = guard
            .get(&second_proposal_id)
            .expect("second staged proposal should exist");

        assert_eq!(first.files, vec![workspace_root.join("notes.txt")]);
        assert_eq!(second.files, vec![workspace_root.join("notes.txt")]);
        assert_eq!(
            first.edits_ns.len(),
            1,
            "first proposal should store one ns edit"
        );
        assert_eq!(
            second.edits_ns.len(),
            1,
            "second proposal should store one ns edit"
        );

        (
            first.edits_ns[0].expected_file_hash,
            second.edits_ns[0].expected_file_hash,
        )
    };

    assert_eq!(
        first_hash, second_hash,
        "both staged same-file proposals are anchored to the same input file version"
    );

    cmd_tx
        .send(StateCommand::ApproveEdits {
            proposal_id: first_proposal_id,
        })
        .await
        .expect("approve first proposal");

    let first_status = wait_for_proposal_status(&state, first_proposal_id, |status| {
        matches!(
            status,
            EditProposalStatus::Applied | EditProposalStatus::Failed(_)
        )
    })
    .await;
    assert!(
        matches!(first_status, EditProposalStatus::Applied),
        "first same-file proposal should apply successfully, got {first_status:?}"
    );
    assert_eq!(
        fs::read_to_string(&fixture_path).expect("read file after first approval"),
        "alpha\nbeta-one\ngamma\ndelta\n",
        "first approval should advance the live file state"
    );

    cmd_tx
        .send(StateCommand::ApproveEdits {
            proposal_id: second_proposal_id,
        })
        .await
        .expect("approve second proposal");

    let second_status = wait_for_proposal_status(&state, second_proposal_id, |status| {
        matches!(
            status,
            EditProposalStatus::Applied | EditProposalStatus::Failed(_)
        )
    })
    .await;
    match second_status {
        EditProposalStatus::Failed(message) => {
            assert!(
                message.contains("No non-semantic edits were applied")
                    || message.contains("content")
                    || message.contains("mismatch")
                    || message.contains("expected"),
                "second approval should fail because its anchor is stale, got: {message}"
            );
        }
        other => panic!(
            "second same-file proposal should fail after the first changes the file, got {other:?}"
        ),
    }

    assert_eq!(
        fs::read_to_string(&fixture_path).expect("read file after second approval"),
        "alpha\nbeta-one\ngamma\ndelta\n",
        "second approval should not apply because it was staged against the old file version"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn ns_patch_same_file_batch_partially_applies_then_fails_due_to_shared_stale_anchor() {
    let fixture_db =
        Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));
    let rt = TestRuntime::new(&fixture_db)
        .spawn_state_manager()
        .spawn_event_bus()
        .spawn_llm_manager();

    let state = rt.state_arc();
    let events = rt.events_builder().build_event_bus_only();
    let mut realtime_rx = events.event_bus_events.realtime_tx_rx;

    let temp_dir = tempdir().expect("temp workspace");
    let workspace_root = temp_dir.path().join("same-file-batch");
    let fixture_path =
        write_named_fixture(&workspace_root, "notes.txt", "alpha\nbeta\ngamma\ndelta\n");
    configure_temp_workspace(&state, &workspace_root).await;

    let app = rt.into_app_with_state_pwd(workspace_root.clone()).await;
    let cmd_tx = app.state_cmd_tx();

    let initial_hash = FileHash::from_bytes(
        fs::read(&fixture_path)
            .expect("read initial fixture for expected hash")
            .as_slice(),
    );

    let request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let call_id = ArcStr::from("ns-patch-same-file-batch");
    let proposal = make_ns_batch_proposal(
        request_id,
        parent_id,
        call_id.clone(),
        &fixture_path,
        initial_hash,
        &[FIRST_SAME_FILE_DIFF, SECOND_SAME_FILE_DIFF],
    );
    let proposal_id = proposal.proposal_id;

    {
        let mut proposals = state.proposals.write().await;
        proposals.insert(proposal_id, proposal);
    }

    cmd_tx
        .send(StateCommand::ApproveEdits { proposal_id })
        .await
        .expect("approve same-file batch proposal");

    let terminal_status = wait_for_proposal_status(&state, proposal_id, |status| {
        matches!(
            status,
            EditProposalStatus::Applied | EditProposalStatus::Failed(_)
        )
    })
    .await;
    match terminal_status {
        EditProposalStatus::Failed(message) => {
            assert!(
                message.contains("Partially applied non-semantic edits"),
                "same-file batch should surface as partial apply failure, got: {message}"
            );
        }
        other => {
            panic!("same-file non-semantic batch should fail after partial apply, got {other:?}")
        }
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    let completed_payload = loop {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for ToolCallCompleted for same-file batch apply"
        );
        match timeout(Duration::from_millis(100), realtime_rx.recv()).await {
            Ok(Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id: event_request_id,
                call_id: event_call_id,
                content,
                ..
            }))) if event_request_id == request_id && event_call_id == call_id => {
                break serde_json::from_str::<serde_json::Value>(&content)
                    .expect("parse ToolCallCompleted payload for same-file batch");
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => {}
        }
    };

    assert_eq!(completed_payload["ok"], false);
    assert_eq!(completed_payload["applied"], 1);
    assert_eq!(completed_payload["partial"], true);
    let results = completed_payload["results"]
        .as_array()
        .expect("results array for same-file batch apply");
    assert_eq!(
        results.len(),
        2,
        "batch should report both same-file edit attempts"
    );
    assert_eq!(
        results
            .iter()
            .filter(|entry| entry.get("error").is_some())
            .count(),
        1,
        "exactly one same-file batch edit should fail after the first changes the file"
    );

    assert_eq!(
        fs::read_to_string(&fixture_path).expect("read file after same-file batch apply"),
        "alpha\nbeta-one\ngamma\ndelta\n",
        "the first same-file diff should apply, while the second fails against the stale anchor"
    );
}
