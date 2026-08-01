# `ploke-rag`

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


`ploke-rag` is Ploke's retrieval and context-assembly crate. It combines BM25 sparse search, HNSW dense vector search, score fusion, optional type-context expansion, and snippet fetching into an `AssembledContext` suitable for prompts and tools.

## Responsibilities

- Start and call the BM25 actor with timeouts, retries, and strict/lenient modes.
- Generate dense query embeddings through `EmbeddingRuntime` and search the DB HNSW indexes.
- Fuse sparse/dense result lists with weighted Reciprocal Rank Fusion and optional MMR.
- Fetch snippet text through `ploke-io`, then deduplicate, order, trim, and package context parts under a token budget.
- Provide observability fields and deterministic tie-breaking for tests/operators.

## Key Files

- [`crates/ploke-rag/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-rag/src/lib.rs) — public API, crate docs, re-exports.
- [`crates/ploke-rag/src/core/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-rag/src/core/mod.rs) — `RagService`, `RagConfig`, `RetrievalStrategy`, BM25/dense/hybrid orchestration.
- [`crates/ploke-rag/src/fusion/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-rag/src/fusion/mod.rs) — `ScoreNorm`, `RrfConfig`, `MmrConfig`, `normalize_scores`, `rrf_fuse`, `mmr_select`.
- [`crates/ploke-rag/src/context/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-rag/src/context/mod.rs) — `TokenBudget`, `AssemblyPolicy`, `TokenCounter`, `assemble_context`.
- [`crates/ploke-rag/src/error.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-rag/src/error.rs) — retrieval/context error type.

## Public API

- `RagService::new`, `new_with_config`, `new_with_io`, `new_full`, `new_rebuilt`, `new_mock`.
- `RagService::search_bm25`, `search_bm25_strict`, `bm25_status`, `search` for dense retrieval, `hybrid_search`, and `get_context`.
- `RetrievalStrategy::{Dense, Sparse, Hybrid}`.
- Fusion helpers: `normalize_scores`, `rrf_fuse`, `mmr_select`.
- Context helpers: `TokenBudget`, `AssemblyPolicy`, `ApproxCharTokenizer`, `assemble_context`, `assemble_context_with_type_context`.

## Internal Structure

`RagService` holds `Arc<Database>`, `Arc<EmbeddingRuntime>`, an `mpsc::Sender<Bm25Cmd>`, config, and optional `IoManagerHandle`. Dense search generates one query embedding, searches each primary/associated node type through `search_similar_args`, converts distances to similarity scores, and truncates to `top_k`. Hybrid search runs BM25 and dense search concurrently, fuses results with RRF, optionally applies MMR, and then context assembly performs snippet lookup and budgeting.

## Dependencies

- **Uses:** `ploke-db`, `ploke-embed`, `ploke-io`, `ploke-core`, Tokio channels/timeouts.
- **Used by:** `ploke-tui` RAG commands, chat prompt/context construction, and code-context tools.

## Notable Patterns / Gotchas

- Lenient BM25 search can fall back to dense search when BM25 is uninitialized/building/empty; strict mode returns errors for those states.
- Context assembly fetches snippets by DB metadata plus I/O actor; it does not trust a raw file path without DB hash/span context.
- The approximate tokenizer is deterministic and simple (`ceil(chars / 4)`) by design; replace it only when downstream token accounting needs real tokenizer parity.
