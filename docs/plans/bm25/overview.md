Condensed essentials and immediate plan for BM25 + hybrid retrieval

What we’re building
- Add an in-memory BM25 indexer to complement dense HNSW for hybrid retrieval.
- Keep BM25 postings in memory; persist only small per-doc metadata (token_length, tracking_hash, tokenizer_version) in Cozo.
- Integrate BM25 indexing alongside the existing dense embedding IndexerTask, driven by the same snippet batches.
- Provide a hybrid search that fuses BM25 and dense results, expands via the code graph, reranks, and packs to a token budget.

Key decisions
- Index scope: Start with primary nodes only (modules, impls, functions, structs, enums…). Do not index methods/params yet to avoid duplication and noise. Revisit after baseline.
- Tokenizer: Use the code-aware tokenizer (already implemented) and store tokenizer_version in metadata so we can detect when to rebuild.
- Avgdl: Use a two-pass initial build (count tokens → compute avgdl → embed/index). For incremental updates, accept small drift and schedule periodic full rebuilds if avgdl shifts materially.
- Hash stability: Stop using DefaultHasher for snippet hashes. Use existing TrackingHash (preferred) or blake3 so values are stable across runs.
- Persistence: Rebuild BM25 index on startup from DB + filesystem; metadata in Cozo keeps it consistent with dense embeddings and supports avgdl recomputation.
- Fusion: Start with RRF (Reciprocal Rank Fusion) to combine BM25 and dense lists. It is robust and parameter-light. Add weighted blending later if desired.
- Reranker: Add a small cross-encoder reranker over the top 30–50 candidates after hybrid fusion. Use Candle for local inference or a simple external endpoint at first.
- Packing: Greedy pack reranked snippets into a model-specific token budget, coalescing adjacent spans per file.

Where to integrate in code (high level)
- ploke-embed
  - Add a BM25 service/actor that wraps Bm25Indexer and holds an Arc<Database> and an IoManagerHandle.
  - Provide a channel-based API so IndexerTask can send IndexBatch/Remove/Rebuild requests.
  - At startup, rebuild BM25 from active primary nodes using two passes.
  - In IndexerTask::process_batch, send the same snippets to the BM25 service (at the // marker).
- ploke-transform
  - Add a bm25_doc_meta relation with columns: id, tracking_hash, token_length, tokenizer_version, span @ 'NOW'.
- ploke-db
  - Add upsert/get helpers for bm25_doc_meta, stream active primary nodes for BM25 build, and helper to compute avg token length.
- ploke-rag
  - Implement hybrid_search(query) orchestration: dense search via Cozo HNSW, BM25 via service, fuse with RRF, graph expansion in Cozo, rerank, pack to budget.
- ploke-tui
  - Add commands to trigger BM25 rebuild, run hybrid search, display costs. Add token counting and pricing tracking.

Immediate next steps (dev-ready)
1) BM25 service/actor in ploke-embed
   - Define enum Bm25Cmd:
     - IndexBatch { docs: Vec<(Uuid, String)>, tokenizer_version: String }
     - Remove { ids: Vec<Uuid> }
     - Rebuild
     - Search { query: String, top_k: usize, resp: oneshot::Sender<Vec<(Uuid, f32)>> }
   - Start the actor on app init; keep an mpsc::Sender<Bm25Cmd> handle.

 2) Wire IndexerTask to BM25 service
    - Add bm25_tx: Option<mpsc::Sender<Bm25Cmd>> to IndexerTask (already planned).
    - In process_batch: construct docs Vec<(Uuid, String)> and send IndexBatch to
 actor. Do not upsert bm25 metadata from IndexerTask.
    - After the complete, successful indexing run (all nodes processed),
 IndexerTask (or the orchestrator) must send FinalizeSeed and wait for the actor's
 ack. Only after an acknowledged Finalize should the system consider BM25 metadata
 committed.
    - If Finalize fails, the system must fail the entire run (atomic "all nodes or
 none") and require a full retry or fallback rebuild.
 3) Rebuild, drift detection, and recovery
    - Persisted metadata allows avgdl to be recomputed cheaply without re-reading
 sources.
    - Periodically check avgdl drift; if drift > threshold (e.g., 2–5%), schedule
 a controlled rebuild.
    - For very large repos where in-memory buffering is infeasible, Option A:
 spill to disk while seeding and commit from the staged file during Finalize;
 Option B: abort seeding and run the controlled two-pass rebuild. Both approaches
 preserve the atomic commit principle (no partial per-doc upserts).
 4) Stable hash for metadata
    - Replace DefaultHasher uses with a stable scheme (TrackingHash or blake3).
    - Store stable tracking_hash in bm25_doc_meta during Finalize.
 5) Database and schema
    - ploke-transform already defines bm25_doc_meta; ensure ploke-db exposes an
 atomic batch upsert and avgdl helpers.
    - Use transactions or a swap/rename pattern in the DB to make the Finalize
 commit atomic and observable.
 6) Hybrid search orchestration (ploke-rag)
    - As before: dense + BM25 + RRF + optional reranker + pack to budget.
 7) Token counting, telemetry and testing
    - Track buffer sizes, staging file usage, Finalize durations, and rebuild
 counts.
    - Test small repo seed path (in-memory buffer), large-repo spill-to-disk path,
 Finalize failure and retry behavior.

 Progress checklist (updated)
 - [x] ploke-transform: add bm25_doc_meta schema and migration hookup
 - [ ] ploke-db: atomic upsert_bm25_doc_meta_batch + avg length + stream active
 nodes
 - [ ] ploke-embed: BM25 actor with seeded single-pass build,
 IndexBatch/Remove/Finalize/Search commands
 - [ ] ploke-embed: IndexerTask field bm25_tx + send IndexBatch at // in
 process_batch; orchestrator to send Finalize on success
 - [ ] Replace DefaultHasher with TrackingHash or blake3 in BM25 metadata
 - [ ] ploke-rag: hybrid search (dense + BM25) with RRF
 - [ ] ploke-rag: graph expansion queries and hop decay
 - [ ] Reranker: cross-encoder hook (Candle or external), integrate on top 30–50
 - [ ] Token budget packer and pricing tracker
 - [ ] TUI: commands to rebuild BM25, run hybrid search, display costs
 - [ ] Periodic BM25 full rebuild trigger when avgdl drift exceeds threshold

 Notes and definitions
 - Upsert = "update if exists; insert if not."
 - Finalize = "atomic commit point for bm25 metadata and index construction; no
 per-doc persistence allowed before this."
 - RRF (Reciprocal Rank Fusion): For each result list, an item at rank r
 contributes 1/(k+r), k≈60, summed across lists; sort by the sum.

 Open choices to confirm (defaults in parentheses)
 - Enrich BM25 text with names/paths/signatures/doc-comments now? (yes)
 - Reranker source: local Candle cross-encoder vs temporary external endpoint?
 (start external if easier, switch to local)
 - Avgdl drift threshold to trigger rebuild? (2–5%)
 - Default buffer policy: in-memory up to 10000 docs or 100 MiB of snippet text;
 then spill-to-disk. (configurable)

 How to resume after context reset
 - Recreate the BM25 actor scaffolding and the IndexerTask send point at // in
 process_batch.
 - Run the normal indexing pass; after successful completion, send FinalizeSeed
 and wait for its success ack. If Finalize fails, the run should be treated as
 failed and retried (ensures atomic "all nodes or none" persistence).
