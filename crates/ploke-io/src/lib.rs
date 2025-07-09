#![allow(unused_variables, unused_imports, dead_code)]
//! # ploke-io
//!
//! `ploke-io` provides a high-performance, non-blocking I/O actor system for reading
//! file snippets concurrently. It is designed for applications that need to read from
//! many files without blocking critical threads, such as a UI or a request-response loop.
//!
//! ## Core Components
//!
//! The crate is built around a few key components:
//!
//! - **`IoManagerHandle`**: The public-facing API and the primary entry point for this crate.
//!   It provides a simple, asynchronous interface to the I/O actor. It is a lightweight
//!   handle that can be cloned and shared across threads.
//!
//! - **`IoManager`**: The internal actor that runs in a dedicated background thread. It listens
//!   for requests, manages a pool of file handles, and executes file operations.
//!
//! - **`EmbeddingData`**: A struct that defines a request to read a specific byte range
//!   (a "snippet") from a file. It includes data integrity checks to ensure that the
//!   file content has not changed since it was indexed.
//!
//! ## Runtime Management
//!
//! The `IoManager` runs its own `tokio` runtime on a dedicated OS thread. This design
//! choice offers several advantages:
//!
//! 1.  **Isolation**: I/O operations are completely isolated from the caller's execution
//!     context. This is crucial for applications with their own async runtimes (like a GUI
//!     or a web server), as it prevents I/O-intensive work from blocking the main event loop.
//! 2.  **Dedicated Resources**: The I/O actor has its own set of resources, including a scheduler
//!     and a thread pool, which can be optimized for file operations.
//! 3.  **Simplified API**: Callers do not need to manage the lifecycle of the I/O runtime.
//!     They simply create an `IoManagerHandle` and start sending requests.
//!
//! The `IoManagerHandle::new()` function spawns a new OS thread and initializes a
//! `tokio::runtime::Builder` with `new_current_thread` and `enable_all`. This creates a
//! single-threaded runtime that is efficient for managing a queue of I/O tasks.
//!
//! ## Usage Example
//!
//! Here's how to use `ploke-io` to read snippets from multiple files:
//!
//! ```rust
//! use ploke_core::EmbeddingData;
//! use ploke_io::IoManagerHandle;
//! use std::fs;
//! use std::path::PathBuf;
//! use tempfile::tempdir;
//! use seahash::SeaHasher;
//! use std::hash::{Hash, Hasher};
//! use uuid::Uuid;  // Add this for ID generation
//!
//! fn hash_content(content: &[u8]) -> u64 {
//!     let mut hasher = SeaHasher::new();
//!     hasher.write(content);
//!     hasher.finish()
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     // 1. Create a temporary directory and some files for the example.
//!     let dir = tempdir().unwrap();
//!     let file_path1 = dir.path().join("test1.txt");
//!     let content1 = "fn main() { println!(\"Hello, world!\"); }";
//!     fs::write(&file_path1, content1).unwrap();
//!
//!     let file_path2 = dir.path().join("test2.txt");
//!     let content2 = "fn example() { println!(\"This is a test.\"); }";
//!     fs::write(&file_path2, content2).unwrap();
//!
//!     // 2. Create an IoManagerHandle. This spawns the actor in the background.
//!     let io_manager = IoManagerHandle::new();
//!
//!     // 3. Create a batch of requests.
//!     let requests = vec![
//!         EmbeddingData {
//!             id: Uuid::new_v4(),  // Add UUID field
//!             file_path: file_path1.clone(),
//!             file_tracking_hash: ploke_core::TrackingHash::generate(
//!                 ploke_core::PROJECT_NAMESPACE_UUID,
//!                 &file_path1,
//!                 &content1.parse().unwrap()
//!             ),
//!             start_byte: content1.find("world").unwrap(),
//!             end_byte: content1.find("world").unwrap() + "world".len(),
//!         },
//!         EmbeddingData {
//!             id: Uuid::new_v4(),  // Add UUID field
//!             file_path: file_path2.clone(),
//!             file_tracking_hash: ploke_core::TrackingHash::generate(
//!                 ploke_core::PROJECT_NAMESPACE_UUID,
//!                 &file_path2,
//!                 &content2.parse().unwrap()
//!             ),
//!             start_byte: content2.find("This").unwrap(),
//!             end_byte: content2.find("This").unwrap() + "This".len(),
//!         },
//!     ];
//!
//!     // 4. Send the requests and await the results.
//!     match io_manager.get_snippets_batch(requests).await {
//!         Ok(results) => {
//!             assert_eq!(results.len(), 2);
//!             assert_eq!(results[0].as_ref().unwrap(), "world");
//!             assert_eq!(results[1].as_ref().unwrap(), "This");
//!             println!("Successfully retrieved snippets!");
//!         }
//!         Err(e) => {
//!             eprintln!("Failed to get snippets: {:?}", e);
//!         }
//!     }
//!
//!     // 5. The IoManager can be shut down gracefully.
//!     io_manager.shutdown().await;
//! }
//! ```

use futures::future::join_all;
use ploke_core::EmbeddingData;
use ploke_core::TrackingHash;
use ploke_error::fatal::FatalError;
use ploke_error::Error as PlokeError;
use quote::ToTokens;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, Semaphore};

/// A handle to the IoManager actor.
/// Used by other parts of the application to send requests.
#[derive(Clone, Debug)]
pub struct IoManagerHandle {
    /// Channel sender to send requests to the IoManager
    request_sender: mpsc::Sender<IoManagerMessage>,
}

impl Default for IoManagerHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl IoManagerHandle {
    /// Spawns the IoManager and returns a handle to it.
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

        Self { request_sender: tx }
    }

    /// Asynchronously requests a batch of code snippets.
    pub async fn get_snippets_batch(
        &self,
        requests: Vec<EmbeddingData>,
    ) -> Result<Vec<Result<String, PlokeError>>, RecvError> {
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

    /// Sends a shutdown signal to the IoManager.
    pub async fn shutdown(&self) {
        let _ = self.request_sender.send(IoManagerMessage::Shutdown).await;
    }
}

/// An internal struct to track the original index of a request.
#[derive(Debug)]
struct OrderedRequest {
    idx: usize,
    request: EmbeddingData,
}

/// A message that can be sent to the IoManager.
#[derive(Debug)]
enum IoManagerMessage {
    Request(IoRequest),
    Shutdown,
}

/// Requests that can be sent to the IoManager.
#[derive(Debug)]
enum IoRequest {
    /// Request to read a batch of snippets from files.
    ReadSnippetBatch {
        requests: Vec<EmbeddingData>,
        responder: oneshot::Sender<Vec<Result<String, PlokeError>>>,
    },
}

/// The `IoManager` is a central actor responsible for handling all file I/O operations
/// in a non-blocking manner. It runs in a dedicated thread and processes requests
/// received through a message-passing channel.
///
/// ## Architecture
///
/// The `IoManager` follows the actor model. It is spawned by an `IoManagerHandle`,
/// which provides a clean API for other parts of the application to send I/O requests.
/// All communication happens through asynchronous channels, preventing the main application
/// from blocking on file operations.
///
/// ## Concurrency
///
/// To avoid exhausting system resources, the `IoManager` uses a `Semaphore` to limit
/// the number of concurrently open files. The limit is dynamically set based on the
/// system's available file descriptors (via `rlimit`), ensuring robust performance
/// without overwhelming the OS.
///
/// ## Request Handling
///
/// When a batch of snippet requests arrives, the `IoManager` performs the following steps:
/// 1.  Groups requests by their file path to minimize the number of file open operations.
/// 2.  For each file, it spawns a new asynchronous task.
/// 3.  Before reading snippets, it verifies the file's content against a provided hash
///     to ensure data integrity and prevent reading from stale files.
/// 4.  It reads the requested byte ranges (snippets) from the file.
/// 5.  The results, including any errors, are collected and returned to the original
///     caller, preserving the order of the initial requests.
///
/// This design ensures that I/O is handled efficiently, concurrently, and safely.
pub struct IoManager {
    request_receiver: mpsc::Receiver<IoManagerMessage>,
    semaphore: Arc<Semaphore>,
}

impl IoManager {
    /// Creates a new IoManager.
    fn new(request_receiver: mpsc::Receiver<IoManagerMessage>) -> Self {
        // Set concurrency based on available file descriptors
        let limit = match rlimit::getrlimit(rlimit::Resource::NOFILE) {
            Ok((soft, _)) => std::cmp::min(100, (soft / 3) as usize),
            Err(_) => 50, // Default to a safe value
        };

        Self {
            request_receiver,
            semaphore: Arc::new(Semaphore::new(limit)),
        }
    }

    /// Runs the actor's event loop.
    async fn run(mut self) {
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
                tokio::spawn(async move {
                    let results = Self::handle_read_snippet_batch(requests, semaphore).await;
                    let _ = responder.send(results);
                });
            }
        }
    }

    /// Groups requests by file path and processes each file concurrently.
    async fn handle_read_snippet_batch(
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
        let mut indexed_results: Vec<(usize, Result<String, PlokeError>)> = Vec::new();
        for task in join_all(file_tasks).await {
            match task {
                Ok(file_results) => indexed_results.extend(file_results),
                Err(e) => {
                    tracing::warn!("[ploke-io] FATAL: File processing task panicked: {:?}", e);
                }
            }
        }

        // 5. Sort results by the original index to restore order
        // TODO: Review the logic here. Not sure if it really makes sense to index the returned
        // results and then sort them like this vs. using a `DashMap` or similar to handle each of
        // the files separately, using a key of the `Uuid` of the nodes referencing the code snippet.
        indexed_results.sort_by_key(|(idx, _)| *idx);

        // 6. Create the final, ordered vector of results
        let mut final_results = Vec::with_capacity(total_requests);
        let mut result_idx = 0;
        for i in 0..total_requests {
            if result_idx < indexed_results.len() && indexed_results[result_idx].0 == i {
                final_results.push(indexed_results[result_idx].1.clone());
                result_idx += 1;
            } else {
                final_results.push(Err(ploke_error::InternalError::InvalidState(
                    "Result missing for request".to_string(),
                )
                .into()));
            }
        }
        final_results
    }

    /// Processes all snippet requests for a single file efficiently.
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

        let mut results = Vec::new();

        // Read the entire file content once
        let bytes = match tokio::fs::read(&file_path).await {
            Ok(content) => content,
            Err(e) => {
                let arced_error = Arc::new(e);
                for req in requests {
                    results.push((
                        req.idx,
                        Err(IoError::FileOperation {
                            operation: "read",
                            path: file_path.clone(),
                            source: Arc::clone(&arced_error),
                            kind: arced_error.kind(),
                        }
                        .into()),
                    ));
                }
                return results;
            }
        };

        let file_content = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                for req in requests {
                    results.push((
                        req.idx,
                        Err(IoError::Utf8 {
                            path: file_path.clone(),
                            source: e.clone(),
                        }
                        .into()),
                    ));
                }
                return results;
            }
        };

        // Parse the file content to a token stream
        let file_tokens = match syn::parse_file(&file_content) {
            Ok(parsed) => parsed.into_token_stream(),
            Err(e) => {
                for req in requests {
                    results.push((
                        req.idx,
                        Err(IoError::ParseError {
                            path: file_path.clone(),
                            message: e.to_string(),
                        }
                        .into()),
                    ));
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

        // Verify against the expected tracking hash
        // TODO: Replace just using the first file_tracking_hash with a better method. we should
        // probably just be sending the file tracking hash along once with the OrederedRequest.
        if actual_tracking_hash != requests[0].request.file_tracking_hash {
            for req in requests {
                eprintln!(
                    "file: {}, database: {}",
                    actual_tracking_hash.0, req.request.file_tracking_hash.0
                );
                results.push((
                    // TODO: Replace req.idx with the actual node id
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
            }
            return results;
        }

        // Extract snippets from the in-memory content
        for req in requests {
            // Validate byte order and bounds first
            if req.request.start_byte > req.request.end_byte
                || req.request.end_byte > file_content.len()
            {
                results.push((
                    req.idx,
                    Err(IoError::OutOfRange {
                        path: file_path.clone(),
                        start_byte: req.request.start_byte,
                        end_byte: req.request.end_byte,
                        file_len: file_content.len(),
                    }
                    .into()),
                ));
            } else {
                // Safe UTF-8 boundary check
                if !file_content.is_char_boundary(req.request.start_byte)
                    || !file_content.is_char_boundary(req.request.end_byte)
                {
                    results.push((
                        req.idx,
                        Err(IoError::InvalidCharBoundary {
                            path: file_path.clone(),
                            start_byte: req.request.start_byte,
                            end_byte: req.request.end_byte,
                        }
                        .into()),
                    ));
                } else {
                    let snippet =
                        file_content[req.request.start_byte..req.request.end_byte].to_string();
                    results.push((req.idx, Ok(snippet)));
                }
            }
        }

        results
    }
}

#[derive(Debug, Error, Clone)]
pub enum RecvError {
    #[error("Failed to send request to IO Manager")]
    SendError,
    #[error("Failed to receive response from IO Manager")]
    RecvError,
}

// impl From<RecvError> for IoError {
//     fn from(e: RecvError) -> Self {
//         IoError::Recv(e);
//     }
// }

// Define the additional error variants locally since we can't edit ploke-error
#[derive(Debug, Error, Clone)]
pub enum IoError {
    #[error("IO channel error")]
    Recv(#[from] RecvError),

    #[error("File content changed since indexing: {path}")]
    ContentMismatch {
        name: String,
        id: uuid::Uuid,
        file_tracking_hash: uuid::Uuid,
        namespace: uuid::Uuid,
        path: PathBuf,
    },

    #[error("Parse error in {path}: {message}")]
    ParseError { path: PathBuf, message: String },

    #[error(
        "Requested byte range {start_byte}..{end_byte} out of range for file {path} (length {file_len})"
    )]
    OutOfRange {
        path: PathBuf,
        start_byte: usize,
        end_byte: usize,
        file_len: usize,
    },

    // Other existing variants...
    #[error("Shutdown initiated")]
    ShutdownInitiated,

    #[error("File operation {operation} failed for {path}: {source} (kind: {kind:?})")]
    FileOperation {
        operation: &'static str,
        path: PathBuf,
        source: Arc<std::io::Error>,
        kind: std::io::ErrorKind,
    },

    #[error("UTF-8 decoding error in {path}: {source}")]
    Utf8 {
        path: PathBuf,
        source: std::string::FromUtf8Error,
    },

    #[error("Invalid UTF-8 boundaries in {path}: indices {start_byte}..{end_byte}")]
    InvalidCharBoundary {
        path: PathBuf,
        start_byte: usize,
        end_byte: usize,
    },
}

impl From<IoError> for ploke_error::Error {
    fn from(e: IoError) -> ploke_error::Error {
        use IoError::*;
        match e {
            ContentMismatch {
                name,
                id,
                file_tracking_hash,
                namespace,
                path,
            } => ploke_error::Error::Fatal(FatalError::ContentMismatch {
                name,
                id,
                file_tracking_hash,
                namespace,
                path,
            }),

            ParseError { path, message } => ploke_error::Error::Fatal(FatalError::SyntaxError(
                format!("Parse error in {}: {}", path.display(), message),
            )),

            OutOfRange {
                path,
                start_byte,
                end_byte,
                file_len,
            } => ploke_error::Error::Fatal(FatalError::FileOperation {
                operation: "read",
                path,
                source: Arc::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "Byte range {}-{} exceeds file length {}",
                        start_byte, end_byte, file_len
                    ),
                )),
            }),

            ShutdownInitiated => ploke_error::Error::Fatal(FatalError::ShutdownInitiated),

            FileOperation {
                operation,
                path,
                source,
                kind,
            } => ploke_error::Error::Fatal(FatalError::FileOperation {
                operation,
                path,
                source,
            }),

            Utf8 { path, source } => ploke_error::Error::Fatal(FatalError::Utf8 { path, source }),
            InvalidCharBoundary {
                path,
                start_byte,
                end_byte,
            } => {
                // Create a FromUtf8Error to capture the decoding failure
                let err_msg = format!(
                    "InvalidCharacterBoundary: Byte range {}-{} splits multi-byte Unicode character in file {}",
                    start_byte, end_byte, path.to_string_lossy()
                );

                ploke_error::Error::Fatal(FatalError::SyntaxError(err_msg))
            }
            Recv(recv_error) => ploke_error::Error::Internal(
                ploke_error::InternalError::CompilerError(recv_error.to_string()),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_common::{fixtures_crates_dir, workspace_root};
    use ploke_test_utils::{setup_db_full, setup_db_full_embeddings};
    use std::fs;
    use syn_parser::discovery::run_discovery_phase;
    use tempfile::tempdir;
    use tracing_error::ErrorLayer;
    use uuid::Uuid;

    use tracing_subscriber::{
        filter, fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry,
    };

    fn init_test_tracing(level: tracing::Level) {
        let filter = filter::Targets::new()
            .with_target("cozo", tracing::Level::ERROR)
            .with_target("", level);
        // .with_target("test_handle_read_snippet_batch", level);
        // .with_target("", tracing::Level::ERROR);

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr) // Write to stderr
                    .with_ansi(true) // Disable colors for cleaner output
                    .pretty()
                    .without_time(), // Optional: remove timestamps
            )
            .with(filter)
            .init();
    }

    fn init_tracing_v2(level: tracing::Level) {
        let filter = filter::Targets::new()
            .with_target("cozo", tracing::Level::WARN)
            .with_target("ploke-io", tracing::Level::DEBUG) // Use your crate name
            .with_target("", tracing::Level::INFO); // Default for other crates

        let fmt_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(true)
            .without_time()
            .with_target(false) // Disable target prefix for cleaner output
            .compact();

        Registry::default()
            .with(filter)
            .with(fmt_layer)
            .with(ErrorLayer::default()) // For span traces on errors
            .init();
    }

    fn tracking_hash(content: &str) -> TrackingHash {
        let file = syn::parse_file(content).expect("Failed to parse content");
        let tokens = file.into_token_stream();
        let file_path = PathBuf::from("placeholder.txt");
        TrackingHash::generate(ploke_core::PROJECT_NAMESPACE_UUID, &file_path, &tokens)
    }

    // Helper function for tests that need path-specific hashing
    fn tracking_hash_with_path(content: &str, file_path: &std::path::Path) -> TrackingHash {
        let file = syn::parse_file(content).expect("Failed to parse content");
        let tokens = file.into_token_stream();

        TrackingHash::generate(ploke_core::PROJECT_NAMESPACE_UUID, file_path, &tokens)
    }

    #[tokio::test]
    #[ignore = "needs redisign to have the id (NodeId) of file"]
    #[allow(unreachable_code)]
    async fn test_get_snippets_batch_preserves_order() {
        let dir = tempdir().unwrap();

        // Use valid Rust syntax
        let file_path1 = dir.path().join("test1.rs");
        let content1 = "fn main() { println!(\"Hello, world!\"); }";
        fs::write(&file_path1, content1).unwrap();

        let file_path2 = dir.path().join("test2.rs");
        let content2 = "fn example() { println!(\"This is a test.\"); }";
        fs::write(&file_path2, content2).unwrap();

        let io_manager = IoManagerHandle::new();

        let namespace = Uuid::new_v4();
        // Create requests with calculated offsets
        let requests = vec![
            EmbeddingData {
                file_path: file_path1.clone(),
                file_tracking_hash: tracking_hash(content1),

                node_tracking_hash: tracking_hash(content1),
                start_byte: content1.find("world").unwrap(),
                end_byte: content1.find("world").unwrap() + "world".len(),
                id: Uuid::new_v4(),
                name: "world".to_string(),
                namespace,
            },
            EmbeddingData {
                file_path: file_path2.clone(),
                file_tracking_hash: tracking_hash(content2),

                node_tracking_hash: tracking_hash(content2),
                start_byte: content2.find("This").unwrap(),
                end_byte: content2.find("This").unwrap() + "This".len(),
                id: Uuid::new_v4(),
                name: "This".to_string(),
                namespace,
            },
            EmbeddingData {
                file_path: file_path1.clone(),
                file_tracking_hash: tracking_hash(content1),

                node_tracking_hash: tracking_hash(content1),
                start_byte: content1.find("Hello").unwrap(),
                end_byte: content1.find("Hello").unwrap() + "Hello".len(),
                id: Uuid::new_v4(),
                name: "Hello".to_string(),
                namespace,
            },
        ];

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].as_ref().unwrap(), "world");
        assert_eq!(results[1].as_ref().unwrap(), "This");
        assert_eq!(results[2].as_ref().unwrap(), "Hello");

        io_manager.shutdown().await;
    }

    #[tokio::test]
    #[allow(unreachable_code)]
    async fn test_content_mismatch() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        let content = "fn main() { println!(\"Hello world!\"); }";
        fs::write(&file_path, content).unwrap();

        // Generate hash with production method
        // let valid_hash = tracking_hash(content);

        let io_manager = IoManagerHandle::new();
        let requests = vec![EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: TrackingHash(Uuid::new_v4()),

            node_tracking_hash: TrackingHash(Uuid::new_v4()),
            start_byte: 0,
            end_byte: 5,
            id: Uuid::new_v4(),
            name: "mismatched_hash".to_string(),
            namespace: Uuid::new_v4(),
        }];

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert!(matches!(
            &results[0],
            Err(PlokeError::Fatal(FatalError::ContentMismatch { path, .. }))
            if path == &file_path
        ))
    }

    #[tokio::test]
    #[allow(unreachable_code)]
    async fn test_io_errors() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        let content = "fn valid() {}";
        fs::write(&file_path, content).unwrap();

        let io_manager = IoManagerHandle::new();
        let namespace = Uuid::nil();
        let results = io_manager
            .get_snippets_batch(vec![EmbeddingData {
                file_path: PathBuf::from("/non/existent"),
                file_tracking_hash: tracking_hash(content),

                node_tracking_hash: tracking_hash(content),
                start_byte: 0,
                end_byte: 10,
                id: Uuid::new_v4(),
                name: "non_existent_file".to_string(),
                namespace,
            }])
            .await
            .unwrap();

        assert!(matches!(
            results[0],
            Err(PlokeError::Fatal(FatalError::FileOperation {
                operation: "read",
                ..
            }))
        ));
    }

    #[tokio::test]
    #[allow(unreachable_code)]
    async fn test_concurrency_throttling() {
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();
        let namespace = Uuid::nil();

        // Create more files than semaphore limit
        let requests: Vec<EmbeddingData> = (0..200)
            .map(|i| {
                let content = format!("const FILE_{}: u32 = {};", i, i);
                let file_path = dir.path().join(format!("file_{}.rs", i));
                fs::write(&file_path, &content).unwrap();
                EmbeddingData {
                    file_path: file_path.to_owned(),
                    file_tracking_hash: tracking_hash_with_path(&content, &file_path),
                    node_tracking_hash: tracking_hash_with_path(&content, &file_path),
                    start_byte: content.find("FILE").unwrap(),
                    end_byte: content.find("FILE").unwrap() + 8,
                    id: Uuid::new_v4(),
                    name: format!("FILE_{}", i),
                    namespace,
                }
            })
            .collect();

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        // Should process all files despite concurrency limits
        assert_eq!(results.len(), 200);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[tokio::test]
    #[allow(unreachable_code)]
    async fn test_seek_errors() {
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        let content = "fn short() {}";
        fs::write(&file_path, content).unwrap();

        let namespace = Uuid::nil();
        let results = io_manager
            .get_snippets_batch(vec![EmbeddingData {
                file_path: file_path.to_owned(),
                file_tracking_hash: tracking_hash_with_path(content, &file_path),

                node_tracking_hash: tracking_hash_with_path(content, &file_path),
                start_byte: 0,
                end_byte: 1000,
                id: Uuid::new_v4(),
                name: "seek_error".to_string(),
                namespace,
            }])
            .await
            .unwrap();
        let res = results[0].as_ref().unwrap_err();

        assert!(matches!(
            res,
            PlokeError::Fatal(FatalError::FileOperation {
                operation: "read",
                ..
            })
        ));
    }

    #[tokio::test]
    #[allow(unreachable_code)]
    async fn test_zero_length_snippet() {
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_zero_length.rs");
        let content = "fn placeholder() {}";
        fs::write(&file_path, content).unwrap();

        // Position after 'f'
        let pos = content.find('f').unwrap() + 1;
        let namespace = Uuid::nil();
        let requests = vec![EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: tracking_hash_with_path(content, &file_path),

            node_tracking_hash: tracking_hash_with_path(content, &file_path),
            start_byte: pos,
            end_byte: pos,
            id: Uuid::new_v4(),
            name: "zero_length".to_string(),
            namespace,
        }];

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().unwrap(), "");
    }

    #[tokio::test]
    #[allow(unreachable_code)]
    async fn test_partial_failure_handling() {
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();

        // Valid file and content
        let file_path1 = dir.path().join("valid_file.rs");
        let content1 = "fn valid() {}";
        fs::write(&file_path1, content1).unwrap();

        // Another valid file
        let file_path2 = dir.path().join("another_valid_file.rs");
        let content2 = "fn another_valid() {}";
        fs::write(&file_path2, content2).unwrap();

        // Non-existent file
        let non_existent_file = dir.path().join("non_existent.rs");

        // Request with content mismatch
        let file_path_mismatch = dir.path().join("mismatch_file.rs");
        let content_mismatch = "fn mismatch() {}";
        fs::write(&file_path_mismatch, content_mismatch).unwrap();
        let hash_mismatch = TrackingHash(Uuid::new_v4()); // random non-matching UUID

        let namespace = Uuid::nil();
        let requests = vec![
            // Valid request 1: "valid"
            EmbeddingData {
                file_path: file_path1.clone(),
                file_tracking_hash: tracking_hash_with_path(content1, &file_path1),

                node_tracking_hash: tracking_hash_with_path(content1, &file_path1),
                start_byte: content1.find("valid").unwrap(),
                end_byte: content1.find("valid").unwrap() + 5,
                id: Uuid::new_v4(),
                name: "any_name".to_string(),
                namespace,
            },
            // Invalid request: non-existent file
            EmbeddingData {
                file_path: non_existent_file.clone(),
                file_tracking_hash: tracking_hash("fn dummy() {}"),

                node_tracking_hash: tracking_hash("fn dummy() {}"),
                start_byte: 0,
                end_byte: 10,
                id: Uuid::new_v4(),
                name: "any_name".to_string(),
                namespace,
            },
            // Valid request 2: "another"
            EmbeddingData {
                file_path: file_path2.clone(),
                file_tracking_hash: tracking_hash_with_path(content2, &file_path2),

                node_tracking_hash: tracking_hash_with_path(content2, &file_path2),
                start_byte: content2.find("another").unwrap(),
                end_byte: content2.find("another").unwrap() + 7,
                id: Uuid::new_v4(),
                name: "any_name".to_string(),
                namespace,
            },
            // Invalid request: content hash mismatch
            EmbeddingData {
                file_path: file_path_mismatch.clone(),
                file_tracking_hash: hash_mismatch,
                node_tracking_hash: hash_mismatch,
                start_byte: 0,
                end_byte: 10,
                id: Uuid::new_v4(),
                name: "any_name".to_string(),
                namespace,
            },
            // Valid request 3: from file1 again
            EmbeddingData {
                file_path: file_path1.clone(),
                file_tracking_hash: tracking_hash_with_path(content1, &file_path1),

                node_tracking_hash: tracking_hash_with_path(content1, &file_path1),
                start_byte: content1.find("fn").unwrap(),
                end_byte: content1.find("fn").unwrap() + 2,
                id: Uuid::new_v4(),
                name: "any_name".to_string(),
                namespace,
            },
        ];

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert_eq!(results.len(), 5);

        // Assert valid results
        assert_eq!(results[0].as_ref().unwrap(), "valid");
        assert_eq!(results[2].as_ref().unwrap(), "another");
        assert_eq!(results[4].as_ref().unwrap(), "fn");

        // Assert failed results
        assert!(matches!(
            results[1].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::FileOperation {
                operation: "read",
                ..
            })
        ));
        assert!(matches!(
            results[3].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::ContentMismatch {
                path,
            ..
            }) if path == &file_path_mismatch
        ));
    }

    #[tokio::test]
    #[allow(unreachable_code)]
    async fn test_concurrent_modification() {
        use std::time::Duration;
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("modify_test.rs");
        let initial_content = "fn initial() {}";
        fs::write(&file_path, initial_content).unwrap();

        // Spawn file modifier task
        let file_path_clone = file_path.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let new_content = "fn modified() {}";
            fs::write(&file_path_clone, new_content).unwrap();
        });

        let namespace = Uuid::nil();
        let results = io_manager
            .get_snippets_batch(vec![EmbeddingData {
                file_path: file_path.clone(),
                file_tracking_hash: tracking_hash(initial_content),

                node_tracking_hash: tracking_hash(initial_content),
                start_byte: initial_content.find("initial").unwrap(),
                end_byte: initial_content.find("initial").unwrap() + 7,
                id: Uuid::new_v4(),
                name: "any_name".to_string(),
                namespace,
            }])
            .await
            .unwrap();

        assert!(matches!(
            results[0].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::ContentMismatch { .. })
        ));
    }

    #[tokio::test]
    #[allow(unreachable_code)]
    async fn test_actor_shutdown_during_ops() {
        use std::time::Duration;
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("slow_test.rs");
        let namespace = Uuid::nil();

        // Create large Rust content (256KB)
        let mut content = String::with_capacity(262144);
        for i in 0..30000 {
            content.push_str(&format!("const VAL_{}: usize = {};\n", i, i));
        }
        fs::write(&file_path, &content).unwrap();

        // Spawn a request that will take time to process
        let handle = {
            let io_manager = io_manager.clone();
            tokio::spawn(async move {
                io_manager
                    .get_snippets_batch(vec![EmbeddingData {
                        file_path,
                        file_tracking_hash: tracking_hash(&content),

                        node_tracking_hash: tracking_hash(&content),
                        start_byte: content.find("VAL_0").unwrap(),
                        end_byte: content.find("VAL_0").unwrap() + 5,
                        id: Uuid::new_v4(),
                        name: "any_name".to_string(),
                        namespace,
                    }])
                    .await
            })
        };

        // Shutdown during processing
        tokio::time::sleep(Duration::from_millis(10)).await;
        io_manager.shutdown().await;

        // Should handle shutdown gracefully
        let res = handle.await.unwrap();
        assert!(matches!(res, Err(RecvError::RecvError)));
    }

    // Add the new tests below the existing tests
    #[tokio::test]
    async fn test_utf8_error() {
        let namespace = Uuid::nil();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("invalid_utf8.rs");
        fs::write(&file_path, b"fn invalid\xc3(\"Hello\")").unwrap();

        let io_manager = IoManagerHandle::new();
        let requests = vec![EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: TrackingHash(Uuid::new_v4()),

            node_tracking_hash: TrackingHash(Uuid::new_v4()),
            start_byte: 0,
            end_byte: 10,
            id: Uuid::new_v4(),
            name: "any_name".to_string(),
            namespace,
        }];

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert!(matches!(
            results[0],
            Err(PlokeError::Fatal(FatalError::Utf8 { .. }))
        ));
    }

    #[tokio::test]
    async fn test_zero_byte_files() {
        let namespace = Uuid::nil();
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("empty.rs");
        fs::write(&file_path, "").unwrap();

        let requests = vec![EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: TrackingHash(Uuid::new_v4()),

            node_tracking_hash: TrackingHash(Uuid::new_v4()),
            start_byte: 0,
            end_byte: 0,
            id: Uuid::new_v4(),
            name: "any_name".to_string(),
            namespace,
        }];

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert!(matches!(
            results[0],
            Err(PlokeError::Fatal(FatalError::ContentMismatch { .. }))
        ));
    }

    #[tokio::test]
    async fn test_multi_byte_unicode_boundaries() {
        let namespace = Uuid::nil();
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("unicode.rs");
        let content = "fn main() { let s = \"こんにちは\"; }";
        fs::write(&file_path, content).unwrap();

        // Valid snippet: whole multi-byte character
        let valid_request = EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: tracking_hash_with_path(content, &file_path),

            node_tracking_hash: tracking_hash_with_path(content, &file_path),
            start_byte: content.find("こ").unwrap(),
            end_byte: content.find("こ").unwrap() + "こ".len(),
            id: Uuid::new_v4(),
            name: "any_name".to_string(),
            namespace,
        };

        // Invalid snippet: partial multi-byte character
        let invalid_request = EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: tracking_hash_with_path(content, &file_path),

            node_tracking_hash: tracking_hash_with_path(content, &file_path),
            start_byte: content.find("こ").unwrap() + 1,
            end_byte: content.find("こ").unwrap() + 2,
            id: Uuid::new_v4(),
            name: "any_name".to_string(),
            namespace,
        };

        let requests = vec![valid_request, invalid_request];
        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert_eq!(results[0].as_ref().unwrap(), "こ");
        assert!(matches!(
            results[1].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::SyntaxError(..))
        ));
    }

    #[tokio::test]
    async fn test_invalid_byte_ranges() {
        let namespace = Uuid::nil();
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        let content = "fn main() {}";
        fs::write(&file_path, content).unwrap();

        // Case 1: start_byte > end_byte
        let reverse_request = EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: tracking_hash_with_path(content, &file_path),

            node_tracking_hash: tracking_hash_with_path(content, &file_path),
            start_byte: 10,
            end_byte: 5,
            id: Uuid::new_v4(),
            name: "any_name".to_string(),
            namespace,
        };

        // Case 2: start_byte == end_byte (but file is shorter)
        let equal_request = EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: tracking_hash_with_path(content, &file_path),

            node_tracking_hash: tracking_hash_with_path(content, &file_path),
            start_byte: 100,
            end_byte: 100,
            id: Uuid::new_v4(),
            name: "name".to_string(),
            namespace,
        };

        let requests = vec![reverse_request, equal_request];
        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert!(matches!(
            results[0].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::FileOperation { .. })
        ));

        assert!(matches!(
            results[1].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::FileOperation { .. })
        ));
    }

    #[tokio::test]
    async fn test_exact_semaphore_limit() {
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();
        let namespace = Uuid::nil();

        // Determine expected concurrency limit
        let limit = match rlimit::getrlimit(rlimit::Resource::NOFILE) {
            Ok((soft, _)) => std::cmp::min(100, (soft / 3) as usize),
            Err(_) => 50,
        };

        let mut requests = Vec::new();
        for i in 0..limit {
            let content = format!("const FILE_{}: u32 = {};", i, i);
            let file_path = dir.path().join(format!("file_{}.rs", i));
            fs::write(&file_path, &content).unwrap();
            requests.push(EmbeddingData {
                file_path: file_path.to_owned(),
                file_tracking_hash: tracking_hash_with_path(&content, &file_path),

                node_tracking_hash: tracking_hash_with_path(&content, &file_path),
                start_byte: content.find("FILE").unwrap(),
                end_byte: content.find("FILE").unwrap() + 8,
                id: Uuid::new_v4(),
                name: "name".to_string(),
                namespace,
            });
        }

        let results = io_manager.get_snippets_batch(requests).await.unwrap();
        assert_eq!(results.len(), limit);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[tokio::test]
    async fn test_token_stream_sensitivity() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("sensitivity.rs");

        // Content with comments
        let content1 = r#"
        // This is a comment
        fn original() {}
        "#;

        // Same functional content without comments
        let content2 = "fn original() {}";

        // Different functional content
        let content3 = "fn modified() {}";

        fs::write(&file_path, content1).unwrap();
        let hash1 = tracking_hash_with_path(content1, &file_path);

        fs::write(&file_path, content2).unwrap();
        let hash2 = tracking_hash_with_path(content2, &file_path);

        fs::write(&file_path, content3).unwrap();
        let hash3 = tracking_hash_with_path(content3, &file_path);

        // Same semantics but should have same hash (comment changes don't matter)
        assert_eq!(hash1, hash2);

        // Functional change should produce different hash
        assert_ne!(hash2, hash3);
    }

    #[tokio::test]
    #[cfg_attr(not(unix), ignore)]
    async fn test_permission_denied() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("protected.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        // Set read-only permissions
        let mut permissions = file_path.metadata().unwrap().permissions();
        permissions.set_mode(0o200);
        fs::set_permissions(&file_path, permissions).unwrap();

        let io_manager = IoManagerHandle::new();
        let namespace = Uuid::nil();
        let requests = vec![EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: tracking_hash("fn main() {}"),

            node_tracking_hash: tracking_hash("fn main() {}"),
            start_byte: 0,
            end_byte: 10,
            id: Uuid::new_v4(),
            name: "main".to_string(),
            namespace,
        }];

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert!(matches!(
            results[0].clone(),
            Err(PlokeError::Fatal(FatalError::FileOperation {
                source,
                ..
            })) if source.kind() == std::io::ErrorKind::PermissionDenied
        ));
    }

    #[tokio::test]
    async fn test_read_during_shutdown() {
        let handle = IoManagerHandle::new();
        handle.shutdown().await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let result = handle.get_snippets_batch(vec![]).await;

        assert!(matches!(result, Err(RecvError::SendError)));
        // handle.shutdown().await; // Already shut down
    }

    #[tokio::test]
    async fn test_send_during_shutdown() {
        let handle = IoManagerHandle::new();
        // Send shutdown and wait for it to process
        let _ = handle.request_sender.send(IoManagerMessage::Shutdown).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Try to send after shutdown
        let result = handle
            .request_sender
            .send(IoManagerMessage::Request(IoRequest::ReadSnippetBatch {
                requests: vec![],
                responder: oneshot::channel().0,
            }))
            .await;

        assert!(result.is_err());
        // Cleanup shutdown
        handle.shutdown().await;
    }

    #[tokio::test]
    async fn test_handle_read_snippet_batch() -> Result<(), ploke_error::Error> {
        init_test_tracing(tracing::Level::TRACE);

        let fixture_name = "fixture_nodes";
        let crate_path = fixtures_crates_dir().join(fixture_name);
        let project_root = workspace_root(); // Use workspace root for context
        let discovery_output = run_discovery_phase(&project_root, &[crate_path.clone()])
            .unwrap_or_else(|e| panic!("Phase 1 Discovery failed for {}: {:?}", fixture_name, e));

        tracing::info!("🚀 Starting test");
        tracing::debug!("Initializing test database");

        let embedding_data = setup_db_full_embeddings(fixture_name)?;
        let data_expensive_clone = embedding_data.clone();
        // Add temporary tracing to inspect embedding data
        for data in &embedding_data {
            tracing::trace!(target: "handle_data",
                "EmbeddingData: id={}, file={}, range={}..{}",
                data.id,
                data.file_path.display(),
                data.start_byte,
                data.end_byte
            );
        }
        assert_ne!(0, embedding_data.len());
        tracing::debug!(target: "handle", "{:?}", embedding_data.len());

        for data in embedding_data.iter() {
            tracing::trace!(target: "handle", "{:#?}", data);
        }
        let embeddings_count = embedding_data.len();
        let semaphore = Arc::new(Semaphore::new(50)); // defaulting to safe value

        let snippets = IoManager::handle_read_snippet_batch(embedding_data, semaphore).await;
        assert_eq!(embeddings_count, snippets.len());

        let mut s_iter = snippets.iter();
        while let Some(Ok(s)) = s_iter.next() {
            tracing::trace!(target: "handle", "{}", s);
        }
        if let Some(Err(e)) = s_iter.next() {
            tracing::debug!(target: "handle", "{}", e);
        }

        let mut correct = 0;
        let mut error_count = 0;
        let mut contains_name = 0;
        let total_snips = snippets.len();
        for (i, s) in snippets.iter().enumerate() {
            match s {
                Ok(snip) => {
                    correct += 1;
                    if let Some(embed_data) = data_expensive_clone
                        .iter()
                        .find(|emb| snip.contains(&emb.name))
                    {
                        tracing::trace!(target: "handle", "name: {}, snip: {}", embed_data.name, snip);
                        contains_name += 1;
                    } else {
                        tracing::error!(target: "handle", 
                            "Name Not Found: \n{}{}",
                            "snippet: ", snip);
                    }
                }
                Err(e) => {
                    error_count += 1;
                    // tracing::error!(target: "handle", "{:?}", e);
                }
            }
            tracing::info!(target: "handle", "correct: {} | error_count: {} | contains_name: {} | total: {}", 
                correct, error_count, contains_name, total_snips
            );
        }

        for (i, (s, embed_data)) in snippets.iter().zip(data_expensive_clone.iter()).enumerate() {
            assert!(
                s.is_ok(),
                "snippet is error: {:?} \n\t| processing {}/{}\n\t| {:.2}% correct\nEmbedding data errored on: {:#?}",
                s,
                i,
                total_snips,
                (i as f32 / total_snips as f32),
                embed_data
            );
        }
        tracing::info!(
            "✅ Test complete. Errors: {}/{}",
            error_count,
            snippets.len()
        );
        tracing::info!(target: "handle", "correct: {} | error_count: {} | contains_name: {} | total: {}\n\tcorrect: {:.2}% | error_count: {:.2}% | contains_name: {:.2}%", 
            correct, error_count, contains_name, total_snips,
            percent(correct, total_snips),
            percent(error_count, total_snips),
            percent(contains_name, total_snips)
        );

        assert_eq!(error_count, 0, "Found {} snippet errors", error_count);

        Ok(())
    }

    #[tokio::test]
    #[ignore = "needs work"]
    async fn test_handle_request() -> Result<(), ploke_error::Error> {
        let embedding_data = setup_db_full_embeddings("fixture_nodes")?;
        let (tx, rx) = mpsc::channel(1000);
        let io_manager = IoManager::new(rx);
        let handle = io_manager.run();

        Ok(())
    }

    fn percent(i: usize, t: usize) -> f32 {
        i as f32 /(t as f32) * 100.0
    }

    // pub fn setup_db_full_embeddings(fixture: &'static str) -> Result<Vec<ploke_core::EmbeddingData>, ploke_error::Error> {
    //     let db = ploke_db::Database::new( setup_db_full(fixture)? );
    //     let embedding_data = db.get_nodes_for_embedding(100, None)?;
    //     Ok(embedding_data)
    // }
}
