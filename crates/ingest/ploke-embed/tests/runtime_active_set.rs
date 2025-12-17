use std::sync::Arc;

use httpmock::prelude::*;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

use ploke_core::embeddings::{EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape};
use ploke_db::multi_embedding::db_ext::EmbeddingExt;
use ploke_db::Database;
use ploke_embed::{
    config::OpenRouterConfig,
    indexer::{EmbeddingProcessor, EmbeddingSource},
    providers::openrouter::OpenRouterBackend,
    runtime::EmbeddingRuntime,
};

static ENV_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

fn set_env(url: &str) {
    if std::env::var("OPENROUTER_API_KEY")
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        std::env::set_var("OPENROUTER_API_KEY", "test-key");
    }
    std::env::set_var("OPENROUTER_EMBEDDINGS_URL", url);
}

fn cfg(model: &str, dims: usize) -> OpenRouterConfig {
    OpenRouterConfig {
        model: model.to_string(),
        dimensions: Some(dims),
        request_dimensions: None,
        max_in_flight: 2,
        requests_per_second: None,
        max_attempts: 2,
        initial_backoff_ms: 1,
        max_backoff_ms: 5,
        input_type: Some("code-snippet".into()),
        timeout_secs: 5,
    }
}

#[tokio::test]
async fn runtime_swaps_active_set_and_embedder_dimensions() -> Result<(), Box<dyn std::error::Error>>
{
    let _env_guard = ENV_MUTEX.lock().await;

    let server_a = MockServer::start();
    let body_a = serde_json::json!({
        "data": [
            { "index": 0, "embedding": [0.1, 0.2, 0.3] }
        ],
        "model": "openai/text-embedding-3-small",
        "id": "req-a"
    })
    .to_string();
    let _m_a = server_a.mock(|when, then| {
        when.method(POST).path("/v1/embeddings");
        then.status(200).body(body_a.clone());
    });
    set_env(&server_a.url("/v1/embeddings"));

    let backend_a = OpenRouterBackend::new(&cfg("openai/text-embedding-3-small", 3))?;
    let runtime = EmbeddingRuntime::with_default_set(EmbeddingProcessor::new(
        EmbeddingSource::OpenRouter(backend_a),
    ));

    let out_a = runtime
        .generate_embeddings(vec!["alpha".into()])
        .await
        .expect("first embedding call");
    assert_eq!(out_a[0].len(), 3, "first embedder should return 3 dims");
    assert_eq!(runtime.dimensions()?, 3);

    let db = Database::init_with_schema()?;
    db.setup_multi_embedding()?;

    let server_b = MockServer::start();
    let body_b = serde_json::json!({
        "data": [
            { "index": 0, "embedding": [1.0, 2.0, 3.0, 4.0] }
        ],
        "model": "openai/text-embedding-3-large",
        "id": "req-b"
    })
    .to_string();
    let _m_b = server_b.mock(|when, then| {
        when.method(POST).path("/v1/embeddings");
        then.status(200).body(body_b.clone());
    });

    let new_set = EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("openrouter"),
        EmbeddingModelId::new_from_str("openai/text-embedding-3-large"),
        EmbeddingShape::new_dims_default(4),
    );
    let backend_b = OpenRouterBackend::new(&cfg("openai/text-embedding-3-large", 4))?;
    runtime.activate(
        &db,
        new_set.clone(),
        Arc::new(EmbeddingProcessor::new(EmbeddingSource::OpenRouter(backend_b))),
    )?;
    set_env(&server_b.url("/v1/embeddings"));

    let out_b = runtime
        .generate_embeddings(vec!["beta".into()])
        .await
        .expect("second embedding call");
    assert_eq!(out_b[0].len(), 4, "second embedder should return 4 dims");
    assert_eq!(runtime.dimensions()?, 4);

    let active = runtime.current_active_set()?;
    assert_eq!(active.model, new_set.model);
    assert!(
        db.is_vector_embedding_registered(&new_set)?,
        "vector relation should exist for activated set"
    );

    Ok(())
}
