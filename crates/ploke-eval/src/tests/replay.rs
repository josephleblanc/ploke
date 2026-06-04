use std::path::Path;
#[cfg(feature = "replay_tests")]
use std::{path::PathBuf, process::Command, sync::Arc, time::Duration};

#[cfg(feature = "replay_tests")]
use ploke_tui::{
    EventBus, EventBusCaps,
    app_state::{AppState, core::derive_edit_proposal_id},
    tools::{Ctx, Tool, ns_patch::NsPatch},
    user_config::{ChatPolicy, ChatTimeoutStrategy},
};
#[cfg(feature = "replay_tests")]
use tempfile::tempdir;
#[cfg(feature = "replay_tests")]
use tracing_subscriber::fmt::SubscriberBuilder;
#[cfg(feature = "replay_tests")]
use uuid::Uuid;

use crate::replay::llm::LoadedResponseTape;
#[cfg(feature = "replay_tests")]
use crate::{
    PreparedSingleRun,
    runner::{AgentTurnArtifact, ObservedTurnEvent, ToolRequestRecord, setup_replay_runtime},
};

#[cfg(feature = "replay_tests")]
fn benchmark_chat_policy() -> ChatPolicy {
    let policy = ChatPolicy {
        tool_call_timeout_secs: 60,
        timeout_strategy: ChatTimeoutStrategy::Backoff { attempts: Some(3) },
        timeout_base_secs: 5,
        error_retry_limit: 3,
        ..Default::default()
    };
    policy.validated()
}

#[cfg(feature = "replay_tests")]
fn init_tracing() {
    let _ = SubscriberBuilder::default()
        .with_max_level(tracing::Level::INFO)
        .with_target(true)
        .with_test_writer()
        .try_init();
}

#[cfg(feature = "replay_tests")]
fn load_prepared_single_run(path: &Path) -> PreparedSingleRun {
    let text = std::fs::read_to_string(path).expect("read historical run manifest");
    serde_json::from_str(&text).expect("historical run manifest must parse")
}

#[cfg(feature = "replay_tests")]
fn load_agent_turn_artifact(path: &Path) -> AgentTurnArtifact {
    let text = std::fs::read_to_string(path).expect("read historical agent turn artifact");
    serde_json::from_str(&text).expect("historical agent turn artifact must parse")
}

#[cfg(feature = "replay_tests")]
fn manual_replay_instances_root() -> Option<PathBuf> {
    std::env::var_os("PLOKE_EVAL_MANUAL_INSTANCES_ROOT").map(PathBuf::from)
}

#[cfg(feature = "replay_tests")]
fn historical_run_dir_with_tool_calls(instance_id: &str, call_ids: &[&str]) -> Option<PathBuf> {
    let instance_root = manual_replay_instances_root()?.join(instance_id);
    let mut candidates = vec![instance_root.clone()];
    let runs_dir = instance_root.join("runs");
    if let Ok(entries) = std::fs::read_dir(&runs_dir) {
        candidates.extend(
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir()),
        );
    }
    candidates.sort();

    candidates.into_iter().find(|candidate| {
        let trace_path = candidate.join("agent-turn-trace.json");
        if !trace_path.exists() {
            return false;
        }
        let artifact = load_agent_turn_artifact(&trace_path);
        call_ids.iter().all(|call_id| {
            artifact.events.iter().any(|event| {
                matches!(
                    event,
                    ObservedTurnEvent::ToolRequested(record) if record.call_id == *call_id
                )
            })
        })
    })
}

#[cfg(feature = "replay_tests")]
fn find_tool_request(artifact: &AgentTurnArtifact, call_id: &str) -> ToolRequestRecord {
    artifact
        .events
        .iter()
        .find_map(|event| match event {
            ObservedTurnEvent::ToolRequested(record) if record.call_id == call_id => {
                Some(record.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing ToolRequested record for call_id {call_id}"))
}

#[test]
fn fixture_backed_response_sidecar_rejects_gaps_for_replay_but_allows_inspection() {
    const ASSISTANT_MESSAGE_ID: &str = "9a1a7000-dc4e-4c50-b00f-cb2fdc60f77d";
    const REPAIR_CALL_ID: &str = "chatcmpl-tool-981ea94a5aab5a0d";

    let run_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/tests/fixtures/response-sidecar-missing-malformed-args");
    assert!(
        run_dir.join("llm-full-responses.jsonl").exists(),
        "expected source-controlled response sidecar fixture under {}",
        run_dir.display()
    );

    let strict_error = LoadedResponseTape::load(&run_dir, ASSISTANT_MESSAGE_ID)
        .expect_err("default replay admission should reject this incomplete sidecar");
    let strict_message = strict_error.to_string();
    assert!(
        strict_message.contains("missing response_index values: 1"),
        "strict replay admission should name the missing malformed response, got {strict_message}"
    );
    assert!(
        strict_message.contains("load_for_inspection"),
        "strict replay admission should keep the forensic-inspection hint, got {strict_message}"
    );

    let loaded = LoadedResponseTape::load_for_inspection(&run_dir, ASSISTANT_MESSAGE_ID)
        .expect("inspection load should allow incomplete historical sidecar fixture");
    let response_indexes = loaded
        .records()
        .iter()
        .map(|record| record.response_index().get())
        .collect::<Vec<_>>();
    assert_eq!(
        response_indexes,
        vec![0, 2],
        "fixture should surround the intentionally missing malformed response"
    );

    let missing = loaded
        .missing_response_indices()
        .into_iter()
        .map(|index| index.get())
        .collect::<Vec<_>>();
    assert_eq!(
        missing,
        vec![1],
        "inspection should expose the missing malformed-provider repair response"
    );

    let repaired_call_present = loaded.records().iter().any(|record| {
        serde_json::to_string(record.response())
            .expect("fixture response should serialize")
            .contains(REPAIR_CALL_ID)
    });
    assert!(
        repaired_call_present,
        "fixture should preserve the repaired request_code_context tool call adjacent to the gap"
    );
}

#[cfg(feature = "replay_tests")]
fn run_git(repo_root: &Path, args: &[&str], label: &str) {
    let status = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .status()
        .unwrap_or_else(|err| panic!("{label}: failed to spawn git: {err}"));
    assert!(
        status.success(),
        "{label}: git exited with status {:?}",
        status.code()
    );
}

#[cfg(feature = "replay_tests")]
fn git_stdout(repo_root: &Path, args: &[&str], label: &str) -> String {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("{label}: failed to spawn git: {err}"));
    assert!(
        output.status.success(),
        "{label}: git exited with status {:?}",
        output.status.code()
    );
    String::from_utf8(output.stdout).expect("git stdout should be utf-8")
}

#[cfg(feature = "replay_tests")]
fn clone_repo_for_replay(source_repo: &Path, dest_repo: &Path) {
    let source = source_repo
        .to_str()
        .expect("historical source repo path should be utf-8");
    let dest = dest_repo
        .to_str()
        .expect("replay destination repo path should be utf-8");
    let status = Command::new("git")
        .args(["clone", "--quiet", "--no-local", source, dest])
        .status()
        .expect("spawn git clone for replay");
    assert!(
        status.success(),
        "git clone for replay failed with status {:?}",
        status.code()
    );
}

#[cfg(feature = "replay_tests")]
async fn replay_ns_patch_request(
    state: Arc<AppState>,
    event_bus: Arc<EventBus>,
    request: &ToolRequestRecord,
) -> Result<(), ploke_error::Error> {
    let request_id = Uuid::parse_str(&request.request_id).expect("request_id should be a uuid");
    let parent_id = Uuid::parse_str(&request.parent_id).expect("parent_id should be a uuid");
    let ctx = Ctx {
        state,
        event_bus,
        request_id,
        parent_id,
        call_id: ploke_core::ArcStr::from(request.call_id.clone()),
    };

    let params = NsPatch::deserialize_params(request.arguments.as_str())
        .expect("historical non_semantic_patch payload should deserialize");
    let ploke_tui::tools::ToolResult {
        content,
        ui_payload,
    } = NsPatch::execute(params, ctx.clone()).await?;
    NsPatch::emit_completed(&ctx, content, ui_payload);
    Ok(())
}

#[cfg(feature = "replay_tests")]
async fn wait_for_terminal_proposal_status(
    state: &Arc<AppState>,
    proposal_id: Uuid,
) -> ploke_tui::app_state::core::EditProposalStatus {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(status) = {
                let proposals = state.proposals.read().await;
                proposals
                    .get(&proposal_id)
                    .map(|proposal| proposal.status.clone())
            } {
                match status {
                    ploke_tui::app_state::core::EditProposalStatus::Pending
                    | ploke_tui::app_state::core::EditProposalStatus::Approved => {}
                    other => return other,
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("proposal should reach a terminal status within timeout")
}

#[tokio::test(flavor = "multi_thread")]
// NOTE: we don't want to ignore these anymore, that is a bad pattern. they end up just getting forgotten.
// better is to use this pattern.
#[cfg(feature = "replay_tests")]
async fn test_replay_historical_fd_1121_partial_non_semantic_patch_runtime_flow() {
    init_tracing();
    const INSTANCE_ID: &str = "sharkdp__fd-1121";
    const JOB_CALL_ID: &str = "call_86042515";
    const WALK_CALL_ID: &str = "call_80363220";

    let Some(run_dir) =
        historical_run_dir_with_tool_calls(INSTANCE_ID, &[JOB_CALL_ID, WALK_CALL_ID])
    else {
        eprintln!(
            "manual replay skipped: set PLOKE_EVAL_MANUAL_INSTANCES_ROOT to an eval instances root containing {INSTANCE_ID}"
        );
        return;
    };
    let run_manifest = run_dir
        .ancestors()
        .find_map(|candidate| {
            let manifest = candidate.join("run.json");
            manifest.exists().then_some(manifest)
        })
        .expect("manual replay run directory should be under an instance or run directory with run.json");
    let turn_trace = run_dir.join("agent-turn-trace.json");
    assert!(
        turn_trace.exists(),
        "expected historical turn trace at {}",
        turn_trace.display()
    );

    // TODO: a lot of the following could be turned into either another helper or a test macro
    let historical = load_prepared_single_run(&run_manifest);
    let trace = load_agent_turn_artifact(&turn_trace);
    let job_request = find_tool_request(&trace, JOB_CALL_ID);
    let walk_request = find_tool_request(&trace, WALK_CALL_ID);

    let temp = tempdir().expect("tempdir");
    let replay_repo_root = temp.path().join("fd-replay");
    let replay_output_dir = temp.path().join("replay-output");
    clone_repo_for_replay(&historical.repo_root, &replay_repo_root);

    let mut prepared = historical.clone();
    prepared.repo_root = replay_repo_root.clone();
    prepared.output_dir = replay_output_dir.clone();

    run_git(
        &prepared.repo_root,
        &["reset", "--hard"],
        "git reset --hard",
    );
    if let Some(base_sha) = prepared.base_sha.as_deref() {
        run_git(
            &prepared.repo_root,
            &["checkout", "--detach", base_sha],
            "git checkout --detach base sha",
        );
    }

    let (_app, state, _config_guard) = setup_replay_runtime(&prepared)
        .await
        .expect("setup replay runtime for fd-1121");
    {
        let mut cfg = state.config.write().await;
        cfg.editing.auto_confirm_edits = true;
        cfg.chat_policy = benchmark_chat_policy();
    }
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    // NOTE: this is fine so far, but you're really under-utilizing the test
    // harness. if we really want to be testing something meaningful here this
    // can all be going through the test harness, or maybe tui_adapter functions
    // or something, I'd need to look at tui_adapter.rs more to say.
    replay_ns_patch_request(Arc::clone(&state), Arc::clone(&event_bus), &job_request)
        .await
        .expect("historical job non_semantic_patch replay should execute");
    let job_request_id = Uuid::parse_str(&job_request.request_id).expect("job request id uuid");
    let job_call_id: ploke_core::ArcStr = job_request.call_id.clone().into();
    let job_proposal_id = derive_edit_proposal_id(job_request_id, &job_call_id);
    let job_status = wait_for_terminal_proposal_status(&state, job_proposal_id).await;
    assert_eq!(
        job_status,
        ploke_tui::app_state::core::EditProposalStatus::Applied
    );

    let walk_request_id = Uuid::parse_str(&walk_request.request_id).expect("walk request id uuid");
    let walk_call_id: ploke_core::ArcStr = walk_request.call_id.clone().into();
    let walk_proposal_id = derive_edit_proposal_id(walk_request_id, &walk_call_id);

    // same here, should go through test harness
    let walk_err =
        replay_ns_patch_request(Arc::clone(&state), Arc::clone(&event_bus), &walk_request)
            .await
            .expect_err("historical walk non_semantic_patch replay should fail before staging");
    let walk_err_text = walk_err.to_string();
    assert!(
        walk_err_text.contains("Patch applied partially"),
        "historical walk replay should fail with partial-apply error, got: {walk_err_text}"
    );
    let walk_proposal = {
        let proposals = state.proposals.read().await;
        proposals.get(&walk_proposal_id).cloned()
    };
    assert!(
        walk_proposal.is_none(),
        "historical walk replay should not stage a proposal after strict rejection"
    );

    let diff = git_stdout(
        &prepared.repo_root,
        &["diff", "--no-ext-diff"],
        "git diff after replay",
    );
    assert!(
        diff.contains("diff --git a/src/exec/job.rs b/src/exec/job.rs"),
        "replayed repo diff should include src/exec/job.rs:\n{diff}"
    );
    assert!(
        !diff.contains("diff --git a/src/walk.rs b/src/walk.rs"),
        "replayed repo diff should exclude src/walk.rs after failed apply:\n{diff}"
    );
}
