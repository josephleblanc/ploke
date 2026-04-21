use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
};

use ploke_db::{Database, NodeType};
use ploke_tui::{
    AppEvent, EventBus, EventBusCaps, EventPriority,
    app::commands::harness::TestRuntime,
    app_state::{AppState, core::derive_edit_proposal_id, events::SystemEvent},
    rag::{
        tools::apply_code_edit_tool,
        utils::{ApplyCodeEditRequest, Edit, ToolCallParams},
    },
    tools::{Ctx, Tool, ToolErrorCode, ToolErrorWire, ToolName, ns_patch::NsPatch},
    user_config::{ChatPolicy, ChatTimeoutStrategy},
};
use serde::Deserialize;
use tempfile::tempdir;
use tracing_subscriber::fmt::SubscriberBuilder;
use uuid::Uuid;

use crate::{
    PreparedSingleRun,
    runner::{
        AgentTurnArtifact, IndexingStatusArtifact, ObservedTurnEvent, RunMsbSingleRequest,
        ToolRequestRecord, setup_replay_runtime,
    },
    spec::PrepareError,
};

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

fn load_agent_turn_artifact(path: &Path) -> AgentTurnArtifact {
    let text = std::fs::read_to_string(path).expect("read historical agent turn artifact");
    serde_json::from_str(&text).expect("historical agent turn artifact must parse")
}

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

    let params = NsPatch::deserialize_params(&request.arguments)
        .expect("historical non_semantic_patch payload should deserialize");
    let ploke_tui::tools::ToolResult {
        content,
        ui_payload,
    } = NsPatch::execute(params, ctx.clone()).await?;
    NsPatch::emit_completed(&ctx, content, ui_payload);
    Ok(())
}

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
    const SNAPSHOT_DB: &str =
        "/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/final-snapshot.db";
    const REPO_ROOT: &str = "/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep";

    assert!(
        PathBuf::from(SNAPSHOT_DB).exists(),
        "expected eval snapshot db to exist at {SNAPSHOT_DB}"
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
        Database::create_new_backup_default(SNAPSHOT_DB)
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

#[tokio::test(flavor = "multi_thread")]
#[ignore = "historical diagnostic replay of ripgrep setup failure"]
#[cfg(not(feature = "convert_keyword_2015"))]
async fn test_historical_ripgrep_setup_failure_reports_indexing_failed_and_status_artifact_without_convert_keyword_2015()
 {
    init_tracing();
    const SOURCE_MANIFEST: &str =
        "/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1642/run.json";

    assert!(
        PathBuf::from(SOURCE_MANIFEST).exists(),
        "expected historical run manifest at {SOURCE_MANIFEST}"
    );

    let temp = tempdir().expect("tempdir");
    let mut prepared = load_prepared_single_run(Path::new(SOURCE_MANIFEST));
    prepared.task_id = format!("{}-without-convert-keyword-2015", prepared.task_id);
    prepared.output_dir = temp.path().join("out");
    std::fs::create_dir_all(&prepared.output_dir).expect("create replay output dir");

    let replay_manifest = temp.path().join("run.json");
    std::fs::write(
        &replay_manifest,
        serde_json::to_string_pretty(&prepared).expect("serialize replay manifest"),
    )
    .expect("write replay manifest");

    // WARN: keep this negative-path replay while `convert_keyword_2015` remains
    // feature-gated. It documents the original historical failure without the
    // fallback enabled.
    let err = RunMsbSingleRequest {
        run_manifest: replay_manifest,
        index_debug_snapshots: false,
        use_default_model: true,
        model_id: None,
        provider: None,
    }
    .run()
    .await
    .expect_err("historical ripgrep setup replay should fail during indexing");

    match err {
        PrepareError::IndexingFailed { detail } => {
            assert!(
                detail.contains("Parse failed for crate"),
                "unexpected indexing failure detail: {detail}"
            );
        }
        other => panic!("expected indexing failure, got {other}"),
    }

    let indexing_status_path = prepared.output_dir.join("indexing-status.json");
    assert!(
        indexing_status_path.exists(),
        "expected indexing status artifact at {}",
        indexing_status_path.display()
    );
    let artifact: IndexingStatusArtifact = serde_json::from_str(
        &std::fs::read_to_string(&indexing_status_path).expect("read indexing status artifact"),
    )
    .expect("parse indexing status artifact");
    assert_eq!(artifact.status, "failed");
    assert!(artifact.detail.contains("Parse failed for crate"));

    let parse_failure_path = prepared.output_dir.join("parse-failure.json");
    assert!(
        parse_failure_path.exists(),
        "expected parse failure artifact at {}",
        parse_failure_path.display()
    );
    let parse_failure: crate::runner::ParseFailureArtifact = serde_json::from_str(
        &std::fs::read_to_string(&parse_failure_path).expect("read parse failure artifact"),
    )
    .expect("parse parse failure artifact");
    let concrete_source_path = parse_failure
        .diagnostics
        .iter()
        .filter_map(|diag| diag.source_path.as_ref())
        .find(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .cloned()
        .expect("historical ripgrep replay should surface a concrete failing rust source path");
    eprintln!(
        "REPLAY_DIAG: ripgrep historical setup concrete failing source path={}",
        concrete_source_path.display()
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "historical diagnostic replay of fd-1121 non-semantic patch partial-apply runtime flow"]
async fn test_replay_historical_fd_1121_partial_non_semantic_patch_runtime_flow() {
    init_tracing();
    const RUN_MANIFEST: &str = "/home/brasides/.ploke-eval/runs/sharkdp__fd-1121/run.json";
    const TURN_TRACE: &str =
        "/home/brasides/.ploke-eval/runs/sharkdp__fd-1121/agent-turn-trace.json";
    const JOB_CALL_ID: &str = "call_86042515";
    const WALK_CALL_ID: &str = "call_80363220";

    let run_manifest = PathBuf::from(RUN_MANIFEST);
    let turn_trace = PathBuf::from(TURN_TRACE);
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

    replay_ns_patch_request(Arc::clone(&state), Arc::clone(&event_bus), &job_request)
        .await
        .expect("historical job non_semantic_patch replay should execute");
    let job_request_id = Uuid::parse_str(&job_request.request_id).expect("job request id uuid");
    let job_call_id: ploke_core::ArcStr = job_request.call_id.clone().into();
    let job_proposal_id = derive_edit_proposal_id(job_request_id, &job_call_id);
    let job_status = wait_for_terminal_proposal_status(
        &state,
        job_proposal_id,
    )
    .await;
    assert_eq!(
        job_status,
        ploke_tui::app_state::core::EditProposalStatus::Applied
    );

    let walk_request_id = Uuid::parse_str(&walk_request.request_id).expect("walk request id uuid");
    let walk_call_id: ploke_core::ArcStr = walk_request.call_id.clone().into();
    let walk_proposal_id = derive_edit_proposal_id(walk_request_id, &walk_call_id);
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
#[ignore = "historical diagnostic replay of ripgrep setup success with convert_keyword_2015"]
#[cfg(feature = "convert_keyword_2015")]
async fn test_historical_ripgrep_setup_replay_gets_past_indexing_with_convert_keyword_2015() {
    init_tracing();
    const SOURCE_MANIFEST: &str =
        "/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1642/run.json";

    assert!(
        PathBuf::from(SOURCE_MANIFEST).exists(),
        "expected historical run manifest at {SOURCE_MANIFEST}"
    );

    let temp = tempdir().expect("tempdir");
    let mut prepared = load_prepared_single_run(Path::new(SOURCE_MANIFEST));
    prepared.task_id = format!("{}-with-convert-keyword-2015", prepared.task_id);
    prepared.output_dir = temp.path().join("out");
    std::fs::create_dir_all(&prepared.output_dir).expect("create replay output dir");

    let replay_manifest = temp.path().join("run.json");
    std::fs::write(
        &replay_manifest,
        serde_json::to_string_pretty(&prepared).expect("serialize replay manifest"),
    )
    .expect("write replay manifest");

    // WARN: this replay exists to prove why `convert_keyword_2015` exists.
    // The only stable contract here is that setup no longer stops at indexing.
    let result = RunMsbSingleRequest {
        run_manifest: replay_manifest,
        index_debug_snapshots: false,
        use_default_model: true,
        model_id: None,
        provider: None,
    }
    .run()
    .await;

    assert!(
        !matches!(result, Err(PrepareError::IndexingFailed { .. })),
        "convert_keyword_2015 should get the historical replay past indexing, got: {result:?}"
    );

    let indexing_status_path = prepared.output_dir.join("indexing-status.json");
    assert!(
        indexing_status_path.exists(),
        "expected indexing status artifact at {}",
        indexing_status_path.display()
    );
    let artifact: IndexingStatusArtifact = serde_json::from_str(
        &std::fs::read_to_string(&indexing_status_path).expect("read indexing status artifact"),
    )
    .expect("parse indexing status artifact");
    assert_ne!(
        artifact.status, "failed",
        "historical replay should not stop at indexing failure with convert_keyword_2015"
    );
}
