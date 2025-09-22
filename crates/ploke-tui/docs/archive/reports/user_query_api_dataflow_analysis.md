# User Query and API Dataflow Analysis

*Generated: 2025-09-06*

This document traces the complete data flow from user input through to LLM API request and response processing in Ploke TUI.

## 1. User Input Processing Chain

### Phase 1: Keyboard Input Capture
**Location**: `crates/ploke-tui/src/app/mod.rs` (main event loop)

**Process**:
1. **Terminal Events**: Crossterm captures keyboard events in main run loop
2. **Input Stream**: `crossterm::event::EventStream` provides async stream of terminal events
3. **Event Processing**: Each `crossterm::event::Event` handled in run loop

### Phase 2: Keymap Translation
**Location**: `crates/ploke-tui/src/app/input/keymap.rs`

**Key Binding Resolution**:
```rust
// Insert Mode: Enter key triggers submission
(m, KeyCode::Enter) if m.is_empty() || m == KeyModifiers::SHIFT => Some(Action::Submit)

// Command Mode: Enter executes command
(m, KeyCode::Enter) if m.is_empty() || m == KeyModifiers::SHIFT => Some(Action::ExecuteCommand)
```

**Mode-Specific Handling**:
- **Insert Mode**: Direct message input, Enter = Submit
- **Command Mode**: Slash commands (e.g., `/model search gpt`), Enter = ExecuteCommand
- **Normal Mode**: Vim-like navigation, 'i' enters Insert mode

### Phase 3: Action Execution
**Location**: `crates/ploke-tui/src/app/mod.rs` - `Action::Submit` handling

**Submit Action Flow**:
```rust
Action::Submit => {
    if !self.input_buffer.is_empty() && !self.input_buffer.starts_with('\n') {
        let new_msg_id = Uuid::new_v4();
        
        // 1. Add user message to chat history
        self.send_cmd(StateCommand::AddUserMessage {
            content: self.input_buffer.clone(),
            new_msg_id,
            completion_tx,
        });
        
        // 2. Scan for file changes
        self.send_cmd(StateCommand::ScanForChange { scan_tx });
        
        // 3. Embed user message for RAG
        self.send_cmd(StateCommand::EmbedMessage {
            new_msg_id,
            completion_rx,
            scan_rx,
        });
        
        // 4. Create system message about embedding
        self.send_cmd(StateCommand::AddMessage { /* ... */ });
    }
}
```

## 2. State Manager Processing

### Command Queue Processing
**Location**: `crates/ploke-tui/src/app_state/dispatcher.rs:33-63`

**StateCommand::AddUserMessage Flow**:
1. **Command Receipt**: State manager receives command from bounded channel (1024 capacity)
2. **Handler Dispatch**: Routes to `handlers::chat::add_user_message()`
3. **Chat History Update**: Message added to conversation tree structure
4. **Event Emission**: `AppEvent::MessageUpdated` sent to event bus

### Chat Handler Processing
**Location**: `crates/ploke-tui/src/app_state/handlers/chat.rs`

**Message Addition Process**:
```rust
pub async fn add_user_message(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    new_msg_id: Uuid,
    content: String,
    completion_tx: oneshot::Sender<Uuid>,
) {
    // 1. Acquire write lock on chat history
    let mut guard = state.chat.0.write().await;
    
    // 2. Create user message with metadata
    let message = Message {
        id: new_msg_id,
        content,
        kind: MessageKind::User,
        status: MessageStatus::Complete,
        // ... other fields
    };
    
    // 3. Add to conversation tree
    let parent_id = guard.current;
    guard.add_message(parent_id, message);
    
    // 4. Notify completion
    let _ = completion_tx.send(new_msg_id);
    
    // 5. Emit events
    event_bus.send(AppEvent::MessageUpdated(/* ... */));
}
```

## 3. RAG Context Assembly

### Embedding Pipeline
**Location**: `StateCommand::EmbedMessage` processing

**Process**:
1. **Message Embedding**: User message converted to vector embedding
2. **Context Assembly**: RAG system queries for relevant code context:
   - **Vector Search**: Semantic similarity using embedding
   - **BM25 Search**: Keyword-based search for exact matches
   - **Hybrid Fusion**: Combines vector + BM25 results using RRF (Reciprocal Rank Fusion)
3. **Context Preparation**: Assembled context prepared for LLM prompt

### Context Integration
**Location**: RAG context processing creates system messages with code context

**Context Message Structure**:
- **Code snippets**: Relevant source code sections
- **File metadata**: File paths, tracking hashes
- **Dependency info**: Import relationships, function signatures

## 4. LLM Request Formation

### Assistant Message Creation
**Trigger**: Context assembly completion triggers `StateCommand::CreateAssistantMessage`

**Location**: `handlers::chat::create_assistant_message()`

**Process**:
1. **Conversation History Extraction**: Builds message chain from conversation tree
2. **System Prompt Construction**: Adds system instructions and context
3. **Message Formatting**: Converts to OpenRouter API format
4. **LLM Event Emission**: `AppEvent::Llm(Event::Request)` sent to event bus

### Message Chain Assembly
**Format Conversion**:
```rust
// Internal Message -> API RequestMessage
RequestMessage {
    role: Role::User,           // or Assistant/System
    content: message.content,
    tool_call_id: None,        // Set for tool responses
}
```

**History Management**:
- **Token Budget**: Messages capped by token limit (default: 8196)
- **Character Budget**: Alternative capping by character count
- **Context Prioritization**: Recent messages + relevant context prioritized

## 5. LLM Manager Processing

### Request Session Creation
**Location**: `crates/ploke-tui/src/llm/mod.rs` - `llm_manager()` function

**Event Loop Processing**:
```rust
while let Ok(event) = event_rx.recv().await {
    match event {
        AppEvent::Llm(Event::Request { parent_id, new_msg_id, .. }) => {
            // 1. Extract provider configuration
            let provider_config = state.config.get_active_provider();
            
            // 2. Build request payload
            let session = RequestSession::new(/* ... */);
            
            // 3. Execute async request
            tokio::spawn(async move {
                match session.run().await {
                    Ok(response) => { /* ... */ },
                    Err(error) => { /* ... */ },
                }
            });
        }
    }
}
```

### API Request Construction
**Location**: `crates/ploke-tui/src/llm/session.rs` - `RequestSession::run()`

**Request Payload Assembly**:
```rust
let request_payload = build_comp_req(
    provider,           // ModelConfig with API key, base URL, model name
    effective_messages, // Conversation history within token budget
    &params,           // LLMParameters (temperature, max_tokens, etc.)
    tools,             // Available tool definitions (optional)
    use_tools,         // Whether to include tools in request
    require_parameters, // Parameter validation flag
);
```

**Tool Integration**:
- **Tool Definitions**: JSON schema for available tools (code_edit, request_context, file_metadata)
- **Tool Calling**: LLM can request tool execution via structured responses
- **Tool Results**: Tool outputs fed back into conversation

## 6. API Request Execution

### HTTP Request Details
**Location**: `RequestSession::run()` - API call execution

**Request Structure**:
```json
{
  "model": "moonshotai/kimi-k2",
  "messages": [
    {
      "role": "system",
      "content": "You are a helpful assistant..."
    },
    {
      "role": "user", 
      "content": "User's query..."
    }
  ],
  "tools": [/* tool definitions */],
  "temperature": 0.7,
  "max_tokens": 8196,
  "provider": {
    "order": ["MoonshotAI"],
    "require_parameters": true
  }
}
```

**Endpoint Details**:
- **URL**: `{provider.base_url}/chat/completions` (e.g., `https://openrouter.ai/api/v1/chat/completions`)
- **Method**: POST
- **Headers**: `Authorization: Bearer {api_key}`, `Content-Type: application/json`
- **Timeout**: 45 seconds default (configurable)

### Error Handling & Retries
**Failure Modes**:
1. **404 Errors**: Retry without tools if provider doesn't support them
2. **Rate Limiting**: Exponential backoff (not implemented yet)
3. **Network Errors**: Timeout and connection failures
4. **Invalid Tool Calls**: Retry with corrected tool call format

## 7. Response Processing

### API Response Parsing
**Location**: `RequestSession::run()` - response handling

**Response Structure Handling**:
```rust
let choice = response_body.choices.first()
    .ok_or(LlmError::InvalidResponse("No choices"))?;

match &choice.message.tool_calls {
    Some(tool_calls) if !tool_calls.is_empty() => {
        // Process tool calls
        for tool_call in tool_calls {
            let result = process_tool(tool_call, &ctx).await?;
            // Add tool result to message history
        }
        // Continue conversation with tool results
    },
    _ => {
        // Regular text response
        let content = choice.message.content
            .ok_or(LlmError::EmptyResponse)?;
        return Ok(content);
    }
}
```

### Tool Call Processing
**Tool Execution Pipeline**:
1. **Tool Validation**: Verify tool call format and parameters
2. **Context Assembly**: Prepare execution context (file paths, state handles)
3. **Tool Execution**: Execute requested operation (file read, code edit, etc.)
4. **Result Serialization**: Convert tool output to JSON for LLM
5. **Conversation Continuation**: Add tool results to message history, request follow-up

### Response Integration
**Message Update Process**:
1. **Assistant Message Creation**: LLM response added to conversation
2. **Status Updates**: Message status updated (Pending -> Complete)
3. **UI Refresh**: `AppEvent::MessageUpdated` triggers UI redraw
4. **Conversation State**: Current message pointer updated

## 8. Event Bus Coordination

### Event Flow Architecture
**Priority Routing**:
- **Realtime Events**: UI updates, user interactions (prioritized)
- **Background Events**: API calls, tool execution, system operations

**Key Events in Query Flow**:
1. `AppEvent::Ui(UiEvent::InputSubmitted)` - User input
2. `AppEvent::MessageUpdated` - Chat history changes  
3. `AppEvent::Llm(Event::Request)` - LLM request initiation
4. `AppEvent::LlmTool(ToolEvent::*)` - Tool execution events
5. `AppEvent::System(SystemEvent::*)` - System state changes

### Coordination Patterns
**Async Coordination**:
- **Oneshot Channels**: Coordination between UI and state manager
- **Broadcast Events**: Loosely coupled subsystem communication
- **Bounded Channels**: Backpressure handling (1024 message buffer)

## 9. Performance Characteristics

### Async Patterns
**Concurrent Execution**:
- **UI Non-Blocking**: Input processing doesn't block rendering
- **Background Processing**: RAG context assembly runs in background
- **Parallel Tool Calls**: Multiple tools can execute concurrently

### Memory Management
**Efficient Data Flow**:
- **Arc Sharing**: State shared efficiently across tasks
- **String Interning**: ArcStr used for frequently shared strings
- **Bounded Buffers**: Prevents unbounded memory growth

### Request Optimization
**Context Management**:
- **Token Budgeting**: Automatic message history truncation
- **Context Caching**: RAG results cached for reuse
- **Connection Pooling**: HTTP client reused across requests

## 10. Observable Data Points

### Tracing Integration
**Structured Logging**:
```rust
tracing::debug!(
    model = %provider.model,
    base_url = %provider.base_url,
    use_tools = use_tools,
    tools = %tool_names.join(","),
    "dispatch_request"
);
```

### Event Telemetry
**Request Lifecycle Events**:
- **Tool Call Requested**: `ToolEvent::Requested`
- **Tool Call Completed**: `ToolEvent::Completed` 
- **Tool Call Failed**: `ToolEvent::Failed`
- **System Events**: Model switches, configuration changes

This comprehensive analysis shows how user input flows through multiple async subsystems before resulting in an LLM API call, with careful attention to error handling, performance, and observability throughout the pipeline.