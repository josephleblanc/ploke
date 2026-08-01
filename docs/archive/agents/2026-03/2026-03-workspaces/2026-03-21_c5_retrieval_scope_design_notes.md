# C5 Retrieval Scope Design Notes 2026-03-21

Backlinks:
- [2026-03-20_workspaces_implementation_plan.md](../../reports/2026-03-20_workspaces_implementation_plan.md)
- [2026-03-20_workspaces_acceptance_criteria.md](../../reports/2026-03-20_workspaces_acceptance_criteria.md)
- [2026-03-20_workspaces_progress_tracker.md](2026-03-20_workspaces_progress_tracker.md)
- [ADR-023-refactor-crate-focus.md](../../../design/adrs/proposed/ADR-023-refactor-crate-focus.md)

This note captures the Phase 6 `C5` discovery pass so retrieval-scope work can be resumed without re-reading the entire `ploke-db`/`ploke-rag` stack.

## Summary

The current retrieval stack is still unscoped.

- `RagService` exposes unscoped `search_bm25`, `search_bm25_strict`, `search`, `hybrid_search`, and `get_context`.
- Dense search already projects `namespace`, but does not use it to constrain search.
- BM25 search truncates in the in-memory indexer before any namespace-aware filter exists.
- The March 2026 workspace docs are the governing design source for `C5`.
- `ADR-023` is directionally consistent, but broader than this phase. `C5` should stay focused on retrieval scope rather than a full runtime-state redesign.

## Current code boundary

### `ploke-rag`

- `RagService` retrieval entrypoints live in [core/mod.rs](../../../../crates/ploke-rag/src/core/mod.rs):
  - `search_bm25(...)`
  - `search_bm25_strict(...)`
  - `search(...)`
  - `hybrid_search(...)`
  - `get_context(...)`
- `assemble_context(...)` in [context/mod.rs](../../../../crates/ploke-rag/src/context/mod.rs) only materializes the IDs it is given. It preserves scope if retrieval is already scoped, but it cannot establish scope on its own.
- `ploke-tui` callers currently do not pass any explicit retrieval scope. Relevant call paths include:
  - [rag/context.rs](../../../../crates/ploke-tui/src/rag/context.rs)
  - [rag/search.rs](../../../../crates/ploke-tui/src/rag/search.rs)
  - [tools/request_code_context.rs](../../../../crates/ploke-tui/src/tools/request_code_context.rs)
  - [app/commands/exec.rs](../../../../crates/ploke-tui/src/app/commands/exec.rs)

### `ploke-db`

- Dense search flows through [index/hnsw.rs](../../../../crates/ploke-db/src/index/hnsw.rs) into `HnswExt::search_similar_for_set(...)` in [multi_embedding/hnsw_ext.rs](../../../../crates/ploke-db/src/multi_embedding/hnsw_ext.rs).
- The dense Cozo query already joins through `module` and `file_mod` and projects `namespace` before `:limit $limit`.
- BM25 flows through `Bm25Cmd::Search` in [bm25_service.rs](../../../../crates/ploke-db/src/bm25_index/bm25_service.rs) into `Bm25Indexer::search(...)` in [bm25_index/mod.rs](../../../../crates/ploke-db/src/bm25_index/mod.rs).
- BM25 currently has no namespace-aware metadata in its search path and truncates immediately after scoring.

## Constraints from the acceptance docs

- Scope must be shared across dense, BM25, hybrid, and `get_context`.
- Scope must be enforced before dense `:limit`, BM25 `top_k`, dense fallback, and RRF fusion.
- Late caller-side filtering in `ploke-rag` or `ploke-tui` is explicitly insufficient.
- BM25 fallback/retry semantics must remain intact after scoping.
- Proof should focus on scope correctness, not exact cross-crate ranking.

## Recommended first-cut implementation

1. Introduce one shared retrieval-scope type in a shared crate.
   - Use the March docs as the source of truth.
   - A first cut can be `LoadedWorkspace | SpecificCrate(CrateId)`.
2. Thread that scope through:
   - `RagService::{search_bm25, search_bm25_strict, search, hybrid_search, get_context}`
   - dense DB search helpers
   - BM25 actor commands
   - the current `ploke-tui` RAG call sites
3. Dense scope should be enforced in the Cozo query before `:limit`.
4. BM25 scope should be enforced inside the in-memory indexer before `truncate(top_k)`.
5. Keep `LoadedWorkspace` behavior aligned with the current loaded dataset.
   - The first real narrowing proof comes from `SpecificCrate(CrateId)`.

## Implementation risks

- BM25 currently stores only `Uuid`-keyed docs plus token metadata. To scope before `top_k`, the in-memory indexer needs namespace information available at search time.
- Dense search already has namespace projection, but filtering after the ANN candidate set may reduce recall if the candidate budget is too narrow.
- The `C5` work widens public APIs across `ploke-rag`, `ploke-db`, and `ploke-tui`; this is a real contract change.
- `LoadedWorkspace` scope may be a no-op for the current whole-workspace restore/index model, but the type still matters because later phases depend on one shared scope contract.

## Witness plan

The intended proof split is:

- `ploke-db` unit/integration tests:
  - dense search respects `SpecificCrate` scope before `:limit`
  - BM25 search respects `SpecificCrate` scope before `top_k`
- `ploke-rag` tests:
  - dense, BM25, and hybrid all return only in-scope IDs
  - `get_context(...)` only materializes IDs already admitted by scoped retrieval
- `ploke-tui` can keep using `LoadedWorkspace` as the default call-site scope for now
  while analysis-time crate focus remains an explicit caller decision in later work.

## Design note

This phase should not turn into a full `crate_focus` retirement. The current ADR is relevant background, but `C5` should only introduce the minimum shared retrieval-scope contract needed to satisfy the acceptance criteria and keep the later phases honest.
