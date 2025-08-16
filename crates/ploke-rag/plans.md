# ploke-rag: Concrete Implementation Plan

This document describes the next stages of work for the `ploke-rag` crate. It focuses on retrieval quality, fusion and deduplication, context assembly, observability, configuration, and evaluation. Direct communication with local/remote LLMs is explicitly out of scope for this crate and will be provided by a separate crate. The plans below assume integration via traits and data structures shared across crates, while maintaining `ploke-rag` as the retrieval and context assembly engine.

## Scope and Interfaces

- In-scope
  - Dense retrieval via `ploke-db` HNSW index.
  - Sparse retrieval via the BM25 in-memory actor in `ploke-db`.
  - Hybrid fusion (BM25 + dense) with reproducible and configurable strategies.
  - Context assembly: selecting, trimming, ordering, and packaging code snippets for downstream prompting.
  - Deduplication and diversity-aware selection.
  - Observability (tracing, metrics-ready events), error handling, config management.
  - Evaluation and benchmarking for retrieval quality and latency.
- Out-of-scope
  - Direct LLM API calls (prompting, tool use, streaming). These belong to the LLM crate.
  - UI/TUI concerns beyond providing clear APIs and events to consumers.

## North-star requirements

- Reliability: deterministic, testable, traceable behavior; strict error propagation.
- Performance: predictable p50/p95 latencies (see targets below).
- Quality: strong recall@k and diversity, minimal duplication, high snippet relevance.
- Maintainability: cohesive modules, clear APIs, documented behavior, fine-grained tests.

## Milestones and Deliverables

- M0 Hardening (bugfixes and parity)
  - Acceptance: existing tests remain green; improved logging in existing paths; no public API breakage.
- M1 Fusion Quality (MMR + configurable RRF + score normalization)
  - Deliver: score normalization utilities, MMR selector, weighted RRF; config-driven strategy.
- M2 Context Assembly and Token Budgeting
  - Deliver: a context assembly pipeline with token budgeting, snippet stitching by file, dedup, and ordering.
- M3 BM25 Lifecycle Improvements (index status and persistence hooks)
  - Deliver: status query, graceful fallbacks, hooks for persistence (implemented in `ploke-db`), retry/backoff.
- M4 Evaluation and Benchmark Suite
  - Deliver: retrieval golden tests and criterion benches covering dense, sparse, hybrid, fusion variants.
- M5 Observability + Config
  - Deliver: structured tracing fields, optional metrics counters, config loader and live reconfig.

## Technical plan by area

### 1) Fusion and score handling

- Score normalization
  - Provide utilities to normalize scores from different modalities to a comparable space.
  - Techniques: min-max per modality, z-score, or calibrated logistic transform. Start with min-max with clamping and configurable epsilon.
- Reciprocal Rank Fusion (RRF)
  - Current basic RRF remains; make `k` and per-modality weights configurable.
  - Weighted formula: fused(u) = w_bm25 / (k + rank_bm25(u)) + w_dense / (k + rank_dense(u)); ranks are 1-based per list and missing ranks contribute 0.
  - Config: RrfConfig { k: f32, weight_bm25: f32, weight_dense: f32 } with defaults k=60.0, weight_bm25=1.0, weight_dense=1.0.
  - Tie-breaking: stable ordering by (fused score desc, then UUID asc) if all else equal.
- Maximal Marginal Relevance (MMR) for diversity
  - Select a final set of results balancing relevance and diversity.
  - Use cosine similarity on vector embeddings (reuse `ploke-embed` for vectors if needed).
  - Configurable lambda (tradeoff), similarity function, and candidate pool size.
  - Objective per candidate x: score(x) = lambda * rel(x) - (1 - lambda) * max_{y in S} sim(x, y), where S is the selected set; higher is better.
  - Config: MmrConfig { lambda: f32, sim_metric: Similarity, candidate_pool: usize } with defaults lambda=0.7, sim_metric=Cosine, candidate_pool=50.
  - Similarity: Cosine on normalized vectors; treat missing embeddings as zero-vector (similarity 0.0).
  - Algorithm: greedy selection from top-N candidates (candidate_pool), start with highest rel(x); on ties break by UUID asc; deterministic given same inputs.
- Proposed internal module and signatures
  - fusion module:
    - rrf_fuse(bm25: &[(Uuid, f32)], dense: &[(Uuid, f32)], cfg: &RrfConfig) -> Vec<(Uuid, f32)>
    - mmr_select(candidates: &[(Uuid, f32)], k: usize, embeddings: &HashMap<Uuid, Vec<f32>>, cfg: &MmrConfig) -> Vec<(Uuid, f32)>
    - normalize_scores(scores: &[(Uuid, f32)], method: ScoreNorm) -> Vec<(Uuid, f32)>

### 2) Context assembly and token budgeting

- Inputs
  - Ranked list of node IDs with fused scores.
  - Snippet fetcher (via `ploke-io`) to retrieve text.
  - Tokenizer abstraction (trait) to count tokens for budgeting. Actual tokenization implementation may live in LLM crate; here we depend on a trait injected by the consumer.
- Core features
  - Deduplicate via tracking hash and file-path/line-range normalization.
  - Stitch contiguous snippets by file to minimize fragmentation.
  - Allocate a token budget per category (e.g., code vs. doc comments) and per node type using a policy.
  - Ordering policy: by fused score, then by structural importance (e.g., definitions before references), configurable.
- Outputs
  - AssembledContext with:
    - parts: ordered list of ContextPart { file_path, span(s), text, reason, score, source_modality }
    - stats: token counts, unique files, duplication ratio, coverage by node type
  - Transform to a downstream PromptParts type expected by the LLM crate.
- Proposed internal module and types
  - context module:
    - struct TokenBudget { max_total: usize, per_file_max: usize, per_part_max: usize, reserves: Option<HashMap<String, usize>> }
    - enum ContextPartKind { Code, Doc, Signature, Metadata }
    - struct ContextPart { id: Uuid, file_path: String, ranges: Vec<(usize, usize)>, kind: ContextPartKind, text: String, score: f32, modality: Modality }
    - struct AssembledContext { parts: Vec<ContextPart>, stats: ContextStats }
    - fn assemble_context(query: &str, hits: &[(Uuid, f32)], budget: &TokenBudget, policy: &AssemblyPolicy, tokenizer: &dyn TokenCounter, db: &Database, io: &IoManagerHandle) -> Result<AssembledContext, RagError>

### 3) BM25 actor lifecycle and fallbacks

Goal
- Make BM25 interactions explicit and observable: status, readiness, persistence, and deterministic fallbacks.
- Provide non-breaking APIs in ploke-rag that delegate to ploke-db’s BM25 actor, with strict vs. lenient behavior selectable per call.
- Ensure predictable error mapping and tracing for operability.

Implementation plan (incremental, PR-sized steps)

A) Status and readiness (requires ploke-db additions)
- Add status query to BM25 actor in ploke-db:
  - New enum Bm25Status { Uninitialized, Building, Ready { docs: usize }, Empty, Error(String) }.
  - New Bm25Cmd::Status { resp: oneshot::Sender<Result<Bm25Status, DbError>> }.
  - Actor maintains internal state transitions:
    - Uninitialized -> Building (on Rebuild) -> Ready|Empty or Error
    - Ready -> Building (on Rebuild) or stays Ready
    - Error can transition back to Building on Rebuild
- ploke-rag wiring:
  - RagService::bm25_status() -> Result<Bm25Status, RagError>:
    - Send Bm25Cmd::Status, map channel/db errors to RagError, return status.
- Behavior changes:
  - search_bm25() uses status for better logging and to decide whether to wait/retry (see backoff below).
  - hybrid_search() can include status in trace fields.

B) Persistence hooks (requires ploke-db additions)
- Add serialization/deserialization commands to BM25 actor:
  - Bm25Cmd::Save { path: PathBuf, resp: oneshot::Sender<Result<(), DbError>> }
  - Bm25Cmd::Load { path: PathBuf, resp: oneshot::Sender<Result<(), DbError>> }
- Define persistence semantics in ploke-db:
  - Save only when state is Ready or Empty (no concurrent Building writes).
  - Load transitions: Uninitialized/Empty -> Building -> Ready/Empty or Error, or Ready -> Ready (replace) with a short swap phase.
  - Validate version/format; incompatible versions return Error.
- ploke-rag call-through:
  - RagService::bm25_save(path: impl AsRef<Path>) -> Result<(), RagError>
  - RagService::bm25_load(path: impl AsRef<Path>) -> Result<(), RagError>
  - Trace events: persistence_started, persistence_done, bytes_written/read (if available), duration_ms.

C) Fallback behavior (strict vs. lenient)
- Keep current lenient fallback in search_bm25():
  - If BM25 returns empty results, call dense search and return dense results.
  - Add structured tracing fields: fallback_used=true, bm25_status=..., dense_len=..., bm25_len=0.
- Add strict variant:
  - RagService::search_bm25_strict(query, k) -> Result<Vec<(Uuid, f32)>, RagError>
    - If BM25 returns empty and status is not Ready, return RagError::Search("bm25 index not ready/empty").
    - If BM25 returns empty but status is Ready with docs=0 (Empty), return RagError::Search("bm25 index empty").
    - Do not fall back to dense; let caller decide strategy.
- Hybrid behavior:
  - hybrid_search() remains strict about dense search errors; BM25 errors bubble as-is.
  - If BM25 returns empty while status is Building/Uninitialized, proceed with dense-only fusion (same as current) and trace fallback_used=true.

D) Timeouts, retry and backoff
- Add per-call timeout for BM25 actor requests (default 250ms; configurable later):
  - If status/search/save/load exceeds timeout, map to RagError::Channel("timeout ...").
- Add lightweight retry for Status during cold start:
  - In search_bm25() lenient mode: on Status=Building or Uninitialized, retry search up to 2 times with exponential backoff (50ms, 100ms) before falling back to dense.
  - Emit trace events with attempt number and sleep_ms.
- All sleeps via tokio::time::sleep to avoid blocking.

E) Error mapping and observability
- Map DbError and channel errors consistently:
  - Channel send/recv -> RagError::Channel
  - Actor/DB-specific failures -> RagError::Db via From
  - Persistence issues -> RagError::Db (surface inner error text)
- Tracing fields to add on all new paths:
  - bm25_status, strict, timeout_ms, attempts, fallback_used, bm25_results, dense_results.
- Optional events for metrics later:
  - bm25_status_counts, bm25_search_latency_ms, bm25_fallback_count, bm25_persist_success/failure.

F) Public API additions in ploke-rag (non-breaking)
- enum Bm25Status (re-export the ploke-db definition to avoid duplication or define a local mirror with From/Into).
- RagService:
  - async fn bm25_status(&self) -> Result<Bm25Status, RagError>
  - async fn bm25_save<P: AsRef<std::path::Path> + Send>(&self, path: P) -> Result<(), RagError>
  - async fn bm25_load<P: AsRef<std::path::Path> + Send>(&self, path: P) -> Result<(), RagError>
  - async fn search_bm25_strict(&self, query: &str, top_k: usize) -> Result<Vec<(Uuid, f32)>, RagError>
- Configuration (future, separate PR):
  - RagConfig.bm25: { timeout_ms, retries, backoff_ms, strict_default, persist_path }

G) Tests (unit + integration)
- Unit tests (ploke-rag, using a stub/fake bm25 actor or feature-flagged mock in ploke-db):
  - bm25_status_ok: returns Ready/Empty/Error deterministically.
  - lenient_fallback_triggers: empty BM25 results -> dense used; strict returns error.
  - timeout and retry: injected delays cause retry then fallback.
- Integration tests (existing fixture DB):
  - Rebuild then search: ensure non-empty results without dense fallback.
  - Cold start strict: search_bm25_strict before rebuild -> error.
  - Save/Load roundtrip (when ploke-db implements persistence): after Load, first search succeeds without Rebuild.
- Tracing assertions:
  - Use test subscriber to assert presence of fallback_used and bm25_status fields on the appropriate spans.

H) Rollout and compatibility
- Keep existing search_bm25() semantics (lenient) to avoid breaking callers.
- Introduce new methods alongside old ones; mark strict behavior and persistence as opt-in.
- Document state transitions and error cases in RagService rustdoc.

Out-of-scope for this point
- Actual persistence file format and storage policy (lives in ploke-db).
- Long-lived background autosave/autoload policies (could be added later behind config).

Deliverables for Point 3
- New BM25 actor messages and status enum in ploke-db; adapter methods in RagService.
- Lenient and strict BM25 search modes with retries/backoff.
- Save/Load call-throughs with tracing and error mapping.
- Tests covering status, fallback correctness, and (if available) persistence.

### 4) Public APIs and configurable strategies

- New high-level API
  - get_context(query: &str, top_k: usize, budget: TokenBudget, strategy: RetrievalStrategy) -> Result<AssembledContext, RagError>
    - RetrievalStrategy: Dense, Sparse, Hybrid { rrf: RrfConfig, mmr: Option<MmrConfig> }
- Configuration surface
  - RagConfig loaded at `RagService::new(...)` with runtime overrides:
    - search: ef, radius, max_hits per `NodeType`; modality weights; rrf_k
    - fusion: ScoreNorm, MmrConfig { lambda, sim_metric, candidate_pool }
    - assembly: Token budgets, per-type caps, dedup policy, ordering policy
    - fallbacks: strict/lenient, BM25 timeout/backoff
- Traits for external integration (residing in `ploke-rag`)
  - trait TokenCounter { fn count(&self, text: &str) -> usize; }
  - trait Reranker { async fn rerank(&self, query: &str, candidates: Vec<(Uuid, String)>) -> Result<Vec<(Uuid, f32)>, RagError>; }
    - Default no-op implementation in this crate; LLM-based reranker provided by the LLM crate.

### 5) Observability and errors

- Tracing
  - Consistent span fields: query_len, top_k, strategy, rrf_k, mmr_lambda, budget_used, files_touched, dedup_ratio, time_to_first_result_ms, p50/p95 internal timings (as events).
- Metrics-ready events
  - Emit structured events that can be adapted to `metrics` later without changing call sites.
- Error taxonomy
  - Extend RagError with Context, Config, Rerank variants while preserving From conversions to `ploke_error::Error`.
  - Maintain strict vs. lenient behaviors in public APIs.

### 6) Evaluation and benchmarking

- Golden retrieval tests
  - Use the existing fixture database; curate a set of queries with expected file/snippet containments.
  - Metrics: recall@k, MRR@k, diversity (unique files/snippets), duplication ratio, assembly coverage.
- Criterion benches
  - Benchmarks for dense-only, sparse-only, hybrid(RRF), hybrid(RRF+MMR), and context assembly.
  - Data sizes: small (~10k), medium (~100k), large (~500k) node counts if feasible.
- Targets (initial)
  - Dense search p50 ≤ 40ms, p95 ≤ 150ms for k=15 on the fixture DB.
  - Hybrid end-to-end (retrieval + fusion, no assembly) p50 ≤ 70ms, p95 ≤ 250ms.
  - Context assembly p50 ≤ 30ms, p95 ≤ 120ms for 15 parts and 4 files.

## Work breakdown and sequencing

- M0 Hardening
  - Add tracing fields to current search paths and `hybrid_search`.
  - Add unit tests for fallback correctness and sorting stability.
- M1 Fusion Quality
  - Implement `normalize_scores`, `rrf_fuse` with weights, and `mmr_select`.
  - Add config structs: `ScoreNorm`, `RrfConfig`, `MmrConfig`; wire into `RagService`.
  - Tests: fusion determinism, diversity improvements, parameter boundaries.
- M2 Context Assembly
  - Implement types in `context` module, stitch ranges, dedup by tracking_hash/file segments.
  - Define `TokenCounter` trait and add an adapter type for consumer-provided tokenizer.
  - Implement `get_context` with policy and budgeting.
  - Tests: token budget enforcement, dedup, ordering, stability across runs.
- M3 BM25 Lifecycle
  - Introduce status/persistence commands (PR to `ploke-db`), wire through `RagService`.
  - Add strict/lenient variants and configuration.
  - Tests: cold start, persisted warm start, empty-index strict failure.
- M4 Evaluation + Benches
  - Golden tests with assertions on recall/diversity/duplication.
  - Criterion benches for core operations with CI-friendly settings.
- M5 Observability + Config
  - Config loader: env + file + builder, with live update hook.
  - Extend tracing coverage; add metrics-ready events.

## Backward compatibility

- Keep existing APIs working; add new ones alongside.
- Default configuration mirrors current behavior (RRF basic, no MMR).
- Strictness and persistence features are opt-in.

## Risks and mitigations

- BM25 persistence semantics span crates: mitigate with a thin call-through in `ploke-rag` and unit tests that stub the actor.
- Tokenization variance: abstract via `TokenCounter`; provide a simple approximate counter for tests to keep determinism.
- Performance regressions: add benches and trace sampling gates early; prefer O(k log k) selection and preallocation.

## Acceptance criteria summary

- Functional: get_context returns assembled, budgeted context; hybrid search supports RRF+MMR; strict/lenient BM25 modes work.
- Quality: improved diversity and lower duplication vs. baseline; golden tests pass.
- Performance: meets initial latency targets under fixture DB.
- Observability: rich tracing fields present; errors informative and typed.
