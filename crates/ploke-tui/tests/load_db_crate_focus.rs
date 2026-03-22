use std::collections::BTreeMap;
use std::sync::Arc;

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_test_utils::{WS_FIXTURE_01_CANONICAL, fresh_backup_fixture_db, workspace_root};
use ploke_tui::EventBus;
use ploke_tui::app_state::core::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState};
use ploke_tui::event_bus::EventBusCaps;

#[tokio::test]
async fn crate_focus_assigns_absolute_root_from_db() {
    // Build minimal app state
    let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
    let io_handle = ploke_io::IoManagerHandle::new();
    let cfg = ploke_tui::user_config::UserConfig::default();
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        cfg.load_embedding_processor().expect("embedder"),
    ));
    let state = Arc::new(AppState {
        chat: ChatState::new(ploke_tui::chat_history::ChatHistory::new()),
        config: ConfigState::new(RuntimeConfig::from(cfg.clone())),
        system: SystemState::default(),
        indexing_state: tokio::sync::RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
        db: db.clone(),
        embedder,
        io_handle,
        rag: None,
        budget: ploke_rag::TokenBudget::default(),
        proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        create_proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    });
    let _event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    // Insert a crate_context row with absolute root_path
    let ns = uuid::Uuid::new_v4();
    let name = format!("crate_{}", uuid::Uuid::new_v4().simple());
    let version = "0.1.0".to_string();
    let abs_root = std::env::temp_dir()
        .join("ploke_focus_root")
        .join(uuid::Uuid::new_v4().to_string());
    let root_str = abs_root.display().to_string();

    // Build params map matching CrateContextSchema fields
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), DataValue::Uuid(UuidWrapper(ns)));
    params.insert("name".to_string(), DataValue::from(name.as_str()));
    params.insert("version".to_string(), DataValue::from(version.as_str()));
    params.insert("namespace".to_string(), DataValue::Uuid(UuidWrapper(ns)));
    params.insert("root_path".to_string(), DataValue::from(root_str.as_str()));
    params.insert("files".to_string(), DataValue::List(vec![]));

    let script =
        ploke_transform::schema::crate_node::CrateContextSchema::SCHEMA.script_put(&params);
    db.run_script(&script, params, ScriptMutability::Mutable)
        .expect("put crate_context");

    // Call the test-only helper to set crate_focus from DB
    ploke_tui::app_state::test_set_crate_focus_from_db(&state, name.clone())
        .await
        .expect("set crate focus");

    // Verify focused root equals absolute root (no current_dir join)
    let got = state
        .system
        .crate_focus_for_test()
        .await
        .expect("crate_focus set by test");
    assert_eq!(got, abs_root);
}

#[tokio::test]
async fn workspace_restore_assigns_loaded_workspace_membership_from_db() {
    let db = Arc::new(
        fresh_backup_fixture_db(&WS_FIXTURE_01_CANONICAL).expect("load workspace backup fixture"),
    );
    let io_handle = ploke_io::IoManagerHandle::new();
    let cfg = ploke_tui::user_config::UserConfig::default();
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        cfg.load_embedding_processor().expect("embedder"),
    ));
    let state = Arc::new(AppState {
        chat: ChatState::new(ploke_tui::chat_history::ChatHistory::new()),
        config: ConfigState::new(RuntimeConfig::from(cfg.clone())),
        system: SystemState::default(),
        indexing_state: tokio::sync::RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
        db: db.clone(),
        embedder,
        io_handle,
        rag: None,
        budget: ploke_rag::TokenBudget::default(),
        proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        create_proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    });
    let _event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    ploke_tui::app_state::test_set_crate_focus_from_db(&state, "ws_fixture_root".to_string())
        .await
        .expect("set workspace member focus");

    let fixture_workspace_root = workspace_root().join("tests/fixture_workspace/ws_fixture_01");
    let expected_member_roots = vec![
        fixture_workspace_root.join("member_root"),
        fixture_workspace_root.join("nested/member_nested"),
    ];

    let loaded_workspace_root = state
        .system
        .loaded_workspace_root_for_test()
        .await
        .expect("loaded workspace root");
    assert_eq!(loaded_workspace_root, fixture_workspace_root);

    let loaded_member_roots = state.system.loaded_workspace_member_roots_for_test().await;
    assert_eq!(loaded_member_roots, expected_member_roots);

    let loaded_roots = state.system.loaded_workspace_member_roots_for_test().await;
    assert!(
        loaded_roots.contains(&fixture_workspace_root.join("member_root")),
        "member_root should be in loaded crates"
    );

    let policy_roots = {
        let guard = state.system.read().await;
        guard
            .derive_path_policy(&[])
            .expect("workspace policy")
            .roots
    };
    let mut expected_policy_roots = vec![fixture_workspace_root.clone()];
    expected_policy_roots.extend(expected_member_roots.iter().cloned());
    assert_eq!(policy_roots, expected_policy_roots);
}
