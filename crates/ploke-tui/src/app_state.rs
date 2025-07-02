use tokio::sync::{mpsc, oneshot, RwLock};
use uuid::Uuid;

// logging

use crate::{
    chat_history::{MessageStatus, MessageUpdate},
    llm::{ChatHistoryTarget, LLMParameters, MessageRole}, utils::helper::truncate_string,
};

use super::*;

// pub struct AppState {
//     pub chat_history: RwLock<ChatHistory>,
//     pub system_status: RwLock<SystemStatus>,
//     /// Stores user config. Low write (if ever), higher read.
//
//     // A channel to signal application shutdown.
//     pub shutdown: tokio::sync::broadcast::Sender<()>,
// }

/// AppState holds all shared application data.
/// It is designed for concurrent reads and synchronized writes.
// TODO: Define the `RagContext` struct
// pub rag_context: RwLock<RagContext>,
#[derive(Debug)]
pub struct AppState {
    pub chat: ChatState,     // High-write frequency
    pub config: ConfigState, // Read-heavy
    pub system: SystemState, // Medium-write
}

// TODO: Implement Deref for all three *State items below

#[derive(Debug)]
pub struct ChatState(pub RwLock<ChatHistory>);
// TODO: Need to handle `Config`, either create struct or
// use `config` crate
#[derive(Debug)]
pub struct ConfigState(RwLock<Config>);
#[derive(Debug)]
pub struct SystemState(RwLock<SystemStatus>);

#[derive(Debug, Default)]
pub struct Config {
    pub llm_params: LLMParameters,
    // ... other config fields
}

// State access API (read-only)
impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn with_history<R>(&self, f: impl FnOnce(&ChatHistory) -> R) -> R {
        // TODO: need to evaluate whether to keep or not, still has old pattern
        let guard = self.chat.0.read().await;
        f(&guard)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            chat: ChatState(RwLock::new(ChatHistory::new())),
            config: ConfigState(RwLock::new(Config::default())),
            system: SystemState(RwLock::new(SystemStatus::default())),
            // TODO: This needs to be handled elsewhere if not handled in AppState
            // shutdown: tokio::sync::broadcast::channel(1).0,
        }
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
        /// The role of the message author (e.g., User or Assistant).
        role: MessageRole,
        /// The content of the message. Can be empty for an initial assistant message.
        content: String,
        /// The specific chat history (e.g., main, scratchpad) to add the message to.
        target: ChatHistoryTarget,
        /// The parent in the conversation tree where this message will be added
        parent_id: Uuid,
        /// The ID of the new message to be added as a child of the parent_id message
        child_id: Uuid,
    },

    /// Adds a new user message and sets it as the current message.
    // TODO: consider if this needs more fields, or if it can/should be folded into the
    // `AddMessage` above
    AddUserMessage {
        content: String,
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

    /// Triggers a background task to index the entire workspace.
    IndexWorkspace,

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
        responder: oneshot::Sender<Uuid>
    }
}

impl StateCommand {
    pub fn discriminant(&self) -> &'static str {
        match self {
            StateCommand::AddMessage { .. } => "AddMessage",
            StateCommand::DeleteMessage { .. } => "DeleteMessage",
            StateCommand::AddUserMessage { .. } => "AddUserMessage",
            StateCommand::UpdateMessage { .. } => "UpdateMessage",
            StateCommand::ClearHistory { .. } => "ClearHistory",
            StateCommand::NewSession => "NewSession",
            StateCommand::SwitchSession { .. } => "SwitchSession",
            StateCommand::SaveState => "SaveState",
            StateCommand::LoadState => "LoadState",
            StateCommand::GenerateLlmResponse { .. } => "GenerateLlmResponse",
            StateCommand::CancelGeneration { .. } => "CancelGeneration",
            StateCommand::PruneHistory { .. } => "PruneHistory",
            StateCommand::NavigateList { .. } => "NavigateList",
            StateCommand::NavigateBranch { .. } => "NavigateBranch",
            StateCommand::CreateAssistantMessage { .. } => "CreateAssistantMessage",
            // TODO: fill out the following
            StateCommand::IndexWorkspace => "IndexWorkspace",
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

// State manager implementation
// #[tracing::instrument(
//     skip_all,
//     fields(
//         cmd = %cmd.discriminant(),
//         msg_id = tracing::field::Empty
//     )
// )]
pub async fn state_manager(
    state: Arc<AppState>,
    mut cmd_rx: mpsc::Receiver<StateCommand>,
    event_bus: Arc<EventBus>,
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
                tracing::Span::current()
                    .record("msg_id", format!("{}", id));
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
            StateCommand::AddUserMessage { content } => {
                let mut chat_guard = state.chat.0.write().await;
                let parent_id = chat_guard.current;
                let child_id = Uuid::new_v4();

                // Add the user's message to the history
                if let Ok(user_message_id) = chat_guard.add_message_user(parent_id, child_id, content.clone())
                {
                    tracing::Span::current()
                        .record("msg_id", format!("{}", user_message_id));
                    tracing::info!(
                        content = %truncate_string(&content, 20),
                        parent_id = %parent_id,
                        "Adding user message"
                    );

                    // Update the current message to the one we just added
                    chat_guard.current = user_message_id;

                    // Notify the UI that the state has changed
                    event_bus.send(MessageUpdatedEvent::new(user_message_id).into());

                    // Trigger the LLM to generate a response to the user's message
                    let llm_request = AppEvent::Llm(llm::Event::Request {
                        request_id: Uuid::new_v4(),
                        parent_id: user_message_id,
                        prompt: content,
                        parameters: Default::default(), // Using mock/default param
                    });
                    event_bus.send(llm_request);
                } else {
                    tracing::error!("Failed to add user message");
                }
            }
            StateCommand::AddMessage {
                parent_id,
                child_id,
                content,
                // TODO: Figure out if I should/need to do more with these
                role,
                target,
            } => {
                let mut chat_guard = state.chat.0.write().await;
                // For assistant messages, lthe status will be Generating initially
                let status = if matches!(role, MessageRole::Assistant) {
                    MessageStatus::Generating
                } else {
                    MessageStatus::Completed
                };

                if let Ok(new_message_id) =
                    chat_guard.add_child(parent_id, child_id, &content, status, role.into())
                {
                    chat_guard.current = new_message_id;
                    event_bus.send(MessageUpdatedEvent::new(new_message_id).into())
                }
                // chat_guard.add_message(parent_id, content);
            }
            StateCommand::PruneHistory { max_messages } => todo!("Handle PruneHistory"),

            StateCommand::NavigateList { direction } => {
                let mut chat_guard = state.chat.0.write().await;
                chat_guard.navigate_list(direction);
                event_bus.send(MessageUpdatedEvent(chat_guard.current).into())
            }

            StateCommand::CreateAssistantMessage { parent_id, responder } => {
                let mut chat_guard = state.chat.0.write().await;
                let child_id = Uuid::new_v4();
                let status = MessageStatus::Generating;
                let role = crate::chat_history::Role::Assistant;

                if let Ok(new_id) = chat_guard.add_child(parent_id, child_id, "Pending...", status, role) {
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

            StateCommand::IndexWorkspace => {
                // TODO: This is a mock implementation. We need to pass the correct handles
                // to the real IndexerTask.
                // Indexer Task will:
                // 1. calling the database to get the non-indexed nodes in the graph using
                //    `get_nodes_for_embedding`
                // 2. calling the `get_snippets_batch` function to retrieve the code snippets from
                //    the target location
                // 3. then either:
                //      a. processing the embeddings locally, likely using `candle` or an
                //      alternative
                //      b. sending the embeddings to a remote API that can process the code
                //      snippets into embeddings.
                // 4. calling the `index_embeddings` function to create the hnsw index for the
                //    embeddings.
                // 5. return here and likely sending some kind of event to alert the rest of the
                //    systems, either through events or by changing state, that the embeddings are
                //    finished.
                //
                // - Note that we will want to ensure there are some other features built in as
                // well, such as a progress bar in the TUI that shows the ongoing progress of the
                // embeddings and ways to fail gracefully if the program is terminated early, and
                // ways to save our progress in processing the embeddings if possible, perhaps
                // through some kind of streaming mechanism or something, I don't know very much
                // about how vector embeddings are handled remotely or locally, and don't know if
                // there are streaming options available for vector embeddings services
                // specifically or through the `candle` crate, which I've never used.
                tokio::spawn(async move {
                    tracing::info!("IndexerTask started");
                });
            }
            // ... other commands
            // TODO: Fill out other fields
            _ => {}
        };
    }
}
