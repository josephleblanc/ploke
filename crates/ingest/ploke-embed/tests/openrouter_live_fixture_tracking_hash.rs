#![cfg(feature = "live_api_tests")]

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use cozo::{DataValue, Vector};
use ploke_core::embeddings::{
    EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::{multi_embedding::db_ext::EmbeddingExt as _, Database};
use ploke_db::{multi_embedding::hnsw_ext::HnswExt as _, DbError};
use ploke_embed::{
    cancel_token::CancellationToken,
    config::OpenRouterConfig,
    indexer::{EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexerCommand, IndexerTask},
    providers::openrouter::OpenRouterBackend,
    runtime::EmbeddingRuntime,
};
use ploke_error::Error as PlokeError;
use ploke_io::IoManagerHandle;
use serde::Serialize;
use tokio::sync::{broadcast, mpsc};

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

    // Discipline: these tests must hit the real endpoint.
    if std::env::var("OPENROUTER_EMBEDDINGS_URL").is_ok() {
        panic!(
            "live gate not satisfied: OPENROUTER_EMBEDDINGS_URL is set; unset it to force real endpoint"
        );
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "cosine_similarity requires equal lengths");
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for (&x, &y) in a.iter().zip(b.iter()) {
        let xf = x as f64;
        let yf = y as f64;
        dot += xf * yf;
        na += xf * xf;
        nb += yf * yf;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    (dot / (na.sqrt() * nb.sqrt())) as f32
}

fn ts_slug() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0));
    format!("{}-{}", now.as_secs(), now.subsec_nanos())
}

fn repo_target_test_output_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/ingest/ploke-embed
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("target")
        .join("test-output")
}

fn sanitize_path_component(s: &str) -> String {
    s.replace('/', "_slash_").replace('\\', "_")
}

fn write_json(path: &Path, value: &impl Serialize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create artifact dir");
    }
    let bytes = serde_json::to_vec_pretty(value).expect("failed to serialize artifact json");
    fs::write(path, bytes).expect("failed to write artifact json");
}

fn openrouter_cfg(model: &str, dims: usize) -> OpenRouterConfig {
    OpenRouterConfig {
        model: model.to_string(),
        dimensions: Some(dims),
        max_in_flight: 2,
        requests_per_second: None,
        max_attempts: 5,
        initial_backoff_ms: 250,
        max_backoff_ms: 10_000,
        input_type: Some("code-snippet".into()),
        timeout_secs: 30,
    }
}

#[derive(Debug, Serialize)]
struct SmokeArtifact {
    model: String,
    dims: usize,
    batch_size: usize,
    vectors_head8: Vec<Vec<f32>>,
}

#[derive(Debug, Serialize)]
struct ParityCosineArtifact {
    local_model: String,
    remote_model: String,
    dims: usize,
    sims: Vec<(String, f32)>,
    p50: f32,
    min: f32,
}

#[tokio::test]
async fn live_openrouter_embed_two_snippets_smoke() -> Result<(), Box<dyn std::error::Error>> {
    require_live_gate();

    let model = "sentence-transformers/all-minilm-l6-v2";
    let dims = 384usize;

    let backend = OpenRouterBackend::new(&openrouter_cfg(model, dims))?;
    let processor = EmbeddingProcessor::new(EmbeddingSource::OpenRouter(backend));

    let inputs = vec![
        "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
        "pub struct Foo { pub x: usize }".to_string(),
        "impl Foo { pub fn inc(&mut self) { self.x += 1; } }".to_string(),
    ];

    let out = processor.generate_embeddings(inputs.clone()).await?;
    assert_eq!(out.len(), inputs.len());
    for v in &out {
        assert_eq!(v.len(), dims);
        assert!(v.iter().all(|f| f.is_finite()), "non-finite float in vec");
    }

    let head8 = out
        .iter()
        .map(|v| v.iter().copied().take(8).collect::<Vec<_>>())
        .collect::<Vec<_>>();

    let artifact = SmokeArtifact {
        model: model.to_string(),
        dims,
        batch_size: inputs.len(),
        vectors_head8: head8,
    };

    let out_path = repo_target_test_output_dir()
        .join("openrouter_embed_smoke")
        .join(format!("{}.json", ts_slug()));
    write_json(&out_path, &artifact);

    Ok(())
}

#[derive(Debug, Serialize)]
struct FixtureRunArtifact {
    fixture: String,
    provider: String,
    model: String,
    dims: usize,
    hash_id: String,
    rel_name: String,

    pending_before_nonfiles: usize,
    pending_after_nonfiles: usize,
    embedded_rows_for_set: usize,

    hnsw_created: bool,
    hnsw_registered: bool,

    sample_node_ids: Vec<String>,
    elapsed_ms: u128,
}

async fn run_fixture_tracking_hash_index(
    model: &str,
    dims: usize,
) -> Result<FixtureRunArtifact, Box<dyn std::error::Error>> {
    use ploke_db::multi_embedding::schema::EmbeddingSetExt as _;

    let fixture = "fixture_tracking_hash";

    let cozo_db = ploke_test_utils::setup_db_full_multi_embedding(fixture)?;
    let mut db = Database::new(cozo_db);

    // TODO:active-embedding-set 2025-12-15
    // update the active embedding set functions to correctly use Arc<RwLock<>> within these
    // functions.
    let new_set = EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("openrouter"),
        EmbeddingModelId::new_from_str(model),
        EmbeddingShape::new_dims_default(dims as u32),
    );
    db.set_active_set(new_set)?;

    // IMPORTANT: Cozo will error on queries that reference an embedding relation that doesn't exist.
    // IndexerTask::run does this setup internally, but the test harness also queries counts *before*
    // running the indexer. Ensure the active embedding set + vector relation exist first.
    db.ensure_embedding_set_relation()?;

    // TODO:active-embedding-set 2025-12-15
    // update the active embedding set functions to correctly use Arc<RwLock<>> within these
    // functions.
    let active_embedding_set = db.with_active_set(|set| set.clone())?;
    db.put_embedding_set(&active_embedding_set)?;
    db.ensure_vector_embedding_relation(&active_embedding_set)?;

    let pending_before_nonfiles = db.count_unembedded_nonfiles()?;

    let backend = OpenRouterBackend::new(&openrouter_cfg(model, dims))?;
    let embedding_runtime = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        EmbeddingProcessor::new(EmbeddingSource::OpenRouter(backend)),
    ));

    let db = std::sync::Arc::new(db);
    let io = IoManagerHandle::new();

    let (cancellation_token, cancel_handle) = CancellationToken::new();

    let idx = IndexerTask::new(
        std::sync::Arc::clone(&db),
        io,
        embedding_runtime,
        cancellation_token,
        cancel_handle,
        8,
    );

    let (progress_tx, mut progress_rx) = broadcast::channel(32);
    let progress_tx = std::sync::Arc::new(progress_tx);
    let (_control_tx, control_rx) = mpsc::channel::<IndexerCommand>(1);

    let started = Instant::now();
    let handle = tokio::spawn(async move { idx.run(progress_tx, control_rx).await });

    // Drain progress until completion/failure (also ensures the sender always has a receiver).
    let index_wait = async {
        loop {
            match progress_rx.recv().await {
                Ok(status) => match status.status {
                    IndexStatus::Completed => break,
                    IndexStatus::Failed(msg) => {
                        return Err::<(), Box<dyn std::error::Error>>(
                            format!("indexing failed: {msg}; errors={:?}", status.errors).into(),
                        );
                    }
                    IndexStatus::Cancelled => {
                        return Err::<(), Box<dyn std::error::Error>>("indexing cancelled".into())
                    }
                    _ => {}
                },
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(e) => {
                    return Err::<(), Box<dyn std::error::Error>>(
                        format!("progress channel error: {e}").into(),
                    )
                }
            }
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    };

    // Hard timeout: live runs must never hang indefinitely.
    tokio::time::timeout(Duration::from_secs(20 * 60), index_wait)
        .await
        .map_err(|_| "indexing timed out (20 minutes)")??;

    // Propagate task errors.
    tokio::time::timeout(Duration::from_secs(20 * 60), handle)
        .await
        .map_err(|_| "indexing task join timed out (20 minutes)")???;

    let elapsed_ms = started.elapsed().as_millis();

    // TODO:active-embedding-set 2025-12-15
    // update the active embedding set functions to correctly use Arc<RwLock<>> within these
    // functions.
    let active = db.with_active_set(|set| set.clone())?;

    // Build HNSW for the active set.
    let hnsw_before = db.is_hnsw_index_registered(&active)?;
    db.create_embedding_index(&active).map_err(DbError::from)?;
    let hnsw_after = db.is_hnsw_index_registered(&active)?;

    let pending_after_nonfiles = db.count_unembedded_nonfiles()?;
    let embedded_rows_for_set = db.count_embeddings_for_set(&active)?;

    // Basic sanity: relation naming is Cozo-safe and includes slash substitution when relevant.
    let rel_name = active.rel_name().as_ref().to_string();

    // Sample some node ids that have embeddings.
    let sample_script = format!(
        "?[node_id] := *{}{{ node_id, vector @ 'NOW' }} :limit 8",
        active.rel_name()
    );
    let rows = db.raw_query(&sample_script).map_err(PlokeError::from)?;
    let sample_node_ids = rows
        .rows
        .into_iter()
        .filter_map(|r| r.into_iter().next())
        .map(|v| v.to_string())
        .collect::<Vec<_>>();

    Ok(FixtureRunArtifact {
        fixture: fixture.to_string(),
        provider: "openrouter".to_string(),
        model: model.to_string(),
        dims,
        hash_id: active.hash_id().to_string(),
        rel_name,
        pending_before_nonfiles,
        pending_after_nonfiles,
        embedded_rows_for_set,
        hnsw_created: !hnsw_before,
        hnsw_registered: hnsw_after,
        sample_node_ids,
        elapsed_ms,
    })
}

use std::sync::Arc;

#[tokio::test]
async fn live_openrouter_index_fixture_tracking_hash_builds_vectors_and_hnsw(
) -> Result<(), Box<dyn std::error::Error>> {
    require_live_gate();

    let model = "sentence-transformers/all-minilm-l6-v2";
    let dims = 384usize;

    let artifact = run_fixture_tracking_hash_index(model, dims).await?;

    assert_eq!(
        artifact.pending_after_nonfiles, 0,
        "expected non-file pending to reach zero for active set"
    );
    assert!(
        artifact.embedded_rows_for_set > 0,
        "expected embeddings to be written"
    );
    assert!(
        artifact.rel_name.contains("_slash_"),
        "expected rel_name to sanitize '/'"
    );
    assert!(
        artifact.hnsw_registered,
        "expected HNSW index to be registered"
    );

    let out_path = repo_target_test_output_dir()
        .join("openrouter_fixture_tracking_hash")
        .join(sanitize_path_component(model))
        .join(format!("{}.json", ts_slug()));
    write_json(&out_path, &artifact);

    Ok(())
}

#[derive(Debug, Serialize)]
struct MatrixRunSummary {
    fixture: String,
    runs: Vec<FixtureRunArtifact>,
    unique_rel_names: usize,
}

#[tokio::test]
async fn live_openrouter_matrix_fixture_tracking_hash() -> Result<(), Box<dyn std::error::Error>> {
    require_live_gate();

    let matrix: Vec<(&str, usize)> = vec![
        ("sentence-transformers/all-minilm-l6-v2", 384),
        ("openai/text-embedding-3-small", 256),
        // Optional extensions can be added once baselined:
        // ("openai/text-embedding-3-small", 384),
        // ("openai/text-embedding-3-small", 512),
    ];

    let mut runs = Vec::new();
    for (model, dims) in matrix {
        let run = run_fixture_tracking_hash_index(model, dims).await?;
        assert_eq!(
            run.pending_after_nonfiles, 0,
            "pending nonfiles should be 0"
        );
        assert!(run.embedded_rows_for_set > 0, "expected embeddings for set");
        assert!(run.hnsw_registered, "expected HNSW registered");
        runs.push(run);
    }

    let unique_rel_names: BTreeSet<String> = runs.iter().map(|r| r.rel_name.clone()).collect();
    assert_eq!(
        unique_rel_names.len(),
        runs.len(),
        "expected distinct rel_name per model/dims"
    );

    let summary = MatrixRunSummary {
        fixture: "fixture_tracking_hash".to_string(),
        runs,
        unique_rel_names: unique_rel_names.len(),
    };

    let out_path = repo_target_test_output_dir()
        .join("openrouter_fixture_tracking_hash")
        .join("matrix")
        .join(format!("{}.json", ts_slug()));
    write_json(&out_path, &summary);

    Ok(())
}

#[derive(Debug, Serialize)]
struct DimOverrideArtifact {
    model: String,
    requested_dims: usize,
    rel_name: String,
    vector_len_observed: usize,
}

#[tokio::test]
async fn live_openrouter_dimensions_override_text_embedding_3_small_256(
) -> Result<(), Box<dyn std::error::Error>> {
    require_live_gate();

    let model = "openai/text-embedding-3-small";
    let dims = 256usize;

    // Run the full fixture path (this also enforces dims at the adapter boundary).
    let _artifact = run_fixture_tracking_hash_index(model, dims).await?;

    // Also do a single-batch embed call to directly observe vector dimensionality.
    let backend = OpenRouterBackend::new(&openrouter_cfg(model, dims))?;
    let processor = EmbeddingProcessor::new(EmbeddingSource::OpenRouter(backend));
    let out = processor
        .generate_embeddings(vec!["hello world".to_string()])
        .await?;
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].len(), dims);

    // Validate relation naming rules are stable.
    let set = EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("openrouter"),
        EmbeddingModelId::new_from_str(model),
        EmbeddingShape::new_dims_default(dims as u32),
    );
    let rel = set.rel_name.as_ref().to_string();
    assert!(rel.contains("_slash_"), "expected rel_name to sanitize '/'");
    assert!(
        rel.ends_with("_256"),
        "expected rel_name to include requested dims suffix: {rel}"
    );

    let artifact = DimOverrideArtifact {
        model: model.to_string(),
        requested_dims: dims,
        rel_name: rel,
        vector_len_observed: out[0].len(),
    };

    let out_path = repo_target_test_output_dir()
        .join("openrouter_dimensions_override")
        .join(format!("{}.json", ts_slug()));
    write_json(&out_path, &artifact);

    Ok(())
}

#[tokio::test]
async fn live_openrouter_dimensions_override_db_vector_len_matches_256(
) -> Result<(), Box<dyn std::error::Error>> {
    require_live_gate();

    let fixture = "fixture_tracking_hash";
    let model = "openai/text-embedding-3-small";
    let dims = 256usize;

    let cozo_db = ploke_test_utils::setup_db_full_multi_embedding(fixture)?;
    let mut db = Database::new(cozo_db);

    let new_set = EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("openrouter"),
        EmbeddingModelId::new_from_str(model),
        EmbeddingShape::new_dims_default(dims as u32),
    );
    db.set_active_set(new_set)?;

    // Index once (remote embeddings).
    let backend = OpenRouterBackend::new(&openrouter_cfg(model, dims))?;
    let embedding_runtime = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        EmbeddingProcessor::new(EmbeddingSource::OpenRouter(backend)),
    ));
    let db = Arc::new(db);
    let io = IoManagerHandle::new();
    let (cancellation_token, cancel_handle) = CancellationToken::new();
    let idx = IndexerTask::new(
        Arc::clone(&db),
        io,
        embedding_runtime,
        cancellation_token,
        cancel_handle,
        8,
    );

    let (progress_tx, mut progress_rx) = broadcast::channel(32);
    let progress_tx = Arc::new(progress_tx);
    let (_control_tx, control_rx) = mpsc::channel::<IndexerCommand>(1);
    let handle = tokio::spawn(async move { idx.run(progress_tx, control_rx).await });

    loop {
        match progress_rx.recv().await {
            Ok(status) => match status.status {
                IndexStatus::Completed => break,
                IndexStatus::Failed(msg) => {
                    return Err(
                        format!("indexing failed: {msg}; errors={:?}", status.errors).into(),
                    )
                }
                IndexStatus::Cancelled => return Err("indexing cancelled".into()),
                _ => {}
            },
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(e) => return Err(format!("progress channel error: {e}").into()),
        }
    }
    handle.await??;

    // TODO:active-embedding-set 2025-12-15
    // update the active embedding set functions to correctly use Arc<RwLock<>> within these
    // functions.
    let active_embedding_set = db.with_active_set(|set| set.clone())?;

    // Pull one stored vector and assert it is 256 dims.
    let rel = active_embedding_set.rel_name.as_ref();
    let script = format!("?[vector] := *{rel}{{ vector @ 'NOW' }} :limit 1");
    let rows = db.raw_query(&script).map_err(PlokeError::from)?;
    let first = rows
        .rows
        .first()
        .and_then(|r| r.first())
        .ok_or("no embedding vectors returned from db")?;

    let observed_len = match first {
        DataValue::Vec(Vector::F32(v)) => v.len(),
        other => {
            return Err(format!("unexpected vector cell type from db: {other:?}").into());
        }
    };

    assert_eq!(observed_len, dims);

    Ok(())
}

/// Optional parity test (gated): compares local vs remote vectors via cosine similarity.
///
/// Enable with: `--features "live_api_tests parity_live_tests"`.
#[cfg(feature = "parity_live_tests")]
#[tokio::test]
async fn live_openrouter_vs_local_all_minilm_l6_v2_cosine_similarity(
) -> Result<(), Box<dyn std::error::Error>> {
    require_live_gate();

    let dims = 384usize;
    let local_model = "sentence-transformers/all-MiniLM-L6-v2";
    let remote_model = "sentence-transformers/all-minilm-l6-v2";

    // A small mixed set; keep stable to avoid accidental drift.
    let snippets: Vec<String> = vec![
        "fn parse(input: &str) -> Result<i32, Error> { input.parse()? }".into(),
        "pub enum Color { Red, Green, Blue }".into(),
        "impl Iterator for Foo { type Item = u8; fn next(&mut self) -> Option<u8> { None } }"
            .into(),
        "Rust uses ownership and borrowing to ensure memory safety.".into(),
        "This sentence is intentionally plain English.".into(),
        "The quick brown fox jumps over the lazy dog.".into(),
        "A short sentence about embeddings and cosine similarity.".into(),
        "struct Point { x: f32, y: f32 }".into(),
        "trait Display { fn fmt(&self) -> String; }".into(),
        "use std::collections::HashMap;".into(),
        "pub fn factorial(n: u64) -> u64 { (1..=n).product() }".into(),
        "Error handling in Rust often uses Result<T, E>.".into(),
        "Concurrent programming can be tricky without structured concurrency.".into(),
        "The database stores vectors in per-embedding-set relations.".into(),
        "This is the last parity snippet.".into(),
    ];

    // Local embeddings.
    let mut local_cfg = ploke_embed::local::EmbeddingConfig::default();
    local_cfg.model_id = local_model.to_string();
    let local = ploke_embed::local::LocalEmbedder::new(local_cfg)?;
    let slices: Vec<&str> = snippets.iter().map(|s| s.as_str()).collect();
    let local_vecs = local.embed_batch(&slices)?;
    assert_eq!(local_vecs.len(), snippets.len());
    assert!(local_vecs.iter().all(|v| v.len() == dims));

    // Remote embeddings.
    let remote_backend = OpenRouterBackend::new(&openrouter_cfg(remote_model, dims))?;
    let remote_proc = EmbeddingProcessor::new(EmbeddingSource::OpenRouter(remote_backend));
    let remote_vecs = remote_proc.generate_embeddings(snippets.clone()).await?;
    assert_eq!(remote_vecs.len(), snippets.len());
    assert!(remote_vecs.iter().all(|v| v.len() == dims));

    let mut sims: Vec<(String, f32)> = Vec::with_capacity(snippets.len());
    for ((text, a), b) in snippets
        .iter()
        .zip(local_vecs.iter())
        .zip(remote_vecs.iter())
    {
        sims.push((text.clone(), cosine_similarity(a, b)));
    }

    let mut only_sims = sims.iter().map(|(_, s)| *s).collect::<Vec<_>>();
    only_sims.sort_by(|a, b| a.total_cmp(b));
    let p50 = only_sims[only_sims.len() / 2];
    let min = *only_sims.first().unwrap_or(&0.0);

    // Suggested thresholds from the plan (may need tuning after first baseline).
    assert!(p50 >= 0.99, "p50 cosine below threshold: {p50}");
    assert!(min >= 0.97, "min cosine below threshold: {min}");

    let artifact = ParityCosineArtifact {
        local_model: local_model.to_string(),
        remote_model: remote_model.to_string(),
        dims,
        sims,
        p50,
        min,
    };

    let out_path = repo_target_test_output_dir()
        .join("openrouter_parity")
        .join("all_minilm_l6_v2")
        .join(format!("{}.json", ts_slug()));
    write_json(&out_path, &artifact);

    Ok(())
}
