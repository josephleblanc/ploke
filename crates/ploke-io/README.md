# `ploke-io`: Asynchronous File I/O Actor

`ploke-io` provides a non-blocking I/O actor system for reading file snippets concurrently. It is designed for applications that need to read from many files without blocking, ensuring that file content has not changed since it was last indexed.

## Architecture

The crate uses an actor model to isolate file I/O from the main application logic. This prevents blocking the caller's thread, which is critical in applications with their own async runtimes (like a TUI or web server).

### High-Level Diagram

```mermaid
graph TD
    subgraph Application
        ClientCode["Client Code (e.g., Indexer, TUI)"]
    end

    subgraph ploke-io
        Handle["IoManagerHandle (Cloneable API)"]
        Actor["IoManager (Dedicated Thread)"]
    end

    subgraph System
        FS["File System"]
    end

    ClientCode --"get_snippets_batch(requests)"--> Handle
    Handle --"IoManagerMessage::Request"--> Actor
    Actor --"Reads files"--> FS
    Actor --"oneshot::Sender"--> ClientCode
```

### Detailed Data Flow (`get_snippets_batch`)

```mermaid
sequenceDiagram
    participant Client
    participant Handle as IoManagerHandle
    participant Actor as IoManager
    participant FS as File System

    Client->>+Handle: get_snippets_batch(requests)
    Handle->>Handle: Creates oneshot channel
    Handle->>Actor: Sends IoManagerMessage::Request(ReadSnippetBatch)
    Note over Actor: Receives message in main loop

    Actor->>Actor: Groups requests by file_path
    loop For each file
        Actor->>Actor: Spawns tokio task to process_file()
    end

    par For each file
        Actor->>Actor: Acquires Semaphore permit
        Actor->>FS: Reads entire file content
        FS-->>Actor: Returns file bytes
        Actor->>Actor: Verifies content hash against request
        Note right of Actor: If hash mismatches, returns ContentMismatch error
        Actor->>Actor: Extracts all requested snippets
        Actor->>Actor: Releases Semaphore permit
    end

    Actor->>Actor: Collects and re-orders results
    Actor->>Client: Sends Vec<Result<String, PlokeError>> via oneshot channel
    Handle-->>-Client: Returns results
```

## Decisions and Defaults

- Symlink Policy: Default is DenyCrossRoot. Configure via IoManagerBuilder::with_symlink_policy; strict canonicalization is enforced before root containment checks.
- Watcher: Feature-gated (watcher). Debounce interval configurable via builder. Roots are taken from builder at startup; runtime root changes are planned.
- Write Durability: Atomic temp write + fsync + rename, with best-effort parent directory fsync by default on Linux targets.
- Concurrency: Effective permits derive from soft NOFILE limit with precedence builder > env (PLOKE_IO_FD_LIMIT, clamped 4..=1024) > heuristic > default 50.
- Error Policy: Channel/shutdown map to Internal errors; file/parse/path-policy/durability issues map to Fatal. Warnings may be emitted for suboptimal but successful operations.
- Platform Scope: Linux first; macOS/Windows support planned with documented caveats.

## API Usage

The primary entry point is `IoManagerHandle`, which can be cloned and shared across threads.

### Initialization

Create a handle to spawn the background I/O actor.

```rust
use ploke_io::IoManagerHandle;

let io_manager = IoManagerHandle::new();
```

### Reading Snippets

To read one or more code snippets, create a `Vec<EmbeddingData>` and pass it to `get_snippets_batch`. The results are returned in the same order as the requests.

```rust
# use ploke_core::{EmbeddingData, TrackingHash};
# use std::path::PathBuf;
# use uuid::Uuid;
# use ploke_io::IoManagerHandle;
#
# async fn example() {
# let io_manager = IoManagerHandle::new();
# let file_path = PathBuf::from("src/lib.rs");
# let content = "fn hello() {}";
# let file_tracking_hash = TrackingHash::generate(Uuid::nil(), &file_path, &content.parse().unwrap());
let requests = vec![
    EmbeddingData {
        id: Uuid::new_v4(),
        file_path: file_path.clone(),
        file_tracking_hash,
        start_byte: 3,
        end_byte: 8,
        // ... other fields
#       name: "hello".into(),
#       namespace: Uuid::nil(),
#       node_tracking_hash: file_tracking_hash,
    },
    // ... more requests
];

match io_manager.get_snippets_batch(requests).await {
    Ok(results) => {
        for result in results {
            match result {
                Ok(snippet) => println!("Retrieved snippet: {}", snippet),
                Err(e) => eprintln!("Failed to get snippet: {:?}", e),
            }
        }
    }
    Err(e) => eprintln!("Batch request failed: {:?}", e),
}
# }
```

### Writing Snippets

Use write_snippets_batch to apply an in-place UTF-8 splice with atomic durability steps. The write verifies the expected file hash to ensure atomicity against concurrent external edits.

```rust
# use ploke_core::{WriteSnippetData, PROJECT_NAMESPACE_UUID};
# use ploke_io::IoManagerHandle;
# use uuid::Uuid;
# use std::path::PathBuf;
# async fn example() {
# let dir = tempfile::tempdir().unwrap();
# let file_path = dir.path().join("example.rs");
# std::fs::write(&file_path, "fn foo() {}\n").unwrap();
# let namespace = PROJECT_NAMESPACE_UUID;
# let expected = {
#   let file = syn::parse_file("fn foo() {}\n").unwrap();
#   let tokens = file.into_token_stream();
#   ploke_core::TrackingHash::generate(namespace, &file_path, &tokens)
# };
let start = 3; // byte offsets on UTF-8 boundaries
let end = 6;

let req = WriteSnippetData {
    id: Uuid::new_v4(),
    name: "rename_fn".into(),
    file_path: file_path.clone(),
    expected_file_hash: expected,
    start_byte: start,
    end_byte: end,
    replacement: "bar".into(),
    namespace,
};

let handle = IoManagerHandle::new();
let results = handle.write_snippets_batch(vec![req]).await.unwrap();
match &results[0] {
    Ok(write_result) => {
        println!("New file hash: {:?}", write_result.new_file_hash);
    }
    Err(e) => eprintln!("Write failed: {e:?}"),
}
handle.shutdown().await;
# }
```

### Scanning for Changes

To check if files have been modified since they were last indexed, use `scan_changes_batch`. It returns a list of files whose content hash has changed.

```rust
# use ploke_core::{FileData, TrackingHash};
# use std::path::PathBuf;
# use uuid::Uuid;
# use ploke_io::IoManagerHandle;
#
# async fn example() {
# let io_manager = IoManagerHandle::new();
# let file_path = PathBuf::from("src/lib.rs");
# let content = "fn hello() {}";
# let file_tracking_hash = TrackingHash::generate(Uuid::nil(), &file_path, &content.parse().unwrap());
let files_to_check = vec![
    FileData {
        file_path,
        file_tracking_hash,
        namespace: Uuid::nil(),
    },
    // ... more files
];

match io_manager.scan_changes_batch(files_to_check).await {
    Ok(Ok(changed_files)) => {
        for changed in changed_files.into_iter().flatten() {
            println!("File changed: {}", changed.file_path.display());
        }
    }
    _ => eprintln!("Failed to scan for changes"),
}
# }
```

### Shutdown

To gracefully shut down the actor, call `shutdown()` and await its completion.

```rust
# use ploke_io::IoManagerHandle;
# async fn example() {
# let io_manager = IoManagerHandle::new();
io_manager.shutdown().await;
# }
```

## Analysis and Future Development

### 1. Areas for Expansion

-   **Write Operations**: Completed for in-place edits with verification and atomic rename. Next steps: a distinct file-creation path, optional origin correlation id, and optional OS advisory locks (behind a feature) if needed.
-   **Content Caching**: Deferred. Add Criterion benchmarks to measure baseline performance before introducing an optional, bounded LRU.
-   **Configuration**: IoManagerBuilder exists with with_semaphore_permits, with_fd_limit, with_roots, with_symlink_policy, and watcher toggles. Consider exposing additional knobs only as needed.

### 2. Impact of a File Watcher

Integrating a file watcher (e.g., using the `notify` crate) would enable proactive change detection, shifting the system from a pull-based model (`scan_changes_batch`) to a push-based one.

-   The `IoManager` would need to manage the watcher and translate its events into system-wide notifications (e.g., "file X has changed").
-   This would allow for real-time updates to the code graph and embeddings, making the RAG system more responsive.
-   The `scan_changes_batch` method might become obsolete or serve as a fallback for systems without efficient file watching.

### 3. Areas for Refactoring

-   **`process_file` Complexity**: The `process_file` function in `src/lib.rs` is overly complex, handling file reading, parsing, hash verification, and snippet extraction in one large block with multiple exit points. It should be broken down into smaller, testable functions.
-   **Hash Verification Logic**: Each request is verified against a fresh per-file TrackingHash computed once per file; keep this behavior and continue to simplify the surrounding code paths.
-   **Error Handling**: The conversion from the internal `IoError` to the workspace-wide `ploke_error::Error` is verbose and could be streamlined. Error propagation within `process_file` is also repetitive and could be simplified using the `?` operator.

## Project Plan and Implementation Logs

To move ploke-io to production readiness, see the phased roadmap and procedures:

- Production Plan: crates/ploke-io/docs/production_plan.md
- Implementation Logs (2-log window): crates/ploke-io/docs/implementation-log-000.md (newest first; keep only the latest two logs)

Implementation process guidelines:

- Each cohesive change should add a new implementation-log-NNN.md documenting rationale, summary of changes, verification, and next steps (referencing the plan).
- Maintain a two-log window by removing the oldest log whenever a new one is added.
- Keep PRs small, add/update tests with each change, and update docs alongside code.
