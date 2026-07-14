use std::{borrow::Cow, collections::HashMap, sync::Arc};

use ploke_core::{
    ArcStr,
    rag_types::{
        CallCalleeInfo, CallContextInfo, CallResolutionKind, CallSiteKind, CallStatusKind,
        CallTargetKind, CrateBoundaryEdgeInfo, ModuleBoundaryEdgeInfo, ProofContextInfo,
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
    AsyncFutureToolFixture, AxumAwaitReceiverToolFixture, AxumBodyEmptyToolFixture,
    AxumBodyNewToolFixture, AxumBoxedIntoRouteToolFixture, AxumCompositeRejectionToolFixture,
    AxumErrorHandlingTraitsToolFixture, AxumExpandWithToolFixture, AxumFromFnBasicToolFixture,
    AxumGeneratedRejectionToolFixture, AxumHandlerCallToolFixture, AxumJsonFromBytesToolFixture,
    AxumParseAttrsToolFixture, AxumRequestExtractPathToolFixture, AxumRunUiTestsToolFixture,
    AxumTaskSpawnEffectToolFixture, CallGraphToolFixture, CallableBlockerFixture,
    CallableBlockerShape, CallableParamResolvedFixture, ChronoAliasConstructorToolFixture,
    ChronoNaiveUtcToolFixture, DirectSelfFieldDispatchFixture, FixtureBranchReceiverToolFixture,
    FixtureDynamicCallableToolFixture, FixtureSelfFieldReceiverToolFixture, ResultCallbackFixture,
    ReturnedClosureToolFixture, assert_ambiguous_candidate_proof,
    assert_ambiguous_dynamic_candidates, assert_ambiguous_dynamic_candidates_with_relation,
    assert_ambiguous_path_candidates, assert_await_result_unwrap_context,
    assert_await_result_unwrap_proof, assert_body_empty_dependency_root_proof,
    assert_body_empty_impact_summary, assert_body_empty_incoming_context,
    assert_body_new_generated_incoming_context, assert_body_new_impact_summary,
    assert_body_new_incoming_context, assert_boxed_into_route_incoming_context,
    assert_branch_receiver_context, assert_branch_receiver_proof, assert_call_path_node,
    assert_chrono_naive_utc_incoming_context, assert_dynamic_context, assert_dynamic_proof,
    assert_expected_path_incoming_context, assert_fixture_extern_c_abs_effects,
    assert_forwarded_async_future_awaited_site, assert_forwarded_returned_closure_binding_flow,
    assert_from_fn_basic_body_empty_crate_boundary, assert_generated_rejection_outgoing_context,
    assert_handler_call_incoming_context, assert_incoming_context,
    assert_initialized_local_receiver_context, assert_initialized_local_receiver_proof,
    assert_json_from_bytes_incoming_context, assert_no_external_summary_need_for_site,
    assert_parse_attrs_incoming_context, assert_path_blocker_proof, assert_path_context,
    assert_path_resolution_proof, assert_process_invariant_findings,
    assert_resolved_callable_param_proof, assert_resolved_dynamic_context,
    assert_resolved_method_target_context, assert_resolved_path_context,
    assert_run_ui_tests_incoming_context, assert_runtime_dispatch_blocker,
    assert_self_field_receiver_context, assert_self_field_receiver_proof,
    assert_serde_json_summary_proof, assert_serde_json_surface_measure_effect, assert_target_proof,
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
async fn code_item_edges_marks_async_targets_in_call_impact() {
    let fixture = CallGraphToolFixture::new().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("make_ready_local_assoc"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("async-target-edges"))
        .await
        .expect("make_ready_local_assoc edge lookup");
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
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:581 defines
    //   `pub async fn make_ready_local_assoc()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:585 defines
    //   `pub async fn call_await_result_instance_method()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:586 calls
    //   `make_ready_local_assoc().await.instance_value()`.
    let target = impact
        .get("target")
        .and_then(serde_json::Value::as_object)
        .expect("call_impact target object");
    assert_eq!(
        target.get("is_async").and_then(serde_json::Value::as_bool),
        Some(true),
        "code_item_edges should serialize async target metadata: {impact:#?}"
    );

    let direct_callers = impact
        .get("direct_callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact direct_callers array");
    assert!(
        direct_callers.iter().any(|caller| {
            caller.get("name").and_then(serde_json::Value::as_str)
                == Some("call_await_result_instance_method")
                && caller.get("is_async").and_then(serde_json::Value::as_bool) == Some(true)
        }),
        "code_item_edges should preserve async direct caller metadata: {direct_callers:#?}"
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
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:2121-2124
    //     a referenced `dyn Fn` binding initialized with `&local_target`
    //     is called as `(referenced_fn)()`.
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
        "call_parenthesized_referenced_dyn_fn_value_binding",
    ] {
        assert_resolved_dynamic_callable_edges(owner_name).await;
    }
}

#[tokio::test]
async fn code_item_edges_returns_private_referenced_dyn_fn_param_callable_context() {
    // Fixture source:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:2156-2161
    //     a private `&dyn Fn` parameter helper has one local caller passing
    //     `&local_target`, then calls `(f)()`.
    assert_resolved_dynamic_callable_edges("call_single_parenthesized_referenced_dyn_fn_param")
        .await;
}

#[tokio::test]
async fn code_item_edges_returns_private_boxed_dyn_fn_param_callable_context() {
    // Fixture source:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF
    //     private `Box<dyn Fn>` helpers have one local caller passing
    //     `Box::new(local_target)`, then call `f()` and `(f)()`.
    assert_callable_param_edges(
        CallableParamResolvedFixture::single_boxed_dyn_fn_param().await,
        "private boxed dyn Fn parameter",
    )
    .await;
    assert_resolved_dynamic_callable_edges("call_single_parenthesized_boxed_dyn_fn_param").await;
}

#[tokio::test]
async fn code_item_edges_returns_mut_referenced_dyn_fnmut_callable_context() {
    // Fixture source:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:2131-2134
    //     a mutable referenced `dyn FnMut` binding initialized with
    //     `&mut target` is called as `(referenced_fn)()`.
    assert_resolved_dynamic_callable_edges(
        "call_parenthesized_mut_referenced_dyn_fnmut_value_binding",
    )
    .await;
}

#[tokio::test]
async fn code_item_edges_returns_forwarded_named_field_dynamic_callable_context() {
    // Fixture source:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF
    //     private `call_forwarded_named_field_leaf(holder)` resolves through a
    //     private wrapper whose complete caller set constructs a holder with
    //     `callback: local_target`.
    // This keeps the new field-forwarding proof covered at the tool boundary
    // without widening the already-expensive dynamic-callable batch.
    assert_resolved_dynamic_callable_edges("call_forwarded_named_field_leaf").await;
}

#[tokio::test]
async fn code_item_edges_returns_two_hop_forwarded_named_field_dynamic_callable_context() {
    // Fixture source:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF
    //     private `call_two_hop_forwarded_named_field_leaf(holder)` resolves
    //     through two private helpers whose complete caller set constructs a
    //     holder with `callback: local_target`.
    // This pins the bounded two-hop holder-field proof at the edge-tool
    // boundary without broadening to arbitrary callable value-flow.
    assert_resolved_dynamic_callable_edges("call_two_hop_forwarded_named_field_leaf").await;
}

#[tokio::test]
async fn code_item_edges_returns_returned_function_pointer_param_dynamic_callable_context() {
    // Fixture source:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:2101-2107
    //     `return_forwarded_function_pointer(f)` returns the function-pointer
    //     parameter directly, and its only local caller passes `local_target`
    //     before invoking the returned callable as
    //     `return_forwarded_function_pointer(local_target)()`.
    // Parser/DB/RAG already prove the outer call as an exact DynamicFunction
    // edge; this pins the same returned-parameter proof at the edge-tool
    // boundary.
    assert_resolved_dynamic_callable_edges(
        "call_returned_forwarded_function_pointer_param_with_local_target",
    )
    .await;
}

async fn assert_resolved_dynamic_callable_edges(owner_name: &'static str) {
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
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:
    //     public `call_function_pointer_param(f)` calls `f()`;
    //     private `call_multi_conflicting_function_pointer_param(f)` also
    //     calls `f()`, but its local callers pass different functions;
    //     private `call_forwarded_conflicting_function_pointer_leaf(f)`
    //     receives `f` through a private wrapper whose callers pass different
    //     functions;
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
    // Public opaque parameters stay blocked and targetless. Private complete
    // local caller sets with conflicting callable arguments expose candidate
    // targets, but still do not fabricate a resolved call edge.
    for fixture in [
        CallableBlockerFixture::function_pointer_param().await,
        CallableBlockerFixture::multi_conflicting_function_pointer_param().await,
        CallableBlockerFixture::forwarded_conflicting_function_pointer_leaf().await,
        CallableBlockerFixture::generic_fn_once_value_binding().await,
        CallableBlockerFixture::multi_conflicting_generic_fn_once_param().await,
        CallableBlockerFixture::multi_conflicting_named_field_function_param().await,
        CallableBlockerFixture::field_function_param().await,
        CallableBlockerFixture::indexed_function_pointer().await,
        CallableBlockerFixture::indexed_field_function_param().await,
        CallableBlockerFixture::indexed_tuple_field_function_param().await,
    ] {
        assert_callable_blocker_edges(fixture).await;
    }
}

#[tokio::test]
async fn code_item_edges_returns_forwarded_named_field_param_blocker() {
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    //     private `call_forwarded_conflicting_named_field_leaf(holder)` receives
    //     `holder` through a private wrapper whose callers pass different
    //     callback functions.
    assert_callable_blocker_edges(
        CallableBlockerFixture::forwarded_conflicting_named_field_leaf().await,
    )
    .await;
}

#[tokio::test]
async fn code_item_edges_returns_returned_conflicting_function_pointer_candidates() {
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:2109-2118:
    //     `return_conflicting_forwarded_function_pointer(f)` returns `f`, and
    //     two local callers immediately invoke the returned callable with
    //     different function items: `local_target` and `other_target`.
    // The edge-tool payload must preserve candidate-only ambiguity and avoid
    // fabricating a resolved edge for either caller.
    for fixture in [
        CallableBlockerFixture::returned_conflicting_function_pointer_local().await,
        CallableBlockerFixture::returned_conflicting_function_pointer_other().await,
    ] {
        assert_callable_blocker_edges(fixture).await;
    }
}

#[tokio::test]
async fn code_item_edges_returns_direct_self_field_dispatch_candidates() {
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:2313-2341:
    //     `DirectSelfFieldDispatcher::invoke` calls `(self.call)(self)`;
    //     the fixture's explicit constructors assign `direct_self_field_local`
    //     and `direct_self_field_other` directly to the `call` field.
    // The edge tool must expose the same candidate-only dynamic row as lookup
    // without admitting a traversal edge.
    let fixture = DirectSelfFieldDispatchFixture::new().await;
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

    let result = CodeItemEdges::execute(params, fixture.ctx("direct-self-field-edges"))
        .await
        .expect("direct self-field edges");
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

    let label = "DirectSelfFieldDispatcher::invoke direct self-field call";
    let path = fixture.path.iter().map(String::as_str).collect::<Vec<_>>();
    let site_id = assert_ambiguous_dynamic_candidates_with_relation(
        call_context,
        fixture.owner,
        Some(path.as_slice()),
        None,
        &fixture.candidates,
        CallTargetKind::DynamicFunction,
        label,
        "code_item_edges",
    );
    assert_ambiguous_candidate_proof(
        proof_context,
        fixture.owner,
        site_id,
        fixture.build_domain,
        &fixture.candidates,
        label,
        "code_item_edges",
    );
    assert_eq!(
        summary_usize(&payload, "blocked"),
        0,
        "code_item_edges summary should not count candidate rows as targetless blockers: {payload:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_edges should surface outgoing direct self-field call context"
    );
    assert_eq!(
        ui_field(ui, "proof_context"),
        proof_context.len().to_string()
    );
}

async fn assert_callable_blocker_edges(fixture: CallableBlockerFixture) {
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
async fn code_item_edges_returns_awaited_async_closure_future_tuple_field_context() {
    assert_awaited_async_closure_future_edges(
        AsyncFutureToolFixture::tuple_field().await,
        "awaited async closure future tuple field",
        "async-future-tuple-edges",
    )
    .await;
}

#[tokio::test]
async fn code_item_edges_returns_awaited_async_closure_future_named_field_context() {
    assert_awaited_async_closure_future_edges(
        AsyncFutureToolFixture::named_field().await,
        "awaited async closure future named field",
        "async-future-named-edges",
    )
    .await;
}

#[tokio::test]
async fn code_item_edges_returns_awaited_async_closure_future_named_field_alias_context() {
    assert_awaited_async_closure_future_edges(
        AsyncFutureToolFixture::named_field_alias().await,
        "awaited async closure future named field alias",
        "async-future-named-alias-edges",
    )
    .await;
}

#[tokio::test]
async fn code_item_edges_returns_awaited_async_closure_future_indexed_array_context() {
    assert_awaited_async_closure_future_edges(
        AsyncFutureToolFixture::indexed_array().await,
        "awaited async closure future indexed array",
        "async-future-indexed-array-edges",
    )
    .await;
}

#[tokio::test]
async fn code_item_edges_returns_awaited_returned_async_closure_context() {
    assert_awaited_returned_async_closure_edges(
        AsyncFutureToolFixture::returned_async_closure().await,
        "awaited returned async closure",
        "returned-async-closure-edges",
    )
    .await;
}

#[tokio::test]
async fn code_item_edges_returns_stored_returned_async_closure_context() {
    assert_awaited_returned_async_closure_edges(
        AsyncFutureToolFixture::stored_returned_async_closure().await,
        "stored returned async closure future",
        "stored-returned-async-closure-edges",
    )
    .await;
}

#[tokio::test]
async fn code_item_edges_returns_forwarded_returned_closure_context() {
    assert_forwarded_returned_closure_edges(
        ReturnedClosureToolFixture::forwarded_returned_closure().await,
        "forwarded returned closure",
        "forwarded-returned-closure-edges",
    )
    .await;
}

#[tokio::test]
async fn code_item_edges_returns_forwarded_async_future_awaited_site() {
    let fixture = CallGraphToolFixture::new().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("call_forwarded_returned_async_future"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("forwarded-async-future-edges"))
        .await
        .expect("forwarded async future edges should succeed");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let node_info = payload.get("node_info").expect("node_info");
    let owner = node_info
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("node_info.id")
        .parse()
        .expect("node_info.id should be a UUID");
    let awaited_sites = node_info
        .get("awaited_call_sites")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.awaited_call_sites array");
    let returned_flows = node_info
        .get("returned_call_binding_flows")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.returned_call_binding_flows array");

    // Same source oracle as lookup:
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2384-2385 awaits the
    // producer call but does not prove returned-call binding flow through the
    // opaque forwarded future boundary.
    assert_forwarded_async_future_awaited_site(
        awaited_sites,
        owner,
        "forwarded returned async future",
        "code_item_edges",
    );
    assert!(
        returned_flows.is_empty(),
        "edges should not expose returned-call binding flow through the forwarded future boundary: {returned_flows:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "awaited_call_sites"), "1");
    assert_eq!(ui_field(ui, "returned_call_binding_flows"), "0");
}

async fn assert_awaited_async_closure_future_edges(
    fixture: AsyncFutureToolFixture,
    label: &'static str,
    ctx_name: &'static str,
) {
    let params = EdgesParams {
        item_name: Cow::Borrowed(fixture.owner_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx(ctx_name))
        .await
        .unwrap_or_else(|err| panic!("{label} edges should succeed: {err}"));
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

    // Same fixture oracles as lookup: `let futures = (closure(),);
    // futures.0.await;` and `holder = AsyncFutureHolder { future: closure() };
    // holder.future.await;` plus `let alias = holder.future; alias.await;` and
    // `let futures = [closure()]; futures[0].await;` prove the exact stored
    // future is polled.
    let callee = CallCalleeInfo::Path {
        path: vec!["closure".to_string()],
    };
    assert_resolved_path_context(
        call_context,
        fixture.owner,
        &callee,
        fixture.closure,
        CallTargetKind::Closure,
        label,
        "code_item_edges",
    );
    assert_target_proof(proof_context, fixture.owner, fixture.closure, label);

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_edges should surface the {label} closure call"
    );
}

async fn assert_awaited_returned_async_closure_edges(
    fixture: AsyncFutureToolFixture,
    label: &'static str,
    ctx_name: &'static str,
) {
    let params = EdgesParams {
        item_name: Cow::Borrowed(fixture.owner_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx(ctx_name))
        .await
        .unwrap_or_else(|err| panic!("{label} edges should succeed: {err}"));
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

    // Same fixture oracle as lookup:
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2359-2365 polls the
    // returned async closure future either directly or through a same-block
    // local future binding.
    assert_resolved_dynamic_context(
        call_context,
        fixture.owner,
        fixture.closure,
        CallTargetKind::DynamicClosure,
        label,
        "code_item_edges",
    );
    assert_target_proof(proof_context, fixture.owner, fixture.closure, label);

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_edges should surface the {label} dynamic closure call"
    );
}

async fn assert_forwarded_returned_closure_edges(
    fixture: ReturnedClosureToolFixture,
    label: &'static str,
    ctx_name: &'static str,
) {
    let params = EdgesParams {
        item_name: Cow::Borrowed(fixture.owner_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx(ctx_name))
        .await
        .unwrap_or_else(|err| panic!("{label} edges should succeed: {err}"));
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
    let returned_flows = payload
        .get("node_info")
        .and_then(|node| node.get("returned_call_binding_flows"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.returned_call_binding_flows array");

    // Same source oracle as lookup:
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1520 forwards a
    // sync returned closure from `make_target_closure()` to the caller.
    assert_resolved_dynamic_context(
        call_context,
        fixture.owner,
        fixture.closure,
        CallTargetKind::DynamicClosure,
        label,
        "code_item_edges",
    );
    assert_target_proof(proof_context, fixture.owner, fixture.closure, label);
    assert_forwarded_returned_closure_binding_flow(
        returned_flows,
        fixture.owner,
        label,
        "code_item_edges",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_edges should surface the {label} dynamic closure call"
    );
    assert_eq!(ui_field(ui, "returned_call_binding_flows"), "1");
}

#[tokio::test]
async fn code_item_edges_returns_multi_caller_function_pointer_param_target() {
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1740-1742
    //     private `call_multi_function_pointer_param(f)` calls `f()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1744-1749
    //     both local callers pass `local_target`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1942-1951
    //     private `call_forwarded_function_pointer_leaf(f)` resolves through a
    //     private wrapper whose complete caller set passes `local_target`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:2004-2018
    //     private `call_two_hop_forwarded_function_pointer_leaf(f)` resolves
    //     through two private forwarding helpers whose complete caller set
    //     passes `local_target`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1764-1768
    //     private `call_multi_generic_fn_once_param(generic_f)` calls
    //     `generic_f()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1771-1776
    //     both local generic callers pass `local_target`.
    for fixture in [
        CallableParamResolvedFixture::multi_function_pointer_param().await,
        CallableParamResolvedFixture::forwarded_function_pointer_leaf().await,
        CallableParamResolvedFixture::two_hop_forwarded_function_pointer_leaf().await,
        CallableParamResolvedFixture::multi_generic_fn_once_param().await,
    ] {
        assert_callable_param_edges(fixture, "multi-caller callable parameter").await;
    }
}

#[tokio::test]
async fn code_item_edges_returns_forwarded_referenced_dyn_fn_param_targets() {
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF
    //     private `&dyn Fn` leaves call `f()` after one-hop and two-hop
    //     forwarding. Each complete private caller chain passes `&local_target`.
    for fixture in [
        CallableParamResolvedFixture::forwarded_referenced_dyn_fn_leaf().await,
        CallableParamResolvedFixture::two_hop_forwarded_referenced_dyn_fn_leaf().await,
    ] {
        assert_callable_param_edges(fixture, "forwarded referenced dyn Fn parameter").await;
    }
}

#[tokio::test]
async fn code_item_edges_returns_forwarded_boxed_dyn_fn_param_targets() {
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF
    //     private `Box<dyn Fn>` leaves call `f()` after one-hop and two-hop
    //     forwarding. Each complete private caller chain passes
    //     `Box::new(local_target)`.
    for fixture in [
        CallableParamResolvedFixture::forwarded_boxed_dyn_fn_leaf().await,
        CallableParamResolvedFixture::two_hop_forwarded_boxed_dyn_fn_leaf().await,
    ] {
        assert_callable_param_edges(fixture, "forwarded boxed dyn Fn parameter").await;
    }
}

#[tokio::test]
async fn code_item_edges_returns_result_method_callback_function_target() {
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF
    //     private `call_single_result_callback(f)` calls
    //     `Ok::<i32, ()>(1).and_then(f)`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF
    //     the only local caller passes `local_result_target`.
    let fixture = ResultCallbackFixture::new().await;
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

    let result = CodeItemEdges::execute(params, fixture.ctx("result-callback-edges"))
        .await
        .expect("result callback edges");
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

    let site_id = assert_resolved_method_target_context(
        call_context,
        fixture.owner,
        &fixture.callee(),
        fixture.target,
        CallTargetKind::MethodCallbackFunction,
        Some(0),
        "result method callback",
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
        "code_item_edges should surface the resolved result callback row"
    );
    assert_eq!(
        ui_field(ui, "proof_context"),
        proof_context.len().to_string()
    );
}

async fn assert_callable_param_edges(fixture: CallableParamResolvedFixture, label: &str) {
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

    let result = CodeItemEdges::execute(params, fixture.ctx("callable-param-edges"))
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

    let callee = CallCalleeInfo::Path {
        path: fixture.path.clone(),
    };
    let site_id = assert_resolved_path_context(
        call_context,
        fixture.owner,
        &callee,
        fixture.target,
        CallTargetKind::Function,
        label,
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
        "code_item_edges should surface the resolved {label} row"
    );
    assert_eq!(
        ui_field(ui, "proof_context"),
        proof_context.len().to_string()
    );
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
async fn code_item_edges_uses_stored_effect_policy_when_allowlist_omitted() {
    let fixture = AxumTaskSpawnEffectToolFixture::new().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("deserialize_error_status_codes"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("axum-task-spawn-stored-policy-edges"))
        .await
        .expect("deserialize_error_status_codes stored-policy edge lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let node_info = payload
        .get("node_info")
        .and_then(serde_json::Value::as_object)
        .expect("node_info object");
    let policy_violations = node_info
        .get("call_effect_policy_violations")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_effect_policy_violations array");

    // Source oracle:
    //   axum/src/form.rs:262 -> TestClient::new(app)
    //   axum/src/test_helpers/test_client.rs:36 -> spawn_service(svc)
    //   axum/src/test_helpers/test_client.rs:23 -> tokio::spawn(...)
    //
    // The fixture admits a stored owner `effect_policy` for
    // `deserialize_error_status_codes` that allows only `ffi_boundary`.
    assert_task_spawn_policy_violation(
        policy_violations,
        &fixture,
        "code_item_edges stored-policy call_effect_policy_violations",
    );
}

#[tokio::test]
async fn code_item_edges_returns_fixture_process_invariant_findings() {
    let fixture = CallGraphToolFixture::new().await;
    let expected = fixture.seed_extern_c_process_invariant();
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

    let result = CodeItemEdges::execute(params, fixture.ctx("extern-c-process-invariant-edges"))
        .await
        .expect("call_extern_c_function edge lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let node_info = payload
        .get("node_info")
        .and_then(serde_json::Value::as_object)
        .expect("node_info object");
    let findings = node_info
        .get("call_proof_invariant_findings")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_proof_invariant_findings array");

    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:839-844 declares
    //   foreign function `abs(input)` inside an `unsafe extern "C"` block and
    //   calls `abs(value)` from `call_extern_c_function`.
    //
    // The fixture marks the external `abs(value)` frontier as an
    // `operating_system_process_create` effect. The exact edges payload should
    // expose the blocked detached-process invariant through `node_info`
    // without inventing a local target for `abs`.
    assert_process_invariant_findings(
        findings,
        &expected,
        "code_item_edges node_info.call_proof_invariant_findings",
    );
    let owner = expected.owner.to_string();
    assert_eq!(
        node_info.get("id").and_then(serde_json::Value::as_str),
        Some(owner.as_str()),
        "code_item_edges should resolve the fixture extern C owner: {payload:#?}"
    );
    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(
        ui_field(ui, "proof_invariant_findings"),
        findings.len().to_string()
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
    let entrypoints = node_info
        .get("call_test_entrypoints")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_test_entrypoints array");
    assert_eq!(
        entrypoints.len(),
        1,
        "code_item_edges should expose one generated-test entrypoint summary: {entrypoints:#?}"
    );
    let entrypoint = entrypoints[0]
        .as_object()
        .expect("node_info.call_test_entrypoints object");
    assert_eq!(
        entrypoint
            .get("entrypoint_summary_id")
            .and_then(serde_json::Value::as_str),
        Some("entrypoint-summary:axum-error-handling-traits-test")
    );
    assert_eq!(
        entrypoint
            .get("definition_id")
            .and_then(serde_json::Value::as_str),
        Some(target_id.as_str())
    );
    assert_eq!(
        entrypoint
            .get("target_kind")
            .and_then(serde_json::Value::as_str),
        Some("test")
    );
    assert_eq!(
        entrypoint
            .get("target_name")
            .and_then(serde_json::Value::as_str),
        Some("generated-test-harness")
    );
    assert_eq!(
        entrypoint
            .get("summary_class")
            .and_then(serde_json::Value::as_str),
        Some("analyzed_source")
    );
    assert_eq!(
        entrypoint
            .get("required_containment")
            .and_then(serde_json::Value::as_str),
        Some("rust-test-harness")
    );
    let allowed_effects = entrypoint
        .get("allowed_effects")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_test_entrypoints.allowed_effects array");
    assert_eq!(
        allowed_effects
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>(),
        vec!["ffi_boundary"]
    );
    let test_selection = node_info
        .get("call_test_selection")
        .and_then(serde_json::Value::as_object)
        .expect("node_info.call_test_selection object");
    assert_eq!(
        test_selection
            .get("target")
            .and_then(|target| target.get("id"))
            .and_then(serde_json::Value::as_str),
        Some(target_id.as_str()),
        "code_item_edges call_test_selection should target error_handling::traits: {test_selection:#?}"
    );
    for field in ["source_test_callers", "source_test_paths"] {
        let rows = test_selection
            .get(field)
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("call_test_selection {field} array: {test_selection:#?}"));
        assert!(
            rows.is_empty(),
            "code_item_edges generated harness selection should not fabricate source {field}: {rows:#?}"
        );
    }
    assert_eq!(
        test_selection
            .get("generated_entrypoints")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1),
        "code_item_edges call_test_selection should include the generated harness entrypoint: {test_selection:#?}"
    );
    assert_eq!(
        test_selection
            .get("build_domains")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1),
        "code_item_edges call_test_selection should include the linked build domain: {test_selection:#?}"
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
    assert_eq!(
        ui_field(ui, "call_test_entrypoints"),
        entrypoints.len().to_string()
    );
    assert_eq!(ui_field(ui, "call_test_selection_source_tests"), "0");
    assert_eq!(ui_field(ui, "call_test_selection_source_paths"), "0");
    assert_eq!(
        ui_field(ui, "call_test_selection_generated_entrypoints"),
        "1"
    );
    assert_eq!(ui_field(ui, "call_test_selection_build_domains"), "1");
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
    let reach = payload
        .get("node_info")
        .and_then(|node| node.get("call_reach"))
        .and_then(serde_json::Value::as_object)
        .expect("node_info.call_reach object");
    let external_frontier = reach
        .get("external_frontier_calls")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach external_frontier_calls array");

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum/src/serve/listener.rs:142 owns `ConnLimiter<T>::accept`.
    //   axum/src/serve/listener.rs:143 calls
    //   `self.sem.clone().acquire_owned().await.unwrap()`.
    // The edge-oriented exact tool should expose the same external targetless
    // AwaitMethodCallResult(acquire_owned) receiver row as lookup, without a
    // fabricated local edge.
    let site_id =
        assert_await_result_unwrap_context(call_context, fixture.owner, "code_item_edges");
    assert_await_result_unwrap_proof(proof_context, fixture.owner, site_id, "code_item_edges");
    let external_calls = external_frontier
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed external frontier rows");
    let external = external_calls
        .iter()
        .find(|call| call.site_id == site_id)
        .unwrap_or_else(|| {
            panic!(
                "code_item_edges should expose AwaitMethodCallResult unwrap in external frontier rows: {external_calls:#?}"
            )
        });
    assert_eq!(external.owner_id, fixture.owner);
    assert_eq!(external.status, CallStatusKind::External);
    assert!(
        external.targets.is_empty(),
        "edge-tool external frontier call should remain targetless: {external:#?}"
    );
    assert!(
        summary_usize(&payload, "blocked") >= 1,
        "code_item_edges summary should report the targetless await receiver frontier row: {payload:#?}"
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
        "code_item_edges should surface blocked frontier count in the UI payload"
    );
    assert_eq!(
        ui_field(ui, "reach_external_frontier_calls"),
        external_calls.len().to_string()
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
    let module_boundary_edges = start_payload
        .get("node_info")
        .and_then(|node| node.get("module_boundary_edges"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.module_boundary_edges array");

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
    let module_edges = module_boundary_edges
        .iter()
        .map(|edge| serde_json::from_value::<ModuleBoundaryEdgeInfo>(edge.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed module boundary rows");
    assert_eq!(
        module_edges.len(),
        1,
        "code_item_edges should expose the enriched module-boundary row for architecture-review questions: {module_edges:#?}"
    );
    let module_edge = &module_edges[0];
    assert_eq!(module_edge.edge.caller_id, fixture.intermediate);
    assert_eq!(module_edge.edge.callee_id, fixture.target);
    assert_eq!(module_edge.edge.source_kind, CallSiteKind::Path);
    assert_eq!(
        module_edge.edge.relation,
        CallTargetKind::AssociatedFunction
    );
    assert_eq!(module_edge.caller.id, fixture.intermediate);
    assert_eq!(module_edge.caller.name, "extract_with_state");
    assert_eq!(
        module_edge.caller.module_path,
        vec![
            "crate".to_string(),
            "ext_traits".to_string(),
            "request".to_string()
        ]
    );
    assert_eq!(module_edge.callee.id, fixture.target);
    assert_eq!(module_edge.callee.name, "from_request");
    assert_eq!(
        module_edge.callee.module_path,
        vec!["crate".to_string(), "extract".to_string()]
    );
    assert_eq!(module_edge.site.owner_id, fixture.intermediate);
    assert_eq!(module_edge.site.kind, CallSiteKind::Path);
    assert_eq!(module_edge.site.status, CallStatusKind::Resolved);
    assert_eq!(module_edge.site.arg_count, Some(2));
    assert!(
        matches!(
            &module_edge.site.callee,
            CallCalleeInfo::Path { path } if path == &vec!["E".to_string(), "from_request".to_string()]
        ),
        "code_item_edges should preserve the E::from_request boundary callsite: {module_edge:#?}"
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
        ui_field(start_ui, "module_boundary_edges"),
        module_edges.len().to_string()
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
        34,
        "code_item_edges impact should expose all direct FromRequest::from_request callsite rows, including generated handler and tuple extractor rows: {target_impact_direct_call_sites:#?}"
    );
    assert!(
        target_impact_callsite_buckets.iter().any(|bucket| {
            bucket.get("kind").and_then(serde_json::Value::as_str) == Some("path")
                && bucket.get("relation").and_then(serde_json::Value::as_str)
                    == Some("associated_function")
                && bucket.get("count").and_then(serde_json::Value::as_u64) == Some(34)
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
async fn code_item_edges_returns_real_corpus_crate_boundary_edges() {
    let fixture = AxumFromFnBasicToolFixture::new().await;
    let ctx = fixture.ctx("axum-from-fn-basic-edges");
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed("basic"),
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
        .expect("from_fn::tests::basic edge lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let crate_boundary_edges = payload
        .get("node_info")
        .and_then(|node| node.get("crate_boundary_edges"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.crate_boundary_edges array");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Architecture and component review:
    //   "Are lower-level crates depending on higher-level application code?"
    //   "Which crates or binaries need rebuilding after this internal function changes?"
    //
    // Source oracle:
    //   tests/fixture_github_clones/corpus/axum/axum/src/middleware/from_fn.rs:394
    //     defines `tests::basic`.
    //   tests/fixture_github_clones/corpus/axum/axum/src/middleware/from_fn.rs:411
    //     calls `Body::empty()`.
    //   tests/fixture_github_clones/corpus/axum/axum-core/src/body.rs:52
    //     defines `Body::empty`.
    // Expected tool traversal: exact edge lookup of the caller owner exposes
    // the resolved outbound cross-crate edge in the node context payload.
    let edges = crate_boundary_edges
        .iter()
        .map(|edge| serde_json::from_value::<CrateBoundaryEdgeInfo>(edge.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed crate-boundary rows");
    assert_from_fn_basic_body_empty_crate_boundary(
        &edges,
        fixture.owner,
        fixture.body_empty_target,
        "code_item_edges",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(
        ui_field(ui, "crate_boundary_edges"),
        edges.len().to_string()
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
async fn code_item_edges_returns_real_corpus_body_new_generated_callers() {
    let fixture = AxumBodyNewToolFixture::new().await;
    let ctx = fixture.ctx("axum-body-new-edges");
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed("new"),
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

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   crates/ploke-db/tests/unit/call_graph_fixture_queries/real_target_matrix.rs
    //   axum-core/src/body.rs:46 defines `Body::new`.
    //   axum-core/src/body.rs:120-126 defines `body_from_impl!`.
    //   axum-core/src/body.rs:129-138 invokes it for seven concrete buffer
    //   types. Each generated `impl From<T> for Body` contains
    //   `Self::new(http_body_util::Full::from(buf))`.
    // Expected tool traversal: exact edge lookup of `Body::new` exposes the
    // full target-centered caller set and includes the seven generated
    // `Body::from -> Body::new` caller-site rows.
    assert_body_new_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_edges",
    );
    assert_body_new_generated_incoming_context(
        call_context,
        &fixture.generated_callers,
        fixture.target,
        "code_item_edges",
    );
    assert_body_new_impact_summary(
        impact,
        &fixture.generated_callers,
        fixture.target,
        "code_item_edges",
    );
    for caller in &fixture.generated_callers {
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
        fixture.callers.len().to_string()
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.generated_callers.len(),
        "code_item_edges should surface real-corpus Body::new generated proof rows"
    );
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_generated_rejection_self_methods() {
    let fixture = AxumGeneratedRejectionToolFixture::new().await;
    let ctx = fixture.ctx("axum-rejection-edges");
    let params = EdgesParams {
        item_name: Cow::Borrowed("into_response"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: Some(Cow::Borrowed("IntoResponse")),
        owner_type: Some(Cow::Borrowed("MissingExtension")),
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, ctx)
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let node_info = payload.get("node_info").expect("node_info object");
    let call_context = node_info
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = node_info
        .get("proof_context")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");
    let owner = fixture.owner.to_string();
    assert_eq!(
        node_info.get("id").and_then(serde_json::Value::as_str),
        Some(owner.as_str()),
        "code_item_edges should resolve the generated MissingExtension::into_response owner"
    );

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   crates/ploke-db/tests/unit/call_graph_fixture_queries/real_target_matrix.rs
    //   axum-core/src/macros.rs:30-115 defines `__define_rejection!`.
    //   axum/src/extract/rejection.rs:42-48 invokes it for
    //   `MissingExtension(Error)`.
    // Expected tool traversal: exact edge lookup of the generated
    // `IntoResponse for MissingExtension` method exposes outgoing one-hop
    // method edges for `self.status()` and `self.body_text()`.
    assert_generated_rejection_outgoing_context(call_context, &fixture.calls, "code_item_edges");
    for call in &fixture.calls {
        assert_target_proof(proof_context, call.owner, call.target, "code_item_edges");
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= fixture.calls.len(),
        "code_item_edges should surface generated rejection outgoing call rows"
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.calls.len(),
        "code_item_edges should surface generated rejection proof rows"
    );
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_composite_rejection_delegate() {
    let fixture = AxumCompositeRejectionToolFixture::new().await;
    let ctx = fixture.ctx("axum-composite-rejection-edges");
    let params = EdgesParams {
        item_name: Cow::Borrowed("into_response"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: Some(Cow::Borrowed("IntoResponse")),
        owner_type: Some(Cow::Borrowed("QueryRejection")),
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, ctx)
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let node_info = payload.get("node_info").expect("node_info object");
    let call_context = node_info
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = node_info
        .get("proof_context")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");
    let owner = fixture.owner.to_string();
    assert_eq!(
        node_info.get("id").and_then(serde_json::Value::as_str),
        Some(owner.as_str()),
        "code_item_edges should resolve the generated QueryRejection::into_response owner"
    );

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   crates/ploke-db/tests/unit/call_graph_fixture_queries/real_target_matrix.rs
    //   axum-core/src/macros.rs:154-180 defines `__composite_rejection!`.
    //   axum/src/extract/rejection.rs:92-100 invokes it for
    //   `QueryRejection { FailedToDeserializeQueryString }`.
    //   axum/src/extract/rejection.rs:84-90 invokes `define_rejection!`
    //   for `FailedToDeserializeQueryString`.
    // Expected tool traversal: exact edge lookup of the generated
    // `IntoResponse for QueryRejection` method exposes the outgoing one-hop
    // enum-variant receiver edge from `inner.into_response()` to generated
    // `FailedToDeserializeQueryString::into_response`.
    assert_generated_rejection_outgoing_context(
        call_context,
        std::slice::from_ref(&fixture.call),
        "code_item_edges",
    );
    assert_target_proof(
        proof_context,
        fixture.call.owner,
        fixture.call.target,
        "code_item_edges",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_edges should surface composite rejection outgoing call rows"
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= 1,
        "code_item_edges should surface composite rejection proof rows"
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
    let reach_effects = payload
        .get("node_info")
        .and_then(|node| node.get("call_reach_effects"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_reach_effects array");
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
    assert_serde_json_surface_measure_effect(
        reach_effects,
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
    assert_eq!(
        ui_field(ui, "reach_effects"),
        reach_effects.len().to_string()
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
