#![allow(unused_variables, unused_imports, dead_code, private_interfaces)]
//! ploke-tui main library entry.
//!
//! Dataflow overview:
//! - Config load: `try_main` reads config (toml/env), and spins up subsystems.
//! - Commands: UI routes parsed commands to `StateCommand` via channels; model/provider
//!   commands update `ModelRegistry` and emit `SystemEvent::ModelSwitched`.
//! - Persistence: config can be saved/loaded atomically with optional key redaction.
//!
//! Subsystems started in `try_main`:
//! - State manager (app_state), LLM manager, EventBus, Observability, FileManager,
//!   Indexer, optional RAG service.

pub mod app;
pub mod app_state;
pub mod chat_history;
pub mod error;
pub mod event_bus;
pub mod file_man;
pub mod observability;
pub mod parser;
pub mod tracing_setup;
pub mod utils;
pub use event_bus::*;
pub mod rag;
#[cfg(test)]
mod tests;
pub mod tools;

pub mod llm;

pub mod user_config;

// use llm::{EndpointsResponse, ModelId,
//     manager::events::ChatEvt,
//     router_only::default_model};
use ploke_llm::{EndpointsResponse, ModelId, router_only::default_model};

pub mod test_utils;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
pub use test_utils::mock;

use ploke_core::{ArcStr, rag_types::AssembledContext};

pub mod test_harness;

use app::App;
use app_state::{
    AppState, ChatState, ConfigState, MessageUpdatedEvent, StateCommand, SystemState,
    core::RuntimeConfig, events::SystemEvent, state_manager,
};
use error::{ErrorExt, ErrorSeverity, ResultExt};
use file_man::FileManager;
use parser::run_parse;
use ploke_db::{
    bm25_index::{self, Bm25Indexer, bm25_service::Bm25Cmd},
    multi_embedding::db_ext::EmbeddingExt,
};
use ploke_embed::{
    cancel_token::CancellationToken,
    indexer::{self, IndexStatus, IndexerTask, IndexingStatus},
};
use ploke_rag::{RrfConfig, TokenBudget};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock, broadcast, mpsc};
use tracing::instrument;
use ui::UiEvent;
use user_config::{OPENROUTER_URL, UserConfig};
use utils::layout::layout_statusline;

use std::{collections::HashMap, sync::Arc};

use chat_history::{ChatHistory, UpdateFailedEvent};
use color_eyre::Result;
use crossterm::event::{DisableBracketedPaste, DisableFocusChange, DisableMouseCapture};
use futures::{FutureExt, StreamExt};
use once_cell::sync::Lazy;
use ratatui::{
    DefaultTerminal, Frame,
    style::Stylize,
    widgets::{Block, Borders, ListItem, ListState, Padding, Paragraph},
};
// for list
use crate::llm::{ChatEvt, LlmEvent};
use ratatui::prelude::*;
use ratatui::{style::Style, widgets::List};
use uuid::Uuid;

pub static TARGET_DIR_FIXTURE: &str = "fixture_tracking_hash";

static GLOBAL_EVENT_BUS: Lazy<Mutex<Option<Arc<EventBus>>>> = Lazy::new(|| Mutex::new(None));

pub const TOP_K: usize = 15;
lazy_static! {
    static ref RETRIEVAL_STRATEGY: ploke_rag::RetrievalStrategy =
        ploke_rag::RetrievalStrategy::Hybrid {
            rrf: RrfConfig::default(),
            mmr: None,
        };
}

/// The number of tool retries to allow if model fails to call tool correctly.
// TODO: Add this to user config
pub const TOOL_RETRIES: u32 = 2;

/// The default number of tokens per LLM request.
// TODO: Add this to user config
pub const TOKEN_LIMIT: u32 = 8196;

/// The default number of seconds for timeout on LLM request loop.
// TODO: Add this to user config
pub const LLM_TIMEOUT_SECS: u64 = 45;

/// Set the global event bus for error handling
pub async fn set_global_event_bus(event_bus: Arc<EventBus>) {
    *GLOBAL_EVENT_BUS.lock().await = Some(event_bus);
}

/// Emit an error event to the global event bus
pub async fn emit_error_event(message: String, severity: ErrorSeverity) {
    if let Some(event_bus) = GLOBAL_EVENT_BUS.lock().await.as_ref() {
        event_bus.send(AppEvent::Error(ErrorEvent { message, severity }));
    }
}

pub async fn emit_app_event(event: AppEvent) {
    if let Some(event_bus) = GLOBAL_EVENT_BUS.lock().await.as_ref() {
        event_bus.send(event);
    }
}
pub async fn try_main() -> color_eyre::Result<()> {
    dotenvy::dotenv().ok();

    // Global panic hook to restore terminal state on unexpected panics
    std::panic::set_hook(Box::new(|_info| {
        ratatui::restore();
        let _ = crossterm::execute!(
            std::io::stdout(),
            DisableBracketedPaste,
            DisableFocusChange,
            DisableMouseCapture,
        );
    }));

    let config = config::Config::builder()
        .add_source(
            config::File::with_name(
                &dirs::config_dir()
                    .unwrap() // TODO: add error handling
                    .join("ploke/config.toml")
                    .to_string_lossy(),
            )
            .required(false),
        )
        .add_source(config::Environment::default().separator("_"))
        .build()?
        .try_deserialize::<UserConfig>()
        .unwrap_or_else(|_| UserConfig::default());

    // llm: registry prefs are used directly; model lists/capabilities fetched via router APIs.
    tracing::debug!("Registry prefs loaded: {:#?}", config.registry);
    let runtime_cfg: RuntimeConfig = config.clone().into();

    let processor = config.load_embedding_processor()?;
    let embedding_runtime =
        Arc::new(ploke_embed::runtime::EmbeddingRuntime::with_default_set(processor));

    let mut new_db = ploke_db::Database::init_with_schema()?;
    new_db.setup_multi_embedding()?;
    new_db.active_embedding_set = embedding_runtime.active_set_handle();
    let db_handle = Arc::new(new_db);

    // Initial parse is now optional - user can run indexing on demand
    // run_parse(Arc::clone(&db_handle), Some(TARGET_DIR_FIXTURE.into()))?;

    // TODO: Change IoManagerHandle so it doesn't spawn its own thread, then use similar pattern to
    // spawning state meager below.
    let io_handle = ploke_io::IoManagerHandle::new();

    // TODO: These numbers should be tested for performance under different circumstances.
    let event_bus_caps = EventBusCaps::default();
    let event_bus = Arc::new(EventBus::new(event_bus_caps));

    let bm25_cmd = bm25_index::bm25_service::start(Arc::clone(&db_handle), 0.0)?;

    // TODO:
    // 1 Implement the cancellation token propagation in IndexerTask
    // 2 Add error handling for embedder initialization failures
    let indexer_task = IndexerTask::new(
        db_handle.clone(),
        io_handle.clone(),
        Arc::clone(&embedding_runtime), // Use configured processor
        CancellationToken::new().0,
        8,
    )
    .with_bm25_tx(bm25_cmd);
    let indexer_task = Arc::new(indexer_task);

    // Initialize RAG orchestration service with full capabilities (BM25 + dense + IoManager)
    let rag = match ploke_rag::RagService::new_full(
        db_handle.clone(),
        Arc::clone(&embedding_runtime),
        io_handle.clone(),
        ploke_rag::RagConfig::default(),
    ) {
        Ok(svc) => Some(Arc::new(svc)),
        Err(e) => {
            tracing::warn!("Failed to initialize RagService: {}", e);
            None
        }
    };

    // NOTE: Now that we got rid of `context_manager`, this event is unused.
    let (rag_event_tx, rag_event_rx) = mpsc::channel(10);
    // let context_manager = ContextManager::new(rag_event_rx, Arc::clone(&event_bus));
    // tokio::spawn(context_manager.run());

    let state = Arc::new(AppState {
        chat: ChatState::new(ChatHistory::new()),
        config: ConfigState::new(runtime_cfg),
        system: SystemState::default(),
        indexing_state: RwLock::new(None), // Initialize as None
        indexer_task: Some(Arc::clone(&indexer_task)),
        indexing_control: Arc::new(Mutex::new(None)),
        db: db_handle,
        embedder: Arc::clone(&embedding_runtime),
        io_handle: io_handle.clone(),
        proposals: RwLock::new(HashMap::new()),
        create_proposals: RwLock::new(HashMap::new()),
        rag,
        // TODO: Add TokenBudget fields to Config
        budget: TokenBudget::default(),
    });

    // Load persisted proposals (best-effort) before starting subsystems
    crate::app_state::handlers::proposals::load_proposals(&state).await;
    crate::app_state::handlers::proposals::load_create_proposals(&state).await;

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

    // Spawn state manager first
    tokio::spawn(state_manager(
        state.clone(),
        cmd_rx,
        event_bus.clone(),
        rag_event_tx,
    ));

    // Set global event bus for error handling
    set_global_event_bus(event_bus.clone()).await;

    // Spawn subsystems with backpressure-aware command sender
    let command_style = config.command_style;
    tokio::spawn(llm::manager::llm_manager(
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

    let terminal = ratatui::init();
    let app = App::new(command_style, state, cmd_tx, &event_bus, default_model());
    let result = app.run(terminal).await;
    ratatui::restore();
    result
}

pub mod ui {
    use uuid::Uuid;

    use crate::chat_history::NavigationDirection;

    #[derive(Clone, Debug)]
    pub enum UiEvent {
        Navigate(NavigationDirection),
        MessageSelected(Uuid),
        InputSubmitted(String),
    }
}

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum UiError {
    ExampleError,
}

impl std::fmt::Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiError::ExampleError => write!(f, "Example error occurred"),
        }
    }
}

// Other domains: file, rag, agent, system, ...

#[derive(Clone, Debug)]
pub enum RagEvent {
    ContextSnippets(Uuid, Vec<String>),
    UserMessages(Uuid, Vec<chat_history::Message>),
    ConstructContext(Uuid),
}

#[derive(Clone, Debug)]
pub enum SearchEvent {
    SearchResults {
        query_id: u64,
        context: AssembledContext,
    },
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    Ui(UiEvent),
    Llm(crate::llm::LlmEvent),
    // placeholder
    LlmTool(ploke_llm::manager::events::ToolEvent),
    // External signal to request a clean UI shutdown
    Quit,
    // TODO:
    // File(file::Event),
    // Rag(rag::Event),
    // Agent(agent::Event),
    System(SystemEvent),
    // A message was successfully updated. UI should refresh this message.
    MessageUpdated(MessageUpdatedEvent),
    Rag(RagEvent),
    ContextSearch(SearchEvent),

    // An attempt to update a message was rejected. UI should show an error.
    UpdateFailed(UpdateFailedEvent),
    Error(ErrorEvent),
    IndexingProgress(indexer::IndexingStatus),
    IndexingStarted,
    IndexingCompleted,
    IndexingFailed,
    EventBusStarted,
    GenerateContext(Uuid),
}

impl AppEvent {
    // NOTE: the split into real-time and background is a good idea, insofar as it prioritizes some
    // things meant for the UI draw function (which should stay lean), but it is pretty darn
    // confusing sometimes when both types are Sender<AppEvent> and Receiver<AppEvent>, with no
    // distinguishing characteristics inside the event themselves.
    // TODO: Change EventPriority to isntead be either a field within the event struct itself or
    // find another way to makes sure we can be more type-safe here, and avoid foot-guns.
    pub fn priority(&self) -> EventPriority {
        use ploke_llm::manager::events::ToolEvent;
        match self {
            AppEvent::Ui(_) => EventPriority::Realtime,
            AppEvent::LlmTool(ev) => match ev {
                ToolEvent::Requested { .. } => EventPriority::Background,
                ToolEvent::Completed { .. } | ToolEvent::Failed { .. } => EventPriority::Realtime,
            },
            AppEvent::Quit => EventPriority::Realtime,
            AppEvent::System(SystemEvent::ModelSwitched(_)) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::ReadQuery { .. }) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::WriteQuery { .. }) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::HistorySaved { .. }) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::BackupDb { .. }) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::LoadDb { .. }) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::ReIndex { .. }) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::ToolCallRequested { .. }) => EventPriority::Background,
            AppEvent::System(SystemEvent::ToolCallCompleted { .. }) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::ToolCallFailed { .. }) => EventPriority::Realtime,
            AppEvent::System(_) => EventPriority::Background,
            AppEvent::MessageUpdated(_) => EventPriority::Realtime,
            AppEvent::UpdateFailed(_) => EventPriority::Background,
            AppEvent::Error(_) => EventPriority::Background,
            AppEvent::IndexingProgress(_) => EventPriority::Realtime,
            AppEvent::IndexingStarted => EventPriority::Background,
            AppEvent::IndexingCompleted => EventPriority::Realtime,
            AppEvent::IndexingFailed => EventPriority::Realtime,
            AppEvent::Rag(_) => EventPriority::Background,
            AppEvent::EventBusStarted => EventPriority::Realtime,
            AppEvent::GenerateContext(_) => EventPriority::Background,
            AppEvent::Llm(llm::LlmEvent::ChatCompletion(ChatEvt::Request { .. })) => {
                EventPriority::Background
            }
            AppEvent::Llm(llm::LlmEvent::ChatCompletion(ChatEvt::Response { .. })) => {
                EventPriority::Realtime
            }
            AppEvent::Llm(llm_event) => EventPriority::Background,
            AppEvent::ContextSearch(search_event) => EventPriority::Realtime,
        }
    }
    pub fn is_system(&self) -> bool {
        matches!(self, AppEvent::System(_))
    }
}

impl From<LlmEvent> for AppEvent {
    fn from(value: LlmEvent) -> Self {
        AppEvent::Llm(value)
    }
}
