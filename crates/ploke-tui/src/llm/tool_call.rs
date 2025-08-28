use std::sync::Arc;

use serde_json::Value;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::llm::ToolEvent;
use crate::{AppEvent, EventBus};

use super::session::await_tool_result;

#[derive(Clone, Debug)]
pub struct ToolCallSpec {
    pub name: String,
    pub arguments: Value,
    pub call_id: String,
}

/// Dispatch a single tool call via the EventBus and await its correlated result.
/// Returns (spec, Ok(content)) on success, or (spec, Err(error)) on failure/timeout.
pub async fn dispatch_and_wait(
    event_bus: &Arc<EventBus>,
    parent_id: Uuid,
    spec: ToolCallSpec,
    timeout_secs: u64,
) -> (ToolCallSpec, Result<String, String>) {
    // Subscribe before sending, to avoid missing fast responses
    let rx = event_bus.realtime_tx.subscribe();

    // Emit typed tool event to trigger tool execution (compat bridge handled in llm_manager)
    let request_id = Uuid::new_v4();
    tracing::info!(
        request_id = %request_id,
        parent_id = %parent_id,
        tool = %spec.name,
        call_id = %spec.call_id,
        "Emitting LlmTool::Requested"
    );
    event_bus.send(AppEvent::LlmTool(ToolEvent::Requested {
        request_id,
        parent_id,
        name: spec.name.clone(),
        arguments: spec.arguments.clone(),
        call_id: spec.call_id.clone(),
    }));

    // Await ToolCallCompleted/Failed with correlation (request_id, call_id)
    let res = await_tool_result(rx, request_id, &spec.call_id, timeout_secs).await;
    (spec, res)
}

/// Execute multiple tool calls concurrently, returning outcomes in stable order by call_id.
pub async fn execute_tool_calls(
    event_bus: &Arc<EventBus>,
    parent_id: Uuid,
    specs: Vec<ToolCallSpec>,
    timeout_secs: u64,
) -> Vec<(ToolCallSpec, Result<String, String>)> {
    let mut set = JoinSet::new();

    for spec in specs.into_iter() {
        let eb = Arc::clone(event_bus);
        set.spawn(async move { dispatch_and_wait(&eb, parent_id, spec, timeout_secs).await });
    }

    let mut outcomes: Vec<(ToolCallSpec, Result<String, String>)> = Vec::new();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(result) => outcomes.push(result),
            Err(e) => {
                // Join error; if this happens, we cannot recover the spec cleanly.
                // Skip recording this entry; the session should handle missing results via timeout.
                tracing::warn!("tool call task join error: {}", e);
            }
        }
    }

    // Stable deterministic ordering by provider call_id
    outcomes.sort_by(|(a, _), (b, _)| a.call_id.cmp(&b.call_id));
    outcomes
}
