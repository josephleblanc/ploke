# Event Flow Summaries

*Generated: 2025-09-06*

This document provides concise summaries of key event flows in Ploke TUI, consolidating the detailed analyses from other reports into actionable flow diagrams.

## 1. User Query to LLM Response Flow

### High-Level Flow
```
User Input → Keymap Translation → Action Execution → State Manager → 
RAG Context Assembly → LLM Request → API Call → Response Processing → UI Update
```

### Detailed Steps
1. **Input Capture**: Crossterm captures keyboard events
2. **Mode Translation**: Insert mode Enter → `Action::Submit`
3. **Message Creation**: `StateCommand::AddUserMessage` with UUID
4. **Chat History Update**: Message added to conversation tree
5. **Context Assembly**: 
   - `StateCommand::EmbedMessage` triggers RAG pipeline
   - Vector search + BM25 search for relevant code
   - Context assembled into system messages
6. **LLM Request**: `AppEvent::Llm(Event::Request)` sent to LLM manager
7. **API Execution**: RequestSession makes HTTP call to OpenRouter
8. **Response Processing**: Tool calls executed, response added to chat
9. **UI Refresh**: `AppEvent::MessageUpdated` triggers UI redraw

**Key Async Boundaries**:
- UI thread → State manager (mpsc channel)
- State manager → LLM manager (broadcast event bus)
- Tool execution (spawned tasks with result channels)

## 2. Model Selection Flow

### Command-Driven Model Switch
```
/model use <alias> → Command Parse → StateCommand::SwitchModel → 
Registry Validation → State Update → SystemEvent::ModelSwitched → UI Update
```

### Interactive Model Selection
```
/model search <keyword> → OpenRouter API → Model Browser Overlay →
User Selection → Provider Endpoints API → StateCommand::SelectModelProvider →
Configuration Update → ModelSwitched Event → Status Line Update
```

**Validation Steps**:
1. **Alias Resolution**: Check aliases HashMap
2. **Provider Existence**: Verify in registry
3. **Strictness Policy**: Enforce OpenRouter/Custom/Any policy
4. **Tool Requirements**: Optional filtering by tool support

## 3. App Startup Event Sequence

### Critical Path Events
```
1. Config Loading (TOML + Environment)
2. Registry Merging (User + Defaults)
3. OpenRouter Refresh (Capabilities Cache)
4. Database Initialization (CozoDB + Schemas)
5. Service Spawning (Event Bus, State Manager, LLM Manager)
6. UI Initialization (Terminal + App State)
7. EventBusStarted → Ready State
```

**Parallel Initialization**:
- File Manager, Observability, and BM25 service spawn concurrently
- RAG service initializes with fallback to degraded mode
- Embedding processor configured but not loaded until first use

## 4. Event Priority Architecture

### Realtime Events (UI Thread Priority)
- `AppEvent::Ui(*)` - User interactions
- `AppEvent::MessageUpdated` - Chat history changes
- `AppEvent::System(SystemEvent::ModelSwitched)` - Model changes
- `AppEvent::LlmTool(ToolEvent::Completed|Failed)` - Tool completions
- `AppEvent::IndexingCompleted|Failed` - Indexing status
- `AppEvent::ModelSearchResults` - Search results

### Background Events (Async Processing)
- `AppEvent::Llm(Event::Request)` - LLM requests
- `AppEvent::LlmTool(ToolEvent::Requested)` - Tool initiation
- `AppEvent::System(SystemEvent::ToolCallRequested)` - System tool calls
- `AppEvent::ModelsEndpointsRequest` - Provider queries
- `AppEvent::IndexingProgress` - Progress updates
- `AppEvent::Rag(*)` - RAG context events

**Routing Mechanism**:
```rust
impl AppEvent {
    pub fn priority(&self) -> EventPriority {
        match self {
            AppEvent::Ui(_) => EventPriority::Realtime,
            AppEvent::System(SystemEvent::ModelSwitched(_)) => EventPriority::Realtime,
            AppEvent::LlmTool(ev) => match ev {
                ToolEvent::Completed { .. } | ToolEvent::Failed { .. } => 
                    EventPriority::Realtime,
                ToolEvent::Requested { .. } => EventPriority::Background,
            },
            // ... other patterns
        }
    }
}
```

## 5. Error Handling Flows

### Recoverable Errors
```
Error Detection → ErrorEvent Creation → Global Event Bus →
AppEvent::Error → Chat Message → User Notification
```

**Common Recoverable Errors**:
- Unknown model selection → Warning message in chat
- OpenRouter API failure → Degraded capabilities, continue with cache
- Tool execution failure → Error result fed back to LLM
- Configuration load failure → Fall back to defaults

### Fatal Errors
```
Critical Failure → Panic Hook → Terminal Restoration → Process Exit
```

**Fatal Error Conditions**:
- Database initialization failure
- Terminal setup failure
- Core service spawn failure

## 6. Tool Execution Flow

### LLM Tool Call Processing
```
LLM Response with Tool Calls → Tool Validation → Context Assembly →
Tool Execution (Async) → Result Serialization → Message History Update →
LLM Continuation Request
```

**Tool Types & Context**:
- **`request_code_context`**: RAG search → code snippets
- **`get_file_metadata`**: File system → tracking hash + metadata  
- **`code_edit`**: File operations → diff preview → approval workflow

**Execution Context**:
```rust
pub struct Ctx {
    pub state: Arc<AppState>,
    pub event_bus: Arc<EventBus>, 
    pub parent_id: Uuid,
    pub call_id: String,
}
```

## 7. Configuration Change Propagation

### Runtime Configuration Update Flow
```
Command Execution → State Lock Acquisition → Configuration Mutation →
Event Emission → Service Notification → UI Refresh
```

**Examples**:
- **Model Switch**: Registry update → ModelSwitched → Status line
- **Strictness Change**: Policy update → No immediate UI change
- **Config Load**: Full replacement → Multiple service notifications
- **API Key Update**: Silent update → Available for next request

## 8. Indexing and RAG Event Flow

### Crate Indexing Sequence
```
/index start <path> → IndexerTask Spawn → File Discovery →
AST Parsing → Embedding Generation → Database Storage →
BM25 Index Update → IndexingCompleted Event → UI Notification
```

**Progress Events**:
- `IndexingStarted` → Progress indicator appears
- `IndexingProgress(status)` → Progress bar updates
- `IndexingCompleted` → Progress indicator disappears + success message
- `IndexingFailed` → Error message in chat

### RAG Context Request
```
User Query → EmbedMessage Command → Vector Embedding →
Hybrid Search (Vector + BM25) → Context Assembly →
System Message Creation → LLM Request Enhancement
```

## 9. Model Browser UI Flow

### Search and Selection Sequence
```
model search <keyword> → Immediate Overlay Open → 
Background API Call → ModelSearchResults Event →
UI Population → User Navigation → Endpoint Loading →
ModelsEndpointsRequest → Provider Results → Selection Confirmation →
SelectModelProvider Command → Model Switch
```

**UI State Management**:
- **Loading States**: Spinners during API calls
- **Expandable Items**: Provider details loaded on-demand
- **Selection Confirmation**: Automatic overlay close on selection
- **Error States**: API failures shown inline

## 10. Performance and Scalability Flows

### Backpressure Handling
```
High Event Volume → Channel Capacity Limits → Backpressure →
Sender Blocking → Flow Control → System Stability
```

**Channel Capacities**:
- StateCommand channel: 1024 messages
- AppEvent channels: Unbounded broadcast (with lagging receiver protection)
- File Manager events: 256 messages
- RAG events: 10 messages

### Memory Management Patterns
```
Arc<AppState> Sharing → Clone on Task Spawn →
Minimal Lock Duration → Automatic Cleanup →
Bounded Buffer Sizes → Stable Memory Usage
```

**Optimization Strategies**:
- ArcStr for frequently shared strings
- Iterator patterns to avoid intermediate collections
- Streaming APIs for large data processing
- Connection pooling for HTTP clients

## 11. Development and Debugging Flows

### Tracing Integration
```
Event Trigger → Span Creation → Structured Logging →
Request Correlation → Performance Metrics → Debug Context
```

**Key Tracing Points**:
- Request sessions with correlation IDs
- Tool execution with timing
- Configuration changes with before/after state
- API calls with request/response details

### Test Harness Integration  
```
Test Setup → Mock Services → Event Injection →
State Validation → Assertion Framework → Cleanup
```

**Test Categories**:
- Unit: Individual command parsing and validation
- Integration: State manager command processing  
- End-to-end: Full user interaction flows
- Live API: Real OpenRouter integration tests

This summary provides a comprehensive overview of how events flow through the Ploke TUI system, enabling both understanding of current behavior and planning for future enhancements.