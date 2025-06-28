//! # ploke-io
//!
//! `ploke-io` is a dedicated crate for handling all I/O operations in the Ploke application.
//! It is designed to run as a separate, non-blocking actor that communicates with the rest of
//! the application via message-passing channels. This ensures that file system operations
//! do not block the main UI thread or other critical components.

use futures::future::join_all;
use ploke_error::fatal::FatalError;
use ploke_error::Error as PlokeError;
use seahash::SeaHasher;
use std::collections::HashMap;
use std::hash::Hasher;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use tokio::sync::{mpsc, oneshot, Semaphore};

/// A request to read a specific snippet from a file.
#[derive(Debug, Clone)]
pub struct SnippetRequest {
    /// The path to the file.
    pub path: PathBuf,
    /// The hash of the file content at the time of indexing.
    pub content_hash: u64,
    /// The start byte of the snippet.
    pub start: usize,
    /// The end byte of the snippet.
    pub end: usize,
}

/// A handle to the IoManager actor.
/// Used by other parts of the application to send requests.
#[derive(Clone, Debug)]
pub struct IoManagerHandle {
    /// Channel sender to send requests to the IoManager
    request_sender: mpsc::Sender<IoManagerMessage>,
}

impl IoManagerHandle {
    /// Spawns the IoManager and returns a handle to it.
    pub fn new() -> Self {
        let (request_sender, request_receiver) = mpsc::channel(100);

        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let manager = IoManager::new(request_receiver);
                manager.run().await;
            });
        });

        Self { request_sender }
    }

    /// Asynchronously requests a batch of code snippets.
    pub async fn get_snippets_batch(
        &self,
        requests: Vec<SnippetRequest>,
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
    request: SnippetRequest,
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
        requests: Vec<SnippetRequest>,
        responder: oneshot::Sender<Vec<Result<String, PlokeError>>>,
    },
}

/// The IoManager actor.
struct IoManager {
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
        requests: Vec<SnippetRequest>,
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
                    .map(|req| (req.idx, Err(FatalError::ShutdownInitiated.into())))
                    .collect();
            }
        };

        let mut results = Vec::new();

        // Open the file just once.
        let mut file = match File::open(&path).await {
            Ok(file) => file,
            Err(e) => {
                let arced_error = Arc::new(e);
                for req in requests {
                    results.push((
                        req.idx,
                        Err(FatalError::FileOperation {
                            operation: "open",
                            path: path.clone(),
                            source: Arc::clone(&arced_error),
                        }
                        .into()),
                    ));
                }
                return results;
            }
        };

        // Verify content hash by streaming the file
        let expected_hash = requests[0].request.content_hash;
        match verify_hash(&mut file, expected_hash, path.clone()).await {
            Ok(_) => {}
            Err(e) => {
                for req in requests {
                    results.push((req.idx, Err(e.clone())));
                }
                return results;
            }
        }

        // Now, read all snippets from the single open file handle
        for req in requests {
            let mut buffer = vec![0; req.request.end - req.request.start];
            let result = match file.seek(SeekFrom::Start(req.request.start as u64)).await {
                Ok(_) => match file.read_exact(&mut buffer).await {
                    Ok(_) => String::from_utf8(buffer).map_err(|e| {
                        FatalError::Utf8 {
                            path: path.clone(),
                            source: e,
                        }
                        .into()
                    }),
                    Err(e) => Err(FatalError::FileOperation {
                        operation: "read",
                        path: path.clone(),
                        source: e.into(),
                    }
                    .into()),
                },
                Err(e) => Err(FatalError::FileOperation {
                    operation: "seek",
                    path: path.clone(),
                    source: e.into(),
                }
                .into()),
            };
            results.push((req.idx, result));
        }

        results
    }
}

/// Reads the file chunk by chunk to verify its hash against the expected value.
async fn verify_hash(file: &mut File, expected_hash: u64, path: PathBuf) -> Result<(), PlokeError> {
    file.seek(SeekFrom::Start(0))
        .await
        .map_err(|e| FatalError::FileOperation {
            operation: "seek",
            path: path.clone(),
            source: e.into(),
        })?;
    let mut hasher = SeaHasher::new();
    let mut buffer = [0; 1024 * 8]; // 8KB buffer

    loop {
        let n = file
            .read(&mut buffer)
            .await
            .map_err(|e| FatalError::FileOperation {
                operation: "read",
                path: path.clone(),
                source: e.into(),
            })?;
        if n == 0 {
            break;
        }
        hasher.write(&buffer[..n]);
    }

    let actual_hash = hasher.finish();
    if actual_hash == expected_hash {
        Ok(())
    } else {
        Err(FatalError::ContentMismatch { path }.into())
    }
}

#[derive(Debug, Error)]
pub enum RecvError {
    #[error("Failed to send request to IO Manager")]
    SendError,
    #[error("Failed to receive response from IO Manager")]
    RecvError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn hash_content(content: &[u8]) -> u64 {
        let mut hasher = SeaHasher::new();
        hasher.write(content);
        hasher.finish()
    }

    #[tokio::test]
    async fn test_get_snippets_batch_preserves_order() {
        // 1. Setup
        let dir = tempdir().unwrap();
        let file_path1 = dir.path().join("test1.txt");
        let content1 = "Hello, world!";
        fs::write(&file_path1, content1).unwrap();

        let file_path2 = dir.path().join("test2.txt");
        let content2 = "This is a test.";
        fs::write(&file_path2, content2).unwrap();

        let io_manager = IoManagerHandle::new();

        // 2. Action
        let requests = vec![
            SnippetRequest {
                path: file_path1.clone(),
                content_hash: hash_content(content1.as_bytes()),
                start: 7,
                end: 12,
            },
            SnippetRequest {
                path: file_path2.clone(),
                content_hash: hash_content(content2.as_bytes()),
                start: 0,
                end: 4,
            },
            SnippetRequest {
                path: file_path1.clone(),
                content_hash: hash_content(content1.as_bytes()),
                start: 0,
                end: 5,
            },
        ];

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        // 3. Assert
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].as_ref().unwrap(), "world");
        assert_eq!(results[1].as_ref().unwrap(), "This");
        assert_eq!(results[2].as_ref().unwrap(), "Hello");

        io_manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_content_mismatch() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let content = "Initial content.";
        fs::write(&file_path, content).unwrap();

        let io_manager = IoManagerHandle::new();

        let requests = vec![SnippetRequest {
            path: file_path.clone(),
            content_hash: 12345, // Incorrect hash
            start: 0,
            end: 7,
        }];

        let results = io_manager.get_snippets_batch(requests).await.unwrap();
        assert!(matches!(
            results[0].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::ContentMismatch { .. })
        ));

        io_manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_io_errors() {
        let io_manager = IoManagerHandle::new();
        // Invalid path
        let results = io_manager
            .get_snippets_batch(vec![SnippetRequest {
                path: PathBuf::from("/non/existent"),
                content_hash: 0,
                start: 0,
                end: 10,
            }])
            .await
            .unwrap();

        assert!(matches!(
            results[0].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::FileOperation {
                operation: "open",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn test_utf8_validation() {
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let content = b"invalid\xFF\xFF";
        fs::write(&file_path, content).unwrap();
        let hash = hash_content(content);

        let results = io_manager
            .get_snippets_batch(vec![SnippetRequest {
                path: file_path.to_owned(),
                content_hash: hash,
                start: 0,
                end: 8,
            }])
            .await
            .unwrap();

        assert!(matches!(
            results[0].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::Utf8 { .. })
        ));
    }

    #[tokio::test]
    async fn test_concurrency_throttling() {
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();

        // Create more files than semaphore limit
        let requests: Vec<SnippetRequest> = (0..200)
            .map(|i| {
                let content = format!("file-{}", i);
                let path = dir.path().join(&content);
                fs::write(&path, &content).unwrap();
                SnippetRequest {
                    path: path.to_owned(),
                    content_hash: hash_content(content.as_bytes()),
                    start: 0,
                    end: content.len(),
                }
            })
            .collect();

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        // Should process all files despite concurrency limits
        assert_eq!(results.len(), 200);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[tokio::test]
    async fn test_seek_errors() {
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let content = b"short";
        fs::write(&file_path, content).unwrap();
        let hash = hash_content(content);

        let binding = io_manager
            .get_snippets_batch(vec![SnippetRequest {
                path: file_path.to_owned(),
                content_hash: hash,
                start: 0,
                // Ends past EOF
                end: 20,
            }])
            .await
            .unwrap();
        let res = binding[0].as_ref().unwrap_err();

        assert!(matches!(
            res,
            PlokeError::Fatal(FatalError::FileOperation { .. })
        ));
    }

    #[tokio::test]
    async fn test_zero_length_snippet() {
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_zero_length.txt");
        let content = "Hello, world!";
        fs::write(&file_path, content).unwrap();
        let hash = hash_content(content.as_bytes());

        let requests = vec![SnippetRequest {
            path: file_path.clone(),
            content_hash: hash,
            start: 5,
            end: 5, // Zero-length snippet
        }];

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().unwrap(), ""); // Expect empty string for zero-length snippet
    }

    #[tokio::test]
    async fn test_partial_failure_handling() {
        let io_manager = IoManagerHandle::new();
        let dir = tempdir().unwrap();

        // Valid file and content
        let file_path1 = dir.path().join("valid_file.txt");
        let content1 = "This is valid content.";
        fs::write(&file_path1, content1).unwrap();
        let hash1 = hash_content(content1.as_bytes());

        // Another valid file
        let file_path2 = dir.path().join("another_valid_file.txt");
        let content2 = "Another piece of valid content.";
        fs::write(&file_path2, content2).unwrap();
        let hash2 = hash_content(content2.as_bytes());

        // Non-existent file
        let non_existent_file = dir.path().join("non_existent.txt");

        // Request with content mismatch
        let file_path_mismatch = dir.path().join("mismatch_file.txt");
        let content_mismatch = "Original content.";
        fs::write(&file_path_mismatch, content_mismatch).unwrap();
        let hash_mismatch = 12345; // Incorrect hash

        let requests = vec![
            // Valid request 1
            SnippetRequest {
                path: file_path1.clone(),
                content_hash: hash1,
                start: 0,
                end: 4,
            },
            // Invalid request: non-existent file
            SnippetRequest {
                path: non_existent_file.clone(),
                content_hash: 0,
                start: 0,
                end: 10,
            },
            // Valid request 2
            SnippetRequest {
                path: file_path2.clone(),
                content_hash: hash2,
                start: 9,
                end: 13,
            },
            // Invalid request: content hash mismatch
            SnippetRequest {
                path: file_path_mismatch.clone(),
                content_hash: hash_mismatch,
                start: 0,
                end: 10,
            },
            // Valid request 3 (from file1 again)
            SnippetRequest {
                path: file_path1.clone(),
                content_hash: hash1,
                start: 5,
                end: 7,
            },
        ];

        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert_eq!(results.len(), 5);

        // Assert valid results
        assert_eq!(results[0].as_ref().unwrap(), "This");
        assert_eq!(results[2].as_ref().unwrap(), "piec");
        assert_eq!(results[4].as_ref().unwrap(), "is");

        // Assert failed results
        assert!(matches!(
            results[1].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::FileOperation {
                operation: "open",
                path,
                ..
            }) if path == &non_existent_file
        ));
        assert!(matches!(
            results[3].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::ContentMismatch {
                path,
            }) if path == &file_path_mismatch
        ));
    }
}
