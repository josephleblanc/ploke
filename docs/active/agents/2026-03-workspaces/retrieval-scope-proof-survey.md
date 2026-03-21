# Retrieval Scope Proof Survey

Backlink: [docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md)

This survey narrows the workspace-scoping question to one proof boundary: scope must be applied before any candidate cap (`top_k`, `limit`, or RRF truncation) if the acceptance claim is meant to be provable rather than heuristic.

## Current Proof Boundary

- Dense search in `RagService::search()` collects per-node-type candidates, converts them to `(Uuid, score)`, sorts globally, and truncates after the merge at [crates/ploke-rag/src/core/mod.rs#L486-L544](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L486-L544).
- The lower-level HNSW query already applies `:limit $limit` inside the Cozo script at [crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L337-L400](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L337-L400). Any workspace filter added only in `RagService` would therefore happen after a candidate cap has already been enforced.
- BM25 search is even earlier-truncated: `Bm25Cmd::Search` calls `indexer.search(&query, top_k)` at [crates/ploke-db/src/bm25_index/bm25_service.rs#L127-L133](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/bm25_service.rs#L127-L133), and `Bm25Indexer::search()` truncates immediately at [crates/ploke-db/src/bm25_index/mod.rs#L660-L669](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/mod.rs#L660-L669).
- `search_bm25()` can fall back to dense search when BM25 is not ready or empty, so the same scope must be threaded into the fallback path too; otherwise the fallback can violate the requested workspace slice at [crates/ploke-rag/src/core/mod.rs#L234-L328](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L234-L328).
- `hybrid_search()` truncates each modality independently, fuses with RRF, then truncates again at [crates/ploke-rag/src/core/mod.rs#L656-L679](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L656-L679). Post-fusion filtering cannot recover scoped candidates removed earlier.
- `assemble_context()` only materializes the IDs it is given; it can preserve scope, but it cannot establish scope correctness on its own at [crates/ploke-rag/src/context/mod.rs#L164-L330](/home/brasides/code/ploke/crates/ploke-rag/src/context/mod.rs#L164-L330).

## What Can Be Proven

- Returned IDs belong to the requested workspace or crate scope on controlled fixtures, if scope is enforced before truncation and each returned ID is checked back against namespace/workspace membership.
- Dense DB search can already prove neighbor and `limit` behavior on a controlled fixture, but not workspace scope, via [crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L926-L1000](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L926-L1000).
- Current BM25 and hybrid tests prove non-empty retrieval and snippet containment for a single corpus, not workspace scoping, at [crates/ploke-rag/src/core/unit_tests.rs#L259-L335](/home/brasides/code/ploke/crates/ploke-rag/src/core/unit_tests.rs#L259-L335) and [crates/ploke-rag/src/core/unit_tests.rs#L605-L639](/home/brasides/code/ploke/crates/ploke-rag/src/core/unit_tests.rs#L605-L639).

## What Is Only Heuristic

- Filtering after `top_k` or after RRF fusion only proves that some returned rows are in scope; it does not prove that the best in-scope candidates were ever considered.
- Exact global ranking across workspace members is not a stable acceptance target for dense retrieval because the HNSW path is approximate.
- Scope checks based only on snippet text are weak when crates have overlapping symbols or similar lexical content.

## Recommended Acceptance Wording

- Dense: "Workspace scope is enforced in the DB search query before `:limit`; the returned dense hits are members of the requested workspace, and controlled fixtures prove at least one expected in-scope hit survives even when out-of-scope documents are present."
- BM25: "Workspace scope is enforced before BM25 `top_k` truncation, including the dense fallback path; caller-side post-filtering is not sufficient to satisfy the claim."
- Hybrid: "Dense and BM25 results are each scoped before fusion; RRF and final truncation operate only on already-scoped candidate lists."
- Context: "Given already-scoped hits, `assemble_context()` must not introduce IDs outside the hit list."

