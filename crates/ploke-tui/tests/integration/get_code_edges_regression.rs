use std::{borrow::Cow, collections::HashMap, sync::Arc};

use ploke_core::{
    ArcStr,
    rag_types::{
        CallCalleeInfo, CallContextInfo, CallSiteKind, CallStatusKind, CallTargetKind,
        ModuleBoundaryEdgeInfo, ProofContextInfo,
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
    AsyncFutureToolFixture, AxumAwaitReceiverToolFixture, AxumErrorHandlingTraitsToolFixture,
    AxumExpandWithToolFixture, AxumRequestExtractPathToolFixture, AxumTaskSpawnEffectToolFixture,
    CallGraphToolFixture, ResultCallbackFixture, ReturnedClosureToolFixture,
    assert_await_result_unwrap_context, assert_await_result_unwrap_proof, assert_call_path_node,
    assert_forwarded_async_future_awaited_site, assert_forwarded_async_future_execution_flow,
    assert_forwarded_async_future_flow, assert_forwarded_returned_closure_binding_flow,
    assert_process_invariant_findings, assert_resolved_callable_param_proof,
    assert_resolved_dynamic_context, assert_resolved_method_target_context,
    assert_resolved_path_context, assert_result_callback_binding_payload, assert_target_proof,
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
        body_contains: None,
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
        body_contains: None,
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
        body_contains: None,
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
    for (item_name, label, ctx_name) in [
        (
            "call_forwarded_returned_async_future",
            "forwarded returned async future",
            "forwarded-async-future-edges",
        ),
        (
            "call_stored_forwarded_returned_async_future_tuple_field",
            "stored forwarded returned async future",
            "stored-forwarded-async-future-edges",
        ),
        (
            "call_aliased_stored_forwarded_returned_async_future_tuple_field",
            "aliased stored forwarded returned async future",
            "aliased-stored-forwarded-async-future-edges",
        ),
    ] {
        let params = EdgesParams {
            item_name: Cow::Borrowed(item_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Borrowed("crate"),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx(ctx_name))
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
        let future_flows = node_info
            .get("returned_future_flows")
            .and_then(serde_json::Value::as_array)
            .expect("node_info.returned_future_flows array");
        let future_execution_flows = node_info
            .get("returned_future_execution_flows")
            .and_then(serde_json::Value::as_array)
            .expect("node_info.returned_future_execution_flows array");

        // Same source oracle as lookup:
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:2401-2407 awaits
        // the producer directly or through `futures.0`, without proving
        // returned-call binding flow through the opaque forwarded future
        // boundary.
        assert_forwarded_async_future_awaited_site(awaited_sites, owner, label, "code_item_edges");
        assert!(
            returned_flows.is_empty(),
            "edges should not expose returned-call binding flow through the forwarded future boundary for {label}: {returned_flows:#?}"
        );
        assert_forwarded_async_future_flow(future_flows, owner, label, "code_item_edges");
        assert_forwarded_async_future_execution_flow(
            future_execution_flows,
            owner,
            label,
            "code_item_edges",
        );

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert_eq!(ui_field(ui, "awaited_call_sites"), "1");
        assert_eq!(ui_field(ui, "returned_call_binding_flows"), "0");
        assert_eq!(ui_field(ui, "returned_future_flows"), "1");
        assert_eq!(ui_field(ui, "returned_future_execution_flows"), "1");
    }
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
        body_contains: None,
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
        body_contains: None,
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
        body_contains: None,
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
        body_contains: None,
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
    let bindings = payload
        .get("node_info")
        .and_then(|node| node.get("local_bindings"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.local_bindings array");
    let edges = payload
        .get("node_info")
        .and_then(|node| node.get("local_binding_edges"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.local_binding_edges array");

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
    assert_result_callback_binding_payload(
        bindings,
        edges,
        fixture.owner,
        fixture.target,
        "call_single_result_callback",
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
    assert_eq!(ui_field(ui, "local_bindings"), bindings.len().to_string());
    assert_eq!(ui_field(ui, "local_binding_edges"), edges.len().to_string());
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
        body_contains: None,
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
        body_contains: None,
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
        body_contains: None,
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
        body_contains: None,
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
        body_contains: None,
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
        body_contains: None,
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
        body_contains: None,
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
        body_contains: None,
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
