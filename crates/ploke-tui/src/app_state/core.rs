use std::path::PathBuf;
use std::sync::Arc;

use crate::llm::LLMParameters;
use crate::user_config::ProviderRegistry;
use crate::{RagEvent, chat_history::ChatHistory};
use ploke_db::Database;
use ploke_embed::indexer::{EmbeddingProcessor, IndexerCommand, IndexerTask, IndexingStatus};
use ploke_io::IoManagerHandle;
use ploke_rag::{RagService, TokenBudget};
use tokio::sync::{Mutex, RwLock, mpsc};

#[derive(Debug)]
pub struct AppState {
    pub chat: ChatState,
    pub config: ConfigState,
    pub system: SystemState,

    // crate-external processes
    pub indexing_state: RwLock<Option<IndexingStatus>>,
    pub indexer_task: Option<Arc<IndexerTask>>,
    pub indexing_control: Arc<Mutex<Option<mpsc::Sender<IndexerCommand>>>>,

    pub db: Arc<Database>,
    pub embedder: Arc<EmbeddingProcessor>,
    pub io_handle: IoManagerHandle,

    // RAG stuff
    pub rag: Option<Arc<ploke_rag::RagService>>,
    pub budget: TokenBudget,
    // pub rag_tx: mpsc::Sender<RagEvent>,
}

#[derive(Debug, Default)]
pub struct ChatState(pub RwLock<ChatHistory>);

impl ChatState {
    pub fn new(history: ChatHistory) -> Self {
        ChatState(RwLock::new(history))
    }
}

impl std::ops::Deref for ChatState {
    type Target = RwLock<ChatHistory>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Default)]
pub struct ConfigState(RwLock<Config>);

impl ConfigState {
    pub fn new(config: Config) -> Self {
        ConfigState(RwLock::new(config))
    }
}

impl std::ops::Deref for ConfigState {
    type Target = RwLock<Config>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Default)]
pub struct SystemState(RwLock<SystemStatus>);

impl SystemState {
    pub fn new(status: SystemStatus) -> Self {
        SystemState(RwLock::new(status))
    }
}

impl std::ops::Deref for SystemState {
    type Target = RwLock<SystemStatus>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct IndexingState(Arc<Mutex<IndexingStatus>>);

impl IndexingState {
    pub fn new(status: IndexingStatus) -> Self {
        IndexingState(Arc::new(Mutex::new(status)))
    }
}

impl std::ops::Deref for IndexingState {
    type Target = Arc<Mutex<IndexingStatus>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Default)]
pub struct Config {
    pub llm_params: LLMParameters,
    pub provider_registry: ProviderRegistry,
}

impl AppState {
    pub fn new(
        db: Arc<Database>,
        embedder: Arc<EmbeddingProcessor>,
        io_handle: IoManagerHandle,
        rag: Arc<RagService>,
        budget: TokenBudget,
        rag_tx: mpsc::Sender<RagEvent>,
    ) -> Self {
        Self {
            chat: ChatState(RwLock::new(ChatHistory::new())),
            config: ConfigState(RwLock::new(Config::default())),
            system: SystemState(RwLock::new(SystemStatus::default())),
            indexing_state: RwLock::new(None),
            indexer_task: None,
            indexing_control: Arc::new(Mutex::new(None)),
            db,
            embedder,
            io_handle,
            rag: Some(rag),
            budget,
            // rag_tx,
        }
    }
}

#[derive(Debug, Default)]
pub struct SystemStatus {
    pub(crate) crate_focus: Option<PathBuf>,
}

impl SystemStatus {
    pub fn new(crate_focus: Option<PathBuf>) -> Self {
        Self { crate_focus }
    }
}
