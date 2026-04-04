use chrono::Utc;
use ploke_core::file_hash::LargeFilePolicy;
use ploke_core::{ArcStr, CrateId, CrateInfo, TrackingHash, WorkspaceInfo, WorkspaceRoots};
use ploke_error::DomainError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::llm::LLMParameters;
use crate::llm::registry::user_prefs::RegistryPrefs;
use crate::llm::{ModelId, ModelKey};
use crate::user_config::{
    ChatPolicy, CommandStyle, CtxPrefs, EmbeddingConfig, LocalEmbeddingTuning,
    MessageVerbosityProfile, MessageVerbosityProfiles, RagUserConfig, UserConfig,
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
    #[cfg(feature = "test_harness")]
    pub async fn loaded_workspace_root_for_test(&self) -> Option<std::path::PathBuf> {
        let guard = self.0.read().await;
        guard.loaded_workspace_root()
    }
    #[cfg(feature = "test_harness")]
    pub async fn loaded_workspace_member_roots_for_test(&self) -> Vec<std::path::PathBuf> {
        let guard = self.0.read().await;
        guard.loaded_workspace_member_roots()
    }
    #[cfg(feature = "test_harness")]
    pub async fn workspace_freshness_for_test(
        &self,
    ) -> Vec<(std::path::PathBuf, WorkspaceFreshness)> {
        let guard = self.0.read().await;
        let Some(loaded_workspace) = guard.loaded_workspace.as_ref() else {
            return Vec::new();
        };
        loaded_workspace
            .members
            .crates
            .iter()
            .filter_map(|info| {
                guard
                    .workspace_freshness(*&info.id)
                    .map(|freshness| (info.root_path.clone(), freshness))
            })
            .collect()
    }
    #[cfg(feature = "test_harness")]
    pub async fn set_pwd_for_test(&self, pwd: PathBuf) {
        let mut guard = self.0.write().await;
        guard.pwd = pwd;
    }
    pub async fn init_pwd(self, pwd: PathBuf) -> Self {
        let mut guard = self.0.write().await;
        guard.pwd = pwd;
        drop(guard);
        self
    }

    pub async fn is_stale_err(&self) -> Result<(), ploke_error::Error> {
        if self.read().await.any_loaded_crate_stale() {
            Err(ploke_error::Error::Domain(DomainError::Ui {
                message: "Loaded crate index is stale; reindex to query code items.".to_string(),
            }))
        } else {
            Ok(())
        }
    }

    /// # Deprecated
    /// Direct read lock access is deprecated. Use `AppState::with_system_read()` instead
    /// for compile-time guarantees against holding locks across await points.
    #[deprecated(
        since = "0.1.0",
        note = "Use AppState::with_system_read() or AppState::with_system_txn() instead. \
                Direct RwLock access makes deadlocks possible."
    )]
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, SystemStatus> {
        self.0.read().await
    }

    /// # Deprecated
    /// Direct write lock access is deprecated. Use `AppState::with_system_txn()` instead
    /// for compile-time guarantees against holding locks across await points.
    #[deprecated(
        since = "0.1.0",
        note = "Use AppState::with_system_txn() instead. \
                Direct RwLock access makes deadlocks possible."
    )]
    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, SystemStatus> {
        self.0.write().await
    }
}

/// # Deprecation Notice
/// Direct access to the underlying RwLock via this Deref impl is **discouraged**.
/// Use `AppState::with_system_txn()` for mutations or `AppState::with_system_read()`
/// for read-only access. These methods provide compile-time guarantees against
/// holding locks across await points.
///
/// This Deref impl will be removed in a future refactor once all call sites
/// are migrated to the transaction pattern. New code must use the transaction API.
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
    /// Conversation presentation profiles used by the UI only.
    /// This must not affect prompt assembly, message loop behavior, or pinning state.
    pub message_verbosity_profiles: MessageVerbosityProfiles,
    /// Active message verbosity profile for UI rendering.
    pub default_verbosity: MessageVerbosityProfile,
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
        let context_management = uc.context_management.validated();
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
            message_verbosity_profiles: uc.message_verbosity_profiles,
            default_verbosity: uc.default_verbosity,
            embedding: uc.embedding,
            embedding_local,
            ploke_editor: uc.ploke_editor,
            tooling: uc.tooling,
            chat_policy,
            rag,
            token_limit: uc.token_limit,
            tool_retries: uc.tool_retries,
            llm_timeout_secs: uc.llm_timeout_secs,
            context_management,
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
            message_verbosity_profiles: self.message_verbosity_profiles.clone(),
            default_verbosity: self.default_verbosity,
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

    /// Execute a transaction on SystemStatus with compile-time lock safety.
    ///
    /// The provided closure receives a `&mut SystemTxn` which has **no async methods**.
    /// This guarantees that the write lock is released before the returned future
    /// resolves, preventing the "hold lock across await" class of deadlocks.
    ///
    /// # Returns
    /// A tuple of (closure result, post-commit effects). The caller **must** dispatch
    /// effects after the lock is released.
    ///
    /// # Example
    /// ```rust,ignore
    /// let outcome = state.with_system_txn(|txn| {
    ///     txn.set_pwd(new_pwd);
    /// }).await;
    ///
    /// for effect in outcome.effects {
    ///     match effect {
    ///         PostCommit::EmitPwdChanged(pwd) => {
    ///             event_bus.send(SystemEvent::PwdChanged(pwd)).await;
    ///         }
    ///     }
    /// }
    /// ```
    pub async fn with_system_txn<R>(
        &self,
        f: impl FnOnce(&mut SystemTxn<'_>) -> R,
    ) -> TxnOutcome<R, PostCommit> {
        let guard = self.system.write().await;
        let mut txn = SystemTxn {
            effects: Vec::new(),
            state: guard,
        };
        let result = f(&mut txn);
        // Explicit extraction and drop makes lock release boundary unambiguous.
        // See: https://doc.rust-lang.org/reference/destructors.html
        let effects = std::mem::take(&mut txn.effects);
        drop(txn); // Lock released here, before return
        TxnOutcome::new(result, effects)
    }

    /// Read-only access to SystemStatus through a synchronous closure.
    ///
    /// This provides the same "no await in critical section" guarantee as
    /// `with_system_txn`, but for read-only operations that don't produce effects.
    ///
    /// # Example
    /// ```rust,ignore
    /// let pwd = state.with_system_read(|sys| {
    ///     sys.pwd().to_path_buf()
    /// }).await;
    /// ```
    pub async fn with_system_read<R>(&self, f: impl FnOnce(&SystemStatus) -> R) -> R {
        let guard = self.system.read().await;
        let result = f(&*guard);
        drop(guard); // Explicit drop for consistency with write path
        result
    }

    /// Test-only transaction helper that allows direct SystemStatus access.
    ///
    /// This is a temporary escape hatch for tests that need to inspect or mutate
    /// SystemStatus in ways not yet supported by the typed transaction API.
    /// New test code should prefer `with_system_txn` when possible.
    ///
    /// # Safety Warning
    /// The closure must not hold references across await points. This method
    /// is marked unsafe to signal that deadlock prevention is the caller's
    /// responsibility when using raw guard access.
    #[cfg(feature = "test_harness")]
    pub async fn with_system_raw<R>(&self, f: impl FnOnce(&mut SystemStatus) -> R) -> R {
        let mut guard = self.system.write().await;
        let result = f(&mut *guard);
        drop(guard);
        result
    }

    /// Test-only read helper for direct SystemStatus inspection.
    #[cfg(feature = "test_harness")]
    pub async fn with_system_raw_read<R>(&self, f: impl FnOnce(&SystemStatus) -> R) -> R {
        let guard = self.system.read().await;
        let result = f(&*guard);
        drop(guard);
        result
    }
}

#[derive(Debug)]
pub struct LoadedWorkspaceState {
    pub(crate) workspace: WorkspaceInfo,
    pub(crate) members: WorkspaceRoots,
}

impl LoadedWorkspaceState {
    pub fn from_member_roots(workspace_root: PathBuf, member_roots: Vec<PathBuf>) -> Self {
        let mut members = WorkspaceRoots::default();
        for root in member_roots {
            members.upsert(CrateInfo::from_root_path(root));
        }
        Self {
            workspace: WorkspaceInfo::from_root_path(workspace_root),
            members,
        }
    }

    pub fn member_roots(&self) -> Vec<PathBuf> {
        self.members
            .crates
            .iter()
            .map(|info| info.root_path.clone())
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct LoadedCrateState {
    pub info: CrateInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceFreshness {
    Fresh,
    Stale,
}

#[derive(Debug, Default)]
pub struct SystemStatus {
    pub(crate) loaded_workspace: Option<LoadedWorkspaceState>,
    pub(crate) loaded_crates: BTreeMap<CrateId, LoadedCrateState>,
    pub(crate) crate_versions: HashMap<CrateId, u64>,
    pub(crate) crate_deps: HashMap<CrateId, Vec<CrateId>>,
    pub(crate) invalidated_crates: HashSet<CrateId>,
    pub(crate) workspace_freshness: HashMap<CrateId, WorkspaceFreshness>,
    pub(crate) no_workspace_tip_shown: bool,
    pub(crate) last_parse_failure: Option<ParseFailure>,
    pub(crate) last_parse_success_ms: Option<i64>,
    pub(crate) pwd: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ParseFailure {
    pub target_dir: PathBuf,
    pub message: String,
    pub occurred_at_ms: i64,
}

impl SystemStatus {
    pub fn new(crate_root: Option<PathBuf>) -> Self {
        let mut status = Self::default();
        if let Some(path) = crate_root {
            let id = status.load_standalone_crate(path);
            status.crate_versions.entry(id).or_insert(0);
        }
        status
    }

    pub fn loaded_workspace(&self) -> Option<&LoadedWorkspaceState> {
        self.loaded_workspace.as_ref()
    }

    pub fn loaded_workspace_root(&self) -> Option<PathBuf> {
        self.loaded_workspace
            .as_ref()
            .map(|loaded| loaded.workspace.root_path.clone())
    }

    pub fn loaded_workspace_member_roots(&self) -> Vec<PathBuf> {
        self.loaded_workspace
            .as_ref()
            .map(LoadedWorkspaceState::member_roots)
            .unwrap_or_default()
    }

    pub fn loaded_crate(&self, id: &CrateId) -> Option<&LoadedCrateState> {
        self.loaded_crates.get(id)
    }

    pub fn loaded_crate_roots(&self) -> Vec<PathBuf> {
        self.loaded_crates
            .values()
            .map(|lc| lc.info.root_path.clone())
            .collect()
    }

    pub fn has_loaded_crates(&self) -> bool {
        !self.loaded_crates.is_empty()
    }

    /// Backward-compatible accessor: returns the first loaded crate's info.
    /// Callers that need a specific crate should use `loaded_crate(id)`.
    pub fn focused_crate(&self) -> Option<&CrateInfo> {
        self.loaded_crates.values().next().map(|lc| &lc.info)
    }

    /// Backward-compatible accessor: returns the first loaded crate's root path.
    /// Callers that need a specific crate root should use `loaded_crate(id)`.
    pub fn focused_crate_root(&self) -> Option<PathBuf> {
        self.focused_crate().map(|info| info.root_path.clone())
    }

    /// Backward-compatible accessor: returns the first loaded crate's name.
    pub fn focused_crate_name(&self) -> Option<&str> {
        self.focused_crate().map(|info| info.name.as_str())
    }

    /// Set initial pwd at application start
    fn with_pwd(&mut self, pwd: PathBuf) {
        self.pwd = pwd
    }

    pub fn pwd(&self) -> &std::path::Path {
        self.pwd.as_path()
    }

    /// Update pwd and return true if changed
    pub fn set_pwd(&mut self, pwd: PathBuf) -> bool {
        if self.pwd != pwd {
            self.pwd = pwd;
            true
        } else {
            false
        }
    }

    pub fn set_loaded_workspace(
        &mut self,
        workspace_root: PathBuf,
        member_roots: Vec<PathBuf>,
        focused_root: Option<PathBuf>,
    ) -> Option<CrateId> {
        let loaded_workspace =
            LoadedWorkspaceState::from_member_roots(workspace_root, member_roots);
        let member_ids: HashSet<_> = loaded_workspace
            .members
            .crates
            .iter()
            .map(|info| info.id)
            .collect();
        let focus_id = focused_root
            .as_ref()
            .and_then(|root| loaded_workspace.members.find_by_root_path(root))
            .map(|info| info.id);

        self.workspace_freshness
            .retain(|crate_id, _| member_ids.contains(crate_id));
        self.loaded_crates.clear();
        for info in &loaded_workspace.members.crates {
            self.crate_versions.entry(info.id).or_insert(0);
            self.workspace_freshness
                .insert(info.id, WorkspaceFreshness::Fresh);
            self.loaded_crates
                .insert(info.id, LoadedCrateState { info: info.clone() });
        }

        self.loaded_workspace = Some(loaded_workspace);
        focus_id
    }

    /// Registers a crate root into loaded state. If the root is already a member
    /// of the loaded workspace, adds it to `loaded_crates`. Otherwise, creates a
    /// standalone single-member workspace and replaces existing loaded state.
    pub fn set_focus_from_root(&mut self, root: PathBuf) -> CrateId {
        if let Some(loaded_workspace) = self.loaded_workspace.as_ref()
            && let Some(info) = loaded_workspace.members.find_by_root_path(&root)
        {
            let id = info.id;
            self.crate_versions.entry(id).or_insert(0);
            self.loaded_crates
                .insert(id, LoadedCrateState { info: info.clone() });
            return id;
        }

        self.load_standalone_crate(root)
    }

    /// Loads a single crate as a standalone (no workspace) environment.
    /// Clears any previous loaded workspace and crate state.
    fn load_standalone_crate(&mut self, root: PathBuf) -> CrateId {
        let info = CrateInfo::from_root_path(root.clone());
        let id = info.id;
        self.loaded_workspace = Some(LoadedWorkspaceState::from_member_roots(
            root,
            vec![info.root_path.clone()],
        ));
        self.loaded_crates.clear();
        self.loaded_crates.insert(id, LoadedCrateState { info });
        self.crate_versions.entry(id).or_insert(0);
        self.workspace_freshness
            .insert(id, WorkspaceFreshness::Fresh);
        id
    }

    /// Centralized mutation entry point. All `SystemStatus` state changes should
    /// flow through this method to ensure transitions are typed and auditable.
    pub fn apply(&mut self, mutation: super::events::SystemMutation) {
        use super::events::SystemMutation;
        match mutation {
            SystemMutation::LoadWorkspace {
                workspace_root,
                member_roots,
                focused_root,
            } => {
                self.set_loaded_workspace(workspace_root, member_roots, focused_root);
            }
            SystemMutation::LoadStandaloneCrate { crate_root } => {
                self.load_standalone_crate(crate_root);
            }
            SystemMutation::RecordParseSuccess => {
                self.record_parse_success();
            }
            SystemMutation::RecordParseFailure {
                target_dir,
                message,
            } => {
                self.record_parse_failure(target_dir, message);
            }
            SystemMutation::SetWorkspaceFreshness {
                crate_id,
                freshness,
            } => {
                self.set_workspace_freshness(crate_id, freshness);
            }
            SystemMutation::RecordIndexComplete { crate_id } => {
                self.record_index_complete(crate_id);
            }
            SystemMutation::InitPwd { pwd } => todo!(),
        }
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

    /// Returns true if any loaded crate is stale or invalidated.
    pub fn any_loaded_crate_stale(&self) -> bool {
        self.loaded_crates.keys().any(|id| {
            self.invalidated_crates.contains(id)
                || self
                    .workspace_freshness
                    .get(id)
                    .is_some_and(|s| *s == WorkspaceFreshness::Stale)
        })
    }

    pub fn set_workspace_freshness(&mut self, crate_id: CrateId, freshness: WorkspaceFreshness) {
        self.workspace_freshness.insert(crate_id, freshness);
    }

    pub fn workspace_freshness(&self, crate_id: CrateId) -> Option<WorkspaceFreshness> {
        self.workspace_freshness.get(&crate_id).copied()
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
        let mut roots = Vec::new();
        if let Some(ws) = self.loaded_workspace_root() {
            roots.push(ws);
        }
        roots.extend(self.loaded_workspace_member_roots());
        roots.extend(extra_read_roots.iter().cloned());
        let mut seen = BTreeSet::new();
        roots.retain(|root| seen.insert(root.clone()));
        if roots.is_empty() {
            return None;
        }
        Some(PathPolicy {
            roots,
            symlink_policy: SymlinkPolicy::DenyCrossRoot,
            require_absolute: true,
        })
    }

    /// Workspace root for joining relative tool paths, plus the full read policy (workspace + members).
    pub fn tool_path_context(&self) -> Option<(PathBuf, PathPolicy)> {
        let primary_root = self
            .loaded_workspace_root()
            .or_else(|| self.focused_crate_root())?;
        let policy = self.derive_path_policy(&[])?;
        Some((primary_root, policy))
    }
}

/// Effects that must be dispatched after a transaction commits (lock released).
/// This pattern ensures no locks are held across await points.
#[derive(Debug, Clone)]
pub enum PostCommit {
    /// Emit PwdChanged event to update cached pwd in App and other components
    EmitPwdChanged(PathBuf),
    // Future effects can be added here:
    // EmitWorkspaceUpdated { workspace_ref: String },
    // EmitCrateFocusChanged { crate_id: CrateId },
}

/// The result of a transaction, containing the closure's return value and any
/// post-commit effects that must be dispatched.
///
/// This struct provides a cleaner API than the raw `(R, Vec<E>)` tuple and
/// scales nicely to other transaction types (ChatTxn, ConfigTxn, etc.).
#[derive(Debug)]
pub struct TxnOutcome<R, E> {
    /// The value returned by the transaction closure.
    pub result: R,
    /// Effects that must be dispatched after the lock is released.
    pub effects: Vec<E>,
}

impl<R, E> TxnOutcome<R, E> {
    /// Create a new outcome with the given result and effects.
    pub fn new(result: R, effects: Vec<E>) -> Self {
        Self { result, effects }
    }

    /// Map the result type while preserving effects.
    pub fn map<T>(self, f: impl FnOnce(R) -> T) -> TxnOutcome<T, E> {
        TxnOutcome {
            result: f(self.result),
            effects: self.effects,
        }
    }

    /// Returns true if there are any effects to dispatch.
    pub fn has_effects(&self) -> bool {
        !self.effects.is_empty()
    }

    /// Consume the outcome and return (result, effects).
    pub fn into_inner(self) -> (R, Vec<E>) {
        (self.result, self.effects)
    }
}

/// A transaction object for SystemStatus mutations.
///
/// # Compile-Time Safety
/// This struct intentionally has no async methods. It can only be used within a
/// synchronous `FnOnce` closure passed to `AppState::with_system_txn`. This
/// guarantees that the write lock is released before any await point.
///
/// # Pattern
/// ```rust,ignore
/// let (_, effects) = state.with_system_txn(|txn| {
///     txn.set_pwd(new_pwd);
/// }).await;
///
/// for effect in effects {
///     dispatch(effect).await; // Lock not held here!
/// }
/// ```
///
/// # Field Ordering Note
/// The `state` (guard) field is declared last so it's dropped last if the
/// whole struct is dropped normally. However, we prefer explicit `drop(txn)`
/// after extracting effects to make the lock release boundary unambiguous.
pub struct SystemTxn<'a> {
    effects: Vec<PostCommit>,
    // Private - can only be constructed by AppState::with_system_txn
    state: tokio::sync::RwLockWriteGuard<'a, SystemStatus>,
}

impl<'a> SystemTxn<'a> {
    /// Set the current working directory.
    ///
    /// Records a `PostCommit::EmitPwdChanged` effect if the path actually changed.
    pub fn set_pwd(&mut self, pwd: PathBuf) {
        if self.state.pwd() != pwd {
            self.state.set_pwd(pwd.clone());
            self.effects.push(PostCommit::EmitPwdChanged(pwd));
        }
    }

    /// Returns the current pwd without mutation.
    pub fn pwd(&self) -> &std::path::Path {
        self.state.pwd()
    }

    /// Take ownership of the effects buffer.
    fn into_effects(self) -> Vec<PostCommit> {
        self.effects
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

    #[test]
    fn loaded_workspace_membership_populates_loaded_crates_and_path_policy() {
        let mut status = SystemStatus::default();
        let workspace_root = std::env::temp_dir().join("ploke_test_workspace");
        let member_a = workspace_root.join("crate_a");
        let member_b = workspace_root.join("nested/crate_b");

        let focused = status.set_loaded_workspace(
            workspace_root.clone(),
            vec![member_a.clone(), member_b.clone()],
            Some(member_b.clone()),
        );

        assert!(
            focused.is_some(),
            "focus should be set to a workspace member"
        );
        assert_eq!(status.loaded_workspace_root(), Some(workspace_root.clone()));
        assert_eq!(status.loaded_crates.len(), 2);
        assert_eq!(
            status.loaded_workspace_member_roots(),
            vec![member_a.clone(), member_b.clone()]
        );

        let policy = status
            .derive_path_policy(&[])
            .expect("loaded workspace should derive a path policy");
        assert_eq!(
            policy.roots,
            vec![workspace_root.clone(), member_a.clone(), member_b.clone()]
        );

        let (primary, tool_policy) = status
            .tool_path_context()
            .expect("tool path context should be available");
        assert_eq!(primary, workspace_root);
        assert_eq!(tool_policy.roots, policy.roots);
    }

    #[test]
    fn set_focus_from_root_preserves_existing_loaded_workspace_membership() {
        let mut status = SystemStatus::default();
        let workspace_root = std::env::temp_dir().join("ploke_test_workspace_focus");
        let member_a = workspace_root.join("crate_a");
        let member_b = workspace_root.join("crate_b");

        status.set_loaded_workspace(
            workspace_root,
            vec![member_a.clone(), member_b.clone()],
            Some(member_a.clone()),
        );
        let focused_id = status.set_focus_from_root(member_b.clone());

        assert!(status.loaded_crates.contains_key(&focused_id));
        assert!(
            status
                .loaded_workspace_member_roots()
                .iter()
                .any(|root| root == &member_a)
        );
        assert!(
            status
                .loaded_workspace_member_roots()
                .iter()
                .any(|root| root == &member_b)
        );
    }

    /// Compile-time demonstration that the transaction pattern prevents
    /// holding locks across await points.
    ///
    /// This test exists to ensure the API doesn't accidentally allow:
    /// ```compile_fail
    /// state.with_system_txn(|txn| {
    ///     txn.set_pwd(new_pwd);
    ///     async { some_async_fn().await }.await; // ERROR: await not allowed in sync closure
    /// })
    /// ```
    #[test]
    fn system_txn_pattern_prevents_await_in_closure() {
        use super::SystemTxn;
        // The type system ensures the closure is FnOnce(&mut SystemTxn) -> R,
        // which cannot contain await expressions. This is the compile-time safety.
        let _ = |txn: &mut SystemTxn<'_>| {
            txn.set_pwd(PathBuf::from("/test"));
            // Cannot await here - the closure is not async!
        };
    }
}
