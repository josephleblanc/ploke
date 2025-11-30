use std::collections::BTreeMap;
use std::sync::Arc;

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use ploke_tui::EventBus;
use ploke_tui::app_state::core::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState};
use ploke_tui::event_bus::EventBusCaps;

#[tokio::test]
async fn crate_focus_assigns_absolute_root_from_db() {
    // Build minimal app state
    let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
    let io_handle = ploke_io::IoManagerHandle::new();
    let cfg = ploke_tui::user_config::UserConfig::default();
    let embedder = Arc::new(cfg.load_embedding_processor().expect("embedder"));
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

    // Verify crate_focus equals absolute root (no current_dir join)
    let got = state
        .system
        .crate_focus_for_test()
        .await
        .expect("crate_focus set by test");
    assert_eq!(got, abs_root);
}
