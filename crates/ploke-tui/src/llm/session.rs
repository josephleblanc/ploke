use std::time::Duration;

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::AppEvent;
use crate::system::SystemEvent;

/// Await a correlated ToolCall completion/failure on the realtime broadcast channel.
///
/// - `rx`: a subscribed `broadcast::Receiver<AppEvent>` (must be subscribed before the request is emitted)
/// - `request_id`: the UUID assigned to this tool call request
/// - `call_id`: provider-assigned tool call id (string)
/// - `timeout_secs`: how many seconds to wait before returning a timeout error
///
/// Returns Ok(content) when ToolCallCompleted is received with matching (request_id, call_id),
/// or Err(error_string) when ToolCallFailed or other failure occurs (including timeout).
pub async fn await_tool_result(
    mut rx: broadcast::Receiver<AppEvent>,
    request_id: Uuid,
    call_id: &str,
    timeout_secs: u64,
) -> Result<String, String> {
    let wait = async {
        loop {
            match rx.recv().await {
                Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id: rid,
                    call_id: cid,
                    content,
                    ..
                })) if rid == request_id && cid == call_id => {
                    break Ok(content);
                }
                Ok(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id: rid,
                    call_id: cid,
                    error,
                    ..
                })) if rid == request_id && cid == call_id => {
                    break Err(error);
                }
                Ok(_) => {
                    // unrelated event; keep waiting
                }
                Err(e) => {
                    break Err(format!("Event channel error: {}", e));
                }
            }
        }
    };

    match tokio::time::timeout(Duration::from_secs(timeout_secs), wait).await {
        Ok(Ok(content)) => Ok(content),
        Ok(Err(err)) => Err(err),
        Err(_) => Err("Timed out waiting for tool result".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use tokio::time::sleep;

    use crate::EventBus;
    use crate::EventBusCaps;
    use crate::AppEvent;
    use crate::system::SystemEvent;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_await_tool_result_completed() {
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let rx = event_bus.realtime_tx.subscribe();
        let request_id = Uuid::new_v4();
        let call_id = "call-123".to_string();
        let content = "tool response".to_string();
        let eb = event_bus.clone();

        // spawn sender that emits completion shortly after
        let call_id_for_task = call_id.clone();
        let content_for_task = content.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            eb.send(AppEvent::System(SystemEvent::ToolCallCompleted {
                request_id,
                parent_id: Uuid::new_v4(),
                call_id: call_id_for_task,
                content: content_for_task,
            }));
        });

        let res = await_tool_result(rx, request_id, &call_id, 5).await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), content);
    }

    #[tokio::test]
    async fn test_await_tool_result_failed() {
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let rx = event_bus.realtime_tx.subscribe();
        let request_id = Uuid::new_v4();
        let call_id = "call-err".to_string();
        let error_msg = "something went wrong".to_string();
        let eb = event_bus.clone();

        let call_id_for_task = call_id.clone();
        let error_msg_for_task = error_msg.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            eb.send(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id,
                parent_id: Uuid::new_v4(),
                call_id: call_id_for_task,
                error: error_msg_for_task,
            }));
        });

        let res = await_tool_result(rx, request_id, &call_id, 5).await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), error_msg);
    }
}
