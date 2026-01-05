#![cfg(feature = "live_api_tests")]

use std::path::{Path, PathBuf};

use ploke_embed::config::{OpenRouterConfig, TruncatePolicy};
use ploke_embed::providers::openrouter::OpenRouterBackend;

fn require_live_gate() {
    let key_ok = std::env::var("OPENROUTER_API_KEY")
        .ok()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !key_ok {
        panic!(
            "live gate not satisfied: set OPENROUTER_API_KEY (tests use live OpenRouter embeddings)"
        );
    }

    if std::env::var("OPENROUTER_EMBEDDINGS_URL").is_ok() {
        panic!(
            "live gate not satisfied: OPENROUTER_EMBEDDINGS_URL is set; unset it to force real endpoint"
        );
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .to_path_buf()
}

fn snippet_path() -> PathBuf {
    repo_root().join("fixtures/snippets/graph_access.rs")
}

fn env_model_and_dims() -> (String, u32) {
    let model = std::env::var("PLOKE_OPENROUTER_EMBED_MODEL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "mistralai/codestral-embed-2505".to_string());
    let dims = std::env::var("PLOKE_OPENROUTER_EMBED_DIMS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1536);
    (model, dims)
}

fn openrouter_cfg(model: &str, dims: u32) -> OpenRouterConfig {
    OpenRouterConfig {
        model: model.to_string(),
        dimensions: Some(dims as usize),
        request_dimensions: None,
        max_in_flight: 1,
        requests_per_second: Some(1),
        max_attempts: 3,
        initial_backoff_ms: 250,
        max_backoff_ms: 10_000,
        input_type: Some("code-snippet".into()),
        timeout_secs: 30,
        truncate_policy: TruncatePolicy::Truncate,
    }
}

/// Live repro for the failing `GraphAccess` snippet from `syn_parser`.
///
/// Enable explicitly with:
/// `cargo test -p ploke-embed --test openrouter_live_snippet_repro -- --ignored --nocapture --test-threads=1`
#[tokio::test]
#[ignore = "hits live OpenRouter embeddings; requires OPENROUTER_API_KEY"]
async fn live_openrouter_snippet_repro() {
    require_live_gate();

    let path = snippet_path();
    let snippet = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read snippet file {}: {e}", path.display()));
    if snippet.trim().is_empty() {
        panic!(
            "snippet fixture is empty: populate {} with the failing snippet content",
            path.display()
        );
    }

    let (model, dims) = env_model_and_dims();
    let backend = OpenRouterBackend::new(&openrouter_cfg(&model, dims))
        .unwrap_or_else(|e| panic!("failed to init OpenRouter backend: {e:?}"));

    let embeddings = backend
        .compute_batch(vec![snippet], None)
        .await
        .unwrap_or_else(|e| panic!("embedding request failed: {e:?}"));

    assert_eq!(
        embeddings.len(),
        1,
        "expected exactly one embedding response"
    );
    assert_eq!(
        embeddings[0].len(),
        dims as usize,
        "embedding dimension mismatch"
    );
}
