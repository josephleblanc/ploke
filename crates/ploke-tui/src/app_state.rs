use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use crate::{
    chat_history::MessageUpdate,
    llm::{ChatHistoryTarget, LLMParameters, MessageRole},
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
pub struct AppState {
    pub chat: ChatState,     // High-write frequency
    pub config: ConfigState, // Read-heavy
    pub system: SystemState, // Medium-write

                             // TODO: Define the `RagContext` struct
                             // pub rag_context: RwLock<RagContext>,
}

// TODO: Implement Deref for all three *State items below
pub struct ChatState(RwLock<ChatHistory>);
// TODO: Need to handle `Config`, either create struct or
// use `config` crate
pub struct ConfigState(RwLock<Config>); 
pub struct SystemState(RwLock<SystemStatus>);

// State access API (read-only)
impl AppState {
    pub fn new() -> Self {
        Self {
            chat: ChatState(RwLock::new(ChatHistory::new())),
            config: todo!(),
            system: SystemState(RwLock::new(SystemStatus::new())),
            // TODO: This needs to be handled elsewhere if not handled in AppState
            // shutdown: tokio::sync::broadcast::channel(1).0,
        }
    }

    pub async fn with_history<R>(&self, f: impl FnOnce(&ChatHistory) -> R) -> R {
        // TODO: need to evaluate whether to keep or not, still has old pattern
        let guard = self.chat_history.read().await;
        f(&guard)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// Placeholder
// TODO: Decide if this is appropriately replaced by `SystemState` or not
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
    AddMessage {
        /// The role of the message author (e.g., User or Assistant).
        role: MessageRole,
        /// The content of the message. Can be empty for an initial assistant message.
        content: String,
        /// The specific chat history (e.g., main, scratchpad) to add the message to.
        target: ChatHistoryTarget,
        /// The parent in the conversation tree where this message will be added
        parent_id: Uuid,
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

    /// Removes a specific message and all of its descendants from the history.
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
pub async fn state_manager(
    state: Arc<AppState>,
    mut cmd_rx: mpsc::Receiver<StateCommand>,
    event_bus: Arc<EventBus>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            StateCommand::UpdateMessage { id, update } => {
                let mut guard = state.chat_history.write().await;

                if let Some(message) = guard.messages.get_mut(&id) {
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

            StateCommand::AddMessage {
                parent_id,
                content,
                // TODO: Figure out if I should/need to do more with these
                role,
                target,
            } => {
                let mut guard = state.chat_history.write().await;
                guard.add_message(parent_id, content);
            }
            StateCommand::PruneHistory { max_messages } => todo!(),
            // ... other commands
            // TODO: Fill out other fields
            _ => {}
        };
    }
}
