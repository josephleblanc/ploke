use std::ops::ControlFlow;
use std::path::PathBuf;

use crate::ModelId;
use crate::chat_history::{ContextTokens, MessageKind};
use crate::llm::{ChatHistoryTarget, LLMParameters, ProviderKey};
use ploke_core::ArcStr;
use ploke_core::embeddings::EmbeddingProviderSlug;
use ploke_llm::ProviderName;
use ploke_rag::{RetrievalStrategy, TokenBudget};
use tokio::sync::oneshot;
use uuid::Uuid;

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

#[derive(Debug)]
pub enum StateCommand {
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
    IndexWorkspace {
        workspace: String,
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
    SaveDb,
    LoadDb {
        crate_name: String,
    },
    ScanForChange {
        scan_tx: oneshot::Sender<Option<Vec<PathBuf>>>,
    },
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
            IndexWorkspace { .. } => "IndexWorkspace",
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
        }
    }
}
