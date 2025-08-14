<!-- Reason: Reflect decision to stage changes and use borrowed &str snippets.
     Also document EmbeddingData fields and consequences for Bm25Indexer and process_batch.
     Updated to reflect new design where avgdl is stored in persistent system data file. -->
Condensed essentials and immediate plan for BM25 + hybrid retrieval

What we're building
- Add an in-memory BM25 indexer to complement dense HNSW for hybrid retrieval.
- Keep BM25 postings in memory; persist only small per-doc metadata (token_length, tracking_hash, tokenizer_version) in Cozo.
- Integrate BM25 indexing alongside the existing dense embedding IndexerTask, driven by the same snippet batches.
- Provide a hybrid search that fuses BM25 and dense results, expands via the code graph, reranks, and packs to a token budget.

Key decisions
- Index scope: Start with primary nodes only (modules, impls, functions, structs, enums…). Do not index methods/params yet to avoid duplication and noise. Revisit after baseline.
- Tokenizer: Use the code-aware tokenizer (already implemented) and store tokenizer_version in metadata so we can detect when to rebuild. The tokenizer_version is now a constant field on Bm25Indexer.
- Avgdl: Store avgdl in a persistent system data file outside the database rather than in the database. This allows us to keep the value accurate to the items in the database and avoid drift. Recalculate occasionally, triggered by user messages in the TUI before LLM response.
- Hash stability: Stop using DefaultHasher for snippet hashes. Use existing TrackingHash (preferred) or blake3 so values are stable across runs.
- Persistence: Rebuild BM25 index on startup from DB + filesystem; metadata in Cozo keeps it consistent with dense embeddings. Persisted metadata is written during Finalize.
- Fusion: Start with RRF (Reciprocal Rank Fusion) to combine BM25 and dense lists. It is robust and parameter-light. Add weighted blending later if desired.
- Reranker: Add a small cross-encoder reranker over the top 30–50 candidates after hybrid fusion. Use Candle for local inference or a simple external endpoint at first.
- Packing: Greedy pack reranked snippets into a model-specific token budget, coalescing adjacent spans per file.

Where to integrate in code (high level)
- ploke-embed
  - Add a BM25 service/actor that wraps Bm25Indexer.
  - Provide a channel-based API so IndexerTask can send IndexBatch/Remove/Rebuild requests.
  - At startup, rebuild BM25 from active primary nodes using two passes.
  - In IndexerTask::process_batch, zip valid_data (Vec<EmbeddingData>) with valid_snippets (Vec<String>) and:
      - compute token_length in process_batch (using CodeTokenizer::count_tokens_in_code),
      - send IndexBatch using DocMeta containing metadata (tracking_hash, token_length) together with Uuid from EmbeddingData (avoid cloning snippet Strings),
      - the BM25 actor will index using DocMeta but will only stage metadata (tracking_hash, token_length).
- ploke-transform
  - Add a bm25_doc_meta relation with columns: id, tracking_hash, token_length, tokenizer_version, span @ 'NOW'.
- ploke-db
  - Add upsert/get helpers for bm25_doc_meta and stream active primary nodes for BM25 build.
  - Crucially, provide atomic helpers that accept the staged metadata (without full snippet texts) for a single transactional commit in Finalize.
- ploke-rag
  - Implement hybrid_search(query) orchestration: dense search via Cozo HNSW, BM25 via service, fuse with RRF, graph expansion in Cozo, rerank, pack to budget.
- ploke-tui
  - Add commands to trigger BM25 rebuild, run hybrid search, display costs. Add token counting and pricing tracking.
  - Implement avgdl recalculation triggered by user messages before LLM response.

Immediate next steps (dev-ready)
1) BM25 service/actor in ploke-embed
   - Define enum Bm25Cmd:
     - IndexBatch { docs: Vec<(Uuid, DocMeta)> }  // note: DocMeta to avoid cloning, tokenizer_version is now a field on Bm25Indexer
     - Remove { ids: Vec<Uuid> }
     - Rebuild
     - FinalizeSeed { resp: oneshot::Sender<Result<(), String>> }
     - Search { query: String, top_k: usize, resp: oneshot::Sender<Vec<(Uuid, f32)>> }
   - Start the actor on app init; keep an mpsc::Sender<Bm25Cmd> handle.
   - The actor stages per-doc metadata (tracking_hash, token_length) but does not persist them incrementally.

2) Wire IndexerTask to BM25 service
   - Add bm25_tx: Option<mpsc::Sender<Bm25Cmd>> to IndexerTask (already present).
   - In process_batch:
     - Use valid_data (Vec<EmbeddingData>) zipped with valid_snippets (Vec<String>) to produce (EmbeddingData, &str) pairs.
     - Compute token_length for each snippet in process_batch, and build a Vec<(Uuid, DocMeta)> to send to IndexBatch.
     - Also prepare per-doc metadata (Uuid, TrackingHash, tokenizer_version, token_length) where token_length is computed locally; the BM25 actor will stage these values.
   - After the complete, successful indexing run (all nodes processed), send FinalizeSeed and wait for the actor's ack. Only after an acknowledged Finalize should the system consider BM25 metadata committed.
   - If Finalize fails, the system must fail the entire run (atomic "all nodes or none") and require a full retry or fallback rebuild.

3) ploke-db: atomic upsert for doc metadata
   - Implement Database::upsert_bm25_doc_meta_batch(docs: Vec<(Uuid, TrackingHash, String /*tokenizer_version*/, usize /*token_length*/)>)
   - Confidence: 0.85

Rebuild, drift detection, and recovery
- Add Rebuild logic to stream active primary nodes and enforce a bounded staging buffer or spill-to-disk policy.
- For very large repos prefer spill-to-disk during staging so memory usage stays bounded; ensure Finalize still composes a single transactional upsert from the staging artifact.
- Confidence: 0.6

How to resume after context reset
 - Recreate the BM25 actor scaffolding and the IndexerTask send point at // in process_batch.
 - Run the normal indexing pass; after successful completion, send FinalizeSeed and wait for its success ack. If Finalize fails, the run should be treated as failed and retried (ensures atomic "all nodes or none" persistence).

Progress update - 2025-08-13/1
 - Schema aligned: bm25_doc_meta now has fields {id, tracking_hash, tokenizer_version, token_length}, matching the design.
 - Stable hash: replaced DefaultHasher with a stable UUID v5–based tracking_hash derived from the snippet bytes; tests updated accordingly.
 - Wiring status: BM25 actor scaffolding exists and IndexerTask sends IndexBatch. Note: IndexBatch now uses DocMeta to avoid allocations; process_batch computes token lengths before sending.
 - Design update: tokenizer_version is now a constant field on Bm25Indexer rather than being passed with each IndexBatch command.

Next step
 - Implement ploke-db persistent helpers for bm25_doc_meta batch upsert to support Finalize.
 - Wire bm25_service::FinalizeSeed to call those helpers inside a single DB transaction and only acknowledge after commit.

Progress update - 2025-08-13/2
 - bm25_service: Added FinalizeSeed command and actor handling that acks success (placeholder). Update required to perform real DB commits on Finalize.
 - IndexerTask: On successful dense indexing, now sends FinalizeSeed to BM25 and awaits ack before marking Completed; fails the run on any error.
 - Hash type: bm25 DocMeta now uses TrackingHash newtype from ploke-core for tracking_hash; generation currently wraps UUID v5 into TrackingHash until full TrackingHash::generate inputs are available. Tests updated.

Progress update - 2025-08-13/3
 - ploke-db: Implemented atomic persistence helpers:
   - Database::upsert_bm25_doc_meta_batch for batch upsert of document metadata
 - bm25_service: Updated BM25 actor to perform real persistence on FinalizeSeed
 - IndexerTask: Now sends DocData (tracking_hash, token_length) to BM25 service along with of full snippets

Next step
 - Update bm25_service::FinalizeSeed to use the new Database helpers for real persistence and return detailed errors on failure.

Progress update - 2025-08-13/4
 - bm25_service: Updated FinalizeSeed implementation to drain staged metadata and persist using new Database helpers in one atomic transaction.
 - IndexerTask now properly integrates with BM25 service by sending DocMeta during process_batch and awaiting FinalizeSeed acknowledgment.

Next steps:
 - Add integration tests to verify end-to-end BM25 indexing and persistence
 - Implement BM25 search functionality in the RAG module
 - Add CLI commands for BM25 rebuild and hybrid search in TUI
 - Implement avgdl recalculation triggered by user messages in TUI
