// Test inventory to guide migration (comment-only)

// Read Path Tests:
// - test_get_snippets_batch_preserves_order
// - test_content_mismatch
// - test_io_errors
// - test_concurrency_throttling
// - test_seek_errors
// - test_zero_length_snippet
// - test_partial_failure_handling
// - test_concurrent_modification
// - test_utf8_error
// - test_zero_byte_files
// - test_multi_byte_unicode_boundaries
// - test_invalid_byte_ranges
// - test_exact_semaphore_limit
// - test_permission_denied
// - test_large_file_snippet_extraction
// - test_mixed_batch_hash_mismatch_per_request
// - test_parse_error_invalid_rust
// - test_reject_relative_path
// - test_roots_enforcement_basic (shared with path policy)

// Scan Path Tests:
// - test_scan_changes_preserves_input_order
// - test_scan_changes_bounded_concurrency

// Builder/Config Tests:
// - test_fd_limit_precedence_and_clamp
// - test_fd_limit_env_applied_when_no_builder
// - test_fd_limit_default_from_soft
// - test_fd_limit_default_on_error

// Actor/Runtime Tests:
// - test_actor_shutdown_during_ops
// - test_read_during_shutdown
// - test_send_during_shutdown
// - test_handle_read_snippet_batch
// - test_handle_request (ignored)

// Misc/Hashing Semantics:
// - test_token_stream_sensitivity

#![cfg(test)]
use crate::read::tests::tracking_hash;

use super::*;
use ploke_common::{fixtures_crates_dir, workspace_root};
use ploke_test_utils::setup_db_full_multi_embedding;
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
        .with_target("ploke-db", tracing::Level::INFO); // Default for other crates

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

// Helper function for tests that need path-specific hashing
fn tracking_hash_with_path(content: &str, file_path: &std::path::Path) -> TrackingHash {
    let file = syn::parse_file(content).expect("Failed to parse content");
    let tokens = file.into_token_stream();

    TrackingHash::generate(ploke_core::PROJECT_NAMESPACE_UUID, file_path, &tokens)
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
    init_test_tracing(tracing::Level::ERROR);

    let fixture_name = "fixture_nodes";
    let crate_path = fixtures_crates_dir().join(fixture_name);
    let project_root = workspace_root(); // Use workspace root for context
    let discovery_output = run_discovery_phase(&project_root, std::slice::from_ref(&crate_path))
        .unwrap_or_else(|e| panic!("Phase 1 Discovery failed for {}: {:?}", fixture_name, e));

    tracing::info!("🚀 Starting test");
    tracing::debug!("Initializing test database");

    let embedding_data = setup_db_full_embeddings(fixture_name)?;
    let flat_nodes: Vec<_> = embedding_data
        .iter()
        .flat_map(|emb| emb.v.iter().cloned())
        .collect();
    // Add temporary tracing to inspect embedding data
    for data in &embedding_data {
        let ty = data.ty;
        for tyemb in data.iter() {
            tracing::trace!(target: "handle_data",
                "EmbeddingData: ty={:?}, id={}, file={}, range={}..{}",
                ty,
                tyemb.id,
                tyemb.file_path.display(),
                tyemb.start_byte,
                tyemb.end_byte
            );
        }
    }
    assert_ne!(0, flat_nodes.len());
    tracing::debug!(target: "handle", "{:?}", flat_nodes.len());

    for data in embedding_data.iter() {
        tracing::trace!(target: "handle", "{:#?}", data);
    }
    let embeddings_count = flat_nodes.len();
    let semaphore = Arc::new(Semaphore::new(50)); // defaulting to safe value

    let snippets = IoManager::handle_read_snippet_batch(flat_nodes.clone(), semaphore).await;
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
                if let Some(embed_data) = flat_nodes.iter().find(|emb| snip.contains(&emb.name)) {
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

    for (i, (s, embed_data)) in snippets.iter().zip(flat_nodes.iter()).enumerate() {
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

#[tokio::test]
async fn test_read_file_plain_success() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("read_me.txt");
    let contents = "hello ns_read\nsecond line";
    fs::write(&file_path, contents).unwrap();

    let handle = IoManagerHandle::new();
    let req = ReadFileRequest {
        file_path: file_path.clone(),
        range: None,
        max_bytes: Some(1024),
        strategy: ReadStrategy::Plain,
    };

    let resp = handle.read_file(req).await.unwrap().unwrap();
    assert!(resp.exists);
    assert_eq!(resp.file_path, file_path);
    assert_eq!(resp.byte_len, Some(contents.len() as u64));
    assert!(!resp.truncated);
    assert_eq!(resp.content.as_deref(), Some(contents));

    handle.shutdown().await;
}

#[tokio::test]
async fn test_read_file_missing_returns_exists_false() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("nope.md");

    let handle = IoManagerHandle::new();
    let req = ReadFileRequest {
        file_path: file_path.clone(),
        range: None,
        max_bytes: Some(256),
        strategy: ReadStrategy::Plain,
    };

    let resp = handle.read_file(req).await.unwrap().unwrap();
    assert!(!resp.exists);
    assert_eq!(resp.file_path, file_path);
    assert!(resp.byte_len.is_none());
    assert!(resp.content.is_none());
    assert!(!resp.truncated);

    handle.shutdown().await;
}

#[tokio::test]
async fn test_read_file_truncates_when_limit_set() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("truncate.txt");
    let contents = "line one\nline two\nline three";
    fs::write(&file_path, contents).unwrap();

    let handle = IoManagerHandle::new();
    let req = ReadFileRequest {
        file_path: file_path.clone(),
        range: None,
        max_bytes: Some(8),
        strategy: ReadStrategy::Plain,
    };

    let resp = handle.read_file(req).await.unwrap().unwrap();
    assert!(resp.exists);
    assert_eq!(resp.file_path, file_path);
    assert_eq!(resp.byte_len, Some(contents.len() as u64));
    assert!(resp.truncated);
    assert_eq!(resp.content.as_deref(), Some("line one"));

    handle.shutdown().await;
}

fn percent(i: usize, t: usize) -> f32 {
    i as f32 / (t as f32) * 100.0
}

/// Compute effective file-descriptor-based concurrency limit given optional sources.
/// Precedence: builder override > env override > OS soft limit heuristic > default (50).
pub(crate) fn compute_fd_limit_from_inputs(
    soft_nofile: Option<u64>,
    env_override: Option<usize>,
    builder_override: Option<usize>,
) -> usize {
    if let Some(n) = builder_override {
        return n.clamp(4, 1024);
    }
    if let Some(n) = env_override {
        return n.clamp(4, 1024);
    }
    if let Some(soft) = soft_nofile {
        return std::cmp::min(100, (soft / 3) as usize);
    }
    50
}

#[test]
fn test_fd_limit_precedence_and_clamp() {
    // builder override below min should clamp to 4 and take precedence over env/soft
    let r = compute_fd_limit_from_inputs(Some(300), Some(9999), Some(2));
    assert_eq!(r, 4);
}

#[test]
fn test_fd_limit_env_applied_when_no_builder() {
    let r = compute_fd_limit_from_inputs(Some(300), Some(16), None);
    assert_eq!(r, 16);
}

#[test]
fn test_fd_limit_default_from_soft() {
    // soft=90 => soft/3=30, min(100,30)=30
    let r = compute_fd_limit_from_inputs(Some(90), None, None);
    assert_eq!(r, 30);
}

#[test]
fn test_fd_limit_default_on_error() {
    // No soft/env/builder => default 50
    let r = compute_fd_limit_from_inputs(None, None, None);
    assert_eq!(r, 50);
}

// pub fn setup_db_full_embeddings(fixture: &'static str) -> Result<Vec<ploke_core::EmbeddingData>, ploke_error::Error> {
//     let db = ploke_db::Database::new( setup_db_full(fixture)? );
//     let embedding_data = db.get_nodes_for_embedding(100, None)?;
//     Ok(embedding_data)
// }
