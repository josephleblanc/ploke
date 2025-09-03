#![cfg(feature = "test_harness")]

//! Minimal end-to-end tool test to validate basic functionality

use std::time::Duration;
use ploke_core::ArcStr;
use ploke_tui::tools::ToolName;
use tokio::time::timeout;
use uuid::Uuid;
use serde_json::json;

mod harness;
use harness::AppHarness;

use ploke_tui::AppEvent;
use ploke_tui::system::SystemEvent;
use ploke_tui::rag::utils::ToolCallParams;

/// Basic test that validates tool execution works
#[tokio::test]
async fn e2e_minimal_get_file_metadata() -> color_eyre::Result<()> {
    let harness = AppHarness::spawn().await?;
    
    // Create a test file
    let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let test_file = temp_dir.path().join("test.rs");
    std::fs::write(&test_file, "fn test() { return 42; }\n")
        .expect("Failed to write test file");

    let request_id = Uuid::new_v4();
    let call_id = ArcStr::from("test_tool_call:0");
    
    // Subscribe to events
    let mut event_rx = harness.event_bus.realtime_tx.subscribe();
    
    // Create tool call parameters
    let tool_params = ToolCallParams {
        state: harness.state.clone(),
        event_bus: harness.event_bus.clone(),
        request_id,
        parent_id: Uuid::new_v4(),
        name: ToolName::GetFileMetadata,
        arguments: json!({"file_path": test_file.display().to_string()}),
        call_id: call_id.clone(),
    };

    // Execute tool
    tokio::spawn(async move {
        ploke_tui::rag::dispatcher::handle_tool_call_requested(tool_params).await;
    });

    // Wait for completion
    let result = timeout(Duration::from_secs(5), async {
        loop {
            match event_rx.recv().await {
                Ok(AppEvent::System(SystemEvent::ToolCallCompleted { 
                    request_id: rid, 
                    call_id: cid, 
                    content, 
                    .. 
                })) if rid == request_id && cid == call_id => {
                    return Ok(content);
                }
                Ok(AppEvent::System(SystemEvent::ToolCallFailed { 
                    request_id: rid, 
                    call_id: cid, 
                    error, 
                    .. 
                })) if rid == request_id && cid == call_id => {
                    return Err(error);
                }
                Ok(_) => continue,
                Err(_) => return Err("Event channel closed".to_string()),
            }
        }
    }).await;

    assert!(result.is_ok(), "Tool execution timed out");
    let content = result.unwrap();
    assert!(content.is_ok(), "Tool execution failed: {:?}", content);
    
    let metadata_json = content.unwrap();
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&metadata_json);
    assert!(parsed.is_ok(), "Tool result is not valid JSON: {}", metadata_json);
    
    let metadata = parsed.unwrap();
    assert!(metadata.get("ok").is_some(), "Tool result missing 'ok' field");
    assert!(metadata.get("exists").is_some(), "Tool result missing 'exists' field");
    
    println!("✓ Basic tool execution test passed");
    
    harness.shutdown().await;
    Ok(())
}
