use std::ops::ControlFlow;
use std::path::PathBuf;

use crate::ModelId;
use crate::app_state::database::IndexTargetDir;
use crate::chat_history::{ContextTokens, MessageKind};
use crate::llm::{ChatHistoryTarget, LLMParameters, ProviderKey};
use ploke_core::ArcStr;
use ploke_core::embeddings::EmbeddingProviderSlug;
use ploke_llm::ProviderName;
use ploke_rag::{RetrievalStrategy, TokenBudget};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::event_bus::ErrorEvent;
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
            // NEW: Grouped commands
            Workspace(cmd) => cmd.discriminant(),
            #[cfg(test)]
            TestTodo { .. } => "TestTodo",
        }
    }
}
