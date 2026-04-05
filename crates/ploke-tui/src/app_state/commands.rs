use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use crate::ModelId;
use crate::app::commands::parser::LoadKind;
use crate::app_state::database::IndexTargetDir;
use crate::chat_history::{ContextTokens, MessageKind};
use crate::llm::{ChatHistoryTarget, LLMParameters, ProviderKey};
use ploke_core::ArcStr;
use ploke_core::embeddings::EmbeddingProviderSlug;
use ploke_llm::ProviderName;
use ploke_rag::{RetrievalStrategy, TokenBudget};
use syn_parser::ManifestKind;
use syn_parser::discovery::workspace::try_parse_manifest;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::event_bus::ErrorEvent;
use crate::parser::resolve_index_target;
use crate::user_config::WorkspaceRegistry;
use crate::{AppEvent, ErrorSeverity, EventBus};

#[derive(thiserror::Error, Clone, Debug)]
pub enum StateError {
    #[error("The app state does not have a currently set crate focus")]
    MissingCrateFocus { msg: &'static str },
}

impl From<StateError> for ploke_error::Error {
    fn from(value: StateError) -> Self {
        match value {
            StateError::MissingCrateFocus { msg } => {
                ploke_error::Error::Domain(ploke_error::domain::DomainError::Ui {
                    message: msg.to_string(),
                })
            }
        }
    }
}

/// Directions which can be taken when selecting an item in a list.
#[derive(Debug, Clone, Copy)]
pub enum ListNavigation {
    Up,
    Down,
    Top,
    Bottom,
}

// =============================================================================
// COMMAND GROUPING ARCHITECTURE
// =============================================================================
//
// This module is transitioning from a flat `StateCommand` enum to a grouped
// architecture where commands are organized by subsystem and validation
// requirements.
//
// ARCHITECTURAL DIRECTION:
// ------------------------
// The eventual goal is to split StateCommand into typed groups:
//
// ```rust
// pub enum StateCommand {
//     Chat(ChatCmd),         // No validation - UI/chat operations
//     Workspace(WorkspaceCmd), // Validates: has_loaded_crates()
//     Db(DbCmd),             // Validates: DB accessible
//     Llm(LlmCmd),           // Validates: LLM configured
//     Rag(RagCmd),           // Validates: workspace loaded (for some)
// }
// ```
//
// Each group implements `Validate` trait:
// ```rust
// pub trait Validate {
//     fn validate(&self, state: &AppState) -> Result<(), ValidationError>;
// }
// ```
//
// The dispatcher validates before handling:
// ```rust
// StateCommand::Workspace(cmd) => {
//     match cmd.validate(&state) {
//         Ok(()) => handle_workspace(state, event_bus, cmd).await,
//         Err(e) => emit_validation_error(event_bus, e).await,
//     }
// }
// ```
//
// MIGRATION STATUS:
// ----------------
// - ✅ WorkspaceCmd: Extracted and validated
// - ⏳ DbCmd: Planned - will include ReadQuery, WriteQuery, BatchPromptSearch
// - ⏳ IndexCmd: decision-tree command for `/index`
// - ⏳ ChatCmd: Planned - all chat/history operations (AddMessage*, etc.)
// - ⏳ LlmCmd: Planned - SwitchModel, SelectModelProvider, etc.
// - ⏳ RagCmd: Planned - Bm25*, Hybrid*, ApproveEdits, etc.
//
// When adding new commands:
// 1. Determine which subsystem they belong to
// 2. If validation needed, add to appropriate group with Validate impl
// 3. If no validation, add to ChatCmd or create new group
// =============================================================================

/// Validation errors that prevent command execution.
#[derive(Debug, thiserror::Error, Clone)]
pub enum ValidationError {
    #[error("No crate or workspace is loaded")]
    NoWorkspaceLoaded,
    #[error("Database is not available")]
    NoDatabase,
    #[error("LLM is not configured")]
    NoLlmConfigured,
    #[error("{0}")]
    Other(String),
}

/// Trait for commands that require validation before execution.
///
/// Implement this for command groups that need to check preconditions
/// (e.g., workspace loaded, DB available, etc.) before handling.
pub trait Validate {
    /// Validates the command against current application state.
    ///
    /// Returns Ok(()) if validation passes, Err(ValidationError) if not.
    /// The error will be emitted as an AppEvent::Error and the command
    /// will not be executed.
    fn validate(
        &self,
        state: &super::AppState,
    ) -> impl std::future::Future<Output = Result<(), ValidationError>> + Send;
}

/// `/index` mode selected by the parser/executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexMode {
    Auto,
    Workspace,
    Crate,
}

/// `/index` command forwarded from the executor.
#[derive(Debug, Clone)]
pub struct IndexCmd {
    pub mode: IndexMode,
    pub target: Option<String>,
}

/// `/load` command forwarded from the executor.
#[derive(Debug, Clone)]
pub struct LoadCmd {
    pub kind: LoadKind,
    pub name: Option<String>,
    pub force: bool,
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum LoadValidationError {
    #[error("Current loaded crate or workspace has stale state")]
    StaleLoadedState,
}

impl LoadValidationError {
    pub fn recovery_suggestion(&self) -> String {
        match self {
            LoadValidationError::StaleLoadedState => {
                "`/save db` before loading another snapshot, or rerunning the load with `--force`."
                    .to_string()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadResolution {
    pub workspace_ref: String,
    pub replaces_loaded_state: bool,
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum LoadResolveError {
    #[error("Missing load target")]
    MissingTarget,
    #[error("Crate '{target}' is already loaded.")]
    AlreadyLoadedCrate { target: String },
    #[error("Crate '{target}' is already loaded in the current workspace.")]
    AlreadyLoadedWorkspaceMember { target: String },
    #[error("Workspace '{target}' is already loaded.")]
    AlreadyLoadedWorkspace { target: String },
    #[error("No saved crate named '{target}' was found.")]
    MissingSavedCrate { target: String },
    #[error("No saved workspace named '{target}' was found.")]
    MissingSavedWorkspace { target: String },
    #[error("Workspace '{target}' was not found.")]
    WorkspaceTargetLooksLikeCrate { target: String },
    #[error("Saved workspace lookup for '{target}' is ambiguous.")]
    AmbiguousSavedTarget { target: String },
    #[error("Failed to load the saved workspace registry.")]
    RegistryUnavailable,
}

impl LoadResolveError {
    pub fn user_message(&self) -> String {
        self.to_string()
    }

    pub fn recovery_suggestion(&self) -> String {
        match self {
            LoadResolveError::MissingTarget => {
                "providing a saved workspace name or id.".to_string()
            }
            LoadResolveError::AlreadyLoadedCrate { target }
            | LoadResolveError::AlreadyLoadedWorkspace { target } => {
                format!("`/index` to re-index '{target}'.")
            }
            LoadResolveError::AlreadyLoadedWorkspaceMember { target } => {
                format!("`/index crate {target}` to re-index that member.")
            }
            LoadResolveError::MissingSavedCrate { target } => {
                format!("`/index crate {target}` to index it before loading.")
            }
            LoadResolveError::MissingSavedWorkspace { .. } => {
                "`/index workspace` from the workspace root before loading it.".to_string()
            }
            LoadResolveError::WorkspaceTargetLooksLikeCrate { target } => {
                format!("`/load crate {target}` if you meant the crate.")
            }
            LoadResolveError::AmbiguousSavedTarget { target } => {
                format!("the exact workspace id for '{target}'.")
            }
            LoadResolveError::RegistryUnavailable => {
                "checking the saved workspace registry configuration.".to_string()
            }
        }
    }
}

/// Resolved `/index` request after applying mode + target semantics.
#[derive(Debug, Clone)]
pub struct IndexResolution {
    pub target_dir: IndexTargetDir,
    pub needs_parse: bool,
    pub focus_root: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum IndexResolveError {
    // Informative errors stay direct; recovery text is formatted separately.
    #[error(
        "Current directory is not a loaded crate. Use `/index crate <path>` to index a specific crate."
    )]
    CurrentDirectoryNotLoadedCrate,
    #[error("Current directory is not a workspace member")]
    CurrentDirectoryNotWorkspaceMember,
    #[error("Cannot resolve `/index workspace` without loaded workspace context")]
    MissingWorkspaceContext,
    #[error("Cannot resolve `/index crate` without focused crate context")]
    MissingCrateContext,
    #[error("{0}")]
    TargetResolution(String),
}

impl IndexCmd {
    /// Resolve `/index` into a concrete target directory using current state.
    pub async fn resolve(
        &self,
        state: &super::AppState,
    ) -> Result<IndexResolution, IndexResolveError> {
        let (pwd, loaded_workspace_root, loaded_member_roots, focused_crate_root) = state
            .with_system_read(|sys| {
                (
                    sys.pwd().to_path_buf(),
                    sys.loaded_workspace_root(),
                    sys.loaded_workspace_member_roots(),
                    sys.focused_crate_root(),
                )
            })
            .await;

        let has_loaded_context = !loaded_member_roots.is_empty();
        let pwd_is_loaded_member = loaded_member_roots.iter().any(|root| root == &pwd);
        let pwd_is_workspace_root = loaded_workspace_root
            .as_ref()
            .is_some_and(|root| root == &pwd);
        let standalone_loaded_workspace =
            loaded_member_roots.len() == 1 && loaded_workspace_root == focused_crate_root;
        let base_dir = match self.mode {
            IndexMode::Auto => {
                if pwd_is_loaded_member {
                    pwd.clone()
                } else if pwd_is_workspace_root {
                    if loaded_member_roots.len() == 1 {
                        focused_crate_root
                            .clone()
                            .ok_or(IndexResolveError::MissingCrateContext)?
                    } else {
                        loaded_workspace_root
                            .clone()
                            .ok_or(IndexResolveError::MissingWorkspaceContext)?
                    }
                } else if has_loaded_context {
                    return Err(IndexResolveError::CurrentDirectoryNotLoadedCrate);
                } else {
                    pwd.clone()
                }
            }
            IndexMode::Workspace => {
                if standalone_loaded_workspace {
                    return Err(IndexResolveError::CurrentDirectoryNotWorkspaceMember);
                }

                if pwd_is_loaded_member || pwd_is_workspace_root {
                    loaded_workspace_root
                        .clone()
                        .ok_or(IndexResolveError::MissingWorkspaceContext)?
                } else if has_loaded_context {
                    return Err(IndexResolveError::CurrentDirectoryNotWorkspaceMember);
                } else {
                    pwd.clone()
                }
            }
            IndexMode::Crate => {
                if let Some(root) = focused_crate_root.clone() {
                    root
                } else if has_loaded_context {
                    return Err(IndexResolveError::MissingCrateContext);
                } else {
                    pwd.clone()
                }
            }
        };

        let mut focus_root = None;
        let resolved_path = match &self.target {
            Some(target) => {
                let target_path = PathBuf::from(target);
                if matches!(self.mode, IndexMode::Crate) && has_loaded_context {
                    match resolve_loaded_crate_target(
                        &target_path,
                        loaded_workspace_root.as_deref(),
                        &loaded_member_roots,
                        standalone_loaded_workspace,
                    ) {
                        Some(resolved) => {
                            if focused_crate_root.as_ref() != Some(&resolved) {
                                focus_root = Some(resolved.clone());
                            }
                            resolved
                        }
                        None => {
                            return Err(IndexResolveError::TargetResolution(format!(
                                "crate '{target}' is not loaded in the current workspace. Failed to normalize target path."
                            )));
                        }
                    }
                } else if target_path.is_absolute() {
                    target_path
                } else {
                    base_dir.join(target_path)
                }
            }
            None => base_dir,
        };

        let resolved = resolve_index_target(Some(resolved_path), &pwd)
            .map_err(|err| IndexResolveError::TargetResolution(err.to_string()))?;

        Ok(IndexResolution {
            target_dir: IndexTargetDir::new(resolved.requested_path),
            needs_parse: true,
            focus_root,
        })
    }

    pub fn discriminant(&self) -> &'static str {
        "Index"
    }
}

impl LoadCmd {
    /// Returns the discriminant name for logging/debugging.
    pub fn discriminant(&self) -> &'static str {
        "Load"
    }

    pub async fn validate(
        &self,
        state: &super::AppState,
        resolution: &LoadResolution,
    ) -> Result<(), LoadValidationError> {
        if self.force || !resolution.replaces_loaded_state {
            return Ok(());
        }

        let stale_loaded_state = state
            .with_system_read(|sys| sys.any_loaded_crate_stale())
            .await;
        if stale_loaded_state {
            Err(LoadValidationError::StaleLoadedState)
        } else {
            Ok(())
        }
    }

    pub async fn resolve(
        &self,
        state: &super::AppState,
    ) -> Result<LoadResolution, LoadResolveError> {
        let (pwd, loaded_workspace_root, loaded_workspace_name, loaded_crate_refs) = state
            .with_system_read(|sys| {
                let pwd = sys.pwd().to_path_buf();
                let loaded_workspace_root = sys.loaded_workspace_root();
                let loaded_workspace_name = sys
                    .loaded_workspace
                    .as_ref()
                    .map(|loaded| loaded.workspace.name.clone());
                let loaded_crate_refs = sys
                    .loaded_crates
                    .values()
                    .map(|loaded| {
                        (
                            loaded.info.name.clone(),
                            loaded.info.root_path.display().to_string(),
                        )
                    })
                    .collect::<Vec<_>>();
                (
                    pwd,
                    loaded_workspace_root,
                    loaded_workspace_name,
                    loaded_crate_refs,
                )
            })
            .await;
        let has_loaded_context = !loaded_crate_refs.is_empty();
        let workspace_ref = match (self.kind, self.name.clone()) {
            (_, Some(name)) => name,
            (LoadKind::Workspace, None) => pwd
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .ok_or(LoadResolveError::MissingTarget)?,
            (LoadKind::Crate, None) => return Err(LoadResolveError::MissingTarget),
        };

        let matches_loaded_crate = loaded_crate_refs
            .iter()
            .any(|(name, root)| workspace_ref == *name || workspace_ref == *root);

        let matches_loaded_workspace = loaded_workspace_root.as_ref().is_some_and(|root| {
            workspace_ref == root.display().to_string()
                || loaded_workspace_name
                    .as_ref()
                    .is_some_and(|name| workspace_ref == *name)
        });

        if matches_loaded_workspace {
            return Err(LoadResolveError::AlreadyLoadedWorkspace {
                target: workspace_ref,
            });
        }

        if matches_loaded_crate {
            return match self.kind {
                LoadKind::Crate => Err(LoadResolveError::AlreadyLoadedCrate {
                    target: workspace_ref,
                }),
                LoadKind::Workspace => Err(LoadResolveError::AlreadyLoadedWorkspaceMember {
                    target: workspace_ref,
                }),
            };
        }

        match registry_lookup_state(&workspace_ref) {
            Ok(RegistryLookupState::Found) => {}
            Ok(RegistryLookupState::Missing) => {
                if matches!(self.kind, LoadKind::Workspace)
                    && workspace_root_from_pwd(&pwd)
                        .and_then(|root| workspace_member_match(&workspace_ref, root.as_path()))
                        .is_some()
                {
                    return Err(LoadResolveError::WorkspaceTargetLooksLikeCrate {
                        target: workspace_ref,
                    });
                }

                return Err(match self.kind {
                    LoadKind::Crate => LoadResolveError::MissingSavedCrate {
                        target: workspace_ref,
                    },
                    LoadKind::Workspace => LoadResolveError::MissingSavedWorkspace {
                        target: workspace_ref,
                    },
                });
            }
            Ok(RegistryLookupState::Ambiguous) => {
                return Err(LoadResolveError::AmbiguousSavedTarget {
                    target: workspace_ref,
                });
            }
            Err(_) => return Err(LoadResolveError::RegistryUnavailable),
        }

        Ok(LoadResolution {
            workspace_ref,
            replaces_loaded_state: has_loaded_context,
        })
    }
}

enum RegistryLookupState {
    Found,
    Missing,
    Ambiguous,
}

fn registry_lookup_state(workspace_ref: &str) -> color_eyre::Result<RegistryLookupState> {
    let registry = WorkspaceRegistry::load_from_path(&WorkspaceRegistry::default_registry_path())?;
    if registry
        .entries
        .iter()
        .any(|entry| entry.workspace_id == workspace_ref)
    {
        return Ok(RegistryLookupState::Found);
    }

    let matches = registry
        .entries
        .iter()
        .filter(|entry| entry.workspace_name == workspace_ref)
        .count();

    Ok(match matches {
        0 => RegistryLookupState::Missing,
        1 => RegistryLookupState::Found,
        _ => RegistryLookupState::Ambiguous,
    })
}

fn workspace_root_from_pwd(pwd: &Path) -> Option<PathBuf> {
    try_parse_manifest(pwd, ManifestKind::WorkspaceRoot)
        .ok()
        .and_then(|manifest| manifest.workspace.map(|_| pwd.to_path_buf()))
}

fn workspace_member_match(target: &str, workspace_root: &Path) -> Option<PathBuf> {
    resolve_loaded_crate_target(Path::new(target), Some(workspace_root), &[], false)
}

fn resolve_loaded_crate_target(
    target: &Path,
    loaded_workspace_root: Option<&Path>,
    loaded_member_roots: &[PathBuf],
    standalone_loaded_workspace: bool,
) -> Option<PathBuf> {
    if target.is_absolute() {
        return loaded_member_roots
            .iter()
            .find(|root| root.as_path() == target)
            .cloned();
    }

    if let Some(workspace_root) = loaded_workspace_root {
        let candidate = workspace_root.join(target);
        if let Some(root) = loaded_member_roots
            .iter()
            .find(|root| root.as_path() == candidate.as_path())
        {
            return Some(root.clone());
        }
    }

    let Some(target_name) = target.file_name().and_then(|name| name.to_str()) else {
        return None;
    };

    if let Some(loaded_match) = loaded_member_roots
        .iter()
        .find(|root| root.file_name().and_then(|name| name.to_str()) == Some(target_name))
        .cloned()
    {
        return Some(loaded_match);
    }

    let workspace_root = loaded_workspace_root?;
    if standalone_loaded_workspace {
        return None;
    }

    let manifest = try_parse_manifest(workspace_root, ManifestKind::WorkspaceRoot).ok()?;
    let workspace = manifest.workspace?;

    let candidate = workspace_root.join(target);
    if let Some(member) = workspace
        .members
        .iter()
        .find(|member| member.as_path() == candidate.as_path())
    {
        return Some(member.clone());
    }

    let mut basename_matches = workspace
        .members
        .into_iter()
        .filter(|member| member.file_name().and_then(|name| name.to_str()) == Some(target_name));
    let first = basename_matches.next()?;
    if basename_matches.next().is_some() {
        return None;
    }
    Some(first)
}

impl IndexResolveError {
    /// Human-readable message that should be surfaced to the user.
    pub fn user_message(&self) -> String {
        self.to_string()
    }

    /// Recovery stays to a single suggestion; the UI formatter keeps the wording indirect.
    pub fn recovery_suggestion(&self) -> String {
        match self {
            IndexResolveError::CurrentDirectoryNotLoadedCrate => {
                "Open or load a crate first, then run `/index` again.".to_string()
            }
            IndexResolveError::CurrentDirectoryNotWorkspaceMember => {
                "Open or load a workspace member first, then run `/index workspace` again."
                    .to_string()
            }
            IndexResolveError::MissingWorkspaceContext => {
                "Open or load a workspace first, then run `/index` again.".to_string()
            }
            IndexResolveError::MissingCrateContext => {
                "Open or load a crate first, then run `/index` again.".to_string()
            }
            IndexResolveError::TargetResolution(err) => {
                format!("Check the target path and try again. ({err})")
            }
        }
    }
}

/// Workspace-related commands that require a loaded workspace/crate.
///
/// These commands operate on workspace state and all require that
/// a workspace or crate is currently loaded (has_loaded_crates() == true).
#[derive(Debug)]
pub enum WorkspaceCmd {
    /// Save the current workspace snapshot to the registry.
    SaveDb,
    /// Load a workspace snapshot from the registry.
    LoadDb { workspace_ref: String },
    /// Load a specific crate from a workspace snapshot.
    LoadWorkspaceCrates {
        workspace_ref: String,
        crate_ref: String,
    },
    /// Display the current workspace status.
    WorkspaceStatus,
    /// Update the workspace (scan for changes, etc.)
    WorkspaceUpdate,
    /// Remove a crate from the current workspace.
    WorkspaceRemove { crate_ref: String },
    /// Scan for file changes in the workspace.
    ScanForChange {
        scan_tx: oneshot::Sender<Option<Vec<PathBuf>>>,
    },
    /// Set the current working directory.
    SetPwd { new_pwd: PathBuf },
}

impl Validate for WorkspaceCmd {
    async fn validate(&self, state: &super::AppState) -> Result<(), ValidationError> {
        // Most workspace commands require loaded crates, but some don't:
        // - LoadDb: doesn't require loaded crates (it's loading one)
        // - SetPwd: doesn't require loaded crates (it's setting pwd)
        match self {
            WorkspaceCmd::LoadDb { .. } | WorkspaceCmd::SetPwd { .. } => Ok(()),
            _ => {
                let has_loaded = state.with_system_read(|sys| sys.has_loaded_crates()).await;

                if has_loaded {
                    Ok(())
                } else {
                    Err(ValidationError::NoWorkspaceLoaded)
                }
            }
        }
    }
}

impl WorkspaceCmd {
    /// Returns the discriminant name for logging/debugging.
    pub fn discriminant(&self) -> &'static str {
        match self {
            WorkspaceCmd::SaveDb => "WorkspaceCmd::SaveDb",
            WorkspaceCmd::LoadDb { .. } => "WorkspaceCmd::LoadDb",
            WorkspaceCmd::LoadWorkspaceCrates { .. } => "WorkspaceCmd::LoadWorkspaceCrates",
            WorkspaceCmd::WorkspaceStatus => "WorkspaceCmd::WorkspaceStatus",
            WorkspaceCmd::WorkspaceUpdate => "WorkspaceCmd::WorkspaceUpdate",
            WorkspaceCmd::WorkspaceRemove { .. } => "WorkspaceCmd::WorkspaceRemove",
            WorkspaceCmd::ScanForChange { .. } => "WorkspaceCmd::ScanForChange",
            WorkspaceCmd::SetPwd { .. } => "WorkspaceCmd::SetPwd",
        }
    }
}

/// Validates a workspace command against the current application state.
pub async fn validate_workspace_cmd(
    cmd: &WorkspaceCmd,
    state: &super::AppState,
) -> Result<(), ValidationError> {
    cmd.validate(state).await
}

/// Validates a state command when the command has a validation contract.
pub async fn validate_state_command(
    command: &StateCommand,
    state: &super::AppState,
) -> Option<Result<(), ValidationError>> {
    match command {
        StateCommand::Workspace(cmd) => Some(validate_workspace_cmd(cmd, state).await),
        _ => None,
    }
}

/// Emits the canonical validation error event for a failed command.
pub fn emit_validation_error(event_bus: &EventBus, error: ValidationError) {
    event_bus.send(AppEvent::Error(ErrorEvent {
        message: error.to_string(),
        severity: ErrorSeverity::Error,
    }));
}

// =============================================================================
// LEGACY FLAT STATECOMMAND ENUM
// =============================================================================
// This is the original flat enum. During migration, commands are being
// extracted into grouped enums (WorkspaceCmd, etc.) above.
//
// The long-term goal is to reduce this to:
// pub enum StateCommand {
//     Chat(ChatCmd),
//     Workspace(WorkspaceCmd),
//     Db(DbCmd),
//     Llm(LlmCmd),
//     Rag(RagCmd),
// }
//
// See "COMMAND GROUPING ARCHITECTURE" section above for details.
// =============================================================================

#[derive(Debug)]
pub enum StateCommand {
    // Chat/Message operations (no validation needed)
    AddMessage {
        kind: MessageKind,
        content: String,
        /// Currently unused, placeholder for adding multi-agent support and/or different branches
        /// in the conversation history tree.
        target: ChatHistoryTarget,
        parent_id: Uuid,
        child_id: Uuid,
    },
    AddMessageImmediate {
        msg: String,
        kind: MessageKind,
        new_msg_id: Uuid,
    },
    AddMessageAtTail {
        msg: String,
        kind: MessageKind,
        new_msg_id: Uuid,
    },
    AddMessageTool {
        msg: String,
        kind: MessageKind,
        new_msg_id: Uuid,
        tool_call_id: ArcStr,
        tool_payload: Option<crate::tools::ToolUiPayload>,
    },
    AddUserMessage {
        content: String,
        new_user_msg_id: Uuid,
        completion_tx: oneshot::Sender<()>,
    },
    UpdateMessage {
        id: Uuid,
        update: crate::chat_history::MessageUpdate,
    },
    DeleteMessage {
        id: Uuid,
    },
    /// Decrement the chat history "turns to live"
    DecrementChatTtl {
        included_message_ids: Vec<Uuid>,
    },
    DeleteNode {
        id: Uuid,
    },
    ClearHistory {
        target: ChatHistoryTarget,
    },
    NewSession,
    SwitchSession {
        session_id: Uuid,
    },
    SaveState,
    LoadState,
    GenerateLlmResponse {
        target: ChatHistoryTarget,
        params_override: Option<LLMParameters>,
    },
    CancelGeneration {
        message_id: Uuid,
    },
    PruneHistory {
        max_messages: u16,
    },
    NavigateList {
        direction: ListNavigation,
    },
    NavigateBranch {
        direction: crate::chat_history::NavigationDirection,
    },
    CreateAssistantMessage {
        parent_id: Uuid,
        new_assistant_msg_id: Uuid,
        responder: oneshot::Sender<Uuid>,
    },

    // Workspace operations - VALIDATED (see WorkspaceCmd above)
    /// `/index` command, resolved in state based on current pwd/loading context.
    Index(IndexCmd),

    /// `/load` command boundary forwarded from the executor.
    /// Temporary backend routing keeps the current behavior intact.
    Load(LoadCmd),

    /// Save workspace snapshot. Validated: requires loaded workspace.
    ///
    /// **Migration Note**: This is the legacy variant. New code should use
    /// `StateCommand::Workspace(WorkspaceCmd::SaveDb)` which provides
    /// validation through the `Validate` trait.
    SaveDb,
    /// Load workspace snapshot.
    ///
    /// **Migration Note**: Use `StateCommand::Workspace(WorkspaceCmd::LoadDb { workspace_ref })`
    /// for validated loading with proper error handling.
    LoadDb {
        workspace_ref: String,
    },
    /// Load specific crate from workspace.
    ///
    /// **Migration Note**: Use `StateCommand::Workspace(WorkspaceCmd::LoadWorkspaceCrates { ... })`
    /// for validated loading.
    LoadWorkspaceCrates {
        workspace_ref: String,
        crate_ref: String,
    },
    /// Show workspace status.
    ///
    /// **Migration Note**: Use `StateCommand::Workspace(WorkspaceCmd::WorkspaceStatus)`
    /// for validated status check.
    WorkspaceStatus,
    /// Update workspace.
    ///
    /// **Migration Note**: Use `StateCommand::Workspace(WorkspaceCmd::WorkspaceUpdate)`
    /// for validated update.
    WorkspaceUpdate,
    /// Remove crate from workspace.
    ///
    /// **Migration Note**: Use `StateCommand::Workspace(WorkspaceCmd::WorkspaceRemove { crate_ref })`
    /// for validated removal.
    WorkspaceRemove {
        crate_ref: String,
    },
    /// Scan for file changes.
    ///
    /// **Migration Note**: Use `StateCommand::Workspace(WorkspaceCmd::ScanForChange { scan_tx })`
    /// for validated scanning.
    ScanForChange {
        scan_tx: oneshot::Sender<Option<Vec<PathBuf>>>,
    },
    /// Set working directory.
    ///
    /// **Migration Note**: Use `StateCommand::Workspace(WorkspaceCmd::SetPwd { new_pwd })`
    /// for validated pwd setting.
    SetPwd {
        new_pwd: PathBuf,
    },

    // NEW: Grouped workspace commands (preferred)
    /// Workspace commands with validation.
    ///
    /// This is the preferred way to send workspace commands. The dispatcher
    /// will validate the command before executing it, emitting an error
    /// event if validation fails.
    ///
    /// Example:
    /// ```rust
    /// app.send_cmd(StateCommand::Workspace(WorkspaceCmd::SaveDb));
    /// ```
    Workspace(WorkspaceCmd),

    // Indexing operations
    IndexTargetDir {
        target_dir: Option<IndexTargetDir>,
        needs_parse: bool,
    },
    PauseIndexing,
    ResumeIndexing,
    CancelIndexing,
    UpdateDatabase,
    /// Record indexing completion in SystemState (version + invalidations).
    RecordIndexCompleted,
    EmbedMessage {
        new_msg_id: Uuid,
        completion_rx: oneshot::Receiver<()>,
        scan_rx: oneshot::Receiver<Option<Vec<PathBuf>>>,
    },

    // LLM operations
    SwitchModel {
        alias_or_id: String,
    },
    WriteQuery {
        query_name: String,
        query_content: String,
    },
    ReadQuery {
        query_name: String,
        file_name: String,
    },
    BatchPromptSearch {
        prompt_file: String,
        out_file: String,
        max_hits: Option<usize>,
        threshold: Option<f32>,
    },

    // RAG operations
    Bm25Rebuild,
    Bm25Search {
        query: String,
        top_k: usize,
    },
    HybridSearch {
        query: String,
        top_k: usize,
    },
    RagBm25Status,
    RagBm25Save {
        path: PathBuf,
    },
    RagBm25Load {
        path: PathBuf,
    },
    RagSparseSearch {
        req_id: Uuid,
        query: String,
        top_k: usize,
        strict: bool,
    },
    RagDenseSearch {
        req_id: Uuid,
        query: String,
        top_k: usize,
    },
    RagAssembleContext {
        req_id: Uuid,
        user_query: String,
        top_k: usize,
        budget: TokenBudget,
        strategy: RetrievalStrategy,
    },
    ProcessWithRag {
        user_query: String,
        strategy: RetrievalStrategy,
        budget: TokenBudget,
    },

    // Editing operations
    SetEditingPreviewMode {
        mode: crate::app_state::core::PreviewMode,
    },
    SetEditingMaxPreviewLines {
        lines: usize,
    },
    SetEditingAutoConfirm {
        enabled: bool,
    },
    ApproveEdits {
        request_id: Uuid,
    },
    DenyEdits {
        request_id: Uuid,
    },
    /// Approve all pending edit proposals (newest wins when overlaps exist).
    ApprovePendingEdits,
    /// Deny all pending edit proposals.
    DenyPendingEdits,
    ApproveCreations {
        request_id: Uuid,
    },
    DenyCreations {
        request_id: Uuid,
    },
    SelectModelProvider {
        model_id_string: String,
        provider_key: Option<ProviderKey>,
    },
    SelectEmbeddingModel {
        // TODO:ploke-llm 2025-12-15
        // Replace this with an EmbeddingModelId instead
        model_id: ModelId,
        provider: ArcStr,
    },
    /// Update the latest context token count for the current prompt.
    UpdateContextTokens {
        tokens: ContextTokens,
    },

    /// Test-only marker command for TDD-style tests.
    /// Used to mark unimplemented executor paths without panicking.
    /// The string describes what behavior is expected when implemented.
    #[cfg(test)]
    TestTodo {
        test_name: String,
        message: String,
    },
}

impl StateCommand {
    pub fn discriminant(&self) -> &'static str {
        use StateCommand::*;
        match self {
            AddMessage { .. } => "AddMessage",
            DeleteMessage { .. } => "DeleteMessage",
            DeleteNode { .. } => "DeleteNode",
            AddUserMessage { .. } => "AddUserMessage",
            AddMessageTool { .. } => "AddMessageTool",
            UpdateMessage { .. } => "UpdateMessage",
            ClearHistory { .. } => "ClearHistory",
            NewSession => "NewSession",
            SwitchSession { .. } => "SwitchSession",
            SaveState => "SaveState",
            LoadState => "LoadState",
            GenerateLlmResponse { .. } => "GenerateLlmResponse",
            CancelGeneration { .. } => "CancelGeneration",
            PruneHistory { .. } => "PruneHistory",
            NavigateList { .. } => "NavigateList",
            NavigateBranch { .. } => "NavigateBranch",
            CreateAssistantMessage { .. } => "CreateAssistantMessage",
            IndexTargetDir { .. } => "IndexTargetDir",
            PauseIndexing => "PauseIndexing",
            ResumeIndexing => "ResumeIndexing",
            CancelIndexing => "CancelIndexing",
            AddMessageImmediate { .. } => "AddMessageImmediate",
            AddMessageAtTail { .. } => "AddMessageAtTail",
            UpdateDatabase => "UpdateDatabase",
            RecordIndexCompleted => "RecordIndexCompleted",
            EmbedMessage { .. } => "EmbedMessage",
            SwitchModel { .. } => "SwitchModel",
            WriteQuery { .. } => "WriteQuery",
            ReadQuery { .. } => "ReadQuery",
            SaveDb => "SaveDb",
            LoadDb { .. } => "LoadDb",
            LoadWorkspaceCrates { .. } => "LoadWorkspaceCrates",
            WorkspaceStatus => "WorkspaceStatus",
            WorkspaceUpdate => "WorkspaceUpdate",
            WorkspaceRemove { .. } => "WorkspaceRemove",
            BatchPromptSearch { .. } => "BatchPromptSearch",
            Bm25Rebuild => "Bm25Rebuild",
            Bm25Search { .. } => "Bm25Search",
            HybridSearch { .. } => "HybridSearch",
            RagBm25Status => "RagBm25Status",
            RagBm25Save { .. } => "RagBm25Save",
            RagBm25Load { .. } => "RagBm25Load",
            RagSparseSearch { .. } => "RagSparseSearch",
            RagDenseSearch { .. } => "RagDenseSearch",
            RagAssembleContext { .. } => "RagAssembleContext",
            ScanForChange { .. } => "ScanForChange",
            ProcessWithRag { .. } => "ProcessWithRag",
            SetEditingPreviewMode { .. } => "SetEditingPreviewMode",
            SetEditingMaxPreviewLines { .. } => "SetEditingMaxPreviewLines",
            SetEditingAutoConfirm { .. } => "SetEditingAutoConfirm",
            ApproveEdits { .. } => "ApproveEdits",
            DenyEdits { .. } => "DenyEdits",
            ApprovePendingEdits => "ApprovePendingEdits",
            DenyPendingEdits => "DenyPendingEdits",
            ApproveCreations { .. } => "ApproveCreations",
            DenyCreations { .. } => "DenyCreations",
            SelectModelProvider { .. } => "SelectModelProvider",
            DecrementChatTtl { .. } => "DecrementChatTtl",
            SelectEmbeddingModel { .. } => "SelectEmbeddingModel",
            UpdateContextTokens { .. } => "UpdateContextTokens",
            SetPwd { .. } => "SetPwd",
            Index(cmd) => cmd.discriminant(),
            Load(cmd) => cmd.discriminant(),
            // NEW: Grouped commands
            Workspace(cmd) => cmd.discriminant(),
            #[cfg(test)]
            TestTodo { .. } => "TestTodo",
        }
    }
}
