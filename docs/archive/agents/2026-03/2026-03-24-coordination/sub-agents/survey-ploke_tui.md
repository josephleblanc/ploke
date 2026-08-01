# Survey: ploke_tui Crate

**Date:** 2026-03-25  
**Task:** M.1 Survey for xtask Commands Feature (A.5 Headless TUI Commands, A.6 Tool Call Commands)  
**Sub-Agent:** ploke_tui Survey

---

## Files Touched During Survey

1. `/home/brasides/code/ploke/crates/ploke-tui/src/lib.rs` - Main library entry, `try_main()`, `AppEvent`
2. `/home/brasides/code/ploke/crates/ploke-tui/src/main.rs` - Application entry point
3. `/home/brasides/code/ploke/crates/ploke-tui/src/app/mod.rs` - `App` struct, `run_with()`, `RunOptions`
4. `/home/brasides/code/ploke/crates/ploke-tui/src/app/events.rs` - Event handling
5. `/home/brasides/code/ploke/crates/ploke-tui/src/test_harness.rs` - `TEST_APP`, `AppHarness`
6. `/home/brasides/code/ploke/crates/ploke-tui/src/test_utils/mock.rs` - Mock app/state creation
7. `/home/brasides/code/ploke/crates/ploke-tui/src/test_utils/new_test_harness.rs` - `AppHarness` implementation
8. `/home/brasides/code/ploke/crates/ploke-tui/src/event_bus/mod.rs` - `EventBus`, broadcast channels
9. `/home/brasides/code/ploke/crates/ploke-tui/src/app_state/mod.rs` - AppState exports
10. `/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs` - `AppState`, `SystemState`, `RuntimeConfig`
11. `/home/brasides/code/ploke/crates/ploke-tui/src/app_state/commands.rs` - `StateCommand` enum
12. `/home/brasides/code/ploke/crates/ploke-tui/src/tools/mod.rs` - `Tool` trait, `process_tool()`, `Ctx`
13. `/home/brasides/code/ploke/crates/ploke-tui/src/tools/ns_read.rs` - `NsRead` tool
14. `/home/brasides/code/ploke/crates/ploke-tui/src/tools/code_item_lookup.rs` - `CodeItemLookup` tool
15. `/home/brasides/code/ploke/crates/ploke-tui/src/tools/get_code_edges.rs` - `CodeItemEdges` tool
16. `/home/brasides/code/ploke/crates/ploke-tui/src/tools/request_code_context.rs` - `RequestCodeContextGat` tool
17. `/home/brasides/code/ploke/crates/ploke-tui/src/tools/code_edit.rs` - `GatCodeEdit` tool
18. `/home/brasides/code/ploke/crates/ploke-tui/src/tools/create_file.rs` - `CreateFile` tool
19. `/home/brasides/code/ploke/crates/ploke-tui/src/tools/ns_patch.rs` - `NsPatch` tool
20. `/home/brasides/code/ploke/crates/ploke-tui/src/tools/list_dir.rs` - `ListDir` tool
21. `/home/brasides/code/ploke/crates/ploke-tui/src/tools/cargo.rs` - `CargoTool` tool
22. `/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs` - RAG tool implementations

---

## A.5 Headless TUI Commands

### A.5.1 App Struct with TestBackend

#### `App` Struct
**Path:** `ploke_tui::app::App`

```rust
#[derive(Debug)]
pub struct App {
    running: bool,
    list: ListState,
    state: Arc<AppState>,
    cmd_tx: mpsc::Sender<StateCommand>,
    event_rx: tokio::sync::broadcast::Receiver<AppEvent>,
    bg_event_rx: tokio::sync::broadcast::Receiver<AppEvent>,
    pub input_buffer: String,
    pub mode: Mode,
    command_style: CommandStyle,
    indexing_state: Option<indexer::IndexingStatus>,
    conversation: ConversationView,
    input_view: InputView,
    // ... additional fields
}
```

**Key Types/Structs Needed:**
- `AppState` - Shared application state (db, embedder, io_handle, rag, etc.)
- `StateCommand` - Commands sent to state manager
- `AppEvent` - Events broadcast via event bus
- `Mode` - Input mode (Normal, Insert, Command)
- `CommandStyle` - Slash vs Colon command style

#### `App::new()` Constructor
**Path:** `ploke_tui::app::App::new`

```rust
pub fn new(
    command_style: CommandStyle,
    state: Arc<AppState>,
    cmd_tx: mpsc::Sender<StateCommand>,
    event_bus: &EventBus,
    active_model_id: String,
    tool_verbosity: ToolVerbosity,
    cancel_tx: watch::Sender<CancelChatToken>,
) -> Self
```

**Special Considerations:**
- Requires `EventBus` for event subscriptions
- Needs `AppState` with properly initialized subsystems
- `cmd_tx` channel for sending commands to state manager

#### `App::run_with()` - Headless Entry Point
**Path:** `ploke_tui::app::App::run_with`

```rust
pub async fn run_with<B, S>(
    mut self,
    mut terminal: ratatui::Terminal<B>,
    mut input: S,
    opts: RunOptions,
) -> Result<()>
where
    B: ratatui::backend::Backend,
    S: futures::Stream<Item = std::result::Result<crossterm::event::Event, std::io::Error>> + Unpin,
```

**Input Parameters:**
- `terminal` - Generic terminal with any `Backend` (use `TestBackend` for headless)
- `input` - Stream of crossterm events (can be synthetic for testing)
- `opts` - `RunOptions` for terminal mode configuration

**Output/Return:** `color_eyre::Result<()>`

**Error Types:** `color_eyre::Error`

**Special Considerations:**
- Use `RunOptions { setup_terminal_modes: false }` for headless testing
- Input stream can be `UnboundedReceiverStream` for synthetic events

#### `RunOptions` Struct
**Path:** `ploke_tui::app::RunOptions`

```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct RunOptions {
    pub setup_terminal_modes: bool,  // false for headless tests
}
```

### A.5.2 Simulating User Input and Key Presses

#### Using `TestBackend` with Synthetic Input

**Example from `test_utils/new_test_harness.rs`:**

```rust
use ratatui::{Terminal, backend::TestBackend};
use tokio_stream::wrappers::UnboundedReceiverStream;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

// Create synthetic input channel
let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel::<
    Result<crossterm::event::Event, std::io::Error>
>();
let input = UnboundedReceiverStream::new(input_rx);

// Create TestBackend terminal
let backend = TestBackend::new(80, 24);
let terminal = Terminal::new(backend).expect("terminal");

// Send synthetic key events
input_tx.send(Ok(Event::Key(KeyEvent::from(KeyCode::Char('q')))));
```

#### Key Event Types
**Path:** `crossterm::event::KeyEvent`

Common key events for simulation:
```rust
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

// Character input
Event::Key(KeyEvent::from(KeyCode::Char('a')))

// With modifier
Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))

// Special keys
Event::Key(KeyEvent::from(KeyCode::Enter))
Event::Key(KeyEvent::from(KeyCode::Esc))
Event::Key(KeyEvent::from(KeyCode::Backspace))
Event::Key(KeyEvent::from(KeyCode::Tab))
Event::Key(KeyEvent::from(KeyCode::Up))
Event::Key(KeyEvent::from(KeyCode::Down))
```

### A.5.3 Event Handling and Broadcast Channels

#### `EventBus` Struct
**Path:** `ploke_tui::event_bus::EventBus`

```rust
#[derive(Debug)]
pub struct EventBus {
    pub realtime_tx: broadcast::Sender<AppEvent>,
    pub background_tx: broadcast::Sender<AppEvent>,
    error_tx: broadcast::Sender<ErrorEvent>,
    pub index_tx: Arc<broadcast::Sender<indexer::IndexingStatus>>,
}
```

**Constructor:**
```rust
impl EventBus {
    pub fn new(b: EventBusCaps) -> Self
}
```

**EventBusCaps (default capacities):**
```rust
pub struct EventBusCaps {
    realtime_cap: usize,    // default: 100
    background_cap: usize,  // default: 1000
    error_cap: usize,       // default: 1000
    index_cap: usize,       // default: 1000
}
```

#### Event Bus Operations

```rust
impl EventBus {
    // Send event (auto-routed by priority)
    pub fn send(&self, event: AppEvent)
    
    // Subscribe to events
    pub fn subscribe(&self, priority: EventPriority) -> broadcast::Receiver<AppEvent>
    
    // Send error
    pub fn send_error(&self, message: String, severity: ErrorSeverity)
    
    // Index-specific subscriber
    pub fn index_subscriber(&self) -> broadcast::Receiver<indexer::IndexingStatus>
}
```

#### `EventPriority` Enum
**Path:** `ploke_tui::event_bus::EventPriority`

```rust
#[derive(Clone, Copy, Debug)]
pub enum EventPriority {
    Realtime,   // UI-critical events
    Background, // Non-urgent events
}
```

#### `AppEvent` Enum (Key Variants)
**Path:** `ploke_tui::AppEvent`

```rust
#[derive(Clone, Debug)]
pub enum AppEvent {
    Ui(UiEvent),
    Llm(LlmEvent),
    LlmTool(ToolEvent),
    Quit,
    System(SystemEvent),
    MessageUpdated(MessageUpdatedEvent),
    UpdateFailed(UpdateFailedEvent),
    Error(ErrorEvent),
    IndexingProgress(indexer::IndexingStatus),
    IndexingStarted,
    IndexingCompleted,
    IndexingFailed,
    EventBusStarted,
    // ... additional variants
}
```

#### `SystemEvent` Enum (Tool-Related)
**Path:** `ploke_tui::app_state::events::SystemEvent`

```rust
pub enum SystemEvent {
    ToolCallRequested { tool_call: ToolCall, request_id: Uuid, parent_id: Uuid },
    ToolCallCompleted { request_id: Uuid, parent_id: Uuid, call_id: ArcStr, content: String, ui_payload: Option<ToolUiPayload> },
    ToolCallFailed { request_id: Uuid, parent_id: Uuid, call_id: ArcStr, error: String, ui_payload: Option<ToolUiPayload> },
    ModelSwitched(String),
    ReadQuery { query_name: String, file_name: String },
    WriteQuery { query_name: String, query_content: String },
    HistorySaved { file_path: String },
    BackupDb { file_dir: String, is_success: bool, error: Option<String> },
    LoadDb { workspace_ref: String, file_dir: String, root_path: Option<PathBuf>, is_success: bool, error: Option<String> },
    ReIndex { workspace: PathBuf },
    // ... additional variants
}
```

### A.5.4 Configuration and Setup Requirements

#### `AppState` Setup Requirements
**Path:** `ploke_tui::app_state::AppState`

```rust
pub struct AppState {
    pub chat: ChatState,
    pub config: ConfigState,
    pub system: SystemState,
    pub indexing_state: RwLock<Option<IndexingStatus>>,
    pub indexer_task: Option<Arc<IndexerTask>>,
    pub indexing_control: Arc<Mutex<Option<mpsc::Sender<IndexerCommand>>>>,
    pub db: Arc<Database>,
    pub embedder: Arc<EmbeddingRuntime>,
    pub io_handle: IoManagerHandle,
    pub proposals: RwLock<HashMap<Uuid, EditProposal>>,
    pub create_proposals: RwLock<HashMap<Uuid, CreateProposal>>,
    pub rag: Option<Arc<RagService>>,
    pub budget: TokenBudget,
}
```

#### Required Subsystems for Headless Operation

1. **Database** (`Arc<Database>`)
   - Initialize with `Database::init_with_schema()`
   - Setup multi-embedding with `setup_multi_embedding()`

2. **Embedding Runtime** (`Arc<EmbeddingRuntime>`)
   - Create with `EmbeddingRuntime::with_default_set(processor)`
   - Or `EmbeddingRuntime::from_shared_set(embedding_set, processor)`

3. **IO Manager** (`IoManagerHandle`)
   - Create with `IoManagerHandle::new()`

4. **EventBus** (`Arc<EventBus>`)
   - Create with `EventBus::new(EventBusCaps::default())`

5. **RAG Service** (optional but recommended)
   - Create with `RagService::new_full(db, embedder, io_handle, config)`

#### Full Headless Setup Example (from `AppHarness::spawn`)

```rust
// 1. Create config
let config = UserConfig::default();
let runtime_cfg: RuntimeConfig = config.clone().into();

// 2. Initialize database
let db_handle = Arc::new(Database::init_with_schema()?);
db_handle.setup_multi_embedding()?;

// 3. Create embedder
let processor = config.load_embedding_processor()?;
let embedding_runtime = Arc::new(EmbeddingRuntime::from_shared_set(
    Arc::clone(&db_handle.active_embedding_set),
    processor,
));

// 4. Create IO handle
let io_handle = IoManagerHandle::new();

// 5. Create event bus
let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

// 6. Start BM25 service
let bm25_cmd = bm25_index::bm25_service::start(Arc::clone(&db_handle), 0.0)?;

// 7. Create indexer
let (index_cancellation_token, index_cancel_handle) = CancellationToken::new();
let indexer_task = IndexerTask::new(
    db_handle.clone(),
    io_handle.clone(),
    Arc::clone(&embedding_runtime),
    index_cancellation_token,
    index_cancel_handle,
    None,
).with_bm25_tx(bm25_cmd);

// 8. Create RAG service (optional)
let rag = match RagService::new_full(
    db_handle.clone(),
    Arc::clone(&embedding_runtime),
    io_handle.clone(),
    RagConfig::default(),
) {
    Ok(svc) => Some(Arc::new(svc)),
    Err(_) => None,
};

// 9. Create AppState
let state = Arc::new(AppState {
    chat: ChatState::new(ChatHistory::new()),
    config: ConfigState::new(runtime_cfg),
    system: SystemState::new(SystemStatus::new(Some(fixture_root))),
    indexing_state: RwLock::new(None),
    indexer_task: Some(Arc::clone(&indexer_task)),
    indexing_control: Arc::new(Mutex::new(None)),
    db: db_handle,
    embedder: Arc::clone(&embedding_runtime),
    io_handle,
    proposals: RwLock::new(HashMap::new()),
    create_proposals: RwLock::new(HashMap::new()),
    rag,
    budget: TokenBudget::default(),
});

// 10. Start state manager
let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(1024);
tokio::spawn(state_manager(state.clone(), cmd_rx, event_bus.clone(), rag_event_tx));

// 11. Start LLM manager
tokio::spawn(llm_manager(
    event_bus.subscribe(EventPriority::Realtime),
    event_bus.subscribe(EventPriority::Background),
    state.clone(),
    cmd_tx.clone(),
    event_bus.clone(),
    cancel_rx,
));

// 12. Create and run App with TestBackend
let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
let input = UnboundedReceiverStream::new(input_rx);
let backend = TestBackend::new(80, 24);
let terminal = Terminal::new(backend)?;

let app = App::new(
    config.command_style,
    state.clone(),
    cmd_tx,
    &event_bus,
    default_model(),
    tool_verbosity,
    cancel_tx,
);

// Run the app
let _ = app.run_with(terminal, input, RunOptions { setup_terminal_modes: false }).await;
```

---

## A.6 Tool Call Commands

### A.6.1 Tool Types Overview

All tools implement the `Tool` trait and can be found in `ploke_tui::tools`.

#### `Tool` Trait
**Path:** `ploke_tui::tools::Tool`

```rust
pub trait Tool {
    type Output: Serialize + Send;
    type OwnedParams: Serialize + Send;
    type Params<'de>: Deserialize<'de> + Send where Self: 'de;

    fn name() -> ToolName;
    fn description() -> ToolDescr;
    fn schema() -> &'static serde_json::Value;
    fn build(ctx: &Ctx) -> Self where Self: Sized;
    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams;
    fn adapt_error(err: ToolInvocationError) -> ToolError;
    fn tool_def() -> ToolDefinition;
    fn emit_completed(ctx: &Ctx, output_json: String, ui_payload: Option<ToolUiPayload>);
    fn emit_err(ctx: &Ctx, error: ToolError);
    fn deserialize_params<'a>(json: &'a str) -> Result<Self::Params<'a>, ToolInvocationError>;
    
    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: Ctx,
    ) -> Result<ToolResult, ploke_error::Error>;
}
```

#### `Ctx` Struct (Tool Context)
**Path:** `ploke_tui::tools::Ctx`

```rust
#[derive(Debug)]
pub struct Ctx {
    pub state: Arc<crate::app_state::AppState>,
    pub event_bus: Arc<crate::EventBus>,
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub call_id: ArcStr,
}
```

#### `ToolResult` Struct
**Path:** `ploke_tui::tools::ToolResult`

```rust
#[derive(Debug, Serialize)]
pub struct ToolResult {
    pub content: String,           // JSON-serialized output
    pub ui_payload: Option<ToolUiPayload>,
}
```

### A.6.2 Individual Tool Documentation

#### 1. `NsRead` Tool (Non-Semantic Read)
**Path:** `ploke_tui::tools::ns_read::NsRead`

**Purpose:** Read file contents outside semantic graph (configs, docs, unindexed files)

**Tool Name:** `ToolName::NsRead` (string: `"ns_read"`)

**Parameters:**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct NsReadParams<'a> {
    pub file: Cow<'a, str>,           // Required: file path
    pub start_line: Option<u32>,      // Optional: 1-based start line
    pub end_line: Option<u32>,        // Optional: 1-based end line (inclusive)
    pub max_bytes: Option<u32>,       // Optional: max bytes (default: 32 KiB)
}
```

**Output:** `NsReadResult`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NsReadResult {
    pub ok: bool,
    pub file_path: String,
    pub exists: bool,
    pub byte_len: Option<u64>,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub truncated: bool,
    pub content: Option<String>,
    pub file_hash: Option<FileHash>,
}
```

**Execution:**
```rust
impl Tool for NsRead {
    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: Ctx,
    ) -> Result<ToolResult, ploke_error::Error>
}
```

**Actor/Setup Requirements:**
- `AppState` with loaded workspace (for path resolution)
- `IoManagerHandle` for file reading
- Path policy validation through `SystemStatus::tool_path_context()`

**Example Usage:**
```rust
use ploke_tui::tools::{NsRead, Tool};

let params = NsRead::deserialize_params(r#"{"file": "src/lib.rs", "start_line": 1, "end_line": 50}"#)?;
let result = NsRead::execute(params, ctx).await?;
let ns_read_result: NsReadResult = serde_json::from_str(&result.content)?;
```

---

#### 2. `CodeItemLookup` Tool
**Path:** `ploke_tui::tools::code_item_lookup::CodeItemLookup`

**Purpose:** Look up code items in the semantic graph by exact path

**Tool Name:** `ToolName::CodeItemLookup` (string: `"code_item_lookup"`)

**Parameters:**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct LookupParams<'a> {
    pub item_name: Cow<'a, str>,      // Required: item name
    pub file_path: Cow<'a, str>,      // Required: file path
    pub node_kind: Cow<'a, str>,      // Required: function|const|enum|impl|...
    pub module_path: Cow<'a, str>,    // Required: e.g., "crate::module::sub"
}
```

**Output:** `ConciseContext` (from ploke_core)
```rust
pub struct ConciseContext {
    pub file_path: NodeFilepath,
    pub canon_path: CanonPath,
    pub snippet: String,
}
```

**Execution:**
```rust
async fn execute<'de>(
    params: Self::Params<'de>,
    ctx: Ctx,
) -> Result<ToolResult, ploke_error::Error>
```

**Actor/Setup Requirements:**
- Database with indexed code graph
- `IoManagerHandle` for snippet retrieval
- Workspace loaded with valid crate focus
- `graph_resolve_exact()` helper function

**Special Considerations:**
- Validates `node_kind` against allowed values
- Requires module_path to start with "crate"
- Returns error if multiple items match

---

#### 3. `CodeItemEdges` Tool
**Path:** `ploke_tui::tools::get_code_edges::CodeItemEdges`

**Purpose:** Get relationship edges for a code item (calls, implements, etc.)

**Tool Name:** `ToolName::CodeItemEdges` (string: `"code_item_edges"`)

**Parameters:** Same as `CodeItemLookup` (`LookupParams`)

**Output:** `NodeEdgeInfo`
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeEdgeInfo {
    node_info: ConciseContext,
    edge_info: Vec<ResolvedEdgeData>,
}
```

**Actor/Setup Requirements:**
- Same as `CodeItemLookup`
- `graph_resolve_edges()` helper function

---

#### 4. `RequestCodeContext` Tool (GAT version: `RequestCodeContextGat`)
**Path:** `ploke_tui::tools::request_code_context::RequestCodeContextGat`

**Purpose:** Search and retrieve code context using hybrid semantic + BM25 search

**Tool Name:** `ToolName::RequestCodeContext` (string: `"request_code_context"`)

**Parameters:**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct RequestCodeContextParams<'a> {
    pub token_budget: Option<u32>,
    pub search_term: Option<Cow<'a, str>>,
}
```

**Output:** `RequestCodeContextResult`
```rust
pub struct RequestCodeContextResult {
    pub context: Vec<ConciseContext>,
    pub search_term: String,
    pub top_k: usize,
}
```

**Actor/Setup Requirements:**
- `RagService` initialized in `AppState`
- Database with embeddings
- BM25 index available

**Special Considerations:**
- Falls back to last user message if `search_term` is None
- Uses configured token budget and retrieval strategy

---

#### 5. `GatCodeEdit` Tool (Apply Code Edit)
**Path:** `ploke_tui::tools::code_edit::GatCodeEdit`

**Purpose:** Apply canonical code edits to source files

**Tool Name:** `ToolName::ApplyCodeEdit` (string: `"apply_code_edit"`)

**Parameters:**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct CodeEditParams<'a> {
    pub edits: Vec<CanonicalEditBorrowed<'a>>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CanonicalEditBorrowed<'a> {
    pub file: Cow<'a, str>,       // File path
    pub canon: Cow<'a, str>,      // Canonical path (e.g., "crate::mod::Item")
    pub node_type: NodeType,      // NodeType enum
    pub code: Cow<'a, str>,       // Replacement code
}
```

**Output:** `ApplyCodeEditResult`
```rust
pub struct ApplyCodeEditResult {
    pub ok: bool,
    pub staged: usize,
    pub applied: usize,
    pub files: Vec<String>,
    pub preview_mode: String,
    pub auto_confirmed: bool,
}
```

**Actor/Setup Requirements:**
- Full `AppState` with all subsystems
- Proposal registry for staging edits
- Editing configuration in `RuntimeConfig`
- Path policy for workspace scoping

**Special Considerations:**
- Edits are staged, not immediately applied (unless auto-confirm enabled)
- Creates `EditProposal` in state
- Supports multiple edit modes: Splice, Canonical

---

#### 6. `CreateFile` Tool
**Path:** `ploke_tui::tools::create_file::CreateFile`

**Purpose:** Create new files with content

**Tool Name:** `ToolName::CreateFile` (string: `"create_file"`)

**Parameters:**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct CreateFileParams<'a> {
    pub file_path: Cow<'a, str>,
    pub content: Cow<'a, str>,
    pub on_exists: Option<Cow<'a, str>>,  // "error" | "overwrite"
    pub create_parents: bool,
}
```

**Output:** `CreateFileResult`
```rust
pub struct CreateFileResult {
    pub ok: bool,
    pub staged: usize,
    pub applied: usize,
    pub files: Vec<String>,
    pub preview_mode: String,
    pub auto_confirmed: bool,
}
```

**Actor/Setup Requirements:**
- `AppState` with loaded workspace
- `CreateProposal` registry access
- File extension validation

---

#### 7. `NsPatch` Tool
**Path:** `ploke_tui::tools::ns_patch::NsPatch`

**Purpose:** Apply non-semantic patches using unified diff format

**Tool Name:** `ToolName::NsPatch` (string: `"ns_patch"`)

**Parameters:**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct NsPatchParams<'a> {
    pub patches: Vec<NsPatchBorrowed<'a>>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NsPatchBorrowed<'a> {
    pub file: Cow<'a, str>,
    pub diff: Cow<'a, str>,       // Unified diff
    pub reasoning: Cow<'a, str>,
}
```

**Output:** `ApplyNsPatchResult`
```rust
pub struct ApplyNsPatchResult {
    pub ok: bool,
    pub staged: usize,
    pub applied: usize,
    pub files: Vec<String>,
    pub preview_mode: String,
    pub auto_confirmed: bool,
}
```

**Actor/Setup Requirements:**
- `mpatch` crate for diff parsing/application
- Same setup as `GatCodeEdit`

---

#### 8. `ListDir` Tool
**Path:** `ploke_tui::tools::list_dir::ListDir`

**Purpose:** Safe directory listing without shell access

**Tool Name:** `ToolName::ListDir` (string: `"list_dir"`)

**Parameters:**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ListDirParams<'a> {
    pub dir: Cow<'a, str>,
    pub include_hidden: Option<bool>,
    pub sort: Option<Cow<'a, str>>,      // "name" | "mtime" | "size" | "none"
    pub max_entries: Option<u32>,
}
```

**Output:** `ListDirResult`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDirResult {
    pub ok: bool,
    pub dir: String,
    pub exists: bool,
    pub truncated: bool,
    pub entries: Vec<ListDirEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDirEntry {
    pub name: String,
    pub path: String,
    pub kind: String,        // "file" | "dir" | "symlink" | "other"
    pub size_bytes: Option<u64>,
    pub modified_ms: Option<i64>,
}
```

**Actor/Setup Requirements:**
- `AppState` with loaded workspace
- Path policy for directory scoping

---

#### 9. `CargoTool` Tool
**Path:** `ploke_tui::tools::cargo::CargoTool`

**Purpose:** Run `cargo check` or `cargo test` with JSON diagnostics

**Tool Name:** `ToolName::Cargo` (string: `"cargo"`)

**Parameters:**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct CargoToolParams<'a> {
    pub command: CargoCommand,        // Test | Check
    pub scope: CargoScope,            // Focused | Workspace
    pub package: Option<Cow<'a, str>>,
    pub features: Option<Vec<Cow<'a, str>>>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub target: Option<Cow<'a, str>>,
    pub profile: Option<Cow<'a, str>>,
    pub release: bool,
    pub lib: bool,
    pub tests: bool,
    pub bins: bool,
    pub examples: bool,
    pub benches: bool,
    pub test_args: Option<Vec<Cow<'a, str>>>,
}
```

**Output:** `CargoToolResult`
```rust
#[derive(Debug, Clone, Serialize)]
pub struct CargoToolResult {
    pub ok: bool,
    pub status_reason: CargoStatusReason,
    pub command: CargoCommand,
    pub scope: CargoScope,
    pub manifest_path: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub summary: CargoSummary,
    pub diagnostics: Vec<CargoDiagnostic>,
    pub stderr_tail: Vec<String>,
    pub non_json_stdout_tail: Vec<String>,
    pub json_parse_errors_tail: Vec<String>,
    pub raw_messages_truncated: bool,
}
```

**Actor/Setup Requirements:**
- Focused crate in `SystemState`
- `cargo` binary in PATH
- Workspace with valid Cargo.toml

---

### A.6.3 Direct Tool Execution (Bypassing LLM Loop)

#### `process_tool()` Function
**Path:** `ploke_tui::tools::process_tool`

```rust
pub(crate) async fn process_tool(
    tool_call: ToolCall,
    ctx: Ctx,
) -> color_eyre::Result<()>
```

This is the main entry point for tool execution. It:
1. Deserializes parameters using `Tool::deserialize_params()`
2. Calls `Tool::execute()`
3. Emits completion events via `Tool::emit_completed()` or `Tool::emit_err()`

#### Manual Tool Execution Example

```rust
use ploke_tui::tools::{NsRead, Tool, Ctx, ToolResult};
use ploke_core::ArcStr;
use uuid::Uuid;

// Create context
let ctx = Ctx {
    state: app_state.clone(),
    event_bus: event_bus.clone(),
    request_id: Uuid::new_v4(),
    parent_id: Uuid::new_v4(),
    call_id: ArcStr::from("manual-call-1"),
};

// Deserialize parameters
let args = r#"{"file": "src/main.rs", "max_bytes": 1024}"#;
let params = NsRead::deserialize_params(args)?;

// Execute tool
let result: ToolResult = NsRead::execute(params, ctx).await?;

// Parse output
let content: NsReadResult = serde_json::from_str(&result.content)?;
```

#### Using `ToolCall` Struct
**Path:** `ploke_llm::response::ToolCall`

```rust
pub struct ToolCall {
    pub call_id: ArcStr,
    pub call_type: FunctionMarker,
    pub function: FunctionCall,
}

pub struct FunctionCall {
    pub name: ToolName,
    pub arguments: String,  // JSON string
}
```

---

## Types and Structs Identified

### Core Types

| Type | Path | Description |
|------|------|-------------|
| `App` | `ploke_tui::app::App` | Main TUI application struct |
| `AppState` | `ploke_tui::app_state::AppState` | Shared application state |
| `EventBus` | `ploke_tui::event_bus::EventBus` | Event broadcast system |
| `AppEvent` | `ploke_tui::AppEvent` | Application events enum |
| `SystemEvent` | `ploke_tui::app_state::events::SystemEvent` | System-level events |
| `StateCommand` | `ploke_tui::app_state::StateCommand` | Commands to state manager |
| `Ctx` | `ploke_tui::tools::Ctx` | Tool execution context |
| `Tool` | `ploke_tui::tools::Tool` | Tool trait |
| `ToolResult` | `ploke_tui::tools::ToolResult` | Tool execution result |
| `RunOptions` | `ploke_tui::app::RunOptions` | App run configuration |

### Tool Types

| Type | Path | Description |
|------|------|-------------|
| `NsRead` | `ploke_tui::tools::ns_read::NsRead` | Non-semantic file read |
| `CodeItemLookup` | `ploke_tui::tools::code_item_lookup::CodeItemLookup` | Code item lookup |
| `CodeItemEdges` | `ploke_tui::tools::get_code_edges::CodeItemEdges` | Code edges lookup |
| `RequestCodeContextGat` | `ploke_tui::tools::request_code_context::RequestCodeContextGat` | Context search |
| `GatCodeEdit` | `ploke_tui::tools::code_edit::GatCodeEdit` | Code editing |
| `CreateFile` | `ploke_tui::tools::create_file::CreateFile` | File creation |
| `NsPatch` | `ploke_tui::tools::ns_patch::NsPatch` | Non-semantic patching |
| `ListDir` | `ploke_tui::tools::list_dir::ListDir` | Directory listing |
| `CargoTool` | `ploke_tui::tools::cargo::CargoTool` | Cargo commands |

### Error Types

| Type | Path | Description |
|------|------|-------------|
| `ToolError` | `ploke_tui::tools::error::ToolError` | Tool execution error |
| `ToolInvocationError` | `ploke_tui::tools::error::ToolInvocationError` | Tool invocation error |
| `ToolErrorCode` | `ploke_tui::tools::error::ToolErrorCode` | Error code enum |

---

## Issues Encountered

1. **Complex Setup Requirements**: Headless TUI requires significant setup - database, embedder, IO manager, event bus, and optional RAG service all need proper initialization.

2. **Feature Flags for Test Harness**: The `AppHarness` and related test utilities require the `test_harness` feature flag to be enabled.

3. **Workspace Loading**: Tools require a loaded workspace via `SystemState::set_focus_from_root()` or `set_loaded_workspace()` for path resolution.

4. **Async Context**: Tool execution is fully async and requires a Tokio runtime.

---

## Tracing Instrumentation Status

| Component | Tracing Status | Notes |
|-----------|---------------|-------|
| `process_tool()` | ✅ Instrumented | Uses `tracing::debug!` with `DEBUG_TOOLS` target |
| `CargoTool::execute()` | ✅ Instrumented | Uses `#[tracing::instrument]` with `TOOL_CALL_TARGET` |
| `EventBus::send()` | ✅ Instrumented | Uses `#[instrument]` |
| `run_event_bus()` | ✅ Instrumented | Uses `#[instrument]` |
| `App::run_with()` | ⚠️ Partial | Some tracing but not fully instrumented |
| Individual tools | ⚠️ Partial | Most use `tracing::debug!` but not `#[instrument]` |

**Tracing Targets:**
- `ploke_tui::tracing_setup::TOOL_CALL_TARGET` - Tool call logging
- `ploke_tui::utils::consts::DEBUG_TOOLS` - Debug tool output
- `ploke_tui::utils::consts::DBG_EVENTS` - Event debugging

---

## Actor/Setup Requirements Summary

### Minimal Setup for Tool Execution

```rust
// 1. Initialize database
let db = Arc::new(Database::init_with_schema()?);
db.setup_multi_embedding()?;

// 2. Create embedder
let processor = EmbeddingProcessor::new_mock(); // or real processor
let embedder = Arc::new(EmbeddingRuntime::with_default_set(processor));
db.active_embedding_set = embedder.active_set_handle();

// 3. Create IO handle
let io_handle = IoManagerHandle::new();

// 4. Create event bus
let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

// 5. Create RAG service (optional, only for RequestCodeContext)
let rag = RagService::new_full(db.clone(), embedder.clone(), io_handle.clone(), RagConfig::default())?;

// 6. Create AppState
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
    rag: Some(Arc::new(rag)),
    budget: TokenBudget::default(),
});

// 7. Create context and execute tool
let ctx = Ctx {
    state,
    event_bus,
    request_id: Uuid::new_v4(),
    parent_id: Uuid::new_v4(),
    call_id: ArcStr::from("test-call"),
};

let result = NsRead::execute(params, ctx).await?;
```

---

## Example Usage from Existing Code

### From `test_utils/new_test_harness.rs` - AppHarness

```rust
/// A running, headless app instance with realistic subsystems and handy senders.
pub struct AppHarness {
    pub state: Arc<AppState>,
    pub event_bus: Arc<EventBus>,
    pub cmd_tx: mpsc::Sender<StateCommand>,
    pub input_tx: tokio::sync::mpsc::UnboundedSender<Result<crossterm::event::Event, std::io::Error>>,
    app_task: tokio::task::JoinHandle<()>,
}

impl AppHarness {
    pub async fn spawn() -> color_eyre::Result<Self> { ... }
    pub async fn add_user_msg(&self, content: impl Into<String>) -> Uuid { ... }
    pub async fn shutdown(self) { ... }
}
```

### From `tools/mod.rs` - process_tool

```rust
pub(crate) async fn process_tool(tool_call: ToolCall, ctx: Ctx) -> color_eyre::Result<()> {
    let name = tool_call.function.name;
    let args = sanitize_tool_args(&tool_call.function.arguments);
    
    tracing::info!(target: TOOL_CALL_TARGET,
        tool = %name.as_str(),
        request_id = %ctx.request_id,
        "tool_call_request"
    );
    
    match tool_call.function.name {
        ToolName::RequestCodeContext => { /* ... */ }
        ToolName::ApplyCodeEdit => { /* ... */ }
        ToolName::NsRead => { /* ... */ }
        // ... additional tools
    }
}
```

---

## Cross-Crate Dependencies

| Dependency | Crate | Usage |
|------------|-------|-------|
| `Database` | `ploke_db` | Code graph storage, queries |
| `EmbeddingRuntime` | `ploke_embed` | Embedding operations |
| `IoManagerHandle` | `ploke_io` | File I/O operations |
| `RagService` | `ploke_rag` | Context retrieval |
| `ToolCall` | `ploke_llm` | LLM tool call format |
| `ToolName`, `ToolDescr` | `ploke_core` | Shared tool types |

---

## Recommendations for xtask Commands

1. **For headless TUI commands**: Use `AppHarness` from `test_utils::new_test_harness` as the reference implementation. It provides the complete setup pattern.

2. **For direct tool execution**: Create a minimal `Ctx` with properly initialized `AppState`. Most tools need: database, embedder (can be mock), io_handle, and loaded workspace.

3. **Tool selection for xtask**: 
   - `NsRead` - Good for file inspection commands
   - `ListDir` - Good for directory exploration commands
   - `CodeItemLookup` - Good for symbol lookup commands
   - `CargoTool` - Good for build/test commands

4. **Avoid for xtask**: Tools that require full LLM context (`RequestCodeContext`) or complex proposal workflows (`GatCodeEdit`, `NsPatch`, `CreateFile`) unless building interactive xtask workflows.
