use cozo::DataValue;
use cozo::{Db, MemStorage, ScriptMutability};
use ploke_db::Database;
use ploke_embed::error::EmbedError;
use ploke_embed::indexer::EmbeddingProcessor;
use ratatui::widgets::ListState;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::app::App;
use crate::app_state::{AppState, ChatState, ConfigState, SystemState};
use crate::chat_history::ChatHistory;
use crate::event_bus::EventBus;
use crate::user_config::CommandStyle;
use crate::llm::openrouter_catalog::ModelEntry;
use crate::RagEvent;
use ploke_rag::{RagService, TokenBudget};
use ploke_embed::indexer::IndexerTask;
use ploke_io::IoManagerHandle;
use tokio::sync::mpsc;

pub fn create_mock_app_state() -> AppState {
    let db = create_mock_db(0);
    let embedder = Arc::new(EmbeddingProcessor::new_mock());
    let io_handle = IoManagerHandle::new();
    let rag = Arc::new(RagService::new_mock());
    let budget = TokenBudget::default();
    let (rag_tx, _rag_rx) = mpsc::channel::<RagEvent>(10);
    
    AppState {
        chat: crate::app_state::ChatState::new(ChatHistory::new()),
        config: crate::app_state::ConfigState::new(crate::app_state::RuntimeConfig::default()),
        system: crate::app_state::SystemState::default(),
        indexing_state: tokio::sync::RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
        db,
        embedder,
        io_handle,
        proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        rag: Some(rag),
        budget,
    }
}

pub fn create_mock_app() -> App {
    let state = Arc::new(create_mock_app_state());
    let (cmd_tx, _cmd_rx) = mpsc::channel(1024);
    let event_bus = EventBus::new(crate::EventBusCaps::default());
    let active_model_id = "mock-model".to_string();
    
    App::new(
        CommandStyle::Slash,
        state,
        cmd_tx,
        &event_bus,
        active_model_id,
    )
}

pub fn create_mock_db(num_unindexed: usize) -> Arc<Database> {
    let db = Database::init_with_schema().unwrap();
    
    if num_unindexed > 0 {
        let script = r#"
        ?[id, file_path, tracking_hash, start_byte, end_byte, namespace] <- [
            [uuid_v4(), "/mock/file.rs", uuid_v4(), 0, 100, uuid_v4()],
        ]

        :put embedding_nodes {
            id => Uuid,
            file_path => String,
            tracking_hash => Uuid,
            start_byte => Int,
            end_byte => Int,
            namespace => Uuid,
            embedding => <F32; 384> default null
        }
        "#;
        
        db.run_script(script, std::collections::BTreeMap::new(), ScriptMutability::Mutable)
            .unwrap();
    }
    
    Arc::new( db )
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
