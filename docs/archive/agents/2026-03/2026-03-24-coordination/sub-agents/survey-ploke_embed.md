# Survey: ploke_embed crate

**Date:** 2026-03-25  
**Agent:** Sub-agent for M.1 Survey  
**Task:** Survey ploke_embed crate for embedding/indexing functions (A.3 commands)

---

## Files Touched During Survey

- `/home/brasides/code/ploke/crates/ingest/ploke-embed/src/lib.rs` - Module exports
- `/home/brasides/code/ploke/crates/ingest/ploke-embed/src/indexer/mod.rs` - Core indexing logic (1000+ lines)
- `/home/brasides/code/ploke/crates/ingest/ploke-embed/src/indexer/unit_tests.rs` - Test examples
- `/home/brasides/code/ploke/crates/ingest/ploke-embed/src/runtime.rs` - EmbeddingRuntime
- `/home/brasides/code/ploke/crates/ingest/ploke-embed/src/cancel_token.rs` - Cancellation handling
- `/home/brasides/code/ploke/crates/ingest/ploke-embed/src/config.rs` - Configuration types
- `/home/brasides/code/ploke/crates/ingest/ploke-embed/src/error.rs` - Error types
- `/home/brasides/code/ploke/crates/ingest/ploke-embed/src/local/mod.rs` - Local embedder (candle)
- `/home/brasides/code/ploke/crates/ingest/ploke-embed/src/providers/openrouter.rs` - OpenRouter backend
- `/home/brasides/code/ploke/crates/ingest/ploke-embed/src/providers/mod.rs` - Provider module
- `/home/brasides/code/ploke/crates/ploke-llm/src/router_only/openrouter/embed.rs` - OpenRouterEmbedEnv (for env var understanding)

---

## Detailed Function Documentation

### 1. EmbeddingProcessor

**Path:** `ploke_embed::indexer::EmbeddingProcessor`

```rust
#[derive(Debug)]
pub struct EmbeddingProcessor {
    source: EmbeddingSource,
}
```

#### 1.1 `EmbeddingProcessor::new`

**Signature:**
```rust
pub fn new(source: EmbeddingSource) -> Self
```

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `source` | `EmbeddingSource` | The embedding backend to use |

**Output:** `Self` (EmbeddingProcessor instance)

**Key Types:**
```rust
#[derive(Debug)]
pub enum EmbeddingSource {
    Local(LocalEmbedder),
    HuggingFace(HuggingFaceBackend),
    OpenAI(OpenAIBackend),
    OpenRouter(OpenRouterBackend),
    Cozo(CozoBackend),  // Mock/test placeholder
}
```

**Example Usage:**
```rust
use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource};
use ploke_embed::local::{LocalEmbedder, EmbeddingConfig};

let model = LocalEmbedder::new(EmbeddingConfig::default())?;
let source = EmbeddingSource::Local(model);
let processor = EmbeddingProcessor::new(source);
```

#### 1.2 `EmbeddingProcessor::new_mock`

**Signature:**
```rust
pub fn new_mock() -> Self
```

**Purpose:** Creates a lightweight mock embedder for tests using CozoBackend with fixed 384 dimensions.

**Example Usage:**
```rust
let processor = EmbeddingProcessor::new_mock();
```

#### 1.3 `EmbeddingProcessor::generate_embeddings`

**Signature:**
```rust
pub async fn generate_embeddings(
    &self,
    snippets: Vec<String>,
) -> Result<Vec<Vec<f32>>, EmbedError>
```

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `snippets` | `Vec<String>` | Text snippets to embed |

**Output:** `Result<Vec<Vec<f32>>, EmbedError>` - Vector of embedding vectors

**Error Type:** `EmbedError` (see Error Types section)

#### 1.4 `EmbeddingProcessor::generate_embeddings_with_cancel`

**Signature:**
```rust
#[instrument(skip_all, fields(source = ?self.source, target = "embed-pipeline"))]
pub async fn generate_embeddings_with_cancel(
    &self,
    snippets: Vec<String>,
    cancel: Option<&CancellationListener>,
) -> Result<Vec<Vec<f32>>, EmbedError>
```

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `snippets` | `Vec<String>` | Text snippets to embed |
| `cancel` | `Option<&CancellationListener>` | Optional cancellation listener |

---

### 2. IndexerTask

**Path:** `ploke_embed::indexer::IndexerTask`

```rust
#[derive(Debug)]
pub struct IndexerTask {
    pub db: Arc<Database>,
    pub io: IoManagerHandle,
    pub embedding_runtime: Arc<EmbeddingRuntime>,
    pub cancellation_token: CancellationToken,
    #[allow(dead_code)]
    cancellation_handle: CancellationHandle,
    pub batch_size_override: Option<usize>,
    pub bm25_tx: Option<mpsc::Sender<bm25_service::Bm25Cmd>>,
    pub cursors: Mutex<HashMap<NodeType, Uuid>>,
    pub total_processed: AtomicUsize,
}
```

#### 2.1 `IndexerTask::new`

**Signature:**
```rust
pub fn new(
    db: Arc<Database>,
    io: IoManagerHandle,
    embedding_runtime: Arc<EmbeddingRuntime>,
    cancellation_token: CancellationToken,
    cancellation_handle: CancellationHandle,
    batch_size_override: Option<usize>,
) -> Self
```

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `db` | `Arc<Database>` | Database handle (CozoDB) |
| `io` | `IoManagerHandle` | I/O manager for reading file snippets |
| `embedding_runtime` | `Arc<EmbeddingRuntime>` | Runtime for generating embeddings |
| `cancellation_token` | `CancellationToken` | Token for cancellation signaling |
| `cancellation_handle` | `CancellationHandle` | Handle to trigger cancellation |
| `batch_size_override` | `Option<usize>` | Optional batch size override |

**Output:** `Self` (IndexerTask instance)

**Special Considerations:**
- The `cancellation_handle` must be kept alive for the duration of the task
- Without it, `CancellationListener::cancelled()` completes immediately (sender dropped)
- This would incorrectly cancel remote embedding requests

**Example Usage:**
```rust
use ploke_embed::indexer::IndexerTask;
use ploke_embed::cancel_token::CancellationToken;
use ploke_db::Database;
use ploke_io::IoManagerHandle;
use std::sync::Arc;

let db = Arc::new(Database::new(cozo_db));
let io = IoManagerHandle::new();
let embedding_runtime = Arc::new(EmbeddingRuntime::from_shared_set(
    Arc::clone(&db.active_embedding_set),
    EmbeddingProcessor::new_mock(),
));

let (cancellation_token, cancel_handle) = CancellationToken::new();

let task = IndexerTask::new(
    Arc::clone(&db),
    io,
    Arc::clone(&embedding_runtime),
    cancellation_token,
    cancel_handle,
    Some(8),  // batch size override
);
```

#### 2.2 `IndexerTask::with_bm25_tx`

**Signature:**
```rust
pub fn with_bm25_tx(mut self, bm25_tx: mpsc::Sender<bm25_service::Bm25Cmd>) -> Self
```

**Purpose:** Builder method to add BM25 service channel for sparse indexing.

#### 2.3 `IndexerTask::run`

**Signature:**
```rust
#[instrument(
    name = "Indexer::run",
    skip(self, progress_tx, control_rx),
    fields(num_not_proc, recent_processed, status="Running")
)]
pub async fn run(
    &self,
    progress_tx: Arc<broadcast::Sender<IndexingStatus>>,
    mut control_rx: mpsc::Receiver<IndexerCommand>,
) -> Result<(), EmbedError>
```

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `progress_tx` | `Arc<broadcast::Sender<IndexingStatus>>` | Channel for progress updates |
| `control_rx` | `mpsc::Receiver<IndexerCommand>` | Channel for control commands |

**Output:** `Result<(), EmbedError>`

**Key Types:**
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum IndexStatus {
    Idle,
    Running,
    Paused,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum IndexerCommand {
    Pause,
    Resume,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct IndexingStatus {
    pub status: IndexStatus,
    pub recent_processed: usize,
    pub num_not_proc: usize,
    pub current_file: Option<PathBuf>,
    pub errors: Vec<String>,
}
```

**Special Considerations:**
- **Async function** - must be run in tokio runtime
- Ensures active embedding set relations exist before batch writes
- Persists metadata for embedding set
- Handles Pause/Resume/Cancel commands via control channel
- Sends progress updates via broadcast channel
- On completion, sends BM25 FinalizeSeed if BM25 is configured

**Example Usage:**
```rust
use tokio::sync::{broadcast, mpsc};

let (progress_tx, mut progress_rx) = broadcast::channel(1000);
let progress_tx_arc = Arc::new(progress_tx);
let (control_tx, control_rx) = mpsc::channel(4);

let idx_handle = tokio::spawn(async move {
    task.run(progress_tx_arc, control_rx).await
});

// Monitor progress
while let Ok(status) = progress_rx.recv().await {
    match status.status {
        IndexStatus::Completed => break,
        IndexStatus::Failed(msg) => panic!("Indexing failed: {}", msg),
        _ => {}
    }
}
```

#### 2.4 `IndexerTask::index_workspace`

**Signature:**
```rust
pub async fn index_workspace(
    task: Arc<Self>,
    workspace_dir: String,
    progress_tx: Arc<broadcast::Sender<IndexingStatus>>,
    mut progress_rx: broadcast::Receiver<IndexingStatus>,
    control_rx: mpsc::Receiver<IndexerCommand>,
    callback_handler: std::thread::JoinHandle<Result<(), ploke_db::DbError>>,
    db_callbacks: crossbeam_channel::Receiver<
        Result<(CallbackOp, NamedRows, NamedRows), ploke_db::DbError>,
    >,
    counter: Arc<AtomicUsize>,
    shutdown: crossbeam_channel::Sender<()>,
) -> Result<(), ploke_error::Error>
```

**Purpose:** Higher-level wrapper that creates HNSW index and manages callback manager lifecycle.

---

### 3. EmbeddingRuntime

**Path:** `ploke_embed::runtime::EmbeddingRuntime`

```rust
#[derive(Debug, Clone)]
pub struct EmbeddingRuntime {
    active_set: Arc<RwLock<EmbeddingSet>>,
    embedder: Arc<RwLock<Arc<EmbeddingProcessor>>>,
}
```

#### 3.1 `EmbeddingRuntime::new`

**Signature:**
```rust
pub fn new(active_set: EmbeddingSet, embedder: EmbeddingProcessor) -> Self
```

**Purpose:** Construct a new runtime with the provided embedding set and embedder.

#### 3.2 `EmbeddingRuntime::from_shared_set`

**Signature:**
```rust
pub fn from_shared_set(
    active_set: Arc<RwLock<EmbeddingSet>>,
    embedder: EmbeddingProcessor,
) -> Self
```

**Purpose:** Construct using an existing active-set handle so the database and runtime share the same lock.

**Example Usage:**
```rust
let runtime = EmbeddingRuntime::from_shared_set(
    Arc::clone(&db.active_embedding_set),
    EmbeddingProcessor::new_mock(),
);
```

#### 3.3 `EmbeddingRuntime::with_default_set`

**Signature:**
```rust
pub fn with_default_set(embedder: EmbeddingProcessor) -> Self
```

**Purpose:** Convenience constructor using the default embedding set.

#### 3.4 `EmbeddingRuntime::activate`

**Signature:**
```rust
pub fn activate(
    &self,
    db: &Database,
    new_set: EmbeddingSet,
    new_embedder: Arc<EmbeddingProcessor>,
) -> Result<(), EmbedError>
```

**Purpose:** Swap both the active embedding set and embedder, updating the database schema for the new set before making it visible to other components.

#### 3.5 `EmbeddingRuntime::generate_embeddings_with_cancel`

**Signature:**
```rust
#[instrument(skip_all, fields(snippet_count = snippets.len()))]
pub async fn generate_embeddings_with_cancel(
    &self,
    snippets: Vec<String>,
    cancel: Option<&CancellationListener>,
) -> Result<Vec<Vec<f32>>, EmbedError>
```

**Purpose:** Delegate embedder methods through the runtime so existing call-sites can share a single handle while allowing hot-swaps.

---

### 4. CancellationToken

**Path:** `ploke_embed::cancel_token`

#### 4.1 `CancellationToken::new`

**Signature:**
```rust
pub fn new() -> (Self, CancellationHandle)
```

**Output:** 
- `CancellationToken` - The token to check for cancellation
- `CancellationHandle` - Handle to trigger cancellation

**Example Usage:**
```rust
use ploke_embed::cancel_token::CancellationToken;

let (token, handle) = CancellationToken::new();
let listener = token.listener();

// Later, to cancel:
handle.cancel();

// Check if cancelled:
if listener.is_cancelled() {
    // Handle cancellation
}
```

#### 4.2 `CancellationToken::listener`

**Signature:**
```rust
pub fn listener(&self) -> CancellationListener
```

**Purpose:** Create a clonable listener that can be shared across tasks.

#### 4.3 `CancellationListener`

**Methods:**
```rust
// Check if cancellation has been requested
pub fn is_cancelled(&self) -> bool

// Wait asynchronously until cancellation is requested
pub async fn cancelled(&self)
```

**Special Considerations:**
- `CancellationListener` is `Clone`, can be shared across async tasks
- If the `CancellationHandle` is dropped, `cancelled()` returns immediately
- Uses `tokio::sync::watch` internally

#### 4.4 `CancellationHandle::cancel`

**Signature:**
```rust
pub fn cancel(&self)
```

**Purpose:** Signal cancellation to all associated tokens.

---

### 5. Configuration Types

#### 5.1 `OpenRouterConfig`

**Path:** `ploke_embed::config::OpenRouterConfig`

```rust
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct OpenRouterConfig {
    /// OpenRouter model id, e.g. `openai/text-embedding-3-small`.
    pub model: String,
    /// Expected embedding dimension.
    pub dimensions: Option<usize>,
    /// Optional router-specific `dimensions` request parameter (OpenRouter-side truncation).
    pub request_dimensions: Option<usize>,
    /// Max snippets per OpenRouter embeddings API request.
    #[serde(default = "default_openrouter_snippet_batch_size")]
    pub snippet_batch_size: usize,
    /// Max in-flight embedding requests.
    #[serde(default = "default_openrouter_max_in_flight")]
    pub max_in_flight: usize,
    /// Optional requests/second cap.
    pub requests_per_second: Option<u32>,
    /// Max attempts for 429/529 retry.
    #[serde(default = "default_openrouter_max_attempts")]
    pub max_attempts: u32,
    /// Initial backoff in milliseconds.
    #[serde(default = "default_openrouter_initial_backoff_ms")]
    pub initial_backoff_ms: u64,
    /// Max backoff in milliseconds.
    #[serde(default = "default_openrouter_max_backoff_ms")]
    pub max_backoff_ms: u64,
    /// Optional hint to OpenRouter about the input type.
    pub input_type: Option<String>,
    /// Per-request timeout in seconds for embeddings.
    #[serde(default = "default_openrouter_timeout_secs")]
    pub timeout_secs: u64,
    /// Controls how the backend handles overlong snippets.
    #[serde(default)]
    pub truncate_policy: TruncatePolicy,
}
```

**Default Values:**
- `snippet_batch_size`: 100
- `max_in_flight`: 2
- `max_attempts`: 5
- `initial_backoff_ms`: 250
- `max_backoff_ms`: 10_000
- `timeout_secs`: 30
- `truncate_policy`: TruncatePolicy::Truncate

#### 5.2 `TruncatePolicy`

**Path:** `ploke_embed::config::TruncatePolicy`

```rust
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TruncatePolicy {
    Truncate,    // Truncate snippets to max length
    Reject,      // Error if snippet exceeds max length
    PassThrough, // Send snippets as-is
}
```

#### 5.3 `EmbeddingConfig` (Local)

**Path:** `ploke_embed::local::EmbeddingConfig`

```rust
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    pub model_id: String,           // default: "sentence-transformers/all-MiniLM-L6-v2"
    pub revision: Option<String>,
    pub device_preference: DevicePreference,  // default: Auto
    pub cuda_device_index: usize,   // default: 0
    pub allow_fallback: bool,       // default: true
    pub approximate_gelu: bool,     // default: false
    pub use_pth: bool,              // default: false
    pub model_batch_size: usize,    // default: 8
    pub max_length: Option<usize>,  // default: None
}
```

#### 5.4 `DevicePreference`

**Path:** `ploke_embed::local::DevicePreference`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize, Serialize)]
pub enum DevicePreference {
    #[default]
    Auto,        // Use GPU if available, fallback to CPU
    ForceCpu,    // Always use CPU
    ForceGpu,    // Use GPU or fail if unavailable (unless allow_fallback)
}
```

#### 5.5 Other Config Types

```rust
// ploke_embed::config::LocalModelConfig
pub struct LocalModelConfig {
    pub model_id: String,
}

// ploke_embed::config::HuggingFaceConfig
pub struct HuggingFaceConfig {
    pub api_key: String,
    pub model: String,
    pub dimensions: usize,
}

// ploke_embed::config::OpenAIConfig
pub struct OpenAIConfig {
    pub api_key: String,
    pub model: String,
}

// ploke_embed::config::CozoConfig
pub struct CozoConfig {
    pub api_key: Option<String>,
}
```

---

### 6. Error Types

**Path:** `ploke_embed::error::EmbedError`

```rust
#[derive(thiserror::Error, Debug, Clone)]
pub enum EmbedError {
    #[error("Snippet fetch failed: {0}")]
    SnippetFetch(#[from] ploke_io::IoError),

    #[error("Embedding computation failed: {0}")]
    Embedding(String),

    #[error("Database operation failed: {0}")]
    Database(#[from] ploke_db::DbError),

    #[error("Local model error: {0}")]
    LocalModel(String),

    #[error("Network Error: {0}")]
    Network(String),

    #[error("Feature not implemented: {0}")]
    NotImplemented(String),

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Cancelled operation: {0}")]
    Cancelled(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Ploke core error: {0}")]
    PlokeCore(#[from] ploke_error::Error),

    #[error("Broadcast send error: {0}")]
    BroadcastSendError(String),

    #[error("HTTP Error {status} at {url}: {body}")]
    HttpError { status: u16, body: String, url: String },

    #[error("Join handle failed for thread: {0}")]
    JoinFailed(String),

    #[error("Runtime state error: {0}")]
    State(String),
}
```

---

## Environment Variable Requirements

### TEST_OPENROUTER_API_KEY

**Requirement:** The embedding command should use `TEST_OPENROUTER_API_KEY` environment variable (different from the `OPENROUTER_API_KEY` used elsewhere).

**Current Implementation Status:**
- Currently, `ploke-llm` uses `OPENROUTER_API_KEY` via `OpenRouter::API_KEY_NAME`
- The `TEST_OPENROUTER_API_KEY` is **NOT** yet implemented in the codebase
- This is a requirement for the xtask command implementation

**Implementation Notes for M.2/M.3:**
- The xtask command should load `TEST_OPENROUTER_API_KEY` from environment
- No overrides should be accepted (by design, per requirements)
- The key will be setup specifically for agents with a budget
- The command should fail gracefully if the key is not set

**How it integrates:**
```rust
// In xtask command, before creating OpenRouterBackend:
let api_key = std::env::var("TEST_OPENROUTER_API_KEY")
    .expect("TEST_OPENROUTER_API_KEY must be set for embedding commands");

// Pass to OpenRouterEmbedEnv
let env = OpenRouterEmbedEnv::from_parts(api_key, url);
let backend = OpenRouterBackend::new_with_env(&config, env)?;
```

**Related types in ploke-llm:**
- `OpenRouterEmbedEnv::from_parts(api_key, url)` - Create env with explicit key
- `OpenRouterEmbedEnv::from_env()` - Currently reads from `OPENROUTER_API_KEY`

---

## Tracing Instrumentation Status

### Current Instrumentation

1. **EmbeddingProcessor::generate_embeddings_with_cancel**
   - `#[instrument(skip_all, fields(source = ?self.source, target = "embed-pipeline"))]`

2. **IndexerTask::run**
   - `#[instrument(name = "Indexer::run", skip(self, progress_tx, control_rx), fields(num_not_proc, recent_processed, status="Running"))]`

3. **IndexerTask::next_batch**
   - `#[instrument(skip_all, fields(total_counted, num_not_proc, recent_processed, status="Running", batch_size))]`

4. **IndexerTask::process_batch**
   - `#[instrument(skip_all, fields(batch_size))]`

5. **EmbeddingRuntime::generate_embeddings_with_cancel**
   - `#[instrument(skip_all, fields(snippet_count = snippets.len()))]`

6. **OpenRouterBackend::compute_batch**
   - `#[instrument(skip_all, fields(expected_len), target = "embed-pipeline")]`

7. **LocalEmbedder::process_batch**
   - `#[instrument(skip(self, texts), fields(model_batch_size, cursor), level = "DEBUG")]`

### Trace Targets Used
- `"embed-pipeline"` - Main embedding pipeline events
- `"ploke-embed::next_batch"` - Batch retrieval diagnostics

---

## Key Dependencies for xtask Commands

### For `ingest embed` command:

```rust
use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource, IndexerTask, IndexStatus, IndexerCommand};
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_embed::cancel_token::{CancellationToken, CancellationHandle, CancellationListener};
use ploke_embed::config::OpenRouterConfig;
use ploke_embed::local::{LocalEmbedder, EmbeddingConfig};
use ploke_embed::error::EmbedError;
use ploke_db::Database;
use ploke_io::IoManagerHandle;
```

### Required External Types:
- `ploke_db::Database` - Database handle
- `ploke_db::CallbackManager` - For DB change callbacks
- `ploke_db::bm25_index::bm25_service` - For BM25 sparse indexing
- `ploke_io::IoManagerHandle` - For file I/O
- `ploke_core::embeddings::EmbeddingSet` - Active embedding set
- `tokio::sync::{broadcast, mpsc}` - For channels

---

## Issues Encountered

1. **TEST_OPENROUTER_API_KEY not implemented**
   - The requirement states embedding commands should use `TEST_OPENROUTER_API_KEY`
   - Currently, the codebase only uses `OPENROUTER_API_KEY`
   - This will need to be handled in the xtask command layer

2. **Complex setup requirements**
   - `IndexerTask` requires many components: Database, IoManagerHandle, EmbeddingRuntime, CancellationToken, CallbackManager
   - Proper channel setup (broadcast for progress, mpsc for control) is critical
   - BM25 service is optional but recommended

3. **Async runtime required**
   - All embedding operations are async
   - Requires tokio runtime
   - Cancellation handling uses tokio::sync::watch internally

4. **Database schema setup**
   - `IndexerTask::run` calls `ensure_embedding_set_relation()` and `ensure_vector_embedding_relation()`
   - These modify the database schema
   - Active embedding set must be initialized before running

---

## Summary for M.2 Architecture

The ploke_embed crate provides:

1. **EmbeddingProcessor** - Core embedding generation with multiple backends (Local, OpenRouter, OpenAI, HuggingFace)
2. **IndexerTask** - High-level indexing workflow that fetches unembedded nodes, generates embeddings, and updates database
3. **EmbeddingRuntime** - Shared runtime handle for active embedding set with hot-swap support
4. **CancellationToken** - Cooperative cancellation for long-running embedding operations
5. **Configuration types** - Comprehensive config structs for different backends

For the xtask `ingest embed` command:
- Use `TEST_OPENROUTER_API_KEY` (new requirement)
- Create `EmbeddingProcessor` → `EmbeddingRuntime` → `IndexerTask`
- Set up channels for progress reporting and control
- Handle async execution in tokio runtime
- Consider adding cancellation support via CLI (Ctrl+C handling)
