use super::{utils::ToolCallParams, *};

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
        call_id = ?call_id,
        name = ?name,
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

    // Route all supported tools via the GAT dispatcher
    crate::tools::dispatch_gat_tool(tool_call_params).await;
}
