#![allow(unused_variables, unused_imports, dead_code)]
use std::collections::HashMap;
use std::sync::Arc;
use error::RagError;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

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
use tokio::time::{timeout, sleep, Duration};
use tracing::{debug, instrument};
use uuid::Uuid;

pub mod error;
pub mod fusion;
pub use fusion::{normalize_scores, ScoreNorm, RrfConfig, MmrConfig, Similarity, rrf_fuse, mmr_select};
pub mod context;
pub use context::{
    TokenBudget, ContextPartKind, Modality, ContextPart, ContextStats, AssembledContext,
    AssemblyPolicy, Ordering, TokenCounter, ApproxCharTokenizer, assemble_context,
};
pub mod core;
pub use core::{RagService, RagConfig, RetrievalStrategy, Reranker, NoopReranker};
pub use ploke_db::bm25_index::bm25_service::Bm25Status;

const BM25_TIMEOUT_MS: u64 = 250;
const BM25_RETRY_BACKOFF_MS: [u64; 2] = [50, 100];
