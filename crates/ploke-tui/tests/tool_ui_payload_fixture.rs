use std::{borrow::Cow, collections::HashMap, sync::Arc};

use ploke_core::ArcStr;
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
    tools::{Ctx, Tool, ToolVerbosity, ns_read::{NsRead, NsReadParams}},
    user_config::UserConfig,
};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

#[tokio::test]
async fn ns_read_tool_payload_renders_with_fixture_verbosity() {
    let db = ploke_tui::test_utils::new_test_harness::TEST_DB_NODES
        .as_ref()
        .expect("fixture db")
        .clone();

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
        state,
        event_bus,
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        call_id: ArcStr::from("call-fixture"),
    };

    let params = NsReadParams {
        file: Cow::Borrowed("src/lib.rs"),
        start_line: None,
        end_line: None,
        max_bytes: None,
    };
    let result = NsRead::execute(params, ctx).await.expect("tool execute");
    let payload = result.ui_payload.expect("ui payload");

    let mut field_map: HashMap<&str, &str> = HashMap::new();
    for field in &payload.fields {
        field_map.insert(field.name.as_ref(), field.value.as_ref());
    }
    assert_eq!(field_map.get("exists"), Some(&"true"));
    assert_eq!(field_map.get("truncated"), Some(&"false"));
    assert_eq!(field_map.get("lines"), Some(&"full"));

    let expected_summary = format!("Read {}", crate_root.join("src/lib.rs").display());

    let minimal = payload.render(ToolVerbosity::Minimal);
    assert!(minimal.contains("read_file"));
    assert!(minimal.contains(&expected_summary));
    assert!(!minimal.contains("Fields:"));

    let normal = payload.render(ToolVerbosity::Normal);
    assert!(normal.contains("Tool: read_file"));
    assert!(normal.contains(&format!("Summary: {expected_summary}")));
    assert!(normal.contains("Fields:"));
    assert!(normal.contains("- exists: true"));

    let verbose = payload.render(ToolVerbosity::Verbose);
    assert!(verbose.contains("Tool: read_file"));
    assert!(verbose.contains(&format!("Summary: {expected_summary}")));
    assert!(verbose.contains("Fields:"));
    assert!(verbose.contains("- truncated: false"));
}
