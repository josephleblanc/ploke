use super::*;
use ploke_core::{WriteResult, WriteSnippetData};

/**
A handle to the IoManager actor.

This is the primary public-facing API. It spawns and communicates with an internal
actor running on a dedicated Tokio runtime thread. Cloneable and cheap to pass.

Typical usage:
- Call `IoManagerHandle::new()` for defaults, or use `IoManagerHandle::builder()` for config.
- Use `get_snippets_batch` to read UTF-8 safe slices with per-request hash verification.
- Use `scan_changes_batch` to recompute hashes and detect changed files.
- Use `write_snippets_batch` to apply atomic edits (splice + fsync + rename).

Notes
- When roots are configured via the builder, all operations enforce absolute paths and
  root containment using the configured symlink policy (default: DenyCrossRoot).
- Shutdown is cooperative; pending operations complete where possible.

Example (builder)
```rust,ignore
use ploke_io::IoManagerHandle;
use ploke_io::path_policy::SymlinkPolicy;

let handle = IoManagerHandle::builder()
    .with_fd_limit(64)
    .with_roots([std::env::current_dir().unwrap()])
    .with_symlink_policy(SymlinkPolicy::DenyCrossRoot)
    .build();
# futures::executor::block_on(async { handle.shutdown().await; });
```
*/
#[derive(Clone, Debug)]
pub struct IoManagerHandle {
    /// Channel sender to send requests to the IoManager
    pub(crate) request_sender: mpsc::Sender<IoManagerMessage>,
    #[cfg(feature = "watcher")]
    pub(crate) events_tx: Option<tokio::sync::broadcast::Sender<FileChangeEvent>>,
}

impl Default for IoManagerHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl IoManagerHandle {
    /// Spawns the IoManager on a dedicated runtime thread and returns a handle.
    ///
    /// The default configuration derives a bounded concurrency limit from the OS soft NOFILE
    /// rlimit with a clamp, and does not configure roots or watcher integration.
    ///
    /// For custom configuration (roots, symlink policy, permits, watcher), use `builder()`.
    ///
    /// Example
    /// ```rust,ignore
    /// let handle = ploke_io::IoManagerHandle::new();
    /// // Use the handle, then shutdown to stop the actor thread.
    /// # futures::executor::block_on(async { handle.shutdown().await; });
    /// ```
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);

        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build Tokio runtime"); // TODO: Add proper error handling

            rt.block_on(async {
                let manager = IoManager::new(rx);
                manager.run().await;
            });
        });

        Self {
            request_sender: tx,
            #[cfg(feature = "watcher")]
            events_tx: None,
        }
    }

    /// Create a builder to configure IoManager before starting it.
    ///
    /// Builder precedence:
    /// - with_semaphore_permits > env PLOKE_IO_FD_LIMIT (clamped 4..=1024) > soft NOFILE/3 > 50
    /// - roots may be provided to enforce path containment; symlink policy defaults to DenyCrossRoot.
    ///
    /// ```rust,ignore
    /// use ploke_io::path_policy::SymlinkPolicy;
    /// let handle = ploke_io::IoManagerHandle::builder()
    ///     .with_semaphore_permits(32)
    ///     .with_roots([std::env::current_dir().unwrap()])
    ///     .with_symlink_policy(SymlinkPolicy::DenyCrossRoot)
    ///     .build();
    /// # futures::executor::block_on(async { handle.shutdown().await; });
    /// ```
    pub fn builder() -> IoManagerBuilder {
        IoManagerBuilder::default()
    }

    /// Read a batch of UTF-8-safe snippets from files with per-request hash verification.
    ///
    /// - Preserves input order in the returned vector.
    /// - Each item is `Ok(String)` or `Err(ploke_error::Error)`.
    /// - Errors include content mismatches, invalid ranges, parse failures, and path-policy violations.
    ///
    /// ```rust,ignore
    /// use ploke_core::{EmbeddingData, TrackingHash, PROJECT_NAMESPACE_UUID};
    /// use uuid::Uuid;
    /// use quote::ToTokens;
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let file = dir.path().join("ex.rs");
    /// let src = "fn foo() {}\n";
    /// std::fs::write(&file, src).unwrap();
    /// let tokens = syn::parse_file(src).unwrap().into_token_stream();
    /// let file_hash = TrackingHash::generate(PROJECT_NAMESPACE_UUID, &file, &tokens);
    /// let start = src.find("foo").unwrap();
    /// let end = start + "foo".len();
    ///
    /// let req = EmbeddingData {
    ///     id: Uuid::new_v4(),
    ///     name: "n".into(),
    ///     file_path: file.clone(),
    ///     file_tracking_hash: file_hash,
    ///     start_byte: start,
    ///     end_byte: end,
    ///     node_tracking_hash: TrackingHash(Uuid::new_v4()),
    ///     namespace: PROJECT_NAMESPACE_UUID,
    /// };
    ///
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let handle = ploke_io::IoManagerHandle::new();
    /// let out = handle.get_snippets_batch(vec![req]).await.unwrap();
    /// assert_eq!(out[0].as_ref().unwrap(), "foo");
    /// handle.shutdown().await;
    /// # });
    /// ```
    pub async fn get_snippets_batch(
        &self,
        requests: Vec<EmbeddingData>,
    ) -> Result<Vec<Result<String, PlokeError>>, errors::RecvError> {
        use tracing::{span, Level};
        let tracing_span = span!(Level::TRACE, "get_snippets_batch");
        let _enter = tracing_span.enter();

        let (responder, response_rx) = oneshot::channel();
        let request = IoRequest::ReadSnippetBatch {
            requests,
            responder,
        };
        self.request_sender
            .send(IoManagerMessage::Request(request))
            .await
            .map_err(|_| RecvError::SendError)?;
        response_rx.await.map_err(|_| RecvError::RecvError)
    }

    /// Scan a batch of files for content changes (tracking-hash mismatch).
    ///
    /// Returns `Ok(Ok(Vec<Option<ChangedFileData>>))` on success; the inner vector preserves
    /// input order and uses `Some` for changed files and `None` for unchanged. IO/parse/policy
    /// errors are propagated as `Err(PlokeError)`.
    ///
    /// ```rust,ignore
    /// use ploke_core::{FileData, TrackingHash, PROJECT_NAMESPACE_UUID};
    /// use uuid::Uuid;
    /// use quote::ToTokens;
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let file = dir.path().join("scan.rs");
    /// let initial = "fn a() {}\n";
    /// std::fs::write(&file, initial).unwrap();
    /// let tokens = syn::parse_file(initial).unwrap().into_token_stream();
    /// let old_hash = TrackingHash::generate(PROJECT_NAMESPACE_UUID, &file, &tokens);
    /// let req = FileData { id: Uuid::new_v4(), namespace: PROJECT_NAMESPACE_UUID, file_tracking_hash: old_hash, file_path: file.clone() };
    /// std::fs::write(&file, "fn a() { let _ = 1; }\n").unwrap();
    ///
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let handle = ploke_io::IoManagerHandle::new();
    /// let changed = handle.scan_changes_batch(vec![req]).await.unwrap().unwrap();
    /// assert!(changed[0].is_some());
    /// handle.shutdown().await;
    /// # });
    /// ```
    pub async fn scan_changes_batch(
        &self,
        requests: Vec<FileData>,
    ) -> Result<Result<Vec<Option<ChangedFileData>>, PlokeError>, IoError> {
        let (responder, response_rx) = oneshot::channel();
        let request = IoRequest::ScanChangeBatch {
            requests,
            responder,
        };
        self.request_sender
            .send(IoManagerMessage::Request(request))
            .await
            .map_err(|_| RecvError::SendError)
            .map_err(IoError::from)?;
        response_rx
            .await
            .map_err(|_| RecvError::RecvError)
            .map_err(IoError::from)
    }

    /// Subscribe to debounced file change events (requires `watcher` feature).
    ///
    /// The watcher is enabled via the builder and emits coalesced events with basic precedence
    /// (Removed > Renamed > Created > Modified). See watcher module docs for details.
    ///
    /// ```rust,ignore
    /// // Requires building your handle with the "watcher" feature enabled and roots configured.
    /// // let mut rx = handle.subscribe_file_events();
    /// // let evt = rx.recv().await.unwrap();
    /// // println!("File event: {:?} {:?}", evt.kind, evt.path);
    /// ```
    #[cfg(feature = "watcher")]
    pub fn subscribe_file_events(&self) -> tokio::sync::broadcast::Receiver<FileChangeEvent> {
        self.events_tx
            .as_ref()
            .expect("Watcher not enabled; use IoManagerBuilder::enable_watcher(true)")
            .subscribe()
    }

    /// Write a batch of snippets atomically (splice + fsync + rename).
    ///
    /// - Each write validates the expected file hash before applying changes.
    /// - Paths are normalized against configured roots with symlink policy enforcement.
    /// - Writes to the same file are serialized in-process via an async lock.
    /// - On success (and when the watcher feature is enabled), a Modified event is broadcast.
    ///
    /// ```rust,ignore
    /// use ploke_core::{WriteSnippetData, TrackingHash, PROJECT_NAMESPACE_UUID};
    /// use quote::ToTokens;
    /// use uuid::Uuid;
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let file = dir.path().join("w.rs");
    /// let initial = "fn hello() {}\n";
    /// std::fs::write(&file, initial).unwrap();
    /// let tokens = syn::parse_file(initial).unwrap().into_token_stream();
    /// let expected = TrackingHash::generate(PROJECT_NAMESPACE_UUID, &file, &tokens);
    /// let start = initial.find("hello").unwrap();
    /// let end = start + "hello".len();
    /// let req = WriteSnippetData {
    ///     id: Uuid::new_v4(),
    ///     name: "node".into(),
    ///     file_path: file.clone(),
    ///     expected_file_hash: expected,
    ///     start_byte: start,
    ///     end_byte: end,
    ///     replacement: "goodbye".into(),
    ///     namespace: PROJECT_NAMESPACE_UUID,
    /// };
    ///
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let handle = ploke_io::IoManagerHandle::new();
    /// let out = handle.write_snippets_batch(vec![req]).await.unwrap();
    /// assert!(out[0].is_ok());
    /// handle.shutdown().await;
    /// # });
    /// ```
    pub async fn write_snippets_batch(
        &self,
        requests: Vec<WriteSnippetData>,
    ) -> Result<Vec<Result<WriteResult, PlokeError>>, IoError> {
        let (responder, response_rx) = oneshot::channel();
        let request = IoRequest::WriteSnippetBatch {
            requests,
            responder,
        };
        self.request_sender
            .send(IoManagerMessage::Request(request))
            .await
            .map_err(|_| RecvError::SendError)
            .map_err(IoError::from)?;
        response_rx
            .await
            .map_err(|_| RecvError::RecvError)
            .map_err(IoError::from)
    }

    /// Sends a shutdown signal to the IoManager.
    pub async fn shutdown(&self) {
        let _ = self.request_sender.send(IoManagerMessage::Shutdown).await;
    }
}
