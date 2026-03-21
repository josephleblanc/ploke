# DB/RAG Workspace Survey

No sub-agent reports were available under `docs/active/agents/2026-03-workspaces/` at survey time, so this is a direct code survey of the DB and RAG layers.

## Summary

The DB layer already supports multiple embedding sets and per-set HNSW relations, but the public orchestration still behaves like there is one active embedding set per loaded DB. BM25 is also still one in-memory index rebuilt from the whole database, not per workspace or per crate. That means the low-level primitives are mostly present, but the workspace-wide load/index/search/edit flow still needs a higher-level scope model threaded through the loaders and search APIs.

## Current State

- `crates/ploke-db/src/multi_embedding/db_ext.rs:1169` loads one backup by `crate_name`, restores one active embedding set, and creates one HNSW index for that set.
- `crates/ploke-db/src/database.rs:187`, `:903`, `:1029`, `:1402`, `:1619` keeps a single `active_embedding_set` on `Database` and routes most embedding operations through it.
- `crates/ploke-db/src/index/hnsw.rs:28` and `crates/ploke-db/src/multi_embedding/hnsw_ext.rs:49` are already set up for a specific embedding set, but the entry points still select that set globally.
- `crates/ploke-db/src/bm25_index/mod.rs:518` and `:699` rebuild BM25 from all primary relations in the DB; `crates/ploke-db/src/bm25_index/bm25_service.rs` keeps a single actor-backed index.
- `crates/ploke-rag/src/core/mod.rs:234`, `:489`, `:547`, `:656` performs BM25, dense, hybrid, and context assembly without a crate/workspace filter.
- `crates/ploke-rag/src/context/mod.rs:172` assembles context from IDs, then resolves file/snippet data, but it assumes the search layer already produced the right scope.
- `crates/ploke-db/src/helpers.rs` and `crates/ploke-db/src/get_by_id/mod.rs` already resolve exact nodes by file/module path, which is useful for edit-time disambiguation.

## Missing Pieces

- A workspace-level ingestion orchestrator that enumerates crates in a workspace and applies parse/import/embed/index per crate, rather than calling `load_db` once for a single crate.
- Scope-aware DB search helpers that can restrict dense and BM25 retrieval to a crate or workspace slice instead of always using the current global active set.
- Workspace-scoped BM25 rebuild/index management, including a way to rebuild or maintain sparse indexes per workspace or per loaded crate group.
- A workspace registry or manifest mapping crate names to backup locations and embedding-set metadata for `/save db` and `/load <workspace>`.
- Search/result plumbing that preserves scope through rerank and context assembly so workspace-wide hits do not bleed across crates with similar names.
- A corresponding edit-selection path that can validate the chosen hit against workspace/file/module scope before applying a semantic edit.

## Likely Touchpoints

- `crates/ploke-db/src/multi_embedding/db_ext.rs:1169`
- `crates/ploke-db/src/database.rs:390`, `:903`, `:976`, `:1029`, `:1402`, `:1619`
- `crates/ploke-db/src/index/hnsw.rs:28`
- `crates/ploke-db/src/multi_embedding/hnsw_ext.rs:49`
- `crates/ploke-db/src/bm25_index/mod.rs:518`, `:699`
- `crates/ploke-db/src/bm25_index/bm25_service.rs`
- `crates/ploke-db/src/query/callbacks_multi.rs:49` and `crates/ploke-db/src/query/callbacks_single.rs:55`
- `crates/ploke-rag/src/core/mod.rs:234`, `:489`, `:547`, `:656`
- `crates/ploke-rag/src/context/mod.rs:172`
- `crates/ploke-db/src/helpers.rs`

## Open Risks

- The biggest correctness risk is widening scope by accident: once multiple crates share one loaded DB, a global active embedding set can point dense search, BM25, and callbacks at the wrong relation.
- Backup/import code should not silently tolerate missing relations or schema drift; the current fixture and restore behavior is intentionally strict.
- Workspace-wide semantic edit will need exact file/module validation to avoid symbol collisions across crates with the same names.
- BM25 persistence is still only a sidecar; treating it as workspace-scoped will need an explicit lifecycle decision, not an implicit reuse of the single-actor state.
