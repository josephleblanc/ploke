#![cfg(feature = "live_api_tests")]

// See report on these tests and their usage in:
// - crates/ploke-tui/docs/reports/remote_embedding_openrouter_fixture_nodes_e2e_test_20251216.md

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ploke_core::embeddings::{
    EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::{multi_embedding::db_ext::EmbeddingExt as _, Database};
use ploke_db::{multi_embedding::hnsw_ext::HnswExt as _, DbError};
use ploke_embed::{
    cancel_token::CancellationToken,
    config::{OpenRouterConfig, TruncatePolicy},
    indexer::{EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexerCommand, IndexerTask},
    providers::openrouter::OpenRouterBackend,
    runtime::EmbeddingRuntime,
};
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

fn write_json(path: &Path, value: &impl Serialize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create artifact dir");
    }
    let bytes = serde_json::to_vec_pretty(value).expect("failed to serialize artifact json");
    fs::write(path, bytes).expect("failed to write artifact json");
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

fn env_model_and_dims() -> (String, u32) {
    let model = std::env::var("PLOKE_OPENROUTER_EMBED_MODEL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "openai/text-embedding-3-small".to_string());
    let dims = std::env::var("PLOKE_OPENROUTER_EMBED_DIMS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(256);
    (model, dims)
}

#[derive(Debug, Serialize)]
struct FixtureNodesE2eArtifact {
    fixture: String,
    provider: String,
    model: String,
    dims: u32,
    total_unembedded_before: usize,
    seeded_local_vectors: usize,
    live_vectors_expected: usize,
    total_unembedded_after_seed: usize,
    total_vectors_after_run: usize,
    progress_messages: usize,
    last_status: String,
    errors: Vec<String>,
}

/// End-to-end regression: parse + transform `tests/fixture_crates/fixture_nodes`,
/// then run remote embeddings against a small live batch and assert vectors are written + HNSW builds.
///
/// Enable explicitly with:
/// `cargo test -p ploke-embed --test openrouter_live_fixture_nodes_e2e -- --ignored --nocapture --test-threads=1`
#[tokio::test]
#[ignore = "hits live OpenRouter embeddings; requires OPENROUTER_API_KEY"]
async fn live_openrouter_fixture_nodes_index_e2e() -> Result<(), Box<dyn std::error::Error>> {
    require_live_gate();

    let (model, dims) = env_model_and_dims();
    let fixture = "fixture_nodes";
    let live_batch_target: usize = 8;

    let cozo_db = ploke_test_utils::setup_db_full_multi_embedding(fixture)?;
    let db = Database::new(cozo_db);

    // Make the active embedding set the live OpenRouter model.
    let new_set = EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("openrouter"),
        EmbeddingModelId::new_from_str(&model),
        EmbeddingShape::new_dims_default(dims),
    );
    db.set_active_set(new_set)?;

    // Ensure embedding-set + vector relations exist before any queries.
    db.ensure_embedding_set_relation()?;
    let active_embedding_set = db.with_active_set(|set| set.clone())?;
    db.put_embedding_set(&active_embedding_set)?;
    db.ensure_vector_embedding_relation(&active_embedding_set)?;

    let total_unembedded_before = db.count_unembedded_nonfiles()?;
    if total_unembedded_before == 0 {
        return Err::<(), Box<dyn std::error::Error>>(
            "fixture_nodes contains no unembedded nodes; cannot run e2e embedding test".into(),
        )
        .map(|_| ());
    }

    // Seed dummy vectors for most nodes so we only hit the live endpoint for a small batch.
    let unembedded = db.get_unembedded_node_data(total_unembedded_before, 0)?;
    let mut ids = Vec::with_capacity(total_unembedded_before);
    let mut seen = std::collections::BTreeSet::new();
    for group in unembedded {
        for item in group.v {
            if seen.insert(item.id) {
                ids.push(item.id);
            }
        }
    }
    if ids.len() != total_unembedded_before {
        return Err::<(), Box<dyn std::error::Error>>(
            format!(
                "expected {total_unembedded_before} unique ids, got {}",
                ids.len()
            )
            .into(),
        )
        .map(|_| ());
    }

    let live_vectors_expected = live_batch_target.min(ids.len());
    let seeded_local_vectors = ids.len().saturating_sub(live_vectors_expected);
    if seeded_local_vectors > 0 {
        let dummy_vec = vec![0.0f32; dims as usize];
        let seed_ids = &ids[..seeded_local_vectors];
        for chunk in seed_ids.chunks(256) {
            let updates = chunk
                .iter()
                .copied()
                .map(|id| (id, dummy_vec.clone()))
                .collect::<Vec<_>>();
            db.update_embeddings_batch(updates)?;
        }
    }
    let total_unembedded_after_seed = db.count_unembedded_nonfiles()?;

    // Now run the indexer which should only embed the small remaining set.
    let backend = OpenRouterBackend::new(&openrouter_cfg(&model, dims))?;
    let embedding_runtime = std::sync::Arc::new(EmbeddingRuntime::from_shared_set(
        std::sync::Arc::clone(&db.active_embedding_set),
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
        live_batch_target,
    );

    let (progress_tx, mut progress_rx) = broadcast::channel(64);
    let progress_tx = std::sync::Arc::new(progress_tx);
    let (_control_tx, control_rx) = mpsc::channel::<IndexerCommand>(1);

    let handle = tokio::spawn(async move { idx.run(progress_tx, control_rx).await });

    let mut progress_messages = 0usize;
    let mut last_status = None::<ploke_embed::indexer::IndexingStatus>;

    let wait = async {
        loop {
            match progress_rx.recv().await {
                Ok(status) => {
                    progress_messages += 1;
                    match status.status {
                        IndexStatus::Completed => {
                            last_status = Some(status);
                            break Ok::<(), Box<dyn std::error::Error>>(());
                        }
                        IndexStatus::Failed(ref msg) => {
                            last_status = Some(status.clone());
                            break Err::<(), Box<dyn std::error::Error>>(
                                format!("indexing failed: {msg}; errors={:?}", status.errors)
                                    .into(),
                            );
                        }
                        IndexStatus::Cancelled => {
                            last_status = Some(status.clone());
                            break Err::<(), Box<dyn std::error::Error>>(
                                "indexing cancelled".into(),
                            );
                        }
                        _ => {
                            last_status = Some(status);
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(e) => {
                    break Err::<(), Box<dyn std::error::Error>>(
                        format!("progress channel error: {e}").into(),
                    )
                }
            }
        }
    };

    tokio::time::timeout(Duration::from_secs(30 * 60), wait)
        .await
        .map_err(|_| "indexing timed out (30 minutes)")??;

    tokio::time::timeout(Duration::from_secs(30 * 60), handle)
        .await
        .map_err(|_| "indexing task join timed out (30 minutes)")???;

    // Validate DB writes + HNSW.
    let active = db.with_active_set(|set| set.clone())?;
    let total_vectors_after_run = db.count_embeddings_for_set(&active)?;
    db.create_embedding_index(&active).map_err(DbError::from)?;
    let _hnsw_registered = db.is_hnsw_index_registered(&active)?;

    let artifact = FixtureNodesE2eArtifact {
        fixture: fixture.to_string(),
        provider: "openrouter".to_string(),
        model: model.clone(),
        dims,
        total_unembedded_before,
        seeded_local_vectors,
        live_vectors_expected,
        total_unembedded_after_seed,
        total_vectors_after_run,
        progress_messages,
        last_status: last_status
            .as_ref()
            .map(|s| format!("{:?}", s.status))
            .unwrap_or_else(|| "none".to_string()),
        errors: last_status
            .as_ref()
            .map(|s| s.errors.clone())
            .unwrap_or_default(),
    };

    let out = repo_target_test_output_dir()
        .join("embedding")
        .join("live")
        .join(format!("openrouter_fixture_nodes_e2e_{}.json", ts_slug()));
    write_json(&out, &artifact);

    // Assertions after artifacts are persisted (so failures have evidence).
    if total_unembedded_after_seed != live_vectors_expected {
        return Err::<(), Box<dyn std::error::Error>>(format!(
            "expected {live_vectors_expected} unembedded after seeding, got {total_unembedded_after_seed} (artifact={})",
            out.display()
        )
        .into())
        .map(|_| ());
    }
    if total_vectors_after_run != total_unembedded_before {
        return Err::<(), Box<dyn std::error::Error>>(format!(
            "expected {total_unembedded_before} vectors after run, got {total_vectors_after_run} (artifact={})",
            out.display()
        )
        .into())
        .map(|_| ());
    }
    if progress_messages < 2 {
        return Err::<(), Box<dyn std::error::Error>>(
            format!(
                "expected multiple progress messages; got {progress_messages} (artifact={})",
                out.display()
            )
            .into(),
        )
        .map(|_| ());
    }

    Ok(())
}
