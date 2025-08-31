#![allow(missing_docs)]
//! Core orchestration: `RagService` and configuration.
//!
//! [`RagService`] is the primary entry point for performing sparse, dense, and hybrid retrieval,
//! as well as assembling a token-budgeted context. It wraps the BM25 actor with timeouts, optional
//! retries/backoff (lenient mode), and exposes strict variants and persistence hooks.
//!
//! Configure behavior via [`RagConfig`], including fusion defaults, dense search parameters, and
//! context assembly policy. For diversity or learning-to-rank experiments, plug in a custom [`Reranker`].
mod unit_tests;
use super::*;
use ploke_core::rag_types::AssembledContext;
use ploke_io::IoManagerHandle;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub enum RetrievalStrategy {
    Dense,
    Sparse {
        strict: Option<bool>,
    },
    Hybrid {
        rrf: RrfConfig,
        mmr: Option<MmrConfig>,
    },
}
// AI: add manual impl of Default for `RetrievalStrategy::Hybrid` AI!

#[derive(Debug, Clone, Copy)]
pub struct SearchParams {
    pub ef: usize,
    pub radius: f64,
    pub max_hits: usize,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            ef: 10,
            radius: 10.0,
            max_hits: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RagConfig {
    pub bm25_timeout_ms: u64,
    pub bm25_retry_backoff_ms: Vec<u64>,
    pub strict_bm25_by_default: bool,
    pub rrf_default: RrfConfig,
    pub mmr_default: Option<MmrConfig>,
    pub score_norm: ScoreNorm,
    pub search_per_type: HashMap<NodeType, SearchParams>,
    pub assembly_policy: AssemblyPolicy,
    pub token_counter: Arc<dyn TokenCounter>,
    pub reranker: Option<Arc<dyn Reranker>>,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            bm25_timeout_ms: crate::BM25_TIMEOUT_MS,
            bm25_retry_backoff_ms: crate::BM25_RETRY_BACKOFF_MS.to_vec(),
            strict_bm25_by_default: false,
            rrf_default: RrfConfig::default(),
            mmr_default: None,
            score_norm: ScoreNorm::default(),
            search_per_type: HashMap::new(),
            assembly_policy: AssemblyPolicy::default(),
            token_counter: Arc::new(crate::context::ApproxCharTokenizer),
            reranker: None,
        }
    }
}

impl RagConfig {
    pub fn params_for(&self, ty: NodeType) -> SearchParams {
        self.search_per_type.get(&ty).copied().unwrap_or_default()
    }
}

/// Optional async reranker for candidate reordering using snippet texts.
#[allow(clippy::type_complexity)]
pub trait Reranker: Send + Sync + std::fmt::Debug {
    fn rerank<'a>(
        &'a self,
        query: &'a str,
        candidates: Vec<(Uuid, String)>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<(Uuid, f32)>, RagError>> + Send + 'a>>;
}

#[derive(Debug, Default)]
pub struct NoopReranker;

impl Reranker for NoopReranker {
    fn rerank<'a>(
        &'a self,
        _query: &'a str,
        candidates: Vec<(Uuid, String)>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<(Uuid, f32)>, RagError>> + Send + 'a>> {
        Box::pin(async move { Ok(candidates.into_iter().map(|(id, _)| (id, 1.0)).collect()) })
    }
}

/// RAG orchestration service.
///
/// This orchestrates hybrid search by combining:
/// - BM25 sparse search served by an in-memory actor
/// - Dense vector search served by the HNSW index in the database
///
/// Notes:
/// - BM25 search will gracefully fall back to dense search if the BM25 index is empty,
///   ensuring callers do not receive empty results due to indexing lag.
/// - Use `hybrid_search` to fuse the results via RRF for robust retrieval.
///
/// See crate tests for end-to-end examples using a fixture database.
#[derive(Debug)]
pub struct RagService {
    db: Arc<Database>,
    dense_embedder: Arc<EmbeddingProcessor>,
    bm_embedder: mpsc::Sender<Bm25Cmd>,
    cfg: RagConfig,
    io: Option<Arc<IoManagerHandle>>,
}

impl RagService {
    /// Construct a new RAG service, starting the BM25 service actor.
    pub fn new(
        db: Arc<Database>,
        dense_embedder: Arc<EmbeddingProcessor>,
    ) -> Result<Self, RagError> {
        // ensure_tracer_initialized();
        let bm_embedder = bm25_service::start_default(db.clone())?;
        Ok(Self {
            db,
            dense_embedder,
            bm_embedder,
            cfg: RagConfig::default(),
            io: None,
        })
    }

    /// Construct with explicit configuration (no IoManager).
    pub fn new_with_config(
        db: Arc<Database>,
        dense_embedder: Arc<EmbeddingProcessor>,
        cfg: RagConfig,
    ) -> Result<Self, RagError> {
        let bm_embedder = bm25_service::start_default(db.clone())?;
        Ok(Self {
            db,
            dense_embedder,
            bm_embedder,
            cfg,
            io: None,
        })
    }

    /// Construct with an IoManager and default configuration.
    pub fn new_with_io(
        db: Arc<Database>,
        dense_embedder: Arc<EmbeddingProcessor>,
        io: IoManagerHandle,
    ) -> Result<Self, RagError> {
        Self::new_full(db, dense_embedder, io, RagConfig::default())
    }

    /// Construct with both IoManager and explicit configuration.
    pub fn new_full(
        db: Arc<Database>,
        dense_embedder: Arc<EmbeddingProcessor>,
        io: IoManagerHandle,
        cfg: RagConfig,
    ) -> Result<Self, RagError> {
        let bm_embedder = bm25_service::start_default(db.clone())?;
        Ok(Self {
            db,
            dense_embedder,
            bm_embedder,
            cfg,
            io: Some(Arc::new(io)),
        })
    }

    /// Construct with both IoManager and rebuild avgld from db contents.
    pub fn new_rebuilt(
        db: Arc<Database>,
        dense_embedder: Arc<EmbeddingProcessor>,
        io: IoManagerHandle,
        cfg: RagConfig,
    ) -> Result<Self, RagError> {
        let bm_embedder = bm25_service::start_rebuilt(db.clone())?;
        Ok(Self {
            db,
            dense_embedder,
            bm_embedder,
            cfg,
            io: Some(Arc::new(io)),
        })
    }

    /// Convenience constructor for tests with an in-memory database and mock embedder.
    pub fn new_mock() -> Self {
        let db = Arc::new(ploke_db::Database::init_with_schema().expect("init test db"));
        let dense_embedder = Arc::new(ploke_embed::indexer::EmbeddingProcessor::new_mock());
        let bm_embedder = bm25_service::start_default(db.clone()).expect("start bm25");
        Self {
            db,
            dense_embedder,
            bm_embedder,
            cfg: RagConfig::default(),
            io: None,
        }
    }

    /// Execute a BM25 search against the in-memory sparse index.
    ///
    /// Lenient mode: if the BM25 index is not ready or empty, this method may fall back to dense search.
    /// Returns a Vec of (document_id, score) pairs sorted by relevance.
    #[instrument(skip(self, query), fields(query_len = %query.len(), top_k = top_k, strict = false, timeout_ms = BM25_TIMEOUT_MS))]
    pub async fn search_bm25(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<(Uuid, f32)>, RagError> {
        let mut attempts: usize = 0;

        loop {
            attempts += 1;

            let status_opt = match self.bm25_status().await {
                Ok(s) => Some(s),
                Err(e) => {
                    debug!("bm25_status check failed: {:?}", e);
                    None
                }
            };

            let (tx, rx) = oneshot::channel();
            self.bm_embedder
                .send(Bm25Cmd::Search {
                    query: query.to_string(),
                    top_k,
                    resp: tx,
                })
                .await
                .map_err(|e| {
                    RagError::Channel(format!(
                        "failed to send BM25 search command (len={}, top_k={}): {}",
                        query.len(),
                        top_k,
                        e
                    ))
                })?;

            let res = match timeout(Duration::from_millis(self.cfg.bm25_timeout_ms), rx).await {
                Ok(Ok(r)) => r,
                Ok(Err(recv_err)) => {
                    return Err(RagError::Channel(format!(
                        "BM25 search response channel closed (len={}, top_k={}): {}",
                        query.len(),
                        top_k,
                        recv_err
                    )))
                }
                Err(_) => {
                    return Err(RagError::Channel(format!(
                        "timeout waiting for BM25 search ({} ms)",
                        self.cfg.bm25_timeout_ms
                    )))
                }
            };

            if !res.is_empty() {
                debug!(
                    bm25_results = res.len(),
                    attempts = attempts,
                    "BM25 search succeeded"
                );
                return Ok(res);
            }

            // Decide retry/fallback based on status.
            let mut fallback_used = false;
            let should_retry = matches!(
                status_opt,
                Some(Bm25Status::Uninitialized) | Some(Bm25Status::Building)
            ) && attempts <= self.cfg.bm25_retry_backoff_ms.len();

            if should_retry {
                let backoff = self.cfg.bm25_retry_backoff_ms[attempts - 1];
                debug!(attempts = attempts, bm25_results = 0, bm25_status = ?status_opt, backoff_ms = backoff, "BM25 empty; retrying after backoff");
                sleep(Duration::from_millis(backoff)).await;
                continue;
            }

            let use_fallback = !matches!(status_opt, Some(Bm25Status::Ready { docs }) if docs > 0);

            if use_fallback {
                fallback_used = true;
                debug!(bm25_results = 0, bm25_status = ?status_opt, attempts = attempts, fallback_used = fallback_used, "BM25 not ready/empty; falling back to dense search");
                let dense_list = self.search(query, top_k).await.map_err(|e| {
                    RagError::Embed(format!(
                        "dense search failed during BM25 fallback (len={}, top_k={}): {:?}",
                        query.len(),
                        top_k,
                        e
                    ))
                })?;
                debug!(dense_results = dense_list.len(), "Dense fallback complete");
                return Ok(dense_list);
            } else {
                debug!(bm25_results = 0, bm25_status = ?status_opt, attempts = attempts, fallback_used = fallback_used, "BM25 ready but returned 0 results; not falling back");
                return Ok(res);
            }
        }
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

    /// Query BM25 actor for current status with a client-side timeout.
    #[instrument(skip(self), fields(timeout_ms = BM25_TIMEOUT_MS))]
    pub async fn bm25_status(&self) -> Result<Bm25Status, RagError> {
        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Status { resp: tx })
            .await
            .map_err(|e| RagError::Channel(format!("failed to send BM25 status command: {}", e)))?;
        match timeout(Duration::from_millis(self.cfg.bm25_timeout_ms), rx).await {
            Ok(Ok(Ok(status))) => Ok(status),
            Ok(Ok(Err(db_err))) => Err(RagError::Db(db_err)),
            Ok(Err(recv_err)) => Err(RagError::Channel(format!(
                "BM25 status response channel closed: {}",
                recv_err
            ))),
            Err(_) => Err(RagError::Channel(format!(
                "timeout waiting for BM25 status ({} ms)",
                self.cfg.bm25_timeout_ms
            ))),
        }
    }

    /// Save BM25 sidecar state to path via actor with timeout.
    #[instrument(skip(self, path), fields(timeout_ms = BM25_TIMEOUT_MS))]
    pub async fn bm25_save<P: AsRef<std::path::Path> + Send>(
        &self,
        path: P,
    ) -> Result<(), RagError> {
        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Save {
                path: path.as_ref().to_path_buf(),
                resp: tx,
            })
            .await
            .map_err(|e| RagError::Channel(format!("failed to send BM25 save command: {}", e)))?;
        match timeout(Duration::from_millis(self.cfg.bm25_timeout_ms), rx).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(db_err))) => Err(RagError::Db(db_err)),
            Ok(Err(recv_err)) => Err(RagError::Channel(format!(
                "BM25 save response channel closed: {}",
                recv_err
            ))),
            Err(_) => Err(RagError::Channel(format!(
                "timeout waiting for BM25 save ({} ms)",
                self.cfg.bm25_timeout_ms
            ))),
        }
    }

    /// Load BM25 state from path via actor with timeout.
    #[instrument(skip(self, path), fields(timeout_ms = BM25_TIMEOUT_MS))]
    pub async fn bm25_load<P: AsRef<std::path::Path> + Send>(
        &self,
        path: P,
    ) -> Result<(), RagError> {
        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Load {
                path: path.as_ref().to_path_buf(),
                resp: tx,
            })
            .await
            .map_err(|e| RagError::Channel(format!("failed to send BM25 load command: {}", e)))?;
        match timeout(Duration::from_millis(self.cfg.bm25_timeout_ms), rx).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(db_err))) => Err(RagError::Db(db_err)),
            Ok(Err(recv_err)) => Err(RagError::Channel(format!(
                "BM25 load response channel closed: {}",
                recv_err
            ))),
            Err(_) => Err(RagError::Channel(format!(
                "timeout waiting for BM25 load ({} ms)",
                self.cfg.bm25_timeout_ms
            ))),
        }
    }

    /// Execute a BM25 search in strict mode: no dense fallback.
    /// Returns error if the index is Uninitialized/Building/Empty when results are empty.
    #[instrument(skip(self, query), fields(query_len = %query.len(), top_k = top_k, strict = true, timeout_ms = BM25_TIMEOUT_MS))]
    pub async fn search_bm25_strict(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<(Uuid, f32)>, RagError> {
        let status = self.bm25_status().await?;

        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Search {
                query: query.to_string(),
                top_k,
                resp: tx,
            })
            .await
            .map_err(|e| {
                RagError::Channel(format!(
                    "failed to send BM25 search command (len={}, top_k={}): {}",
                    query.len(),
                    top_k,
                    e
                ))
            })?;

        let res = match timeout(Duration::from_millis(self.cfg.bm25_timeout_ms), rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(recv_err)) => {
                return Err(RagError::Channel(format!(
                    "BM25 search response channel closed (len={}, top_k={}): {}",
                    query.len(),
                    top_k,
                    recv_err
                )))
            }
            Err(_) => {
                return Err(RagError::Channel(format!(
                    "timeout waiting for BM25 search ({} ms)",
                    self.cfg.bm25_timeout_ms
                )))
            }
        };

        if !res.is_empty() {
            debug!(bm25_results = res.len(), "BM25 strict search succeeded");
            return Ok(res);
        }

        match status {
            Bm25Status::Uninitialized | Bm25Status::Building => {
                Err(RagError::Search("bm25 index not ready".to_string()))
            }
            Bm25Status::Empty | Bm25Status::Ready { docs: 0 } => {
                Err(RagError::Search("bm25 index empty".to_string()))
            }
            Bm25Status::Error(msg) => Err(RagError::Search(format!("bm25 error state: {}", msg))),
            Bm25Status::Ready { docs: _ } => {
                debug!("BM25 strict: Ready but query returned 0 results");
                Ok(res)
            }
        }
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

        // Collect results from all node types; pre-allocate to avoid reallocations
        let mut all_results: Vec<(Uuid, f32)> =
            Vec::with_capacity(NodeType::primary_nodes().len() * top_k);

        for node_type in NodeType::primary_nodes() {
            let params = self.cfg.params_for(node_type);
            let max_hits = if params.max_hits == 0 {
                top_k
            } else {
                params.max_hits
            };
            let args = SimilarArgs {
                db: &self.db,
                vector_query: &query_embedding,
                k: top_k,
                ef: params.ef, // Configurable ef value
                ty: node_type,
                max_hits,
                radius: params.radius, // Configurable radius value
            };

            let result = search_similar_args(args)?;

            // Convert distance to similarity score (lower distance = higher similarity)
            all_results.extend(
                result
                    .typed_data
                    .v
                    .into_iter()
                    .zip(result.dist.into_iter())
                    .map(|(embed_data, distance)| (embed_data.id, 1.0 - distance as f32)),
            );
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

    /// High-level API: retrieve and assemble a context using the chosen strategy and budget.
    /// Uses configured defaults (policy, tokenizer, strict bm25) unless overridden by the strategy.
    #[instrument(skip(self, query, budget, strategy), fields(query_len = %query.len(), top_k = top_k))]
    pub async fn get_context(
        &self,
        query: &str,
        top_k: usize,
        budget: &TokenBudget,
        strategy: &RetrievalStrategy,
    ) -> Result<AssembledContext, RagError> {
        // 1) Retrieve hits according to strategy
        let hits: Vec<(Uuid, f32)> = match strategy {
            RetrievalStrategy::Dense => self
                .search(query, top_k)
                .await
                .map_err(|e| RagError::Embed(format!("dense search failed: {:?}", e)))?,
            RetrievalStrategy::Sparse { strict } => {
                let use_strict = strict.unwrap_or(self.cfg.strict_bm25_by_default);
                if use_strict {
                    self.search_bm25_strict(query, top_k).await?
                } else {
                    self.search_bm25(query, top_k).await?
                }
            }
            RetrievalStrategy::Hybrid { rrf, mmr } => {
                let bm25_fut = self.search_bm25(query, top_k);
                let dense_fut = self.search(query, top_k);
                let (bm25_res, dense_res) = tokio::join!(bm25_fut, dense_fut);
                let bm25_list = bm25_res?;
                let dense_list = dense_res
                    .map_err(|e| RagError::Embed(format!("dense search failed: {:?}", e)))?;
                let mut fused = rrf_fuse(&bm25_list, &dense_list, rrf);
                fused.truncate(top_k);
                if let Some(mcfg) = mmr {
                    // Optional diversity re-ranking based on embeddings; default to no-embeddings map.
                    let embed_map: HashMap<Uuid, Vec<f32>> = HashMap::new();

                    mmr_select(&fused, top_k, &embed_map, mcfg)
                } else {
                    fused
                }
            }
        };

        // Optional reranker: requires IoManager to fetch texts
        let final_hits: Vec<(Uuid, f32)> = if let Some(rr) = &self.cfg.reranker {
            let io = self
                .io
                .as_ref()
                .ok_or_else(|| {
                    RagError::Search("IoManagerHandle not configured for reranking".to_string())
                })?
                .clone();

            // Fetch texts for candidates
            let ids: Vec<Uuid> = hits.iter().map(|(id, _)| *id).collect();
            let nodes = self
                .db
                .get_nodes_ordered(ids.clone())
                .map_err(|e| RagError::Embed(e.to_string()))?;
            let texts = io.get_snippets_batch(nodes).await.map_err(|e| {
                RagError::Search(format!("get_snippets_batch failed for rerank: {:?}", e))
            })?;
            let mut cand: Vec<(Uuid, String)> = Vec::new();
            for (i, res) in texts.into_iter().enumerate() {
                if let Some(id) = ids.get(i) {
                    match res {
                        Ok(s) => cand.push((*id, s)),
                        Err(e) => {
                            if self.cfg.assembly_policy.strict_io {
                                return Err(RagError::Search(format!(
                                    "IO error during rerank snippet fetch: {:?}",
                                    e
                                )));
                            }
                        }
                    }
                }
            }
            rr.rerank(query, cand).await?
        } else {
            hits
        };

        // 2) Assemble context
        let io = self
            .io
            .as_ref()
            .ok_or_else(|| RagError::Search("IoManagerHandle not configured".to_string()))?
            .clone();

        assemble_context(
            query,
            &final_hits,
            budget,
            &self.cfg.assembly_policy,
            &*self.cfg.token_counter,
            &self.db,
            &io,
        )
        .await
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

        // Fuse with configurable, weighted RRF and stable UUID tie-breaking.
        let mut out: Vec<(Uuid, f32)> = rrf_fuse(&bm25_list, &dense_list, &RrfConfig::default());
        out.truncate(top_k);

        debug!("Hybrid search returning {} fused results", out.len());
        Ok(out)
    }
}
