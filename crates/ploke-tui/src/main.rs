#![allow(unused_variables, unused_imports, dead_code)]

// TODO:
//
// 1 Add serialization support for saving/loading conversations
// 2 Implement scrolling through long message histories
// 3 Add visual indicators for branch points
// 4 Implement sibling navigation (up/down between children of same parent)
// 5 Add color coding for different message types (user vs assistant)
// mod app;
// pub mod app_state;
// mod chat_history;
// mod file_man;
// pub mod llm;
// mod tracing_setup;
// mod user_config;
// mod utils;
//
// use app::App;
// use app_state::{
//     AppState, ChatState, ConfigState, MessageUpdatedEvent, StateCommand, SystemState, state_manager,
// };
// use file_man::FileManager;
// use llm::llm_manager;
// use ploke_embed::{
//     cancel_token::CancellationToken,
//     indexer::{
//         self, CozoBackend, EmbeddingProcessor, EmbeddingSource, IndexerTask, IndexingStatus,
//     },
//     local::LocalEmbedder,
//     providers::{hugging_face::HuggingFaceBackend, openai::OpenAIBackend},
// };
// use thiserror::Error;
// use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
// use user_config::{DEFAULT_MODEL, OPENROUTER_URL, ProviderConfig};
// use utils::layout::layout_statusline;
//
// use std::sync::Arc;
//
// use chat_history::{ChatHistory, UpdateFailedEvent};
// use color_eyre::Result;
// use futures::{FutureExt, StreamExt};
// use ratatui::{
//     DefaultTerminal, Frame,
//     style::Stylize,
//     widgets::{Block, Borders, ListItem, ListState, Padding, Paragraph},
// };
// // for list
// use ratatui::prelude::*;
// use ratatui::{style::Style, widgets::List};
// use uuid::Uuid;

use ploke_tui::{tracing_setup, try_main};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let _guard = tracing_setup::init_tracing();
    color_eyre::config::HookBuilder::default()
        .display_location_section(false)
        .install()?;

    if let Err(e) = try_main().await {
        tracing::error!(error = %e, "Application error");
        return Err(e);
    }
    tracing::info!("Application exited normally");
    Ok(())
}

