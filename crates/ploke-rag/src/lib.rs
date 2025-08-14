#![allow(unused_variables, unused_imports, dead_code)]
use std::sync::Arc;
use std::collections::HashMap;

use ploke_db::{
    bm25_index::bm25_service::{self, Bm25Cmd}, search_similar_args, Database, DbError, NodeType, SimilarArgs, TypedEmbedData
};
use ploke_embed::indexer::{IndexerTask, EmbeddingProcessor};
use ploke_core::EmbeddingData;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum RagError {
    #[error("Database error: {0}")]
    Db(#[from] DbError),

    #[error("Channel error: {0}")]
    Channel(String),

    #[error("Embedding error: {0}")]
    Embed(String),
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
    pub fn new(db: Arc<Database>, dense_embedder: Arc<EmbeddingProcessor>) -> Result<Self, RagError> {
        let bm_embedder = bm25_service::start_default(db.clone())?;
        Ok(Self { db, dense_embedder, bm_embedder })
    }

    /// Execute a BM25 search against the in-memory sparse index.
    /// Returns a Vec of (document_id, score) pairs sorted by relevance.
    pub async fn search_bm25(&self, query: &str, top_k: usize) -> Result<Vec<(Uuid, f32)>, RagError> {
        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Search {
                query: query.to_string(),
                top_k,
                resp: tx,
            })
            .await
            .map_err(|e| RagError::Channel(format!("failed to send BM25 search command: {}", e)))?;

        rx.await
            .map_err(|e| RagError::Channel(format!("BM25 search response channel closed: {}", e)))
    }

    /// Trigger a rebuild of the BM25 sparse index.
    /// For now, this is a fire-and-forget command to the BM25 service.
    pub async fn bm25_rebuild(&self) -> Result<(), RagError> {
        self.bm_embedder
            .send(Bm25Cmd::Rebuild)
            .await
            .map_err(|e| RagError::Channel(format!("failed to send BM25 rebuild command: {}", e)))?;
        Ok(())
    }

    /// Perform a dense search using the HNSW index in the database.
    /// Returns a Vec of (document_id, score) pairs sorted by relevance.
    pub async fn search(&self, query: &str, top_k: usize) -> Result<Vec<(Uuid, f32)>, RagError> {
        // Generate embedding for the query
        let embeddings = self.dense_embedder.generate_embeddings(vec![query.to_string()]).await
            .map_err(|e| RagError::Embed(format!("failed to generate embeddings: {:?}", e)))?;
        
        let query_embedding = embeddings.into_iter().next()
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
                radius: 1.0, // Default radius value
            };
            
            let result = search_similar_args(args)
                .map_err(|e| RagError::Db(e))?;
            
            // Convert distance to similarity score (lower distance = higher similarity)
            let typed_results: Vec<(Uuid, f32)> = result.typed_data.v.into_iter()
                .zip(result.dist.into_iter())
                .map(|(embed_data, distance)| (embed_data.id, 1.0 - distance as f32))
                .collect();
            
            all_results.extend(typed_results);
        }

        // Sort by score (highest first) and take top_k
        all_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        all_results.truncate(top_k);
        
        Ok(all_results)
    }

    /// Perform a hybrid search (BM25 + dense).
    ///
    /// Strict mode: if the dense search fails, propagate an error (do not silently fall back).
    /// Runs BM25 and dense searches concurrently, fuses results using Reciprocal Rank Fusion (RRF),
    /// and returns the top_k results ordered by fused score (higher = better).
    pub async fn hybrid_search(&self, query: &str, top_k: usize) -> Result<Vec<(Uuid, f32)>, RagError> {
        // Kick off both searches concurrently.
        let bm25_fut = self.search_bm25(query, top_k);
        let dense_fut = self.search(query, top_k);

        let (bm25_res, dense_res) = tokio::join!(bm25_fut, dense_fut);

        // Propagate BM25 errors as-is.
        let bm25_list = bm25_res?;

        // Dense errors are mapped to RagError::Embed since we're in strict mode.
        let dense_list = dense_res
            .map_err(|e| RagError::Embed(format!("dense search failed: {:?}", e)))?;

        // RRF fusion parameters
        let rrf_k: f32 = 60.0;

        // Accumulate fused scores using ranks (1-based)
        let mut fused: HashMap<Uuid, f32> = HashMap::new();

        for (i, (id, _score)) in bm25_list.iter().enumerate() {
            let id = id.clone();
            let rank = (i + 1) as f32;
            let add = 1.0_f32 / (rrf_k + rank);
            *fused.entry(id).or_insert(0.0) += add;
        }

        for (i, (id, _score)) in dense_list.iter().enumerate() {
            let id = id.clone();
            let rank = (i + 1) as f32;
            let add = 1.0_f32 / (rrf_k + rank);
            *fused.entry(id).or_insert(0.0) += add;
        }

        // Collect, sort by fused score descending, and take top_k
        let mut out: Vec<(Uuid, f32)> = fused.into_iter().collect();
        out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(top_k);

        Ok(out)
    }
}
