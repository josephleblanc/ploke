use std::sync::Arc;

use chrono::{SecondsFormat, Utc};
use ploke_core::ArcStr;
use tokio::sync::oneshot;
use tracing::{debug, instrument, trace};
use uuid::Uuid;

use crate::EventBus;
use crate::app_state::commands;
use crate::chat_history::{
    ContextStatus, Message, MessageAnnotation, MessageKind, MessageStatus, MessageUpdate,
    UpdateFailedEvent,
};
use crate::llm::{ChatEvt, LlmEvent};
// use ploke_llm::manager::events::ChatEvt;
use crate::tracing_setup::{CHAT_TARGET, MESSAGE_UPDATE_TARGET};
use crate::utils::helper::truncate_string;

use crate::{AppEvent, AppState, MessageUpdatedEvent};

#[instrument(skip(state, event_bus, update), fields(msg_id = %id, new_status = ?update.status))]
pub async fn update_message(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    id: Uuid,
    update: MessageUpdate,
) {
    let ts = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let thread_name = std::thread::current()
        .name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unnamed".to_string());
    let thread_id = format!("{:?}", std::thread::current().id());
    let span_id = tracing::Span::current().id();
    debug!(
        target: MESSAGE_UPDATE_TARGET,
        %id,
        ts,
        thread_name,
        thread_id,
        span_id = ?span_id,
        requested_status = ?update.status,
        requested_content_len = update.content.as_ref().map(|c| c.len()),
        requested_append_len = update.append_content.as_ref().map(|c| c.len()),
        "message update requested"
    );
    let update_for_result = update.clone();
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
        let old_meta = message.metadata.clone();
        let mut new_meta = None;
        match message.try_update(update) {
            Ok(_) => {
                new_meta = message.metadata.clone();
                debug!(
                    target: CHAT_TARGET,
                    msg_id = %id,
                    kind = ?msg_kind,
                    old_status = ?old_status,
                    new_status = ?new_status,
                    "Message updated successfully; dispatching MessageUpdatedEvent"
                );
                debug!(
                    target: MESSAGE_UPDATE_TARGET,
                    %id,
                    ts,
                    thread_name,
                    thread_id,
                    span_id = ?span_id,
                    kind = ?msg_kind,
                    old_status = ?old_status,
                    new_status = ?new_status,
                    content_len = message.content.len(),
                    metadata = message.metadata.as_ref().map(|_| "present"),
                    "message update applied"
                );
                event_bus.send(MessageUpdatedEvent::new(id).into());
            }
            Err(e) => {
                tracing::error!(
                    target: CHAT_TARGET,
                    msg_id = %id,
                    kind = ?msg_kind,
                    old_status = ?old_status,
                    error = %e,
                    "Message update failed; dispatching UpdateFailedEvent"
                );
                debug!(
                    target: MESSAGE_UPDATE_TARGET,
                    %id,
                    ts,
                    thread_name,
                    thread_id,
                    span_id = ?span_id,
                    kind = ?msg_kind,
                    old_status = ?old_status,
                    requested_status = ?update_for_result.status,
                    error = %e,
                    "message update validation failed"
                );
                event_bus.send(UpdateFailedEvent::new(id, e).into());
            }
        }
        let _ = update_for_result;
        let _ = message;

        if let Some(meta) = new_meta {
            let old_usage = old_meta.as_ref().map(|m| m.usage);
            let old_cost = old_meta.as_ref().map(|m| m.cost).unwrap_or(0.0);
            let delta_prompt = meta
                .usage
                .prompt_tokens
                .saturating_sub(old_usage.as_ref().map_or(0, |u| u.prompt_tokens));
            let delta_completion = meta
                .usage
                .completion_tokens
                .saturating_sub(old_usage.as_ref().map_or(0, |u| u.completion_tokens));
            let delta_cost = meta.cost - old_cost;
            chat_guard.record_usage_delta(delta_prompt, delta_completion, delta_cost);
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

    if let Ok(new_message_id) =
        chat_guard.add_child(parent_id, child_id, &content, status, kind, None, None)
    {
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
    new_assistant_msg_id: Uuid,
) {
    let mut chat_guard = state.chat.0.write().await;
    let status = MessageStatus::Generating;
    let kind = crate::chat_history::MessageKind::Assistant;

    if let Ok(new_id) = chat_guard.add_child(
        parent_id,
        new_assistant_msg_id,
        "Pending...",
        status,
        kind,
        None,
        None,
    ) {
        chat_guard.current = new_id;
        let _ = responder.send(new_id);
        event_bus.send(MessageUpdatedEvent::new(new_id).into());
    }
}

pub async fn prune_history() {
    todo!("Handle PruneHistory")
}

pub async fn add_tool_msg_immediate(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    new_msg_id: Uuid,
    content: String,
    tool_call_id: ArcStr,
    tool_payload: Option<crate::tools::ToolUiPayload>,
) {
    trace!(target: CHAT_TARGET, "Starting add_tool_msg_immediate");
    let mut chat_guard = state.chat.0.write().await;
    let parent_id = chat_guard.current;
    let before_children = chat_guard
        .messages
        .get(&parent_id)
        .map(|m| m.children.len())
        .unwrap_or(0);

    let inserted = chat_guard.add_message_tool(
        parent_id,
        new_msg_id,
        MessageKind::Tool,
        content.clone(),
        Some(tool_call_id),
        tool_payload,
    );
    match inserted {
        Ok(_) => {
            chat_guard.current = new_msg_id;
            let after_children = chat_guard
                .messages
                .get(&parent_id)
                .map(|m| m.children.len())
                .unwrap_or(before_children);
            trace!(
                target: CHAT_TARGET,
                "Inserted tool message; parent={parent_id} children_before={before_children} children_after={after_children} new_current={new_msg_id}"
            );
        }
        Err(e) => {
            trace!(
                target: CHAT_TARGET,
                "Failed to insert tool message under parent={parent_id}: {e:?}"
            );
        }
    }
    drop(chat_guard);

    if inserted.is_ok() {
        event_bus.send(MessageUpdatedEvent::new(new_msg_id).into());
        trace!(
            target: CHAT_TARGET,
            "Emitted MessageUpdatedEvent for tool message id={new_msg_id}"
        );
    }
}
/// Add a message under the current parent and move focus to it.
///
/// SysInfo messages added here inherit the default context status (pinned).
#[instrument(skip(state), level = "trace")]
pub async fn add_msg_immediate(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    new_msg_id: Uuid,
    content: String,
    kind: MessageKind,
) {
    trace!("Starting add_msg_immediate");
    let mut chat_guard = state.chat.0.write().await;
    let parent_id = chat_guard.current;

    let message_wrapper = match kind {
        MessageKind::User => chat_guard.add_message_user(parent_id, new_msg_id, content.clone()),
        MessageKind::System => {
            chat_guard.add_message_system(parent_id, new_msg_id, kind, content.clone())
        }
        MessageKind::Assistant => {
            chat_guard.add_message_llm(parent_id, new_msg_id, kind, content.clone())
        }
        MessageKind::Tool => {
            panic!("Use add_tool_msg_immediate to add tool messages");
        }
        MessageKind::SysInfo => {
            chat_guard.add_message_sysinfo(parent_id, new_msg_id, kind, content.clone())
        }
    };
    drop(chat_guard);
    if let Ok(message_id) = message_wrapper {
        let mut chat_guard = state.chat.0.write().await;
        chat_guard.current = message_id;
        drop(chat_guard);

        event_bus.send(MessageUpdatedEvent::new(message_id).into());

        if kind == MessageKind::User {
            let llm_request = AppEvent::Llm(LlmEvent::ChatCompletion(ChatEvt::Request {
                parent_id: message_id,
                request_msg_id: Uuid::new_v4(),
            }));
            trace!(
                target: CHAT_TARGET,
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

/// Add a message as the new tail without changing the current selection.
#[instrument(skip(state), level = "trace")]
pub async fn add_msg_immediate_at_tail(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    new_msg_id: Uuid,
    content: String,
    kind: MessageKind,
) {
    trace!("Starting add_msg_immediate_at_tail");
    let mut chat_guard = state.chat.0.write().await;
    let parent_id = chat_guard.tail;

    let message_wrapper = match kind {
        MessageKind::User => chat_guard.add_message_user(parent_id, new_msg_id, content.clone()),
        MessageKind::System => {
            chat_guard.add_message_system(parent_id, new_msg_id, kind, content.clone())
        }
        MessageKind::Assistant => {
            chat_guard.add_message_llm(parent_id, new_msg_id, kind, content.clone())
        }
        MessageKind::Tool => {
            panic!("Use add_tool_msg_immediate to add tool messages");
        }
        MessageKind::SysInfo => {
            chat_guard.add_message_sysinfo(parent_id, new_msg_id, kind, content.clone())
        }
    };
    drop(chat_guard);

    if let Ok(message_id) = message_wrapper {
        event_bus.send(MessageUpdatedEvent::new(message_id).into());
    } else {
        tracing::error!("Failed to add message at tail of kind: {}", kind);
    }
}

/// Add a SysInfo message without pinning it into the LLM context window.
///
/// Use this for UI-only summaries that should appear in chat history but not
/// be sent back to the model.
pub async fn add_msg_immediate_sysinfo_unpinned(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    new_msg_id: Uuid,
    content: String,
) {
    trace!("Starting add_msg_immediate_sysinfo_unpinned");
    let mut chat_guard = state.chat.0.write().await;
    let parent_id = chat_guard.current;
    let message_wrapper = chat_guard.add_message_sysinfo_with_context(
        parent_id,
        new_msg_id,
        content.clone(),
        ContextStatus::Unpinned,
    );
    drop(chat_guard);

    if let Ok(message_id) = message_wrapper {
        let mut chat_guard = state.chat.0.write().await;
        chat_guard.current = message_id;
        drop(chat_guard);

        event_bus.send(MessageUpdatedEvent::new(message_id).into());
    } else {
        tracing::error!("Failed to add unpinned sysinfo message");
    }
}

/// Add a message under the current parent without changing focus.
///
/// Useful for SysInfo updates that should not advance the chat selection.
pub async fn add_msg_immediate_nofocus(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    new_msg_id: Uuid,
    content: String,
    kind: MessageKind,
) {
    trace!(target: CHAT_TARGET, "Starting add_msg_immediate_nofocus");
    let mut chat_guard = state.chat.0.write().await;
    let parent_id = chat_guard.current;

    let message_wrapper = match kind {
        MessageKind::User => chat_guard.add_message_user(parent_id, new_msg_id, content.clone()),
        MessageKind::System => {
            chat_guard.add_message_system(parent_id, new_msg_id, kind, content.clone())
        }
        MessageKind::Assistant => {
            chat_guard.add_message_llm(parent_id, new_msg_id, kind, content.clone())
        }
        MessageKind::Tool => panic!("Use add_tool_msg_immediate to add tool messages"),
        MessageKind::SysInfo => {
            chat_guard.add_message_sysinfo(parent_id, new_msg_id, kind, content.clone())
        }
    };
    drop(chat_guard);

    if let Ok(message_id) = message_wrapper {
        // Do NOT change current selection; emit event so UI can render the new message
        event_bus.send(MessageUpdatedEvent::new(message_id).into());
    } else {
        tracing::error!("Failed to add message (nofocus) of kind: {}", kind);
    }
}

/// Add a message while preserving the current tail and selected child.
///
/// This is intended for SysInfo or background events that should not alter
/// the user’s navigation position.
pub async fn add_msg_immediate_background(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    new_msg_id: Uuid,
    content: String,
    kind: MessageKind,
) {
    trace!(target: CHAT_TARGET, "Starting add_msg_immediate_background");
    let mut chat_guard = state.chat.0.write().await;
    let parent_id = chat_guard.current;
    let old_tail = chat_guard.tail;
    let old_selected_child = chat_guard
        .messages
        .get(&parent_id)
        .and_then(|msg| msg.selected_child);

    let message_wrapper = match kind {
        MessageKind::User => chat_guard.add_message_user(parent_id, new_msg_id, content.clone()),
        MessageKind::System => {
            chat_guard.add_message_system(parent_id, new_msg_id, kind, content.clone())
        }
        MessageKind::Assistant => {
            chat_guard.add_message_llm(parent_id, new_msg_id, kind, content.clone())
        }
        MessageKind::Tool => panic!("Use add_tool_msg_immediate to add tool messages"),
        MessageKind::SysInfo => {
            chat_guard.add_message_sysinfo(parent_id, new_msg_id, kind, content.clone())
        }
    };

    if message_wrapper.is_ok() {
        chat_guard.tail = old_tail;
        if let Some(parent) = chat_guard.messages.get_mut(&parent_id) {
            parent.selected_child = old_selected_child;
        }
        chat_guard.rebuild_path_cache();
    }
    drop(chat_guard);

    if let Ok(message_id) = message_wrapper {
        event_bus.send(MessageUpdatedEvent::new(message_id).into());
    } else {
        tracing::error!("Failed to add background message of kind: {}", kind);
    }
}

pub async fn add_message_annotation(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    message_id: Uuid,
    annotation: MessageAnnotation,
) {
    let mut chat_guard = state.chat.0.write().await;
    chat_guard.add_annotation(message_id, annotation);
    drop(chat_guard);

    event_bus.send(MessageUpdatedEvent::new(message_id).into());
}

pub async fn update_tool_message_by_call_id(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    call_id: &ArcStr,
    content: Option<String>,
    tool_payload: Option<crate::tools::ToolUiPayload>,
) {
    let mut chat_guard = state.chat.0.write().await;
    let mut updated_id = None;
    for (id, msg) in chat_guard.messages.iter_mut() {
        if msg.tool_call_id.as_ref() == Some(call_id) {
            if let Some(content) = content {
                msg.content = content;
            }
            if tool_payload.is_some() {
                msg.tool_payload = tool_payload;
            }
            updated_id = Some(*id);
            break;
        }
    }
    drop(chat_guard);

    if let Some(message_id) = updated_id {
        event_bus.send(MessageUpdatedEvent::new(message_id).into());
    }
}
