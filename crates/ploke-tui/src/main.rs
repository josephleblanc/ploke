// TODO:
//
// 1 Add serialization support for saving/loading conversations
// 2 Implement scrolling through long message histories
// 3 Add visual indicators for branch points
// 4 Implement sibling navigation (up/down between children of same parent)
// 5 Add color coding for different message types (user vs assistant)

mod chat_history;
mod utils;
mod app;
pub mod app_state;
pub mod llm;

use app::App;
use app_state::{state_manager, AppState, MessageUpdatedEvent, StateCommand};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
use utils::layout::{self, layout_statusline};

use std::{collections::HashMap, sync::Arc, thread::current};

use chat_history::{ChatError, ChatHistory, NavigationDirection, UpdateFailedEvent};
use color_eyre::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::{FutureExt, StreamExt};
use ratatui::{
    DefaultTerminal, Frame,
    style::Stylize,
    text::Line,
    widgets::{Block, Borders, ListItem, ListState, Padding, Paragraph},
};
// for list
use ratatui::prelude::*;
use ratatui::{
    style::Style,
    widgets::{List, ListDirection},
};
use uuid::Uuid;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    // let terminal = ratatui::init();
    // let result = App::new().run(terminal).await;
    // ratatui::restore();
    // result
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(num_cpus::get())
        .max_blocking_threads(4)
        .build()?;

    let event_bus = Arc::new(EventBus::new(100, 1000));
    let state = Arc::new(AppState::default());

    // Create command channel with backpressure
    let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(1024);

    // Sapwn state manager first
    runtime.spawn(state_manager(
        state.clone(),
        cmd_rx,
        event_bus.clone()
    ));

    // Spawn subsystems with backpressure-aware command sender
    runtime.spawn(llm_manager(
        event_bus.clone(),
        state.clone(),
        cmd_tx.clone() // Clone for each subsystem
    ));

    let terminal = ratatui::init();
    let app = App::new(state.clone(), event_bus.clone(), cmd_tx);
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
        InputSubmitted(String)
    }
}

#[derive(Debug, Clone, Error)]
pub enum UiError {
    ExampleError
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
    pub enum Event {
        MutationFailed(UiError)
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
    System(system::Event),
    // A message was successfully updated. UI should refresh this message.
    MessageUpdated(MessageUpdatedEvent),

    // An attempt to update a message was rejected. UI should show an error.
    UpdateFailed(UpdateFailedEvent),
}

impl AppEvent {
    pub fn priority(&self) -> EventPriority {
        match self {
            AppEvent::Ui(_) => EventPriority::Realtime,
            AppEvent::Llm(_) => EventPriority::Background,
            AppEvent::System(_) => todo!(),
            AppEvent::MessageUpdated(_) => EventPriority::Realtime,
            AppEvent::UpdateFailed(_) => EventPriority::Background,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum EventPriority {
    Realtime,
    Background
}

pub struct EventBus {
    realtime_tx: broadcast::Sender<AppEvent>,
    background_tx: broadcast::Sender<AppEvent>
}

impl EventBus {
    pub fn new(realtime_cap: usize, background_cap: usize) -> Self {
        Self {
            realtime_tx: broadcast::channel(realtime_cap).0,
            background_tx: broadcast::channel(background_cap).0,
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
    
    pub fn subscribe(&self, priority: EventPriority) -> broadcast::Receiver<AppEvent> {
        match priority {
            EventPriority::Realtime => self.realtime_tx.subscribe(),
            EventPriority::Background => self.background_tx.subscribe(),
        }
    }
}


