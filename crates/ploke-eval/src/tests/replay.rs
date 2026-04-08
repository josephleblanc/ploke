use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use ploke_db::{Database, NodeType};
use ploke_tui::{AppEvent, EventBus, EventBusCaps, EventPriority, app::commands::harness::TestRuntime, app_state::{AppState, events::SystemEvent}, rag::{tools::apply_code_edit_tool, utils::{ApplyCodeEditRequest, Edit, ToolCallParams}}, tools::ToolName, user_config::{ChatPolicy, ChatTimeoutStrategy}};
use serde::Deserialize;
use uuid::Uuid;


#[derive(Debug, Deserialize)]
struct RecordedApplyCodeEditToolRequest {
    request_id: Uuid,
    parent_id: Uuid,
    call_id: String,
    tool: String,
    arguments: RecordedApplyCodeEditArguments,
}

#[derive(Debug, Deserialize)]
struct RecordedToolFailedFixture {
    request_id: Uuid,
    parent_id: Uuid,
    call_id: String,
    tool: ToolName,
    error: String,
    ui_payload: serde_json::Value,
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

impl RecordedApplyCodeEditToolRequest {
    fn into_tool_params(
        self,
        state: Arc<AppState>,
        event_bus: Arc<EventBus>,
    ) -> ToolCallParams {
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

fn load_recorded_apply_code_edit_failure() -> RecordedToolFailedFixture {
    serde_json::from_str(include_str!(
        "fixtures/BurntSushi__ripgrep-2209_apply_code_edit_tool_failed.json"
    ))
    .expect("recorded apply_code_edit ToolFailed fixture must be valid json")
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

    let strict = ploke_db::helpers::graph_resolve_exact(
        db,
        relation,
        abs_path,
        &tool_mod_path,
        item_name,
    )
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
            eprintln!("REPLAY_DIAG: list_primary_nodes file_rows={}", file_rows.len());

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
    eprintln!("REPLAY_DIAG: probing primary candidates for {}", abs_path.display());

    match ploke_db::helpers::list_primary_nodes(db) {
        Ok(rows) => {
            let mut file_rows = rows
                .into_iter()
                .filter(|row| row.file_path.as_path() == abs_path)
                .collect::<Vec<_>>();
            file_rows.sort_by(|a, b| a.relation.cmp(&b.relation).then(a.name.cmp(&b.name)));

            eprintln!("REPLAY_DIAG: primary_candidates file_rows={}", file_rows.len());

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
    let name_lit = serde_json::to_string(item_name)
        .expect("stringifying replay probe name must succeed");

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
#[ignore = "diagnostic replay of eval-run artifact"]
async fn test_apply_code_edit_failure_path() {
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
    let expected_failure = load_recorded_apply_code_edit_failure();
    assert_eq!(expected_failure.tool, ToolName::ApplyCodeEdit);

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
        "recorded failure path should not stage a proposal"
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
        .expect("expected ToolCallFailed for the recorded strict+fallback node miss within timeout");

    assert_eq!(request_id, expected_failure.request_id);
    assert_eq!(parent_id, expected_failure.parent_id);
    assert_eq!(call_id.as_ref(), expected_failure.call_id);
    assert_eq!(error, expected_failure.error);

    let ui_payload = ui_payload.expect("expected ui_payload on ToolCallFailed");
    let observed_ui_payload =
        serde_json::to_value(&ui_payload).expect("serialize observed ui_payload");
    assert_eq!(observed_ui_payload, expected_failure.ui_payload);
    assert_eq!(ui_payload.tool, expected_failure.tool);
    assert_eq!(ui_payload.call_id.as_ref(), expected_failure.call_id);
    assert!(
        ui_payload
            .error
            .as_ref()
            .map(|err| err.system.contains("No matching node found (strict+fallback)"))
            .unwrap_or(false),
        "expected ToolCallFailed for the recorded strict+fallback node miss"
    );

    assert!(
        ui_payload.error_code.is_some(),
        "expected structured error code on ToolCallFailed"
    );
}
