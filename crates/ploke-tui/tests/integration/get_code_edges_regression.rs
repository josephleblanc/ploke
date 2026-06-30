use std::{borrow::Cow, collections::HashMap, sync::Arc};

use ploke_core::ArcStr;
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
    AxumHandlerCallToolFixture, AxumJsonFromBytesToolFixture, AxumParseAttrsToolFixture,
    AxumRequestExtractPathToolFixture, AxumRunUiTestsToolFixture, CallGraphToolFixture,
    assert_await_result_unwrap_context, assert_await_result_unwrap_proof,
    assert_body_empty_incoming_context, assert_boxed_into_route_incoming_context,
    assert_call_path_node, assert_handler_call_incoming_context, assert_incoming_context,
    assert_json_from_bytes_incoming_context, assert_parse_attrs_incoming_context,
    assert_run_ui_tests_incoming_context, assert_target_proof, assert_two_hop_call_path, ui_field,
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

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_edges should surface outgoing targetless call-context count"
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

    let target_params = EdgesParams {
        item_name: Cow::Borrowed("from_request"),
        file_path: Cow::Owned(fixture.target_file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(fixture.target_module_path_arg()),
        owner_trait: Some(Cow::Borrowed("FromRequest")),
        owner_type: None,
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
    assert_two_hop_call_path(
        incoming_paths,
        fixture.start,
        fixture.intermediate,
        fixture.target,
        "code_item_edges incoming paths",
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
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core/src/body.rs:110 and :116 call `Self::empty()`.
    //   axum-core/src/response/into_response.rs response conversion rows call
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
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_edges",
        );
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "4");
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
    assert_eq!(ui_field(ui, "call_context_incoming"), "8");
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
    let ctx = fixture.ctx("axum-json-from-bytes-edges");
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed("from_bytes"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
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
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:112 and :128 call `Self::from_bytes(&bytes)`.
    // Expected tool traversal: exact edge lookup of the callee method exposes
    // the same two trait-impl caller-site edges and projected proof rows as
    // the DB target-centered query.
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

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "2");
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
    //   axum/src/boxed.rs:38 calls `BoxedIntoRoute(Box::new(...))`.
    // Expected tool traversal: exact edge lookup of the tuple-struct target
    // exposes the one incoming constructor edge and its projected proof row.
    assert_boxed_into_route_incoming_context(
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
            .expect("proof count")
            >= 1,
        "code_item_edges should surface real-corpus BoxedIntoRoute proof rows"
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
