use std::collections::BTreeMap;

use crate::{
    chat_history::{Message, MessageKind},
    parser::resolve_target_dir,
    user_config::ProviderRegistry,
};
use ploke_db::{Database, NodeType, create_index_warn, replace_index_warn, search_similar};
use ploke_embed::indexer::{EmbeddingProcessor, IndexStatus, IndexerCommand, IndexerTask};
use ploke_io::IoManagerHandle;
use syn_parser::parser::types::TypeNode;
use tokio::{
    sync::{Mutex, RwLock, mpsc, oneshot},
    time,
};
use tracing::{Level, debug_span, instrument};
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
// TODO: Define the `RagContext` struct
// pub rag_context: RwLock<RagContext>,
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
}

// TODO: Implement Deref for all three *State items below

#[derive(Debug, Default)]
pub struct ChatState(pub RwLock<ChatHistory>);

impl ChatState {
    pub fn new(history: ChatHistory) -> Self {
        ChatState(RwLock::new(history))
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

#[derive(Debug, Default)]
pub struct SystemState(RwLock<SystemStatus>);

impl SystemState {
    pub fn new(status: SystemStatus) -> Self {
        SystemState(RwLock::new(status))
    }
}

#[derive(Debug)]
pub struct IndexingState(Arc<Mutex<IndexingStatus>>);

impl IndexingState {
    pub fn new(status: IndexingStatus) -> Self {
        IndexingState(Arc::new(Mutex::new(status)))
    }
}

impl std::ops::Deref for ConfigState {
    type Target = RwLock<Config>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::Deref for SystemState {
    type Target = RwLock<SystemStatus>;
    fn deref(&self) -> &Self::Target {
        &self.0
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

// State access API (read-only)
impl AppState {
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
            // TODO: This needs to be handled elsewhere if not handled in AppState
            // shutdown: tokio::sync::broadcast::channel(1).0,
        }
    }

    pub async fn with_history<R>(&self, f: impl FnOnce(&ChatHistory) -> R) -> R {
        // TODO: need to evaluate whether to keep or not, still has old pattern
        let guard = self.chat.0.read().await;
        f(&guard)
    }
}

// Placeholder
#[derive(Debug)]
pub struct SystemStatus {/* ... */}
impl SystemStatus {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SystemStatus {
    fn default() -> Self {
        Self::new()
    }
}
pub enum StateError {/* ... */}

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
    },
    PauseIndexing,
    ResumeIndexing,
    CancelIndexing,
    UpdateDatabase,
    EmbedMessage {
        new_msg_id: Uuid,
        completion_rx: oneshot::Receiver<()>,
    },
    ForwardContext {
        new_msg_id: Uuid,
    },
    SwitchModel {
        alias_or_id: String,
    },
    LoadQuery {
        query_name: String,
        query_content: String,
    },
    ReadQuery {
        query_name: String,
        file_name: String,
    }
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
            // TODO: fill out the following
            IndexWorkspace { .. } => "IndexWorkspace",
            PauseIndexing => "PauseIndexing",
            ResumeIndexing => "ResumeIndexing",
            CancelIndexing => "CancelIndexing",
            AddMessageImmediate { .. } => "AddMessageImmediate",
            UpdateDatabase => "UpdateDatabase",
            EmbedMessage { .. } => "EmbedMessage",
            ForwardContext { .. } => "ForwardContext",
            SwitchModel { .. } => "SwitchModel",
            LoadQuery { .. } => "LoadQuery",
            ReadQuery { .. } => "ReadQuery",
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
            },

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
            StateCommand::IndexWorkspace { workspace } => {
                let (control_tx, control_rx) = tokio::sync::mpsc::channel(4);
                // Extract task from mutex (consumes guard)

                match run_parse(Arc::clone(&state.db), Some(workspace.clone().into())) {
                    Ok(_) => tracing::info!("Parse of target workspace {} successful", &workspace),
                    Err(e) => {
                        tracing::info!(
                            "Failure parsing directory from IndexWorkspace event: {}",
                            e
                        );
                        return;
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
            } => {
                match completion_rx.await {
                    Ok(_) => {
                        tracing::trace!("UserMessage received new_msg_id: {new_msg_id}")
                    }
                    Err(e) => {
                        tracing::warn!(
                            "SendUserMessage dropped before EmbedMessage process received it for new_msg_id: {new_msg_id}"
                        );
                        return;
                    }
                }
                let chat_guard = state.chat.0.read().await;
                match chat_guard.last_user_msg() {
                    Ok(Some((last_usr_msg_id, last_user_msg))) => {
                        tracing::info!("Start embedding user message: {}", last_user_msg);
                        let temp_embed = state
                            .embedder
                            .generate_embeddings(vec![last_user_msg])
                            .await
                            .expect("Error while generating embeddings");
                        // drop guard after we are done with last_usr_message, which is consumed by
                        // generate_embeddings
                        drop(chat_guard);
                        let embeddings = temp_embed.first().expect("No results from vector search");
                        tracing::info!("Finish embedding user message");
                        match search_similar(
                            &state.db,
                            embeddings.clone(),
                            100,
                            200,
                            NodeType::Function,
                        ) {
                            Ok(ty_emb_data) => {
                                tracing::info!(
                                    "search_similar Success! with result {:?}",
                                    ty_emb_data
                                );

                                // CHANGE: Instead of ReadSnippet, directly send snippets to RAG
                                let snippets = state
                                    .io_handle
                                    .get_snippets_batch(ty_emb_data.v)
                                    .await
                                    .unwrap_or_default()
                                    .into_iter()
                                    .filter_map(|r| r.ok())
                                    .collect::<Vec<String>>();

                                // Send snippets directly to context manager
                                let _ = context_tx
                                    .send(RagEvent::ContextSnippets(new_msg_id, snippets))
                                    .await;

                                // Then trigger context construction with the correct parent ID
                                let messages: Vec<Message> =
                                    state.chat.0.read().await.clone_current_path_conv();
                                // .messages
                                // .iter()
                                // .map(|m| m.1.clone())
                                // .collect();

                                let _ = context_tx
                                    .send(RagEvent::UserMessages(new_msg_id, messages))
                                    .await;
                                let _ = context_tx
                                    .send(RagEvent::ConstructContext(new_msg_id))
                                    .await;
                            }
                            Err(e) => {
                                tracing::error!(
                                    "The at tempt to create the index at the database failed
                                    error message: {:?}",
                                    e
                                );
                            }
                        };
                    }
                    Ok(None) => {
                        tracing::warn!(
                            "Could not retreive last user message from the conversation history"
                        );
                    }
                    Err(e) => {
                        tracing::error!("{:#}", e);
                    }
                }
            }

            StateCommand::SwitchModel { alias_or_id } => {
                tracing::debug!("inside StateCommand::SwitchModel {}", alias_or_id);

                let mut cfg = state.config.write().await;
                if cfg.provider_registry.set_active(&alias_or_id) {
                    tracing::debug!(
                        "sending AppEvent::System(SystemEvent::ModelSwitched {}
                        Trying to find cfg.provider_registry.get_active_provider(): {:#?}",
                        alias_or_id,
                        cfg.provider_registry.get_active_provider(),
                    );
                    let actual_model = cfg
                        .provider_registry
                        .get_active_provider()
                        .map(|p| p.model.clone())
                        .unwrap_or_else(|| alias_or_id.clone());
                    event_bus.send(AppEvent::System(SystemEvent::ModelSwitched(
                        actual_model, // Using actual model ID
                    )));
                } else {
                    tracing::debug!("Sending AppEvent::Error(ErrorEvent {}", alias_or_id);
                    event_bus.send(AppEvent::Error(ErrorEvent {
                        message: format!("Unknown model '{}'", alias_or_id),
                        severity: ErrorSeverity::Warning,
                    }));
                }
            }

            StateCommand::LoadQuery { query_name, query_content } => {
                let result = state.db.run_script_read_only(&query_content, BTreeMap::new());
                tracing::info!(target: "load_query", "testing query result\n{:#?}", result);
                // let db_return = result.unwrap();
                // tracing::info!(target: "load_query", "db_return:\n{:#?}", db_return);
            }
            StateCommand::ReadQuery { query_name, file_name } => {
                let _ = event_bus.realtime_tx.send(AppEvent::System(SystemEvent::ReadQuery {
                    query_name: query_name.clone(), 
                    file_name: file_name.clone()
                })).inspect_err(|e| tracing::warn!("Error forwarding event: {e:?}"));
                let _ = event_bus.background_tx.send(AppEvent::System(SystemEvent::ReadQuery {
                    query_name, 
                    file_name
                })).inspect_err(|e| tracing::warn!("Error forwarding event: {e:?}"));
            }

            // ... other commands
            // TODO: Fill out other fields
            _ => {}
        };
    }
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

#[cfg(test)]
mod tests {
    use ploke_embed::local::EmbeddingConfig;

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
                LocalEmbedder::new(EmbeddingConfig::default()).expect(
                    "LocalEmbedder failed to construct within test - should not happen",
                ),
            ))
        }
    }

    impl MockTrait for IoManagerHandle {
        fn mock() -> Self {
            // Simple mock that does nothing
            IoManagerHandle::new()
        }
    }

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
                })
                .await
                .unwrap();
            }
        );

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check if the embed message read the user message or not
        let chat = state.chat.0.read().await;
        let last_user_msg = chat.last_user_msg();
        assert!(last_user_msg.is_ok_and(|m| m.is_some_and(|im| !im.1.is_empty())), "User message should be present");
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
