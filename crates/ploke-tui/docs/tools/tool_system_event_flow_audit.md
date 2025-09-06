# Tool System Event Flow Audit Report

## Executive Summary

The current tool system has **significant redundancy and complexity issues** that need immediate attention:

1. **Dual Event Pathways**: Two completely separate event types handle the same tool operations
2. **Sequential Execution**: Tools are executed sequentially instead of concurrently
3. **Event Correlation Complexity**: Multiple listeners for essentially duplicate events
4. **Legacy Fallback Issues**: GAT implementation falls back to legacy code instead of failing properly

## Current Event Flow Analysis

### Complete Tool Call Lifecycle

```
User Message → LLM Request → OpenRouter API Response → Tool Calls Detected
    ↓
RequestSession::run() processes tool_calls sequentially:
    ↓
For each tool call:
    1. event_bus.send(AppEvent::LlmTool(ToolEvent::Requested))
    2. await_tool_result() subscribes to events  
    3. llm_manager() receives ToolEvent::Requested
    4. llm_manager() spawns spawn_tool_call()
    5. spawn_tool_call() → rag::dispatcher::handle_tool_call_requested()
    6. handle_tool_call_requested() → tools::dispatch_gat_tool()
    7. dispatch_gat_tool() tries GAT, falls back to legacy on failure
    8. Tool execution emits BOTH:
       - AppEvent::System(SystemEvent::ToolCallCompleted)
       - AppEvent::LlmTool(ToolEvent::Completed) [from legacy path]
    9. await_tool_result() listens for EITHER event type
    10. Tool result added to conversation
    11. Continue to next tool call (sequential)
```

### Identified Problems

#### 1. **Redundant Event Pathways**

**Problem**: Two event types do the same thing:
- `AppEvent::LlmTool(ToolEvent::Requested/Completed/Failed)`
- `AppEvent::System(SystemEvent::ToolCallRequested/Completed/Failed)`

**Evidence**:
```rust
// await_tool_result() listens for BOTH event types:
AppEvent::LlmTool(ToolEvent::Completed { .. }) => return Ok(content),
AppEvent::System(SystemEvent::ToolCallCompleted { .. }) => return Ok(content),
```

**Impact**: 
- Event bus pollution
- Complex correlation logic
- Unclear which path is actually used
- Maintenance burden

#### 2. **Sequential Tool Execution**

**Problem**: Tools are executed one at a time in `RequestSession::run()`:

```rust
// Current implementation - SEQUENTIAL
for call in tool_calls {
    // ... setup ...
    match await_tool_result(rx, request_id, call_id.clone(), timeout).await {
        Ok(content) => { /* add to messages */ }
        Err(err) => { /* handle error */ }
    }
}
// Continue loop after ALL tools complete
```

**Should be**: Concurrent execution with `tokio::task::spawn` and future joining.

#### 3. **Legacy Fallback Anti-Pattern**

**Problem**: GAT tools fall back to legacy instead of failing:

```rust
// In dispatch_gat_tool() - BAD PATTERN
"apply_code_edit" => {
    match code_edit::GatCodeEdit::deserialize_params(&args_str) {
        Ok(params) => /* GAT path */,
        Err(_e) => {
            // Fallback to legacy handler - WRONG!
            crate::rag::tools::apply_code_edit_tool(tool_call_params.clone()).await;
        }
    }
}
```

**Impact**: 
- Hides GAT implementation bugs
- Legacy code never gets removed
- Inconsistent behavior
- Testing complexity

#### 4. **Tool Call Persistence Issues**

**Problem**: Full tool outputs are stored in conversation history:

```rust
// In RequestSession::run()
self.messages.push(RequestMessage::new_tool(content, call_id.clone()));
```

**Should be**: Tool call cache with summary references in conversation.

## Recommended Event Flow (Fixed)

### Simplified Event Pathway

**Single Event Type**: Use only `AppEvent::System(SystemEvent::ToolCall*)`

```
User Message → LLM Request → OpenRouter API Response → Tool Calls Detected
    ↓
RequestSession::run() processes tool_calls CONCURRENTLY:
    ↓
1. Spawn all tool calls concurrently:
   let tasks: Vec<_> = tool_calls.into_iter()
       .map(|call| tokio::task::spawn(async move { /* tool execution */ }))
       .collect();

2. Each spawned task:
   a. event_bus.send(SystemEvent::ToolCallRequested) [optional for logging]
   b. tools::dispatch_gat_tool() - NO FALLBACK
   c. Tool execution emits SystemEvent::ToolCallCompleted/Failed
   d. Tool details stored in ToolCallCache
   e. Return tool summary for conversation

3. Join all futures:
   let results = futures::future::join_all(tasks).await;

4. Add tool summaries (not full outputs) to conversation
5. Continue LLM request with tool summaries
```

### Tool Call Cache Architecture

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub id: Uuid,
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub call_id: ArcStr,
    pub tool_name: ArcStr,
    pub arguments: Value,
    pub result: ToolCallResult,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub execution_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolCallResult {
    Success { content: serde_json::Value, summary: String },
    Error { error: String },
}

pub struct ToolCallCache {
    records: Arc<RwLock<HashMap<Uuid, ToolCallRecord>>>,
    // Future: persistence to ploke-db
}
```

## Action Items

### Phase 1: Event Pathway Cleanup ✅ (Identified)
- Remove `AppEvent::LlmTool` events entirely
- Use only `AppEvent::System(SystemEvent::ToolCall*)`
- Update `await_tool_result()` to listen for single event type
- Remove event correlation complexity

### Phase 2: Concurrent Execution Implementation
- Replace sequential tool execution in `RequestSession::run()`
- Implement `tokio::task::spawn` for each tool call
- Add proper future joining with error handling
- Test concurrent event emission and correlation

### Phase 3: Remove Legacy Fallbacks
- Make GAT tool deserialization failure a hard error
- Remove `apply_code_edit_tool()` legacy function
- Complete `GatCodeEdit` implementation
- Remove fallback logic from `dispatch_gat_tool()`

### Phase 4: Tool Call Persistence
- Implement `ToolCallCache` data structure
- Store full tool details in cache
- Add only tool summaries to conversation history
- Link cache entries to conversation via IDs

### Phase 5: Test Quality Audit
- Review all tool tests for actual property verification
- Ensure tests fail when fallbacks are used
- Add concurrent execution testing
- Validate end-to-end API integration

## Critical Issues Summary

1. **Event Redundancy**: Two event pathways doing the same job
2. **Sequential Execution**: Missing concurrent tool processing
3. **Legacy Fallback**: Hiding GAT implementation failures
4. **Storage Inefficiency**: Full tool outputs in conversation history
5. **Test Quality**: Tests may pass with fallbacks instead of intended paths

These issues create maintenance burden, performance problems, and architectural complexity that must be addressed systematically.
