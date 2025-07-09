pub mod app;
pub mod app_state;
pub mod chat_history;
pub mod file_man;
pub mod llm;
pub mod tracing_setup;
pub mod user_config;
pub mod utils;

#[cfg(test)]
mod test_utils;

use app::App;
use app_state::{
    AppState, ChatState, ConfigState, MessageUpdatedEvent, StateCommand, SystemState, state_manager,
};
use file_man::FileManager;
use llm::llm_manager;
use ploke_embed::{
    cancel_token::CancellationToken,
    indexer::{self, IndexerTask, IndexingStatus},
};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock, broadcast, mpsc};
use ui::UiEvent;
use user_config::{DEFAULT_MODEL, OPENROUTER_URL, ProviderConfig};
use utils::layout::layout_statusline;

use std::sync::Arc;

use chat_history::{ChatHistory, UpdateFailedEvent};
use color_eyre::Result;
use futures::{FutureExt, StreamExt};
use ratatui::{
    DefaultTerminal, Frame,
    style::Stylize,
    widgets::{Block, Borders, ListItem, ListState, Padding, Paragraph},
};
// for list
use ratatui::prelude::*;
use ratatui::{style::Style, widgets::List};
use uuid::Uuid;

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
        .try_deserialize::<crate::user_config::Config>()?;

    if let Ok(openrouter_api_key) = std::env::var("OPENROUTER_API_KEY") {
        config.provider = ProviderConfig {
            api_key: openrouter_api_key,
            base_url: OPENROUTER_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
        };
    }

    let new_db = ploke_db::Database::init_with_schema()?;
    let db_handle = Arc::new(new_db);

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

    // TODO:
    // 1 Implement the cancellation token propagation in IndexerTask
    // 2 Add error handling for embedder initialization failures
    // 3 Complete the UI progress reporting integration
    let indexer_task = IndexerTask {
        db: db_handle.clone(),
        io: io_handle.clone(),
        embedding_processor: processor, // Use configured processor
        cancellation_token: CancellationToken::new().0,
        batch_size: 1024,
        cursor: Arc::new(Mutex::new(0)),
    };

    let state = Arc::new(AppState {
        chat: ChatState::new(ChatHistory::new()),
        config: ConfigState::default(),
        system: SystemState::default(),
        indexing_state: RwLock::new(None), // Initialize as None
        indexer_task: Some(Arc::new(indexer_task)),
        indexing_control: Arc::new(Mutex::new(None)),
    });

    let (cancellation_token, cancel_handle) = CancellationToken::new();

    // Create command channel with backpressure
    let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(1024);

    let (filemgr_tx, filemgr_rx) = mpsc::channel::<AppEvent>(256);
    let file_manager = FileManager::new(
        io_handle.clone(),
        event_bus.subscribe(EventPriority::Background),
    );

    tokio::spawn(file_manager.run());

    // Spawn state manager first
    tokio::spawn(state_manager(state.clone(), cmd_rx, event_bus.clone()));

    // Spawn subsystems with backpressure-aware command sender
    tokio::spawn(llm_manager(
        event_bus.subscribe(EventPriority::Background),
        state.clone(),
        cmd_tx.clone(), // Clone for each subsystem
        config.provider.clone(),
    ));

    let terminal = ratatui::init();
    let app = App::new(config.command_style, state, cmd_tx, &event_bus);
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
    use crate::UiError;

    #[derive(Clone, Debug)]
    pub enum SystemEvent {
        SaveRequested(Vec<u8>), // Serialized content
        MutationFailed(UiError),
        CommandDropped(&'static str),
    }
}

// Other domains: file, rag, agent, system, ...

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

    // An attempt to update a message was rejected. UI should show an error.
    UpdateFailed(UpdateFailedEvent),
    Error(ErrorEvent),
    IndexingProgress(indexer::IndexingStatus),
    IndexingStarted,
    IndexingCompleted,
    IndexingFailed(String),
}

impl AppEvent {
    pub fn priority(&self) -> EventPriority {
        match self {
            AppEvent::Ui(_) => EventPriority::Realtime,
            AppEvent::Llm(_) => EventPriority::Background,
            AppEvent::System(_) => EventPriority::Background,
            AppEvent::MessageUpdated(_) => EventPriority::Realtime,
            AppEvent::UpdateFailed(_) => EventPriority::Background,
            AppEvent::Error(_) => EventPriority::Background,
            AppEvent::IndexingProgress(_) => EventPriority::Realtime,
            AppEvent::IndexingStarted => EventPriority::Background,
            AppEvent::IndexingCompleted => EventPriority::Background,
            AppEvent::IndexingFailed(_) => EventPriority::Background,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ErrorEvent {
    pub message: String,
    pub severity: ErrorSeverity,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorSeverity {
    Warning, // simple warning
    Error,   // recoverable error
    Fatal,   // indicates invalid state
}

#[derive(Clone, Copy, Debug)]
pub enum EventPriority {
    Realtime,
    Background,
}

pub struct EventBus {
    realtime_tx: broadcast::Sender<AppEvent>,
    background_tx: broadcast::Sender<AppEvent>,
    error_tx: broadcast::Sender<ErrorEvent>,
    // NOTE: dedicated for indexing manager control
    index_tx: broadcast::Sender<indexer::IndexingStatus>,
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

impl EventBus {
    pub fn new(b: EventBusCaps) -> Self {
        Self {
            realtime_tx: broadcast::channel(b.realtime_cap).0,
            background_tx: broadcast::channel(b.background_cap).0,
            error_tx: broadcast::channel(b.error_cap).0,
            index_tx: broadcast::channel(b.index_cap).0,
        }
    }

    pub fn send(&self, event: AppEvent) {
        let priority = event.priority();
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
