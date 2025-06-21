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
use app_state::AppState;
use thiserror::Error;
use tokio::sync::broadcast;
use utils::layout::{self, layout_statusline};

use std::{collections::HashMap, sync::Arc, thread::current};

use chat_history::{ChatError, ChatHistory, NavigationDirection};
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
    let state = Arc::new(AppState::new());
    todo!()
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

pub mod system {
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
}

impl AppEvent {
    pub fn priority(&self) -> EventPriority {
        match self {
            AppEvent::Ui(_) => EventPriority::Realtime,
            AppEvent::Llm(_) => EventPriority::Background,
            AppEvent::System(_) => todo!(),
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


