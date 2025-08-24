use crate::rag::{tools::{apply_code_edit_tool, get_file_metadata_tool}, utils::calc_top_k_for_budget};

use super::{tools::handle_request_context, utils::ToolCallParams, *};

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
        "request_code_context" => handle_request_context(tool_call_params).await,
        _ => {
            tracing::warn!("Unsupported tool call: {}", name);
            let err = format!("Unsupported tool: {}", name);
            let _ = event_bus.realtime_tx.send(tool_call_failed(err.clone()));
            return;
        }
    }
}
