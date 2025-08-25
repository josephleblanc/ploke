# ploke-error v3 — Before/After Examples

This document shows how the new, policy-driven error design reduces boilerplate and clarifies control-flow across some of the most error-heavy areas of the codebase. These examples are illustrative and forward-looking; they assume the v3 additions described in V3_PLAN.md (Result alias, DomainError, Severity, ErrorPolicy, ContextExt, ResultExt).

Conventions used below:
- use ploke_error::{Error, Result, DomainError, FatalError, InternalError, WarningError}
- use ploke_error::{ContextExt, ErrorPolicy, ResultExt}

---

## 1) ploke-io/src/actor.rs — concurrent read snippet batch

Before (hand-rolled grouping, task spawn/join mapping, manual error plumbing):

```rust
pub async fn handle_read_snippet_batch(
    requests: Vec<EmbeddingData>,
    semaphore: Arc<Semaphore>,
) -> Vec<Result<String, PlokeError>> {
    let total_requests = requests.len();
    let ordered_requests = requests.into_iter().enumerate().map(|(idx, request)| OrderedRequest { idx, request });
    let mut requests_by_file: HashMap<PathBuf, Vec<OrderedRequest>> = HashMap::new();
    for ordered_req in ordered_requests {
        requests_by_file.entry(ordered_req.request.file_path.clone()).or_default().push(ordered_req);
    }

    let file_tasks = requests_by_file.into_iter().map(|(path, reqs)| {
        let semaphore = semaphore.clone();
        tokio::spawn(async move { Self::process_file(path, reqs, semaphore).await })
    });

    let mut final_results: Vec<Option<Result<String, PlokeError>>> = vec![None; total_requests];

    for task in join_all(file_tasks).await {
        match task {
            Ok(file_results) => {
                for (idx, res) in file_results {
                    if idx < final_results.len() {
                        final_results[idx] = Some(res);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("[ploke-io] FATAL: File processing task panicked: {:?}", e);
            }
        }
    }

    final_results
        .into_iter()
        .map(|opt| {
            opt.unwrap_or_else(|| {
                Err(ploke_error::InternalError::InvalidState("Result missing for request").into())
            })
        })
        .collect()
}
```

After (utility encapsulates ordering and join; error propagation via `?`; emissions via policy at the edge if desired):

```rust
use ploke_error::{Result, ErrorPolicy, ResultExt};

pub async fn handle_read_snippet_batch(
    requests: Vec<EmbeddingData>,
    semaphore: Arc<Semaphore>,
    policy: &impl ErrorPolicy, // optional: app supplies it
) -> Result<Vec<Result<String>>> {
    // Utility handles grouping by path, spawning, and preserving order internally.
    let results = bounded_by_file(requests, semaphore, |path, reqs| async move {
        Self::process_file(path, reqs).await
    })
    .await? // channel/join failures map via From into ploke_error::Error
    .emit_event(policy); // policy-optional emission without contaminating flow

    Ok(results)
}
```

Benefits:
- No manual join/match boilerplate.
- Clear return type and single `?` for fatal concurrency failures.
- Emission/logging is policy-driven at the boundary, not interleaved with logic.

---

## 2) ploke-io/src/write.rs — write path and conversions

Before (manual conversions and repeated mapping; eager context):

```rust
async fn process_one_write(req: WriteSnippetData, roots: Option<Arc<Vec<PathBuf>>>, symlink_policy: Option<SymlinkPolicy>) -> Result<WriteResult, IoError> {
    let file_path = if let Some(roots) = roots.as_ref() {
        // ...
    } else {
        if !req.file_path.is_absolute() {
            return Err(IoError::FileOperation { operation: "write", path: req.file_path.clone(), kind: std::io::ErrorKind::InvalidInput, source: Arc::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, "path must be absolute",)), });
        }
        req.file_path.clone()
    };

    let _write_lock_guard = get_file_lock(&file_path).await.lock().await;

    let content = read_file_to_string_abs(&file_path).await?;
    // ... compute/compare hashes; return IoError variants on mismatch/range/char boundary ...
    // temp write, fsync, rename with per-call mapping of std::io::Error into IoError::FileOperation
    Ok(WriteResult::new(new_hash))
}
```

After (standard From conversions once; `Result` alias and `?` everywhere; lazy context via ContextExt when helpful):

```rust
use ploke_error::{Result, FatalError, ContextExt};

async fn process_one_write(
    req: WriteSnippetData,
    roots: Option<Arc<Vec<PathBuf>>>,
    symlink_policy: Option<SymlinkPolicy>,
) -> Result<WriteResult> {
    let file_path = normalize_for_write(&req.file_path, roots.as_deref(), symlink_policy)
        .with_path(req.file_path.clone())?; // attach path only if normalization fails

    let _guard = get_file_lock(&file_path).lock().await;

    let content = read_file_to_string_abs(&file_path).await?; // std::io::Error -> FatalError via From
    let actual = compute_tracking_hash(&content, &file_path, req.namespace)?; // domain-specific errors -> Fatal/Internal via From

    ensure_expected_hash(actual, req.expected_file_hash, &req)?; // returns Result<()>

    let new_content = splice_utf8(&content, req.start_byte, req.end_byte, &req.replacement, &file_path)?; // boundary checks inside

    let new_hash = compute_tracking_hash(&new_content, &file_path, req.namespace)?;

    atomic_write(&file_path, new_content).await?; // encapsulates temp, fsync, rename; uses `?`

    Ok(WriteResult::new(new_hash))
}
```

Benefits:
- One consistent mapping layer via `From` implementations; no per-site error construction.
- Context (`with_path`) is opt-in and lazy.
- File I/O, splice, and hashing compose naturally with `?`.

---

## 3) ploke-rag/src/core/mod.rs — BM25 with timeout/fallback

Before (repeated timeout handling and stringy error mapping):

```rust
let (tx, rx) = oneshot::channel();
self.bm_embedder.send(Bm25Cmd::Search { query: query.to_string(), top_k, resp: tx }).await
    .map_err(|e| RagError::Channel(format!("failed to send BM25 search command (len={}, top_k={}): {}", query.len(), top_k, e)))?;

let res = match timeout(Duration::from_millis(self.cfg.bm25_timeout_ms), rx).await {
    Ok(Ok(r)) => r,
    Ok(Err(recv_err)) => {
        return Err(RagError::Channel(format!(
            "BM25 search response channel closed (len={}, top_k={}): {}",
            query.len(), top_k, recv_err
        )))
    }
    Err(_) => {
        return Err(RagError::Channel(format!(
            "timeout waiting for BM25 search ({} ms)", self.cfg.bm25_timeout_ms
        )))
    }
};
```

After (unified helper; structured Domain/Internal mapping; `?`-first):

```rust
use ploke_error::{Result, DomainError, InternalError};

async fn bm25_call<T>(&self, cmd: Bm25Cmd, timeout_ms: u64) -> Result<T>
where
    T: Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    self.bm_embedder.send(cmd.with_resp(tx)).await
        .map_err(|e| InternalError::CompilerError(format!("bm25 send failed: {e}")))?;
    timeout(Duration::from_millis(timeout_ms), rx).await
        .map_err(|_| InternalError::CompilerError("bm25 timeout".into()))?
        .map_err(|e| InternalError::CompilerError(format!("bm25 channel closed: {e}")) )
        .map_err(Into::into)
}

pub async fn search_bm25(&self, query: &str, top_k: usize) -> Result<Vec<(Uuid, f32)>> {
    let res: Vec<(Uuid, f32)> = self.bm25_call(Bm25Cmd::search(query, top_k), self.cfg.bm25_timeout_ms).await?;
    if res.is_empty() && !self.bm25_ready_or_nonempty().await? {
        // Fallback to dense as a domain-level decision:
        return self.search(query, top_k).await.map_err(|e| DomainError::Rag { message: format!("dense fallback failed: {e}") }.into());
    }
    Ok(res)
}
```

Benefits:
- Eliminates repeated timeout/channel mapping.
- Clear separation between Internal (infra failures) and Domain (search/fallback logic).

---

## 4) ploke-tui/src/app/events.rs — user-facing error emission

Before (embed strings and push messages inside the event handler):

```rust
AppEvent::Error(error_event) => {
    let msg = format!("Error: {}", error_event.message);
    app.send_cmd(StateCommand::AddMessageImmediate {
        msg,
        kind: MessageKind::SysInfo,
        new_msg_id: Uuid::new_v4(),
    });
}
```

After (policy-driven rendering; optional rich diagnostics with miette):

```rust
use ploke_error::{Error, ErrorPolicy};

AppEvent::Error(err) => {
    // err is or can be converted into ploke_error::Error
    let e: Error = err.into();
    // The policy decides severity and how to emit (tracing/miette/event-bus).
    app.error_policy.emit(&e);
    // UI remains free of ad-hoc stringification; rendering is centralized and consistent.
}
```

Benefits:
- Moves formatting and severity decisions to a single policy.
- UI logic is simplified and consistent across subsystems.

---

## 5) Optional: contextual snippets when it matters (opt-in)

Before (eager backtrace capture in ErrorContext::new, rarely used):

```rust
let ctx = ErrorContext::new(path.clone()); // always captures backtrace
return Err(ContextualError::WithContext { source: Box::new(e.into()), context: ctx }.into());
```

After (lazy, opt-in context via extension methods):

```rust
use ploke_error::ContextExt;

read_file_to_string_abs(&file_path)
    .await
    .with_path(file_path.clone())?; // captures path only on error; no backtrace unless requested
```

Benefits:
- No overhead on happy path.
- Richer, targeted context where it helps.

---

These examples show how the v3 design consolidates error mapping, defers presentation to policies, and dramatically reduces boilerplate while preserving or increasing functionality and diagnosability.
