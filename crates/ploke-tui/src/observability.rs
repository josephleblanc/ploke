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

use serde::{Serialize, Deserialize};

/// Parameter bundle for persisting a tool-call "requested" lifecycle event.
/// Prefer typed fields; serialize to JSON strings only at DB boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolRequestPersistParams {
    request_id: Uuid,
    parent_id: Uuid,
    vendor: crate::llm::ToolVendor,
    tool_name: String,
    arguments: Value,
    call_id: String,
}

/// Parameter bundle for persisting a tool-call terminal lifecycle event.
/// outcome carries structured JSON; error is a string message when failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolDonePersistParams {
    request_id: Uuid,
    parent_id: Uuid,
    call_id: String,
    outcome: Option<Value>,
    error: Option<String>,
    status: ToolStatus,
}

/// NOTE: These typed params keep conversion at the boundary, moving us toward
/// more type-safe patterns. Future work: ploke-db to accept Json directly to
/// avoid string round-trips; track latency by correlating start/end timestamps.
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
            let params = ToolRequestPersistParams {
                request_id,
                parent_id,
                vendor,
                tool_name: name,
                arguments,
                call_id,
            };
            if let Err(e) = persist_tool_requested(state, &params).await {
                tracing::warn!("observability: record_tool_call_requested failed: {}", e);
            }
        }
        AppEvent::LlmTool(ToolEvent::Completed {
            request_id,
            parent_id,
            call_id,
            content,
        }) => {
            let params = ToolDonePersistParams {
                request_id,
                parent_id,
                call_id,
                outcome: Some(Value::String(content)),
                error: None,
                status: ToolStatus::Completed,
            };
            if let Err(e) = persist_tool_done(state, &params).await {
                tracing::warn!("observability: record_tool_call_done (completed) failed: {}", e);
            }
        }
        AppEvent::LlmTool(ToolEvent::Failed {
            request_id,
            parent_id,
            call_id,
            error,
        }) => {
            let params = ToolDonePersistParams {
                request_id,
                parent_id,
                call_id,
                outcome: None,
                error: Some(error),
                status: ToolStatus::Failed,
            };
            if let Err(e) = persist_tool_done(state, &params).await {
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
            let params = ToolRequestPersistParams {
                request_id,
                parent_id,
                vendor,
                tool_name: name,
                arguments,
                call_id,
            };
            if let Err(e) = persist_tool_requested(state, &params).await {
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
            let params = ToolDonePersistParams {
                request_id,
                parent_id,
                call_id,
                outcome: Some(Value::String(content)),
                error: None,
                status: ToolStatus::Completed,
            };
            if let Err(e) = persist_tool_done(state, &params).await {
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
            let params = ToolDonePersistParams {
                request_id,
                parent_id,
                call_id,
                outcome: None,
                error: Some(error),
                status: ToolStatus::Failed,
            };
            if let Err(e) = persist_tool_done(state, &params).await {
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
    params: &ToolRequestPersistParams,
) -> Result<(), String> {
    let args_json = serde_json::to_string(&params.arguments).unwrap_or_else(|_| "null".to_string());
    let req = ToolCallReq {
        request_id: params.request_id,
        call_id: params.call_id.clone(),
        parent_id: params.parent_id,
        vendor: vendor_str(&params.vendor).to_string(),
        tool_name: params.tool_name.clone(),
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
    params: &ToolDonePersistParams,
) -> Result<(), String> {
    let outcome_json = match &params.outcome {
        Some(v) => Some(serde_json::to_string(v).unwrap_or_else(|_| "null".to_string())),
        None => None,
    };
    let ended_at_ms = now_ms();
    let started_at_ms = match state
        .db
        .get_tool_call(params.request_id, &params.call_id)
    {
        Ok(Some((req, _))) => req.started_at.at,
        _ => ended_at_ms,
    };
    let latency_ms = if ended_at_ms >= started_at_ms {
        ended_at_ms - started_at_ms
    } else {
        0
    };
    let done = ToolCallDone {
        request_id: params.request_id,
        call_id: params.call_id.clone(),
        ended_at: Validity {
            at: ended_at_ms,
            is_valid: true,
        },
        latency_ms,
        outcome_json,
        error_kind: None,
        error_msg: params.error.clone(),
        status: params.status,
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


fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}
