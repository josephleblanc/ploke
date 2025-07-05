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
//! - **`EmbeddingNode`**: A struct that defines a request to read a specific byte range
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
//! ```rust,no_run
//! use ploke_core::EmbeddingNode;
//! use ploke_io::IoManagerHandle;
//! use std::fs;
//! use std::path::PathBuf;
//! use tempfile::tempdir;
//! use seahash::SeaHasher;
//! use std::hash::{Hash, Hasher};
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
//!     let content1 = "Hello, world!";
//!     fs::write(&file_path1, content1).unwrap();
//!
//!     let file_path2 = dir.path().join("test2.txt");
//!     let content2 = "This is a test.";
//!     fs::write(&file_path2, content2).unwrap();
//!
//!     // 2. Create an IoManagerHandle. This spawns the actor in the background.
//!     let io_manager = IoManagerHandle::new();
//!
//!     // 3. Create a batch of requests.
//!     let requests = vec![
//!         EmbeddingNode {
//!             path: file_path1.clone(),
//!             file_tracking_hash: hash_content(content1.as_bytes()),
//!             start_byte: 7,
//!             end_byte: 12, // "world"
//!         },
//!         EmbeddingNode {
//!             path: file_path2.clone(),
//!             file_tracking_hash: hash_content(content2.as_bytes()),
//!             start_byte: 0,
//!             end_byte: 4,  // "This"
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
use ploke_core::EmbeddingNode;
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
        requests: Vec<EmbeddingNode>,
    ) -> Result<Vec<Result<String, PlokeError>>, RecvError> {
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
    request: EmbeddingNode,
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
        requests: Vec<EmbeddingNode>,
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
        requests: Vec<EmbeddingNode>,
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
                .entry(ordered_req.request.path.clone())
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
                    eprintln!("[ploke-io] FATAL: File processing task panicked: {:?}", e);
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
        path: PathBuf,
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
        let file_content = match tokio::fs::read_to_string(&path).await {
            Ok(content) => content,
            Err(e) => {
                let arced_error = Arc::new(e);
                for req in requests {
                    results.push((
                        req.idx,
                        Err(IoError::FileOperation {
                            operation: "read",
                            path: path.clone(),
                            source: Arc::clone(&arced_error),
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
                            path: path.clone(),
                            message: e.to_string(),
                        }
                        .into()),
                    ));
                }
                return results;
            }
        };

        // Generate tracking hash from token stream
        let actual_tracking_hash = TrackingHash::generate(
            ploke_core::PROJECT_NAMESPACE_UUID, // crate_namespace not needed for file-level hash
            &path,
            &file_tokens,
        );

        // Verify against the expected tracking hash
        if actual_tracking_hash != requests[0].request.file_tracking_hash {
            for req in requests {
                results.push((
                    req.idx,
                    Err(IoError::ContentMismatch { path: path.clone() }.into()),
                ));
            }
            return results;
        }

        // Extract snippets from the in-memory content
        for req in requests {
            if req.request.end_byte > file_content.len() {
                results.push((
                    req.idx,
                    Err(IoError::OutOfRange {
                        path: path.clone(),
                        start_byte: req.request.start_byte,
                        end_byte: req.request.end_byte,
                        file_len: file_content.len(),
                    }
                    .into()),
                ));
            } else {
                let snippet =
                    file_content[req.request.start_byte..req.request.end_byte].to_string();
                results.push((req.idx, Ok(snippet)));
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
    ContentMismatch { path: PathBuf },

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

    #[error("File operation {operation} failed for {path}: {source}")]
    FileOperation {
        operation: &'static str,
        path: PathBuf,
        source: Arc<std::io::Error>,
    },

    #[error("UTF-8 decoding error in {path}: {source}")]
    Utf8 {
        path: PathBuf,
        source: std::string::FromUtf8Error,
    },
}

impl From<IoError> for ploke_error::Error {
    fn from(e: IoError) -> ploke_error::Error {
        use IoError::*;
        match e {
            ContentMismatch { path } => {
                ploke_error::Error::Fatal(FatalError::ContentMismatch { path })
            }

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
            } => ploke_error::Error::Fatal(FatalError::FileOperation {
                operation,
                path,
                source,
            }),

            Utf8 { path, source } => ploke_error::Error::Fatal(FatalError::Utf8 { path, source }),
            Recv(recv_error) => ploke_error::Error::Internal(
                ploke_error::InternalError::CompilerError(recv_error.to_string()),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn tracking_hash(content: &str) -> TrackingHash {
        let file = syn::parse_file(content).expect("Failed to parse content");
        let tokens = file.into_token_stream();

        TrackingHash::generate(
            Uuid::nil(),                       // Matches production call
            &PathBuf::from("placeholder.txt"), // Path not important for tests
            &tokens,
        )
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

        // Create requests with calculated offsets
        let requests = vec![
            EmbeddingNode {
                path: file_path1.clone(),
                file_tracking_hash: tracking_hash(content1),
                start_byte: content1.find("world").unwrap(),
                end_byte: content1.find("world").unwrap() + 5,
                id: todo!(),
            },
            EmbeddingNode {
                path: file_path2.clone(),
                file_tracking_hash: tracking_hash(content2),
                start_byte: content2.find("This").unwrap(),
                end_byte: content2.find("This").unwrap() + 4,
                id: todo!(),
            },
            EmbeddingNode {
                path: file_path1.clone(),
                file_tracking_hash: tracking_hash(content1),
                start_byte: content1.find("Hello").unwrap(),
                end_byte: content1.find("Hello").unwrap() + 5,
                id: todo!(),
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
        let requests = vec![EmbeddingNode {
            path: file_path.clone(),
            file_tracking_hash: TrackingHash(Uuid::new_v4()),
            start_byte: 0,
            end_byte: 5,
            id: todo!(),
        }];

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert!(matches!(
            &results[0],
            Err(PlokeError::Fatal(FatalError::ContentMismatch { path }))
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
        let results = io_manager
            .get_snippets_batch(vec![EmbeddingNode {
                path: PathBuf::from("/non/existent"),
                file_tracking_hash: tracking_hash(content),
                start_byte: 0,
                end_byte: 10,
                id: todo!(),
            }])
            .await
            .unwrap();

        assert!(matches!(
            results[0],
            Err(PlokeError::Fatal(FatalError::FileOperation {
                operation: "open",
                ..
            }))
        ));
    }

    #[tokio::test]
    #[allow(unreachable_code)]
    async fn test_concurrency_throttling() {
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();

        // Create more files than semaphore limit
        let requests: Vec<EmbeddingNode> = (0..200)
            .map(|i| {
                let content = format!("const FILE_{}: u32 = {};", i, i);
                let path = dir.path().join(format!("file_{}.rs", i));
                fs::write(&path, &content).unwrap();
                EmbeddingNode {
                    path: path.to_owned(),
                    file_tracking_hash: tracking_hash(&content),
                    start_byte: content.find("FILE").unwrap(),
                    end_byte: content.find("FILE").unwrap() + 8,
                    id: todo!(),
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

        let results = io_manager
            .get_snippets_batch(vec![EmbeddingNode {
                path: file_path.to_owned(),
                file_tracking_hash: tracking_hash(content),
                start_byte: 0,
                end_byte: 1000,
                id: todo!(),
            }])
            .await
            .unwrap();
        let res = results[0].as_ref().unwrap_err();

        assert!(matches!(
            res,
            PlokeError::Fatal(FatalError::FileOperation { .. })
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
        let requests = vec![EmbeddingNode {
            path: file_path.clone(),
            file_tracking_hash: tracking_hash(content),
            start_byte: pos,
            end_byte: pos,
            id: todo!(),
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

        let requests = vec![
            // Valid request 1: "valid"
            EmbeddingNode {
                path: file_path1.clone(),
                file_tracking_hash: tracking_hash(content1),
                start_byte: content1.find("valid").unwrap(),
                end_byte: content1.find("valid").unwrap() + 5,
                id: todo!(),
            },
            // Invalid request: non-existent file
            EmbeddingNode {
                path: non_existent_file.clone(),
                file_tracking_hash: tracking_hash("fn dummy() {}"),
                start_byte: 0,
                end_byte: 10,
                id: todo!(),
            },
            // Valid request 2: "another"
            EmbeddingNode {
                path: file_path2.clone(),
                file_tracking_hash: tracking_hash(content2),
                start_byte: content2.find("another").unwrap(),
                end_byte: content2.find("another").unwrap() + 7,
                id: todo!(),
            },
            // Invalid request: content hash mismatch
            EmbeddingNode {
                path: file_path_mismatch.clone(),
                file_tracking_hash: hash_mismatch,
                start_byte: 0,
                end_byte: 10,
                id: todo!(),
            },
            // Valid request 3: from file1 again
            EmbeddingNode {
                path: file_path1.clone(),
                file_tracking_hash: tracking_hash(content1),
                start_byte: content1.find("fn").unwrap(),
                end_byte: content1.find("fn").unwrap() + 2,
                id: todo!(),
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
                operation: "open",
                ..
            })
        ));
        assert!(matches!(
            results[3].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::ContentMismatch {
                path,
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

        let results = io_manager
            .get_snippets_batch(vec![EmbeddingNode {
                path: file_path.clone(),
                file_tracking_hash: tracking_hash(initial_content),
                start_byte: initial_content.find("initial").unwrap(),
                end_byte: initial_content.find("initial").unwrap() + 7,
                id: todo!(),
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
                    .get_snippets_batch(vec![EmbeddingNode {
                        path: file_path,
                        file_tracking_hash: tracking_hash(&content),
                        start_byte: content.find("VAL_0").unwrap(),
                        end_byte: content.find("VAL_0").unwrap() + 5,
                        id: todo!(),
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
}
