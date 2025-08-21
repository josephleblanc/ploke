use cozo::DataValue;
use cozo::{Db, MemStorage, ScriptMutability};
use ploke_db::Database;
use ploke_embed::error::EmbedError;
use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource};
use ratatui::widgets::ListState;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::app::App;
use crate::app_state::{AppState, ChatState, ConfigState, SystemState};
use crate::chat_history::ChatHistory;

pub fn create_mock_app_state() -> AppState {
    // AI: fill out AI!
    AppState::new(db, embedder, io_handle, rag, budget, rag_tx)
}

pub fn create_mock_app() -> App {
    // AI: fill out AI!
    App::new(command_style, state, cmd_tx, event_bus, active_model_id)
}

pub fn create_mock_db(num_unindexed: usize) -> Arc<Database> {
    let storage = MemStorage::default();
    let db = Arc::new(Database::new(Db::new(storage).unwrap()));

    let script = r#"
    ?[id, path, tracking_hash, start_byte, end_byte] <- [
        $unindexed,
    ]

    :create embedding_nodes {
        id => Uuid
    }
    "#;

    todo!("define and insert params, ensure db.run_script works correctly");

    // db.run_script(script, params, ScriptMutability::Mutable).unwrap();
    #[allow(unreachable_code)]
    db
}

#[derive(Debug, PartialEq, Eq)]
pub enum MockBehavior {
    Normal,
    RateLimited,
    DimensionMismatch,
    NetworkError,
}

pub struct MockEmbedder {
    pub dimensions: usize,
    pub behavior: MockBehavior,
}
