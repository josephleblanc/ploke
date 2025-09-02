#![cfg(test)]

use std::sync::Arc;
use ploke_core::ArcStr;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

#[tokio::test]
async fn tool_dispatch_malformed_args_emits_failed() {
    use ploke_tui::event_bus::EventBusCaps;
    use ploke_tui::{AppEvent, EventBus};
    use ploke_tui::system::SystemEvent;
    use ploke_tui::rag::utils::ToolCallParams;

    let state = ploke_tui::test_harness::get_state().await;
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let request_id = Uuid::new_v4();
    let call_id = ArcStr::from("test_tool_call:0");
    let mut rx = event_bus.realtime_tx.subscribe();

    // Malformed args for get_file_metadata (expects { file_path: string })
    let bad_args = serde_json::json!({"pathz": 123});
    let params = ToolCallParams {
        state: Arc::clone(&state),
        event_bus: Arc::clone(&event_bus),
        request_id,
        parent_id: Uuid::new_v4(),
        name: "get_file_metadata".to_string(),
        arguments: bad_args,
        call_id: call_id.clone(),
    };

    tokio::spawn(async move {
        ploke_tui::rag::dispatcher::handle_tool_call_requested(params).await;
    });

    // Expect ToolCallFailed on the realtime channel
    let got = timeout(Duration::from_secs(5), async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::System(SystemEvent::ToolCallFailed { request_id: rid, call_id: cid, error, .. }))
                    if rid == request_id && cid == call_id => break Some(error),
                Ok(_) => continue,
                Err(_) => break None,
            }
        }
    }).await.expect("timeout");

    assert!(got.is_some(), "expected ToolCallFailed for malformed args");
}

