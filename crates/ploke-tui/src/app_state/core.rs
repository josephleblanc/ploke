use ploke_core::{ArcStr, TrackingHash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::llm::LLMParameters;
use crate::user_config::{CommandStyle, EmbeddingConfig, ModelRegistry, UserConfig};
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

    // In-memory registry for staged code-edit proposals (M1)
    pub proposals: RwLock<HashMap<Uuid, EditProposal>>,

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
pub struct ConfigState(RwLock<RuntimeConfig>);

impl ConfigState {
    pub fn new<C: Into<RuntimeConfig>>(config: C) -> Self {
        ConfigState(RwLock::new(config.into()))
    }
}

impl std::ops::Deref for ConfigState {
    type Target = RwLock<RuntimeConfig>;
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

// Editing configuration for M1 safe-editing pipeline
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum PreviewMode {
    #[default]
    CodeBlock,
    Diff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditingConfig {
    pub preview_mode: PreviewMode,
    pub auto_confirm_edits: bool,
    pub max_preview_lines: usize,
}

impl Default for EditingConfig {
    fn default() -> Self {
        Self {
            preview_mode: PreviewMode::CodeBlock,
            auto_confirm_edits: false,
            max_preview_lines: 300,
        }
    }
}

#[cfg(not(feature = "llm_refactor"))]
pub use old_refactor_block::*;
#[cfg(not(feature = "llm_refactor"))]
pub mod old_refactor_block {
    use super::*;

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    pub struct RuntimeConfig {
        pub llm_params: LLMParameters,
        pub model_registry: ModelRegistry,
        pub editing: EditingConfig,
        pub command_style: CommandStyle,
        pub embedding: EmbeddingConfig,
        pub ploke_editor: Option<String>,
    }

    impl From<UserConfig> for RuntimeConfig {
        fn from(uc: UserConfig) -> Self {
            let registry = uc.registry;
            // Choose LLM params from active provider or default
            let llm_params = registry
                .get_active_model_config()
                .and_then(|p| p.llm_params.clone())
                .unwrap_or_default();

            // Map persisted editing -> runtime editing
            let editing = EditingConfig {
                preview_mode: PreviewMode::CodeBlock,
                auto_confirm_edits: uc.editing.auto_confirm_edits,
                max_preview_lines: 300,
            };

            RuntimeConfig {
                llm_params,
                model_registry: registry,
                editing,
                command_style: uc.command_style,
                embedding: uc.embedding,
                ploke_editor: uc.ploke_editor,
            }
        }
    }

    impl RuntimeConfig {
        /// Convert the live runtime config back into a persisted UserConfig for saving.
        pub fn to_user_config(&self) -> UserConfig {
            let editing = crate::user_config::EditingConfig {
                auto_confirm_edits: self.editing.auto_confirm_edits,
                agent: crate::user_config::EditingAgentConfig::default(),
            };

            UserConfig {
                registry: self.model_registry.clone(),
                command_style: self.command_style,
                embedding: self.embedding.clone(),
                editing,
                ploke_editor: self.ploke_editor.clone(),
            }
        }
    }
}

pub use new_refactor_block::*;
#[cfg(feature = "llm_refactor")]
pub mod new_refactor_block {
    use super::*;

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    pub struct RuntimeConfig {
        pub llm_params: LLMParameters,
        pub model_registry: ModelRegistry,
        pub editing: EditingConfig,
        pub command_style: CommandStyle,
        pub embedding: EmbeddingConfig,
        pub ploke_editor: Option<String>,
    }

    impl From<UserConfig> for RuntimeConfig {
        fn from(uc: UserConfig) -> Self {
            let registry = uc.registry;
            // Choose LLM params from active provider or default
            let llm_params = registry
                .get_active_model_config()
                .and_then(|p| p.llm_params.clone())
                .unwrap_or_default();

            // Map persisted editing -> runtime editing
            let editing = EditingConfig {
                preview_mode: PreviewMode::CodeBlock,
                auto_confirm_edits: uc.editing.auto_confirm_edits,
                max_preview_lines: 300,
            };

            RuntimeConfig {
                llm_params,
                model_registry: registry,
                editing,
                command_style: uc.command_style,
                embedding: uc.embedding,
                ploke_editor: uc.ploke_editor,
            }
        }
    }

    impl RuntimeConfig {
        /// Convert the live runtime config back into a persisted UserConfig for saving.
        pub fn to_user_config(&self) -> UserConfig {
            let editing = crate::user_config::EditingConfig {
                auto_confirm_edits: self.editing.auto_confirm_edits,
                agent: crate::user_config::EditingAgentConfig::default(),
            };

            UserConfig {
                registry: self.model_registry.clone(),
                command_style: self.command_style,
                embedding: self.embedding.clone(),
                editing,
                ploke_editor: self.ploke_editor.clone(),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EditProposalStatus {
    Pending,
    Approved,
    Denied,
    Applied,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeforeAfter {
    pub file_path: PathBuf,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffPreview {
    CodeBlocks { per_file: Vec<BeforeAfter> },
    UnifiedDiff { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditProposal {
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub call_id: ArcStr,
    pub proposed_at_ms: i64,
    pub edits: Vec<ploke_core::WriteSnippetData>,
    pub files: Vec<PathBuf>,
    pub preview: DiffPreview,
    pub status: EditProposalStatus,
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
            config: ConfigState(RwLock::new(RuntimeConfig::default())),
            system: SystemState(RwLock::new(SystemStatus::default())),
            indexing_state: RwLock::new(None),
            indexer_task: None,
            indexing_control: Arc::new(Mutex::new(None)),
            db,
            embedder,
            io_handle,
            proposals: RwLock::new(HashMap::new()),
            rag: Some(rag),
            budget,
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
