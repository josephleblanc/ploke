use crate::rag::{tools::{apply_code_edit_tool, get_file_metadata_tool}, utils::calc_top_k_for_budget};

use super::{utils::ToolCallParams, *};

#[tracing::instrument(skip(tool_call_params))]
pub async fn handle_tool_call_requested<'a>(tool_call_params: ToolCallParams<'a>) {
    let ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        vendor,
        name,
        arguments,
        call_id,
    } = tool_call_params.clone();
    tracing::info!(
        "handle_tool_call_requested: vendor={:?}, name={}",
        vendor,
        name
    );
    tracing::warn!(
        "DEPRECATED PATH: SystemEvent::ToolCallRequested execution path is deprecated; will be refactored into dedicated tool events. Kept for compatibility."
    );
    let tool_call_failed = |error| {
        AppEvent::System(SystemEvent::ToolCallFailed {
            request_id,
            parent_id,
            call_id: call_id.clone(),
            error,
        })
    };

    let tool_call_params = ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        vendor,
        name: name.clone(),
        arguments: arguments.clone(),
        call_id: call_id.clone(),
    };
    match name.as_str() {
        "apply_code_edit" => apply_code_edit_tool(tool_call_params).await,
        // New: get_file_metadata tool for fetching current file hash and basic metadata
        "get_file_metadata" => get_file_metadata_tool(tool_call_params).await,
        "request_code_context" => {}
        _ => {
            tracing::warn!("Unsupported tool call: {}", name);
            let err = format!("Unsupported tool: {}", name);
            let _ = event_bus.realtime_tx.send(tool_call_failed(err.clone()));
            return;
        }
    }

    if name != "request_code_context" {
        tracing::warn!("Unsupported tool call: {}", name);
        let err = format!("Unsupported tool: {}", name);
        let _ = event_bus.realtime_tx.send(tool_call_failed(err.clone()));
        return;
    }

    // Parse arguments
    let token_budget = arguments
        .get("token_budget")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    if token_budget.is_none() || token_budget == Some(0) {
        let msg = "Invalid or missing token_budget".to_string();
        let _ = event_bus.realtime_tx.send(tool_call_failed(msg.clone()));
        return;
    }
    let token_budget = token_budget.unwrap();
    let hint = arguments
        .get("hint")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Determine query: prefer hint, otherwise last user message
    let query = if let Some(h) = hint.filter(|s| !s.trim().is_empty()) {
        h
    } else {
        let guard = state.chat.read().await;
        match guard.last_user_msg() {
            Ok(Some((_id, content))) => content,
            _ => String::new(),
        }
    };

    if query.trim().is_empty() {
        let msg = "No query available (no hint provided and no recent user message)".to_string();
        let _ = event_bus.realtime_tx.send(tool_call_failed(msg.clone()));
        return;
    }

    let top_k = calc_top_k_for_budget(token_budget);

    if let Some(rag) = &state.rag {
        match rag.hybrid_search(&query, top_k).await {
            Ok(results) => {
                let results_json: Vec<serde_json::Value> = results
                    .into_iter()
                    .map(|(id, score)| serde_json::json!({"id": id.to_string(), "score": score}))
                    .collect();

                let content = serde_json::json!({
                    "ok": true,
                    "query": query,
                    "top_k": top_k,
                    "results": results_json
                })
                .to_string();

                let _ =
                    event_bus
                        .realtime_tx
                        .send(AppEvent::System(SystemEvent::ToolCallCompleted {
                            request_id,
                            parent_id,
                            call_id: call_id.clone(),
                            content,
                        }));
            }
            Err(e) => {
                let msg = format!("RAG hybrid_search failed: {}", e);
                tracing::warn!("{}", msg);
                let _ = event_bus.realtime_tx.send(tool_call_failed(msg));
            }
        }
    } else {
        let msg = "RAG service unavailable".to_string();
        tracing::warn!("{}", msg);
        let _ = event_bus.realtime_tx.send(tool_call_failed(msg.clone()));
    }
}
