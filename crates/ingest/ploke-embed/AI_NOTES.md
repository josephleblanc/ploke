# `ploke-embed`: AI Developer Notes

This document provides a dense, technical overview of the `ploke-embed` crate's API for developers integrating it into other parts of the Ploke system (e.g., the TUI).

## Core API: `IndexerTask`

The primary entry point for using this crate is the `ploke_embed::indexer::IndexerTask`. It is an actor designed to run as a background task, responsible for the entire process of embedding a codebase.

### Construction

An `IndexerTask` is created using `IndexerTask::new()` and configured with `with_bm25_tx()`.

```rust
// Simplified example
use ploke_embed::indexer::{IndexerTask, EmbeddingProcessor, EmbeddingSource};
use ploke_embed::local::LocalEmbedder;
use ploke_embed::cancel_token::CancellationToken;
use ploke_db::Database;
use ploke_io::IoManagerHandle;
use std::sync::Arc;

// 1. Setup dependencies
let db: Arc<Database> = ...;
let io: IoManagerHandle = ...;
let (cancellation_token, cancel_handle) = CancellationToken::new();
let bm25_tx = ...; // Sender for the BM25 service

// 2. Choose and configure an embedding source
let local_embedder = LocalEmbedder::new(Default::default())?;
let source = EmbeddingSource::Local(local_embedder);
let embedding_processor = Arc::new(EmbeddingProcessor::new(source));

// 3. Create the task
let indexer_task = IndexerTask::new(
    db,
    io,
    embedding_processor,
    cancellation_token,
    8, // batch_size
)
.with_bm25_tx(bm25_tx);
```

### Execution

The task is executed by calling the `run` method. This is a long-running future that should be spawned onto a Tokio runtime. The `indexer::index_workspace` function provides a complete example of how to manage the task's lifecycle, including handling callbacks and shutdown signals.

```rust
// Simplified example
let (progress_tx, progress_rx) = tokio::sync::broadcast::channel(100);
let (control_tx, control_rx) = tokio::sync::mpsc::channel(4);

let task_handle = tokio::spawn(async move {
    indexer_task.run(Arc::new(progress_tx), control_rx).await
});

// Now monitor progress_rx and send commands via control_tx
```

## Configuration

### Embedding Backends (`EmbeddingSource`)

The `EmbeddingProcessor` is configured with an `EmbeddingSource` enum, which determines where embeddings are generated.

-   `EmbeddingSource::Local(LocalEmbedder)`: Uses a local sentence-transformer model via `candle`.
-   `EmbeddingSource::OpenAI(OpenAIBackend)`: Uses the OpenAI Embeddings API.
-   `EmbeddingSource::HuggingFace(HuggingFaceBackend)`: Uses the HuggingFace Inference API.
-   `EmbeddingSource::Cozo(CozoBackend)`: Placeholder, not yet implemented.

### Backend-Specific Configuration

-   **Local (`local::EmbeddingConfig`)**:
    -   `model_id`: The HuggingFace model to use (e.g., `"sentence-transformers/all-MiniLM-L6-v2"`).
    -   `device_preference`: `Auto`, `ForceCpu`, `ForceGpu`.
    -   `model_batch_size`: Number of snippets to process at once on the local device.
-   **OpenAI (`config::OpenAIConfig`)**:
    -   `api_key`: Your OpenAI API key.
    -   `model`: The model name (e.g., `"text-embedding-ada-002"`).
-   **HuggingFace (`config::HuggingFaceConfig`)**:
    -   `api_key`: Your HuggingFace API token.
    -   `model`: The model name.
    -   `dimensions`: The output dimension size of the model.

## Monitoring and Control

Interaction with a running `IndexerTask` happens over two channels.

### 1. Progress Tracking (`broadcast::Receiver<IndexingStatus>`)

The task broadcasts `IndexingStatus` updates. Subscribe to the receiver to monitor the state.

-   **`IndexingStatus` struct**:
    -   `status: IndexStatus`: The current state (`Idle`, `Running`, `Paused`, `Completed`, `Failed`, `Cancelled`).
    -   `num_not_proc: usize`: Total number of items to be indexed.
    -   `recent_processed: usize`: Number of items processed since the task started.
    -   `current_file: Option<PathBuf>`: The file currently being processed.
    -   `errors: Vec<String>`: A list of non-fatal errors encountered.

### 2. Task Control (`mpsc::Sender<IndexerCommand>`)

Send `IndexerCommand` messages to the task to change its state.

-   **`IndexerCommand` enum**:
    -   `Pause`: Pauses processing after the current batch.
    -   `Resume`: Resumes a paused task.
    -   `Cancel`: Stops the task gracefully.

## Key System Interactions

-   **`ploke-db::Database`**: The `IndexerTask` reads nodes where `embedding is null` and writes back computed vectors using `update_embeddings_batch`. It does **not** perform any parsing or initial data insertion.
-   **`ploke-io::IoManagerHandle`**: Required to fetch the text content (snippets) of code nodes from disk before they can be embedded.
-   **`ploke-db::bm25_service`**: The task simultaneously seeds a BM25 sparse index by sending `DocData` to the service. This is for hybrid search capabilities. The dense embedding process will wait for the BM25 service to acknowledge finalization before marking itself as `Completed`.

## Error Handling

-   The `IndexerTask::run` method returns a `Result<(), EmbedError>`. A fatal error (e.g., configuration issue, unrecoverable network error) will cause the future to resolve with an `Err`.
-   Transient errors (e.g., a single failed API call for a batch) are logged, added to `IndexingStatus.errors`, and the task continues with the next batch.
