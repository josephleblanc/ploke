use std::sync::Arc;

use tokio::sync::oneshot;
use tracing::instrument;
use uuid::Uuid;

use crate::app_state::commands;
use crate::chat_history::{Message, MessageKind, MessageStatus, MessageUpdate, UpdateFailedEvent};
use crate::utils::helper::truncate_string;
use crate::{EventBus, llm};

use crate::{AppEvent, AppState, MessageUpdatedEvent};

// How can we add tracking during our test so we can ensure we see the expected outcomes? For now
// we just want to be able to see what the LLM is doing when we send the tool call request, but we
// would also like to find a way of actually testing that the tool runs as expected. Let's start by
#[instrument(skip(state, event_bus, update), fields(msg_id = %id, new_status = ?update.status))]
pub async fn update_message(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    id: Uuid,
    update: MessageUpdate,
) {
    tracing::info!(
        "Updating message {} status={:?} content_preview={}",
        id,
        update.status,
        update
            .content
            .as_ref()
            .map(|c| truncate_string(c, 100))
            .unwrap_or_default()
    );
    let mut chat_guard = state.chat.0.write().await;

    if let Some(message) = chat_guard.messages.get_mut(&id) {
        let old_status = message.status.clone();
        let msg_kind = message.kind;
        let new_status = update.status.clone().unwrap_or(old_status.clone());
        match message.try_update(update) {
            Ok(_) => {
                tracing::info!(
                    msg_id = %id,
                    kind = ?msg_kind,
                    old_status = ?old_status,
                    new_status = ?new_status,
                    "Message updated successfully; dispatching MessageUpdatedEvent"
                );
                event_bus.send(MessageUpdatedEvent::new(id).into());
            }
            Err(e) => {
                tracing::error!(
                    msg_id = %id,
                    kind = ?msg_kind,
                    old_status = ?old_status,
                    error = %e,
                    "Message update failed; dispatching UpdateFailedEvent"
                );
                event_bus.send(UpdateFailedEvent::new(id, e).into());
            }
        }
    }
}

pub async fn delete_message(state: &Arc<AppState>, event_bus: &Arc<EventBus>, id: Uuid) {
    // Perform deletion and compute new current selection, if any
    let new_current = {
        let mut chat_guard = state.chat.0.write().await;
        chat_guard.delete_message(id)
    };

    if let Some(curr) = new_current {
        {
            let mut chat_guard = state.chat.0.write().await;
            chat_guard.current = curr;
        }
        // Notify UI to refresh based on the new current selection
        event_bus.send(MessageUpdatedEvent::new(curr).into());
    }
}

pub async fn add_user_message(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    new_msg_id: Uuid,
    content: String,
    completion_tx: oneshot::Sender<()>,
) {
    add_msg_immediate(state, event_bus, new_msg_id, content, MessageKind::User).await;
    let _ = completion_tx.send(());
}

pub async fn add_message(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    parent_id: Uuid,
    child_id: Uuid,
    content: String,
    kind: MessageKind,
) {
    let mut chat_guard = state.chat.0.write().await;
    let status = if matches!(kind, MessageKind::Assistant) {
        MessageStatus::Generating
    } else {
        MessageStatus::Completed
    };

    if let Ok(new_message_id) = chat_guard.add_child(parent_id, child_id, &content, status, kind) {
        chat_guard.current = new_message_id;
        event_bus.send(MessageUpdatedEvent::new(new_message_id).into())
    }
}

pub async fn navigate_list(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    direction: commands::ListNavigation,
) {
    let mut chat_guard = state.chat.0.write().await;
    chat_guard.navigate_list(direction);
    event_bus.send(MessageUpdatedEvent(chat_guard.current).into())
}

pub async fn create_assistant_message(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    parent_id: Uuid,
    responder: oneshot::Sender<Uuid>,
) {
    let mut chat_guard = state.chat.0.write().await;
    let child_id = Uuid::new_v4();
    let status = MessageStatus::Generating;
    let kind = crate::chat_history::MessageKind::Assistant;

    if let Ok(new_id) = chat_guard.add_child(parent_id, child_id, "Pending...", status, kind) {
        chat_guard.current = new_id;
        let _ = responder.send(new_id);
        event_bus.send(MessageUpdatedEvent::new(new_id).into());
    }
}

pub async fn prune_history() {
    todo!("Handle PruneHistory")
}

#[instrument(skip(state))]
pub async fn add_msg_immediate(
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
    if let Ok(message_id) = message_wrapper {
        let mut chat_guard = state.chat.0.write().await;
        chat_guard.current = message_id;
        drop(chat_guard);

        event_bus.send(MessageUpdatedEvent::new(message_id).into());

        if kind == MessageKind::User {
            let llm_request = AppEvent::Llm(llm::Event::Request {
                request_id: Uuid::new_v4(),
                parent_id: message_id,
                new_msg_id,
                prompt: content,
                parameters: Default::default(),
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
