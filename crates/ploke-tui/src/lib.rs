#![allow(unused_variables, unused_imports, dead_code)]

pub mod app;
pub mod app_state;
pub mod chat_history;
pub mod context;
pub mod database;
pub mod error;
pub mod file_man;
pub mod llm;
pub mod parser;
pub mod tracing_setup;
pub mod user_config;
pub mod utils;

#[cfg(test)]
mod test_utils;

use app::App;
use app_state::{
    AppState, ChatState, ConfigState, MessageUpdatedEvent, StateCommand, SystemState, state_manager,
};
use context::ContextManager;
use error::{ErrorExt, ErrorSeverity, ResultExt};
use file_man::FileManager;
use llm::llm_manager;
use parser::run_parse;
use ploke_embed::{
    cancel_token::CancellationToken,
    indexer::{self, IndexStatus, IndexerTask, IndexingStatus},
};
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
    let event_bus_caps = EventBusCaps {
        realtime_cap: 100,
        background_cap: 1000,
        error_cap: 100,
        index_cap: 1000,
    };
    let event_bus = Arc::new(EventBus::new(event_bus_caps));

    let processor = config.load_embedding_processor()?;
    let proc_arc = Arc::new(processor);

    // TODO:
    // 1 Implement the cancellation token propagation in IndexerTask
    // 2 Add error handling for embedder initialization failures
    let indexer_task = IndexerTask::new(
        db_handle.clone(),
        io_handle.clone(),
        Arc::clone(&proc_arc), // Use configured processor
        CancellationToken::new().0,
        8,
    );

    let state = Arc::new(AppState {
        chat: ChatState::new(ChatHistory::new()),
        config: ConfigState::default(),
        system: SystemState::default(),
        indexing_state: RwLock::new(None), // Initialize as None
        indexer_task: Some(Arc::new(indexer_task)),
        indexing_control: Arc::new(Mutex::new(None)),
        db: db_handle,
        embedder: Arc::clone(&proc_arc),
        io_handle: io_handle.clone(),
    });

    // Create command channel with backpressure
    let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(1024);

    let (rag_event_tx, rag_event_rx) = mpsc::channel(10);
    let context_manager = ContextManager::new(rag_event_rx, Arc::clone(&event_bus));
    tokio::spawn(context_manager.run());

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
    ));
    tokio::spawn(run_event_bus(Arc::clone(&event_bus)));

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

    use ploke_db::TypedEmbedData;

    use crate::UiError;

    #[derive(Clone, Debug)]
    pub enum SystemEvent {
        SaveRequested(Vec<u8>), // Serialized content
        MutationFailed(UiError),
        CommandDropped(&'static str),
        ReadSnippet(TypedEmbedData),
        CompleteReadSnip(Vec<String>),
        ModelSwitched(String),
        ReadQuery {
            file_name: String,
            query_name: String,
        },
        LoadQuery {
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
            workspace: String
        }
    }
}

// Other domains: file, rag, agent, system, ...

pub mod ploke_rag {
    use crate::chat_history::Message;

    use super::*;
    #[derive(Clone, Debug)]
    pub enum RagEvent {
        ContextSnippets(Uuid, Vec<String>),
        UserMessages(Uuid, Vec<Message>),
        ConstructContext(Uuid),
    }
}

use ploke_rag::RagEvent;
#[derive(Clone, Debug)]
pub enum AppEvent {
    Ui(UiEvent),
    Llm(llm::Event),
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
            // Make sure the ModelSwitched event is in real-time priority, since it is intended to
            // update the UI.
            AppEvent::System(SystemEvent::ModelSwitched(_)) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::ReadQuery { .. }) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::LoadQuery { .. }) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::BackupDb { .. }) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::LoadDb { .. }) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::ReIndex { .. }) => EventPriority::Realtime,
            AppEvent::System(_) => EventPriority::Background,
            AppEvent::MessageUpdated(_) => EventPriority::Realtime,
            AppEvent::UpdateFailed(_) => EventPriority::Background,
            AppEvent::Error(_) => EventPriority::Background,
            AppEvent::IndexingProgress(_) => EventPriority::Realtime,
            AppEvent::IndexingStarted => EventPriority::Background,
            AppEvent::IndexingCompleted => EventPriority::Realtime,
            AppEvent::IndexingFailed => EventPriority::Realtime,
            AppEvent::Rag(rag_event) => EventPriority::Background,
            AppEvent::GenerateContext(_) => EventPriority::Background,
        }
    }
    pub fn is_system(&self) -> bool {
        matches!(self, AppEvent::System(_))
    }
}

#[derive(Debug, Clone)]
pub struct ErrorEvent {
    pub message: String,
    pub severity: ErrorSeverity,
}

#[derive(Clone, Copy, Debug)]
pub enum EventPriority {
    Realtime,
    Background,
}

// Import the error handling traits

// Implement the ResultExt trait for Results with ploke_error::Error
// This implementation is only for Result<T, ploke_error::Error> to comply with orphan rule
impl<T> ResultExt<ploke_error::Error> for Result<T, ploke_error::Error> {
    fn emit_event(self, severity: ErrorSeverity) -> Result<T, ploke_error::Error> {
        if let Err(ref e) = self {
            let message = e.to_string();
            tokio::spawn(async move {
                emit_error_event(message, severity).await;
            });
        }
        self
    }

    fn emit_warning(self) -> Result<T, ploke_error::Error> {
        self.emit_event(ErrorSeverity::Warning)
    }

    fn emit_error(self) -> Result<T, ploke_error::Error> {
        self.emit_event(ErrorSeverity::Error)
    }

    fn emit_fatal(self) -> Result<T, ploke_error::Error> {
        self.emit_event(ErrorSeverity::Fatal)
    }
}

impl ErrorExt for ploke_error::Error {
    fn emit_event(&self, severity: ErrorSeverity) {
        let message = self.to_string();
        tokio::spawn(async move {
            emit_error_event(message, severity).await;
        });
    }
}

#[derive(Debug)]
pub struct EventBus {
    realtime_tx: broadcast::Sender<AppEvent>,
    background_tx: broadcast::Sender<AppEvent>,
    error_tx: broadcast::Sender<ErrorEvent>,
    // NOTE: dedicated for indexing manager control
    index_tx: Arc<broadcast::Sender<indexer::IndexingStatus>>,
    // NOTE: Dedicated for context control
}

/// Convenience struct to help with the initialization of EventBus
#[derive(Clone, Copy)]
pub struct EventBusCaps {
    realtime_cap: usize,
    background_cap: usize,
    error_cap: usize,
    index_cap: usize,
}

impl Default for EventBusCaps {
    fn default() -> Self {
        Self {
            realtime_cap: 100,
            background_cap: 1000,
            error_cap: 1000,
            index_cap: 1000,
        }
    }
}

async fn run_event_bus(event_bus: Arc<EventBus>) -> Result<()> {
    use broadcast::error::RecvError;
    let mut index_rx = event_bus.index_subscriber();
    #[allow(unused_mut)]
    let mut bg_rx = event_bus.background_tx.subscribe();
    // more here?
    loop {
        tokio::select! {
        // bg_event = bg_rx.recv() => {
        // tracing::trace!("event bus received a background event: {:?}", bg_event);
        //     match bg_event {
        //         Ok(AppEvent::System(sys_event)) => match sys_event {
        //             SystemEvent::ModelSwitched(alias_or_id) => {
        //                 tracing::info!("event bus Sent RAG event with snippets: {:#?}", alias_or_id);
        //                 event_bus.send(AppEvent::System(SystemEvent::ModelSwitched(alias_or_id)));
        //             }
        //             SystemEvent::SaveRequested(vec_bytes) => {
        //                 tracing::info!("event bus Sent save event of Vec<u8> len = {}", vec_bytes.len());
        //                 event_bus.send(AppEvent::System(SystemEvent::SaveRequested(vec_bytes)));
        //         }
        //             _ => {}
        //         },
        //         Ok(_) => {}
        //         Err(e) => {
        //             match e {
        //                 RecvError::Closed => {
        //                     tracing::trace!("System Event event channel closed {}", e.to_string());
        //                     break;
        //                 }
        //                 RecvError::Lagged(lag) => {
        //                     tracing::trace!(
        //                         "System Event event channel lagging {} with {} messages",
        //                         e.to_string(),
        //                         lag,
        //                     )
        //                 }
        //             };
        //         }
        //     }
        // }
            index_event = index_rx.recv() => {
            // let index_event = index_rx.recv().await;
            tracing::trace!("event bus received IndexStatus");
            match index_event {
                Ok(status) => {
                    match status.status {
                        IndexStatus::Running => {
                            tracing::warn!("event bus sending {:?}", status.status);
                            let result = event_bus
                                .realtime_tx
                                .send(AppEvent::IndexingProgress(status));
                            tracing::warn!("with result {:?}", result);
                            continue;
                        }
                        IndexStatus::Completed => {
                            let result = event_bus.realtime_tx.send(AppEvent::IndexingCompleted);
                            tracing::warn!(
                                "event bus sending {:?} with result {:?}",
                                status.status,
                                result
                            );
                            continue;
                        }
                        IndexStatus::Cancelled => {
                            // WARN: Consider whether this should count as a failure or not
                            // when doing better error handling later.
                            let result = event_bus.realtime_tx.send(AppEvent::IndexingFailed);
                            tracing::warn!(
                                "event bus sending {:?} with result {:?}",
                                status.status,
                                result
                            );
                            continue;
                        }
                        _ => {}
                    }
                }
                Err(e) => match e {
                    RecvError::Closed => {
                        tracing::trace!("indexing task event channel closed {}", e.to_string());
                        // break;
                    }
                    RecvError::Lagged(lag) => {
                        tracing::trace!(
                            "indexing task event channel lagging {} with {} messages",
                            e.to_string(),
                            lag
                        )
                    }
                },
            }
            }
            // };
        };
    }
    // Ok(())
}
impl EventBus {
    pub fn new(b: EventBusCaps) -> Self {
        Self {
            realtime_tx: broadcast::channel(b.realtime_cap).0,
            background_tx: broadcast::channel(b.background_cap).0,
            error_tx: broadcast::channel(b.error_cap).0,
            index_tx: Arc::new(broadcast::channel(b.index_cap).0),
        }
    }

    #[instrument]
    pub fn send(&self, event: AppEvent) {
        let priority = event.priority();
        tracing::debug!("event_priority: {:?}", priority);
        let tx = match priority {
            EventPriority::Realtime => &self.realtime_tx,
            EventPriority::Background => &self.background_tx,
        };
        let _ = tx.send(event); // Ignore receiver count
    }

    pub fn send_error(&self, message: String, severity: ErrorSeverity) {
        let _ = self.error_tx.send(ErrorEvent { message, severity });
    }

    pub fn subscribe(&self, priority: EventPriority) -> broadcast::Receiver<AppEvent> {
        match priority {
            EventPriority::Realtime => self.realtime_tx.subscribe(),
            EventPriority::Background => self.background_tx.subscribe(),
        }
    }

    pub fn index_subscriber(&self) -> broadcast::Receiver<indexer::IndexingStatus> {
        self.index_tx.subscribe()
    }
}
