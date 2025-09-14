//! Test harness for running the App with a headless ratatui backend
//! and realistic subsystems (state_manager, llm_manager) against the
//! shared fixture database.

use lazy_static::lazy_static;
use ploke_core::ArcStr;
use ploke_tui::app_state::events::SystemEvent;
use ploke_tui::app_state::SystemStatus;
use ploke_tui::test_harness::openrouter_env;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, oneshot};

use ratatui::{Terminal, backend::TestBackend};
use tokio_stream::wrappers::UnboundedReceiverStream;

use ploke_tui as tui;
use tui::app::{App, RunOptions};
use tui::app_state::{self, AppState, ChatState, ConfigState, StateCommand, SystemState};
use tui::llm::llm_manager;
use tui::user_config::{UserConfig, default_model};
use tui::{AppEvent, EventBus, EventBusCaps, EventPriority};

use ploke_db::{Database, bm25_index, create_index_primary};
use ploke_embed::cancel_token::CancellationToken;
use ploke_embed::indexer::IndexerTask;
use ploke_rag::{RagConfig, RagService, TokenBudget};
use ploke_test_utils::workspace_root;
use uuid::Uuid;

lazy_static! {
    /// Shared DB restored from a backup of `fixture_nodes` (if present), with primary index created.
    pub static ref TEST_DB_NODES: Result<Arc<Database>, ploke_error::Error> = {
        let db = Database::init_with_schema()?;
        let mut backup = workspace_root();
        backup.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
        if backup.exists() {
            let prior_rels_vec = db.relations_vec()?;
            db.import_from_backup(&backup, &prior_rels_vec)
                .map_err(ploke_db::DbError::from)
                .map_err(ploke_error::Error::from)?;
        }
        create_index_primary(&db)?;
        Ok(Arc::new(db))
    };
}

/// A running, headless app instance with realistic subsystems and handy senders.
pub struct AppHarness {
    pub state: Arc<AppState>,
    pub event_bus: Arc<EventBus>,
    pub cmd_tx: mpsc::Sender<StateCommand>,
    pub input_tx:
        tokio::sync::mpsc::UnboundedSender<Result<crossterm::event::Event, std::io::Error>>,
    app_task: tokio::task::JoinHandle<()>,
}

impl AppHarness {
    /// Spawn the App with TestBackend, state_manager, and llm_manager.
    pub async fn spawn() -> color_eyre::Result<Self> {
        // Config + registry
        let mut config = UserConfig::default();
        config.registry = config.registry.with_defaults();
        config.registry.load_api_keys();
        if let Some(openrouter_env) = openrouter_env() {
            config.registry.set_openrouter_key(&openrouter_env.key)
        }
        let runtime_cfg: app_state::core::RuntimeConfig = config.clone().into();

        // DB from shared fixture
        let db_handle = TEST_DB_NODES
            .as_ref()
            .expect("TEST_DB_NODES must initialize")
            .clone();

        // IO + EventBus
        let io_handle = ploke_io::IoManagerHandle::new();
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

        // Embedder + Indexer
        let processor = config
            .load_embedding_processor()
            .expect("load embedding processor");
        let proc_arc = Arc::new(processor);
        let bm25_cmd = bm25_index::bm25_service::start(Arc::clone(&db_handle), 0.0)
            .expect("start bm25 service");
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
        // Use the path for the target fixture that was used to create the backup database in
        // TEST_DB_NODES
        let system = SystemState::new(SystemStatus::new(Some(
            PathBuf::from_str("tests/fixture_crates/fixture_nodes")
                .expect("incorrect fixture format"),
        )));

        // Shared app state
        let state = Arc::new(AppState {
            chat: ChatState::new(ploke_tui::chat_history::ChatHistory::new()),
            config: ConfigState::new(runtime_cfg),
            system,
            indexing_state: RwLock::new(None),
            indexer_task: Some(Arc::clone(&indexer_task)),
            indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
            db: db_handle,
            embedder: Arc::clone(&proc_arc),
            io_handle,
            proposals: RwLock::new(std::collections::HashMap::new()),
            rag,
            budget: TokenBudget::default(),
        });

        // Command channel + state manager
        let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(1024);
        let (rag_event_tx, _rag_event_rx) = mpsc::channel(10);
        {
            let state_c = state.clone();
            let eb_c = event_bus.clone();
            tokio::spawn(ploke_tui::app_state::state_manager(
                state_c,
                cmd_rx,
                eb_c,
                rag_event_tx,
            ));
        }

        // LLM manager (background subscriber)
        {
            let eb_bg = event_bus.subscribe(EventPriority::Background);
            let state_c = state.clone();
            let eb_c = event_bus.clone();
            let cmd_c = cmd_tx.clone();
            tokio::spawn(llm_manager(eb_bg, state_c, cmd_c, eb_c));
        }

        // App + headless terminal + synthetic input stream
        let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel::<
            Result<crossterm::event::Event, std::io::Error>,
        >();
        let input = UnboundedReceiverStream::new(input_rx);
        let backend = TestBackend::new(80, 24);
        let terminal = Terminal::new(backend).expect("terminal");
        let command_style = config.command_style;
        let app = App::new(
            command_style,
            state.clone(),
            cmd_tx.clone(),
            &event_bus,
            default_model(),
        );
        let app_task = tokio::spawn(async move {
            let _ = app
                .run_with(
                    terminal,
                    input,
                    RunOptions {
                        setup_terminal_modes: false,
                    },
                )
                .await;
        });

        let harness = AppHarness {
            state,
            event_bus,
            cmd_tx,
            input_tx,
            app_task,
        };
        Ok(harness)
    }

    /// Submit a realistic user message and kick the RAG pipeline, returning the new message id.
    pub async fn add_user_msg(&self, content: impl Into<String>) -> Uuid {
        let new_user_msg_id = Uuid::new_v4();
        let (completion_tx, completion_rx) = oneshot::channel();
        let (scan_tx, scan_rx) = oneshot::channel();
        let _ = self
            .cmd_tx
            .send(StateCommand::AddUserMessage {
                content: content.into(),
                new_user_msg_id,
                completion_tx,
            })
            .await;
        let _ = self
            .cmd_tx
            .send(StateCommand::ScanForChange { scan_tx })
            .await;
        let _ = self
            .cmd_tx
            .send(StateCommand::EmbedMessage {
                new_msg_id: new_user_msg_id,
                completion_rx,
                scan_rx,
            })
            .await;
        new_user_msg_id
    }

    /// Emit a synthesized ToolEvent::Completed (typed path) for convenience.
    pub fn emit_tool_completed(
        &self,
        request_id: Uuid,
        parent_id: Uuid,
        call_id: ArcStr,
        content: impl Into<String>,
    ) {
        self.event_bus
            .send(AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id,
                parent_id,
                call_id,
                content: content.into(),
            }));
    }

    /// Subscribe to test harness API response events for validation.
    #[cfg(all(feature = "test_harness", feature = "live_api_tests"))]
    pub fn subscribe_api_responses(&self) -> tokio::sync::broadcast::Receiver<AppEvent> {
        self.event_bus.subscribe(EventPriority::Realtime)
    }

    /// Gracefully shut down the app (sends Quit and waits for completion).
    pub async fn shutdown(self) {
        self.event_bus.send(AppEvent::Quit);
        let _ = self.app_task.await;
    }
}

/// Read the current buffer of a TestBackend terminal into a string grid.
/// Useful for snapshot assertions when you control the `Terminal`.
pub fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
    let buf = term.backend().buffer();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let ch = buf
                .cell((x, y))
                .expect("in-bounds")
                .symbol()
                .chars()
                .next()
                .unwrap_or(' ');
            out.push(ch);
        }
        out.push('\n');
    }
    out
}
#![cfg(not(feature = "llm_refactor"))]
