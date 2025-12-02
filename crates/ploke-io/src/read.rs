use super::*;
// Target module for read-path helpers and batch processing

pub(crate) async fn read_file_to_string_abs(path: &Path) -> Result<String, IoError> {
    if !path.is_absolute() {
        return Err(IoError::FileOperation {
            operation: "read",
            path: path.to_path_buf(),
            source: Arc::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "path must be absolute",
            )),
            kind: std::io::ErrorKind::InvalidInput,
        });
    }

    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(e) => {
            let kind = e.kind();
            return Err(IoError::FileOperation {
                operation: "read",
                path: path.to_path_buf(),
                source: Arc::new(e),
                kind,
            });
        }
    };

    let content = String::from_utf8(bytes).map_err(|e| IoError::Utf8 {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(content)
}

pub(crate) fn parse_tokens_from_str(
    content: &str,
    path: &Path,
) -> Result<proc_macro2::TokenStream, IoError> {
    let parsed = syn::parse_file(content).map_err(|e| IoError::ParseError {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    Ok(parsed.into_token_stream())
}

pub(crate) fn extract_snippet_str(
    content: &str,
    start: usize,
    end: usize,
    path: &Path,
) -> Result<String, IoError> {
    if start > end || end > content.len() {
        return Err(IoError::OutOfRange {
            path: path.to_path_buf(),
            start_byte: start,
            end_byte: end,
            file_len: content.len(),
        });
    }

    if !content.is_char_boundary(start) || !content.is_char_boundary(end) {
        return Err(IoError::InvalidCharBoundary {
            path: path.to_path_buf(),
            start_byte: start,
            end_byte: end,
        });
    }

    Ok(content[start..end].to_string())
}

// async fn handle_read_snippet_batch(requests: Vec<EmbeddingData>, semaphore: Arc<Semaphore>) -> Vec<Result<String, PlokeError>> { ... }
// async fn handle_read_snippet_batch_with_roots(requests: Vec<EmbeddingData>, semaphore: Arc<Semaphore>, roots: Option<Arc<Vec<PathBuf>>>) -> Vec<Result<String, PlokeError>> { ... }

// async fn process_file(file_path: PathBuf, requests: Vec<OrderedRequest>, semaphore: Arc<Semaphore>) -> Vec<(usize, Result<String, PlokeError>)> { ... }
// async fn process_file_with_roots(file_path: PathBuf, requests: Vec<OrderedRequest>, semaphore: Arc<Semaphore>, roots: Option<Arc<Vec<PathBuf>>>) -> Vec<(usize, Result<String, PlokeError>)> { ... }

// Related path policy helper (or via path_policy module):
fn path_within_roots(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

pub async fn generate_hash_for_file(
    abs_path: &Path,
    namespace: uuid::Uuid,
) -> Result<TrackingHash, IoError> {
    let contents = read_file_to_string_abs(abs_path).await?;
    let tokens = parse_tokens_from_str(&contents, abs_path)?;

    let new_hash = TrackingHash::generate(namespace, abs_path, &tokens);
    Ok(new_hash)
}

pub async fn read_and_compute_filehash(
    abs_path: &Path,
    namespace: uuid::Uuid,
) -> Result<FileHashData, IoError> {
    let contents = read_file_to_string_abs(abs_path).await?;
    let tokens = parse_tokens_from_str(&contents, abs_path)?;

    let new_hash = TrackingHash::generate(namespace, abs_path, &tokens);
    let file_data = FileHashData::from((new_hash, contents));
    Ok(file_data)
}

#[derive(Clone, Debug)]
pub struct FileHashData {
    pub hash: TrackingHash,
    pub contents: String,
}

impl From<(TrackingHash, String)> for FileHashData {
    fn from(value: (TrackingHash, String)) -> Self {
        Self {
            hash: value.0,
            contents: value.1,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
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

    pub(crate) fn tracking_hash(content: &str) -> TrackingHash {
        let file = syn::parse_file(content).expect("Failed to parse content");
        let tokens = file.into_token_stream();
        let file_path = PathBuf::from("placeholder.txt");
        TrackingHash::generate(ploke_core::PROJECT_NAMESPACE_UUID, &file_path, &tokens)
    }

    // Helper that allows specifying namespace so tests align with per-request verification
    pub(crate) fn tracking_hash_with_path_ns(
        content: &str,
        file_path: &std::path::Path,
        namespace: Uuid,
    ) -> TrackingHash {
        let file = syn::parse_file(content).expect("Failed to parse content");
        let tokens = file.into_token_stream();

        TrackingHash::generate(namespace, file_path, &tokens)
    }

    // Tests to move here:
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
                    file_tracking_hash: tracking_hash_with_path_ns(&content, &file_path, namespace),
                    node_tracking_hash: tracking_hash_with_path_ns(&content, &file_path, namespace),
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
                file_tracking_hash: tracking_hash_with_path_ns(content, &file_path, namespace),

                node_tracking_hash: tracking_hash_with_path_ns(content, &file_path, namespace),
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
            file_tracking_hash: tracking_hash_with_path_ns(content, &file_path, namespace),

            node_tracking_hash: tracking_hash_with_path_ns(content, &file_path, namespace),
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
                file_tracking_hash: tracking_hash_with_path_ns(content1, &file_path1, namespace),

                node_tracking_hash: tracking_hash_with_path_ns(content1, &file_path1, namespace),
                start_byte: content1.find("valid").unwrap(),
                end_byte: content1.find("valid").unwrap() + 5,
                id: Uuid::new_v4(),
                name: "any_name".to_string(),
                namespace,
            },
            // Invalid request: non-existent file
            EmbeddingData {
                file_path: non_existent_file.clone(),
                file_tracking_hash: tracking_hash_with_path_ns(
                    "fn dummy() {}",
                    &non_existent_file,
                    namespace,
                ),

                node_tracking_hash: tracking_hash_with_path_ns(
                    "fn dummy() {}",
                    &non_existent_file,
                    namespace,
                ),
                start_byte: 0,
                end_byte: 10,
                id: Uuid::new_v4(),
                name: "any_name".to_string(),
                namespace,
            },
            // Valid request 2: "another"
            EmbeddingData {
                file_path: file_path2.clone(),
                file_tracking_hash: tracking_hash_with_path_ns(content2, &file_path2, namespace),

                node_tracking_hash: tracking_hash_with_path_ns(content2, &file_path2, namespace),
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
                file_tracking_hash: tracking_hash_with_path_ns(content1, &file_path1, namespace),

                node_tracking_hash: tracking_hash_with_path_ns(content1, &file_path1, namespace),
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
            file_tracking_hash: tracking_hash_with_path_ns(content, &file_path, namespace),

            node_tracking_hash: tracking_hash_with_path_ns(content, &file_path, namespace),
            start_byte: content.find("こ").unwrap(),
            end_byte: content.find("こ").unwrap() + "こ".len(),
            id: Uuid::new_v4(),
            name: "any_name".to_string(),
            namespace,
        };

        // Invalid snippet: partial multi-byte character
        let invalid_request = EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: tracking_hash_with_path_ns(content, &file_path, namespace),

            node_tracking_hash: tracking_hash_with_path_ns(content, &file_path, namespace),
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
            file_tracking_hash: tracking_hash_with_path_ns(content, &file_path, namespace),

            node_tracking_hash: tracking_hash_with_path_ns(content, &file_path, namespace),
            start_byte: 10,
            end_byte: 5,
            id: Uuid::new_v4(),
            name: "any_name".to_string(),
            namespace,
        };

        // Case 2: start_byte == end_byte (but file is shorter)
        let equal_request = EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: tracking_hash_with_path_ns(content, &file_path, namespace),

            node_tracking_hash: tracking_hash_with_path_ns(content, &file_path, namespace),
            start_byte: 100,
            end_byte: 100,
            id: Uuid::new_v4(),
            name: "name".to_string(),
            namespace,
        };

        let requests = vec![reverse_request, equal_request];
        let results = io_manager.get_snippets_batch(requests).await.unwrap();

        assert!(
            matches!(
                results[0].as_ref().unwrap_err(),
                PlokeError::Fatal(FatalError::FileOperation { .. }),
            ),
            "actual error found: {:?}",
            results[0]
        );

        assert!(
            matches!(
                results[1].as_ref().unwrap_err(),
                PlokeError::Fatal(FatalError::FileOperation { .. })
            ),
            "actual error found: {:?}",
            results[1]
        );
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
                file_tracking_hash: tracking_hash_with_path_ns(&content, &file_path, namespace),

                node_tracking_hash: tracking_hash_with_path_ns(&content, &file_path, namespace),
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
    async fn test_large_file_snippet_extraction() {
        let io_manager = IoManagerHandle::new();
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("large.rs");

        // Build a ~2MB file of ASCII-safe content to ensure char boundaries align with bytes
        let mut content = String::new();
        for i in 0..200_000 {
            content.push_str(&format!("const A_{:06}: usize = {};\n", i, i));
        }
        std::fs::write(&file_path, &content).unwrap();

        let namespace = uuid::Uuid::nil();
        let start = content.find("A_000100").unwrap();
        let end = start + "A_000100".len();

        let req = EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: tracking_hash_with_path_ns(&content, &file_path, namespace),
            node_tracking_hash: tracking_hash_with_path_ns(&content, &file_path, namespace),
            start_byte: start,
            end_byte: end,
            id: uuid::Uuid::new_v4(),
            name: "A_000100".to_string(),
            namespace,
        };

        let results = io_manager.get_snippets_batch(vec![req]).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().unwrap(), "A_000100");
    }

    #[tokio::test]
    async fn test_mixed_batch_hash_mismatch_per_request() {
        let io_manager = IoManagerHandle::new();
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("mixed.rs");
        let content = r#"
            fn alpha() {}
            fn beta() {}
        "#;
        std::fs::write(&file_path, content).unwrap();

        let namespace = uuid::Uuid::new_v4();
        let good_hash = tracking_hash_with_path_ns(content, &file_path, namespace);
        let bad_hash = TrackingHash(uuid::Uuid::new_v4());

        let alpha_start = content.find("alpha").unwrap();
        let alpha_end = alpha_start + "alpha".len();
        let beta_start = content.find("beta").unwrap();
        let beta_end = beta_start + "beta".len();

        let reqs = vec![
            EmbeddingData {
                file_path: file_path.clone(),
                file_tracking_hash: good_hash,
                node_tracking_hash: good_hash,
                start_byte: alpha_start,
                end_byte: alpha_end,
                id: uuid::Uuid::new_v4(),
                name: "alpha".to_string(),
                namespace,
            },
            EmbeddingData {
                file_path: file_path.clone(),
                file_tracking_hash: bad_hash,
                node_tracking_hash: bad_hash,
                start_byte: beta_start,
                end_byte: beta_end,
                id: uuid::Uuid::new_v4(),
                name: "beta".to_string(),
                namespace,
            },
        ];

        let results = io_manager.get_snippets_batch(reqs).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].as_ref().unwrap(), "alpha");
        assert!(matches!(
            results[1].as_ref().unwrap_err(),
            PlokeError::Fatal(FatalError::ContentMismatch { .. })
        ));
    }

    #[tokio::test]
    async fn test_parse_error_invalid_rust() {
        let io_manager = IoManagerHandle::new();
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("invalid.rs");
        std::fs::write(&file_path, "fn {").unwrap();

        let namespace = uuid::Uuid::nil();
        let req = EmbeddingData {
            file_path: file_path.clone(),
            file_tracking_hash: TrackingHash(uuid::Uuid::new_v4()),
            node_tracking_hash: TrackingHash(uuid::Uuid::new_v4()),
            start_byte: 0,
            end_byte: 1,
            id: uuid::Uuid::new_v4(),
            name: "invalid".to_string(),
            namespace,
        };

        let results = io_manager.get_snippets_batch(vec![req]).await.unwrap();
        assert!(matches!(
            results[0],
            Err(PlokeError::Fatal(FatalError::SyntaxError(..)))
        ));
    }

    #[tokio::test]
    async fn test_reject_relative_path() {
        let io_manager = IoManagerHandle::new();
        let rel_path = std::path::PathBuf::from("relative/path.rs");

        let namespace = uuid::Uuid::nil();
        let req = EmbeddingData {
            file_path: rel_path.clone(),
            file_tracking_hash: TrackingHash(uuid::Uuid::new_v4()),
            node_tracking_hash: TrackingHash(uuid::Uuid::new_v4()),
            start_byte: 0,
            end_byte: 0,
            id: uuid::Uuid::new_v4(),
            name: "rel".to_string(),
            namespace,
        };

        let results = io_manager.get_snippets_batch(vec![req]).await.unwrap();
        assert!(matches!(
            results[0],
            Err(PlokeError::Fatal(FatalError::FileOperation { .. }))
        ));
    }

    #[tokio::test]
    async fn test_roots_enforcement_basic() {
        let root_dir = tempfile::tempdir().unwrap();
        let other_dir = tempfile::tempdir().unwrap();

        // File inside the configured root
        let in_path = root_dir.path().join("in.rs");
        let in_content = "fn inside() {}";
        std::fs::write(&in_path, in_content).unwrap();

        // File outside the configured root
        let out_path = other_dir.path().join("out.rs");
        let out_content = "fn outside() {}";
        std::fs::write(&out_path, out_content).unwrap();

        let handle = IoManagerHandle::builder()
            .with_roots(vec![root_dir.path().to_path_buf()])
            .build();

        let namespace = uuid::Uuid::nil();

        let ok_req = EmbeddingData {
            file_path: in_path.clone(),
            file_tracking_hash: tracking_hash_with_path_ns(in_content, &in_path, namespace),
            node_tracking_hash: tracking_hash_with_path_ns(in_content, &in_path, namespace),
            start_byte: in_content.find("inside").unwrap(),
            end_byte: in_content.find("inside").unwrap() + "inside".len(),
            id: uuid::Uuid::new_v4(),
            name: "inside".to_string(),
            namespace,
        };

        let bad_req = EmbeddingData {
            file_path: out_path.clone(),
            file_tracking_hash: tracking_hash_with_path_ns(out_content, &out_path, namespace),
            node_tracking_hash: tracking_hash_with_path_ns(out_content, &out_path, namespace),
            start_byte: 0,
            end_byte: 1,
            id: uuid::Uuid::new_v4(),
            name: "bad".to_string(),
            namespace,
        };

        let results = handle
            .get_snippets_batch(vec![ok_req, bad_req])
            .await
            .unwrap();

        assert_eq!(results[0].as_ref().unwrap(), "inside");
        assert!(matches!(
            results[1],
            Err(PlokeError::Fatal(FatalError::FileOperation { .. }))
        ));
    }
}
