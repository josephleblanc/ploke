mod unit_tests;
use super::*;

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

            let res = match timeout(Duration::from_millis(BM25_TIMEOUT_MS), rx).await {
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
                        BM25_TIMEOUT_MS
                    )))
                }
            };

            if !res.is_empty() {
                debug!(bm25_results = res.len(), attempts = attempts, "BM25 search succeeded");
                return Ok(res);
            }

            // Decide retry/fallback based on status.
            let mut fallback_used = false;
            let should_retry = matches!(status_opt, Some(Bm25Status::Uninitialized) | Some(Bm25Status::Building))
                && attempts <= BM25_RETRY_BACKOFF_MS.len();

            if should_retry {
                let backoff = BM25_RETRY_BACKOFF_MS[attempts - 1];
                debug!(attempts = attempts, bm25_results = 0, bm25_status = ?status_opt, backoff_ms = backoff, "BM25 empty; retrying after backoff");
                sleep(Duration::from_millis(backoff)).await;
                continue;
            }

            let use_fallback = match status_opt {
                Some(Bm25Status::Ready { docs }) if docs > 0 => false,
                _ => true,
            };

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
        match timeout(Duration::from_millis(BM25_TIMEOUT_MS), rx).await {
            Ok(Ok(Ok(status))) => Ok(status),
            Ok(Ok(Err(db_err))) => Err(RagError::Db(db_err)),
            Ok(Err(recv_err)) => Err(RagError::Channel(format!("BM25 status response channel closed: {}", recv_err))),
            Err(_) => Err(RagError::Channel(format!("timeout waiting for BM25 status ({} ms)", BM25_TIMEOUT_MS))),
        }
    }

    /// Save BM25 sidecar state to path via actor with timeout.
    #[instrument(skip(self, path), fields(timeout_ms = BM25_TIMEOUT_MS))]
    pub async fn bm25_save<P: AsRef<std::path::Path> + Send>(&self, path: P) -> Result<(), RagError> {
        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Save { path: path.as_ref().to_path_buf(), resp: tx })
            .await
            .map_err(|e| RagError::Channel(format!("failed to send BM25 save command: {}", e)))?;
        match timeout(Duration::from_millis(BM25_TIMEOUT_MS), rx).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(db_err))) => Err(RagError::Db(db_err)),
            Ok(Err(recv_err)) => Err(RagError::Channel(format!("BM25 save response channel closed: {}", recv_err))),
            Err(_) => Err(RagError::Channel(format!("timeout waiting for BM25 save ({} ms)", BM25_TIMEOUT_MS))),
        }
    }

    /// Load BM25 state from path via actor with timeout.
    #[instrument(skip(self, path), fields(timeout_ms = BM25_TIMEOUT_MS))]
    pub async fn bm25_load<P: AsRef<std::path::Path> + Send>(&self, path: P) -> Result<(), RagError> {
        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Load { path: path.as_ref().to_path_buf(), resp: tx })
            .await
            .map_err(|e| RagError::Channel(format!("failed to send BM25 load command: {}", e)))?;
        match timeout(Duration::from_millis(BM25_TIMEOUT_MS), rx).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(db_err))) => Err(RagError::Db(db_err)),
            Ok(Err(recv_err)) => Err(RagError::Channel(format!("BM25 load response channel closed: {}", recv_err))),
            Err(_) => Err(RagError::Channel(format!("timeout waiting for BM25 load ({} ms)", BM25_TIMEOUT_MS))),
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

        let res = match timeout(Duration::from_millis(BM25_TIMEOUT_MS), rx).await {
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
                    BM25_TIMEOUT_MS
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
        let mut out: Vec<(Uuid, f32)> =
            rrf_fuse(&bm25_list, &dense_list, &RrfConfig::default());
        out.truncate(top_k);

        debug!("Hybrid search returning {} fused results", out.len());
        Ok(out)
    }
}
