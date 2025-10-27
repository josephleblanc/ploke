# App Startup and Initialization Data Flow Analysis

*Generated: 2025-09-06*

This document analyzes the complete data flow during Ploke TUI application startup and initialization, tracing the sequence from binary launch through to ready state.

## 1. Application Entry Point

**Location**: `crates/ploke-tui/src/main.rs:14-28`

### Sequence:
1. **Tokio Runtime Initialization**: `#[tokio::main]` creates async runtime
2. **Tracing Setup**: `tracing_setup::init_tracing()` - initializes logging infrastructure
3. **Main Application Launch**: Calls `try_main().await` 
4. **Error Handling**: On error, logs via tracing and propagates error up

## 2. Core Startup Sequence (`try_main()`)

**Location**: `crates/ploke-tui/src/lib.rs:121-276`

### Phase 1: Environment & Configuration Loading
1. **Dotenv Loading**: `dotenvy::dotenv().ok()` - loads `.env` file if present
2. **Panic Hook Setup**: Global panic handler to restore terminal state
3. **Configuration Resolution**:
   - Config file: `~/.config/ploke/config.toml` (optional, fallback to defaults)
   - Environment variables: Overrides with `_` separator (e.g., `PLOKE_EMBEDDING__LOCAL__MODEL_ID`)
   - User config deserialized into `UserConfig` struct

### Phase 2: Configuration Processing & Validation
1. **Registry Merging**: `config.registry.with_defaults()` - merges user config with curated defaults
2. **OpenRouter Capabilities Refresh**: 
   - `config.registry.refresh_from_openrouter().await`
   - Fetches current model capabilities and pricing from OpenRouter API
   - Updates `ModelCapabilities` cache with tool support, context lengths, costs
   - **Failure Mode**: Warns on failure, continues with cached/default capabilities
3. **API Key Resolution**: `config.registry.load_api_keys()` - resolves API keys from environment variables
4. **Runtime Config Creation**: Converts `UserConfig` to `RuntimeConfig`

### Phase 3: Database & Storage Initialization
1. **Database Setup**: `ploke_db::Database::init_with_schema()` - CozoDB with full schema
2. **I/O Manager**: `ploke_io::IoManagerHandle::new()` - file operations manager (spawns thread)
3. **Event Bus**: `EventBus::new(EventBusCaps::default())` - multi-priority event system

### Phase 4: Embedding & Search Services
1. **Embedding Processor**: `config.load_embedding_processor()`
   - Resolves embedding backend (Local/HuggingFace/OpenAI/Cozo)
   - Default: Local sentence-transformers model "all-MiniLM-L6-v2"
2. **BM25 Service**: `bm25_index::bm25_service::start()` - hybrid search indexing
3. **Indexer Task**: `IndexerTask::new()` - AST parsing and embedding pipeline
   - Links: DB handle, I/O manager, embedding processor, cancellation token
   - **Note**: Connected to BM25 service via channel
4. **RAG Service**: `ploke_rag::RagService::new_full()`
   - Combines BM25 + dense vector search + I/O manager
   - **Failure Mode**: Warns on failure, continues without RAG (None)

### Phase 5: Application State Assembly
1. **AppState Construction**: `Arc<AppState>` containing:
   - `ChatState`: Wraps `ChatHistory::new()` for conversation management
   - `ConfigState`: Runtime configuration
   - `SystemState`: Default system state
   - `indexing_state`: `RwLock<None>` - will be updated during indexing operations
   - All service handles (DB, embedder, I/O, RAG)
   - `proposals`: `RwLock<HashMap>` for edit proposals
   - `TokenBudget`: Default budget configuration
2. **Proposal Loading**: `load_proposals(&state).await` - restores persisted edit proposals (best-effort)

### Phase 6: Subsystem Spawning
1. **Command Channel**: `mpsc::channel<StateCommand>(1024)` - backpressure-aware commands
2. **File Manager**: Spawned with event subscriptions and channels
3. **State Manager**: `tokio::spawn(state_manager())` - central state coordinator
4. **Global Event Bus**: `set_global_event_bus()` - for error handling
5. **LLM Manager**: `tokio::spawn(llm_manager())` - handles LLM interactions
6. **Event Bus Runner**: `tokio::spawn(run_event_bus())` - event routing
7. **Observability**: `tokio::spawn(observability::run_observability())` - metrics/logging

### Phase 7: UI Initialization
1. **Terminal Setup**: `ratatui::init()` - creates default terminal
2. **App Construction**: `App::new()` with:
   - Command style (slash/neovim)
   - State handle (Arc)
   - Command channel sender
   - Event bus subscription (realtime priority)
   - Active model ID
3. **Application Launch**: `app.run(terminal).await`
4. **Cleanup**: `ratatui::restore()` on completion

## 3. System State After Startup

### Core Infrastructure Ready:
- **Database**: CozoDB initialized with 34 schemas, ready for queries
- **Event System**: Multi-priority event bus routing background/realtime events
- **File I/O**: IoManagerHandle managing file operations with safety checks
- **Embedding Pipeline**: Processor ready for vector generation
- **Search Services**: BM25 + vector search ready (if RAG initialized successfully)

### Configuration Loaded:
- **User Preferences**: Command style, editing config, model registry
- **API Keys**: Resolved from environment variables per provider type
- **Model Registry**: OpenRouter capabilities cached, active model set
- **Default State**: No crate indexed, no conversation history

### UI Ready State:
- **Terminal**: Configured with bracketed paste, focus change, mouse capture
- **Input Mode**: Default mode (Insert), empty input buffer
- **UI Components**: Conversation view, input view, status indicators
- **Event Subscriptions**: Real-time event processing for UI updates

### Services Running:
1. **State Manager**: Coordinates state updates via StateCommand channel
2. **LLM Manager**: Handles chat completions and tool calling
3. **File Manager**: Monitors file system events and manages I/O
4. **Event Bus**: Routes AppEvents between subsystems
5. **Observability**: Collects metrics and manages logging

## 4. Key Data Structures at Startup

### UserConfig Resolution Order:
1. Hardcoded defaults in code
2. TOML config file (`~/.config/ploke/config.toml`)
3. Environment variables (highest priority)

### Event Flow Architecture:
- **Realtime Channel**: UI updates, user interactions, model switches
- **Background Channel**: API calls, tool execution, system events
- **Priority Routing**: Events automatically routed based on `AppEvent::priority()`

### Threading Model:
- **Main Thread**: Tokio async runtime, UI event loop
- **I/O Thread**: Dedicated thread for file operations (IoManagerHandle)
- **Background Tasks**: Multiple spawned tokio tasks for subsystems

## 5. Failure Modes & Resilience

### Graceful Degradation:
- **Config Loading**: Falls back to defaults on parse errors
- **OpenRouter Refresh**: Continues with cached capabilities on API failure
- **RAG Service**: Continues without RAG if initialization fails
- **Proposal Loading**: Best-effort, continues if proposals can't be restored

### Error Propagation:
- **Fatal Errors**: Database init failure, terminal setup failure
- **Recoverable Errors**: API key missing (runtime resolution), service degradation
- **Global Error Bus**: Centralized error event handling via global event bus

## 6. Performance Characteristics

### Async Patterns:
- **Concurrent Initialization**: Services spawned in parallel where possible
- **Backpressure Handling**: Command channels with bounded capacity (1024)
- **Resource Sharing**: Arc-wrapped state for efficient cloning across tasks

### Memory Layout:
- **State Sharing**: Single Arc<AppState> shared across all subsystems
- **Event Buffering**: Bounded channels prevent unbounded memory growth
- **String Management**: ArcStr preferred over String for cross-thread sharing

## 7. Observable Startup Events

### Tracing Output:
- Config loading and merging steps
- Service initialization success/failure
- API key resolution results
- Model registry refresh status

### Event Bus Emissions:
- `AppEvent::EventBusStarted` when event routing begins
- `SystemEvent::*` for various system initialization steps
- Error events for any failures during startup

This analysis provides the foundation for understanding how subsequent user interactions (queries, commands, indexing) build upon this initialized system state.