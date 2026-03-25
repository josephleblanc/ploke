# Milestone M.5 planning: Expand into `ploke-tui` app commands (A.5–A.6)

**Date:** 2026-03-25  
**PRIMARY_TASK_SPEC milestone:** **M.5** (sections A.5–A.6; see [PRIMARY_TASK_SPEC.md](../PRIMARY_TASK_SPEC.md))
**Entry pointer:** [m5-planning.md](../m5-planning.md) (this file’s topic; body lives here)  
**Filename note:** This document is stored as `m6-planning.md` to avoid breaking existing links; treat **M.5** as the spec milestone name.  
**Status:** Planning Phase  
**Depends On:** M.4 (full implementation of A.1–A.4 in `xtask`)

---

## Executive Summary

This document provides detailed planning for implementing the `ploke-tui` application commands (sections A.5–A.6 from the main specification). These commands differ significantly from A.1–A.4 because they:

1. **Require heavy dependencies** from `ploke-tui` (ratatui, crossterm, etc.)
2. **Need complex actor setup** (EventBus, AppState, LLM managers)
3. **Operate in async context** with Tokio runtime
4. **Require headless UI testing** infrastructure

---

## A.5 Headless TUI Commands

### Overview

Commands to run `ploke-tui::App` in a headless mode using `ratatui::backend::TestBackend` for automated testing and agent interaction.

### A.5.1 Command: `tui headless`

**Purpose:** Initialize and run the TUI application in headless mode.

#### Command Specification

```
Usage: cargo xtask tui headless [OPTIONS]

Options:
  --config <path>          Path to config file (default: use default config)
  --workspace <path>       Workspace root to load (default: current directory)
  --fixture <id>           Load a fixture database before starting
  --timeout <secs>         Global timeout for headless operation (default: 300)
  --event-log <path>       Log all events to file for debugging
  --no-llm                 Start without LLM manager (tools only mode)
  --port <number>          Port for headless control API (default: 0 = auto)
  
Output:
  JSON with headless session info:
  {
    "session_id": "uuid",
    "control_port": 12345,
    "event_stream_url": "ws://localhost:12345/events",
    "status": "running"
  }
```

#### Implementation Details

**Required Types from `ploke_tui`:**

| Type | Path | Purpose |
|------|------|---------|
| `App` | `ploke_tui::app::App` | Main TUI application |
| `AppState` | `ploke_tui::app_state::AppState` | Shared state container |
| `EventBus` | `ploke_tui::event_bus::EventBus` | Event broadcast system |
| `StateCommand` | `ploke_tui::app_state::commands::StateCommand` | State manager commands |
| `RunOptions` | `ploke_tui::app::RunOptions` | App run configuration |
| `TestBackend` | `ratatui::backend::TestBackend` | Headless terminal backend |

**Actor Setup Required:**

```rust
// 1. Database (Arc<Database>)
let db = Arc::new(Database::init_with_schema()?);
db.setup_multi_embedding()?;

// 2. Embedding Runtime (Arc<EmbeddingRuntime>)
let processor = config.load_embedding_processor()?;
let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
    Arc::clone(&db.active_embedding_set),
    processor,
));

// 3. IO Manager (IoManagerHandle)
let io_handle = IoManagerHandle::new();

// 4. EventBus (Arc<EventBus>)
let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

// 5. BM25 Service
let bm25_cmd = bm25_index::bm25_service::start(Arc::clone(&db), 0.0)?;

// 6. Indexer Task (optional)
let indexer_task = IndexerTask::new(...).with_bm25_tx(bm25_cmd);

// 7. RAG Service (optional but recommended)
let rag = RagService::new_full(db.clone(), embedder.clone(), io_handle.clone(), config)?;

// 8. AppState
let state = Arc::new(AppState {
    chat: ChatState::new(ChatHistory::new()),
    config: ConfigState::new(runtime_cfg),
    system: SystemState::new(SystemStatus::new(Some(workspace_root))),
    indexing_state: RwLock::new(None),
    indexer_task: Some(Arc::clone(&indexer_task)),
    indexing_control: Arc::new(Mutex::new(None)),
    db,
    embedder,
    io_handle,
    proposals: RwLock::new(HashMap::new()),
    create_proposals: RwLock::new(HashMap::new()),
    rag: Some(Arc::new(rag)),
    budget: TokenBudget::default(),
});

// 9. Start state manager actor
let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(1024);
tokio::spawn(state_manager(state.clone(), cmd_rx, event_bus.clone(), rag_event_tx));

// 10. Start LLM manager actor (unless --no-llm)
tokio::spawn(llm_manager(
    event_bus.subscribe(EventPriority::Realtime),
    event_bus.subscribe(EventPriority::Background),
    state.clone(),
    cmd_tx.clone(),
    event_bus.clone(),
    cancel_rx,
));

// 11. Create synthetic input channel
let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
let input = UnboundedReceiverStream::new(input_rx);

// 12. Create TestBackend terminal
let backend = TestBackend::new(80, 24);
let terminal = Terminal::new(backend)?;

// 13. Create and run App
let app = App::new(
    config.command_style,
    state.clone(),
    cmd_tx,
    &event_bus,
    default_model(),
    tool_verbosity,
    cancel_tx,
);

// Run in background task
let app_handle = tokio::spawn(async move {
    app.run_with(terminal, input, RunOptions { setup_terminal_modes: false }).await
});
```

**Error Handling:**

| Error | Recovery Path |
|-------|---------------|
| Database init fails | Check disk space, permissions; suggest `cargo xtask setup test-env` |
| Config load fails | Suggest using `--config` with valid path or use defaults |
| Port already in use | Suggest different `--port` or auto-assign |
| Actor spawn fails | Log error, shutdown gracefully, check resource limits |

---

### A.5.2 Command: `tui input`

**Purpose:** Send text input to a running headless TUI and wait for response.

#### Command Specification

```
Usage: cargo xtask tui input [OPTIONS] --session <id> <text>

Arguments:
  <text>                   Text to send as input

Options:
  --session <id>           Session ID from `tui headless` (required)
  --timeout <secs>         Max time to wait for response (default: 30)
  --wait-for <event>       Wait for specific event instead of timeout
  --event-types <list>     Event types to capture (default: Llm,LlmTool,System)
  --output-format <fmt>    Output format: json, text, events (default: text)
  
Output (text format):
  [Response content from TUI]
  
Output (json format):
  {
    "input": "original input",
    "response": "TUI response text",
    "events_captured": [...],
    "response_time_ms": 1234,
    "timed_out": false
  }
  
Output (events format):
  [List of all events captured during wait period]
```

#### Implementation Details

**Key Mechanism:**

1. Connect to headless session's control port
2. Send text as synthetic key events (character-by-character)
3. Send Enter key event
4. Subscribe to relevant AppEvent channels
5. Wait for:
   - Specified event type (if `--wait-for` used)
   - Timeout (default 30s)
   - Response indication from TUI

**Event Subscription Strategy:**

```rust
// Subscribe to both realtime and background events
let mut realtime_rx = event_bus.subscribe(EventPriority::Realtime);
let mut background_rx = event_bus.subscribe(EventPriority::Background);

// Also subscribe to specific tool events if needed
let mut tool_events: Vec<AppEvent> = Vec::new();

// Send input character by character
for ch in text.chars() {
    input_tx.send(Ok(Event::Key(KeyEvent::from(KeyCode::Char(ch)))))
        .map_err(|_| "Input channel closed")?;
}

// Send Enter
input_tx.send(Ok(Event::Key(KeyEvent::from(KeyCode::Enter))))
    .map_err(|_| "Input channel closed")?;

// Collect events until condition met or timeout
let result = tokio::time::timeout(Duration::from_secs(timeout), async {
    loop {
        tokio::select! {
            Ok(event) = realtime_rx.recv() => {
                if should_capture(&event, &event_types) {
                    tool_events.push(event.clone());
                }
                if matches_target(&event, wait_for) {
                    break Ok(event);
                }
            }
            Ok(event) = background_rx.recv() => {
                if should_capture(&event, &event_types) {
                    tool_events.push(event.clone());
                }
            }
            else => break Err("Event channels closed"),
        }
    }
}).await;
```

**Events to Capture:**

| Event Type | When Captured | Information |
|------------|---------------|-------------|
| `AppEvent::Llm(llm_event)` | Always | LLM request/response |
| `AppEvent::LlmTool(tool_event)` | Always | Tool call request/completion |
| `AppEvent::System(SystemEvent::ToolCallCompleted)` | With `--wait-for tool` | Tool completion |
| `AppEvent::MessageUpdated` | With `--wait-for message` | Chat message update |
| `AppEvent::Error` | Always | Error events |

---

### A.5.3 Command: `tui key`

**Purpose:** Send specific key codes and key combinations to the headless TUI.

#### Command Specification

```
Usage: cargo xtask tui key [OPTIONS] --session <id> <key>

Arguments:
  <key>                    Key to send (see KEY SYNTAX below)

Options:
  --session <id>           Session ID from `tui headless` (required)
  --repeat <n>             Repeat key press n times (default: 1)
  --timeout <secs>         Max time to wait for response (default: 5)
  
Key Syntax:
  <char>                   Single character (e.g., 'a', '1', '/')
  <Esc>                    Escape key
  <Enter>                  Enter/Return key
  <Tab>                    Tab key
  <Backspace>              Backspace key
  <Space>                  Space key
  <Up>, <Down>, <Left>, <Right>  Arrow keys
  <Home>, <End>            Home/End keys
  <PageUp>, <PageDown>     Page keys
  <F1>-<F12>               Function keys
  Ctrl+<key>               Control+key combination
  Alt+<key>                Alt+key combination
  Shift+<key>              Shift+key combination
  
Examples:
  cargo xtask tui key --session abc123 <Esc>
  cargo xtask tui key --session abc123 Ctrl+f
  cargo xtask tui key --session abc123 <Enter> --repeat 3
```

#### Implementation Details

**Key Parsing and Mapping:**

```rust
pub enum KeySpec {
    Char(char),
    Escape,
    Enter,
    Tab,
    Backspace,
    Space,
    Up, Down, Left, Right,
    Home, End,
    PageUp, PageDown,
    F(u8), // F1-F12
    Ctrl(Box<KeySpec>),
    Alt(Box<KeySpec>),
    Shift(Box<KeySpec>),
}

impl KeySpec {
    pub fn parse(input: &str) -> Result<Self, String> {
        // Parse key specification from string
        // Handle special syntax like <Esc>, Ctrl+f, etc.
    }
    
    pub fn to_crossterm_event(&self) -> Event {
        match self {
            KeySpec::Char(c) => Event::Key(KeyEvent::from(KeyCode::Char(*c))),
            KeySpec::Escape => Event::Key(KeyEvent::from(KeyCode::Esc)),
            KeySpec::Enter => Event::Key(KeyEvent::from(KeyCode::Enter)),
            KeySpec::Ctrl(inner) => {
                let inner_event = inner.to_crossterm_event();
                if let Event::Key(key) = inner_event {
                    Event::Key(KeyEvent::new(key.code, KeyModifiers::CONTROL))
                } else {
                    inner_event
                }
            }
            // ... other mappings
        }
    }
}
```

---

### A.5.4 Command: `tui shutdown`

**Purpose:** Gracefully shut down a running headless TUI session.

#### Command Specification

```
Usage: cargo xtask tui shutdown [OPTIONS] --session <id>

Options:
  --session <id>           Session ID from `tui headless` (required)
  --force                  Force shutdown (send SIGTERM instead of graceful)
  --timeout <secs>         Wait time for graceful shutdown (default: 10)
  
Output:
  {
    "session_id": "abc123",
    "shutdown_status": "graceful|forced|timeout",
    "cleanup_successful": true
  }
```

#### Implementation Details

**Graceful Shutdown Sequence:**

```rust
// 1. Send Quit command through input channel
input_tx.send(Ok(Event::Key(KeyEvent::from(KeyCode::Char('q')))));

// OR send Quit event directly through EventBus
event_bus.send(AppEvent::Quit);

// 2. Wait for app task to complete with timeout
match tokio::time::timeout(Duration::from_secs(timeout), app_handle).await {
    Ok(Ok(())) => ShutdownStatus::Graceful,
    Ok(Err(e)) => ShutdownStatus::Error(e.to_string()),
    Err(_) => {
        // Timeout - force shutdown
        app_handle.abort();
        ShutdownStatus::Forced
    }
}

// 3. Cleanup resources
// - Database connections
// - File handles
// - Temporary files
```

---

## A.6 Tool Execution Commands

### Overview

Commands to execute tools from `ploke-tui` directly, bypassing the LLM tool call loop. These are useful for testing tools and for agents that need direct access to tool functionality.

### A.6.1 Tool Selection and Prioritization

Based on survey results, the following tools are prioritized for xtask commands:

| Priority | Tool | Purpose | Complexity |
|----------|------|---------|------------|
| P1 | `NsRead` | Read files outside semantic graph | Low |
| P1 | `ListDir` | Safe directory listing | Low |
| P1 | `CodeItemLookup` | Look up code items by path | Medium |
| P2 | `CodeItemEdges` | Get relationship edges | Medium |
| P2 | `CargoTool` | Run cargo check/test | Medium |
| P3 | `RequestCodeContextGat` | Hybrid search (needs RAG) | High |
| P3 | `GatCodeEdit` | Apply code edits | High |
| P4 | `CreateFile` | Create new files | Medium |
| P4 | `NsPatch` | Apply patches | Medium |

---

### A.6.2 Command: `tool ns-read`

**Purpose:** Execute the `NsRead` tool to read file contents.

#### Command Specification

```
Usage: cargo xtask tool ns-read [OPTIONS] --file <path> [ARGS]

Arguments via command line:
  --file <path>            File path to read (required)
  --start-line <n>         Start line (1-based, inclusive)
  --end-line <n>           End line (1-based, inclusive)
  --max-bytes <n>          Maximum bytes to read (default: 32768)
  
Arguments via JSON:
  --json <string>          JSON arguments as string
  --json-file <path>       Path to JSON file containing arguments
  --stdin                  Read JSON from stdin
  
Output Options:
  --format <fmt>           Output format: json, pretty, raw (default: pretty)
  --include-hash           Include file hash in output
  
Output (json format):
  {
    "ok": true,
    "file_path": "src/main.rs",
    "exists": true,
    "byte_len": 1234,
    "start_line": 1,
    "end_line": 50,
    "truncated": false,
    "content": "...file contents...",
    "file_hash": "abc123..."
  }
```

#### Implementation Details

**Minimal Actor Setup for Tool Execution:**

```rust
// Unlike tui headless, tools need minimal setup
pub async fn execute_tool_ns_read(args: NsReadArgs) -> Result<ToolResult, XtaskError> {
    // 1. Initialize database (lightweight)
    let db = Arc::new(Database::init_with_schema()?);
    
    // 2. Create minimal embedder (can be mock)
    let processor = EmbeddingProcessor::new_mock();
    let embedder = Arc::new(EmbeddingRuntime::with_default_set(processor));
    
    // 3. Create IO handle
    let io_handle = IoManagerHandle::new();
    
    // 4. Create event bus
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    
    // 5. Create minimal AppState
    let state = Arc::new(AppState {
        chat: ChatState::new(ChatHistory::new()),
        config: ConfigState::new(RuntimeConfig::default()),
        system: SystemState::new(SystemStatus::new(Some(workspace_root))),
        indexing_state: RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(Mutex::new(None)),
        db,
        embedder,
        io_handle,
        proposals: RwLock::new(HashMap::new()),
        create_proposals: RwLock::new(HashMap::new()),
        rag: None, // NsRead doesn't need RAG
        budget: TokenBudget::default(),
    });
    
    // 6. Create tool context
    let ctx = Ctx {
        state,
        event_bus,
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        call_id: ArcStr::from("xtask-ns-read"),
    };
    
    // 7. Deserialize parameters
    let params_json = serde_json::to_string(&args)?;
    let params = NsRead::deserialize_params(&params_json)?;
    
    // 8. Execute tool
    let result = NsRead::execute(params, ctx).await?;
    
    Ok(result)
}
```

---

### A.6.3 Command: `tool code-lookup`

**Purpose:** Execute the `CodeItemLookup` tool to find code items in the semantic graph.

#### Command Specification

```
Usage: cargo xtask tool code-lookup [OPTIONS]

Arguments via command line:
  --item-name <name>       Item name to look up (required)
  --file-path <path>       File path containing the item (required)
  --node-kind <kind>       Node kind: function|const|static|enum|struct|trait|impl|type|macro (required)
  --module-path <path>     Module path, e.g., "crate::module::sub" (required)
  
Arguments via JSON:
  --json <string>          JSON arguments as string
  --json-file <path>       Path to JSON file containing arguments
  --stdin                  Read JSON from stdin
  
Database Options:
  --db <path>              Path to database (default: ephemeral)
  --fixture <id>           Load fixture database before lookup
  
Output (json format):
  {
    "ok": true,
    "results": [
      {
        "file_path": "src/lib.rs",
        "canon_path": "crate::module::MyStruct",
        "snippet": "...code snippet..."
      }
    ],
    "count": 1
  }
```

#### Implementation Details

**Additional Requirements:**
- Database must contain indexed code graph
- Workspace must be loaded for path resolution
- Tool validates `node_kind` against allowed values

---

### A.6.4 Command: `tool code-edges`

**Purpose:** Execute the `CodeItemEdges` tool to get relationship edges for a code item.

#### Command Specification

```
Usage: cargo xtask tool code-edges [OPTIONS]

Arguments via command line:
  --item-name <name>       Item name (required)
  --file-path <path>       File path (required)
  --node-kind <kind>       Node kind (required)
  --module-path <path>     Module path (required)
  --edge-types <list>      Filter by edge types: calls,impls,contains,uses (default: all)
  
Output (json format):
  {
    "ok": true,
    "node_info": {
      "file_path": "src/lib.rs",
      "canon_path": "crate::module::my_func",
      "snippet": "..."
    },
    "edges": [
      {
        "kind": "calls",
        "target": "crate::other::target_func",
        "file_path": "src/other.rs"
      }
    ]
  }
```

---

### A.6.5 Command: `tool cargo`

**Purpose:** Execute the `CargoTool` to run cargo commands with JSON diagnostics.

#### Command Specification

```
Usage: cargo xtask tool cargo [OPTIONS] <command>

Arguments:
  <command>                Cargo command: check, test, build

Options:
  --scope <scope>          Scope: focused (default), workspace
  --package <name>         Package to target
  --features <list>        Features to enable (comma-separated)
  --all-features           Enable all features
  --no-default-features    Disable default features
  --target <triple>        Target triple
  --release                Release mode
  --lib                    Build/test lib only
  --tests                  Build/test tests only
  --test-args <args>       Additional test arguments
  
Output (json format):
  {
    "ok": true,
    "status_reason": "Success",
    "command": "Check",
    "scope": "Focused",
    "manifest_path": "/path/to/Cargo.toml",
    "exit_code": 0,
    "duration_ms": 1234,
    "summary": {
      "passed": 10,
      "failed": 0,
      "ignored": 2
    },
    "diagnostics": [...],
    "stderr_tail": [...]
  }
```

---

### A.6.6 Command: `tool list-dir`

**Purpose:** Execute the `ListDir` tool for safe directory listing.

#### Command Specification

```
Usage: cargo xtask tool list-dir [OPTIONS] <dir>

Arguments:
  <dir>                    Directory path to list

Options:
  --include-hidden         Include hidden files/directories
  --sort <by>              Sort by: name, mtime, size, none (default: name)
  --max-entries <n>        Maximum entries to return (default: 1000)
  
Output (json format):
  {
    "ok": true,
    "dir": "/path/to/dir",
    "exists": true,
    "truncated": false,
    "entries": [
      {
        "name": "src",
        "path": "/path/to/dir/src",
        "kind": "dir",
        "size_bytes": null,
        "modified_ms": 1234567890
      },
      {
        "name": "Cargo.toml",
        "path": "/path/to/dir/Cargo.toml",
        "kind": "file",
        "size_bytes": 1234,
        "modified_ms": 1234567890
      }
    ]
  }
```

---

### A.6.7 JSON Input Methods

All tool commands support three JSON input methods:

#### Method 1: Command-line Arguments (Preferred for simple cases)

```bash
cargo xtask tool ns-read --file src/main.rs --start-line 1 --end-line 50
```

#### Method 2: JSON String

```bash
cargo xtask tool ns-read --json '{"file": "src/main.rs", "start_line": 1, "end_line": 50}'
```

#### Method 3: JSON File

```bash
cargo xtask tool ns-read --json-file input.json
```

Where `input.json` contains:
```json
{
  "file": "src/main.rs",
  "start_line": 1,
  "end_line": 50,
  "max_bytes": 1024
}
```

#### Method 4: Stdin

```bash
cat input.json | cargo xtask tool ns-read --stdin
```

---

## Module Structure for M.5

### Proposed File Organization

```
xtask/src/
├── main.rs                      # CLI entry point
├── lib.rs                       # Public exports
├── error.rs                     # XtaskError (from M.3)
├── context.rs                   # CommandContext (from M.3)
├── commands/
│   ├── mod.rs                   # Common command traits
│   ├── parse.rs                 # A.1 commands (from M.4)
│   ├── transform.rs             # A.2 commands (from M.4)
│   ├── ingest.rs                # A.3 commands (from M.4)
│   ├── db.rs                    # A.4 commands (from M.4)
│   ├── tui.rs                   # A.5 commands (this milestone)
│   └── tool.rs                  # A.6 commands (this milestone)
├── tui_harness/                 # NEW: Headless TUI support
│   ├── mod.rs                   # Public exports
│   ├── harness.rs               # HeadlessTuiHarness struct
│   ├── session.rs               # Session management
│   ├── input.rs                 # Input simulation
│   ├── events.rs                # Event capture
│   └── shutdown.rs              # Graceful shutdown
├── tool_executor/               # NEW: Direct tool execution
│   ├── mod.rs                   # Public exports
│   ├── context.rs               # Minimal tool context builder
│   ├── ns_read.rs               # NsRead tool command
│   ├── code_lookup.rs           # CodeItemLookup tool command
│   ├── code_edges.rs            # CodeItemEdges tool command
│   ├── cargo.rs                 # CargoTool command
│   ├── list_dir.rs              # ListDir tool command
│   └── json_input.rs            # JSON input parsing utilities
└── test_harness.rs              # From M.3
```

---

## Dependencies

### Cargo.toml Additions Required

```toml
[dependencies]
# Existing dependencies from M.3/M.4
# ...

# NEW for M.5 - ploke-tui integration
ploke-tui = { path = "../crates/ploke-tui" }

# NEW for M.5 - ratatui for TestBackend
ratatui = { version = "0.29", features = ["backend-test"] }
crossterm = "0.28"

# NEW for M.5 - WebSocket for headless control API
tokio-tungstenite = "0.24"

# NEW for M.5 - UUID generation for sessions
uuid = { version = "1.11", features = ["v4", "serde"] }

# NEW for M.5 - ArcStr for tool call IDs
arcstr = "1.2"
```

### Feature Flags

Consider adding a feature flag to make `ploke-tui` dependencies optional:

```toml
[features]
default = ["tui-commands"]
tui-commands = ["dep:ploke-tui", "dep:ratatui", "dep:crossterm"]
```

---

## Success Criteria

### A.5 Headless TUI Criteria

| Criterion | Test Approach |
|-----------|---------------|
| `tui headless` starts successfully | Integration test with timeout |
| `tui input` sends text and receives response | Test with known input/output pair |
| `tui key` sends special keys correctly | Test mode changes (Esc, Ctrl+f) |
| `tui shutdown` terminates gracefully | Verify process exit, cleanup |
| Event capture works correctly | Subscribe to events, verify receipt |
| Timeout handling works | Test with deliberately slow operations |

### A.6 Tool Execution Criteria

| Criterion | Test Approach |
|-----------|---------------|
| `tool ns-read` reads files correctly | Compare output to known file content |
| `tool code-lookup` finds items correctly | Query known fixtures, verify results |
| `tool cargo` runs cargo commands | Execute `cargo check` on test crate |
| `tool list-dir` lists directories | Verify against `std::fs::read_dir` |
| JSON input methods all work | Test --json, --json-file, --stdin |
| Error messages are helpful | Test invalid inputs, verify error context |

---

## Test Strategies

### Unit Tests

Located in module files (e.g., `tool_executor/ns_read.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_ns_read_basic() {
        // To Prove: NsRead tool reads file correctly
        // Given: A test file with known content
        // When: tool ns-read is executed
        // Then: Output matches file content
    }
}
```

### Integration Tests

Located in `xtask/tests/integration/`:

```rust
// tests/integration/tui_tests.rs
#[tokio::test]
async fn test_headless_tui_lifecycle() {
    // 1. Start headless TUI
    // 2. Send input
    // 3. Verify response
    // 4. Shutdown gracefully
}
```

### Test Fixtures

| Fixture | Purpose | Source |
|---------|---------|--------|
| `fixture_test_crate` | Basic parsing/workspace tests | ploke-test-utils |
| `FIXTURE_NODES_CANONICAL` | Database with code graph | ploke-test-utils |
| `minimal_test_file.rs` | File read tests | Create in test temp dir |

---

## Implementation Phases

### Phase 1: Foundation (Week 1)

**Goal:** Get basic structure in place

| Task | Effort | Owner |
|------|--------|-------|
| Add ploke-tui dependency to Cargo.toml | 1h | Engineering |
| Create `tui_harness/` module structure | 2h | Engineering |
| Create `tool_executor/` module structure | 2h | Engineering |
| Implement `tool ns-read` (simplest tool) | 4h | Engineering |
| Write unit tests for ns-read | 2h | Engineering |

**Deliverable:** `cargo xtask tool ns-read` works end-to-end

### Phase 2: Tool Commands (Week 1-2)

**Goal:** Implement all P1 tool commands

| Task | Effort | Owner |
|------|--------|-------|
| Implement `tool list-dir` | 3h | Engineering |
| Implement `tool code-lookup` | 4h | Engineering |
| Implement `tool code-edges` | 3h | Engineering |
| Implement `tool cargo` | 4h | Engineering |
| Write integration tests for tools | 4h | Engineering |

**Deliverable:** All P1 tool commands work

### Phase 3: Headless TUI Foundation (Week 2)

**Goal:** Get `tui headless` working

| Task | Effort | Owner |
|------|--------|-------|
| Implement `HeadlessTuiHarness` | 6h | Engineering |
| Implement `tui headless` command | 4h | Engineering |
| Implement session management | 3h | Engineering |
| Write integration tests | 4h | Engineering |

**Deliverable:** `cargo xtask tui headless` starts and can be shut down

### Phase 4: Input/Key Commands (Week 3)

**Goal:** Complete A.5 commands

| Task | Effort | Owner |
|------|--------|-------|
| Implement `tui input` command | 4h | Engineering |
| Implement `tui key` command | 4h | Engineering |
| Implement event capture system | 4h | Engineering |
| Implement `tui shutdown` command | 2h | Engineering |
| Write comprehensive tests | 4h | Engineering |

**Deliverable:** Full A.5 command suite works

### Phase 5: Polish and Documentation (Week 3-4)

**Goal:** Production-ready implementation

| Task | Effort | Owner |
|------|--------|-------|
| Add comprehensive error handling | 4h | Engineering |
| Add tracing instrumentation | 2h | Engineering |
| Write command documentation | 3h | Documentation |
| Update help system | 2h | Engineering |
| Final integration testing | 4h | Testing |

**Deliverable:** M.5 complete, all tests pass

---

## Dependencies on M.4

### Hard Dependencies (M.5 blocked without these)

| M.4 Component | M.5 Usage | Impact if Missing |
|---------------|-----------|-------------------|
| `CommandContext` | Resource management for tools | Would need reimplementation |
| `XtaskError` | Error handling | Inconsistent error types |
| `Command` trait | Command dispatch | No unified command interface |
| Database initialization | Tool context setup | Tools can't access database |
| Output formatters | Tool output formatting | Inconsistent output |

### Soft Dependencies (M.5 can proceed with workarounds)

| M.4 Component | M.5 Usage | Workaround if Missing |
|---------------|-----------|----------------------|
| Usage tracking | Track tool usage | Skip statistics initially |
| `CommandExecutor` registry | Command dispatch | Manual match statement |
| Integration test harness | Test infrastructure | Write ad-hoc tests |

### Integration Points

```rust
// From M.4 - Command trait
pub trait Command: Send + Sync + 'static {
    type Output: Serialize;
    type Error: Into<XtaskError>;
    
    fn name(&self) -> &'static str;
    fn requires_async(&self) -> bool;
    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error>;
}

// M.5 adds TuiCommand and ToolCommand specializations
pub trait TuiCommand: Command {
    fn requires_headless_session(&self) -> bool;
    fn session_timeout(&self) -> Duration;
}

pub trait ToolCommand: Command {
    fn tool_name(&self) -> ToolName;
    fn json_schema(&self) -> &'static serde_json::Value;
}
```

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| ploke-tui dependency increases build time | High | Medium | Feature flag to make optional |
| TestBackend behavior differs from real TUI | Medium | High | Document limitations; add real TUI tests |
| Complex actor setup causes resource leaks | Medium | High | Implement robust cleanup; use RAII |
| Race conditions in event handling | Medium | High | Careful async design; comprehensive tests |
| Tool context requirements change | Low | Medium | Abstract context builder |
| Embedding runtime initialization slow | Medium | Low | Allow mock embedder; cache instances |

---

## Documentation Requirements

### Code Documentation

- All public types and functions must have doc comments
- Complex async functions need examples
- Error types need recovery hints

### User Documentation

- Update `cargo xtask help` with new commands
- Add tool-specific help (e.g., `cargo xtask tool ns-read --help`)
- Document JSON schemas for tool inputs
- Provide example commands

### Agent Documentation

- Create `docs/active/agents/2026-03-24-coordination/tui-commands-guide.md`
- Document headless TUI usage patterns
- Provide troubleshooting guide

---

## Related Documents

| Document | Purpose |
|----------|---------|
| `../PRIMARY_TASK_SPEC.md` | Main specification (A.5-A.6) |
| `../sub-agents/survey-ploke_tui.md` | TUI survey results |
| `architecture-decision.md` | Selected architecture |
| `test-design-requirements.md` | Test design for all commands |
| `../2026-03-25-command-matrix.md` | Command-function mapping |

---

## Open Questions

1. **Should we implement a WebSocket API for headless control?** Would enable external control but adds complexity.

2. **Should tool commands support batch execution?** E.g., `cargo xtask tool batch --file tools.json`.

3. **How should we handle tool events that require user confirmation?** Some tools (like `GatCodeEdit`) may prompt for confirmation.

4. **Should we persist headless sessions across xtask invocations?** Or is each command self-contained?

---

**Planning Document Version:** 1.0  
**Last Updated:** 2026-03-25  
**Planned Start:** After M.4 completion  
**Estimated Duration:** 3-4 weeks  
