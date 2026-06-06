# `ploke-db`

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


`ploke-db` is the Cozo-backed graph and retrieval database wrapper. It initializes schema, stores transformed code graph relations, manages embedding-set/vector metadata, exposes HNSW dense search helpers, and hosts BM25 indexing support.

## Responsibilities

- Initialize and wrap an in-memory Cozo database with Ploke schema.
- Provide typed query helpers, result formatting, and raw query escape hatches.
- Manage active embedding-set metadata and vector relations across model swaps and backup restores.
- Expose HNSW search and typed embedding result data for dense retrieval.
- Start and support BM25 services for sparse retrieval.
- Import/export/remove namespaces and report conflict/invalidation metadata.

## Key Files

- [`crates/ploke-db/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-db/src/lib.rs) — public exports and crate module boundary.
- [`crates/ploke-db/src/database.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-db/src/database.rs) — `Database`, schema initialization, queries, namespace import/export, snippet context lookup.
- [`crates/ploke-db/src/multi_embedding/db_ext.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-db/src/multi_embedding/db_ext.rs) — embedding-set metadata and vector relation helpers.
- [`crates/ploke-db/src/multi_embedding/hnsw_ext.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-db/src/multi_embedding/hnsw_ext.rs) — HNSW extension helpers.
- [`crates/ploke-db/src/bm25_index/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-db/src/bm25_index/mod.rs) — BM25 tokenizer/index/service module boundary.
- [`crates/ploke-db/src/get_by_id/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-db/src/get_by_id/mod.rs) — snippet/path lookup helpers used by RAG.

## Public API

- `Database::init_with_schema()` — creates a Cozo `MemStorage` database and initializes schema.
- `Database` derefs to `Db<MemStorage>` for lower-level Cozo operations.
- `ImmutQuery::raw_query` — immutable raw Cozo queries.
- `get_snippet_context_nodes_ordered(nodes)` — strict ordered lookup for snippet metadata and path projection used by context assembly.
- `setup_multi_embedding`, `put_embedding_set`, `ensure_vector_embedding_relation`, `set_active_set`, `restore_embedding_set` through `EmbeddingExt`.
- HNSW functions re-exported from `index::hnsw`, including `search_similar_args` and `create_index_for_set`.
- BM25 service commands/status under `bm25_index::bm25_service`.

## Internal Structure

The central type is `Database { db: Db<MemStorage>, active_embedding_set: Arc<RwLock<EmbeddingSet>> }`. The active set is shared with `EmbeddingRuntime` so database writes/searches and embedder identity can move together. `database.rs` also contains namespace inventory/export/import result types, conflict reports, and backup/restore embedding-set selection logic.

## Dependencies

- **Uses:** `cozo`, `ploke-transform` schema definitions, `syn_parser` node IDs/types, `ploke-core`, `ploke-error`, `itertools`, `uuid`.
- **Used by:** `ploke-tui`, `ploke-embed`, `ploke-rag`, tests/fixtures, and any code graph query surface.

## Notable Patterns / Gotchas

- `ACTIVE_EMBEDDING_SET_REL` and vector relation names are part of the multi-embedding contract; changing them affects backup/import and retrieval.
- Backup fixture handling should stay strict. Do not silently tolerate schema drift in backup fixtures without explicit approval.
- `get_snippet_context_nodes_ordered` intentionally supports sparse retrieval results that may not have dense embedding rows yet.
