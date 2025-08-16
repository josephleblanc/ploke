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
  - Tie-breaking: stable ordering by UUID if all else equal.
- Maximal Marginal Relevance (MMR) for diversity
  - Select a final set of results balancing relevance and diversity.
  - Use cosine similarity on vector embeddings (reuse `ploke-embed` for vectors if needed).
  - Configurable lambda (tradeoff), similarity function, and candidate pool size.
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

- Index status and readiness
  - Expose a lightweight status query on actor channel (to be implemented in `ploke-db`) so `ploke-rag` can vary behavior without guessing.
- Persistence hooks
  - Add hooks in `ploke-db` for serializing/deserializing the sparse index to disk; `ploke-rag` triggers these during startup/shutdown or on-demand.
- Fallback behavior
  - Maintain current graceful fallback to dense when BM25 returns empty. Make it configurable per-call (strict vs. lenient).
- API additions in RagService (non-breaking)
  - bm25_status() -> Result<Bm25Status, RagError>
  - bm25_save() / bm25_load() -> Result<(), RagError> (call-through to `ploke-db` when available)
  - search_bm25_strict(query, k) -> Result<Vec<(Uuid, f32)>, RagError> (fails if index empty)

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
