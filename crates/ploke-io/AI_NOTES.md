# `ploke-io` Crate Analysis

-   **Review Date**: 2025-08-15
-   **Summary**: `ploke-io` is an actor-based asynchronous file I/O crate for batch-reading file snippets. Its core strength is isolating I/O in a dedicated thread, preventing the main application from blocking. However, it suffers from performance inefficiencies due to excessive parsing, complex internal logic, and brittle hash verification.

---

## 1. Architectural Review

### Strengths

-   **Actor Model**: The use of a dedicated thread with a Tokio runtime for the `IoManager` is an excellent design choice. It isolates I/O-bound work and provides a clean, non-blocking API (`IoManagerHandle`) to the rest of the application.
-   **Resource Management**: Dynamically setting the concurrency semaphore limit based on `rlimit` is a robust way to prevent file descriptor exhaustion while maximizing parallelism.
-   **Data Integrity**: Using `syn`-based token stream hashing (`TrackingHash`) ensures that checks are resilient to whitespace and comment changes, focusing only on functional code modifications.

### Weaknesses

-   **Performance Bottlenecks**: The current implementation re-reads and re-parses entire files for every batch request. Since parsing with `syn` is computationally expensive, this is a major performance issue.
-   **Complex Logic**: Key functions like `handle_read_snippet_batch` and `process_file` are overly complex, with convoluted error handling and result-ordering logic that is difficult to follow and maintain.
-   **Brittle API Contracts**: The `get_snippets_batch` implementation assumes all `EmbeddingData` for the same file carry the same `file_tracking_hash`, using the first one it finds (`requests[0]`) for verification. This is a latent bug waiting to happen.

---

## 2. Identified Issues & Recommendations

### High-Priority Issues

#### 1. Inefficient Hashing and Parsing

-   **Issue**: `process_file` calls `tokio::fs::read` and `syn::parse_file` for every batch of requests targeting a file. If ten batches request snippets from `lib.rs` in quick succession, the file is read and parsed ten times.
-   **Recommendation**: Implement a cache for file contents and/or tracking hashes.
    -   **Option A (Content Cache)**: Use an LRU cache (e.g., `lru::LruCache`) to store `(PathBuf, Arc<String>)`. This avoids disk reads.
    -   **Option B (Hash Cache)**: Cache the computed `TrackingHash` keyed by `PathBuf` and file modification time. This avoids re-reading and re-parsing if the file hasn't changed. This is the more comprehensive solution.

#### 2. Flawed Hash Verification

-   **Issue**: The line `if actual_tracking_hash != requests[0].request.file_tracking_hash` in `process_file` is incorrect. It assumes all requests in a batch for a single file have the same hash, which may not be true if the data comes from a stale cache.
-   **Recommendation**: The hash check should be performed per-request, not per-file. The logic should iterate through the requests and check each `request.file_tracking_hash` against the `actual_tracking_hash`. This would correctly generate `ContentMismatch` errors for individual requests that are out of date.

#### 3. `process_file` Complexity

-   **Issue**: The function is over 100 lines long and contains multiple levels of indentation, manual error propagation, and duplicated logic for creating error variants.
-   **Recommendation**: Refactor `process_file` into smaller, single-purpose functions:
    1.  `read_and_verify_hash(path, expected_hash) -> Result<String, IoError>`
    2.  `extract_snippet(content, start, end) -> Result<String, IoError>`
    -   This would allow for the use of `?` and make the main loop much cleaner.

### Medium-Priority Issues

#### 1. Result Re-ordering Logic

-   **Issue**: `handle_read_snippet_batch` uses a complex method of collecting results into a vector of tuples, sorting it, and then rebuilding the final vector. The code contains a `TODO` acknowledging this complexity.
-   **Recommendation**: Pre-allocate the results vector and insert results directly at their original index.
    ```rust
    // In handle_read_snippet_batch
    let mut final_results: Vec<Option<Result<String, PlokeError>>> = vec![None; total_requests];
    // ... in the loop collecting task results
    for (idx, result) in file_results {
        final_results[idx] = Some(result);
    }
    // After loop, convert Vec<Option<...>> to Vec<Result<...>>
    let final_results = final_results.into_iter().map(|opt| opt.unwrap_or_else(|| Err(/* ... */))).collect();
    ```

### Low-Priority Issues

#### 1. Inconsistent Error Handling

-   **Issue**: The conversion from `RecvError` to `IoError` is commented out, and the call sites handle the mapping to `PlokeError` manually. This is inconsistent with how other errors are handled.
-   **Recommendation**: Uncomment the `From<RecvError> for IoError` implementation and streamline the error chain in `IoManagerHandle`.

---

## 3. Future Development Strategy

1.  **Stabilize**: First, address the high-priority issues: refactor `process_file`, fix the hash verification bug, and implement caching. This will make the existing system robust and performant.
2.  **Expand**: Introduce a file watcher. This will likely require a new actor or a long-running task within `IoManager` that communicates with the rest of the system via channels, signaling file changes.
3.  **Extend**: Add write capabilities. This will require a new set of `IoRequest` variants and handler logic, with careful consideration for transactional writes and backups.
