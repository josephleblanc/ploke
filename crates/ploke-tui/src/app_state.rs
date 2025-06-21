use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use crate::chat_history::MessageUpdate;

use super::*;

/// AppState holds all shared application data.
/// It is designed for concurrent reads and synchronized writes.
pub struct AppState {
    pub chat_history: RwLock<ChatHistory>,
    pub system_status: RwLock<SystemStatus>,
    // A channel to signal application shutdown.
    pub shutdown: tokio::sync::broadcast::Sender<()>,
}

// State access API (read-only)
impl AppState {
    pub async fn with_history<R>(&self, f: impl FnOnce(&ChatHistory) -> R) -> R {
        let guard = self.chat_history.read().await;
        f(&guard)
    }
}

// Placeholder
pub struct SystemStatus {/* ... */}
impl SystemStatus {
    pub fn new() -> Self {
        Self {}
    }
}
pub enum StateError {/* ... */}

// State mutation API (only via commands)
pub enum StateCommand {
    AddMessage { parent_id: Uuid, content: String },
    UpdateMessage { id: Uuid, update: MessageUpdate },
    PruneHistory { max_messages: usize },
    // ...
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
        Self (message_id)
    }
}

impl From<MessageUpdatedEvent> for AppEvent {
    fn from(event: MessageUpdatedEvent) -> Self {
        AppEvent::MessageUpdated(event)
    }
}

// State manager implementation
async fn state_manager(
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
                            event_bus.send(UpdateFailedEvent::new(id, e));
                        }
                    }
                }
            }

            StateCommand::AddMessage { parent_id, content } => {
                let mut guard = state.chat_history.write().await;
                guard.add_message(parent_id, content);
            }
            StateCommand::PruneHistory { max_messages } => todo!(), // ... other commands
                                                                    // ... other commands
        };
    }
}
