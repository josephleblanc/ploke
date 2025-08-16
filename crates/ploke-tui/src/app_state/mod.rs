mod database;
mod models;

use std::{
    collections::{BTreeMap, HashSet},
    env::current_dir,
    ops::ControlFlow,
    path::PathBuf,
    str::FromStr,
};

use crate::{
    chat_history::{Message, MessageKind},
    parser::{ParserOutput, resolve_target_dir, run_parse_no_transform},
    user_config::ProviderRegistry,
    utils::helper::find_file_by_prefix,
};
use database::batch_prompt_search;
use itertools::Itertools;
use ploke_core::NodeId;
use ploke_db::{
    Database, DbError, NodeType, create_index_warn, replace_index_warn, search_similar,
};
use ploke_embed::indexer::{EmbeddingProcessor, IndexStatus, IndexerCommand, IndexerTask};
use ploke_io::IoManagerHandle;
use ploke_rag::{TokenBudget, RetrievalStrategy};
use ploke_transform::{macro_traits::HasAnyNodeId, transform::transform_parsed_graph};
use serde::{Deserialize, Serialize};
use syn_parser::{
    GraphAccess, ModuleTree, TestIds,
    parser::{
        nodes::{AnyNodeId, AsAnyNodeId, GraphNode, ModuleNodeId, PrimaryNodeId},
        types::TypeNode,
    },
    resolve::RelationIndexer,
};
use tokio::{
    sync::{Mutex, RwLock, mpsc, oneshot},
    time,
};
use tracing::{Level, Value, debug_span, instrument};
use uuid::Uuid;

use crate::{
    chat_history::{MessageStatus, MessageUpdate},
    llm::{ChatHistoryTarget, LLMParameters},
    system::SystemEvent,
    utils::helper::truncate_string,
};

use super::*;

/// AppState holds all shared application data.
/// It is designed for concurrent reads and synchronized writes.
///
/// This struct serves as the central hub for all application state, following a
/// Command-Query Responsibility Segregation (CQRS) pattern. State is divided into
/// distinct areas based on access patterns:
///
/// - **ChatState**: High-frequency writes for message updates and chat history
/// - **ConfigState**: Read-heavy configuration that changes infrequently
/// - **SystemState**: Medium-write system status and workspace information
///
/// External system integrations are managed through:
/// - **Database**: CozoDB instance for persistent storage and queries
/// - **IndexerTask**: Background indexing of source code and embeddings
/// - **EmbeddingProcessor**: Vector embedding generation for semantic search
/// - **IoManagerHandle**: File system operations and snippet retrieval
///
/// Thread safety is achieved through:
/// - `RwLock` for read-heavy data (allows multiple concurrent readers)
/// - `Mutex` for write-heavy operations (ensures exclusive access)
/// - `Arc` for shared ownership across async tasks
/// - Message passing via channels for inter-task communication
#[derive(Debug)]
pub struct AppState {
    pub chat: ChatState,     // High-write frequency
    pub config: ConfigState, // Read-heavy
    pub system: SystemState, // Medium-write

    // crate-external processes
    pub indexing_state: RwLock<Option<IndexingStatus>>,
    pub indexer_task: Option<Arc<indexer::IndexerTask>>,
    pub indexing_control: Arc<Mutex<Option<mpsc::Sender<indexer::IndexerCommand>>>>,

    pub db: Arc<Database>,
    pub embedder: Arc<EmbeddingProcessor>,
    pub io_handle: IoManagerHandle,
    pub rag: Option<Arc<ploke_rag::RagService>>,
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

// TODO: Need to handle `Config`, either create struct or
// use `config` crate

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
    // ... other config fields
}

impl AppState {
    /// Creates a new `AppState` instance with default-initialized subsystems.
    ///
    /// This constructor establishes the foundational state container for the entire
    /// application. It initializes:
    ///
    /// - **ChatState**: Empty chat history ready for user interactions
    /// - **ConfigState**: Default LLM configuration and provider registry
    /// - **SystemState**: Empty system status with no focused crate
    /// - **Database**: Shared CozoDB instance for persistent storage
    /// - **EmbeddingProcessor**: Configured embedder for semantic search
    /// - **IoManagerHandle**: File system operations interface
    ///
    /// The indexing subsystem is initialized in a dormant state (`indexing_state: None`)
    /// and can be activated later via `StateCommand::IndexWorkspace`.
    ///
    /// # Arguments
    ///
    /// * `db` - Shared database handle for all persistence operations
    /// * `embedder` - Configured embedding processor for vector operations
    /// * `io_handle` - File system interface for snippet retrieval
    ///
    /// # Thread Safety
    ///
    /// All state components use `Arc` for shared ownership and appropriate
    /// synchronization primitives (`RwLock`, `Mutex`) for thread-safe access.
    pub fn new(
        db: Arc<Database>,
        embedder: Arc<EmbeddingProcessor>,
        io_handle: IoManagerHandle,
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
            rag: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct SystemStatus {
    crate_focus: Option<PathBuf>,
}
impl SystemStatus {
    pub fn new(crate_focus: Option<PathBuf>) -> Self {
        Self { crate_focus }
    }
}

#[derive(thiserror::Error, Clone, Debug)]
pub enum StateError {
    #[error("The app state does not have a currently set crate focus")]
    MissingCrateFocus { msg: &'static str },
}

impl From<StateError> for ploke_error::Error {
    fn from(value: StateError) -> Self {
        match value {
            StateError::MissingCrateFocus { msg } => ploke_error::Error::UiError(msg.to_string()),
        }
    }
}

/// Directions which can be taken when selecting an item in a list.
/// Note that `left` and `right` are not included, because rather than moving `left` or `right` in
/// the list, we are swapping to the right or left.
#[derive(Debug, Clone, Copy)]
pub enum ListNavigation {
    Up,
    Down,
    Top,
    Bottom,
}

/// Defines the complete set of possible state mutation operations for the application.
///
/// Each variant represents a unique, atomic command that can be sent to the central
/// `state_manager` actor. This enum is the sole entry point for modifying `AppState`,
/// embodying the Command-Query Responsibility Segregation (CQRS) pattern.
#[derive(Debug)]
pub enum StateCommand {
    // --- Message and Chat History Commands ---
    /// Adds a new message to a chat history. This is used for both user input
    /// and for creating the initial placeholder for an assistant's response.
    // TODO: Fold the `AddUserMessage` into `AddMessage`
    AddMessage {
        /// The kind of the message author (e.g., User or Assistant).
        kind: MessageKind,
        /// The content of the message. Can be empty for an initial assistant message.
        content: String,
        /// The specific chat history (e.g., main, scratchpad) to add the message to.
        target: ChatHistoryTarget,
        /// The parent in the conversation tree where this message will be added
        parent_id: Uuid,
        /// The ID of the new message to be added as a child of the parent_id message
        child_id: Uuid,
    },

    /// Adds a new message from the provided string and type.
    /// This has much less flexibility than the `AddMessage` above, but is more convenient to use
    /// for certain kinds of system messages.
    AddMessageImmediate {
        msg: String,
        kind: MessageKind,
        new_msg_id: Uuid,
    },

    /// Adds a new user message and sets it as the current message.
    // TODO: consider if this needs more fields, or if it can/should be folded into the
    // `AddMessage` above
    AddUserMessage {
        content: String,
        new_msg_id: Uuid,
        completion_tx: oneshot::Sender<()>,
    },

    /// Applies a set of partial updates to an existing message.
    /// This is the primary command for streaming LLM responses, updating status,
    /// and attaching metadata.
    UpdateMessage {
        /// The unique identifier of the message to update.
        id: Uuid,
        /// A struct containing optional fields for the update.
        update: MessageUpdate,
    },

    /// Removes a specific message.
    /// **Does not** delete following messages.
    DeleteMessage {
        /// The unique identifier of the message to delete.
        id: Uuid,
    },

    /// Clears all messages from a specific chat history.
    ClearHistory {
        /// The target chat history to clear.
        target: ChatHistoryTarget,
    },

    // --- Application and Session Commands ---
    /// Creates a new, empty chat session, making it the active one.
    NewSession,

    /// Switches the active view to a different chat session.
    SwitchSession {
        /// The unique identifier of the session to switch to.
        session_id: Uuid,
    },

    /// Saves the current state of the application to a file.
    /// This is a "fire-and-forget" command that triggers a background task.
    SaveState,

    /// Loads application state from a file, replacing the current state.
    LoadState,

    // --- LLM and Agent Commands ---
    /// Submits the current chat history to the LLM for a response.
    /// The `state_manager` will prepare the prompt and dispatch it to the `llm_manager`.
    GenerateLlmResponse {
        /// The specific chat history to use as the context for the prompt.
        target: ChatHistoryTarget,
        /// Overrides for the default LLM parameters for this specific generation.
        params_override: Option<LLMParameters>,
    },

    /// Cancels an in-progress LLM generation task.
    CancelGeneration {
        /// The ID of the assistant message whose generation should be cancelled.
        message_id: Uuid,
    },

    // TODO: Documentation, look at this again, might need more fields
    PruneHistory {
        max_messages: u16,
    },

    // --- Navigate the List ---
    /// Navigates the primary message list (up, down, top, bottom).
    NavigateList {
        direction: ListNavigation,
    },

    /// Navigates between sibling message branches (left/right).
    NavigateBranch {
        direction: chat_history::NavigationDirection,
    },
    CreateAssistantMessage {
        parent_id: Uuid,
        responder: oneshot::Sender<Uuid>,
    },

    /// Triggers a background task to index the entire workspace.
    IndexWorkspace {
        workspace: String,
        needs_parse: bool,
    },
    /// Pauses the currently running indexing task.
    PauseIndexing,
    /// Resumes a paused indexing task.
    ResumeIndexing,
    /// Cancels the currently running indexing task.
    CancelIndexing,
    /// Updates the database indexes for improved query performance.
    UpdateDatabase,
    /// Embeds the last user message and searches for similar code snippets.
    EmbedMessage {
        new_msg_id: Uuid,
        completion_rx: oneshot::Receiver<()>,
        scan_rx: oneshot::Receiver<Option<Vec<PathBuf>>>,
    },
    /// Switches the active LLM model to the specified alias or ID.
    SwitchModel {
        alias_or_id: String,
    },
    /// Loads a named query into the database for execution.
    WriteQuery {
        query_name: String,
        query_content: String,
    },
    /// Reads a saved query from a file and prepares it for execution.
    ReadQuery {
        query_name: String,
        file_name: String,
    },
    /// Batch-embeds prompts from a file and writes similarity results to another file.
    BatchPromptSearch {
        prompt_file: String,
        out_file: String,
        max_hits: Option<usize>,
        threshold: Option<f32>,
    },
    /// Saves the current database state to disk.
    SaveDb,
    /// Loads a database for the specified crate into memory.
    LoadDb {
        crate_name: String,
    },
    /// Scans the workspace for file changes and updates the database.
    ScanForChange {
        scan_tx: oneshot::Sender<Option<Vec<PathBuf>>>,
    },

    /// Triggers a rebuild of the BM25 sparse index.
    Bm25Rebuild,

    /// Runs a BM25-only search with the given query and returns top_k results.
    Bm25Search {
        query: String,
        top_k: usize,
    },

    /// Runs a hybrid search (dense + BM25) for the given query.
    HybridSearch {
        query: String,
        top_k: usize,
    },

    /// Queries the status of the BM25 index via the RAG service.
    RagBm25Status,

    /// Saves the BM25 index sidecar to the given path.
    RagBm25Save {
        path: PathBuf,
    },

    /// Loads the BM25 index sidecar from the given path.
    RagBm25Load {
        path: PathBuf,
    },

    /// Runs a BM25 search with an explicit strict flag and request correlation ID.
    RagSparseSearch {
        req_id: Uuid,
        query: String,
        top_k: usize,
        strict: bool,
    },

    /// Runs a dense-only search (HNSW) with request correlation ID.
    RagDenseSearch {
        req_id: Uuid,
        query: String,
        top_k: usize,
    },

    /// Assembles a retrieval-augmented context for LLM prompting.
    RagAssembleContext {
        req_id: Uuid,
        user_query: String,
        top_k: usize,
        budget: TokenBudget,
        strategy: RetrievalStrategy,
    },
}

impl StateCommand {
    pub fn discriminant(&self) -> &'static str {
        use StateCommand::*;
        match self {
            AddMessage { .. } => "AddMessage",
            DeleteMessage { .. } => "DeleteMessage",
            AddUserMessage { .. } => "AddUserMessage",
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
            UpdateDatabase => "UpdateDatabase",
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
            // ... other variants
        }
    }
}

/// Event fired when a message is successfully updated.
///
/// This is a "minimal" or "ID-based" event. It intentionally does not contain
/// the new message data. Subscribers are expected to use the `message_id` to
/// query the central `AppState` for the latest, guaranteed-to-be-fresh data.
// This enforces a single source of truth and prevents UI from ever rendering stale data.
#[derive(Debug, Clone, Copy)]
pub struct MessageUpdatedEvent(pub Uuid);

impl MessageUpdatedEvent {
    pub fn new(message_id: Uuid) -> Self {
        Self(message_id)
    }
}

impl From<MessageUpdatedEvent> for AppEvent {
    fn from(event: MessageUpdatedEvent) -> Self {
        AppEvent::MessageUpdated(event)
    }
}

// TODO: Disentangle the event_bus sender from this most likely.
// - Needs to be evaluated against new implementation of EventBus
pub async fn state_manager(
    state: Arc<AppState>,
    mut cmd_rx: mpsc::Receiver<StateCommand>,
    event_bus: Arc<EventBus>,
    context_tx: mpsc::Sender<RagEvent>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        // Update the span with the command discriminant
        let span = tracing::debug_span!(
            "processing",
            cmd = %cmd.discriminant(),
        );
        let _enter = span.enter();

        match cmd {
            StateCommand::UpdateMessage { id, update } => {
                tracing::Span::current().record("msg_id", format!("{}", id));
                tracing::debug!(
                    content = ?update.content.as_ref().map(|c| truncate_string(c, 20)),
                    "Updating message"
                );
                let mut chat_guard = state.chat.0.write().await;

                if let Some(message) = chat_guard.messages.get_mut(&id) {
                    match message.try_update(update) {
                        Ok(_) => {
                            // Notify UI of update
                            event_bus.send(MessageUpdatedEvent::new(id).into());
                        }
                        Err(e) => {
                            event_bus.send(UpdateFailedEvent::new(id, e).into());
                        }
                    }
                }
            }
            StateCommand::AddUserMessage {
                content,
                new_msg_id,
                completion_tx,
            } => {
                add_msg_immediate(&state, &event_bus, new_msg_id, content, MessageKind::User).await;
                completion_tx
                    .send(())
                    .expect("AddUserMessage should never fail to send tx");
            }
            StateCommand::AddMessage {
                parent_id,
                child_id,
                content,
                // TODO: Figure out if I should/need to do more with these
                kind,
                target,
            } => {
                let mut chat_guard = state.chat.0.write().await;
                // For assistant messages, lthe status will be Generating initially
                let status = if matches!(kind, MessageKind::Assistant) {
                    MessageStatus::Generating
                } else {
                    MessageStatus::Completed
                };

                if let Ok(new_message_id) =
                    chat_guard.add_child(parent_id, child_id, &content, status, kind)
                {
                    chat_guard.current = new_message_id;
                    event_bus.send(MessageUpdatedEvent::new(new_message_id).into())
                }
            }
            StateCommand::AddMessageImmediate {
                msg,
                kind,
                new_msg_id,
            } => {
                add_msg_immediate(&state, &event_bus, new_msg_id, msg, kind).await;
            }
            StateCommand::PruneHistory { max_messages } => {
                // TODO: This will provide a way to prune the alternate branches of the
                // conversation tree, once the conversation tree has been implemented.
                todo!("Handle PruneHistory")
            }

            StateCommand::NavigateList { direction } => {
                let mut chat_guard = state.chat.0.write().await;
                chat_guard.navigate_list(direction);
                event_bus.send(MessageUpdatedEvent(chat_guard.current).into())
            }

            StateCommand::CreateAssistantMessage {
                parent_id,
                responder,
            } => {
                let mut chat_guard = state.chat.0.write().await;
                let child_id = Uuid::new_v4();
                let status = MessageStatus::Generating;
                let kind = crate::chat_history::MessageKind::Assistant;

                if let Ok(new_id) =
                    chat_guard.add_child(parent_id, child_id, "Pending...", status, kind)
                {
                    // update the state of the current id to the newly generated pending message.
                    chat_guard.current = new_id;

                    // Send the ID back to the requester.
                    // Ignore error in case the requester timed out and dropped the receiver.
                    let _ = responder.send(new_id);

                    // Notify the UI to render the new placeholder message.
                    event_bus.send(MessageUpdatedEvent::new(new_id).into());
                }
                // TODO: Consider if this is proper error handling or not.
                // If add_child fails, the responder is dropped, signaling an error to the awaiter.
            }
            StateCommand::IndexWorkspace {
                workspace,
                needs_parse,
            } => {
                let (control_tx, control_rx) = tokio::sync::mpsc::channel(4);
                let target_dir = {
                    let mut write_guard = state.system.write().await;
                    let crate_focus = match std::env::current_dir() {
                        Ok(current_dir) => {
                            let mut pwd = current_dir;
                            pwd.push(&workspace);
                            pwd
                        }
                        Err(e) => {
                            tracing::error!("Error resolving current dir: {e}");
                            continue;
                        }
                    };
                    tracing::debug!("Setting crate_focus to {}", crate_focus.display());
                    write_guard.crate_focus = Some(crate_focus.clone());
                    crate_focus
                };

                // TODO: maybe run_parse should be returning the name of the crate it parsed, as
                // defined in the `Cargo.toml`? For now we are just going to use the directory name
                // as the name of the crate.
                if needs_parse {
                    match run_parse(Arc::clone(&state.db), Some(target_dir.clone())) {
                        Ok(_) => tracing::info!(
                            "Parse of target workspace {} successful",
                            &target_dir.display()
                        ),
                        Err(e) => {
                            tracing::info!(
                                "Failure parsing directory from IndexWorkspace event: {}",
                                e
                            );
                            return;
                        }
                    }
                }
                // let mut chat_guard = state.chat.0.write().await;
                add_msg_immediate(
                    &state,
                    &event_bus,
                    Uuid::new_v4(), // double check this is OK
                    "Indexing...".to_string(),
                    MessageKind::SysInfo,
                )
                .await;
                let event_bus_clone = event_bus.clone();
                // let progress_tx = Arc::clone(&event_bus.index_tx);
                let progress_tx = Arc::clone(&event_bus.index_tx);
                let progress_rx = event_bus.index_subscriber();

                let state_arc = state.indexer_task.as_ref().map(Arc::clone);
                if let Some(indexer_task) = state_arc {
                    if let Ok((callback_manager, db_callbacks, unreg_codes_arc, shutdown)) =
                        ploke_db::CallbackManager::new_bounded(Arc::clone(&indexer_task.db), 1000)
                    {
                        let counter = callback_manager.clone_counter();
                        let callback_handler = std::thread::spawn(move || callback_manager.run());
                        let res = tokio::spawn(async move {
                            let indexing_result = IndexerTask::index_workspace(
                                indexer_task,
                                workspace,
                                progress_tx,
                                progress_rx,
                                control_rx,
                                callback_handler,
                                db_callbacks,
                                counter,
                                shutdown,
                            )
                            .await;
                            tracing::info!("Indexer task returned");
                            match indexing_result {
                                Ok(_) => {
                                    tracing::info!("Sending Indexing Completed");
                                    event_bus_clone.send(AppEvent::IndexingCompleted)
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Sending Indexing Failed with error message: {}",
                                        e.to_string()
                                    );
                                    event_bus_clone.send(AppEvent::IndexingFailed)
                                }
                            }
                        })
                        .await;
                        match res {
                            Ok(_) => {
                                tracing::info!("Sending Indexing Completed");
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Sending Indexing Failed with error message: {}",
                                    e.to_string()
                                );
                            }
                        }
                        tracing::info!("Indexer task returned");
                    }
                }
            }
            StateCommand::PauseIndexing => {
                if let Some(ctrl) = &mut *state.indexing_control.lock().await {
                    ctrl.send(IndexerCommand::Pause).await.ok();
                }
            }

            StateCommand::ResumeIndexing => {
                if let Some(ctrl) = &mut *state.indexing_control.lock().await {
                    ctrl.send(IndexerCommand::Resume).await.ok();
                }
            }

            StateCommand::CancelIndexing => {
                if let Some(ctrl) = &mut *state.indexing_control.lock().await {
                    ctrl.send(IndexerCommand::Cancel).await.ok();
                }
            }
            StateCommand::SaveState => {
                let serialized_content = {
                    let guard = state.chat.0.read().await;
                    guard.format_for_persistence().as_bytes().to_vec()
                };
                event_bus.send(AppEvent::System(SystemEvent::SaveRequested(
                    serialized_content,
                )))
            }
            StateCommand::UpdateDatabase => {
                let start = time::Instant::now();
                let new_msg_id = Uuid::new_v4();
                add_msg_immediate(
                    &state,
                    &event_bus,
                    new_msg_id,
                    "Indexing HNSW...".to_string(),
                    MessageKind::SysInfo,
                )
                .await;
                // TODO: Decide if this needs to be replaced.
                for ty in NodeType::primary_nodes() {
                    match create_index_warn(&state.db, ty) {
                        Ok(_) => {
                            tracing::info!(
                                "Database index updated by create_index_warn for rel: {}",
                                ty.relation_str()
                            );
                        }
                        Err(e) => {
                            match replace_index_warn(&state.db, ty) {
                                Ok(_) => {
                                    tracing::info!(
                                        "Database index updated by replace_index_warn for rel: {}",
                                        ty.relation_str()
                                    );
                                }
                                Err(e) => tracing::warn!(
                                    "The attempt to replace the index at the database failed"
                                ),
                            }
                            tracing::warn!("The attempt to create the index at the database failed")
                        }
                    }
                }
                let after = time::Instant::now();
                let msg = format!("..finished in {}", after.duration_since(start).as_millis());
                let second_new_message_id = Uuid::new_v4();
                add_msg_immediate(
                    &state,
                    &event_bus,
                    second_new_message_id,
                    msg,
                    MessageKind::SysInfo,
                )
                .await;
            }
            StateCommand::EmbedMessage {
                new_msg_id,
                completion_rx,
                scan_rx,
            } => {
                if let ControlFlow::Break(_) = wait_on_oneshot(new_msg_id, completion_rx).await {
                    continue;
                }
                let chat_guard = state.chat.0.read().await;
                match chat_guard.last_user_msg() {
                    Ok(Some((last_usr_msg_id, last_user_msg))) => {
                        tracing::info!("Start embedding user message: {}", last_user_msg);
                        let temp_embed = state
                            .embedder
                            .generate_embeddings(vec![last_user_msg])
                            .await
                            .expect("Error while generating embedding of user message");
                        // drop guard after we are done with last_usr_message, which is consumed by
                        // generate_embeddings
                        drop(chat_guard);
                        let embeddings = temp_embed
                            .into_iter()
                            .next()
                            .expect("No results from user message embedding generation");
                        tracing::info!("Finish embedding user message");

                        tracing::info!("Waiting to finish processing updates to files, if any");
                        // Wait on the oneshot from `scan_for_change`, letting us know that the database
                        // has been updated with the embeddings from any recent changes, if there were any.
                        if let ControlFlow::Break(_) = wait_on_oneshot(new_msg_id, scan_rx).await {
                            continue;
                        }
                        tracing::info!("Finished waiting on parsing target crate");

                        if let Err(e) =
                            embedding_search_similar(&state, &context_tx, new_msg_id, embeddings)
                                .await
                        {
                            tracing::error!("error during embedding search: {}", e);
                        };
                    }
                    Ok(None) => {
                        tracing::warn!(
                            "Could not retreive last user message from the conversation history"
                        );
                    }
                    Err(e) => {
                        tracing::error!("Error accessing last user message: {:#}", e);
                    }
                }
            }

            StateCommand::SwitchModel { alias_or_id } => {
                models::switch_model(&state, &event_bus, alias_or_id).await;
            }

            StateCommand::WriteQuery {
                query_name,
                query_content,
            } => {
                database::write_query(&state, query_content).await;
            }
            StateCommand::ReadQuery {
                query_name,
                file_name,
            } => {
                let _ = event_bus
                    .realtime_tx
                    .send(AppEvent::System(SystemEvent::ReadQuery {
                        query_name: query_name.clone(),
                        file_name: file_name.clone(),
                    }))
                    .inspect_err(|e| tracing::warn!("Error forwarding event: {e:?}"));
                let _ = event_bus
                    .background_tx
                    .send(AppEvent::System(SystemEvent::ReadQuery {
                        query_name,
                        file_name,
                    }))
                    .inspect_err(|e| tracing::warn!("Error forwarding event: {e:?}"));
            }
            StateCommand::SaveDb => {
                use database::save_db;
                // TODO: Refactor `save_db` to return a message that is sent through the
                // `event_bus` here so we don't use too much indirection when sending messages
                // through the event system.
                if let ControlFlow::Break(_) = save_db(&state, &event_bus).await {
                    continue;
                }
            }
            StateCommand::BatchPromptSearch {
                prompt_file,
                out_file,
                max_hits,
                threshold,
            } => {
                match batch_prompt_search(&state, prompt_file, out_file, max_hits, threshold).await {
                    Ok(embed_data) => {
                        tracing::info!("Batch prompt search succeeded with {} results.", embed_data.len());
                    },
                    Err(e) =>{
                        // Log the full error with context using tracing's error macro
                        // color_eyre::Report provides rich context including backtrace
                        tracing::error!(
                            error = %e,
                            error_chain = ?e.chain().collect::<Vec<_>>(),
                            "Batch prompt search failed"
                        );

                        // TODO: Once I'm sure the rest of this works I'll add this, but for now I
                        // don't want to make too many changes, especially to the event and messaging
                        // loop, before testing.
                        // Also emit as a warning event to the UI
                        // event_bus.send(AppEvent::System(SystemEvent::Error(
                        //     format!("Batch prompt search failed: {}", e)
                        // )));
                    }
                }
            }
            StateCommand::LoadDb { crate_name } => {
                // TODO: Refactor this to be a function, and change the `continue` to handling the
                // result with `?`
                if let Err(e) = database::load_db(&state, &event_bus, crate_name).await {
                    match e {
                        ploke_error::Error::Fatal(_) => e.emit_fatal(),
                        ploke_error::Error::Warning(_) | ploke_error::Error::Internal(_) => {
                            e.emit_warning()
                        }
                        _ => {
                            todo!("These should never happen.")
                        }
                    }
                }
                // TODO: run hnsw indexer again here using cozo command.
            }

            StateCommand::ScanForChange { scan_tx } => {
                let _ = database::scan_for_change(&state, &event_bus, scan_tx)
                    .await
                    .inspect_err(|e| {
                        e.emit_error();
                        tracing::error!("Error in ScanForChange:\n{e}");
                    });
            }

            StateCommand::Bm25Rebuild => {
                if let Some(rag) = &state.rag {
                    match rag.bm25_rebuild().await {
                        Ok(()) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                "BM25 rebuild requested".to_string(),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                        Err(e) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("BM25 rebuild failed: {}", e),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                    }
                } else {
                    add_msg_immediate(
                        &state,
                        &event_bus,
                        Uuid::new_v4(),
                        "RAG service unavailable; cannot rebuild BM25".to_string(),
                        MessageKind::SysInfo,
                    )
                    .await;
                }
            }

            StateCommand::Bm25Search { query, top_k } => {
                if let Some(rag) = &state.rag {
                    match rag.search_bm25(&query, top_k).await {
                        Ok(results) => {
                            let lines: Vec<String> = results
                                .into_iter()
                                .map(|(id, score)| format!("{}: {:.3}", id, score))
                                .collect();
                            let content = if lines.is_empty() {
                                format!("BM25 results (top {}): <no hits>", top_k)
                            } else {
                                format!("BM25 results (top {}):\n{}", top_k, lines.join("\n"))
                            };
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                content,
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                        Err(e) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("BM25 search failed: {}", e),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                    }
                } else {
                    add_msg_immediate(
                        &state,
                        &event_bus,
                        Uuid::new_v4(),
                        "RAG service unavailable; cannot run BM25 search".to_string(),
                        MessageKind::SysInfo,
                    )
                    .await;
                }
            }

            StateCommand::HybridSearch { query, top_k } => {
                if let Some(rag) = &state.rag {
                    match rag.hybrid_search(&query, top_k).await {
                        Ok(results) => {
                            let lines: Vec<String> = results
                                .into_iter()
                                .map(|(id, score)| format!("{}: {:.3}", id, score))
                                .collect();
                            let content = if lines.is_empty() {
                                format!("Hybrid results (top {}): <no hits>", top_k)
                            } else {
                                format!("Hybrid results (top {}):\n{}", top_k, lines.join("\n"))
                            };
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                content,
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                        Err(e) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("Hybrid search failed: {}", e),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                    }
                } else {
                    add_msg_immediate(
                        &state,
                        &event_bus,
                        Uuid::new_v4(),
                        "RAG service unavailable; cannot run hybrid search".to_string(),
                        MessageKind::SysInfo,
                    )
                    .await;
                }
            }

            StateCommand::RagBm25Status => {
                if let Some(rag) = &state.rag {
                    match rag.bm25_status().await {
                        Ok(status) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("BM25 status: {:?}", status),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                        Err(e) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("BM25 status error: {}", e),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                    }
                } else {
                    add_msg_immediate(
                        &state,
                        &event_bus,
                        Uuid::new_v4(),
                        "RAG service unavailable; cannot query BM25 status".to_string(),
                        MessageKind::SysInfo,
                    )
                    .await;
                }
            }

            StateCommand::RagBm25Save { path } => {
                if let Some(rag) = &state.rag {
                    match rag.bm25_save(path.clone()).await {
                        Ok(()) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("BM25 index saved to {}", path.display()),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                        Err(e) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("BM25 save failed: {}", e),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                    }
                } else {
                    add_msg_immediate(
                        &state,
                        &event_bus,
                        Uuid::new_v4(),
                        "RAG service unavailable; cannot save BM25 index".to_string(),
                        MessageKind::SysInfo,
                    )
                    .await;
                }
            }

            StateCommand::RagBm25Load { path } => {
                if let Some(rag) = &state.rag {
                    match rag.bm25_load(path.clone()).await {
                        Ok(()) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("BM25 index load requested from {}", path.display()),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                        Err(e) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("BM25 load failed: {}", e),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                    }
                } else {
                    add_msg_immediate(
                        &state,
                        &event_bus,
                        Uuid::new_v4(),
                        "RAG service unavailable; cannot load BM25 index".to_string(),
                        MessageKind::SysInfo,
                    )
                    .await;
                }
            }

            StateCommand::RagSparseSearch { req_id, query, top_k, strict } => {
                if let Some(rag) = &state.rag {
                    let result = if strict {
                        rag.search_bm25_strict(&query, top_k).await
                    } else {
                        rag.search_bm25(&query, top_k).await
                    };
                    match result {
                        Ok(results) => {
                            let lines: Vec<String> = results
                                .into_iter()
                                .map(|(id, score)| format!("{}: {:.3}", id, score))
                                .collect();
                            let header = format!("BM25 {}results (req_id: {}, top {}):",
                                if strict { "strict " } else { "" }, req_id, top_k);
                            let content = if lines.is_empty() {
                                format!("{} <no hits>", header)
                            } else {
                                format!("{}\n{}", header, lines.join("\n"))
                            };
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                content,
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                        Err(e) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("BM25 search (req_id: {}) failed: {}", req_id, e),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                    }
                } else {
                    add_msg_immediate(
                        &state,
                        &event_bus,
                        Uuid::new_v4(),
                        format!("RAG service unavailable; cannot run BM25 search (req_id: {})", req_id),
                        MessageKind::SysInfo,
                    )
                    .await;
                }
            }

            StateCommand::RagDenseSearch { req_id, query, top_k } => {
                if let Some(rag) = &state.rag {
                    match rag.search(&query, top_k).await {
                        Ok(results) => {
                            let lines: Vec<String> = results
                                .into_iter()
                                .map(|(id, score)| format!("{}: {:.3}", id, score))
                                .collect();
                            let header = format!("Dense results (req_id: {}, top {}):", req_id, top_k);
                            let content = if lines.is_empty() {
                                format!("{} <no hits>", header)
                            } else {
                                format!("{}\n{}", header, lines.join("\n"))
                            };
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                content,
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                        Err(e) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("Dense search (req_id: {}) failed: {}", req_id, e),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                    }
                } else {
                    add_msg_immediate(
                        &state,
                        &event_bus,
                        Uuid::new_v4(),
                        format!("RAG service unavailable; cannot run dense search (req_id: {})", req_id),
                        MessageKind::SysInfo,
                    )
                    .await;
                }
            }

            StateCommand::RagAssembleContext { req_id, user_query, top_k, budget, strategy } => {
                if let Some(rag) = &state.rag {
                    match rag.get_context(&user_query, top_k, budget, strategy).await {
                        Ok(_ctx) => {
                            // Until dedicated AppEvent variants are added, post a summary line to the UI.
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("Assembled context successfully (req_id: {}, top_k: {})", req_id, top_k),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                        Err(e) => {
                            add_msg_immediate(
                                &state,
                                &event_bus,
                                Uuid::new_v4(),
                                format!("Assemble context (req_id: {}) failed: {}", req_id, e),
                                MessageKind::SysInfo,
                            )
                            .await;
                        }
                    }
                } else {
                    add_msg_immediate(
                        &state,
                        &event_bus,
                        Uuid::new_v4(),
                        format!("RAG service unavailable; cannot assemble context (req_id: {})", req_id),
                        MessageKind::SysInfo,
                    )
                    .await;
                }
            }

            // ... other commands
            // TODO: Fill out other fields
            _ => {}
        };
    }
}

/// Performs semantic search using the provided message embedding and sends results to the context manager.
///
/// This function takes a vector embedding of a user message and searches the database for similar
/// code snippets based on semantic similarity. The search results are processed into human-readable
/// snippets and sent to the context manager for inclusion in the conversation context.
///
/// # Arguments
///
/// * `state` - Shared application state containing database and I/O handles
/// * `context_tx` - Channel sender for communicating with the context manager
/// * `new_msg_id` - UUID of the message being processed (used for correlation)
/// * `embeddings` - Vector embedding of the user's message for similarity search
///
/// # Process
///
/// 1. Searches the database for similar function nodes using the provided embedding
/// 2. Retrieves actual code snippets for the matching nodes via I/O manager
/// 3. Sends snippets to context manager via `RagEvent::ContextSnippets`
/// 4. Retrieves conversation history and triggers context construction
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if any step fails
async fn embedding_search_similar(
    state: &Arc<AppState>,
    context_tx: &mpsc::Sender<RagEvent>,
    new_msg_id: Uuid,
    embeddings: Vec<f32>,
) -> color_eyre::Result<()> {
    let ty_embed_data =
        search_similar(&state.db, embeddings.clone(), 100, 200, NodeType::Function).emit_error()?;
    tracing::info!("search_similar Success! with result {:?}", ty_embed_data);

    // Directly send snippets to RAG
    let snippets = state
        .io_handle
        .get_snippets_batch(ty_embed_data.v)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| r.ok())
        .collect::<Vec<String>>();

    // Send snippets directly to context manager
    context_tx
        .send(RagEvent::ContextSnippets(new_msg_id, snippets))
        .await?;

    // Then trigger context construction with the correct parent ID
    let messages: Vec<Message> = state.chat.0.read().await.clone_current_path_conv();

    context_tx
        .send(RagEvent::UserMessages(new_msg_id, messages))
        .await?;
    context_tx
        .send(RagEvent::ConstructContext(new_msg_id))
        .await?;
    Ok(())
}

fn print_module_set(
    merged: &syn_parser::ParsedCodeGraph,
    tree: &ModuleTree,
    module_set: &HashSet<ModuleNodeId>,
) {
    let item_map_printable = module_set
        .iter()
        .filter_map(|id| {
            tree.modules()
                .get(id)
                .filter(|m| m.items().is_some())
                .map(|m| {
                    let module = format!(
                        "name: {} | is_file: {} | id: {}",
                        m.name,
                        m.id.as_any(),
                        m.is_file_based()
                    );
                    let items = m
                        .items()
                        .unwrap()
                        .iter()
                        .filter_map(|item_id| {
                            merged
                                .find_any_node(item_id.as_any())
                                .map(|n| format!("\tname: {} | id: {}", n.name(), n.any_id()))
                        })
                        .join("\n");
                    format!("{}\n{}", module, items)
                })
        })
        .join("\n");
    tracing::info!("--- items by module ---\n{}", item_map_printable);
}

fn printable_nodes<'a>(
    merged: &syn_parser::ParsedCodeGraph,
    union: impl Iterator<Item = &'a AnyNodeId>,
) -> String {
    let mut printable_union_items = String::new();
    for id in union.into_iter() {
        if let Some(node) = merged.find_any_node(*id) {
            let printable_node = format!("name: {} | id: {}\n", node.name(), id);
            printable_union_items.push_str(&printable_node);
        }
    }
    printable_union_items
}

async fn wait_on_oneshot<T>(
    new_msg_id: Uuid,
    completion_rx: oneshot::Receiver<T>,
) -> ControlFlow<()> {
    match completion_rx.await {
        Ok(_) => {
            tracing::trace!("UserMessage received new_msg_id: {}", new_msg_id)
        }
        Err(e) => {
            tracing::warn!(
                "SendUserMessage dropped before EmbedMessage process received it for new_msg_id: {}",
                new_msg_id
            );
            return ControlFlow::Break(());
        }
    }
    ControlFlow::Continue(())
}

#[instrument(skip(state))]
async fn add_msg_immediate(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    new_msg_id: Uuid,
    content: String,
    kind: MessageKind,
) {
    tracing::trace!("Starting add_msg_immediate");
    let mut chat_guard = state.chat.0.write().await;
    let parent_id = chat_guard.current;

    let message_wrapper = match kind {
        MessageKind::User => chat_guard.add_message_user(parent_id, new_msg_id, content.clone()),
        MessageKind::System => todo!(),
        MessageKind::Assistant => {
            chat_guard.add_message_llm(parent_id, new_msg_id, kind, content.clone())
        }
        MessageKind::Tool => todo!(),
        MessageKind::SysInfo => {
            chat_guard.add_message_system(parent_id, new_msg_id, kind, content.clone())
        }
    };
    drop(chat_guard);
    // Add the user's message to the history
    if let Ok(message_id) = message_wrapper {
        let mut chat_guard = state.chat.0.write().await;
        // Update the current message to the one we just added
        chat_guard.current = message_id;
        drop(chat_guard);

        // Notify the UI that the state has changed
        event_bus.send(MessageUpdatedEvent::new(message_id).into());

        if kind == MessageKind::User {
            // Trigger the LLM to generate a response to the user's message
            let llm_request = AppEvent::Llm(llm::Event::Request {
                request_id: Uuid::new_v4(),
                parent_id: message_id,
                new_msg_id,
                prompt: content,
                parameters: Default::default(), // Using mock/default param
            });
            tracing::info!(
                "sending llm_request wrapped in an AppEvent::Llm of kind {kind} with ids 
                new_msg_id (not sent): {new_msg_id},
                parent_id: {parent_id}
                message_id: {message_id},",
            );
            event_bus.send(llm_request);
        }
    } else {
        tracing::error!("Failed to add message of kind: {}", kind);
    }
}

#[derive(Serialize)]
struct BatchResult {
    prompt_idx: usize,
    prompt: String,
    snippets: Vec<String>,
}

#[cfg(test)]
mod tests {
    use std::ops::Index;

    use cozo::DataValue;
    use ploke_db::QueryResult;
    use ploke_embed::local::EmbeddingConfig;
    use syn_parser::parser::nodes::ToCozoUuid;

    use crate::tracing_setup::init_tracing;

    use super::*;
    use ploke_embed::{
        indexer::{EmbeddingProcessor, EmbeddingSource},
        local::LocalEmbedder,
    };
    use rand::Rng;
    use tokio::time::{Duration, sleep};
    pub trait MockTrait {
        fn mock() -> Self;
    }

    // Mock implementations for testing
    impl MockTrait for EmbeddingProcessor {
        fn mock() -> Self {
            // Simple mock that does nothing
            Self::new(EmbeddingSource::Local(
                LocalEmbedder::new(EmbeddingConfig::default())
                    .expect("LocalEmbedder failed to construct within test - should not happen"),
            ))
        }
    }

    impl MockTrait for IoManagerHandle {
        fn mock() -> Self {
            // Simple mock that does nothing
            IoManagerHandle::new()
        }
    }

    use color_eyre::Result;
    use error::{ErrorExt, ErrorSeverity, ResultExt};
    use futures::{FutureExt, StreamExt};
    use ploke_test_utils::{init_test_tracing, setup_db_full, setup_db_full_crate, workspace_root};
    use thiserror::Error;

    #[tokio::test]
    async fn test_race_condition_without_oneshot() {
        let db = Database::new_init().unwrap();
        let state = Arc::new(AppState::new(
            Arc::new(db),
            Arc::new(EmbeddingProcessor::mock()),
            IoManagerHandle::mock(),
        ));
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

        // Start state manager
        tokio::spawn(state_manager(
            state.clone(),
            cmd_rx,
            event_bus.clone(),
            mpsc::channel(32).0,
        ));

        let parent_id = Uuid::new_v4();
        let user_msg_id = Uuid::new_v4();
        let embed_msg_id = Uuid::new_v4();

        // Simulate sending both commands concurrently without synchronization
        let tx1 = cmd_tx.clone();
        let tx2 = cmd_tx.clone();

        tokio::join!(
            async {
                tx1.send(StateCommand::AddUserMessage {
                    content: "tell me a haiku".to_string(),
                    new_msg_id: user_msg_id,
                    completion_tx: oneshot::channel().0, // dummy
                })
                .await
                .unwrap();
            },
            async {
                tx2.send(StateCommand::EmbedMessage {
                    new_msg_id: embed_msg_id,
                    completion_rx: oneshot::channel().1, // dummy
                    scan_rx: oneshot::channel().1,       // dummy
                })
                .await
                .unwrap();
            }
        );

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check if the embed message read the user message or not
        let chat = state.chat.0.read().await;
        let last_user_msg = chat.last_user_msg();
        assert!(
            last_user_msg.is_ok_and(|m| m.is_some_and(|im| !im.1.is_empty())),
            "User message should be present"
        );
    }

    #[tokio::test]
    async fn test_fix_with_oneshot() {
        let db = Database::new_init().unwrap();
        let state = Arc::new(AppState::new(
            Arc::new(db),
            Arc::new(EmbeddingProcessor::mock()),
            IoManagerHandle::mock(),
        ));
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

        // Start state manager
        tokio::spawn(state_manager(
            state.clone(),
            cmd_rx,
            event_bus.clone(),
            mpsc::channel(32).0,
        ));

        let parent_id = Uuid::new_v4();
        let user_msg_id = Uuid::new_v4();
        let embed_msg_id = Uuid::new_v4();

        let (tx, rx) = oneshot::channel();

        cmd_tx
            .send(StateCommand::AddUserMessage {
                content: "tell me a haiku".to_string(),
                new_msg_id: user_msg_id,
                completion_tx: tx,
            })
            .await
            .unwrap();

        cmd_tx
            .send(StateCommand::EmbedMessage {
                new_msg_id: embed_msg_id,
                completion_rx: rx,
                // TODO: revisit this test
                scan_rx: oneshot::channel().1, // dummy
            })
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let chat = state.chat.0.read().await;
        let last_user_msg = chat.last_user_msg();
        assert!(
            last_user_msg.is_ok_and(|m| m.is_some_and(|im| !im.1.is_empty())),
            "User message should always be present"
        );
    }

    #[tokio::test]
    async fn test_concurrency_with_fuzzing() {
        let db = Database::new_init().unwrap();
        let state = Arc::new(AppState::new(
            Arc::new(db),
            Arc::new(EmbeddingProcessor::mock()),
            IoManagerHandle::mock(),
        ));
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

        // Start state manager
        tokio::spawn(state_manager(
            state.clone(),
            cmd_rx,
            event_bus.clone(),
            mpsc::channel(32).0,
        ));

        let mut rng = rand::rng();

        // Send 50 pairs of commands with random delays
        for i in 0..50 {
            let delay_ms = rng.random_range(5..=20);
            sleep(Duration::from_millis(delay_ms)).await;

            let user_msg_id = Uuid::new_v4();
            let embed_msg_id = Uuid::new_v4();
            let (tx, rx) = oneshot::channel();

            // Send both commands
            cmd_tx
                .send(StateCommand::AddUserMessage {
                    content: format!("message {}", i),
                    new_msg_id: user_msg_id,
                    completion_tx: tx,
                })
                .await
                .unwrap();

            cmd_tx
                .send(StateCommand::EmbedMessage {
                    new_msg_id: embed_msg_id,
                    completion_rx: rx,
                    // TODO: Revisit and update this test
                    scan_rx: oneshot::channel().1, // dummy
                })
                .await
                .unwrap();
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        // Verify all messages were processed
        let chat = state.chat.0.read().await;
        let messages = chat.messages.len();
        assert!(messages >= 50, "Should have processed at least 50 messages");
    }
}
