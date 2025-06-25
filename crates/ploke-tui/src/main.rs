// TODO:
//
// 1 Add serialization support for saving/loading conversations
// 2 Implement scrolling through long message histories
// 3 Add visual indicators for branch points
// 4 Implement sibling navigation (up/down between children of same parent)
// 5 Add color coding for different message types (user vs assistant)

mod app;
pub mod app_state;
mod chat_history;
pub mod llm;
mod user_config;
mod utils;

use app::App;
use app_state::{AppState, MessageUpdatedEvent, StateCommand, state_manager};
use llm::llm_manager;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
use user_config::{ProviderConfig, DEFAULT_MODEL, OPENROUTER_URL};
use utils::layout::layout_statusline;

use std::{collections::HashMap, sync::Arc};

use chat_history::{ChatHistory, UpdateFailedEvent};
use color_eyre::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use futures::{FutureExt, StreamExt};
use ratatui::{
    DefaultTerminal, Frame,
    style::Stylize,
    text::Line,
    widgets::{Block, Borders, ListItem, ListState, Padding, Paragraph},
};
// for list
use ratatui::prelude::*;
use ratatui::{style::Style, widgets::List};
use uuid::Uuid;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    // TODO: Add error handling
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

    let event_bus = Arc::new(EventBus::new(100, 1000, 100));
    let state = Arc::new(AppState::default());

    // Create command channel with backpressure
    let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(1024);

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
    let app = App::new(state, cmd_tx, &event_bus);
    let result = app.run(terminal).await;
    ratatui::restore();
    result
}

pub mod ui {
    use uuid::Uuid;

    use crate::chat_history::NavigationDirection;

    #[derive(Clone, Debug)]
    pub enum Event {
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
        MutationFailed(UiError),
        CommandDropped(&'static str),
    }
}

// Other domains: file, rag, agent, system, ...

#[derive(Clone, Debug)]
pub enum AppEvent {
    Ui(ui::Event),
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
    // NOTE: `broadcast` is being used here for now, but may not be the best option long term. We
    // are leaving it as-is for flexibility, but there may not be a need for there to be a multiple
    // sender model for the `AppEvent` elements.
    realtime_tx: broadcast::Sender<AppEvent>,
    background_tx: broadcast::Sender<AppEvent>,
    error_tx: broadcast::Sender<ErrorEvent>,
}

impl EventBus {
    pub fn new(realtime_cap: usize, background_cap: usize, error_cap: usize) -> Self {
        Self {
            realtime_tx: broadcast::channel(realtime_cap).0,
            background_tx: broadcast::channel(background_cap).0,
            error_tx: broadcast::channel(error_cap).0,
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
}
