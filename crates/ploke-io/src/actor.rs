use crate::{
    path_policy::path_within_roots,
    read::{extract_snippet_str, parse_tokens_from_str, read_file_to_string_abs}, 
};
#[cfg(test)]
use crate::scan::test_instrumentation;

use super::*;

pub struct IoManager {
    request_receiver: mpsc::Receiver<IoManagerMessage>,
    semaphore: Arc<Semaphore>,
    roots: Option<Arc<Vec<PathBuf>>>,
}

/// A message that can be sent to the IoManager.
#[derive(Debug)]
pub enum IoManagerMessage {
    Request(IoRequest),
    Shutdown,
}

/// Requests that can be sent to the IoManager.
#[derive(Debug)]
pub enum IoRequest {
    /// Request to read a batch of snippets from files.
    ReadSnippetBatch {
        requests: Vec<EmbeddingData>,
        responder: oneshot::Sender<Vec<Result<String, PlokeError>>>,
    },
    ScanChangeBatch {
        requests: Vec<FileData>,
        responder: oneshot::Sender<Result<Vec<Option<ChangedFileData>>, PlokeError>>,
    },
}

/// An internal struct to track the original index of a request.
#[derive(Debug)]
struct OrderedRequest {
    idx: usize,
    request: EmbeddingData,
}

impl IoManager {
    /// Creates a new IoManager.
    pub(crate) fn new(request_receiver: mpsc::Receiver<IoManagerMessage>) -> Self {
        // Set concurrency based on available file descriptors with env override
        let default_limit = match rlimit::getrlimit(rlimit::Resource::NOFILE) {
            Ok((soft, _)) => std::cmp::min(100, (soft / 3) as usize),
            Err(_) => 50, // Default to a safe value
        };
        let limit = std::env::var("PLOKE_IO_FD_LIMIT")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .map(|n| n.clamp(4, 1024))
            .unwrap_or(default_limit);

        Self {
            request_receiver,
            semaphore: Arc::new(Semaphore::new(limit)),
            roots: None,
        }
    }

    /// Creates a new IoManager with explicit semaphore permits and optional roots.
    pub fn new_with(
        request_receiver: mpsc::Receiver<IoManagerMessage>,
        semaphore_permits: usize,
        roots: Option<Vec<PathBuf>>,
    ) -> Self {
        Self {
            request_receiver,
            semaphore: Arc::new(Semaphore::new(semaphore_permits)),
            roots: roots.map(Arc::new),
        }
    }

    /// Runs the actor's event loop.
    pub async fn run(mut self) {
        while let Some(message) = self.request_receiver.recv().await {
            match message {
                IoManagerMessage::Request(request) => self.handle_request(request).await,
                IoManagerMessage::Shutdown => break,
            }
        }
    }

    async fn handle_request(&self, request: IoRequest) {
        match request {
            IoRequest::ReadSnippetBatch {
                requests,
                responder,
            } => {
                let semaphore = self.semaphore.clone();
                let roots = self.roots.clone();
                tokio::spawn(async move {
                    let results =
                        Self::handle_read_snippet_batch_with_roots(requests, semaphore, roots)
                            .await;
                    let _ = responder.send(results);
                });
            }
            IoRequest::ScanChangeBatch {
                requests,
                responder,
            } => {
                let semaphore = self.semaphore.clone();
                let roots = self.roots.clone();
                tokio::spawn(async move {
                    let results =
                        Self::handle_scan_batch_with_roots(requests, semaphore, roots).await;
                    let _ = responder.send(results);
                });
            }
        }
    }

    /// Groups requests by file path and processes each file concurrently.
    pub async fn handle_read_snippet_batch(
        requests: Vec<EmbeddingData>,
        semaphore: Arc<Semaphore>,
    ) -> Vec<Result<String, PlokeError>> {
        let total_requests = requests.len();

        // 1. Assign original index to each request (0-indexed)
        let ordered_requests = requests
            .into_iter()
            .enumerate()
            .map(|(idx, request)| OrderedRequest { idx, request });

        // 2. Group requests by file path
        let mut requests_by_file: HashMap<PathBuf, Vec<OrderedRequest>> = HashMap::new();
        for ordered_req in ordered_requests {
            requests_by_file
                .entry(ordered_req.request.file_path.clone())
                .or_default()
                .push(ordered_req);
        }

        // 3. Spawn a task for each file, processing them concurrently
        // TODO: consider using `rayon` here instead of `tokio`. Might be worth benchmarking later.
        let file_tasks = requests_by_file.into_iter().map(|(path, reqs)| {
            let semaphore = semaphore.clone();
            tokio::spawn(async move { Self::process_file(path, reqs, semaphore).await })
        });

        // 4. Collect results and preserve order
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
                    Err(
                        ploke_error::InternalError::InvalidState("Result missing for request")
                            .into(),
                    )
                })
            })
            .collect()
    }

    pub async fn handle_read_snippet_batch_with_roots(
        requests: Vec<EmbeddingData>,
        semaphore: Arc<Semaphore>,
        roots: Option<Arc<Vec<PathBuf>>>,
    ) -> Vec<Result<String, PlokeError>> {
        let total_requests = requests.len();

        // 1. Assign original index to each request (0-indexed)
        let ordered_requests = requests
            .into_iter()
            .enumerate()
            .map(|(idx, request)| OrderedRequest { idx, request });

        // 2. Group requests by file path
        let mut requests_by_file: HashMap<PathBuf, Vec<OrderedRequest>> = HashMap::new();
        for ordered_req in ordered_requests {
            requests_by_file
                .entry(ordered_req.request.file_path.clone())
                .or_default()
                .push(ordered_req);
        }

        // 3. Spawn a task for each file, processing them concurrently
        let file_tasks = requests_by_file.into_iter().map(|(path, reqs)| {
            let semaphore = semaphore.clone();
            let roots = roots.clone();
            tokio::spawn(async move {
                Self::process_file_with_roots(path, reqs, semaphore, roots).await
            })
        });

        // 4. Collect results and preserve order
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
                    Err(
                        ploke_error::InternalError::InvalidState("Result missing for request")
                            .into(),
                    )
                })
            })
            .collect()
    }

    /// Processes all snippet requests for a single file efficiently.
    // TODO: refactor to return a result and use `?` instead of all the match and returns below
    async fn process_file(
        file_path: PathBuf,
        requests: Vec<OrderedRequest>,
        semaphore: Arc<Semaphore>,
    ) -> Vec<(usize, Result<String, PlokeError>)> {
        let _permit = match semaphore.acquire().await {
            Ok(permit) => permit,
            Err(_) => {
                return requests
                    .into_iter()
                    .map(|req| (req.idx, Err(IoError::ShutdownInitiated.into())))
                    .collect();
            }
        };

        // TODO: separate checking file hash into its own function, apart from checking for the has
        // or contents of each other node
        let mut results = Vec::new();

        // Read and parse file once using helpers
        let file_content = match read_file_to_string_abs(&file_path).await {
            Ok(s) => s,
            Err(e) => {
                let err = e.clone();
                for req in requests {
                    results.push((req.idx, Err(err.clone().into())));
                }
                return results;
            }
        };

        let file_tokens = match parse_tokens_from_str(&file_content, &file_path) {
            Ok(tokens) => tokens,
            Err(e) => {
                let err = e.clone();
                for req in requests {
                    results.push((req.idx, Err(err.clone().into())));
                }
                return results;
            }
        };

        // Generate tracking hash from token stream
        let namespace = requests
            .first()
            .expect("All read requests must have at least one request")
            .request
            .namespace;
        let actual_tracking_hash = TrackingHash::generate(namespace, &file_path, &file_tokens);

        // Verify per-request against the expected tracking hash (handled in the loop below)

        // Extract snippets from the in-memory content
        for req in requests {
            // Per-request hash verification
            if actual_tracking_hash != req.request.file_tracking_hash {
                tracing::error!(
                    "file: {}, database: {}\nfull request dump:\n{:#?}",
                    actual_tracking_hash.0,
                    req.request.file_tracking_hash.0,
                    req
                );
                results.push((
                    req.idx,
                    Err(IoError::ContentMismatch {
                        path: file_path.clone(),
                        name: req.request.name.clone(),
                        id: req.request.id,
                        file_tracking_hash: req.request.file_tracking_hash.0,
                        namespace,
                    }
                    .into()),
                ));
                continue;
            }

            match extract_snippet_str(
                &file_content,
                req.request.start_byte,
                req.request.end_byte,
                &file_path,
            ) {
                Ok(snippet) => results.push((req.idx, Ok(snippet))),
                Err(e) => results.push((req.idx, Err(e.into()))),
            }
        }

        results
    }

    async fn process_file_with_roots(
        file_path: PathBuf,
        requests: Vec<OrderedRequest>,
        semaphore: Arc<Semaphore>,
        roots: Option<Arc<Vec<PathBuf>>>,
    ) -> Vec<(usize, Result<String, PlokeError>)> {
        if let Some(roots) = roots.as_ref() {
            if !path_within_roots(&file_path, roots) {
                let err = IoError::FileOperation {
                    operation: "read",
                    path: file_path.clone(),
                    source: Arc::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "path outside configured roots",
                    )),
                    kind: std::io::ErrorKind::InvalidInput,
                };
                return requests
                    .into_iter()
                    .map(|req| (req.idx, Err(err.clone().into())))
                    .collect();
            }
        }

        Self::process_file(file_path, requests, semaphore).await
    }

    /// Scans a batch of `FileData` in parallel, bounded by the given semaphore,
    /// and returns the paths whose contents no longer match their stored tracking hash.
    ///
    /// Files that have changed are returned as `Some(path)`; unchanged files are omitted
    /// (`None`).  I/O or parse errors are propagated as `Err`.
    ///
    /// Concurrency is limited by the semaphore’s available permits; no more permits than
    /// `semaphore.available_permits()` files are processed concurrently.
    async fn handle_scan_batch(
        requests: Vec<FileData>,
        semaphore: Arc<Semaphore>,
    ) -> Result<Vec<Option<ChangedFileData>>, PlokeError> {
        use futures::stream::StreamExt;

        let total = requests.len();
        let concurrency_limit = std::cmp::max(1, semaphore.available_permits());

        // Process with bounded concurrency but keep track of original indices.
        let results_idx =
            futures::stream::iter(requests.into_iter().enumerate().map(|(idx, file_data)| {
                let sem = semaphore.clone();
                async move { (idx, Self::check_file_hash(file_data, sem).await) }
            }))
            .buffer_unordered(concurrency_limit)
            .collect::<Vec<_>>()
            .await;

        // Pre-allocate output preserving input order.
        let mut ordered: Vec<Option<ChangedFileData>> = vec![None; total];
        // Capture the first error by input order (deterministic).
        let mut first_err: Option<(usize, PlokeError)> = None;

        for (idx, res) in results_idx {
            match res {
                Ok(opt) => ordered[idx] = opt,
                Err(e) => {
                    if first_err.is_none()
                        || first_err.as_ref().map(|(i, _)| idx < *i).unwrap_or(false)
                    {
                        first_err = Some((idx, e));
                    }
                }
            }
        }

        if let Some((_, e)) = first_err {
            Err(e)
        } else {
            Ok(ordered)
        }
    }

    pub async fn handle_scan_batch_with_roots(
        requests: Vec<FileData>,
        semaphore: Arc<Semaphore>,
        roots: Option<Arc<Vec<PathBuf>>>,
    ) -> Result<Vec<Option<ChangedFileData>>, PlokeError> {
        use futures::stream::StreamExt;

        let total = requests.len();
        let concurrency_limit = std::cmp::max(1, semaphore.available_permits());

        let results_idx =
            futures::stream::iter(requests.into_iter().enumerate().map(|(idx, file_data)| {
                let sem = semaphore.clone();
                let roots = roots.clone();
                async move {
                    (
                        idx,
                        Self::check_file_hash_with_roots(file_data, sem, roots).await,
                    )
                }
            }))
            .buffer_unordered(concurrency_limit)
            .collect::<Vec<_>>()
            .await;

        let mut ordered: Vec<Option<ChangedFileData>> = vec![None; total];
        let mut first_err: Option<(usize, PlokeError)> = None;

        for (idx, res) in results_idx {
            match res {
                Ok(opt) => ordered[idx] = opt,
                Err(e) => {
                    if first_err.is_none()
                        || first_err.as_ref().map(|(i, _)| idx < *i).unwrap_or(false)
                    {
                        first_err = Some((idx, e));
                    }
                }
            }
        }

        if let Some((_, e)) = first_err {
            Err(e)
        } else {
            Ok(ordered)
        }
    }

    /// Computes a fresh tracking hash for a single file and compares it to the store value.
    ///
    /// Acquires one semaphore permit while the file is read.
    /// Returns `Some(path)` if the hash differs, `None` if it matches, or an error in the
    /// file cannot be read or parsed.
    async fn check_file_hash(
        file_data: FileData,
        semaphore: Arc<Semaphore>,
    ) -> Result<Option<ChangedFileData>, PlokeError> {
        let _permit = semaphore
            .acquire()
            .await
            .map_err(|_| IoError::ShutdownInitiated)?;
        #[cfg(test)]
        let _probe_guard = test_instrumentation::enter();
        #[cfg(test)]
        test_instrumentation::maybe_sleep().await;
        let file_content = read_file_to_string_abs(&file_data.file_path).await?;
        let tokens = parse_tokens_from_str(&file_content, &file_data.file_path)?;

        let new_hash = TrackingHash::generate(file_data.namespace, &file_data.file_path, &tokens);

        if new_hash != file_data.file_tracking_hash {
            Ok(Some(ChangedFileData::from_file_data(file_data, new_hash)))
        } else {
            Ok(None)
        }
    }

    async fn check_file_hash_with_roots(
        file_data: FileData,
        semaphore: Arc<Semaphore>,
        roots: Option<Arc<Vec<PathBuf>>>,
    ) -> Result<Option<ChangedFileData>, PlokeError> {
        if let Some(roots) = roots.as_ref() {
            if !path_within_roots(&file_data.file_path, roots) {
                return Err(IoError::FileOperation {
                    operation: "read",
                    path: file_data.file_path.clone(),
                    source: Arc::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "path outside configured roots",
                    )),
                    kind: std::io::ErrorKind::InvalidInput,
                }
                .into());
            }
        }

        Self::check_file_hash(file_data, semaphore).await
    }
}
