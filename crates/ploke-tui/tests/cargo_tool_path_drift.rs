use std::{collections::HashMap, sync::Arc};

use ploke_core::ArcStr;
use ploke_db::Database;
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::IoManagerHandle;
use ploke_rag::TokenBudget;
use ploke_test_utils::workspace_root;
use ploke_tui::{
    EventBus,
    app_state::{
        core::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState},
        SystemStatus,
        test_set_crate_focus_from_db,
    },
    chat_history::ChatHistory,
    event_bus::EventBusCaps,
    tools::{
        Ctx,
        Tool,
        cargo::{CargoCommand, CargoScope, CargoTool, CargoToolParams},
    },
    user_config::UserConfig,
};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

#[tokio::test]
async fn cargo_tool_fails_when_db_root_outside_workspace() {
    let mut backup = workspace_root();
    backup.push("tests/backup_dbs/ploke-db-2025-12-20-abs-drift");
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
        Arc::new(db)
    };

    let cfg = UserConfig::default();
    let runtime_cfg = RuntimeConfig::from(cfg.clone());
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        cfg.load_embedding_processor().expect("embedder"),
    ));
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

    test_set_crate_focus_from_db(&state, "ploke-db".to_string())
        .await
        .expect("set crate focus from db");

    let focused_root = state
        .system
        .crate_focus_for_test()
        .await
        .expect("crate focus set");
    assert!(
        !focused_root.starts_with(&workspace_root()),
        "expected backup to point at a different workspace root; update the fixture if this changes"
    );

    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let ctx = Ctx {
        state: state.clone(),
        event_bus,
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        call_id: ArcStr::from("call"),
    };

    let params = CargoToolParams {
        command: CargoCommand::Check,
        scope: CargoScope::Focused,
        package: None,
        features: None,
        all_features: false,
        no_default_features: false,
        target: None,
        profile: None,
        release: false,
        lib: false,
        tests: false,
        bins: false,
        examples: false,
        benches: false,
        test_args: None,
    };

    let err = <CargoTool as Tool>::execute(params, ctx)
        .await
        .expect_err("path drift should error before running cargo");
    let msg = err.to_string();
    assert!(
        msg.contains("outside the current workspace"),
        "unexpected error message: {msg}"
    );
}
