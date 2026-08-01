# `ploke-embed`

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


`ploke-embed` owns embedding providers, the active embedding-set runtime, and the background indexing task that fills vector/BM25 retrieval data.

## Responsibilities

- Abstract multiple embedding backends behind `EmbeddingProcessor`.
- Track the active `EmbeddingSet` and active processor through `EmbeddingRuntime`.
- Atomically activate new embedding sets by updating database metadata, vector relations, and runtime handles.
- Batch, cancel, and dispatch embedding requests for code snippets and query text.
- Run `IndexerTask` to fetch unembedded graph nodes, obtain snippets, generate embeddings, and persist vector rows/BM25 docs.

## Key Files

- [`crates/ingest/ploke-embed/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/ploke-embed/src/lib.rs) — crate module boundary.
- [`crates/ingest/ploke-embed/src/runtime.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/ploke-embed/src/runtime.rs) — `EmbeddingRuntime`, active-set hot swap, shared handles.
- [`crates/ingest/ploke-embed/src/indexer/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/ploke-embed/src/indexer/mod.rs) — `EmbeddingProcessor`, backends, `IndexerTask`, progress/status types.
- [`crates/ingest/ploke-embed/src/providers/openrouter.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/ploke-embed/src/providers/openrouter.rs) — OpenRouter embedding backend.
- [`crates/ingest/ploke-embed/src/cancel_token.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/ploke-embed/src/cancel_token.rs) — cancellation handle/listener used by indexing and remote embedding calls.

## Public API

- `EmbeddingRuntime::with_default_set`, `new`, `from_shared_set` — construct runtime handles.
- `EmbeddingRuntime::activate(db, new_set, new_embedder)` — creates/persists vector relation metadata and swaps active runtime state.
- `EmbeddingRuntime::generate_embeddings` and `generate_embeddings_with_cancel` — delegate to the active processor.
- `EmbeddingProcessor::new`, `new_mock`, `generate_embeddings_with_cancel`, `dimensions`, `snippet_batch_size`.
- `IndexerTask::new`, `with_bm25_tx`, status/progress types such as `IndexingStatus`, `IndexStatus`, and `IndexerCommand`.

## Internal Structure

`EmbeddingRuntime` holds two `Arc<RwLock<_>>` values: one for the active `EmbeddingSet`, and one for the active `Arc<EmbeddingProcessor>`. This lets `Database` and RAG/indexing code observe the same active embedding identity. `EmbeddingProcessor` is a small dispatcher over local, HuggingFace, OpenAI, OpenRouter, and mock/Cozo-style backends. `IndexerTask` owns DB, I/O, runtime, cancellation, optional BM25 command sender, per-node-type cursors, and processed counters.

## Dependencies

- **Uses:** `ploke-db`, `ploke-io`, `ploke-core`, `tokio`, remote provider clients, local embedding backend.
- **Used by:** `ploke-tui` runtime startup/index commands and `ploke-rag` dense query embedding.

## Notable Patterns / Gotchas

- Activation must update database schema/metadata before exposing the new runtime active set.
- The indexer keeps the cancellation channel sender alive; dropping it would make listeners complete and incorrectly cancel remote embedding requests.
- Batch size is backend-specific and can be derived from `EmbeddingRuntime::snippet_batch_size` unless a task override is supplied.
