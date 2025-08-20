#![allow(unused_variables, unused_imports, dead_code)]

pub mod app;
pub mod app_state;
pub mod chat_history;
pub mod error;
pub mod event_bus;
pub mod file_man;
pub mod llm;
pub mod observability;
pub mod parser;
pub mod tracing_setup;
pub mod user_config;
pub mod utils;
pub use event_bus::*;

#[cfg(test)]
mod test_utils;

use app::App;
use app_state::{
    AppState, ChatState, ConfigState, MessageUpdatedEvent, StateCommand, SystemState, state_manager,
};
use error::{ErrorExt, ErrorSeverity, ResultExt};
use file_man::FileManager;
use llm::llm_manager;
use parser::run_parse;
use ploke_db::bm25_index::{self, Bm25Indexer, bm25_service::Bm25Cmd};
use ploke_embed::{
    cancel_token::CancellationToken,
    indexer::{self, IndexStatus, IndexerTask, IndexingStatus},
};
use ploke_rag::TokenBudget;
use system::SystemEvent;
use thiserror::Error;
use tokio::sync::{Mutex, RwLock, broadcast, mpsc};
use tracing::instrument;
use ui::UiEvent;
use user_config::{OPENROUTER_URL, ProviderConfig, ProviderType, default_model};
use utils::layout::layout_statusline;

use std::sync::Arc;

use chat_history::{ChatHistory, UpdateFailedEvent};
use color_eyre::Result;
use futures::{FutureExt, StreamExt};
use once_cell::sync::Lazy;
use ratatui::{
    DefaultTerminal, Frame,
    style::Stylize,
    widgets::{Block, Borders, ListItem, ListState, Padding, Paragraph},
};
// for list
use ratatui::prelude::*;
use ratatui::{style::Style, widgets::List};
use uuid::Uuid;

pub static TARGET_DIR_FIXTURE: &str = "fixture_tracking_hash";

static GLOBAL_EVENT_BUS: Lazy<Mutex<Option<Arc<EventBus>>>> = Lazy::new(|| Mutex::new(None));

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
pub async fn try_main() -> color_eyre::Result<()> {
    dotenvy::dotenv().ok();

    let mut config = config::Config::builder()
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
        .try_deserialize::<crate::user_config::Config>()
        .unwrap_or_else(|_| crate::user_config::Config::default());

    // Merge curated defaults with user overrides
    config.registry = config.registry.with_defaults();

    // Apply API keys from environment variables to all providers
    // config.registry.load_api_keys();
    tracing::debug!("Registry after merge: {:#?}", config.registry);
    let new_db = ploke_db::Database::init_with_schema()?;
    let db_handle = Arc::new(new_db);

    // Initial parse is now optional - user can run indexing on demand
    // run_parse(Arc::clone(&db_handle), Some(TARGET_DIR_FIXTURE.into()))?;

    // TODO: Change IoManagerHandle so it doesn't spawn its own thread, then use similar pattern to
    // spawning state meager below.
    let io_handle = ploke_io::IoManagerHandle::new();

    // TODO: These numbers should be tested for performance under different circumstances.
    let event_bus_caps = EventBusCaps::default();
    let event_bus = Arc::new(EventBus::new(event_bus_caps));

    let processor = config.load_embedding_processor()?;
    let proc_arc = Arc::new(processor);
    let bm25_cmd = bm25_index::bm25_service::start(Arc::clone(&db_handle), 0.0)?;

    // TODO:
    // 1 Implement the cancellation token propagation in IndexerTask
    // 2 Add error handling for embedder initialization failures
    let indexer_task = IndexerTask::new(
        db_handle.clone(),
        io_handle.clone(),
        Arc::clone(&proc_arc), // Use configured processor
        CancellationToken::new().0,
        8,
    )
    .with_bm25_tx(bm25_cmd);
    let indexer_task = Arc::new(indexer_task);

    // Initialize RAG orchestration service with full capabilities (BM25 + dense + IoManager)
    let rag = match ploke_rag::RagService::new_full(
        db_handle.clone(),
        Arc::clone(&proc_arc),
        io_handle.clone(),
        ploke_rag::RagConfig::default(),
    ) {
        Ok(svc) => Some(Arc::new(svc)),
        Err(e) => {
            tracing::warn!("Failed to initialize RagService: {}", e);
            None
        }
    };

    let (rag_event_tx, rag_event_rx) = mpsc::channel(10);
    // let context_manager = ContextManager::new(rag_event_rx, Arc::clone(&event_bus));
    // tokio::spawn(context_manager.run());

    let state = Arc::new(AppState {
        chat: ChatState::new(ChatHistory::new()),
        config: ConfigState::default(),
        system: SystemState::default(),
        indexing_state: RwLock::new(None), // Initialize as None
        indexer_task: Some(Arc::clone(&indexer_task)),
        indexing_control: Arc::new(Mutex::new(None)),
        db: db_handle,
        embedder: Arc::clone(&proc_arc),
        io_handle: io_handle.clone(),
        rag,
        // TODO: Add TokenBudget fields to Config
        budget: TokenBudget::default(),
    });

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

#[derive(Debug, Clone, Error)]
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

pub mod system {
    use std::{borrow::Cow, sync::Arc};

    use crate::llm::ToolVendor;
    use ploke_db::TypedEmbedData;
    use serde_json::Value;
    use uuid::Uuid;

    use crate::UiError;

    #[derive(Clone, Debug)]
    pub enum SystemEvent {
        SaveRequested(Vec<u8>), // Serialized content
        HistorySaved {
            file_path: String,
        },
        MutationFailed(UiError),
        CommandDropped(&'static str),
        ReadSnippet(TypedEmbedData),
        CompleteReadSnip(Vec<String>),
        ModelSwitched(String),
        ToolCallRequested {
            request_id: Uuid,
            parent_id: Uuid,
            vendor: ToolVendor,
            name: String,
            arguments: Value,
            call_id: String,
        },
        ToolCallCompleted {
            request_id: Uuid,
            parent_id: Uuid,
            call_id: String,
            content: String,
        },
        ToolCallFailed {
            request_id: Uuid,
            parent_id: Uuid,
            call_id: String,
            error: String,
        },
        ReadQuery {
            file_name: String,
            query_name: String,
        },
        WriteQuery {
            query_name: String,
            query_content: String,
        },
        BackupDb {
            file_dir: String,
            is_success: bool,
            error: Option<String>,
        },
        LoadDb {
            crate_name: String,
            file_dir: Option<Arc<std::path::PathBuf>>,
            is_success: bool,
            error: Option<&'static str>,
        },
        ReIndex {
            workspace: String,
        },
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
pub enum AppEvent {
    Ui(UiEvent),
    Llm(llm::Event),
    LlmTool(llm::ToolEvent),
    // TODO:
    // File(file::Event),
    // Rag(rag::Event),
    // Agent(agent::Event),
    System(system::SystemEvent),
    // A message was successfully updated. UI should refresh this message.
    MessageUpdated(MessageUpdatedEvent),
    Rag(RagEvent),

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
        match self {
            AppEvent::Ui(_) => EventPriority::Realtime,
            AppEvent::Llm(_) => EventPriority::Background,
            AppEvent::LlmTool(ev) => match ev {
                llm::ToolEvent::Requested { .. } => EventPriority::Background,
                llm::ToolEvent::Completed { .. } | llm::ToolEvent::Failed { .. } => {
                    EventPriority::Realtime
                }
            },
            // Make sure the ModelSwitched event is in real-time priority, since it is intended to
            // update the UI.
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
        }
    }
    pub fn is_system(&self) -> bool {
        matches!(self, AppEvent::System(_))
    }
}
