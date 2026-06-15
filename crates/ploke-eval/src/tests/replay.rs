use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

#[cfg(feature = "replay_tests")]
use ploke_tui::tools::insert_rust_item::InsertRustItem;
#[cfg(feature = "replay_tests")]
use std::process::Command;

use ploke_db::{Database, NodeType};
use ploke_records::{
    agent_turn::{
        AgentTurnTraceRecord, ObservedTurnEventRecord,
        ToolRequestRecord as PersistedToolRequestRecord,
    },
    tool_contracts::{PersistedToolCallArguments, ToolCallArguments},
};
use ploke_test_utils::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};
use ploke_tui::{
    AppEvent, EventBus, EventBusCaps, EventPriority,
    app::commands::harness::TestRuntime,
    app_state::{AppState, events::SystemEvent},
    parser::{IndexTargetKind, resolve_index_target},
    rag::{
        tools::apply_code_edit_tool,
        utils::{ApplyCodeEditRequest, Edit, ToolCallParams},
    },
    tools::{
        Ctx, Tool, ToolErrorCode, ToolErrorWire, ToolName,
        ns_read::{NsRead, NsReadResult},
    },
    user_config::{ChatPolicy, ChatTimeoutStrategy},
};
use serde::Deserialize;
#[cfg(feature = "replay_tests")]
use tempfile::tempdir;
use tracing_subscriber::fmt::SubscriberBuilder;
use uuid::Uuid;

use crate::{
    PreparedSingleRun,
    replay::llm::LoadedResponseTape,
    runner::{
        RepoStateArtifact, checkout_repo_to_base, init_runtime_db, prepare_sparse_workspace,
        sparse_headless_embedding_processor,
    },
};

#[cfg(feature = "replay_tests")]
use crate::runner::{
    AgentTurnArtifact, ObservedTurnEvent, ToolRequestRecord, setup_replay_runtime,
};

#[cfg(feature = "replay_tests")]
use ploke_tui::{app_state::core::derive_edit_proposal_id, tools::ns_patch::NsPatch};

const READ_FIX_TRACE: &str = "/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r2.headless-tui.json";
const READ_FIX_WORKSPACE: &str = "/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/workspaces/edit-harness/node-552c19a55f53dbe6-r2";
const READ_FIX_EVENT_INDEX: usize = 80;
const READ_FIX_CALL_ID: &str = "function-call-cd7bb6c0-97b8-4928-9324-57aa0c08b4e5";
const READ_FIX_TARGET: &str = "real_tui_resolver_touch_is_checked_before_adapter_apply";

#[derive(Debug, Deserialize)]
struct RecordedApplyCodeEditToolRequest {
    request_id: Uuid,
    parent_id: Uuid,
    call_id: String,
    tool: String,
    arguments: RecordedApplyCodeEditArguments,
}

#[derive(Debug, Deserialize)]
struct RecordedApplyCodeEditArguments {
    edits: Vec<RecordedCanonicalEdit>,
    #[serde(default)]
    confidence: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct RecordedCanonicalEdit {
    file: String,
    canon: String,
    node_type: NodeType,
    code: String,
}

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

fn init_tracing() {
    let _ = SubscriberBuilder::default()
        .with_max_level(tracing::Level::INFO)
        .with_target(true)
        .with_test_writer()
        .try_init();
}

impl RecordedApplyCodeEditToolRequest {
    fn into_tool_params(self, state: Arc<AppState>, event_bus: Arc<EventBus>) -> ToolCallParams {
        let typed_req = ApplyCodeEditRequest {
            confidence: self.arguments.confidence,
            edits: self
                .arguments
                .edits
                .into_iter()
                .map(|edit| Edit::Canonical {
                    file: edit.file,
                    canon: edit.canon,
                    node_type: edit.node_type,
                    code: edit.code,
                })
                .collect(),
        };

        ToolCallParams {
            state,
            event_bus,
            request_id: self.request_id,
            parent_id: self.parent_id,
            name: ToolName::ApplyCodeEdit,
            typed_req,
            call_id: ploke_core::ArcStr::from(self.call_id),
        }
    }
}

fn load_recorded_apply_code_edit_request() -> RecordedApplyCodeEditToolRequest {
    serde_json::from_str(include_str!(
        "fixtures/BurntSushi__ripgrep-2209_apply_code_edit.json"
    ))
    .expect("recorded apply_code_edit tool request fixture must be valid json")
}

fn load_prepared_single_run(path: &Path) -> PreparedSingleRun {
    let text = std::fs::read_to_string(path).expect("read historical run manifest");
    serde_json::from_str(&text).expect("historical run manifest must parse")
}

#[cfg(feature = "replay_tests")]
fn load_agent_turn_artifact(path: &Path) -> AgentTurnArtifact {
    let text = std::fs::read_to_string(path).expect("read historical agent turn artifact");
    serde_json::from_str(&text).expect("historical agent turn artifact must parse")
}

fn load_agent_turn_trace_record(path: &Path) -> AgentTurnTraceRecord {
    let text = std::fs::read_to_string(path).expect("read historical agent turn trace");
    serde_json::from_str(&text).expect("historical agent turn trace must parse")
}

fn historical_instance_root(instance_id: &str) -> PathBuf {
    PathBuf::from("/home/brasides/.ploke-eval/instances").join(instance_id)
}

fn historical_run_dir_with(instance_id: &str, required_artifacts: &[&str]) -> PathBuf {
    let instance_root = historical_instance_root(instance_id);
    if required_artifacts
        .iter()
        .all(|artifact| instance_root.join(artifact).exists())
    {
        return instance_root;
    }

    let runs_dir = instance_root.join("runs");
    let mut candidates = std::fs::read_dir(&runs_dir)
        .unwrap_or_else(|err| panic!("read historical runs dir {}: {err}", runs_dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| {
            required_artifacts
                .iter()
                .all(|artifact| path.join(artifact).exists())
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.pop().unwrap_or_else(|| {
        panic!(
            "expected historical artifacts {:?} under {} or its runs/* directories",
            required_artifacts,
            instance_root.display()
        )
    })
}

#[cfg(feature = "replay_tests")]
fn historical_run_dir_with_tool_calls(instance_id: &str, call_ids: &[&str]) -> PathBuf {
    let instance_root = historical_instance_root(instance_id);
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

    let mut checked = Vec::new();
    for candidate in candidates {
        let trace_path = candidate.join("agent-turn-trace.json");
        if !trace_path.exists() {
            continue;
        }
        checked.push(trace_path.clone());
        let artifact = load_agent_turn_artifact(&trace_path);
        let has_all_calls = call_ids.iter().all(|call_id| {
            artifact.events.iter().any(|event| {
                matches!(
                    event,
                    ObservedTurnEvent::ToolRequested(record) if record.call_id == *call_id
                )
            })
        });
        if has_all_calls {
            return candidate;
        }
    }

    let checked = checked
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    panic!(
        "expected historical trace for {instance_id} with call ids {call_ids:?}; checked {checked}"
    );
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
#[ignore = "real-run replay completeness check for p1-broad-batch-admission-20260518-1"]
fn test_real_run_full_response_sidecar_exposes_missing_malformed_tool_arg_response() {
    const RUN_DIR: &str = "/home/brasides/.ploke-eval/instances/prototype1/p1-broad-batch-admission-20260518-1/BurntSushi__ripgrep-2209/runs/run-1779088559136-structured-current-policy-6d8a8756";
    const ASSISTANT_MESSAGE_ID: &str = "9a1a7000-dc4e-4c50-b00f-cb2fdc60f77d";
    const REPAIR_CALL_ID: &str = "chatcmpl-tool-981ea94a5aab5a0d";

    let run_dir = Path::new(RUN_DIR);
    assert!(run_dir.exists(), "expected real run dir at {RUN_DIR}");
    let repo_state_path = run_dir.join("repo-state.json");
    let checkpoint_db_path = run_dir.join("indexing-checkpoint.db");
    let trace_path = run_dir.join("agent-turn-trace.json");
    assert!(
        checkpoint_db_path.exists(),
        "expected starting DB snapshot at {}",
        checkpoint_db_path.display()
    );

    let repo_state: RepoStateArtifact = serde_json::from_str(
        &std::fs::read_to_string(&repo_state_path).expect("read repo-state.json"),
    )
    .expect("repo-state.json must parse");
    assert!(
        repo_state.repo_root.exists(),
        "expected recorded repo root to exist at {}",
        repo_state.repo_root.display()
    );

    let trace = load_agent_turn_trace_record(&trace_path);
    let repair_debug = trace.0.events.iter().any(|event| match event {
        ObservedTurnEventRecord::DebugCommand(message) => {
            message.contains("Provider emitted invalid arguments")
                && message.contains("request_code_context")
                && message.contains("WrongType")
        }
        _ => false,
    });
    assert!(
        repair_debug,
        "agent-turn trace should contain the malformed request_code_context repair message"
    );

    let corrected_tool_requested = trace.0.events.iter().any(|event| match event {
        ObservedTurnEventRecord::ToolRequested(request) => {
            request.call_id == REPAIR_CALL_ID && request.tool == "request_code_context"
        }
        _ => false,
    });
    assert!(
        corrected_tool_requested,
        "agent-turn trace should contain the corrected request_code_context call"
    );

    let strict_error = LoadedResponseTape::load(run_dir, ASSISTANT_MESSAGE_ID)
        .expect_err("default replay admission should reject this incomplete sidecar");
    let strict_message = strict_error.to_string();
    assert!(
        strict_message.contains("missing response_index values: 34"),
        "strict replay admission should name the missing malformed response, got {strict_message}"
    );

    let loaded = LoadedResponseTape::load_for_inspection(run_dir, ASSISTANT_MESSAGE_ID)
        .expect("inspection load should allow incomplete historical sidecar");
    let response_indexes = loaded
        .records()
        .iter()
        .map(|record| record.response_index().get())
        .collect::<Vec<_>>();
    assert!(
        response_indexes.contains(&33) && response_indexes.contains(&35),
        "expected sidecar to surround the missing malformed response, got {response_indexes:?}"
    );
    let missing = loaded
        .missing_response_indices()
        .into_iter()
        .map(|index| index.get())
        .collect::<Vec<_>>();
    assert_eq!(
        missing,
        vec![34],
        "current sidecar cannot faithfully replay the malformed-provider repair turn"
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
fn request_ids(request: &ToolRequestRecord) -> (Uuid, Uuid, ploke_core::ArcStr) {
    let request_id = Uuid::parse_str(&request.request_id).expect("request_id should be a uuid");
    let parent_id = Uuid::parse_str(&request.parent_id).expect("parent_id should be a uuid");
    let call_id = ploke_core::ArcStr::from(request.call_id.clone());
    (request_id, parent_id, call_id)
}

#[cfg(feature = "replay_tests")]
fn edit_params(
    state: Arc<AppState>,
    event_bus: Arc<EventBus>,
    request: &ToolRequestRecord,
) -> ToolCallParams {
    assert_eq!(request.tool, "apply_code_edit");
    let arguments: RecordedApplyCodeEditArguments =
        serde_json::from_str(request.arguments.as_str())
            .expect("historical apply_code_edit payload should deserialize");
    let (request_id, parent_id, call_id) = request_ids(request);
    ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        name: ToolName::ApplyCodeEdit,
        typed_req: ApplyCodeEditRequest {
            confidence: arguments.confidence,
            edits: arguments
                .edits
                .into_iter()
                .map(|edit| Edit::Canonical {
                    file: edit.file,
                    canon: edit.canon,
                    node_type: edit.node_type,
                    code: edit.code,
                })
                .collect(),
        },
        call_id,
    }
}

#[cfg(feature = "replay_tests")]
async fn replay_apply(
    state: Arc<AppState>,
    event_bus: Arc<EventBus>,
    request: &ToolRequestRecord,
) -> Option<Uuid> {
    let params = edit_params(state, event_bus, request);
    apply_code_edit_tool(params).await
}

#[cfg(feature = "replay_tests")]
async fn wait_tool_applied(
    event_rx: &mut tokio::sync::broadcast::Receiver<AppEvent>,
    request: &ToolRequestRecord,
    context: &str,
) -> Option<ploke_core::TrackingHash> {
    let (request_id, _, call_id) = request_ids(request);
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let event = match event_rx.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(err) => panic!("{context}: event bus failed while waiting for apply: {err}"),
            };
            match event {
                AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id: observed_id,
                    call_id: observed_call,
                    content,
                    ..
                }) if observed_id == request_id && observed_call == call_id => {
                    let value = serde_json::from_str::<serde_json::Value>(&content).ok();
                    let applied = value
                        .as_ref()
                        .and_then(|value| value.get("applied").and_then(|count| count.as_u64()))
                        .unwrap_or(0);
                    if applied > 0 {
                        return value
                            .as_ref()
                            .and_then(|value| value.get("results"))
                            .and_then(|results| results.as_array())
                            .and_then(|results| results.first())
                            .and_then(|result| result.get("new_file_hash"))
                            .and_then(|hash| hash.as_str())
                            .and_then(|hash| uuid::Uuid::parse_str(hash).ok())
                            .map(ploke_core::TrackingHash);
                    }
                }
                AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id: observed_id,
                    call_id: observed_call,
                    error,
                    ..
                }) if observed_id == request_id && observed_call == call_id => {
                    panic!("{context}: tool failed before applied completion: {error}");
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{context}: timed out waiting for applied ToolCallCompleted"))
}

#[cfg(feature = "replay_tests")]
async fn expect_apply_applied(
    state: Arc<AppState>,
    event_bus: Arc<EventBus>,
    request: &ToolRequestRecord,
    context: &str,
) -> Uuid {
    let (request_id, _, call_id) = request_ids(request);
    let mut event_rx = event_bus.subscribe(EventPriority::Realtime);
    if let Some(proposal_id) = replay_apply(state, Arc::clone(&event_bus), request).await {
        let _ = wait_tool_applied(&mut event_rx, request, context).await;
        return proposal_id;
    }

    let error = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let AppEvent::System(SystemEvent::ToolCallFailed {
                request_id: observed_id,
                call_id: observed_call,
                error,
                ..
            }) = event_rx.recv().await.expect("event bus dropped")
                && observed_id == request_id
                && observed_call == call_id
            {
                return error;
            }
        }
    })
    .await
    .unwrap_or_else(|_| "timed out waiting for ToolCallFailed".to_string());
    panic!("{context}: apply_code_edit failed before staging: {error}");
}

#[cfg(feature = "replay_tests")]
async fn expect_insert_applied(
    state: Arc<AppState>,
    event_bus: Arc<EventBus>,
    request: &ToolRequestRecord,
    context: &str,
) -> Result<(Uuid, Option<ploke_core::TrackingHash>), ploke_error::Error> {
    let mut event_rx = event_bus.subscribe(EventPriority::Realtime);
    let proposal_id = replay_insert(state, Arc::clone(&event_bus), request).await?;
    let file_hash = wait_tool_applied(&mut event_rx, request, context).await;
    Ok((proposal_id, file_hash))
}

#[cfg(feature = "replay_tests")]
async fn assert_target_readable_after_refresh(
    state: &Arc<AppState>,
    file: &Path,
    node_type: NodeType,
    module_path: &[String],
    item_name: &str,
) {
    let rows = ploke_db::helpers::graph_resolve_exact(
        &state.db,
        node_type.relation_str(),
        file,
        module_path,
        item_name,
    )
    .expect("resolve replay target after refresh");
    assert!(
        !rows.is_empty(),
        "target {module_path:?}::{item_name} should resolve after refresh"
    );
    let snippets = state
        .io_handle
        .get_snippets_batch(rows)
        .await
        .expect("read snippets through refreshed semantic anchors");
    assert!(
        snippets.iter().all(Result::is_ok),
        "target {module_path:?}::{item_name} should be readable after refresh, got {snippets:?}"
    );
}

#[cfg(feature = "replay_tests")]
async fn replay_insert(
    state: Arc<AppState>,
    event_bus: Arc<EventBus>,
    request: &ToolRequestRecord,
) -> Result<Uuid, ploke_error::Error> {
    assert_eq!(request.tool, "insert_rust_item");
    let (request_id, parent_id, call_id) = request_ids(request);
    let ctx = Ctx {
        state,
        event_bus,
        request_id,
        parent_id,
        call_id,
    };

    let params = InsertRustItem::deserialize_params(request.arguments.as_str())
        .expect("historical insert_rust_item payload should deserialize");
    let ploke_tui::tools::ToolResult {
        content,
        ui_payload,
    } = InsertRustItem::execute(params, ctx.clone()).await?;
    InsertRustItem::emit_completed(&ctx, content, ui_payload);
    Ok(derive_edit_proposal_id(ctx.request_id, &ctx.call_id))
}

fn compact_headless_tool_request(path: &Path, event_index: usize) -> PersistedToolRequestRecord {
    let text = std::fs::read_to_string(path).expect("read compact headless evidence");
    let summary: crate::cli::prototype1_state::edit_surface::tui_adapter::evidence::Summary =
        serde_json::from_str(&text).expect("compact headless evidence should parse");
    let event = summary
        .events
        .get(event_index)
        .unwrap_or_else(|| panic!("missing compact headless event {event_index}"));
    let mut requests =
        crate::replay::self_edit::tool_requests_from_events(std::slice::from_ref(event))
            .expect("compact headless event should convert to a tool request");
    assert_eq!(requests.len(), 1);
    requests.remove(0)
}

async fn replay_ns_read_request(
    state: Arc<AppState>,
    event_bus: Arc<EventBus>,
    request: &PersistedToolRequestRecord,
) -> Result<NsReadResult, Box<dyn std::error::Error>> {
    let request_id = Uuid::parse_str(&request.request_id).expect("request_id should be a uuid");
    let parent_id = Uuid::parse_str(&request.parent_id).expect("parent_id should be a uuid");
    let ctx = Ctx {
        state,
        event_bus,
        request_id,
        parent_id,
        call_id: ploke_core::ArcStr::from(request.call_id.clone()),
    };

    let params = NsRead::deserialize_params(request.arguments.as_str())
        .expect("historical read_file payload should deserialize");
    let ploke_tui::tools::ToolResult {
        content,
        ui_payload,
    } = NsRead::execute(params, ctx.clone()).await?;
    let parsed = serde_json::from_str::<NsReadResult>(&content)?;
    NsRead::emit_completed(&ctx, content, ui_payload);
    Ok(parsed)
}

async fn replay_read_workspace_state(workspace: &Path) -> Arc<AppState> {
    let fixture_db =
        Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));
    let runtime = TestRuntime::new(&fixture_db);
    runtime
        .setup_loaded_workspace(
            workspace.to_path_buf(),
            vec![workspace.to_path_buf()],
            Some(workspace.to_path_buf()),
        )
        .await;

    let state = runtime.state_arc();
    let policy = state
        .with_system_read(|sys| sys.derive_path_policy(&[]).expect("path policy after load"))
        .await;
    state
        .io_handle
        .update_roots(Some(policy.roots.clone()), Some(policy.symlink_policy))
        .await;
    state
}

fn shared_path_stem(left: &Path, right: &Path) -> PathBuf {
    let mut stem = PathBuf::new();
    for (left_component, right_component) in left.components().zip(right.components()) {
        if left_component != right_component {
            break;
        }
        stem.push(left_component.as_os_str());
    }
    stem
}

fn path_after_stem<'a>(path: &'a Path, stem: &Path) -> &'a Path {
    path.strip_prefix(stem).unwrap_or(path)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "historical compact broad-harness replay of read_file fix"]
async fn read_fix() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    let trace_path = PathBuf::from(READ_FIX_TRACE);
    let workspace = PathBuf::from(READ_FIX_WORKSPACE);
    assert!(
        trace_path.exists(),
        "expected compact headless evidence at {}",
        trace_path.display()
    );
    assert!(
        workspace.exists(),
        "expected replay workspace at {}",
        workspace.display()
    );

    let request = compact_headless_tool_request(&trace_path, READ_FIX_EVENT_INDEX);
    assert_eq!(request.call_id, READ_FIX_CALL_ID);
    assert_eq!(request.tool, "read_file");
    let PersistedToolCallArguments::Decoded(ToolCallArguments::NsRead(args)) =
        request.arguments.decode_for_tool(&request.tool)
    else {
        panic!("expected compact event to decode as read_file arguments");
    };
    assert_eq!(
        args.file,
        "crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs"
    );
    assert_eq!(args.start_line, Some(1750));
    assert_eq!(args.end_line, Some(1800));

    let state = replay_read_workspace_state(&workspace).await;
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let mut event_rx = event_bus.subscribe(EventPriority::Realtime);
    let request_id = Uuid::parse_str(&request.request_id).expect("request_id uuid");
    let parent_id = Uuid::parse_str(&request.parent_id).expect("parent_id uuid");

    let read_result = replay_ns_read_request(state, Arc::clone(&event_bus), &request).await?;
    let completed_result = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let event = event_rx.recv().await.expect("event bus dropped");
            if let AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id: observed_request_id,
                parent_id: observed_parent_id,
                call_id,
                content,
                ..
            }) = event
                && observed_request_id == request_id
                && observed_parent_id == parent_id
                && call_id.as_ref() == READ_FIX_CALL_ID
            {
                break serde_json::from_str::<NsReadResult>(&content)
                    .expect("ToolCallCompleted content should be full NsReadResult JSON");
            }
        }
    })
    .await
    .expect("expected ToolCallCompleted for replayed read_file within timeout");

    let content = read_result
        .content
        .as_deref()
        .expect("read_file result should include content");
    let emitted_content = completed_result
        .content
        .as_deref()
        .expect("emitted read_file result should include content");
    let shared_stem = shared_path_stem(&trace_path, &workspace);
    let trace_display = path_after_stem(&trace_path, &shared_stem);
    let workspace_display = path_after_stem(&workspace, &shared_stem);
    println!(
        "\nREAD_FIX call_id={}\n\
    shared_stem={}\n\
    trace={}\n\
    workspace={}\n\
    file={}\n\
    range={:?}-{:?}\n\
    ok={}\n\
    exists={}\n\
    truncated={}\n\
    byte_len={:?}\n\
    content_chars={}\n\
    contains_target={}\n\
    target_lines:\n{}\n",
        request.call_id,
        shared_stem.display(),
        trace_display.display(),
        workspace_display.display(),
        args.file,
        args.start_line,
        args.end_line,
        read_result.ok,
        read_result.exists,
        read_result.truncated,
        read_result.byte_len,
        content.chars().count(),
        content.contains(READ_FIX_TARGET),
        content.trim_end(),
    );

    assert!(read_result.ok);
    assert!(read_result.exists);
    assert!(
        !content.is_empty(),
        "compact r2 replay should no longer reproduce successful empty read_file content"
    );
    assert_eq!(emitted_content, content);
    assert!(
        content.contains(READ_FIX_TARGET),
        "replayed read_file should return the requested high-line range"
    );
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

/// Debug aid for this replay: show the DB resolution behavior around `canon` parsing.
///
/// This is intentionally print-based because it is diagnostic, not a stable contract.
fn diag_resolve_canon(db: &Database, node_type: NodeType, abs_path: &Path, canon: &str) {
    let relation = node_type.relation_str();
    let canon_trim = canon.trim();
    let segs: Vec<&str> = canon_trim.split("::").filter(|s| !s.is_empty()).collect();
    if segs.is_empty() {
        eprintln!("REPLAY_DIAG: empty canon, skipping diagnostics");
        return;
    }

    let item_name = segs[segs.len().saturating_sub(1)];
    let mut tool_mod_path: Vec<String> = segs[..segs.len().saturating_sub(1)]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    if tool_mod_path.first().map(|s| s.as_str()) != Some("crate") {
        tool_mod_path.insert(0, "crate".to_string());
    }

    eprintln!("REPLAY_DIAG: file={}", abs_path.display());
    eprintln!("REPLAY_DIAG: canon={}", canon_trim);
    eprintln!(
        "REPLAY_DIAG: tool_parse relation={} mod_path={:?} item_name={}",
        relation, tool_mod_path, item_name
    );

    let strict =
        ploke_db::helpers::graph_resolve_exact(db, relation, abs_path, &tool_mod_path, item_name)
            .unwrap_or_else(|e| {
                eprintln!("REPLAY_DIAG: graph_resolve_exact error: {}", e);
                Vec::new()
            });
    eprintln!("REPLAY_DIAG: graph_resolve_exact hits={}", strict.len());

    // Show what the DB thinks exists in this file (for embedded primary nodes).
    // This helps answer "is the node present at all?" and "under which relation?".
    match ploke_db::helpers::list_primary_nodes(db) {
        Ok(rows) => {
            let file_rows = rows
                .into_iter()
                .filter(|row| row.file_path.as_path() == abs_path)
                .collect::<Vec<_>>();
            eprintln!(
                "REPLAY_DIAG: list_primary_nodes file_rows={}",
                file_rows.len()
            );

            let mut by_rel: std::collections::BTreeMap<String, usize> =
                std::collections::BTreeMap::new();
            for row in &file_rows {
                *by_rel.entry(row.relation.clone()).or_insert(0) += 1;
            }
            eprintln!("REPLAY_DIAG: list_primary_nodes by_relation={:?}", by_rel);

            let mut exact = file_rows
                .iter()
                .filter(|row| row.name == item_name)
                .collect::<Vec<_>>();
            exact.sort_by(|a, b| a.relation.cmp(&b.relation));
            if exact.is_empty() {
                eprintln!(
                    "REPLAY_DIAG: list_primary_nodes: no primary-node name=={} found in file",
                    item_name
                );
            } else {
                for row in exact {
                    eprintln!(
                        "REPLAY_DIAG: primary_node EXACT_MATCH relation={} name={} mod_path={:?}",
                        row.relation, row.name, row.module_path
                    );
                }
            }

            let mut same_rel = file_rows
                .iter()
                .filter(|row| row.relation == relation)
                .collect::<Vec<_>>();
            same_rel.sort_by(|a, b| a.name.cmp(&b.name));
            for row in same_rel.iter().take(12) {
                eprintln!(
                    "REPLAY_DIAG: primary_node (sample) relation={} name={} mod_path={:?}",
                    row.relation, row.name, row.module_path
                );
            }
        }
        Err(e) => {
            eprintln!("REPLAY_DIAG: list_primary_nodes error: {}", e);
        }
    }

    // Show relaxed resolution attempts across likely canon interpretations:
    // - as-parsed module path + simple item name (current tool fallback)
    // - progressively popping module segments (type segments are often present for methods)
    // - module path without the trailing type segment + item name "Type::method"
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut dump_relaxed = |label: &str, mod_path: Vec<String>, item: String| {
        let hits = ploke_db::helpers::resolve_nodes_by_canon(db, relation, &mod_path, &item)
            .unwrap_or_else(|e| {
                eprintln!("REPLAY_DIAG: {label} resolve_nodes_by_canon error: {}", e);
                Vec::new()
            });
        if hits.is_empty() {
            eprintln!(
                "REPLAY_DIAG: {label} mod_path={:?} item={} hits=0",
                mod_path, item
            );
            return;
        }
        let mut files: BTreeSet<String> = BTreeSet::new();
        for hit in hits {
            files.insert(hit.file_path.display().to_string());
        }
        for f in &files {
            seen.insert(f.clone());
        }
        eprintln!(
            "REPLAY_DIAG: {label} mod_path={:?} item={} hits={} files={:?}",
            mod_path,
            item,
            files.len(),
            files
        );
    };

    dump_relaxed(
        "relaxed_as_tool_parsed",
        tool_mod_path.clone(),
        item_name.to_string(),
    );

    // Pop segments to detect whether the "type segment" is the mismatch.
    let mut popped = tool_mod_path.clone();
    while popped.len() > 1 {
        popped.pop();
        dump_relaxed(
            "relaxed_popped_mod_path",
            popped.clone(),
            item_name.to_string(),
        );
    }

    // Try treating the last non-item segment as a type name, folding it into the item name.
    if segs.len() >= 3 {
        let type_name = segs[segs.len().saturating_sub(2)];
        let item = format!("{type_name}::{item_name}");
        let mut mod_path: Vec<String> = segs[..segs.len().saturating_sub(2)]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        if mod_path.first().map(|s| s.as_str()) != Some("crate") {
            mod_path.insert(0, "crate".to_string());
        }
        dump_relaxed("relaxed_type_folded_into_item", mod_path, item);
    }

    // If we did find candidates, also surface whether this is a pure path-normalization mismatch.
    if !seen.is_empty() {
        let req = abs_path.display().to_string();
        let req_canon = std::fs::canonicalize(abs_path)
            .ok()
            .map(|p| p.display().to_string());
        eprintln!("REPLAY_DIAG: requested abs_path={req}");
        if let Some(canon) = req_canon.as_ref() {
            eprintln!("REPLAY_DIAG: requested canonicalize(abs_path)={canon}");
        }
        for f in seen {
            if f == req {
                eprintln!("REPLAY_DIAG: candidate matches requested path exactly: {f}");
                continue;
            }
            if let Ok(cand_canon) = std::fs::canonicalize(&f) {
                let cand_canon = cand_canon.display().to_string();
                if req_canon.as_ref().is_some_and(|r| r == &cand_canon) {
                    eprintln!(
                        "REPLAY_DIAG: candidate differs but canonicalize() matches requested: {f}"
                    );
                }
            }
        }
    }
}

fn diag_probe_primary_candidates(db: &Database, abs_path: &Path, item_name: &str) {
    eprintln!(
        "REPLAY_DIAG: probing primary candidates for {}",
        abs_path.display()
    );

    match ploke_db::helpers::list_primary_nodes(db) {
        Ok(rows) => {
            let mut file_rows = rows
                .into_iter()
                .filter(|row| row.file_path.as_path() == abs_path)
                .collect::<Vec<_>>();
            file_rows.sort_by(|a, b| a.relation.cmp(&b.relation).then(a.name.cmp(&b.name)));

            eprintln!(
                "REPLAY_DIAG: primary_candidates file_rows={}",
                file_rows.len()
            );

            let mut by_rel: BTreeMap<String, usize> = BTreeMap::new();
            for row in &file_rows {
                *by_rel.entry(row.relation.clone()).or_insert(0) += 1;
            }
            eprintln!("REPLAY_DIAG: primary_candidates by_relation={:?}", by_rel);

            let exact = file_rows.iter().filter(|row| row.name == item_name).count();
            eprintln!(
                "REPLAY_DIAG: primary_candidates exact_name_matches={}",
                exact
            );

            let function_rows = file_rows
                .iter()
                .filter(|row| row.relation == "function")
                .collect::<Vec<_>>();
            eprintln!(
                "REPLAY_DIAG: primary_candidates function_rows={}",
                function_rows.len()
            );
            for row in function_rows.iter().take(20) {
                eprintln!(
                    "REPLAY_DIAG: primary_candidate relation={} name={} mod_path={:?}",
                    row.relation, row.name, row.module_path
                );
            }
        }
        Err(e) => {
            eprintln!("REPLAY_DIAG: list_primary_nodes error: {}", e);
        }
    }
}

fn diag_probe_name_anywhere(db: &Database, item_name: &str) {
    let name_lit =
        serde_json::to_string(item_name).expect("stringifying replay probe name must succeed");

    let relations = NodeType::primary_and_assoc_nodes();
    eprintln!(
        "REPLAY_DIAG: probing item name across {} primary+assoc relations",
        relations.len()
    );

    let mut matches: BTreeMap<String, usize> = BTreeMap::new();
    for relation in relations {
        let relation_name = relation.relation_str().to_string();
        let script = format!(
            r#"
?[name] :=
    *{rel}{{ name @ 'NOW' }},
    name == {name_lit}
"#,
            rel = relation_name,
            name_lit = name_lit
        );
        let rows = ploke_db::QueryResult::from(
            db.raw_query(&script)
                .unwrap_or_else(|e| panic!("name probe query failed for {relation_name}: {e}")),
        );
        if !rows.rows.is_empty() {
            matches.insert(relation_name, rows.rows.len());
        }
    }

    if matches.is_empty() {
        eprintln!("REPLAY_DIAG: name_anywhere name={} hits=0", item_name);
        return;
    }

    let total_hits: usize = matches.values().copied().sum();
    eprintln!(
        "REPLAY_DIAG: name_anywhere name={} total_hits={} by_relation={:?}",
        item_name, total_hits, matches
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "historical diagnostic replay of eval-run artifact"]
async fn test_apply_code_edit_historical_failure_path() {
    const INSTANCE_ID: &str = "BurntSushi__ripgrep-2209";
    const REPO_ROOT: &str = "/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep";

    let run_dir = historical_run_dir_with(INSTANCE_ID, &["final-snapshot.db"]);
    let snapshot_db_path = run_dir.join("final-snapshot.db");
    assert!(
        snapshot_db_path.exists(),
        "expected eval snapshot db to exist at {}",
        snapshot_db_path.display()
    );
    assert!(
        PathBuf::from(REPO_ROOT).exists(),
        "expected eval repo root to exist at {REPO_ROOT}"
    );

    let recorded = load_recorded_apply_code_edit_request();
    assert_eq!(recorded.tool, "apply_code_edit");
    println!(
        "REPLAY request_id={} parent_id={} call_id={} edits={}",
        recorded.request_id,
        recorded.parent_id,
        recorded.call_id,
        recorded.arguments.edits.len()
    );
    for edit in &recorded.arguments.edits {
        println!(
            "REPLAY edit file={} canon={} node_type={:?}",
            edit.file, edit.canon, edit.node_type
        );
    }

    let snapshot_db = Arc::new(
        Database::create_new_backup_default(&snapshot_db_path)
            .await
            .expect("load eval snapshot db"),
    );
    let processor = ploke_tui::user_config::UserConfig::default()
        .load_embedding_processor()
        .expect("load embedding processor");
    let runtime = TestRuntime::new_with_embedding_processor(&snapshot_db, processor);
    runtime
        .setup_loaded_standalone_crate(PathBuf::from(REPO_ROOT))
        .await;

    {
        let state = runtime.state_arc();
        let mut cfg = state.config.write().await;
        cfg.editing.auto_confirm_edits = true;
        cfg.chat_policy = benchmark_chat_policy();
    }

    let state = runtime.state_arc();
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let mut event_rx = event_bus.subscribe(EventPriority::Realtime);
    let recorded_request_id = recorded.request_id;
    let recorded_parent_id = recorded.parent_id;
    let recorded_call_id = recorded.call_id.clone();
    // Historical replay only: this fixture preserves the earlier failure mode for diagnosis.
    if let Some(edit) = recorded.arguments.edits.first() {
        let abs_path = PathBuf::from(&edit.file);
        diag_resolve_canon(
            &snapshot_db,
            edit.node_type,
            abs_path.as_path(),
            &edit.canon,
        );
        // Note: `item_name` for the recorded canon is the final segment ("replace_all").
        // We probe candidates using that, not the full canon string.
        let item_name = edit
            .canon
            .split("::")
            .filter(|s| !s.is_empty())
            .last()
            .unwrap_or(edit.canon.as_str());
        diag_probe_primary_candidates(&snapshot_db, abs_path.as_path(), item_name);
        diag_probe_name_anywhere(&snapshot_db, item_name);
    }

    let params = recorded.into_tool_params(state, Arc::clone(&event_bus));
    apply_code_edit_tool(params).await;

    let state = runtime.state_arc();
    let proposals = state.proposals.read().await;
    assert!(
        proposals.is_empty(),
        "historical replay should not stage a proposal; it reproduces the recorded failure mode"
    );
    drop(proposals);

    let (request_id, parent_id, call_id, error, ui_payload) =
        tokio::time::timeout(Duration::from_secs(2), async move {
            loop {
                let event = event_rx.recv().await.expect("event bus dropped");
                if let AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    parent_id,
                    call_id,
                    error,
                    ui_payload,
                }) = event
                {
                    break (request_id, parent_id, call_id, error, ui_payload);
                }
            }
        })
        .await
        .expect("expected ToolCallFailed for the recorded historical request within timeout");

    assert_eq!(request_id, recorded_request_id);
    assert_eq!(parent_id, recorded_parent_id);
    assert_eq!(call_id.as_ref(), recorded_call_id);

    let wire = ToolErrorWire::parse(&error).expect("parse ToolCallFailed wire payload");
    assert_eq!(wire.llm["code"].as_str(), Some("WrongType"));
    assert_eq!(wire.llm["field"].as_str(), Some("node_type"));
    assert_eq!(wire.llm["expected"].as_str(), Some("method"));
    assert_eq!(wire.llm["received"].as_str(), Some("function"));
    assert!(
        wire.llm["retry_hint"]
            .as_str()
            .is_some_and(|hint| hint.contains("node_type=method")),
        "expected method retry hint in wire payload"
    );

    let retry_context = wire.llm["retry_context"]
        .as_object()
        .expect("retry_context object");
    assert_eq!(
        retry_context
            .get("requested_node_type")
            .and_then(|v| v.as_str()),
        Some("function")
    );
    assert_eq!(
        retry_context
            .get("suggested_node_type")
            .and_then(|v| v.as_str()),
        Some("method")
    );
    assert_eq!(
        retry_context.get("owner_name").and_then(|v| v.as_str()),
        Some("Replacer")
    );
    assert_eq!(
        retry_context.get("canon").and_then(|v| v.as_str()),
        Some("crate::util::Replacer::replace_all")
    );
    assert!(
        retry_context
            .get("reason")
            .and_then(|v| v.as_str())
            .is_some_and(|reason| reason.contains("unique method target")),
        "expected method retry reason in retry_context"
    );

    let ui_payload = ui_payload.expect("expected ui_payload on ToolCallFailed");
    assert_eq!(ui_payload.call_id.as_ref(), recorded_call_id);
    assert_eq!(ui_payload.tool, ToolName::ApplyCodeEdit);
    assert_eq!(ui_payload.error_code, Some(ToolErrorCode::WrongType));
}

// Regression for the historical BurntSushi/ripgrep setup failure (edition-2015 mixed
// workspace). Edition-2015 members route through the dual-syn syn1 path unconditionally,
// so indexing succeeds even when `convert_keyword_2015` is disabled.
#[tokio::test(flavor = "multi_thread")]
async fn regression_ripgrep_setup_indexes_without_convert_keyword_2015() {
    init_tracing();
    const SOURCE_MANIFEST: &str =
        "/home/brasides/.ploke-eval/instances/BurntSushi__ripgrep-1642/run.json";

    assert!(
        PathBuf::from(SOURCE_MANIFEST).exists(),
        "expected historical run manifest at {SOURCE_MANIFEST}"
    );

    let mut prepared = load_prepared_single_run(Path::new(SOURCE_MANIFEST));
    prepared.task_id = format!("{}-regression-dual-syn", prepared.task_id);
    checkout_repo_to_base(&prepared.repo_root, prepared.base_sha.as_deref())
        .expect("historical ripgrep repo should checkout base sha");
    let resolved = resolve_index_target(Some(prepared.repo_root.clone()), &prepared.repo_root)
        .expect("historical ripgrep root should resolve");
    assert_eq!(resolved.kind, IndexTargetKind::Workspace);
    assert!(
        resolved
            .member_roots
            .iter()
            .any(|root| root.ends_with("crates/printer")),
        "expected ripgrep workspace members to include target crate: {:?}",
        resolved.member_roots
    );

    let runtime_db = init_runtime_db().expect("init runtime db");
    let runtime = TestRuntime::new_with_embedding_processor(
        &runtime_db,
        sparse_headless_embedding_processor(),
    );
    let state = runtime.state_arc();

    prepare_sparse_workspace(&state, &prepared.repo_root, &[])
        .await
        .expect("ripgrep setup should index successfully via dual-syn syn1 path without convert_keyword_2015");

    let crate_rows = state
        .db
        .list_crate_context_rows()
        .expect("crate context rows after sparse workspace parse");
    let crate_names = crate_rows
        .iter()
        .map(|row| row.name.as_str())
        .collect::<BTreeSet<_>>();
    for expected in ["grep-printer", "ignore", "globset"] {
        assert!(
            crate_names.contains(expected),
            "expected ripgrep workspace member '{expected}' after indexing, got {crate_rows:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
// NOTE: we don't want to ignore these anymore, that is a bad pattern. they end up just getting forgotten.
// better is to use this pattern.
#[cfg(feature = "replay_tests")]
async fn test_replay_historical_fd_1121_partial_non_semantic_patch_runtime_flow() {
    init_tracing();
    const INSTANCE_ID: &str = "sharkdp__fd-1121";
    // TODO: change these to point towards a local git-entered fixture of the previous live run record.
    const RUN_MANIFEST: &str = "/home/brasides/.ploke-eval/instances/sharkdp__fd-1121/run.json";
    const JOB_CALL_ID: &str = "call_86042515";
    const WALK_CALL_ID: &str = "call_80363220";

    // TODO: update helper function as well.
    let run_dir = historical_run_dir_with_tool_calls(INSTANCE_ID, &[JOB_CALL_ID, WALK_CALL_ID]);
    let run_manifest = PathBuf::from(RUN_MANIFEST);
    let turn_trace = run_dir.join("agent-turn-trace.json");
    assert!(
        run_manifest.exists(),
        "expected historical run manifest at {}",
        run_manifest.display()
    );
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

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "replay_tests")]
async fn historical_ripgrep_ignore_post_apply_refresh_replays_stale_anchor_recovery() {
    init_tracing();
    const RUN_MANIFEST: &str = "/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state-20260608-234216/BurntSushi__ripgrep-2295/run.json";
    const RUN_DIR: &str = "/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state-20260608-234216/BurntSushi__ripgrep-2295/runs/run-1780987402061-structured-current-policy-27d5f4ae";
    const APPLY_MATCHED_IGNORE_CALL_ID: &str = "function-call-39b4339b-483c-4548-9482-1c596bf61158";
    const INSERT_STRIP_OVERLAP_CALL_ID: &str = "function-call-54859f90-0820-4835-8cb5-d780d43e1935";
    const APPLY_LINKED_WORKTREE_TEST_CALL_ID: &str =
        "function-call-d0de1cd9-b0d9-4f40-a10c-6e09ac022af5";

    let run_manifest = PathBuf::from(RUN_MANIFEST);
    let turn_trace = PathBuf::from(RUN_DIR).join("agent-turn-trace.json");
    assert!(
        run_manifest.exists(),
        "expected historical run manifest at {}",
        run_manifest.display()
    );
    assert!(
        turn_trace.exists(),
        "expected historical turn trace at {}",
        turn_trace.display()
    );

    let historical = load_prepared_single_run(&run_manifest);
    let trace = load_agent_turn_artifact(&turn_trace);
    let first_request = find_tool_request(&trace, APPLY_MATCHED_IGNORE_CALL_ID);
    let insert_request = find_tool_request(&trace, INSERT_STRIP_OVERLAP_CALL_ID);
    let later_request = find_tool_request(&trace, APPLY_LINKED_WORKTREE_TEST_CALL_ID);

    let temp = tempdir().expect("tempdir");
    let replay_repo_root = temp.path().join("ripgrep-ignore-refresh-replay");
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
        .expect("setup replay runtime for ripgrep ignore refresh");
    {
        let mut cfg = state.config.write().await;
        cfg.editing.auto_confirm_edits = true;
        cfg.chat_policy = benchmark_chat_policy();
    }
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    let first_id = expect_apply_applied(
        Arc::clone(&state),
        Arc::clone(&event_bus),
        &first_request,
        "historical first apply_code_edit",
    )
    .await;
    let first_status = wait_for_terminal_proposal_status(&state, first_id).await;
    assert_eq!(
        first_status,
        ploke_tui::app_state::core::EditProposalStatus::Applied
    );

    let (insert_id, _insert_hash) = expect_insert_applied(
        Arc::clone(&state),
        Arc::clone(&event_bus),
        &insert_request,
        "historical insert_rust_item",
    )
    .await
    .expect("historical insert_rust_item should apply");
    let insert_status = wait_for_terminal_proposal_status(&state, insert_id).await;
    assert_eq!(
        insert_status,
        ploke_tui::app_state::core::EditProposalStatus::Applied
    );

    let target_file = prepared.repo_root.join("crates/ignore/src/dir.rs");
    assert_target_readable_after_refresh(
        &state,
        &target_file,
        NodeType::Function,
        &["crate".to_string(), "dir".to_string(), "tests".to_string()],
        "git_info_exclude_in_linked_worktree",
    )
    .await;

    let later_id = expect_apply_applied(
        Arc::clone(&state),
        Arc::clone(&event_bus),
        &later_request,
        "historical later apply_code_edit",
    )
    .await;
    let later_status = wait_for_terminal_proposal_status(&state, later_id).await;
    assert_eq!(
        later_status,
        ploke_tui::app_state::core::EditProposalStatus::Applied
    );

    let diff = git_stdout(
        &prepared.repo_root,
        &["diff", "--no-ext-diff"],
        "git diff after replay",
    );
    assert!(
        diff.contains("diff --git a/crates/ignore/src/dir.rs b/crates/ignore/src/dir.rs"),
        "replayed repo diff should include crates/ignore/src/dir.rs:\n{diff}"
    );
    assert!(
        diff.contains("fn strip_overlap"),
        "replayed insert_rust_item diff should include strip_overlap:\n{diff}"
    );
    assert!(
        diff.contains("fn regression_1757"),
        "replayed later apply_code_edit diff should include regression_1757:\n{diff}"
    );
}
