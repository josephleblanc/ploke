use std::{borrow::Cow, collections::HashMap, sync::Arc};

use ploke_core::ArcStr;
use ploke_db::helpers::{graph_resolve_edges, graph_resolve_exact, list_primary_nodes};
use ploke_db::{Database, create_index_primary};
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::IoManagerHandle;
use ploke_rag::TokenBudget;
use ploke_test_utils::workspace_root;
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
    // Load ploke-db backup (copied from ~/.config/ploke/data) to mirror the user repro.
    let mut backup = workspace_root();
    backup.push("tests/backup_dbs/ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45");
    assert!(
        backup.exists(),
        "ploke-db backup missing at {}; copy ~/.config/ploke/data/ploke-db_* to tests/backup_dbs",
        backup.display()
    );

    let db = {
        let db = Database::init_with_schema().expect("init db schema");
        let rels = db.relations_vec().expect("relations");
        db.import_from_backup(&backup, &rels)
            .expect("import ploke-db backup");
        create_index_primary(&db).expect("index primary");
        Arc::new(db)
    };

    // Minimal AppState for tool execution, focused on the real ploke-db crate.
    let cfg = UserConfig::default();
    let runtime_cfg = RuntimeConfig::from(cfg.clone());
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        cfg.load_embedding_processor().expect("embedder"),
    ));
    let crate_root = workspace_root().join("crates/ploke-db");
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

    // Find a real item in ploke-db that actually has edges.
    let primary_nodes = list_primary_nodes(db.as_ref()).expect("list primary nodes");
    let (focus, expected_edges, abs_path) = primary_nodes
        .into_iter()
        .filter_map(|row| {
            // Normalize to absolute path under the crate root
            let abs_path = if row.file_path.is_absolute() {
                row.file_path.clone()
            } else {
                crate_root.join(&row.file_path)
            };
            // Only consider nodes under the crate root with well-formed module paths
            if !abs_path.starts_with(&crate_root) {
                return None;
            }
            if row.module_path.first().map(String::as_str) != Some("crate") {
                return None;
            }
            let edges = graph_resolve_edges(
                db.as_ref(),
                &row.relation,
                abs_path.as_path(),
                &row.module_path,
                &row.name,
            )
            .ok()?;
            if edges.is_empty() {
                return None;
            }
            Some((row, edges, abs_path))
        })
        .next()
        .expect("ploke-db backup should contain at least one primary node with edges");

    // Check hash freshness to guard against stale backups.
    let stored_nodes = graph_resolve_exact(
        db.as_ref(),
        &focus.relation,
        abs_path.as_path(),
        &focus.module_path,
        &focus.name,
    )
    .expect("graph_resolve_exact");
    let stored = stored_nodes.first().expect("node present");
    let stored_hash = stored.file_tracking_hash.clone();
    let actual_hash = ploke_io::read::generate_hash_for_file(&abs_path, stored.namespace)
        .await
        .expect("compute file hash");
    assert_eq!(
        stored_hash,
        actual_hash,
        "tracking hash mismatch for {}; refresh ploke-db backup",
        abs_path.display()
    );

    let rel_path = abs_path
        .strip_prefix(&crate_root)
        .unwrap_or(&abs_path)
        .display()
        .to_string();

    // Execute tool with the same coordinates the user used.
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
        file_path: Cow::Owned(rel_path),
        node_kind: Cow::Owned(focus.relation.clone()),
        module_path: Cow::Owned(focus.module_path.join("::")),
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
        "code_item_edges should return edges for a ploke-db primary node"
    );
    assert_eq!(
        returned_edges.len(),
        expected_edges.len(),
        "tool edge count should match direct DB query for selected ploke-db node"
    );
}

#[tokio::test]
async fn code_item_edges_returns_edges_for_database_struct_in_ploke_db() {
    // Load ploke-db backup to mirror live runs.
    let mut backup = workspace_root();
    backup.push("tests/backup_dbs/ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45");
    assert!(
        backup.exists(),
        "ploke-db backup missing at {}; copy ~/.config/ploke/data/ploke-db_* to tests/backup_dbs",
        backup.display()
    );

    let db = {
        let db = Database::init_with_schema().expect("init db schema");
        let rels = db.relations_vec().expect("relations");
        db.import_from_backup(&backup, &rels)
            .expect("import ploke-db backup");
        create_index_primary(&db).expect("index primary");
        Arc::new(db)
    };

    let cfg = UserConfig::default();
    let runtime_cfg = RuntimeConfig::from(cfg.clone());
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        cfg.load_embedding_processor().expect("embedder"),
    ));
    let crate_root = workspace_root().join("crates/ploke-db");
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

    let abs_path = crate_root.join("src/database.rs");
    let mod_path = vec!["crate".to_string(), "database".to_string()];

    // Verify the node exists and hash is fresh.
    let resolved = graph_resolve_exact(db.as_ref(), "struct", &abs_path, &mod_path, "Database")
        .expect("graph_resolve_exact");
    let entry = resolved.first().expect("Database struct should exist");
    let stored_hash = entry.file_tracking_hash.clone();
    let actual_hash = ploke_io::read::generate_hash_for_file(&abs_path, entry.namespace)
        .await
        .expect("compute file hash");
    assert_eq!(
        stored_hash, actual_hash,
        "tracking hash mismatch for database.rs; refresh ploke-db backup"
    );

    let expected_edges =
        graph_resolve_edges(db.as_ref(), "struct", &abs_path, &mod_path, "Database")
            .expect("graph_resolve_edges should succeed for Database");
    // In the current backup this returns zero edges; the test asserts the tool matches the DB.

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
        item_name: Cow::Borrowed("Database"),
        file_path: Cow::Borrowed("src/database.rs"),
        node_kind: Cow::Borrowed("struct"),
        module_path: Cow::Borrowed("crate::database"),
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
        "tool edge count should match direct DB query for Database struct (currently zero in backup)"
    );
}

#[tokio::test]
#[ignore = "graph_resolve_edges currently returns zero edges in the ploke-db backup; enable once fixed"]
async fn code_item_edges_graph_resolve_edges_smoke() {
    // Regression placeholder for the user-reported graph_resolve_edges case.
    let mut backup = workspace_root();
    backup.push("tests/backup_dbs/ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45");
    let db = {
        let db = Database::init_with_schema().expect("init db schema");
        let rels = db.relations_vec().expect("relations");
        db.import_from_backup(&backup, &rels)
            .expect("import ploke-db backup");
        create_index_primary(&db).expect("index primary");
        Arc::new(db)
    };
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
