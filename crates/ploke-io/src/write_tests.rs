use crate::handle::IoManagerHandle;
use ploke_core::{TrackingHash, WriteResult, WriteSnippetData, PROJECT_NAMESPACE_UUID};
use quote::ToTokens;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;
use uuid::Uuid;

fn compute_hash(content: &str, file_path: &PathBuf, namespace: Uuid) -> TrackingHash {
    let file = syn::parse_file(content).expect("Failed to parse content");
    let tokens = file.into_token_stream();
    TrackingHash::generate(namespace, file_path, &tokens)
}

#[tokio::test]
async fn test_write_splice_and_hash_recompute() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("splice.rs");
    let initial = "fn foo() {}\n";
    fs::write(&file_path, initial).unwrap();

    let namespace = PROJECT_NAMESPACE_UUID;
    let expected = compute_hash(initial, &file_path, namespace);

    let start = initial.find("foo").unwrap();
    let end = start + "foo".len();

    let req = WriteSnippetData {
        id: Uuid::new_v4(),
        name: "test_node".to_string(),
        file_path: file_path.clone(),
        expected_file_hash: expected,
        start_byte: start,
        end_byte: end,
        replacement: "bar".to_string(),
        namespace,
    };

    let handle = IoManagerHandle::new();
    let results = handle.write_snippets_batch(vec![req]).await.unwrap();
    assert_eq!(results.len(), 1);
    let wr = results.into_iter().next().unwrap().expect("write should succeed");

    let new_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(new_content, "fn bar() {}\n");

    // Verify returned hash matches recomputed hash from file content
    let recomputed = compute_hash(&new_content, &file_path, namespace);
    assert_eq!(wr.new_file_hash, recomputed);

    handle.shutdown().await;
}

#[tokio::test]
async fn test_write_invalid_range() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("range.rs");
    let initial = "fn a() {}\n";
    fs::write(&file_path, initial).unwrap();

    let namespace = PROJECT_NAMESPACE_UUID;
    let expected = compute_hash(initial, &file_path, namespace);

    // Invalid: end beyond length
    let req = WriteSnippetData {
        id: Uuid::new_v4(),
        name: "range_node".to_string(),
        file_path: file_path.clone(),
        expected_file_hash: expected,
        start_byte: 0,
        end_byte: initial.len() + 10,
        replacement: "x".to_string(),
        namespace,
    };

    let handle = IoManagerHandle::new();
    let results = handle.write_snippets_batch(vec![req]).await.unwrap();
    assert!(results[0].is_err(), "expected OutOfRange error");

    handle.shutdown().await;
}

#[tokio::test]
async fn test_write_roots_enforcement() {
    let root_dir = tempdir().unwrap();
    let outside_dir = tempdir().unwrap();

    // File outside the configured root
    let file_path = outside_dir.path().join("outside.rs");
    let initial = "fn z() {}\n";
    fs::write(&file_path, initial).unwrap();

    let namespace = PROJECT_NAMESPACE_UUID;
    let expected = compute_hash(initial, &file_path, namespace);

    let req = WriteSnippetData {
        id: Uuid::new_v4(),
        name: "outside_node".to_string(),
        file_path: file_path.clone(),
        expected_file_hash: expected,
        start_byte: 0,
        end_byte: initial.len(),
        replacement: "fn z() { let _ = 1; }\n".to_string(),
        namespace,
    };

    // Build handle with roots restricted to root_dir
    let handle = crate::builder::IoManagerBuilder::default()
        .with_roots([root_dir.path().to_path_buf()])
        .build();

    let results = handle.write_snippets_batch(vec![req]).await.unwrap();
    assert!(
        results[0].is_err(),
        "expected write to be rejected due to path outside configured roots"
    );

    handle.shutdown().await;
}
