use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
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
use crate::rag::editing::{
    rescan_for_changes_calls_for_test, reset_rescan_for_changes_calls_for_test,
};
use crate::utils::path_scoping::WriteScope;
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

const STALE_SAME_FILE_REPAIR_DIFF: &str = r#"--- a/notes.txt
+++ b/notes.txt
@@ -1,4 +1,4 @@
 alpha
-beta
+beta-repair
 gamma
 delta
"#;

const BARE_HUNK_DIFF: &str = r#"@@ -1,4 +1,4 @@
 alpha
-beta
+beta-one
 gamma
 delta
"#;

fn ns_patch_event_test_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

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
    ns_patch_tool_call(call_id, "notes.txt", diff, reasoning)
}

fn ns_patch_tool_call(call_id: &str, file: &str, diff: &str, reasoning: &str) -> ToolCall {
    ToolCall {
        call_id: ArcStr::from(call_id),
        call_type: FunctionMarker,
        function: FunctionCall {
            name: ToolName::NsPatch,
            arguments: serde_json::json!({
                "patches": [{
                    "file": file,
                    "diff": diff,
                    "reasoning": reasoning,
                }]
            })
            .to_string(),
        },
        extra_content: None,
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

async fn stage_tool_call_failure_via_llm_manager(
    request_id: Uuid,
    parent_id: Uuid,
    tool_call: ToolCall,
    realtime_rx: &mut tokio::sync::broadcast::Receiver<AppEvent>,
) -> String {
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
            Ok(Ok(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id: event_request_id,
                call_id: event_call_id,
                error,
                ..
            }))) if event_request_id == request_id && event_call_id == expected_call_id => {
                return error;
            }
            Ok(Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id: event_request_id,
                call_id: event_call_id,
                content,
                ..
            }))) if event_request_id == request_id && event_call_id == expected_call_id => {
                panic!("ns_patch unexpectedly completed while staging: {content}");
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => {}
        }
    }

    panic!("timed out waiting for ToolCallFailed while staging ns_patch");
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

// regr:samefiletui:19-05-26_15-43
#[tokio::test(flavor = "multi_thread")]
async fn ns_patch_rejects_fuzzy_same_file_repair_after_applied_proposal_before_staging() {
    let _guard = ns_patch_event_test_lock().lock().await;
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
    let workspace_root = temp_dir.path().join("same-file-fuzzy-repair");
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
        "ns-patch-fuzzy-repair-first",
        FIRST_SAME_FILE_DIFF,
        "Stage first same-file patch",
    );
    let stale_call = same_file_tool_call(
        "ns-patch-fuzzy-repair-stale",
        STALE_SAME_FILE_REPAIR_DIFF,
        "Attempt stale same-file repair",
    );

    let first_stage = stage_tool_call_via_llm_manager(
        first_request_id,
        parent_id,
        first_call.clone(),
        &mut realtime_rx,
    )
    .await;
    assert!(first_stage.ok, "first ns_patch should stage successfully");

    let first_proposal_id = derive_edit_proposal_id(first_request_id, &first_call.call_id);
    cmd_tx
        .send(StateCommand::ApproveEdits {
            proposal_id: first_proposal_id,
        })
        .await
        .expect("approve first same-file proposal");

    let first_status = wait_for_proposal_status(&state, first_proposal_id, |status| {
        matches!(status, EditProposalStatus::Applied)
    })
    .await;
    assert!(
        matches!(first_status, EditProposalStatus::Applied),
        "first proposal should apply before stale repair, got {first_status:?}"
    );

    let error = stage_tool_call_failure_via_llm_manager(
        second_request_id,
        parent_id,
        stale_call.clone(),
        &mut realtime_rx,
    )
    .await;
    let wire = crate::tools::ToolErrorWire::parse(&error)
        .expect("stale same-file repair should use structured tool error wire");
    assert_eq!(wire.llm.code, crate::tools::ToolErrorCode::Io);
    assert!(
        wire.llm.message.contains("failed to patch")
            && wire.llm.message.contains("matched only fuzzily"),
        "failure should explain stale fuzzy same-file rejection, got: {}",
        wire.llm.message
    );

    let stale_proposal_id = derive_edit_proposal_id(second_request_id, &stale_call.call_id);
    let proposals = state.proposals.read().await;
    assert!(
        proposals.contains_key(&first_proposal_id),
        "first proposal should remain recorded"
    );
    assert!(
        !proposals.contains_key(&stale_proposal_id),
        "stale fuzzy same-file repair must be rejected before staging"
    );
    assert_eq!(
        proposals.len(),
        1,
        "only the first applied proposal should remain recorded"
    );
    drop(proposals);

    assert_eq!(
        fs::read_to_string(&fixture_path).expect("read file after stale repair rejection"),
        "alpha\nbeta-one\ngamma\ndelta\n",
        "stale repair must not rewrite the file after the first applied patch"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn ns_patch_malformed_diff_emits_one_failure_and_stages_zero_proposals() {
    let fixture_db =
        Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));
    let rt = TestRuntime::new(&fixture_db);

    let state = rt.state_arc();
    let event_bus = Arc::new(crate::EventBus::new(crate::EventBusCaps::default()));
    let mut realtime_rx = event_bus.subscribe(EventPriority::Realtime);

    let request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let call = ns_patch_tool_call(
        "ns-patch-bare-hunk",
        "notes.txt",
        BARE_HUNK_DIFF,
        "Attempt a malformed bare-hunk diff",
    );
    let expected_call_id = call.call_id.clone();

    let ctx = crate::tools::Ctx {
        state: Arc::clone(&state),
        event_bus: Arc::clone(&event_bus),
        request_id,
        parent_id,
        call_id: expected_call_id.clone(),
    };

    let _dispatcher_err = crate::tools::process_tool(call, ctx)
        .await
        .expect_err("malformed ns_patch should fail in dispatcher validation");
    tokio::time::sleep(Duration::from_millis(20)).await;

    let mut failure_count = 0usize;
    let mut completed_count = 0usize;
    let mut first_error = None;
    while let Ok(event) = realtime_rx.try_recv() {
        match event {
            AppEvent::System(SystemEvent::ToolCallFailed {
                request_id: event_request_id,
                call_id,
                error,
                ..
            }) if event_request_id == request_id && call_id == expected_call_id => {
                failure_count += 1;
                first_error.get_or_insert(error);
            }
            AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id: event_request_id,
                call_id,
                ..
            }) if event_request_id == request_id && call_id == expected_call_id => {
                completed_count += 1;
            }
            _ => {}
        }
    }

    assert_eq!(failure_count, 1, "malformed diff should emit one failure");
    assert_eq!(
        completed_count, 0,
        "malformed diff must not emit a completed event"
    );
    let wire = crate::tools::ToolErrorWire::parse(first_error.as_deref().expect("failure error"))
        .expect("ToolCallFailed should carry structured error wire");
    assert_eq!(
        wire.llm.code,
        crate::tools::ToolErrorCode::MalformedDiff,
        "malformed diff should remain a validation error"
    );
    assert!(
        wire.llm.retry_hint.is_some(),
        "retry hint should be present"
    );
    assert_eq!(
        wire.llm["retry_context"]
            .as_object()
            .and_then(|ctx| ctx.get("patch_index"))
            .and_then(|value| value.as_u64()),
        Some(0)
    );

    let proposals = state.proposals.read().await;
    assert!(
        proposals.is_empty(),
        "malformed diff must be rejected before staging"
    );
}

// regr:protectedpreflight:19-05-26_15-43
#[tokio::test(flavor = "multi_thread")]
async fn ns_patch_protected_path_preflight_rejects_repeat_before_staging() {
    let _guard = ns_patch_event_test_lock().lock().await;
    let fixture_db =
        Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));
    let rt = TestRuntime::new(&fixture_db);
    let state = rt.state_arc();
    let tmp = tempdir().expect("temp workspace");
    configure_temp_workspace(&state, tmp.path()).await;
    state
        .with_system_txn(|txn| {
            txn.set_write_scope(Some(
                WriteScope::new()
                    .deny_filenames(["Cargo.toml".to_string()])
                    .deny_prefixes([PathBuf::from(".cargo")]),
            ));
        })
        .await;

    let event_bus = Arc::new(crate::EventBus::new(crate::EventBusCaps::default()));
    let mut realtime_rx = event_bus.subscribe(EventPriority::Realtime);
    let request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let diff = r#"--- a/Cargo.toml
+++ b/Cargo.toml
@@ -1,2 +1,2 @@
-[package]
+[workspace]
 name = "fixture"
"#;

    for call_id in ["protected-manifest-1", "protected-manifest-2"] {
        let call = ns_patch_tool_call(call_id, "Cargo.toml", diff, "Try a manifest edit");
        let ctx = crate::tools::Ctx {
            state: Arc::clone(&state),
            event_bus: Arc::clone(&event_bus),
            request_id,
            parent_id,
            call_id: call.call_id.clone(),
        };
        let _ = crate::tools::process_tool(call, ctx)
            .await
            .expect_err("protected path preflight should reject");
    }
    tokio::time::sleep(Duration::from_millis(20)).await;

    let mut failures = Vec::new();
    let mut completed_count = 0usize;
    while let Ok(event) = realtime_rx.try_recv() {
        match event {
            AppEvent::System(SystemEvent::ToolCallFailed {
                request_id: event_request_id,
                error,
                ..
            }) if event_request_id == request_id => failures.push(error),
            AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id: event_request_id,
                ..
            }) if event_request_id == request_id => completed_count += 1,
            _ => {}
        }
    }

    assert_eq!(failures.len(), 2, "both attempts should fail preflight");
    assert_eq!(
        completed_count, 0,
        "protected preflight must not emit completed events"
    );
    assert!(
        state.proposals.read().await.is_empty(),
        "protected preflight must not stage proposals"
    );

    let first = crate::tools::ToolErrorWire::parse(&failures[0])
        .expect("first protected failure should be structured");
    let second = crate::tools::ToolErrorWire::parse(&failures[1])
        .expect("second protected failure should be structured");
    assert_eq!(first.llm.code, crate::tools::ToolErrorCode::InvalidFormat);
    assert!(
        first
            .llm
            .retry_hint
            .as_deref()
            .is_some_and(|hint| hint.contains("protected manifests/configs")),
        "first failure should include the full policy guidance"
    );
    assert_eq!(
        first.llm["retry_context"]
            .as_object()
            .and_then(|ctx| ctx.get("repeated"))
            .is_some_and(|value| matches!(value, crate::tools::ToolLlmErrorValue::Bool(false))),
        true
    );
    assert_eq!(
        second.llm["retry_context"]
            .as_object()
            .and_then(|ctx| ctx.get("repeated"))
            .is_some_and(|value| matches!(value, crate::tools::ToolLlmErrorValue::Bool(true))),
        true,
        "second identical protected write should be marked as a repeat"
    );
    assert!(
        second
            .llm
            .retry_hint
            .as_deref()
            .is_some_and(|hint| hint.contains("already denied")),
        "repeat failure should tell the model not to retry the same target"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn ns_patch_approval_triggers_rescan_helper() {
    let _guard = ns_patch_event_test_lock().lock().await;
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
    let workspace_root = temp_dir.path().join("rescan-after-ns-patch");
    let fixture_path =
        write_named_fixture(&workspace_root, "notes.txt", "alpha\nbeta\ngamma\ndelta\n");
    configure_temp_workspace(&state, &workspace_root).await;

    let app = rt.into_app_with_state_pwd(workspace_root.clone()).await;
    let cmd_tx = app.state_cmd_tx();

    tokio::time::sleep(Duration::from_millis(50)).await;

    reset_rescan_for_changes_calls_for_test();

    let request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let call = ns_patch_tool_call(
        "ns-patch-triggers-rescan",
        "notes.txt",
        FIRST_SAME_FILE_DIFF,
        "Apply one non-semantic edit and rescan",
    );

    let staged =
        stage_tool_call_via_llm_manager(request_id, parent_id, call.clone(), &mut realtime_rx)
            .await;
    assert!(staged.ok, "staged ns_patch should complete successfully");
    assert_eq!(staged.staged, 1, "request should stage exactly one ns edit");

    let proposal_id = derive_edit_proposal_id(request_id, &call.call_id);
    cmd_tx
        .send(StateCommand::ApproveEdits { proposal_id })
        .await
        .expect("approve proposal");

    let terminal_status = wait_for_proposal_status(&state, proposal_id, |status| {
        matches!(
            status,
            EditProposalStatus::Applied | EditProposalStatus::Failed(_)
        )
    })
    .await;
    assert!(
        matches!(terminal_status, EditProposalStatus::Applied),
        "ns_patch approval should apply successfully, got {terminal_status:?}"
    );
    assert_eq!(
        fs::read_to_string(&fixture_path).expect("read file after approval"),
        "alpha\nbeta-one\ngamma\ndelta\n",
        "ns_patch approval should advance the live file state"
    );

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if rescan_for_changes_calls_for_test() > 0 {
            break;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for ns_patch approval to trigger rescan helper");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ns_patch_same_file_staged_siblings_fail_second_approval_due_to_stale_anchor() {
    let _guard = ns_patch_event_test_lock().lock().await;
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
    let _guard = ns_patch_event_test_lock().lock().await;
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
