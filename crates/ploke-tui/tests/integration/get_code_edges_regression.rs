use std::{borrow::Cow, collections::HashMap, sync::Arc};

use ploke_core::{
    ArcStr,
    rag_types::{
        CallCalleeInfo, CallContextInfo, CallResolutionKind, CallSiteKind, CallStatusKind,
        CallTargetKind, ProofContextInfo,
    },
};
use ploke_db::helpers::{graph_resolve_edges, graph_resolve_exact, list_primary_nodes};
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::IoManagerHandle;
use ploke_rag::TokenBudget;
use ploke_test_utils::{PLOKE_DB_PRIMARY, shared_backup_fixture_db, workspace_root};
use ploke_tui::{
    EventBus,
    app_state::{
        SystemStatus,
        core::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState},
    },
    chat_history::ChatHistory,
    event_bus::EventBusCaps,
    tools::{
        Ctx, Tool,
        get_code_edges::{CodeItemEdges, EdgesParams},
    },
    user_config::UserConfig,
};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::call_graph_tool_support::{
    AxumAwaitReceiverToolFixture, AxumBodyEmptyToolFixture, AxumBoxedIntoRouteToolFixture,
    AxumErrorHandlingTraitsToolFixture, AxumExpandWithToolFixture, AxumHandlerCallToolFixture,
    AxumJsonFromBytesToolFixture, AxumParseAttrsToolFixture, AxumRequestExtractPathToolFixture,
    AxumRunUiTestsToolFixture, AxumTaskSpawnEffectToolFixture, CallGraphToolFixture,
    CallableBlockerFixture, CallableBlockerShape, CallableParamResolvedFixture,
    ChronoAliasConstructorToolFixture, ChronoNaiveUtcToolFixture, FixtureBranchReceiverToolFixture,
    FixtureDynamicCallableToolFixture, FixtureSelfFieldReceiverToolFixture,
    assert_ambiguous_dynamic_candidates, assert_ambiguous_path_candidates,
    assert_await_result_unwrap_context, assert_await_result_unwrap_proof,
    assert_body_empty_dependency_root_proof, assert_body_empty_impact_summary,
    assert_body_empty_incoming_context, assert_boxed_into_route_incoming_context,
    assert_branch_receiver_context, assert_branch_receiver_proof, assert_call_path_node,
    assert_chrono_naive_utc_incoming_context, assert_dynamic_context, assert_dynamic_proof,
    assert_expected_path_incoming_context, assert_fixture_extern_c_abs_effects,
    assert_handler_call_incoming_context, assert_incoming_context,
    assert_initialized_local_receiver_context, assert_initialized_local_receiver_proof,
    assert_json_from_bytes_incoming_context, assert_no_external_summary_need_for_site,
    assert_parse_attrs_incoming_context, assert_path_blocker_proof, assert_path_context,
    assert_path_resolution_proof, assert_resolved_callable_param_proof,
    assert_resolved_path_context, assert_run_ui_tests_incoming_context,
    assert_runtime_dispatch_blocker, assert_self_field_receiver_context,
    assert_self_field_receiver_proof, assert_serde_json_summary_proof, assert_target_proof,
    assert_task_spawn_effects, assert_task_spawn_policy_violation, assert_two_hop_call_path,
    ui_field,
};

#[tokio::test]
async fn code_item_edges_handles_trailing_module_separators() {
    // Shared fixture DB with parsed nodes/edges from tests/fixture_crates/fixture_nodes
    let db = ploke_tui::test_utils::new_test_harness::TEST_DB_NODES
        .as_ref()
        .expect("fixture db")
        .clone();

    // Build minimal AppState with focused crate pointing at the fixture crate
    let cfg = UserConfig::default();
    let runtime_cfg = RuntimeConfig::from(cfg.clone());
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        cfg.load_embedding_processor().expect("embedder"),
    ));
    let crate_root = workspace_root().join("tests/fixture_crates/fixture_nodes");
    let state = Arc::new(AppState {
        chat: ChatState::new(ChatHistory::new()),
        config: ConfigState::new(runtime_cfg),
        system: SystemState::new(SystemStatus::new(None)),
        indexing_state: RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(Mutex::new(None)),
        db: db.clone(),
        embedder,
        io_handle: IoManagerHandle::new(),
        proposals: RwLock::new(HashMap::new()),
        create_proposals: RwLock::new(HashMap::new()),
        rag: None,
        budget: TokenBudget::default(),
    });
    state
        .system
        .set_crate_focus_for_test(crate_root.clone())
        .await;

    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let ctx = Ctx {
        state: state.clone(),
        event_bus,
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        call_id: ArcStr::from("call"),
    };

    // Find a primary node that actually has edges so we can detect the regression.
    let primary_nodes = list_primary_nodes(db.as_ref()).expect("list primary nodes");
    let (focus, expected_edges) = primary_nodes
        .into_iter()
        .filter(|row| row.module_path.first().map(String::as_str) == Some("crate"))
        .find_map(|row| {
            let edges = graph_resolve_edges(
                db.as_ref(),
                &row.relation,
                row.file_path.as_path(),
                &row.module_path,
                &row.name,
            )
            .ok()?;
            if edges.is_empty() {
                return None;
            }
            Some((row, edges))
        })
        .expect("fixture db must contain at least one node with edges");

    // Capture stored vs recomputed tracking hashes before invoking the tool
    let stored_nodes = graph_resolve_exact(
        db.as_ref(),
        &focus.relation,
        focus.file_path.as_path(),
        &focus.module_path,
        &focus.name,
    )
    .expect("graph_resolve_exact");
    let stored = stored_nodes.first().expect("node present");
    let stored_file_hash = stored.file_tracking_hash;
    let actual_file_hash =
        ploke_io::read::generate_hash_for_file(stored.file_path.as_path(), stored.namespace)
            .await
            .expect("compute file hash");
    assert_eq!(
        stored_file_hash,
        actual_file_hash,
        "tracking hash mismatch for {}; DB likely stale relative to fixture contents",
        stored.file_path.display()
    );

    // Add redundant separators around the module path to mirror the regression scenario.
    let module_path_with_gaps = format!("::{}::", focus.module_path.join("::"));
    let params = EdgesParams {
        item_name: Cow::Owned(focus.name.clone()),
        file_path: Cow::Owned(focus.file_path.display().to_string()),
        node_kind: Cow::Owned(focus.relation.clone()),
        module_path: Cow::Owned(module_path_with_gaps),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, ctx)
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let returned_edges = payload
        .get("edge_info")
        .and_then(|v| v.as_array())
        .expect("edge_info array");

    assert!(
        !returned_edges.is_empty(),
        "tool should return edges even when module_path includes redundant separators"
    );
    assert_eq!(
        returned_edges.len(),
        expected_edges.len(),
        "sanitized module_path should match direct db query"
    );
}

#[tokio::test]
async fn code_item_edges_returns_edges_for_ploke_db_primary_node() {
    // Shared fixture DB with parsed nodes/edges from tests/fixture_crates/fixture_nodes.
    let db = ploke_tui::test_utils::new_test_harness::TEST_DB_NODES
        .as_ref()
        .expect("fixture db")
        .clone();

    // Minimal AppState for tool execution, focused on the fixture crate.
    let cfg = UserConfig::default();
    let runtime_cfg = RuntimeConfig::from(cfg.clone());
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        cfg.load_embedding_processor().expect("embedder"),
    ));
    let crate_root = workspace_root().join("tests/fixture_crates/fixture_nodes");
    let state = Arc::new(AppState {
        chat: ChatState::new(ChatHistory::new()),
        config: ConfigState::new(runtime_cfg),
        system: SystemState::new(SystemStatus::new(None)),
        indexing_state: RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(Mutex::new(None)),
        db: db.clone(),
        embedder,
        io_handle: IoManagerHandle::new(),
        proposals: RwLock::new(HashMap::new()),
        create_proposals: RwLock::new(HashMap::new()),
        rag: None,
        budget: TokenBudget::default(),
    });
    state
        .system
        .set_crate_focus_for_test(crate_root.clone())
        .await;

    // Find a primary node that actually has edges.
    let primary_nodes = list_primary_nodes(db.as_ref()).expect("list primary nodes");
    let (focus, expected_edges) = primary_nodes
        .into_iter()
        .filter(|row| row.module_path.first().map(String::as_str) == Some("crate"))
        .find_map(|row| {
            let edges = graph_resolve_edges(
                db.as_ref(),
                &row.relation,
                row.file_path.as_path(),
                &row.module_path,
                &row.name,
            )
            .ok()?;
            if edges.is_empty() {
                return None;
            }
            Some((row, edges))
        })
        .expect("fixture db must contain at least one primary node with edges");

    // Capture stored vs recomputed tracking hashes before invoking the tool.
    let stored_nodes = graph_resolve_exact(
        db.as_ref(),
        &focus.relation,
        focus.file_path.as_path(),
        &focus.module_path,
        &focus.name,
    )
    .expect("graph_resolve_exact");
    let stored = stored_nodes.first().expect("node present");
    let stored_file_hash = stored.file_tracking_hash;
    let actual_file_hash =
        ploke_io::read::generate_hash_for_file(stored.file_path.as_path(), stored.namespace)
            .await
            .expect("compute file hash");
    assert_eq!(
        stored_file_hash,
        actual_file_hash,
        "tracking hash mismatch for {}; DB likely stale relative to fixture contents",
        stored.file_path.display()
    );

    // Execute tool with the same coordinates the DB query used.
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let ctx = Ctx {
        state: state.clone(),
        event_bus,
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        call_id: ArcStr::from("call"),
    };
    let params = EdgesParams {
        item_name: Cow::Owned(focus.name.clone()),
        file_path: Cow::Owned(focus.file_path.display().to_string()),
        node_kind: Cow::Owned(focus.relation.clone()),
        module_path: Cow::Owned(focus.module_path.join("::")),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };
    let result = CodeItemEdges::execute(params, ctx)
        .await
        .expect("tool execution");

    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let returned_edges = payload
        .get("edge_info")
        .and_then(|v| v.as_array())
        .expect("edge_info array");

    assert!(
        !returned_edges.is_empty(),
        "code_item_edges should return edges for a fixture primary node"
    );
    assert_eq!(
        returned_edges.len(),
        expected_edges.len(),
        "tool edge count should match direct DB query for selected fixture node"
    );
}

#[tokio::test]
async fn code_item_edges_returns_recursive_cycle_paths() {
    let fixture = CallGraphToolFixture::new().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("recursive_fixture_call"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("recursive-edges-cycles"))
        .await
        .expect("recursive_fixture_call edges");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let owner = payload
        .get("node_info")
        .and_then(|node| node.get("id"))
        .and_then(serde_json::Value::as_str)
        .expect("resolved item id");
    let cycles = payload
        .get("call_cycles_from_owner")
        .and_then(serde_json::Value::as_array)
        .expect("call_cycles_from_owner array");

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // `recursive_fixture_call(depth - 1)` is a resolved direct self-call.
    // Edge lookup should expose the same DB/RAG cycle helper as exact lookup,
    // while leaving generic path traversal semantics unchanged.
    assert_eq!(cycles.len(), 1, "recursive cycle paths: {cycles:#?}");
    let cycle = &cycles[0];
    assert_eq!(
        cycle.get("start_id").and_then(serde_json::Value::as_str),
        Some(owner)
    );
    assert_eq!(
        cycle.get("end_id").and_then(serde_json::Value::as_str),
        Some(owner)
    );
    assert_eq!(
        cycle.get("depth").and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        cycle
            .get("edges")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_cycles_from_owner"), "1");
}

#[tokio::test]
async fn code_item_edges_returns_edges_for_database_struct_in_ploke_db() {
    // Shared fixture DB with parsed nodes/edges from tests/fixture_crates/fixture_nodes.
    let db = ploke_tui::test_utils::new_test_harness::TEST_DB_NODES
        .as_ref()
        .expect("fixture db")
        .clone();

    // Minimal AppState for tool execution, focused on the fixture crate.
    let cfg = UserConfig::default();
    let runtime_cfg = RuntimeConfig::from(cfg.clone());
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        cfg.load_embedding_processor().expect("embedder"),
    ));
    let crate_root = workspace_root().join("tests/fixture_crates/fixture_nodes");
    let state = Arc::new(AppState {
        chat: ChatState::new(ChatHistory::new()),
        config: ConfigState::new(runtime_cfg),
        system: SystemState::new(SystemStatus::new(None)),
        indexing_state: RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(Mutex::new(None)),
        db: db.clone(),
        embedder,
        io_handle: IoManagerHandle::new(),
        proposals: RwLock::new(HashMap::new()),
        create_proposals: RwLock::new(HashMap::new()),
        rag: None,
        budget: TokenBudget::default(),
    });
    state
        .system
        .set_crate_focus_for_test(crate_root.clone())
        .await;

    // Find a struct node that actually has edges.
    let primary_nodes = list_primary_nodes(db.as_ref()).expect("list primary nodes");
    let (focus, expected_edges) = primary_nodes
        .into_iter()
        .filter(|row| {
            row.module_path.first().map(String::as_str) == Some("crate") && row.relation == "struct"
        })
        .find_map(|row| {
            let edges = graph_resolve_edges(
                db.as_ref(),
                &row.relation,
                row.file_path.as_path(),
                &row.module_path,
                &row.name,
            )
            .ok()?;
            if edges.is_empty() {
                return None;
            }
            Some((row, edges))
        })
        .expect("fixture db should contain at least one primary struct node with edges");

    // Capture stored vs recomputed tracking hashes before invoking the tool.
    let stored_nodes = graph_resolve_exact(
        db.as_ref(),
        &focus.relation,
        focus.file_path.as_path(),
        &focus.module_path,
        &focus.name,
    )
    .expect("graph_resolve_exact");
    let stored = stored_nodes.first().expect("node present");
    let stored_file_hash = stored.file_tracking_hash;
    let actual_file_hash =
        ploke_io::read::generate_hash_for_file(stored.file_path.as_path(), stored.namespace)
            .await
            .expect("compute file hash");
    assert_eq!(
        stored_file_hash,
        actual_file_hash,
        "tracking hash mismatch for {}; DB likely stale relative to fixture contents",
        stored.file_path.display()
    );

    // Execute tool with the same coordinates the LLM used.
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let ctx = Ctx {
        state: state.clone(),
        event_bus,
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        call_id: ArcStr::from("call"),
    };
    let params = EdgesParams {
        item_name: Cow::Owned(focus.name.clone()),
        file_path: Cow::Owned(focus.file_path.display().to_string()),
        node_kind: Cow::Owned(focus.relation.clone()),
        module_path: Cow::Owned(focus.module_path.join("::")),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, ctx)
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let returned_edges = payload
        .get("edge_info")
        .and_then(|v| v.as_array())
        .expect("edge_info array");

    assert_eq!(
        returned_edges.len(),
        expected_edges.len(),
        "tool edge count should match direct DB query for selected fixture struct node"
    );
}

#[tokio::test]
async fn code_item_edges_returns_call_context_for_call_graph_item() {
    let fixture = CallGraphToolFixture::new().await;
    let ctx = fixture.ctx("call-graph-edges");
    let params = EdgesParams {
        item_name: Cow::Borrowed("call_crate_local_target"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, ctx)
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(|value| value.as_array())
        .expect("node_info.call_context array");
    let proof_context = payload
        .get("node_info")
        .and_then(|node| node.get("proof_context"))
        .and_then(|value| value.as_array())
        .expect("node_info.proof_context array");
    let owner = fixture.owner.to_string();

    assert!(
        call_context.iter().any(|call| {
            call.get("owner_id").and_then(serde_json::Value::as_str) == Some(owner.as_str())
                && call.get("kind").and_then(serde_json::Value::as_str) == Some("path")
                && call
                    .get("targets")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|targets| !targets.is_empty())
        }),
        "code_item_edges should return node-scoped call context for call_crate_local_target: {call_context:#?}"
    );
    assert!(
        proof_context.iter().any(|proof| {
            proof.get("kind").and_then(serde_json::Value::as_str) == Some("call_edge")
                && proof
                    .get("caller_def_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(owner.as_str())
        }),
        "code_item_edges should return node-scoped proof context for call_crate_local_target: {proof_context:#?}"
    );
    let call_count = call_context.len().to_string();
    let proof_count = proof_context.len().to_string();
    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context"), call_count.as_str());
    assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_edges should surface outgoing call-context count for owner lookups"
    );
}

#[tokio::test]
async fn code_item_edges_marks_unsafe_targets_in_call_impact() {
    let fixture = CallGraphToolFixture::new().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("unsafe_target"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("unsafe-target-edges"))
        .await
        .expect("unsafe_target edge lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let impact = payload
        .get("node_info")
        .and_then(|node| node.get("call_impact"))
        .and_then(serde_json::Value::as_object)
        .expect("node_info.call_impact object");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis:
    //   "Which call paths can reach `unsafe` blocks or FFI boundaries?"
    //
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:766 defines
    //   `pub unsafe fn unsafe_target()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:770 calls it from
    //   the safe wrapper `call_unsafe_function()`.
    // Expected exact-tool behavior: edge lookup on the callee exposes the same
    // target-centered unsafe metadata and direct-caller distinction as the
    // DB/RAG/lookup layers.
    let target = impact
        .get("target")
        .and_then(serde_json::Value::as_object)
        .expect("call_impact target object");
    assert_eq!(
        target.get("is_unsafe").and_then(serde_json::Value::as_bool),
        Some(true),
        "code_item_edges should serialize unsafe target metadata: {impact:#?}"
    );

    let direct_callers = impact
        .get("direct_callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact direct_callers array");
    assert!(
        direct_callers.iter().any(|caller| {
            caller.get("name").and_then(serde_json::Value::as_str) == Some("call_unsafe_function")
                && caller.get("is_unsafe").and_then(serde_json::Value::as_bool) == Some(false)
        }),
        "code_item_edges should preserve the safe direct caller without marking it unsafe: {direct_callers:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "impact_direct_callers"), "1");
}

#[tokio::test]
async fn code_item_edges_surfaces_extern_c_calls_as_external_frontier() {
    let fixture = CallGraphToolFixture::new().await;
    let expected = fixture.seed_extern_c_abs_effect();
    let params = EdgesParams {
        item_name: Cow::Borrowed("call_extern_c_function"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("extern-c-frontier-edges"))
        .await
        .expect("call_extern_c_function edge lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let reach = payload
        .get("node_info")
        .and_then(|node| node.get("call_reach"))
        .and_then(serde_json::Value::as_object)
        .expect("node_info.call_reach object");
    let effects = payload
        .get("node_info")
        .and_then(|node| node.get("call_reach_effects"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach_effects array");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis:
    //   "Which call paths can reach `unsafe` blocks or FFI boundaries?"
    //   "Which external dependency calls are made from this user-facing entrypoint?"
    //
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:839-842 declares
    //   foreign function `abs(input)` inside an `unsafe extern "C"` block.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:844 calls
    //   `abs(value)` from `call_extern_c_function`.
    // Expected exact-tool behavior: edge lookup on the owner exposes the FFI
    // call as a targetless external frontier row without fabricating a local
    // callee path.
    let paths = reach
        .get("paths")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach paths array");
    let callees = reach
        .get("callees")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach callees array");
    assert!(
        paths.is_empty() && callees.is_empty(),
        "extern C calls should not fabricate local reach paths or callees: {reach:#?}"
    );

    let external_frontier = reach
        .get("external_frontier_calls")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach external_frontier_calls array");
    let external_frontier_calls = external_frontier
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed external frontier call rows");
    let abs_call = external_frontier_calls
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.status == CallStatusKind::External
                && call.targets.is_empty()
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["abs".to_string()],
                    }
        })
        .unwrap_or_else(|| {
            panic!(
                "code_item_edges should surface targetless extern C abs frontier row: {external_frontier_calls:#?}"
            )
        });
    assert_eq!(abs_call.arg_count, Some(1));

    let source_files = reach
        .get("source_files")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach source_files array");
    assert_source_file_json(
        source_files,
        "fixture_call_graph/src/lib.rs",
        "code_item_edges extern C reach source files",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "reach_external_frontier_calls"), "1");
    assert_fixture_extern_c_abs_effects(effects, &expected, "code_item_edges call_reach_effects");
    assert_eq!(ui_field(ui, "reach_effects"), effects.len().to_string());
}

#[tokio::test]
async fn code_item_edges_returns_resolved_dynamic_callable_field_index_context() {
    // Fixture source:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:894-899
    //     `call_aliased_indexed_named_field_function_binding` constructs
    //     `CallbackArrayHolder { callbacks: [local_target] }`, aliases the
    //     holder, and calls `alias.callbacks[0]()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1555 and 1598-1606
    //     private helper parameters receive constructed holder values from a
    //     single local caller, then call `(holder.callback)()`,
    //     `holder.callbacks[0]()` / `holder.0[0]()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1635-1642
    //     a private helper receives `[local_target]` from its only local caller,
    //     then calls `funcs[0]()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1682-1684
    //     a private helper aliases a function-pointer parameter with
    //     `let g = f`, then calls `(g)()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1696-1699
    //     a boxed `dyn Fn` binding initialized with `Box::new(local_target)`
    //     is dereferenced and called as `(*boxed_fn)()`.
    // Parser/DB/RAG already prove these as exact DynamicFunction edges; this
    // pins the same field/index proof at the code_item_edges tool boundary.
    for owner_name in [
        "call_aliased_indexed_named_field_function_binding",
        "call_single_named_field_function_param",
        "call_single_indexed_function_pointer_param",
        "call_single_indexed_field_function_param",
        "call_single_indexed_tuple_field_function_param",
        "call_single_parenthesized_aliased_function_pointer_param",
        "call_dereferenced_boxed_dyn_fn_value_binding",
    ] {
        let fixture = FixtureDynamicCallableToolFixture::new_for_owner(owner_name).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(fixture.owner_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Borrowed("crate"),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("dynamic-callable-edges"))
            .await
            .expect("dynamic callable edges");
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");

        let calls = call_context
            .iter()
            .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
            .filter(|call| {
                call.owner_id == fixture.owner
                    && call.kind == CallSiteKind::Dynamic
                    && call.callee == CallCalleeInfo::Dynamic
            })
            .collect::<Vec<_>>();
        assert_eq!(
            calls.len(),
            1,
            "code_item_edges should expose exactly one resolved field/index dynamic row for {}: {call_context:#?}",
            fixture.owner_name
        );
        let call = calls[0].clone();
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1, "{call:#?}");
        assert_eq!(call.targets[0].target_id, fixture.target);
        assert_eq!(call.targets[0].relation, CallTargetKind::DynamicFunction);

        let owner = fixture.owner.to_string();
        let site = call.site_id.to_string();
        let target = fixture.target.to_string();
        let proof_rows = proof_context
            .iter()
            .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
            .collect::<Vec<_>>();
        assert!(
            proof_rows.iter().any(|proof| {
                proof.kind == "call_site"
                    && proof.caller_def_id.as_deref() == Some(owner.as_str())
                    && proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.build_domain_id.as_deref() == Some("bd:fixture-call-graph")
            }),
            "code_item_edges should return the dynamic call_site proof row for {}: {proof_context:#?}",
            fixture.owner_name
        );
        assert!(
            proof_rows.iter().any(|proof| {
                proof.kind == "call_edge"
                    && proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.caller_def_id.as_deref() == Some(owner.as_str())
                    && proof.callee_def_id.as_deref() == Some(target.as_str())
                    && proof.resolution_state.as_deref() == Some("resolved")
            }),
            "code_item_edges should return the resolved dynamic call_edge proof row for {}: {proof_context:#?}",
            fixture.owner_name
        );
        assert!(
            proof_rows.iter().any(|proof| {
                proof.kind == "call_resolution"
                    && proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.resolution_state.as_deref() == Some("resolved")
                    && proof.resolved_def_id.as_deref() == Some(target.as_str())
            }),
            "code_item_edges should return the resolved dynamic call_resolution proof row for {}: {proof_context:#?}",
            fixture.owner_name
        );

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface outgoing dynamic callable call context for {}",
            fixture.owner_name
        );
        assert!(
            ui_field(ui, "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 3,
            "code_item_edges should surface resolved dynamic callable proof rows for {}",
            fixture.owner_name
        );
    }
}

#[tokio::test]
async fn code_item_edges_returns_branch_receiver_method_context() {
    // Same fixture source and branch-path receiver proof as the lookup test:
    // the edge tool should expose the resolved method context/proof under
    // `node_info` for both if-expression and match-expression receivers.
    for owner_name in [
        "call_if_expression_receiver_method",
        "call_match_expression_receiver_method",
    ] {
        let fixture = FixtureBranchReceiverToolFixture::new_for_owner(owner_name).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(fixture.owner_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Borrowed("crate"),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("branch-receiver-edges"))
            .await
            .expect("branch receiver edges");
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");

        let call = assert_branch_receiver_context(call_context, &fixture, "code_item_edges");
        assert_branch_receiver_proof(proof_context, &fixture, call.site_id, "code_item_edges");

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface outgoing branch receiver call context for {owner_name}"
        );
        assert_eq!(
            ui_field(ui, "proof_context"),
            proof_context.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_edges_returns_branch_initialized_receiver_method_context() {
    // Same fixture source and initialized-local receiver proof as the lookup
    // test: edge lookup should expose the resolved method context/proof under
    // `node_info` for both if-initialized and match-initialized locals.
    for owner_name in [
        "call_if_initialized_local_instance_method",
        "call_match_initialized_local_instance_method",
    ] {
        let fixture = FixtureBranchReceiverToolFixture::new_for_owner(owner_name).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(fixture.owner_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Borrowed("crate"),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("branch-init-receiver-edges"))
            .await
            .expect("branch initialized receiver edges");
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");

        let call =
            assert_initialized_local_receiver_context(call_context, &fixture, "code_item_edges");
        assert_initialized_local_receiver_proof(
            proof_context,
            &fixture,
            call.site_id,
            "code_item_edges",
        );

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface outgoing initialized receiver call context for {owner_name}"
        );
        assert_eq!(
            ui_field(ui, "proof_context"),
            proof_context.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_edges_returns_nested_self_field_method_context() {
    // Same nested self-field source/proof as the lookup test, pinned at the
    // edge tool boundary under `node_info`.
    let fixture = FixtureSelfFieldReceiverToolFixture::nested_self_field().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed(fixture.owner_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed(fixture.owner_type)),
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("nested-self-field-edges"))
        .await
        .expect("nested self-field edges");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = payload
        .get("node_info")
        .and_then(|node| node.get("proof_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");

    let call = assert_self_field_receiver_context(call_context, &fixture, "code_item_edges");
    assert_self_field_receiver_proof(proof_context, &fixture, call.site_id, "code_item_edges");

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_edges should surface outgoing nested self-field call context"
    );
    assert_eq!(
        ui_field(ui, "proof_context"),
        proof_context.len().to_string()
    );
}

#[tokio::test]
async fn code_item_edges_returns_function_pointer_param_blocker() {
    for fixture in [
        CallableBlockerFixture::function_pointer_param().await,
        CallableBlockerFixture::multi_conflicting_function_pointer_param().await,
        CallableBlockerFixture::generic_fn_once_value_binding().await,
        CallableBlockerFixture::multi_conflicting_generic_fn_once_param().await,
        CallableBlockerFixture::multi_conflicting_named_field_function_param().await,
        CallableBlockerFixture::field_function_param().await,
        CallableBlockerFixture::indexed_function_pointer().await,
        CallableBlockerFixture::indexed_field_function_param().await,
        CallableBlockerFixture::indexed_tuple_field_function_param().await,
    ] {
        let params = EdgesParams {
            item_name: Cow::Borrowed(fixture.owner_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Borrowed("crate"),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("fn-pointer-param-edges"))
            .await
            .expect("function pointer param edges");
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");

        // Source oracle:
        //   tests/fixture_crates/fixture_call_graph/src/lib.rs:
        //     public `call_function_pointer_param(f)` calls `f()`;
        //     private `call_multi_conflicting_function_pointer_param(f)` also
        //     calls `f()`, but its local callers pass different functions;
        //     public `call_generic_fn_once_value_binding(generic_f)` calls
        //     `generic_f()`;
        //     private `call_multi_conflicting_generic_fn_once_param(generic_f)`
        //     also calls `generic_f()`, but its local callers pass different
        //     functions;
        //     private `call_multi_conflicting_named_field_function_param(holder)`
        //     calls `(holder.callback)()`, but its local callers pass different
        //     functions in that field;
        //     public `call_field_function_param(holder)` calls
        //     `(holder.callback)()`;
        //     public `call_indexed_function_pointer(funcs)` calls `funcs[0]()`;
        //     public `call_indexed_field_function_param(holder)` calls
        //     `holder.callbacks[0]()`;
        //     public `call_indexed_tuple_field_function_param(holder)` calls
        //     `holder.0[0]()`.
        //
        // Public opaque parameters stay blocked and targetless. Private
        // complete local caller sets with conflicting callable arguments expose
        // candidate targets, but still do not fabricate a resolved call edge.
        let label = format!("{} callable parameter call", fixture.owner_name);
        let callee = CallCalleeInfo::Path {
            path: fixture.path.clone(),
        };
        let site_id = match fixture.shape {
            CallableBlockerShape::Path => assert_path_context(
                call_context,
                fixture.owner,
                &callee,
                &CallStatusKind::Unsupported,
                label.as_str(),
                "code_item_edges",
            ),
            CallableBlockerShape::Dynamic => assert_dynamic_context(
                call_context,
                fixture.owner,
                None,
                label.as_str(),
                "code_item_edges",
            ),
            CallableBlockerShape::AmbiguousPath => assert_ambiguous_path_candidates(
                call_context,
                fixture.owner,
                &callee,
                &fixture.candidates,
                label.as_str(),
                "code_item_edges",
            ),
            CallableBlockerShape::AmbiguousDynamic => assert_ambiguous_dynamic_candidates(
                call_context,
                fixture.owner,
                &fixture.candidates,
                label.as_str(),
                "code_item_edges",
            ),
        };
        match fixture.shape {
            CallableBlockerShape::Path => assert_path_resolution_proof(
                proof_context,
                fixture.owner,
                site_id,
                fixture.build_domain,
                "blocked",
                "type_resolution_missing",
                label.as_str(),
                "code_item_edges",
            ),
            CallableBlockerShape::Dynamic => assert_dynamic_proof(
                proof_context,
                fixture.owner,
                site_id,
                fixture.build_domain,
                label.as_str(),
                "code_item_edges",
            ),
            CallableBlockerShape::AmbiguousPath | CallableBlockerShape::AmbiguousDynamic => {
                assert_path_resolution_proof(
                    proof_context,
                    fixture.owner,
                    site_id,
                    fixture.build_domain,
                    "ambiguous",
                    "type_resolution_missing",
                    label.as_str(),
                    "code_item_edges",
                );
            }
        }
        if matches!(
            fixture.shape,
            CallableBlockerShape::AmbiguousPath | CallableBlockerShape::AmbiguousDynamic
        ) {
            assert_eq!(
                summary_usize(&payload, "blocked"),
                0,
                "code_item_edges summary should not count candidate rows as targetless blockers: {payload:#?}"
            );
        } else {
            assert!(
                summary_usize(&payload, "blocked") >= 1,
                "code_item_edges summary should count the targetless callable parameter row: {payload:#?}"
            );
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface the callable parameter call"
        );
        assert_eq!(
            ui_field(ui, "proof_context"),
            proof_context.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_edges_returns_non_awaited_async_closure_poll_resume_blockers() {
    let fixture = CallGraphToolFixture::new().await;

    for (owner_name, label) in [
        (
            "call_async_closure_binding_without_await_with_body_call",
            "non-awaited async closure binding",
        ),
        (
            "call_async_closure_future_binding_without_await_with_body_call",
            "unawaited async closure future binding",
        ),
    ] {
        let expected = fixture.async_closure_blocker(owner_name);
        let params = EdgesParams {
            item_name: Cow::Borrowed(owner_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Borrowed("crate"),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("async-closure-blocker-edges"))
            .await
            .expect("async closure poll/resume blocker edges");
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");

        // Fixture source:
        //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1701-1714
        //   calls an async closure without awaiting the returned future. The
        //   edge tool must preserve the targetless row plus its derived async
        //   poll/resume blocker without fabricating traversal.
        let callee = CallCalleeInfo::Path {
            path: expected.path.clone(),
        };
        let site_id = assert_path_context(
            call_context,
            expected.owner,
            &callee,
            &CallStatusKind::Unsupported,
            label,
            "code_item_edges",
        );
        assert_eq!(site_id, expected.site);
        assert_path_blocker_proof(
            proof_context,
            expected.owner,
            site_id,
            "bd:fixture-call-graph",
            "type_resolution_missing",
            label,
            "code_item_edges",
        );
        assert_runtime_dispatch_blocker(proof_context, site_id, label, "code_item_edges");
        assert!(
            summary_usize(&payload, "blocked") >= 1,
            "code_item_edges summary should count the targetless async closure row: {payload:#?}"
        );

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface the targetless async closure call"
        );
        assert_eq!(
            ui_field(ui, "proof_context"),
            proof_context.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_edges_returns_multi_caller_function_pointer_param_target() {
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1740-1742
    //     private `call_multi_function_pointer_param(f)` calls `f()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1744-1749
    //     both local callers pass `local_target`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1764-1768
    //     private `call_multi_generic_fn_once_param(generic_f)` calls
    //     `generic_f()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1771-1776
    //     both local generic callers pass `local_target`.
    for fixture in [
        CallableParamResolvedFixture::multi_function_pointer_param().await,
        CallableParamResolvedFixture::multi_generic_fn_once_param().await,
    ] {
        let params = EdgesParams {
            item_name: Cow::Borrowed(fixture.owner_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Borrowed("crate"),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("multi-callable-param-edges"))
            .await
            .unwrap_or_else(|err| panic!("{} edges: {err}", fixture.owner_name));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");

        // Edge payloads should preserve the same resolved Function edge that
        // DB/RAG expose for complete private same-target caller proof.
        let callee = CallCalleeInfo::Path {
            path: fixture.path.clone(),
        };
        let site_id = assert_resolved_path_context(
            call_context,
            fixture.owner,
            &callee,
            fixture.target,
            CallTargetKind::Function,
            "multi-caller callable parameter",
            "code_item_edges",
        );
        assert_resolved_callable_param_proof(
            proof_context,
            fixture.owner,
            fixture.target,
            site_id,
            fixture.build_domain,
            "code_item_edges",
        );

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface the resolved multi-caller callable parameter row"
        );
        assert_eq!(
            ui_field(ui, "proof_context"),
            proof_context.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_reachable_effects() {
    let fixture = AxumTaskSpawnEffectToolFixture::new().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("deserialize_error_status_codes"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: vec![Cow::Borrowed("ffi_boundary")],
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("axum-task-spawn-effect-edges"))
        .await
        .expect("deserialize_error_status_codes edge lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let node_info = payload
        .get("node_info")
        .and_then(serde_json::Value::as_object)
        .expect("node_info object");
    let effects = node_info
        .get("call_reach_effects")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach_effects array");
    let policy_violations = node_info
        .get("call_effect_policy_violations")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_effect_policy_violations array");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Can this entrypoint reach a sensitive sink?"
    //   "Is the reachable sink outside the caller's explicit effect policy?"
    //   "Which call chain reaches a task-spawn point?"
    //
    // Source-oracle chain:
    //   axum/src/form.rs:262
    //     `deserialize_error_status_codes` calls `TestClient::new(app)`.
    //   axum/src/test_helpers/test_client.rs:36
    //     `TestClient::new` calls `spawn_service(svc)`.
    //   axum/src/test_helpers/test_client.rs:23
    //     `spawn_service` calls `tokio::spawn(...)`.
    assert_task_spawn_effects(effects, &fixture, "code_item_edges call_reach_effects");
    assert_task_spawn_policy_violation(
        policy_violations,
        &fixture,
        "code_item_edges call_effect_policy_violations",
    );
    let owner = fixture.owner.to_string();
    assert_eq!(
        node_info.get("id").and_then(serde_json::Value::as_str),
        Some(owner.as_str()),
        "code_item_edges should resolve the upstream axum test owner: {payload:#?}"
    );
    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "reach_effects"), effects.len().to_string());
    assert_eq!(
        ui_field(ui, "effect_policy_violations"),
        policy_violations.len().to_string()
    );
}

#[tokio::test]
async fn code_item_edges_reports_private_target_without_incoming_callers() {
    let fixture = AxumErrorHandlingTraitsToolFixture::new().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("traits"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("axum-traits-zero-impact-edges"))
        .await
        .expect("error_handling::traits edge lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Dead code detection:
    //   "Which private helpers have no incoming callers?"
    //   "Is this function reachable from any binary, test, macro entrypoint,
    //   or exported API?"
    //
    // Source oracle:
    //   axum/src/error_handling/mod.rs:257 defines `#[test] fn traits()`.
    //   No checked-in axum source row calls `traits(...)`; generated test
    //   harness entrypoints are represented as proof context, not source edges.
    let incoming_paths = payload
        .get("call_paths_to_target")
        .and_then(serde_json::Value::as_array)
        .expect("call_paths_to_target array");
    assert!(
        incoming_paths.is_empty(),
        "code_item_edges should expose zero incoming call paths: {incoming_paths:#?}"
    );

    let node_info = payload
        .get("node_info")
        .and_then(serde_json::Value::as_object)
        .expect("node_info object");
    let target_id = fixture.target.to_string();
    assert_eq!(
        node_info.get("id").and_then(serde_json::Value::as_str),
        Some(target_id.as_str()),
        "code_item_edges should resolve error_handling::traits: {payload:#?}"
    );

    let impact = node_info
        .get("call_impact")
        .and_then(serde_json::Value::as_object)
        .expect("node_info.call_impact object");
    for field in [
        "paths",
        "callers",
        "direct_callers",
        "direct_call_sites",
        "callsite_buckets",
        "public_callers",
        "test_callers",
        "non_test_callers",
    ] {
        let rows = impact
            .get(field)
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("call_impact {field} array: {impact:#?}"));
        assert!(
            rows.is_empty(),
            "code_item_edges zero-caller impact {field} should be empty: {rows:#?}"
        );
    }

    let proof_context = node_info
        .get("proof_context")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");
    let proof_rows = proof_context
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        proof_rows.iter().any(|proof| {
            proof.kind == "entrypoint_summary"
                && proof.definition_id.as_deref() == Some(target_id.as_str())
                && proof.target_kind.as_deref() == Some("test")
                && proof.target_name.as_deref() == Some("generated-test-harness")
                && proof.summary_class.as_deref() == Some("analyzed_source")
                && proof.status.as_deref() == Some("admitted")
        }),
        "code_item_edges should expose the generated test-harness entrypoint proof summary without source callers: {proof_context:#?}"
    );
    let build_domains = node_info
        .get("call_build_domains")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_build_domains array");
    assert_eq!(
        build_domains.len(),
        1,
        "code_item_edges should expose one generated-test build domain: {build_domains:#?}"
    );
    let domain = build_domains[0]
        .as_object()
        .expect("node_info.call_build_domains object");
    assert_eq!(
        domain
            .get("build_domain_id")
            .and_then(serde_json::Value::as_str),
        Some("bd:corpus-axum-call-graph")
    );
    assert_eq!(
        domain
            .get("target_kind")
            .and_then(serde_json::Value::as_str),
        Some("library")
    );
    assert_eq!(
        domain
            .get("target_name")
            .and_then(serde_json::Value::as_str),
        Some("axum")
    );
    assert_eq!(
        domain
            .get("target_root")
            .and_then(serde_json::Value::as_str),
        Some("axum/src/lib.rs")
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(
        ui_field(ui, "proof_context"),
        proof_context.len().to_string()
    );
    assert_eq!(
        ui_field(ui, "call_build_domains"),
        build_domains.len().to_string()
    );
    assert_eq!(ui_field(ui, "call_paths_to_target"), "0");
    assert_eq!(ui_field(ui, "impact_callers"), "0");
    assert_eq!(ui_field(ui, "impact_direct_callers"), "0");
    assert_eq!(ui_field(ui, "impact_direct_call_sites"), "0");
    assert_eq!(ui_field(ui, "impact_public_callers"), "0");
    assert_eq!(ui_field(ui, "impact_test_callers"), "0");
    assert_eq!(ui_field(ui, "impact_non_test_callers"), "0");
}

#[tokio::test]
async fn code_item_edges_surfaces_proc_macro_impact_callers() {
    let fixture = AxumExpandWithToolFixture::new().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("expand_with"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("axum-expand-with-impact-edges"))
        .await
        .expect("expand_with edges");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let node_info = payload.get("node_info").expect("node_info");
    let impact = node_info
        .get("call_impact")
        .and_then(serde_json::Value::as_object)
        .expect("node_info.call_impact object");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Dead code detection:
    //   "Is this function reachable from any binary, test, macro entrypoint,
    //   or exported API?"
    //
    // Source oracle:
    //   axum-macros/src/lib.rs:377,426,665,715 call `expand_with(...)` from
    //   public proc-macro entrypoints.
    // Current contract: `code_item_edges` should expose the same target-
    // centered impact summary as `code_item_lookup`, preserving the four
    // one-hop public macro callers and their direct path callsite rows.
    let target_id = fixture.target.to_string();
    assert_eq!(
        impact
            .get("target")
            .and_then(|target| target.get("id"))
            .and_then(serde_json::Value::as_str),
        Some(target_id.as_str())
    );
    let expected_callers = [
        "derive_from_request",
        "derive_from_request_parts",
        "derive_typed_path",
        "derive_from_ref",
    ];
    for field in ["paths", "callers", "direct_callers", "direct_call_sites"] {
        let rows = impact
            .get(field)
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("call_impact {field} array: {impact:#?}"));
        assert_eq!(
            rows.len(),
            expected_callers.len(),
            "expand_with edge impact {field} should expose one row per proc-macro caller: {rows:#?}"
        );
    }
    let callers = impact
        .get("callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact callers array");
    let public_callers = impact
        .get("public_callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact public_callers array");
    assert_eq!(
        public_callers.len(),
        expected_callers.len(),
        "expand_with edge impact public_callers should expose public proc-macro entrypoints: {public_callers:#?}"
    );
    for name in expected_callers {
        assert!(
            callers.iter().any(|caller| {
                caller.get("name").and_then(serde_json::Value::as_str) == Some(name)
            }),
            "expand_with edge impact callers should include {name}: {callers:#?}"
        );
        assert!(
            public_callers.iter().any(|caller| {
                caller.get("name").and_then(serde_json::Value::as_str) == Some(name)
            }),
            "expand_with edge impact public_callers should include {name}: {public_callers:#?}"
        );
    }
    let direct_call_sites = impact
        .get("direct_call_sites")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact direct_call_sites array")
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed expand_with direct callsite rows");
    for call in direct_call_sites {
        assert!(
            matches!(&call.callee, CallCalleeInfo::Path { path } if path == &vec!["expand_with".to_string()]),
            "expand_with direct callsite should preserve the path callee: {call:#?}"
        );
        assert_eq!(
            call.arg_count,
            Some(2),
            "expand_with direct callsite should preserve arity: {call:#?}"
        );
        assert!(
            call.targets.iter().any(|target| {
                target.target_id.to_string() == target_id
                    && target.relation == CallTargetKind::Function
            }),
            "expand_with direct callsite should target the looked-up function: {call:#?}"
        );
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "impact_callers"), "4");
    assert_eq!(ui_field(ui, "impact_direct_callers"), "4");
    assert_eq!(ui_field(ui, "impact_direct_call_sites"), "4");
    assert_eq!(ui_field(ui, "impact_public_callers"), "4");
    assert_eq!(
        ui_field(ui, "impact_source_cfgs"),
        impact
            .get("source_cfgs")
            .and_then(serde_json::Value::as_array)
            .expect("call_impact source_cfgs array")
            .len()
            .to_string()
    );
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_await_receiver_targetless_row() {
    let fixture = AxumAwaitReceiverToolFixture::new().await;
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed("accept"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed("ConnLimiter")),
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("axum-await-edges"))
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = payload
        .get("node_info")
        .and_then(|node| node.get("proof_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum/src/serve/listener.rs:142 owns `ConnLimiter<T>::accept`.
    //   axum/src/serve/listener.rs:143 calls
    //   `self.sem.clone().acquire_owned().await.unwrap()`.
    // The edge-oriented exact tool should expose the same unsupported,
    // targetless AwaitResult receiver row as lookup, without a fabricated edge.
    let site_id =
        assert_await_result_unwrap_context(call_context, fixture.owner, "code_item_edges");
    assert_await_result_unwrap_proof(proof_context, fixture.owner, site_id, "code_item_edges");
    assert!(
        summary_usize(&payload, "blocked") >= 1,
        "code_item_edges summary should report the unsupported targetless await receiver row: {payload:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_edges should surface outgoing targetless call-context count"
    );
    assert!(
        ui_field(ui, "blocked_calls")
            .parse::<usize>()
            .expect("blocked call count")
            >= 1,
        "code_item_edges should surface blocked callsite count in the UI payload"
    );
    let proof_count = proof_context.len().to_string();
    assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_two_hop_call_paths() {
    let fixture = AxumRequestExtractPathToolFixture::new().await;
    let start_params = EdgesParams {
        item_name: Cow::Borrowed("extract"),
        file_path: Cow::Owned(fixture.start_file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(fixture.start_module_path_arg()),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed("Request")),
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let start_result = CodeItemEdges::execute(
        start_params,
        fixture.ctx("axum-request-extract-start-paths"),
    )
    .await
    .expect("RequestExt::extract edge lookup");
    let start_payload: serde_json::Value =
        serde_json::from_str(&start_result.content).expect("deserialize start NodeEdgeInfo");
    let start_id = fixture.start.to_string();
    assert_eq!(
        start_payload
            .get("node_info")
            .and_then(|node| node.get("id"))
            .and_then(serde_json::Value::as_str),
        Some(start_id.as_str()),
        "code_item_edges should resolve the same RequestExt::extract method used by the path oracle: {start_payload:#?}"
    );
    let outgoing_paths = start_payload
        .get("call_paths_from_owner")
        .and_then(serde_json::Value::as_array)
        .expect("call_paths_from_owner array");
    let start_path_nodes = start_payload
        .get("call_path_nodes")
        .and_then(serde_json::Value::as_array)
        .expect("call_path_nodes array");
    let start_reach = start_payload
        .get("node_info")
        .and_then(|node| node.get("call_reach"))
        .expect("node_info.call_reach should be present for source edge lookup");
    let start_reach_paths = start_reach
        .get("paths")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach.paths array");
    let start_reach_callees = start_reach
        .get("callees")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach.callees array");
    let start_reach_direct_call_sites = start_reach
        .get("direct_call_sites")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach.direct_call_sites array");
    let start_reach_boundary_edges = start_reach
        .get("boundary_edges")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach.boundary_edges array");
    let start_reach_source_files = start_reach
        .get("source_files")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach.source_files array");
    let start_reach_source_crates = start_reach
        .get("source_crates")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach.source_crates array");
    let start_reach_source_modules = start_reach
        .get("source_modules")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach.source_modules array");

    // Matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    // The graph-oriented exact tool should expose the same ordered multi-hop
    // path as DB and RAG traversal, so a tool caller can answer call-chain
    // questions without manually joining one-hop call context rows.
    assert_two_hop_call_path(
        outgoing_paths,
        fixture.start,
        fixture.intermediate,
        fixture.target,
        "code_item_edges outgoing paths",
    );
    assert_two_hop_call_path(
        start_reach_paths,
        fixture.start,
        fixture.intermediate,
        fixture.target,
        "code_item_edges node_info.call_reach paths",
    );
    assert_call_path_node(
        start_path_nodes,
        fixture.start,
        "::extract",
        "axum-core/src/ext_traits/request.rs",
        "code_item_edges outgoing paths",
    );
    assert_call_path_node(
        start_path_nodes,
        fixture.intermediate,
        "::extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "code_item_edges outgoing paths",
    );
    assert_call_path_node(
        start_path_nodes,
        fixture.target,
        "::from_request",
        "axum-core/src/extract/mod.rs",
        "code_item_edges outgoing paths",
    );
    assert_eq!(
        start_reach_direct_call_sites.len(),
        1,
        "code_item_edges reach should expose the exact direct callsite made by RequestExt::extract: {start_reach_direct_call_sites:#?}"
    );
    assert_eq!(
        start_reach_boundary_edges.len(),
        1,
        "code_item_edges reach should expose the transitive cross-module FromRequest edge: {start_reach_boundary_edges:#?}"
    );
    let boundary_edge = &start_reach_boundary_edges[0];
    let intermediate_id = fixture.intermediate.to_string();
    let target_id = fixture.target.to_string();
    assert_eq!(
        boundary_edge
            .get("caller_id")
            .and_then(serde_json::Value::as_str),
        Some(intermediate_id.as_str()),
        "code_item_edges reach boundary edge should start at extract_with_state: {boundary_edge:#?}"
    );
    assert_eq!(
        boundary_edge
            .get("callee_id")
            .and_then(serde_json::Value::as_str),
        Some(target_id.as_str()),
        "code_item_edges reach boundary edge should target FromRequest::from_request: {boundary_edge:#?}"
    );
    assert_source_file_json(
        start_reach_source_files,
        "axum-core/src/ext_traits/request.rs",
        "code_item_edges reach source files",
    );
    assert_source_file_json(
        start_reach_source_files,
        "axum-core/src/extract/mod.rs",
        "code_item_edges reach source files",
    );
    assert_source_crate_json(
        start_reach_source_crates,
        "axum-core",
        "code_item_edges reach source crates",
    );
    assert_source_module_json(
        start_reach_source_modules,
        &["crate", "ext_traits", "request"],
        "code_item_edges reach source modules",
    );
    assert_source_module_json(
        start_reach_source_modules,
        &["crate", "extract"],
        "code_item_edges reach source modules",
    );
    assert!(
        summary_usize(&start_payload, "calls") >= 1,
        "RequestExt::extract summary should report outgoing direct callsites: {start_payload:#?}"
    );
    assert!(
        summary_usize(&start_payload, "callees") >= 1,
        "RequestExt::extract summary should report resolved direct callees: {start_payload:#?}"
    );
    assert!(
        summary_usize(&start_payload, "outgoing_paths") >= 2,
        "RequestExt::extract summary should report bounded outgoing call paths: {start_payload:#?}"
    );
    assert!(
        summary_usize(&start_payload, "outgoing_depth") >= 2,
        "RequestExt::extract summary should report the two-hop axum path depth: {start_payload:#?}"
    );
    assert!(
        summary_usize(&start_payload, "path_nodes") >= 3,
        "RequestExt::extract summary should report source-labeled path nodes: {start_payload:#?}"
    );
    let start_ui = start_result.ui_payload.as_ref().expect("start ui payload");
    assert!(
        ui_field(start_ui, "call_paths_from_owner")
            .parse::<usize>()
            .expect("outgoing path count")
            >= 2,
        "code_item_edges should surface outgoing path count for RequestExt::extract"
    );
    assert_eq!(
        ui_field(start_ui, "callees"),
        summary_usize(&start_payload, "callees").to_string()
    );
    assert_eq!(
        ui_field(start_ui, "reach_callees"),
        start_reach_callees.len().to_string()
    );
    assert_eq!(
        ui_field(start_ui, "reach_direct_call_sites"),
        start_reach_direct_call_sites.len().to_string()
    );
    assert_eq!(
        ui_field(start_ui, "reach_boundary_edges"),
        start_reach_boundary_edges.len().to_string()
    );
    assert_eq!(
        ui_field(start_ui, "reach_source_files"),
        start_reach_source_files.len().to_string()
    );
    assert_eq!(
        ui_field(start_ui, "reach_source_crates"),
        start_reach_source_crates.len().to_string()
    );
    assert_eq!(
        ui_field(start_ui, "reach_source_cfgs"),
        start_reach
            .get("source_cfgs")
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_reach.source_cfgs array")
            .len()
            .to_string()
    );
    assert_eq!(
        ui_field(start_ui, "reach_source_modules"),
        start_reach_source_modules.len().to_string()
    );

    let target_params = EdgesParams {
        item_name: Cow::Borrowed("from_request"),
        file_path: Cow::Owned(fixture.target_file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(fixture.target_module_path_arg()),
        owner_trait: Some(Cow::Borrowed("FromRequest")),
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };
    let target_result = CodeItemEdges::execute(
        target_params,
        fixture.ctx("axum-request-extract-target-paths"),
    )
    .await
    .expect("FromRequest::from_request edge lookup");
    let target_payload: serde_json::Value =
        serde_json::from_str(&target_result.content).expect("deserialize target NodeEdgeInfo");
    let target_id = fixture.target.to_string();
    assert_eq!(
        target_payload
            .get("node_info")
            .and_then(|node| node.get("id"))
            .and_then(serde_json::Value::as_str),
        Some(target_id.as_str()),
        "code_item_edges should resolve the same FromRequest::from_request method used by the path oracle: {target_payload:#?}"
    );
    let incoming_paths = target_payload
        .get("call_paths_to_target")
        .and_then(serde_json::Value::as_array)
        .expect("call_paths_to_target array");
    let target_path_nodes = target_payload
        .get("call_path_nodes")
        .and_then(serde_json::Value::as_array)
        .expect("call_path_nodes array");
    let target_impact = target_payload
        .get("node_info")
        .and_then(|node| node.get("call_impact"))
        .expect("node_info.call_impact should be present for target edge lookup");
    let target_impact_paths = target_impact
        .get("paths")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_impact.paths array");
    let target_impact_callers = target_impact
        .get("callers")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_impact.callers array");
    let target_impact_direct_call_sites = target_impact
        .get("direct_call_sites")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_impact.direct_call_sites array");
    let target_impact_callsite_buckets = target_impact
        .get("callsite_buckets")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_impact.callsite_buckets array");
    let target_impact_public_callers = target_impact
        .get("public_callers")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_impact.public_callers array");
    let target_impact_source_files = target_impact
        .get("source_files")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_impact.source_files array");
    let target_impact_source_modules = target_impact
        .get("source_modules")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_impact.source_modules array");
    assert_two_hop_call_path(
        incoming_paths,
        fixture.start,
        fixture.intermediate,
        fixture.target,
        "code_item_edges incoming paths",
    );
    assert_two_hop_call_path(
        target_impact_paths,
        fixture.start,
        fixture.intermediate,
        fixture.target,
        "code_item_edges node_info.call_impact paths",
    );
    assert_call_path_node(
        target_path_nodes,
        fixture.start,
        "::extract",
        "axum-core/src/ext_traits/request.rs",
        "code_item_edges incoming paths",
    );
    assert_call_path_node(
        target_path_nodes,
        fixture.intermediate,
        "::extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "code_item_edges incoming paths",
    );
    assert_call_path_node(
        target_path_nodes,
        fixture.target,
        "::from_request",
        "axum-core/src/extract/mod.rs",
        "code_item_edges incoming paths",
    );
    assert_eq!(
        target_impact_direct_call_sites.len(),
        2,
        "code_item_edges impact should expose both direct FromRequest::from_request callsite rows: {target_impact_direct_call_sites:#?}"
    );
    assert!(
        target_impact_callsite_buckets.iter().any(|bucket| {
            bucket.get("kind").and_then(serde_json::Value::as_str) == Some("path")
                && bucket.get("relation").and_then(serde_json::Value::as_str)
                    == Some("associated_function")
                && bucket.get("count").and_then(serde_json::Value::as_u64) == Some(2)
        }),
        "code_item_edges impact should summarize direct path/associated-function callsites: {target_impact_callsite_buckets:#?}"
    );
    assert!(
        target_impact_public_callers.is_empty(),
        "code_item_edges impact should preserve the DB-owned public caller bucket without inventing trait-effective visibility: {target_impact_public_callers:#?}"
    );
    assert_source_file_json(
        target_impact_source_files,
        "axum-core/src/ext_traits/request.rs",
        "code_item_edges impact source files",
    );
    assert_source_file_json(
        target_impact_source_files,
        "axum-core/src/extract/mod.rs",
        "code_item_edges impact source files",
    );
    assert_source_module_json(
        target_impact_source_modules,
        &["crate", "ext_traits", "request"],
        "code_item_edges impact source modules",
    );
    assert_source_module_json(
        target_impact_source_modules,
        &["crate", "extract"],
        "code_item_edges impact source modules",
    );
    assert!(
        summary_usize(&target_payload, "callers") >= 1,
        "FromRequest::from_request summary should report incoming callers: {target_payload:#?}"
    );
    assert!(
        summary_usize(&target_payload, "incoming_paths") >= 2,
        "FromRequest::from_request summary should report bounded incoming paths: {target_payload:#?}"
    );
    assert!(
        summary_usize(&target_payload, "incoming_depth") >= 2,
        "FromRequest::from_request summary should report the two-hop axum path depth: {target_payload:#?}"
    );
    assert!(
        summary_usize(&target_payload, "path_nodes") >= 3,
        "FromRequest::from_request summary should report source-labeled path nodes: {target_payload:#?}"
    );
    let target_ui = target_result
        .ui_payload
        .as_ref()
        .expect("target ui payload");
    assert!(
        ui_field(target_ui, "call_paths_to_target")
            .parse::<usize>()
            .expect("incoming path count")
            >= 2,
        "code_item_edges should surface incoming path count for FromRequest::from_request"
    );
    assert_eq!(
        ui_field(target_ui, "callers"),
        summary_usize(&target_payload, "callers").to_string()
    );
    assert_eq!(
        ui_field(target_ui, "impact_callers"),
        target_impact_callers.len().to_string()
    );
    assert_eq!(
        ui_field(target_ui, "impact_direct_call_sites"),
        target_impact_direct_call_sites.len().to_string()
    );
    assert_eq!(
        ui_field(target_ui, "impact_callsite_buckets"),
        target_impact_callsite_buckets.len().to_string()
    );
    assert_eq!(
        ui_field(target_ui, "impact_public_callers"),
        target_impact_public_callers.len().to_string()
    );
    assert_eq!(
        ui_field(target_ui, "impact_source_files"),
        target_impact_source_files.len().to_string()
    );
    assert_eq!(
        ui_field(target_ui, "impact_source_modules"),
        target_impact_source_modules.len().to_string()
    );
}

#[tokio::test]
async fn code_item_edges_returns_incoming_callers_for_call_graph_target() {
    let fixture = CallGraphToolFixture::new().await;
    let ctx = fixture.ctx("call-graph-target-edges");
    let params = EdgesParams {
        item_name: Cow::Borrowed("local_target"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, ctx)
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = payload
        .get("node_info")
        .and_then(|node| node.get("proof_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");

    assert_incoming_context(
        call_context,
        fixture.owner,
        fixture.target,
        "code_item_edges",
    );
    assert_target_proof(
        proof_context,
        fixture.owner,
        fixture.target,
        "code_item_edges",
    );

    let call_count = call_context.len().to_string();
    let proof_count = proof_context.len().to_string();
    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context"), call_count.as_str());
    assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
    assert!(
        ui_field(ui, "call_context_incoming")
            .parse::<usize>()
            .expect("incoming count")
            >= 1,
        "code_item_edges should surface incoming caller count for target lookups"
    );
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_body_empty_callers() {
    let fixture = AxumBodyEmptyToolFixture::new().await;
    let ctx = fixture.ctx("axum-body-empty-edges");
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed("empty"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, ctx)
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = payload
        .get("node_info")
        .and_then(|node| node.get("proof_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");
    let impact = payload
        .get("node_info")
        .and_then(|node| node.get("call_impact"))
        .and_then(serde_json::Value::as_object)
        .expect("node_info.call_impact object");
    let impact_source_crates = impact
        .get("source_crates")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_impact source_crates array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core/src/body.rs:110 and :116 call `Self::empty()`.
    //   axum-core/src/response/into_response.rs response conversion rows call
    //   `Body::empty()`.
    //   Four axum direct parsed-workspace import rows and eleven local
    //   re-exported, inherited, closure, and local-item workspace rows also call
    //   `Body::empty()`.
    // Expected tool traversal: exact edge lookup of the callee method exposes
    // the same incoming caller-site edges and projected proof rows as the DB
    // target-centered query.
    assert_body_empty_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_edges",
    );
    assert_body_empty_impact_summary(impact, "code_item_edges");
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_edges",
        );
    }
    assert_body_empty_dependency_root_proof(
        proof_context,
        &fixture.dependency_root_sites,
        fixture.target,
        "code_item_edges",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "23");
    assert_eq!(
        ui_field(ui, "impact_source_crates"),
        impact_source_crates.len().to_string()
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_edges should surface real-corpus Body::empty proof rows"
    );
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_parse_attrs_callers() {
    let fixture = AxumParseAttrsToolFixture::new().await;
    let ctx = fixture.ctx("axum-parse-attrs-edges");
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed("parse_attrs"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, ctx)
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = payload
        .get("node_info")
        .and_then(|node| node.get("proof_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-macros/src/attr_parsing.rs:59 defines `parse_attrs`.
    //   axum-macros/src/typed_path.rs:23 calls
    //   `crate::attr_parsing::parse_attrs(...)`.
    //   from_ref.rs:30 and from_request/mod.rs:{112,196,598,727,892,908}
    //   call imported `parse_attrs(...)`.
    // Expected tool traversal: exact edge lookup of the callee function exposes
    // all eight incoming caller-site edges and projected proof rows.
    assert_parse_attrs_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_edges",
    );
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_edges",
        );
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(
        ui_field(ui, "call_context_incoming"),
        fixture.callers.len().to_string().as_str()
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_edges should surface real-corpus parse_attrs proof rows"
    );
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_json_from_bytes_callers() {
    let fixture = AxumJsonFromBytesToolFixture::new().await;
    let serde_site = fixture.admit_serde_summary();
    let ctx = fixture.ctx("axum-json-from-bytes-edges");
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed("from_bytes"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, ctx)
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = payload
        .get("node_info")
        .and_then(|node| node.get("proof_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");
    let summary_needs = payload
        .get("node_info")
        .and_then(|node| node.get("external_summary_needs"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.external_summary_needs array");
    let reach = payload
        .get("node_info")
        .and_then(|node| node.get("call_reach"))
        .and_then(serde_json::Value::as_object)
        .expect("node_info.call_reach object");
    let reach_source_cfgs = reach
        .get("source_cfgs")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach source_cfgs array");
    let external_frontier = reach
        .get("external_frontier_calls")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach external_frontier_calls array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:112 and :128 call `Self::from_bytes(&bytes)`.
    //   axum/src/json.rs:184 calls
    //     `serde_json::Deserializer::from_slice(bytes)`.
    //   axum/src/lib.rs:488-489 gates the file module with
    //     `#[cfg(feature = "json")] mod json;`.
    // Expected tool traversal: exact edge lookup of the callee method exposes
    // the same two trait-impl caller-site edges and projected proof rows as
    // the DB target-centered query. Its owner reach summary also exposes the
    // serde_json dependency-root call as a targetless external frontier. The
    // fixture admits an audited summary before tool execution, so the payload
    // should expose the externally-summarized proof and no longer queue this
    // site as an active missing-summary need, while preserving the inherited
    // feature gate.
    assert_json_from_bytes_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_edges",
    );
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_edges",
        );
    }
    let external_frontier_calls = external_frontier
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed external frontier call rows");
    let serde_frontier = external_frontier_calls
        .iter()
        .find(|call| {
            call.owner_id == fixture.target
                && call.kind == CallSiteKind::Path
                && call.status == CallStatusKind::External
                && call.targets.is_empty()
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path }
                        if path
                            .iter()
                            .map(String::as_str)
                            .eq(["serde_json", "Deserializer", "from_slice"])
                )
        })
        .unwrap_or_else(|| {
            panic!(
                "code_item_edges should surface serde_json in external frontier rows: {external_frontier_calls:#?}"
            )
    });
    assert_eq!(serde_frontier.site_id, serde_site);
    assert_serde_json_summary_proof(
        proof_context,
        fixture.target,
        serde_site,
        "Json::from_bytes serde_json::Deserializer::from_slice",
        "code_item_edges",
    );
    assert_no_external_summary_need_for_site(
        summary_needs,
        serde_site,
        "Json::from_bytes serde_json::Deserializer::from_slice",
        "code_item_edges",
    );
    assert!(
        reach_source_cfgs
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|cfg| cfg == r#"feature = "json""#),
        "code_item_edges Json::from_bytes reach should preserve the json feature cfg: {reach_source_cfgs:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "2");
    assert_eq!(
        ui_field(ui, "reach_source_cfgs"),
        reach_source_cfgs.len().to_string()
    );
    assert_eq!(
        ui_field(ui, "external_summary_needs"),
        summary_needs.len().to_string()
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_edges should surface real-corpus Json::from_bytes proof rows"
    );
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_boxed_into_route_constructor_callers() {
    let fixture = AxumBoxedIntoRouteToolFixture::new().await;
    let ctx = fixture.ctx("axum-boxed-into-route-edges");
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed("BoxedIntoRoute"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("struct"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, ctx)
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = payload
        .get("node_info")
        .and_then(|node| node.get("proof_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/boxed.rs:12 defines `BoxedIntoRoute<S, E>(...)`.
    //   axum/src/boxed.rs:{23,38,51} call `Self(...)`,
    //   `BoxedIntoRoute(...)`, and `Self(...)`.
    // Expected tool traversal: exact edge lookup of the tuple-struct target
    // exposes the incoming constructor edges and their projected proof rows.
    assert_boxed_into_route_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_edges",
    );
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_edges",
        );
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(
        ui_field(ui, "call_context_incoming"),
        fixture.callers.len().to_string().as_str()
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_edges should surface real-corpus BoxedIntoRoute proof rows"
    );
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_chrono_alias_constructor_callers() {
    let fixture = ChronoAliasConstructorToolFixture::new().await;
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed("Single"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("variant"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("chrono-alias-constructor-edges"))
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = payload
        .get("node_info")
        .and_then(|node| node.get("proof_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   chrono/src/offset/mod.rs:77 aliases
    //   `MappedLocalTime<T> = LocalResult<T>`.
    //   chrono/src/offset/mod.rs:81-83 defines `LocalResult::Single(T)`.
    //   chrono/src/offset/mod.rs:{143,156,468,502,535},
    //   offset/{fixed.rs:135,138,utc.rs:122,125,local/unix.rs:159}, and
    //   datetime/tests.rs:{75,79} call `MappedLocalTime::Single(...)`.
    // Expected tool traversal: exact edge lookup of the underlying enum
    // variant exposes all 12 incoming alias constructor edges and proof rows.
    assert_expected_path_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_edges",
        "MappedLocalTime::Single",
    );
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_edges",
        );
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "12");
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_edges should surface real-corpus chrono alias constructor proof rows"
    );
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_chrono_option_ok_or_try_receiver_callers() {
    let fixture = ChronoNaiveUtcToolFixture::new().await;
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed("naive_utc"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed("DateTime")),
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("chrono-naive-utc-edges"))
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = payload
        .get("node_info")
        .and_then(|node| node.get("proof_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   chrono/src/datetime/mod.rs:563 defines `DateTime<Tz>::naive_utc`.
    //   chrono/src/datetime/mod.rs:768,803 define associated constructors
    //   returning `Option<Self>`.
    //   chrono/src/format/parsed.rs:836,953 call
    //   `DateTime::from_timestamp*(...).ok_or(OUT_OF_RANGE)?.naive_utc()`.
    // Expected tool traversal: exact edge lookup of `DateTime::naive_utc`
    // exposes both incoming try-receiver method edges and proof rows.
    assert_chrono_naive_utc_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_edges",
    );
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_edges",
        );
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "2");
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_edges should surface real-corpus chrono naive_utc proof rows"
    );
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_run_ui_tests_callers() {
    let fixture = AxumRunUiTestsToolFixture::new().await;
    let ctx = fixture.ctx("axum-run-ui-tests-edges");
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed("run_ui_tests"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, ctx)
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = payload
        .get("node_info")
        .and_then(|node| node.get("proof_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-macros/src/lib.rs:797 defines `run_ui_tests`.
    //   debug_handler.rs:885,890; typed_path.rs:443; from_ref.rs:104;
    //   from_request/mod.rs:1050 call `crate::run_ui_tests(...)`.
    // Expected tool traversal: exact edge lookup of the callee function exposes
    // all five incoming caller-site edges and projected proof rows.
    assert_run_ui_tests_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_edges",
    );
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_edges",
        );
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "5");
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_edges should surface real-corpus run_ui_tests proof rows"
    );
}

#[tokio::test]
async fn code_item_edges_disambiguates_real_corpus_handler_call_by_owner_trait() {
    let fixture = AxumHandlerCallToolFixture::new().await;
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed("call"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(module_path),
        owner_trait: Some(Cow::Borrowed("Handler")),
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("axum-handler-call-edges"))
        .await
        .expect("owner-qualified Handler::call edge lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let node_info = payload.get("node_info").expect("node_info");
    let call_context = node_info
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = node_info
        .get("proof_context")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/handler/mod.rs:153 declares trait method `Handler::call`.
    //   axum/src/handler/service.rs:171 calls
    //   `Handler::call(handler, req, self.state.clone())`.
    // Expected exact-tool behavior: owner_trait="Handler" selects the trait
    // method in a file/module that otherwise contains multiple `call` methods,
    // and the tool exposes the DB-proven incoming caller edge.
    assert_handler_call_incoming_context(
        call_context,
        &fixture.caller,
        fixture.target,
        "code_item_edges",
    );
    assert_target_proof(
        proof_context,
        fixture.caller.owner,
        fixture.target,
        "code_item_edges",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "1");
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof context count")
            >= 1,
        "code_item_edges should surface real-corpus Handler::call proof rows"
    );
}

fn summary_usize(payload: &serde_json::Value, field: &str) -> usize {
    payload
        .get("call_graph_summary")
        .and_then(|summary| summary.get(field))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| panic!("missing call_graph_summary.{field}: {payload:#?}")) as usize
}

fn assert_source_file_json(files: &[serde_json::Value], suffix: &str, label: &str) {
    assert!(
        files
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|path| path.ends_with(suffix)),
        "{label} should include source file ending with {suffix:?}: {files:#?}"
    );
}

fn assert_source_crate_json(crates: &[serde_json::Value], expected: &str, label: &str) {
    assert!(
        crates
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|name| name == expected),
        "{label} should include source crate {expected:?}: {crates:#?}"
    );
}

fn assert_source_module_json(modules: &[serde_json::Value], expected: &[&str], label: &str) {
    assert!(
        modules.iter().any(|module| {
            module.as_array().is_some_and(|actual| {
                actual
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .eq(expected.iter().copied())
            })
        }),
        "{label} should include source module {expected:?}: {modules:#?}"
    );
}

#[tokio::test]
#[ignore = "graph_resolve_edges currently returns zero edges in the ploke-db backup; enable once fixed"]
async fn code_item_edges_graph_resolve_edges_smoke() {
    // Regression placeholder for the user-reported graph_resolve_edges case.
    let db = shared_backup_fixture_db(&PLOKE_DB_PRIMARY).expect("load ploke_db_primary fixture");
    let crate_root = workspace_root().join("crates/ploke-db");
    let abs_path = crate_root.join("src/helpers.rs");
    let mod_path = vec!["crate".to_string(), "helpers".to_string()];
    let edges = graph_resolve_edges(
        db.as_ref(),
        "function",
        &abs_path,
        &mod_path,
        "graph_resolve_edges",
    )
    .expect("graph_resolve_edges call should succeed");
    assert!(
        !edges.is_empty(),
        "Expected graph_resolve_edges to have edges once the underlying issue is fixed"
    );
}
