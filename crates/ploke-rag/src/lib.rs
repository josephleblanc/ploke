#![allow(unused_variables, unused_imports, dead_code)]
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(not(test))]
static TRACER_INIT: std::sync::Once = std::sync::Once::new();

#[cfg(test)]
static _TRACE_MARKER: () = ();

#[cfg(not(test))]
fn ensure_tracer_initialized() {
    TRACER_INIT.call_once(|| {
        let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);
        let _ = tracing_subscriber::registry().with(fmt_layer).try_init();
    });
}

#[cfg(test)]
fn ensure_tracer_initialized() {
    // Tests configure tracing themselves; no-op to avoid interfering with test harness.
}

use ploke_core::EmbeddingData;
use ploke_db::{
    bm25_index::bm25_service::{self, Bm25Cmd},
    search_similar_args, Database, DbError, NodeType, SimilarArgs, TypedEmbedData,
};
use ploke_embed::indexer::{EmbeddingProcessor, IndexerTask};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;
use tracing::{debug, instrument};

#[derive(Error, Debug)]
pub enum RagError {
    #[error("Database error: {0}")]
    Db(#[from] DbError),

    #[error("Channel error: {0}")]
    Channel(String),

    #[error("Embedding error: {0}")]
    Embed(String),
}

impl From<RagError> for ploke_error::Error {
    fn from(value: RagError) -> ploke_error::Error {
        match value {
            RagError::Db(db_err) => {
                ploke_error::Error::Internal(ploke_error::internal::InternalError::CompilerError(
                    format!("DB error: {}", db_err),
                ))
            }
            RagError::Channel(msg) => ploke_error::Error::Internal(
                ploke_error::internal::InternalError::InvalidState("Channel communication error"),
            ),
            RagError::Embed(msg) => {
                ploke_error::Error::Internal(ploke_error::internal::InternalError::NotImplemented(
                    format!("Embedding error: {}", msg),
                ))
            }
        }
    }
}

/// RAG orchestration service.
/// Holds handles to the database, dense embedder, and BM25 service actor.
#[derive(Debug)]
pub struct RagService {
    db: Arc<Database>,
    dense_embedder: Arc<EmbeddingProcessor>,
    bm_embedder: mpsc::Sender<Bm25Cmd>,
}

impl RagService {
    /// Construct a new RAG service, starting the BM25 service actor.
    pub fn new(
        db: Arc<Database>,
        dense_embedder: Arc<EmbeddingProcessor>,
    ) -> Result<Self, RagError> {
        ensure_tracer_initialized();
        let bm_embedder = bm25_service::start_default(db.clone())?;
        Ok(Self {
            db,
            dense_embedder,
            bm_embedder,
        })
    }

    /// Execute a BM25 search against the in-memory sparse index.
    /// Returns a Vec of (document_id, score) pairs sorted by relevance.
    #[instrument(skip(self, query), fields(query_len = %query.len(), top_k = top_k))]
    pub async fn search_bm25(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<(Uuid, f32)>, RagError> {
        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Search {
                query: query.to_string(),
                top_k,
                resp: tx,
            })
            .await
            .map_err(|e| RagError::Channel(format!("failed to send BM25 search command: {}", e)))?;

        let res = rx
            .await
            .map_err(|e| RagError::Channel(format!("BM25 search response channel closed: {}", e)))?;
        debug!("BM25 search returned {} results", res.len());
        Ok(res)
    }

    /// Trigger a rebuild of the BM25 sparse index.
    /// For now, this is a fire-and-forget command to the BM25 service.
    #[instrument(skip(self))]
    pub async fn bm25_rebuild(&self) -> Result<(), RagError> {
        self.bm_embedder.send(Bm25Cmd::Rebuild).await.map_err(|e| {
            RagError::Channel(format!("failed to send BM25 rebuild command: {}", e))
        })?;
        debug!("BM25 rebuild command sent");
        Ok(())
    }

    /// Perform a dense search using the HNSW index in the database.
    /// Returns a Vec of (snippet_id, score) pairs sorted by relevance.
    #[instrument(skip(self, query), fields(query_len = %query.len(), top_k = top_k))]
    pub async fn search(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<(Uuid, f32)>, ploke_error::Error> {
        // Generate embedding for the query
        let embeddings = self
            .dense_embedder
            .generate_embeddings(vec![query.to_string()])
            .await?;

        let query_embedding = embeddings
            .into_iter()
            .next()
            .ok_or_else(|| RagError::Embed("failed to generate query embedding".to_string()))?;

        // Collect results from all node types
        let mut all_results: Vec<(Uuid, f32)> = Vec::new();

        for node_type in NodeType::primary_nodes() {
            let args = SimilarArgs {
                db: &self.db,
                vector_query: &query_embedding,
                k: top_k,
                ef: 10, // Default ef value
                ty: node_type,
                max_hits: top_k,
                radius: 10.0, // Default radius value
            };

            let result = search_similar_args(args)?;

            // Convert distance to similarity score (lower distance = higher similarity)
            let typed_results: Vec<(Uuid, f32)> = result
                .typed_data
                .v
                .into_iter()
                .zip(result.dist.into_iter())
                .map(|(embed_data, distance)| (embed_data.id, 1.0 - distance as f32))
                .collect();

            all_results.extend(typed_results);
        }

        debug!(
            "Dense search collected {} results across {} node types",
            all_results.len(),
            NodeType::primary_nodes().len()
        );

        // Sort by score (highest first) and take top_k
        all_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        all_results.truncate(top_k);

        debug!("Dense search returning {} results", all_results.len());
        Ok(all_results)
    }

    /// Perform a hybrid search (BM25 + dense).
    ///
    /// Strict mode: if the dense search fails, propagate an error (do not silently fall back).
    /// Runs BM25 and dense searches concurrently, fuses results using Reciprocal Rank Fusion (RRF),
    /// and returns the top_k results ordered by fused score (higher = better).
    #[instrument(skip(self, query), fields(query_len = %query.len(), top_k = top_k))]
    pub async fn hybrid_search(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<(Uuid, f32)>, RagError> {
        // Kick off both searches concurrently.
        let bm25_fut = self.search_bm25(query, top_k);
        let dense_fut = self.search(query, top_k);

        let (bm25_res, dense_res) = tokio::join!(bm25_fut, dense_fut);

        // Propagate BM25 errors as-is.
        let bm25_list = bm25_res?;

        // Dense errors are mapped to RagError::Embed since we're in strict mode.
        let dense_list =
            dense_res.map_err(|e| RagError::Embed(format!("dense search failed: {:?}", e)))?;

        // RRF fusion parameters
        let rrf_k: f32 = 60.0;

        // Accumulate fused scores using ranks (1-based)
        let mut fused: HashMap<Uuid, f32> = HashMap::new();

        for (i, (id, _score)) in bm25_list.iter().enumerate() {
            let rank = (i + 1) as f32;
            let add = 1.0_f32 / (rrf_k + rank);
            *fused.entry(*id).or_insert(0.0) += add;
        }

        for (i, (id, _score)) in dense_list.iter().enumerate() {
            let rank = (i + 1) as f32;
            let add = 1.0_f32 / (rrf_k + rank);
            *fused.entry(*id).or_insert(0.0) += add;
        }

        // Collect, sort by fused score descending, and take top_k
        let mut out: Vec<(Uuid, f32)> = fused.into_iter().collect();
        out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(top_k);

        debug!("Hybrid search returning {} fused results", out.len());
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use itertools::Itertools;
    use lazy_static::lazy_static;
    use ploke_core::EmbeddingData;
    use ploke_db::{create_index_primary, Database};
    use ploke_embed::{
        indexer::{EmbeddingProcessor, EmbeddingSource},
        local::{EmbeddingConfig, LocalEmbedder},
    };
    use ploke_error::Error;
    use ploke_io::IoManagerHandle;
    use ploke_test_utils::workspace_root;
    use tokio::time::{sleep, Duration};
    use tracing::Level;
    use uuid::Uuid;

    use crate::RagService;
    use std::sync::Once;
    static TEST_TRACING: Once = Once::new();
    fn init_tracing_once() {
        TEST_TRACING.call_once(|| {
            ploke_test_utils::init_test_tracing(tracing::Level::INFO);
        });
    }

    lazy_static! {
        /// This test db is restored from the backup of an earlier parse of the `fixture_nodes`
        /// crate located in `tests/fixture_crates/fixture_nodes`, and has a decent sampling of all
        /// rust code items. It provides a good target for other tests because it has already been
        /// extensively tested in `syn_parser`, with each item individually verified to have all
        /// fields correctly parsed for expected values.
        ///
        /// One "gotcha" of laoding the Cozo database is that the hnsw items are not retained
        /// between backups, so they must be recalculated each time. However, by restoring the
        /// backup database we do retain the dense vector embeddings, allowing our tests to be
        /// significantly sped up by using a lazy loader here and making calls to the same backup.
        ///
        /// If needed, other tests can re-implement the load from this file, which may become a
        /// factor for some tests that need to alter the database, but as long as things are
        /// cleaned up afterwards it should be OK.
        // TODO: Add a mutex guard to avoid cross-contamination of tests.
        pub static ref TEST_DB_NODES: Result<Arc< Database >, Error> = {
            let db = Database::init_with_schema()?;

            let mut target_file = workspace_root();
            target_file.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
            let prior_rels_vec = db.relations_vec()?;
            db.import_from_backup(&target_file, &prior_rels_vec)
                .map_err(ploke_db::DbError::from)
                .map_err(ploke_error::Error::from)?;
            create_index_primary(&db)?;
            Ok(Arc::new( db ))
        };
    }

    async fn fetch_and_assert_snippet(
        db: &Arc<Database>,
        ordered_node_ids: Vec<Uuid>,
        search_term: &str,
    ) -> Result<(), Error> {
        let node_info: Vec<EmbeddingData> = db.get_nodes_ordered(ordered_node_ids)?;
        let io_handle = IoManagerHandle::new();

        let snippet_results: Vec<Result<String, Error>> =
            io_handle.get_snippets_batch(node_info).await.expect("Problem receiving");

        let mut snippets: Vec<String> = Vec::new();
        for snip in snippet_results.into_iter() {
            snippets.push(snip?);
        }

        let snippet_match = snippets.iter().find(|s| s.contains(search_term));
        assert!(
            snippet_match.is_some(),
            "No snippet found containing '{}'",
            search_term
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_search() -> Result<(), Error> {
        // Initialize tracing for the test
        init_tracing_once();
        let db = TEST_DB_NODES.as_ref().expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "use_all_const_static";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 15).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_bm25_rebuild() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES.as_ref().expect("Must set up TEST_DB_NODES correctly.");

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        // Should not error
        rag.bm25_rebuild().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_bm25_search_basic() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES.as_ref().expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "use_all_const_static";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        // Trigger a rebuild to ensure index is fresh, then retry a few times in case it's async.
        rag.bm25_rebuild().await?;

        let mut bm25_res: Vec<(Uuid, f32)> = Vec::new();
        for _ in 0..10 {
            bm25_res = rag.search_bm25(search_term, 15).await?;
            if !bm25_res.is_empty() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert!(
            !bm25_res.is_empty(),
            "BM25 search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = bm25_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_hybrid_search() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES.as_ref().expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "use_all_const_static";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        let fused: Vec<(Uuid, f32)> = rag.hybrid_search(search_term, 15).await?;
        assert!(
            !fused.is_empty(),
            "Hybrid search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = fused.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }
}
