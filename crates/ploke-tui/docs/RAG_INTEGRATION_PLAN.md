# ploke-rag ↔ ploke-tui Integration Plan

Status: Draft v1
Audience: ploke-tui maintainers and contributors
Goal: Introduce BM25 and hybrid (BM25 + dense) retrieval via ploke-rag, preserving ploke-tui’s actor architecture, performance, and reliability.

## 1) Objectives

- Modularize retrieval and context assembly: all RAG logic (BM25, dense, hybrid, fusion, budgets) lives in ploke-rag.
- Preserve TUI architecture: StateManager is sole mutator; EventBus mediates; actors remain independent; UI stays responsive.
- Enable BM25 lifecycle controls (rebuild/save/load/status) and hybrid retrieval in UI workflows.
- Improve context building for LLM prompts with budgeted, ordered, deduplicated snippets.
- Provide a robust error/observability surface tailored for mission-critical Rust developer workflows.

Non-Goals (for now):
- Custom reranker integration beyond NoopReranker (leave extension points in place).
- Persisting BM25 embeddings across restarts beyond the sidecar hook (we wire the hooks, keep format stable).

## 2) Architectural Alignment

- Keep AppState as the shared data holder; mutations only via StateManager.
- EventBus retains two priorities: Realtime (UI updates) and Background (worker traffic).
- The new RagService is an actor client object (Arc-cloneable) that:
  - Talks to BM25 actor via mpsc/oneshot with timeouts/backoff (lenient mode).
  - Performs dense search via ploke-db HNSW.
  - Assembles contexts with budgeting and optional reranking if IoManager is provided.

Key principle: TUI never directly touches DB for retrieval/assembly; it calls RagService.

## 3) Construction and Ownership

Current code in ploke-tui/src/lib.rs constructs RagService with new(...), omitting IoManager, which prevents get_context from functioning. We will:
- Construct RagService with new_full(db, embedder, io, cfg) so context assembly can read snippet texts.
- Store Arc<RagService> in AppState (already an Option). We will make this a Some(_) on boot; on construction failure, we’ll emit an Error event and gracefully degrade (disable RAG actions).

RagConfig defaults are acceptable initially; expose toggles in user_config (see Section 7).

## 4) Command and Event Surfaces

Extend StateCommand and AppEvent to cover RAG operations. Respect existing conventions (CQRS-like flow; AppEvent priority mapping).

Commands (StateCommand):
- RagHybridSearch { req_id: Uuid, query: String, top_k: usize }
- RagSparseSearch { req_id: Uuid, query: String, top_k: usize, strict: bool }
- RagDenseSearch { req_id: Uuid, query: String, top_k: usize }
- RagAssembleContext { req_id: Uuid, user_query: String, top_k: usize, budget: TokenBudgetLike, strategy: RetrievalStrategyLike }
- RagBm25Status
- RagBm25Rebuild
- RagBm25Save { path: PathBuf }
- RagBm25Load { path: PathBuf }

Events (AppEvent):
- Rag(SearchResults { req_id: Uuid, results: Vec<(Uuid, f32)> })  // Realtime
- Rag(ContextConstructed { req_id: Uuid, ctx: AssembledContext }) // Background -> Realtime upon UI-ready
- System events reused for BM25 status/save/load (or add Rag(Bm25StatusUpdated(Bm25Status))) as Realtime.
- Keep Error(ErrorEvent) path for RagError -> ploke_error mapping.

Note: We already have RagEvent and GenerateContext(Uuid). We will either:
- Extend RagEvent with SearchResults and ContextConstructed variants; or
- Replace RagEvent with dedicated AppEvent variants for clarity.
Pick the option that minimizes churn in ContextManager and EventBus routing after we inspect app_state and context modules.

## 5) Flow Integration

A) Search (quick suggestions / commands):
- UI triggers a StateCommand::{RagHybridSearch|RagSparseSearch|RagDenseSearch}.
- StateManager calls rag.hybrid_search / rag.search_bm25(_strict) / rag.search, maps errors to ploke_error::Error (RagError -> Into).
- On success: AppEvent::Rag(SearchResults { ... }) is emitted (Realtime).

B) Prompt assembly:
- UI or LLM pipeline emits StateCommand::RagAssembleContext when a user message is added or when “Generate Context” is explicitly requested.
- StateManager calls rag.get_context(user_query, top_k, budget, strategy).
- On success: AppEvent::Rag(ContextConstructed { ... }) (Realtime). On error: Error(ErrorEvent) with appropriate severity.
- LlmManager consumes ContextConstructed to form prompts.

C) BM25 lifecycle:
- On boot: optionally call rag.bm25_status(); if Uninitialized or Empty, UI can show a hint to run indexing.
- Background action: RagBm25Rebuild spawns rebuild (fire-and-forget), status polled via RagBm25Status.
- Persistence: RagBm25Save / RagBm25Load bound to user commands; on completion, show success/failure in UI.

## 6) EventBus Considerations

- Keep Rag events as Background during heavy work, switch to Realtime on final delivery to render promptly.
- Maintain bounded channels to prevent backpressure collapse (existing 100/1000 caps fine initially).
- StateManager is responsible for deciding when to emit Realtime vs Background events for RAG.

## 7) Configuration Surface (user_config)

Expose a minimal, opinionated set with safe defaults:
- rag.strict_bm25_by_default: bool (default false).
- rag.bm25_timeout_ms: u64 (default 250).
- rag.bm25_retry_backoff_ms: Vec<u64> (default [50, 100]).
- rag.hybrid.rrf: RrfConfig (k defaults; weights default 1.0).
- rag.hybrid.mmr: Option<MmrConfig> (default None).
- rag.budget: TokenBudget { max_total: 1024..1536, per_file_max: 512, per_part_max: 256 }.

Apply at RagService construction via RagConfig; allow overrides via :commands. Future: presets (recall/precision/balanced).

## 8) Error Handling and Severity

- Map RagError into ploke_error::Error (already implemented in ploke-rag).
- Emit ErrorEvent via existing ResultExt/ErrorExt helpers:
  - Channel errors (timeouts/closed) => Warning (recoverable/transient).
  - Db errors => Error (persistent until fixed).
  - Embed errors => Error (user-actionable if embedder misconfigured).
  - Search errors (strict violations) => Warning (informative).
- Keep UI non-blocking; show lightweight toasts/banners with detail on demand.

## 9) Observability

- RagService initializes minimal fmt layer in non-test; TUI uses tracing_setup to control output/filters.
- Emit structured fields when sending commands and receiving results:
  - bm25_status, strict, attempts, timeout_ms, fallback_used, bm25_results, dense_results, rrf_applied, mmr_applied.
- For critical paths (context assembly), record timing spans and snippet counts.

## 10) Performance and Concurrency

- Construct RagService once; share Arc in AppState.
- Ensure all RAG calls are spawned in Background tasks; only final delivery is Realtime.
- Warm-up path: optional bm25_status check on boot; optionally trigger rebuild on user action.
- Keep top_k conservative in UI interactions (e.g., 10–20) to minimize allocations and sorting overhead.
- Avoid unnecessary clones; reuse Vecs where possible in StateManager/ContextManager.

## 11) Security and Privacy

- Retrieval operates locally against DB and BM25 actor; no external code exfiltration.
- Respect user-configured workspace boundaries; refuse to read outside workspace_root (already practiced).
- Ensure all IO for snippet fetching uses IoManager (single surface for file access and policy).

## 12) Testing Strategy

Unit:
- Map RagError -> ploke_error::Error severity emission.
- Command handlers in StateManager (search/context/bm25) happy-path and failure-path.

Integration:
- Boot with fixture DB; run RagHybridSearch and RagAssembleContext flows; verify UI events emitted.
- BM25 lifecycle: status -> rebuild -> status, save/load roundtrip (mocked paths).
- Concurrency/backpressure: bounded mpsc/broadcast with controlled lag (Lagged handling).

Bench:
- Dense and hybrid retrieval latency under controlled corpus sizes.
- Context assembly throughput for typical top_k and budgets.

## 13) Migration Plan (Stages)

Stage 0 (Planning): This document.

Stage 1 (Wire-in RagService correctly):
- [x] Change construction to RagService::new_full(db, embedder, io, cfg).
- [x] Store Some(Arc<RagService>) in AppState.rag; 
- (ongoing) remove None-paths in downstream code or guard with clear fallbacks.

Stage 2 (Commands and events):
- [x] Extend StateCommand and AppEvent (or RagEvent) for search/context/BM25.
- [x] Implement handlers in StateManager; route results via EventBus.

Stage 3 (ContextManager refactor):
- [x] Replace in-crate context building with rag.get_context.
- [x] Ensure llm_manager integration consumes ContextConstructed to build prompts.

Stage 4 (UI polish):
- Add modeless quick-search (hybrid) and context preview panes.
- [x] Expose BM25 status and lifecycle actions via commands.

Stage 5 (Cleanup and parity):
- Remove legacy RAG code paths in ploke-tui once parity verified.
- Add docs and help entries.

## 14) Files Likely to Change (request before edits)

Please add these files to the chat when ready to implement:
- crates/ploke-tui/src/app_state/mod.rs  (StateCommand/AppEvent additions, AppState.rag wiring)
- crates/ploke-tui/src/app/mod.rs        (UI triggers; display search/context results; commands)
- crates/ploke-tui/src/context/mod.rs     (ContextManager to call RagService::get_context)
- crates/ploke-tui/src/llm/mod.rs         (Consume ContextConstructed to build prompts)
- crates/ploke-tui/src/user_config.rs     (RagConfig surface; defaults and persistence)
- crates/ploke-tui/src/lib.rs             (Swap RagService::new -> new_full; error routing)
- crates/ploke-tui/src/error.rs           (If new error variants or mapping helpers are needed)
- crates/ploke-tui/src/tracing_setup.rs   (Ensure filters include rag paths as needed)

## 15) Acceptance Criteria

- RagService constructed with IoManager (new_full) and available via AppState.rag (Some).
- StateManager exposes search/context/BM25 commands; emits corresponding events.
- UI can trigger hybrid search and display results; can assemble and display context; BM25 status visible.
- No UI stalls; background tasks bounded; errors surfaced with appropriate severity.
- Tests cover happy/failure paths; basic benchmark numbers recorded.

Build the best possible thing now; iterate to better. This plan de-risks integration by keeping changes localized and reversible, while enabling high-quality retrieval and context assembly for Rust developers.
