use chrono::Utc;
use ploke_core::file_hash::LargeFilePolicy;
use ploke_core::{ArcStr, CrateId, CrateInfo, TrackingHash, WorkspaceRoots};
use ploke_error::DomainError;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::llm::LLMParameters;
use crate::llm::registry::user_prefs::RegistryPrefs;
use crate::llm::{ModelId, ModelKey};
use crate::user_config::{
    ChatPolicy, CommandStyle, CtxPrefs, EmbeddingConfig, LocalEmbeddingTuning, RagUserConfig,
    UserConfig,
};
use crate::{RagEvent, chat_history::ChatHistory};
use ploke_db::Database;
use ploke_embed::indexer::{IndexerCommand, IndexerTask, IndexingStatus};
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::path_policy::{PathPolicy, SymlinkPolicy};
use ploke_io::{IoManagerHandle, NsWriteSnippetData, PatchApplyOptions};
use ploke_rag::{RagService, TokenBudget};
use tokio::sync::{Mutex, RwLock, mpsc};

#[derive(Debug)]
pub struct AppState {
    pub chat: ChatState,
    pub config: ConfigState,
    pub system: SystemState,
    // TODO:multi-router 2025-12-15
    // Add router field for <dyn Router> or wrapper type or something

    // crate-external processes
    pub indexing_state: RwLock<Option<IndexingStatus>>,
    pub indexer_task: Option<Arc<IndexerTask>>,
    pub indexing_control: Arc<Mutex<Option<mpsc::Sender<IndexerCommand>>>>,

    pub db: Arc<Database>,
    pub embedder: Arc<EmbeddingRuntime>,
    pub io_handle: IoManagerHandle,

    // In-memory registry for staged code-edit proposals (M1)
    pub proposals: RwLock<HashMap<Uuid, EditProposal>>,
    // In-memory registry for staged file-creation proposals
    pub create_proposals: RwLock<HashMap<Uuid, CreateProposal>>,

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
    #[cfg(feature = "test_harness")]
    pub async fn set_crate_focus_for_test(&self, p: std::path::PathBuf) {
        let mut guard = self.0.write().await;
        guard.set_focus_from_root(p);
    }
    #[cfg(feature = "test_harness")]
    pub async fn crate_focus_for_test(&self) -> Option<std::path::PathBuf> {
        let guard = self.0.read().await;
        guard.focused_crate_root()
    }

    pub async fn is_stale_err(&self) -> Result<(), ploke_error::Error> {
        if self.read().await.focused_crate_stale() == Some(true) {
            Err(ploke_error::Error::Domain(DomainError::Ui {
                message: "Focused crate index is stale; reindex to query code items.".to_string(),
            }))
        } else {
            Ok(())
        }
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
    pub patch_cfg: PatchApplyOptions,
    pub large_file_policy: LargeFilePolicy,
}

impl Default for EditingConfig {
    fn default() -> Self {
        Self {
            preview_mode: PreviewMode::CodeBlock,
            auto_confirm_edits: false,
            max_preview_lines: 300,
            large_file_policy: Default::default(),
            patch_cfg: Default::default(),
        }
    }
}

use super::*;
use crate::tools::ToolVerbosity;
use crate::user_config::ToolingConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub llm_params: LLMParameters,
    pub model_registry: RegistryPrefs,
    pub active_model: ModelId,
    pub editing: EditingConfig,
    pub command_style: CommandStyle,
    pub tool_verbosity: ToolVerbosity,
    pub embedding: EmbeddingConfig,
    pub embedding_local: LocalEmbeddingTuning,
    pub ploke_editor: Option<String>,
    pub tooling: ToolingConfig,
    pub chat_policy: ChatPolicy,
    pub rag: RagUserConfig,
    pub token_limit: u32,
    pub tool_retries: u32,
    pub llm_timeout_secs: u64,
    pub context_management: CtxPrefs,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        UserConfig::default().into()
    }
}

impl From<UserConfig> for RuntimeConfig {
    fn from(uc: UserConfig) -> Self {
        let registry: RegistryPrefs = uc.registry;
        // Choose LLM params from default profile for default model if present
        let default_key: ModelKey = Default::default();
        let llm_params = registry
            .models
            .get(&default_key)
            .and_then(|mp| mp.get_default_profile())
            .map(|prof| prof.params.clone())
            .unwrap_or_default();

        // Map persisted editing -> runtime editing
        let editing = EditingConfig {
            preview_mode: PreviewMode::CodeBlock,
            auto_confirm_edits: uc.editing.auto_confirm_edits,
            max_preview_lines: 300,
            ..Default::default()
        };

        // Validate/chat/rag advanced knobs.
        let chat_policy = uc.chat_policy.validated();
        let rag = uc.rag.validated();
        let embedding_local = LocalEmbeddingTuning {
            model_batch_size: uc.embedding_local.model_batch_size.max(1),
            ..uc.embedding_local
        };

        RuntimeConfig {
            llm_params,
            model_registry: registry,
            active_model: ModelId::from(ModelKey::default()),
            editing,
            command_style: uc.command_style,
            tool_verbosity: uc.tool_verbosity,
            embedding: uc.embedding,
            embedding_local,
            ploke_editor: uc.ploke_editor,
            tooling: uc.tooling,
            chat_policy,
            rag,
            token_limit: uc.token_limit,
            tool_retries: uc.tool_retries,
            llm_timeout_secs: uc.llm_timeout_secs,
            context_management: uc.context_management,
        }
    }
}

pub fn rag_budget_from_config(rag: &RagUserConfig) -> TokenBudget {
    TokenBudget {
        per_part_max: rag.per_part_max_tokens,
        ..TokenBudget::default()
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
            tool_verbosity: self.tool_verbosity,
            embedding: self.embedding.clone(),
            embedding_local: self.embedding_local,
            editing,
            ploke_editor: self.ploke_editor.clone(),
            context_management: self.context_management.clone(),
            tooling: self.tooling.clone(),
            chat_policy: self.chat_policy.clone(),
            rag: self.rag.clone(),
            token_limit: self.token_limit,
            tool_retries: self.tool_retries,
            llm_timeout_secs: self.llm_timeout_secs,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditProposalStatus {
    Pending,
    Approved,
    Denied,
    Applied,
    Failed(String),
    /// Stale indicates the workspace changed enough that the proposal likely no longer applies.
    /// TODO: wire to validation/DB checks to detect stale edits vs. live workspace content.
    Stale(String),
}

impl EditProposalStatus {
    pub(crate) fn as_str_outer(&self) -> &'static str {
        match &self {
            EditProposalStatus::Pending => "Pending",
            EditProposalStatus::Approved => "Approved",
            EditProposalStatus::Denied => "Denied",
            EditProposalStatus::Applied => "Applied",
            EditProposalStatus::Failed(_) => "Failed",
            EditProposalStatus::Stale(_) => "Stale",
        }
    }
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
    pub edits_ns: Vec<NsWriteSnippetData>,
    pub preview: DiffPreview,
    pub status: EditProposalStatus,
    /// Whether or not the proposal is for a semantic edit.
    pub is_semantic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProposal {
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub call_id: ArcStr,
    pub proposed_at_ms: i64,
    pub creates: Vec<ploke_core::CreateFileData>,
    pub files: Vec<PathBuf>,
    pub preview: DiffPreview,
    pub status: EditProposalStatus,
}

impl AppState {
    pub fn new(
        db: Arc<Database>,
        embedder: Arc<EmbeddingRuntime>,
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
            create_proposals: RwLock::new(HashMap::new()),
            rag: Some(rag),
            budget,
        }
    }
}

#[derive(Debug, Default)]
pub struct SystemStatus {
    pub(crate) workspace_roots: WorkspaceRoots,
    pub(crate) crate_focus: Option<CrateId>,
    pub(crate) crate_versions: HashMap<CrateId, u64>,
    pub(crate) crate_deps: HashMap<CrateId, Vec<CrateId>>,
    pub(crate) invalidated_crates: HashSet<CrateId>,
    pub(crate) no_workspace_tip_shown: bool,
    pub(crate) last_parse_failure: Option<ParseFailure>,
    pub(crate) last_parse_success_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ParseFailure {
    pub target_dir: PathBuf,
    pub message: String,
    pub occurred_at_ms: i64,
}

impl SystemStatus {
    pub fn new(crate_focus: Option<PathBuf>) -> Self {
        let mut workspace_roots = WorkspaceRoots::default();
        let mut crate_versions = HashMap::new();
        let crate_focus = crate_focus.map(|path| {
            let info = CrateInfo::from_root_path(path);
            let id = info.id;
            workspace_roots.upsert(info);
            crate_versions.entry(id).or_insert(0);
            id
        });
        Self {
            workspace_roots,
            crate_focus,
            crate_versions,
            crate_deps: HashMap::new(),
            invalidated_crates: HashSet::new(),
            no_workspace_tip_shown: false,
            last_parse_failure: None,
            last_parse_success_ms: None,
        }
    }

    pub fn focused_crate(&self) -> Option<&CrateInfo> {
        self.crate_focus
            .and_then(|id| self.workspace_roots.find_by_id(id))
    }

    pub fn focused_crate_root(&self) -> Option<PathBuf> {
        self.focused_crate().map(|info| info.root_path.clone())
    }

    pub fn focused_crate_name(&self) -> Option<&str> {
        self.focused_crate().map(|info| info.name.as_str())
    }

    pub fn set_focus_from_root(&mut self, root: PathBuf) -> CrateId {
        let info = CrateInfo::from_root_path(root);
        let id = info.id;
        self.workspace_roots.upsert(info);
        self.crate_versions.entry(id).or_insert(0);
        self.crate_focus = Some(id);
        id
    }

    pub fn set_crate_deps(&mut self, crate_id: CrateId, deps: Vec<CrateId>) {
        self.crate_deps.insert(crate_id, deps);
    }

    pub fn record_index_complete(&mut self, crate_id: CrateId) -> u64 {
        let dependents = self.dependents_of(crate_id);
        let version = self
            .crate_versions
            .entry(crate_id)
            .and_modify(|v| *v += 1)
            .or_insert(1);
        self.invalidated_crates.remove(&crate_id);
        for dependent in dependents {
            self.invalidated_crates.insert(dependent);
        }
        *version
    }

    pub fn focused_crate_stale(&self) -> Option<bool> {
        let crate_id = self.crate_focus?;
        Some(self.invalidated_crates.contains(&crate_id))
    }

    pub fn record_parse_failure(&mut self, target_dir: PathBuf, message: String) {
        self.last_parse_failure = Some(ParseFailure {
            target_dir,
            message,
            occurred_at_ms: Utc::now().timestamp_millis(),
        });
    }

    pub fn record_parse_success(&mut self) {
        self.last_parse_failure = None;
        self.last_parse_success_ms = Some(Utc::now().timestamp_millis());
    }

    pub fn last_parse_failure(&self) -> Option<&ParseFailure> {
        self.last_parse_failure.as_ref()
    }

    fn dependents_of(&self, changed: CrateId) -> Vec<CrateId> {
        self.crate_deps
            .iter()
            .filter_map(|(crate_id, deps)| {
                if deps.contains(&changed) {
                    Some(*crate_id)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn derive_path_policy(&self, extra_read_roots: &[PathBuf]) -> Option<PathPolicy> {
        let focus_root = self.focused_crate_root()?;
        let mut roots = Vec::with_capacity(1 + extra_read_roots.len());
        roots.push(focus_root);
        roots.extend(extra_read_roots.iter().cloned());
        Some(PathPolicy {
            roots,
            symlink_policy: SymlinkPolicy::DenyCrossRoot,
            require_absolute: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SystemStatus;
    use std::path::PathBuf;

    #[test]
    fn record_index_complete_marks_dependents_stale() {
        let mut status = SystemStatus::default();
        let root_a = std::env::temp_dir().join("ploke_test_crate_a");
        let root_b = std::env::temp_dir().join("ploke_test_crate_b");
        let id_a = status.set_focus_from_root(root_a);
        let id_b = status.set_focus_from_root(root_b);

        status.set_crate_deps(id_b, vec![id_a]);

        let version = status.record_index_complete(id_a);
        assert_eq!(version, 1);
        assert!(status.invalidated_crates.contains(&id_b));
        assert!(!status.invalidated_crates.contains(&id_a));
    }
}
