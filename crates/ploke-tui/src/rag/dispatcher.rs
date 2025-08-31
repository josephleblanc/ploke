use crate::rag::tools::apply_code_edit_tool;

use super::{tools::get_file_metadata_tool, utils::ToolCallParams, *};
// TODO: Route get_file_metadata via GAT once Send/'static issue resolved in spawn path

pub async fn handle_tool_call_requested(tool_call_params: ToolCallParams) {
    let ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        name,
        arguments,
        call_id,
    } = tool_call_params.clone();
    tracing::info!(
        request_id = %request_id,
        parent_id = %parent_id,
        call_id = %call_id,
        name = %name,
        "handle_tool_call_requested"
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
        name: name.clone(),
        arguments: arguments.clone(),
        call_id: call_id.clone(),
    };

    match name.as_str() {
        "apply_code_edit" => apply_code_edit_tool(tool_call_params).await,
        // Route get_file_metadata through the GAT-based dispatcher
        "get_file_metadata" => get_file_metadata_tool(tool_call_params).await,
        // Keep request_code_context on legacy RAG handler for now
        // "request_code_context" => handle_request_context(tool_call_params).await,
        _ => {
            tracing::warn!("Unsupported tool call: {}", name);
            let err = format!("Unsupported tool: {}", name);
            let _ = tool_call_params.event_bus.realtime_tx.send(tool_call_failed(err));
            return;
        }
    }
}
