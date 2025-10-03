use std::path::PathBuf;
use std::sync::Arc;

use ploke_core::ArcStr;
use ploke_tui::{
    app_state::core::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState},
    event_bus::EventBusCaps,
    EventBus,
};

#[test]
fn resolve_in_crate_root_unit_cases() {
    use ploke_tui::utils::path_scoping::resolve_in_crate_root;

    let root = PathBuf::from("/tmp/ploke_test_root");

    // Relative path inside
    let p = resolve_in_crate_root("src/lib.rs", &root).expect("join ok");
    assert!(p.starts_with(&root), "relative inside should be under root");

    // Absolute path inside should be accepted and unchanged
    let inside_abs = PathBuf::from("/tmp/ploke_test_root/src/lib.rs");
    let p2 = resolve_in_crate_root(&inside_abs, &root).expect("abs inside ok");
    assert_eq!(p2, inside_abs, "abs inside should be preserved");

    // Relative escape should be rejected (.. outside root)
    let esc = resolve_in_crate_root("../outside.rs", &root);
    assert!(
        esc.is_err(),
        "relative paths escaping root must be rejected"
    );

    // Absolute path outside should be rejected
    let outside_abs = PathBuf::from("/tmp/outside_ploke_test.rs");
    let out = resolve_in_crate_root(&outside_abs, &root);
    assert!(out.is_err(), "abs outside must be rejected");
}

#[tokio::test]
async fn create_file_rejects_outside_root_e2e() {
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
        db,
        embedder,
        io_handle,
        rag: None,
        budget: ploke_rag::TokenBudget::default(),
        proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        create_proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    });
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    // Set crate root focus
    let crate_root = std::env::temp_dir().join("ploke_t_path_scope_root").join(uuid::Uuid::new_v4().to_string());
    state.system.set_crate_focus_for_test(crate_root.clone()).await;

    // Build params with absolute path outside the crate root
    let outside_abs = std::env::temp_dir().join("outside_ploke_test.rs");
    let args = serde_json::json!({
        "file_path": outside_abs.display().to_string(),
        "content": "// test file\n",
        "on_exists": "error",
        "create_parents": true,
    });

    // Subscribe to realtime events to assert failure
    let mut rx = event_bus.realtime_tx.subscribe();

    let params = ploke_tui::rag::utils::ToolCallParams {
        state: state.clone(),
        event_bus: event_bus.clone(),
        request_id: uuid::Uuid::new_v4(),
        parent_id: uuid::Uuid::new_v4(),
        name: ploke_tui::tools::ToolName::CreateFile,
        arguments: args,
        call_id: ArcStr::from("test-call"),
    };

    // Execute tool
    ploke_tui::rag::tools::create_file_tool(params.clone()).await;

    // Expect a ToolCallFailed event for this request_id
    use tokio::time::{timeout, Duration};
    let got_failed = timeout(Duration::from_secs(1), async {
        loop {
            match rx.recv().await {
                Ok(ploke_tui::AppEvent::System(ploke_tui::app_state::events::SystemEvent::ToolCallFailed { request_id, .. })) => {
                    // Match on our call id by request_id
                    break request_id == params.request_id;
                }
                Ok(_) => continue,
                Err(_) => break false,
            }
        }
    })
    .await
    .unwrap_or(false);

    assert!(got_failed, "expected ToolCallFailed for outside-root path");
}
