#![allow(dead_code)]

use lazy_static::lazy_static;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, mpsc};

use crate::{
    EventBus, EventBusCaps,
    app::App,
    app_state::{self, AppState, ChatState, ConfigState, StateCommand, SystemState},
    chat_history::ChatHistory,
    user_config::{UserConfig, default_model},
};
use ploke_db::{bm25_index, create_index_primary};
use ploke_embed::{cancel_token::CancellationToken, indexer::IndexerTask};
use ploke_rag::{RagConfig, RagService, TokenBudget};
use ploke_test_utils::workspace_root;

lazy_static! {
    /// A globally accessible App instance for tests, wrapped in Arc<Mutex<...>>.
    pub static ref TEST_APP: Arc<Mutex<App>> = {
        // Build a realistic App instance without spawning UI/event loops.
        // Keep this synchronous for ergonomic use in tests.
        let mut config = UserConfig::default();
        // Merge curated defaults with user overrides (none in tests by default)
        config.registry = config.registry.with_defaults();
        // Apply any API keys from env for more realistic behavior if present
        config.registry.load_api_keys();

        // Convert to runtime configuration
        let runtime_cfg: app_state::core::RuntimeConfig = config.clone().into();

        // Initialize an in-memory database with schema; optionally restore a pre-loaded backup for realistic tests
        let mut db = ploke_db::Database::init_with_schema().expect("init test db");

        // Prefer env override; otherwise use the standard fixture backup path if it exists
        let backup_path = std::env::var("PLOKE_TEST_DB_BACKUP")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                let mut p = workspace_root();
                p.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
                p
            });

        if backup_path.exists() {
            let prior_rels_vec = db.relations_vec().expect("relations_vec");
            db.import_from_backup(&backup_path, &prior_rels_vec)
                .expect("import_from_backup");
        }
        // Ensure primary index exists for consistent behavior in tests using Rag/DB lookups
        create_index_primary(&db).expect("create primary index");

        let db_handle = Arc::new(db);

        // IO manager
        let io_handle = ploke_io::IoManagerHandle::new();

        // Event bus for the app
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

        // Embedder (from config)
        let processor = config
            .load_embedding_processor()
            .expect("load embedding processor");
        let proc_arc = Arc::new(processor);

        // BM25 service (used by indexer/RAG)
        let bm25_cmd = bm25_index::bm25_service::start(Arc::clone(&db_handle), 0.0)
            .expect("start bm25 service");

        // Indexer task
        let indexer_task = IndexerTask::new(
            db_handle.clone(),
            io_handle.clone(),
            Arc::clone(&proc_arc),
            CancellationToken::new().0,
            8,
        )
        .with_bm25_tx(bm25_cmd);
        let indexer_task = Arc::new(indexer_task);

        // RAG service (optional)
        let rag = match RagService::new_full(
            db_handle.clone(),
            Arc::clone(&proc_arc),
            io_handle.clone(),
            RagConfig::default(),
        ) {
            Ok(svc) => Some(Arc::new(svc)),
            Err(_e) => None,
        };

        // Shared app state
        let state = Arc::new(AppState {
            chat: ChatState::new(ChatHistory::new()),
            config: ConfigState::new(runtime_cfg),
            system: SystemState::default(),
            indexing_state: RwLock::new(None),
            indexer_task: Some(Arc::clone(&indexer_task)),
            indexing_control: Arc::new(Mutex::new(None)),
            db: db_handle,
            embedder: Arc::clone(&proc_arc),
            io_handle: io_handle.clone(),
            proposals: RwLock::new(std::collections::HashMap::new()),
            rag,
            budget: TokenBudget::default(),
        });

        // Command channel (not wired to a state_manager loop in tests)
        let (cmd_tx, _cmd_rx) = mpsc::channel::<StateCommand>(1024);

        // Build the App
        let command_style = config.command_style;
        let app = App::new(command_style, state, cmd_tx, &event_bus, default_model());

        Arc::new(Mutex::new(app))
    };
}

/// Convenience accessor for the global test App.
pub fn app() -> &'static Arc<Mutex<App>> {
    &TEST_APP
}
