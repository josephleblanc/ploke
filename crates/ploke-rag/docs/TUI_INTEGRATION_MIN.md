# ploke-rag ↔ ploke-tui: Minimal Integration Guide

This document captures the smallest set of facts you need to wire the TUI to the RAG service without pulling in the entire crate’s source into the chat context.

Recommended files to keep in chat while integrating (small total size)
- crates/ploke-rag/TUI_INTEGRATION_MIN.md   ← this file
- crates/ploke-rag/src/lib.rs               ← exports, public docs, constants
- crates/ploke-rag/src/core/mod.rs          ← RagService APIs used by the TUI
- crates/ploke-rag/src/error.rs             ← RagError (maps into ploke_error::Error)

What the TUI needs to do
1) Construct RagService once and share (Arc) across UI subsystems.
2) For quick search results (e.g., suggestions): call hybrid_search or search_bm25.
3) For prompt assembly prior to LLM calls: call get_context with a TokenBudget and a RetrievalStrategy.
4) Optionally expose persistence actions (bm25_save/load) and rebuild controls to the user.
5) Initialize tracing (once) in the TUI binary for diagnostics.

Key types and functions to import
- use ploke_rag::{RagService, RagConfig, RetrievalStrategy, TokenBudget, RrfConfig, MmrConfig};
- use ploke_rag::{Bm25Status, RagError};
- use std::sync::Arc;

Constructing RagService
- Prefer new_full when you have an IoManagerHandle; otherwise use new_with_config/new_with_io.
- RagService internally starts the BM25 actor via ploke-db’s bm25_service::start_default.

Example (Rust, synchronous layout with async blocks)
```rust
use std::sync::Arc;
use ploke_rag::{RagService, RagConfig, RetrievalStrategy, TokenBudget, RrfConfig};
use ploke_rag::error::RagError;
use ploke_db::Database;
use ploke_embed::indexer::EmbeddingProcessor;
use ploke_io::IoManagerHandle;

# async fn boot(db: Arc<Database>, embedder: Arc<EmbeddingProcessor>, io: IoManagerHandle) -> Result<RagService, RagError> {
    // Optional: customize timeouts/backoff/budgets via RagConfig::default() then mutate.
    let cfg = RagConfig::default();

    // Prefer new_full to enable snippet fetching in get_context.
    let rag = RagService::new_full(db, embedder, io, cfg)?;
    Ok(rag)
# }
```

Retrieval from the TUI
- Sparse lenient (falls back to dense on unready/empty):
  - rag.search_bm25(query, top_k).await -> Result<Vec<(Uuid, f32)>, RagError>
- Sparse strict (no fallback; errors if uninitialized/building/empty and results empty):
  - rag.search_bm25_strict(query, top_k).await -> Result<Vec<(Uuid, f32)>, RagError>
- Dense:
  - rag.search(query, top_k).await -> Result<Vec<(Uuid, f32)>, ploke_error::Error>  // convert to RagError::Embed as needed
- Hybrid fuse (BM25 + dense, weighted RRF):
  - rag.hybrid_search(query, top_k).await -> Result<Vec<(Uuid, f32)>, RagError>

Context assembly for LLM prompting
- Use get_context to fetch snippets, budget, dedup, and order:
```rust
# async fn assemble_prompt(rag: &RagService, user_query: &str) -> Result<(), RagError> {
    let budget = TokenBudget {
        max_total: 1024,
        per_file_max: 512,
        per_part_max: 256,
        reserves: None,
    };
    let strategy = RetrievalStrategy::Hybrid { rrf: RrfConfig::default(), mmr: None };
    let ctx = rag.get_context(user_query, 12, budget, strategy).await?;
    // ctx.parts: Vec<ContextPart> with { id, file_path (placeholder "id://UUID"), text, score, modality }
    // ctx.stats: counts/totals you can display in the TUI
    Ok(())
# }
```

BM25 lifecycle controls (optional UI binds)
- bm25_status().await -> Bm25Status { Uninitialized | Building | Ready { docs } | Empty | Error(String) }
- bm25_rebuild().await -> fire-and-forget signal to actor
- bm25_save(path).await / bm25_load(path).await -> persistence hooks (load triggers rebuild from DB in current implementation)

Timeouts, retries, and errors (what the TUI should expect)
- BM25 RPC timeout default: 250ms (configurable via RagConfig).
- Status retries/backoff on cold start: 50ms, 100ms.
- Errors:
  - RagError::Channel for mpsc/oneshot timeouts/closures.
  - RagError::Db for actor/DB-level issues from Save/Load/Status.
  - RagError::Search for strict-mode violations or IO issues during assembly.
  - From<RagError> -> ploke_error::Error is implemented.

Tracing/observability
- RagService ensures a minimal fmt subscriber in non-test builds; TUI binaries should set up their own subscriber early in main to control output/filters.
- Structured fields include: bm25_status, strict, attempts, timeout_ms, fallback_used, bm25_results, dense_results.

Suggested defaults for a TUI UX
- Retrieval: Hybrid with RRF defaults; omit MMR initially for lower latency.
- Token budget: 1k–1.5k total tokens, 256 per part, 512 per file.
- Provide a toggle for “BM25 strict mode” to surface indexing state vs. silently falling back.

Deeper integrations to consider later
- Background warm-up and readiness gate: call bm25_rebuild on startup and poll bm25_status until Ready/Empty using backoff.
- Rich snippet metadata: replace placeholder file_path with real paths and span stitching; highlight ranges in the TUI.
- Config UI: expose RRF/MMR/timeout/budget tuning; presets for “recall”, “precision”, “balanced”.
- Metrics pipeline: emit event counters (fallbacks, latencies) for telemetry.
- Persistence mgmt: autosave index sidecar on app exit; autoload on startup.
- Diversity: enable MMR with hydrated embeddings for better multi-file coverage.
