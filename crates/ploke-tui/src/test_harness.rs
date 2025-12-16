#![allow(dead_code)]

use lazy_static::lazy_static;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, mpsc};

use crate::{
    AppEvent, EventBus, EventBusCaps, EventPriority,
    app::App,
    app_state::{self, AppState, ChatState, ConfigState, StateCommand, SystemState, state_manager},
    chat_history::ChatHistory,
    default_model,
    file_man::FileManager,
    llm::manager::llm_manager,
    observability, run_event_bus,
    user_config::{OPENROUTER_URL, UserConfig, openrouter_url},
};
use ploke_db::{bm25_index, create_index_primary};
use ploke_embed::{cancel_token::CancellationToken, indexer::IndexerTask};
use ploke_rag::{RagConfig, RagService, TokenBudget};
use ploke_test_utils::workspace_root;

#[cfg(feature = "test_harness")]
lazy_static! {
    /// A globally accessible App instance for tests, wrapped in Arc<Mutex<...>>.
    pub static ref TEST_APP: Arc<Mutex<App>> = {
        // Build a realistic App instance without spawning UI/event loops.
        // Keep this synchronous for ergonomic use in tests.
        let config = UserConfig::default();
        // Registry defaults are already represented by `Default` impls in llm; API keys are
        // resolved by routers at call time from env in tests.

        // Convert to runtime configuration
        let runtime_cfg: app_state::core::RuntimeConfig = config.clone().into();

        // Initialize an in-memory database with schema; optionally restore a pre-loaded backup for realistic tests
        let db = ploke_db::Database::init_with_schema().expect("init test db");

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

        // Embedder (from config) and shared runtime
        let processor = config
            .load_embedding_processor()
            .expect("load embedding processor");
        let embedding_runtime =
            Arc::new(ploke_embed::runtime::EmbeddingRuntime::with_default_set(processor));
        // Share the active set handle with the database
        let db_handle = Arc::new({
            let mut db = db;
            db.active_embedding_set = embedding_runtime.active_set_handle();
            db
        });

        // IO manager
        let io_handle = ploke_io::IoManagerHandle::new();

        // Event bus for the app
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

        // BM25 service (used by indexer/RAG)
        let bm25_cmd = bm25_index::bm25_service::start(Arc::clone(&db_handle), 0.0)
            .expect("start bm25 service");

        // Indexer task
        let (index_cancellation_token, index_cancel_handle) = CancellationToken::new();
        let indexer_task = IndexerTask::new(
            db_handle.clone(),
            io_handle.clone(),
            Arc::clone(&embedding_runtime),
            index_cancellation_token,
            index_cancel_handle,
            8,
        )
        .with_bm25_tx(bm25_cmd);
        let indexer_task = Arc::new(indexer_task);

        // RAG service (optional)
        let rag = match RagService::new_full(
            db_handle.clone(),
            Arc::clone(&embedding_runtime),
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
            embedder: Arc::clone(&embedding_runtime),
            io_handle: io_handle.clone(),
            proposals: RwLock::new(std::collections::HashMap::new()),
            create_proposals: RwLock::new(std::collections::HashMap::new()),
            rag,
            budget: TokenBudget::default(),
        });

        let (rag_event_tx, rag_event_rx) = mpsc::channel(10);
        // Create command channel with backpressure
        let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(1024);

        let (cancellation_token, cancel_handle) = CancellationToken::new();
        let (filemgr_tx, filemgr_rx) = mpsc::channel::<AppEvent>(256);
        let file_manager = FileManager::new(
            io_handle.clone(),
            event_bus.subscribe(EventPriority::Background),
            event_bus.background_tx.clone(),
            rag_event_tx.clone(),
            event_bus.realtime_tx.clone(),
        );

        tokio::spawn(file_manager.run());

        tokio::spawn(state_manager(
            state.clone(),
            cmd_rx,
            event_bus.clone(),
            rag_event_tx,
        ));

        // Build the App
        // Spawn subsystems with backpressure-aware command sender
        let command_style = config.command_style;
        tokio::spawn(llm_manager(
            event_bus.subscribe(EventPriority::Background),
            state.clone(),
            cmd_tx.clone(), // Clone for each subsystem
            event_bus.clone(),
        ));
        tokio::spawn(run_event_bus(Arc::clone(&event_bus)));
        tokio::spawn(observability::run_observability(
            event_bus.clone(),
            state.clone(),
        ));
        let app = App::new(command_style, state, cmd_tx, &event_bus, default_model());

        Arc::new(Mutex::new(app))
    };
}

/// Convenience accessor for the global test App.
#[cfg(feature = "test_harness")]
pub fn app() -> &'static Arc<Mutex<App>> {
    &TEST_APP
}

/// Accessor for the shared AppState used by TEST_APP.
/// This provides a clone of the Arc<AppState> so integration tests can stage
/// proposals or inspect state efficiently without recreating the app.
#[cfg(feature = "test_harness")]
pub async fn get_state() -> Arc<AppState> {
    let app = TEST_APP.lock().await;
    app.test_get_state()
}

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
pub fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    let referer = HeaderName::from_static("http-referer");
    let x_title = HeaderName::from_static("x-title");
    headers.insert(
        referer,
        HeaderValue::from_static("https://github.com/ploke-ai/ploke"),
    );
    headers.insert(x_title, HeaderValue::from_static("Ploke TUI E2E Tests"));
    headers
}

pub struct OpenRouterEnv {
    pub key: String,
    pub base_url: reqwest::Url,
}
impl OpenRouterEnv {
    pub fn new(key: String, base_url: reqwest::Url) -> Self {
        Self { key, base_url }
    }
}

pub fn openrouter_env() -> Option<OpenRouterEnv> {
    // Try current process env first; if missing, load from .env as a fallback
    let key_opt = std::env::var("OPENROUTER_API_KEY").ok();
    let key = match key_opt {
        Some(k) if !k.trim().is_empty() => k,
        _ => {
            let _ = dotenvy::dotenv();
            let k = std::env::var("OPENROUTER_API_KEY").ok()?;
            if k.trim().is_empty() {
                return None;
            }
            k
        }
    };
    Some(OpenRouterEnv::new(key, openrouter_url()))
}

// Re-export AppHarness for integration tests in tests/ directory
#[cfg(feature = "test_harness")]
pub use crate::test_utils::new_test_harness::AppHarness;
