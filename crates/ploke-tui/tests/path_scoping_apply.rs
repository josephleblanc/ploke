use std::path::PathBuf;
use std::sync::Arc;

use ploke_core::ArcStr;
use ploke_tui::{
    app_state::core::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState},
    event_bus::EventBusCaps,
    EventBus,
};

#[tokio::test]
async fn apply_code_edit_rejects_outside_root_e2e() {
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
    let crate_root = std::env::temp_dir().join("ploke_t_path_scope_root_apply").join(uuid::Uuid::new_v4().to_string());
    state.system.set_crate_focus_for_test(crate_root.clone()).await;

    // Prepare apply_code_edit params with splice using absolute outside path
    let outside_abs = std::env::temp_dir().join("outside_apply_test.rs");
    let req = ploke_tui::rag::utils::ApplyCodeEditRequest {
        edits: vec![ploke_tui::rag::utils::Edit::Splice {
            file_path: outside_abs.display().to_string(),
            expected_file_hash: ploke_core::TrackingHash(uuid::Uuid::new_v4()),
            start_byte: 0,
            end_byte: 0,
            replacement: "".to_string(),
            namespace: ploke_core::PROJECT_NAMESPACE_UUID,
        }],
        confidence: None,
    };
    let args = serde_json::to_value(&req).expect("serialize request");

    // Subscribe to realtime events
    let mut rx = event_bus.realtime_tx.subscribe();

    let params = ploke_tui::rag::utils::ToolCallParams {
        state: state.clone(),
        event_bus: event_bus.clone(),
        request_id: uuid::Uuid::new_v4(),
        parent_id: uuid::Uuid::new_v4(),
        name: ploke_tui::tools::ToolName::ApplyCodeEdit,
        arguments: args,
        call_id: ArcStr::from("test-call"),
    };

    // Execute tool
    ploke_tui::rag::tools::apply_code_edit_tool(params.clone()).await;

    // Expect a ToolCallFailed event for this request_id
    use tokio::time::{timeout, Duration};
    let got_failed = timeout(Duration::from_secs(1), async {
        loop {
            match rx.recv().await {
                Ok(ploke_tui::AppEvent::System(ploke_tui::app_state::events::SystemEvent::ToolCallFailed { request_id, .. })) => {
                    break request_id == params.request_id;
                }
                Ok(_) => continue,
                Err(_) => break false,
            }
        }
    })
    .await
    .unwrap_or(false);

    assert!(got_failed, "expected ToolCallFailed for outside-root path in apply_code_edit");
}
