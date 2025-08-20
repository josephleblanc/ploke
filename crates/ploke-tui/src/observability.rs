use std::sync::Arc;

use chrono::Utc;
use serde_json::Value;
use tokio::select;
use uuid::Uuid;

use crate::app_state::MessageUpdatedEvent;
use crate::chat_history::{Message, MessageKind};
use crate::{AppEvent, EventBus, EventPriority};
use crate::{llm::ToolEvent, system::SystemEvent};

use crate::app_state::AppState;

// ploke-db observability contract
use ploke_db::observability::{
    ConversationTurn, ObservabilityStore, ToolCallDone, ToolCallReq, ToolStatus, Validity,
};

pub async fn run_observability(event_bus: Arc<EventBus>, state: Arc<AppState>) {
    let mut rt_rx = event_bus.subscribe(EventPriority::Realtime);
    let mut bg_rx = event_bus.subscribe(EventPriority::Background);

    loop {
        select! {
            evt = rt_rx.recv() => {
                match evt {
                    Ok(ev) => handle_event(&state, ev).await,
                    Err(e) => {
                        tracing::trace!("observability realtime channel closed/lagged: {}", e);
                        // do not break; continue receiving from background
                    }
                }
            }
            evt = bg_rx.recv() => {
                match evt {
                    Ok(ev) => handle_event(&state, ev).await,
                    Err(e) => {
                        tracing::trace!("observability background channel closed/lagged: {}", e);
                        // continue loop
                    }
                }
            }
        }
    }
}

async fn handle_event(state: &Arc<AppState>, ev: AppEvent) {
    match ev {
        // Persist conversation turns when a message is (created/)updated
        AppEvent::MessageUpdated(MessageUpdatedEvent(id)) => {
            if let Some(msg) = get_message(state, id).await {
                if let Err(e) = persist_conversation_turn(state, &msg).await {
                    tracing::warn!("observability: upsert_conversation_turn failed: {}", e);
                }
            }
        }

        // Typed tool events (preferred path)
        AppEvent::LlmTool(ToolEvent::Requested {
            request_id,
            parent_id,
            name,
            arguments,
            call_id,
            vendor,
        }) => {
            if let Err(e) =
                persist_tool_requested(state, request_id, parent_id, &vendor, &name, &arguments, &call_id).await
            {
                tracing::warn!("observability: record_tool_call_requested failed: {}", e);
            }
        }
        AppEvent::LlmTool(ToolEvent::Completed {
            request_id,
            parent_id,
            call_id,
            content,
        }) => {
            if let Err(e) = persist_tool_done(
                state,
                request_id,
                parent_id,
                &call_id,
                Some(json_string(&content)),
                None,
                None,
                ToolStatus::Completed,
            )
            .await
            {
                tracing::warn!("observability: record_tool_call_done (completed) failed: {}", e);
            }
        }
        AppEvent::LlmTool(ToolEvent::Failed {
            request_id,
            parent_id,
            call_id,
            error,
        }) => {
            if let Err(e) = persist_tool_done(
                state,
                request_id,
                parent_id,
                &call_id,
                None,
                None,
                Some(error),
                ToolStatus::Failed,
            )
            .await
            {
                tracing::warn!("observability: record_tool_call_done (failed) failed: {}", e);
            }
        }

        // Compatibility path during M0: System tool events
        AppEvent::System(SystemEvent::ToolCallRequested {
            request_id,
            parent_id,
            vendor,
            name,
            arguments,
            call_id,
        }) => {
            if let Err(e) =
                persist_tool_requested(state, request_id, parent_id, &vendor, &name, &arguments, &call_id).await
            {
                tracing::warn!(
                    "observability: record_tool_call_requested (compat SystemEvent) failed: {}",
                    e
                );
            }
        }
        AppEvent::System(SystemEvent::ToolCallCompleted {
            request_id,
            parent_id,
            call_id,
            content,
        }) => {
            if let Err(e) = persist_tool_done(
                state,
                request_id,
                parent_id,
                &call_id,
                Some(json_string(&content)),
                None,
                None,
                ToolStatus::Completed,
            )
            .await
            {
                tracing::warn!(
                    "observability: record_tool_call_done (compat SystemEvent::Completed) failed: {}",
                    e
                );
            }
        }
        AppEvent::System(SystemEvent::ToolCallFailed {
            request_id,
            parent_id,
            call_id,
            error,
        }) => {
            if let Err(e) = persist_tool_done(
                state,
                request_id,
                parent_id,
                &call_id,
                None,
                None,
                Some(error),
                ToolStatus::Failed,
            )
            .await
            {
                tracing::warn!(
                    "observability: record_tool_call_done (compat SystemEvent::Failed) failed: {}",
                    e
                );
            }
        }

        _ => {}
    }
}

async fn persist_conversation_turn(state: &Arc<AppState>, msg: &Message) -> Result<(), String> {
    let turn = ConversationTurn {
        id: msg.id,                // Use message id as row id for M0
        parent_id: msg.parent,     // parent message id if any
        message_id: msg.id,        // also record as message_id
        kind: kind_str(msg.kind).to_string(),
        content: msg.content.clone(),
        created_at: Validity {
            at: now_ms(),
            is_valid: true,
        },
        thread_id: None,
    };
    state
        .db
        .upsert_conversation_turn(turn)
        .map_err(|e| e.to_string())
}

async fn persist_tool_requested(
    state: &Arc<AppState>,
    request_id: Uuid,
    parent_id: Uuid,
    vendor: &crate::llm::ToolVendor,
    name: &str,
    arguments: &Value,
    call_id: &str,
) -> Result<(), String> {
    let args_json = canonical_json(arguments);
    let req = ToolCallReq {
        request_id,
        call_id: call_id.to_string(),
        parent_id,
        vendor: vendor_str(vendor).to_string(),
        tool_name: name.to_string(),
        args_sha256: fnv1a64_hex(&args_json),
        arguments_json: Some(args_json),
        started_at: Validity {
            at: now_ms(),
            is_valid: true,
        },
    };
    state
        .db
        .record_tool_call_requested(req)
        .map_err(|e| e.to_string())
}

async fn persist_tool_done(
    state: &Arc<AppState>,
    request_id: Uuid,
    _parent_id: Uuid,
    call_id: &str,
    outcome_json: Option<String>,
    error_kind: Option<String>,
    error_msg: Option<String>,
    status: ToolStatus,
) -> Result<(), String> {
    let done = ToolCallDone {
        request_id,
        call_id: call_id.to_string(),
        ended_at: Validity {
            at: now_ms(),
            is_valid: true,
        },
        latency_ms: 0, // M0: not tracked; future: measure from requested->done
        outcome_json,
        error_kind,
        error_msg,
        status,
    };
    state
        .db
        .record_tool_call_done(done)
        .map_err(|e| e.to_string())
}

async fn get_message(state: &Arc<AppState>, id: Uuid) -> Option<Message> {
    let guard = state.chat.0.read().await;
    guard.messages.get(&id).cloned()
}

fn kind_str(k: MessageKind) -> &'static str {
    match k {
        MessageKind::User => "user",
        MessageKind::Assistant => "assistant",
        MessageKind::System => "system",
        MessageKind::SysInfo => "sysinfo",
        MessageKind::Tool => "tool",
    }
}

fn vendor_str(v: &crate::llm::ToolVendor) -> &'static str {
    match v {
        crate::llm::ToolVendor::OpenAI => "openai",
        crate::llm::ToolVendor::Other => "other",
    }
}

// Deterministic JSON serialization for hashing/persistence
fn canonical_json(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".to_string())
}

// M0: FNV-1a 64-bit hex as a lightweight stand-in for SHA-256 to avoid new deps here.
// Replace with real SHA-256 at a later pass when Cargo changes are allowed.
fn fnv1a64_hex(s: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("fnv1a64:{:016x}", hash)
}

// Quote arbitrary string as JSON string for parse_json on the DB side
fn json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}
