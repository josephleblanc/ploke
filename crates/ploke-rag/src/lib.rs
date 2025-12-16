#![allow(unused_variables, unused_imports, dead_code)]
//! ploke-rag — Retrieval and context assembly for the PLOKE workspace
//!
//! This crate provides the "R" and "A" of RAG for the PLOKE workspace:
//! - Retrieval: sparse (BM25) via an in-memory actor and dense (HNSW) via `ploke-db`.
//! - Fusion: score normalization, weighted Reciprocal Rank Fusion (RRF), optional Maximal Marginal Relevance (MMR).
//! - Context assembly: selecting, trimming, ordering and packaging snippets under a token budget.
//!
//! The design emphasizes deterministic behavior, strong observability, and strict error handling,
//! while integrating with the rest of the PLOKE system (database, IO manager, and embedding backends).
//!
//! Highlights
//! - Async BM25 client wrapper with timeouts, retries/backoff, and strict vs. lenient modes.
//! - Dense retrieval across all primary node types using [`ploke_db`]'s HNSW helpers.
//! - Pluggable fusion (RRF + MMR) and score normalization utilities.
//! - Context assembly with budgeting, deduplication, and stable ordering.
//! - Structured tracing for operability (suitable for tests and metrics-ready).
//!
//! Concurrency and actors
//! - BM25 is served by an actor (Tokio task) in `ploke-db` receiving `Bm25Cmd` over `mpsc` and
//!   replying via `oneshot` channels. All calls from this crate are enforced with client-side timeouts.
//! - Dense retrieval executes against `ploke-db` HNSW via synchronous calls wrapped in async fns.
//! - Snippet IO is delegated to `ploke-io`'s actor (if configured) and is used by reranking and
//!   by the context assembler.
//!
//! Error handling
//! - Channel send/recv failures map to [`RagError::Channel`].
//! - Database/actor errors map to [`RagError::Db`] while preserving inner messages.
//! - Embedding pipeline failures map to [`RagError::Embed`].
//! - Search state violations (e.g., strict BM25 while uninitialized) map to [`RagError::Search`].
//! - A conversion into the workspace error type is provided (`impl From<RagError> for ploke_error::Error`).
//!
//! Observability
//! - The BM25 client wrapper emits fields such as: `bm25_status`, `strict`, `attempts`,
//!   `timeout_ms`, `fallback_used`, `bm25_results`, and `dense_results`.
//! - Tests can opt-in to tracing; non-test binaries get a minimal, target-less fmt layer by default.
//!
//! Configuration
//! - See [`RagConfig`] to control BM25 timeouts/backoff/strict default, fusion settings,
//!   per-type dense search parameters, context assembly policy, tokenizer, and (optional) reranker.
//!
//! Quickstart
//! ```no_run
//! use std::sync::Arc;
//! use ploke_rag::{RagService, RetrievalStrategy, TokenBudget};
//! use ploke_embed::runtime::EmbeddingRuntime;
//! use ploke_db::{Database};
//!
//! # async fn doc_example(mut db: Arc<Database>, embedder: Arc<EmbeddingRuntime>) -> Result<(), Box<dyn std::error::Error>> {
//! // Construct the service (BM25 actor is started internally).
//! let rag = RagService::new(db.clone(), embedder.clone())?;
//!
//! // Sparse-only (lenient): falls back to dense if BM25 is unready/empty.
//! let bm25_hits = rag.search_bm25("how to implement graph nodes", 15).await?;
//!
//! // Hybrid: concurrent BM25 + dense, fused via weighted RRF.
//! let fused = rag.hybrid_search("module visibility rules", 15).await?;
//!
//! // Assemble a context for downstream prompting.
//! let budget = TokenBudget { max_total: 1024, per_file_max: 512, per_part_max: 256, reserves: None };
//! let ctx = rag.get_context(
//!     "module visibility rules",
//!     12,
//!     &budget,
//!     RetrievalStrategy::Hybrid { rrf: Default::default(), mmr: None },
//! ).await?;
//! # Ok(()) }
//! ```
//!
//! BM25 lifecycle and persistence
//! - This crate re-exports [`Bm25Status`] and provides thin call-throughs for `save` and `load`.
//!   Current persistence is a lightweight sidecar (version + doc_count) and `load` triggers a rebuild
//!   from the DB in `ploke-db`. Keep API usage stable to benefit from future serialization improvements.
//!
//! Algorithms
//! - RRF: [`rrf_fuse`] implements weighted reciprocal rank fusion with stable UUID tie-breaking.
//! - Score normalization: [`normalize_scores`] provides min-max, z-score, and logistic transforms.
//! - MMR: [`mmr_select`] performs diversity-aware selection using cosine similarity on normalized vectors.
//!
//! Threading and performance
//! - `RagService` holds `Arc`s to the database handle and embedding processor, and an `mpsc` sender to BM25.
//!   Clone or share it behind `Arc` in your application services.
//! - BM25 RPCs are local and fast; a client-side timeout guards unexpected stalls (default 250ms) with
//!   up to two status/backoff retries in lenient mode.
//!
//! Stability and versioning
//! - Public APIs follow SemVer. The default behavior of existing methods is preserved; strict and persistence
//!   features are opt-in. See `crates/ploke-rag/IMPLEMENTATION_NOTES.md` and `plans.md` for cross-crate design.
//!
//! Re-exports
//! - [`Bm25Status`], [`RrfConfig`], [`MmrConfig`], [`Similarity`], [`ScoreNorm`],
//!   [`normalize_scores`], [`rrf_fuse`], [`mmr_select`], and context assembly types.
//!
//! Tips
//! - In tests, initialize tracing with your preferred subscriber; the library avoids double-registration.
//! - Provide a custom [`TokenCounter`] to align budgeting with your downstream LLM tokenizer.
//!
use std::collections::HashMap;
use std::sync::Arc;
// use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

#[cfg(not(test))]
static TRACER_INIT: std::sync::Once = std::sync::Once::new();

#[cfg(test)]
static _TRACE_MARKER: () = ();

// #[cfg(not(test))]
// fn ensure_tracer_initialized() {
//     TRACER_INIT.call_once(|| {
//         let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);
//         let _ = tracing_subscriber::registry().with(fmt_layer).try_init();
//     });
// }

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
use tokio::time::{sleep, timeout, Duration};
use tracing::{debug, instrument};
use uuid::Uuid;

pub mod error;
pub mod fusion;
pub use error::RagError;
pub use fusion::{
    mmr_select, normalize_scores, rrf_fuse, MmrConfig, RrfConfig, ScoreNorm, Similarity,
};
pub mod context;
pub use context::{
    assemble_context, ApproxCharTokenizer, AssemblyPolicy, Ordering, TokenBudget, TokenCounter,
};
pub mod core;
pub use core::{NoopReranker, RagConfig, RagService, Reranker, RetrievalStrategy};
pub use ploke_db::bm25_index::bm25_service::Bm25Status;

const BM25_TIMEOUT_MS: u64 = 250;
const BM25_RETRY_BACKOFF_MS: [u64; 2] = [50, 100];
