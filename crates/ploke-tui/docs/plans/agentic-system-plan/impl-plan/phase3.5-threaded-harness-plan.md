# Phase 3.5: Comprehensive Threaded Test Harness - Implementation Plan

## Overview
Create a robust test harness that enables deep integration testing of the complete tool-call message lifecycle with real database, vector embeddings, and multi-threaded app execution.

## Problem Statement
Current tests are too shallow because they lack:
1. **Real Database Integration**: No parsed fixture codebase with vector embeddings
2. **Full Message Lifecycle**: Missing RAG → LLM → Tool → Response pipeline  
3. **Multi-threaded Execution**: App not running on separate thread with realistic subsystems
4. **Comprehensive Deserialization**: No testing of tool calls with `required` mode and failure scenarios
5. **Real Multi-turn Conversations**: No genuine conversation context with code search integration

## Proposed Solution: `RealAppHarness`

### Architecture Design

```rust
pub struct RealAppHarness {
    // App running on dedicated thread
    app_handle: JoinHandle<()>,
    
    // Communication channels to running app
    user_input_tx: mpsc::Sender<String>,
    app_events_rx: broadcast::Receiver<AppEvent>,
    
    // Direct access to app state for verification
    state: Arc<AppState>,
    event_bus: Arc<EventBus>,
    
    // Control and synchronization
    shutdown_tx: oneshot::Sender<()>,
    message_completion_notifier: Arc<Notify>,
}
```

### Key Features

#### ✅ Real Database with Fixture
- Uses `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`
- Pre-loaded with parsed Rust code from `syn_parser` crate
- Vector embeddings ready for HNSW search
- BM25 index available for hybrid search

#### ✅ Full Message Lifecycle Integration  
- **User Input** → `StateCommand::AddUserMessage`
- **Database Scan** → `StateCommand::ScanForChange` 
- **RAG Processing** → `StateCommand::EmbedMessage` with code context
- **LLM Request** → `AppEvent::Llm(Event::Request)` 
- **Tool Execution** → GAT tool dispatch with real database queries
- **Response Integration** → Assistant message with tool results

#### ✅ Multi-threaded App Execution
- App runs on dedicated thread with full subsystems:
  - `state_manager` processing commands
  - `llm_manager` handling requests
  - `run_event_bus` routing events  
  - `observability` tracking metrics
- Test thread communicates via channels
- Realistic timing and concurrency patterns

#### ✅ Comprehensive Tool Testing
- **Required Tool Calls**: Test with `tool_choice: "required"`
- **Deserialization Validation**: Use `tools/mod.rs` match patterns to validate all tool types
- **Error Scenarios**: Test malformed arguments, timeouts, and failures
- **Live API Integration**: Full OpenRouter roundtrips with artifact persistence

## Implementation Steps

### Step 1: Create `RealAppHarness` Structure
```rust
// Location: crates/ploke-tui/tests/real_app_harness.rs

impl RealAppHarness {
    pub async fn spawn_with_fixture() -> Self {
        // Load fixture database with embeddings
        // Spawn app on dedicated thread  
        // Set up communication channels
        // Wait for app initialization
    }
    
    pub async fn send_user_message(&self, content: &str) -> MessageTracker {
        // Send user input to running app
        // Return tracker for monitoring completion
    }
    
    pub async fn wait_for_assistant_response(&self, tracker: MessageTracker) -> AssistantMessage {
        // Wait for complete message lifecycle
        // Return assistant response with tool results
    }
    
    pub async fn shutdown(self) {
        // Graceful shutdown of app thread
        // Clean up resources
    }
}
```

### Step 2: Enhanced Message Tracking
```rust
pub struct MessageTracker {
    pub user_message_id: Uuid,
    pub completion_notifier: Arc<Notify>,
    pub events: Vec<AppEvent>, // Collected events for validation
}

pub struct AssistantMessage {
    pub id: Uuid,
    pub content: String,
    pub tool_calls_made: Vec<ToolCallRecord>,
    pub processing_time: Duration,
}
```

### Step 3: Comprehensive Tool Validation Tests
```rust
#[tokio::test]
async fn e2e_required_tool_calls_comprehensive() {
    let harness = RealAppHarness::spawn_with_fixture().await;
    
    // Test each tool type with required mode
    for tool_name in ["get_file_metadata", "request_code_context", "apply_code_edit"] {
        let tracker = harness.send_user_message(&format!(
            "Use the {} tool to help with this request", tool_name
        )).await;
        
        let response = harness.wait_for_assistant_response(tracker).await;
        
        // Validate tool was called
        assert!(!response.tool_calls_made.is_empty());
        
        // Test deserialization using tools/mod.rs patterns
        for tool_call in &response.tool_calls_made {
            match tool_call.name {
                ToolName::GetFileMetadata => {
                    let params = GetFileMetadata::deserialize_params(&tool_call.params_json)
                        .expect("GetFileMetadata deserialization should succeed");
                    // Validate params structure
                },
                ToolName::RequestCodeContext => {
                    let params = RequestCodeContextGat::deserialize_params(&tool_call.params_json)
                        .expect("RequestCodeContext deserialization should succeed");
                    // Validate params structure  
                },
                ToolName::ApplyCodeEdit => {
                    let params = GatCodeEdit::deserialize_params(&tool_call.params_json)
                        .expect("ApplyCodeEdit deserialization should succeed");
                    // Validate params structure
                },
            }
        }
    }
    
    harness.shutdown().await;
}
```

### Step 4: Multi-turn Conversation with Real RAG
```rust
#[tokio::test]
async fn e2e_multi_turn_with_rag_integration() {
    let harness = RealAppHarness::spawn_with_fixture().await;
    
    // Turn 1: Ask about code in fixture
    let tracker1 = harness.send_user_message(
        "What Rust structs are defined in the fixture codebase?"
    ).await;
    let response1 = harness.wait_for_assistant_response(tracker1).await;
    
    // Validate RAG was used (should have code context)
    assert!(!response1.tool_calls_made.is_empty());
    let context_calls: Vec<_> = response1.tool_calls_made.iter()
        .filter(|call| call.name == ToolName::RequestCodeContext)
        .collect();
    assert!(!context_calls.is_empty(), "Should use request_code_context");
    
    // Turn 2: Follow-up question building on context
    let tracker2 = harness.send_user_message(
        "Can you get the metadata for the main Cargo.toml file?"
    ).await;
    let response2 = harness.wait_for_assistant_response(tracker2).await;
    
    // Validate file metadata tool was used
    let metadata_calls: Vec<_> = response2.tool_calls_made.iter()
        .filter(|call| call.name == ToolName::GetFileMetadata)
        .collect();
    assert!(!metadata_calls.is_empty(), "Should use get_file_metadata");
    
    // Turn 3: Make an edit based on gathered information
    let tracker3 = harness.send_user_message(
        "Add a comment to one of the structs you found earlier"
    ).await;
    let response3 = harness.wait_for_assistant_response(tracker3).await;
    
    // Validate edit tool was used
    let edit_calls: Vec<_> = response3.tool_calls_made.iter()
        .filter(|call| call.name == ToolName::ApplyCodeEdit)
        .collect();
    assert!(!edit_calls.is_empty(), "Should use apply_code_edit");
    
    harness.shutdown().await;
}
```

## Success Criteria

### ✅ Infrastructure Validation
- [ ] RealAppHarness spawns app on separate thread successfully
- [ ] Fixture database loads with vector embeddings and BM25 index
- [ ] Message lifecycle completes end-to-end with real RAG integration
- [ ] Communication channels work reliably under load

### ✅ Tool Call Validation  
- [ ] All tool types deserialize correctly using GAT patterns
- [ ] Required tool calls function properly with live endpoints
- [ ] Tool execution integrates with real database queries
- [ ] Error scenarios handled gracefully with proper fallbacks

### ✅ Multi-turn Conversation Validation
- [ ] Context builds across multiple turns with RAG
- [ ] Tool results influence subsequent tool selections
- [ ] Conversation state persists correctly across requests
- [ ] Performance remains acceptable under realistic usage

### ✅ Deserialization Testing
- [ ] Test with malformed tool arguments (should panic as requested)
- [ ] Validate all fields in tool parameters are handled correctly
- [ ] Test edge cases like empty parameters, missing fields
- [ ] Confirm zero-copy borrowing works in realistic scenarios

## Phase Completion Evidence
- Working multi-turn conversations with real code context
- Comprehensive tool deserialization validation
- Performance metrics for realistic usage patterns  
- Artifact generation showing tool execution traces
- Database integrity maintained across all test scenarios

This harness will provide the foundation for all subsequent testing phases, enabling true end-to-end validation of the agentic tool system.
