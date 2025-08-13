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
  - In IndexerTask::process_batch, send the same snippets to the BM25 service (at the // AI: marker).
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

2) Wire IndexerTask to BM25 service in crates/ingest/ploke-embed/src/indexer/mod.rs
   - Add a new field to IndexerTask: bm25_tx: Option<mpsc::Sender<Bm25Cmd>>.
   - In process_batch, right after valid_snippets are built (at the // comment), send docs to BM25:
     - Build docs: Vec<(Uuid, String)> by pairing valid_data ids and valid_snippets.
     - Clone snippets for BM25; keep dense path unchanged.
     - Log and non-fatal on send failure.

   Brief code changes:
   - struct IndexerTask { … bm25_tx: Option<mpsc::Sender<Bm25Cmd>>, … }
   - IndexerTask::new(..., bm25_tx: Option<mpsc::Sender<Bm25Cmd>>) -> Self { … }
   - In process_batch:
     - let docs: Vec<(Uuid, String)> = valid_data.iter().map(|d| d.id).zip(valid_snippets.clone().into_iter()).collect();
     - if let Some(tx) = &self.bm25_tx { let _ = tx.try_send(Bm25Cmd::IndexBatch { docs, tokenizer_version: "code_tokenizer_v1".into() }); }

3) Two-pass initial BM25 build (service)
   - Pass 1: stream all active primary nodes (NOW), retrieve snippets via IoManagerHandle, compute token_length with CodeTokenizer::count_tokens_in_code, upsert bm25_doc_meta with token_length and tracking_hash, accumulate sum_dl and doc_count → avgdl.
   - Pass 2: rebuild Bm25Indexer with that avgdl and index all docs.

4) Stable hash for metadata
   - Replace DefaultHasher in index_batch_with_cozo with a stable hash:
     - Prefer inserting tracking_hash from DB if available.
     - If not, compute blake3(snippet) and store u64 or full digest (choose one and keep consistent).
   - If you keep the trait, change CozoClient::upsert_doc_meta to take &self and return Result.

5) Database and schema
   - ploke-transform: define bm25_doc_meta schema and call create_and_insert in schema setup.
   - ploke-db: implement:
     - upsert_bm25_doc_meta(id: Uuid, token_length: usize, tokenizer_version: &str) -> Result<(), DbError>
     - previously tracking_hash was kept in upsert_bm25_doc_meta, but will instead remain as a field in the original nodes to avoid duplication issues.
     - avg_bm25_doc_length_now() -> Result<f32, DbError>
     - stream_active_primary_nodes_now(...) -> iterator/stream or paginated fetch returning ids + spans + file info.
     - prune hooks: when nodes are redacted/removed, emit Remove to BM25 service.

6) Hybrid search orchestration (ploke-rag)
   - Implement RagService::search_hybrid(query: &str, k: usize):
     - Dense: current HNSW search via Cozo per-type or unified.
     - Sparse: send Search to BM25 actor.
     - Fuse with RRF (k=60 commonly used).
     - Expand neighbors/containers/children via Cozo; apply hop decay.
     - Optional: rerank top 30–50 with cross-encoder; else use heuristic reranker.
     - Pack snippets to token budget and return.

7) Token counting and pricing
   - Add a tokenization service (tiktoken-rs/tokenizers) to count:
     - user message tokens
     - retrieved context tokens
     - model outputs
   - Track per-call cost and log.

Progress checklist (to be updated as we go)
- [ ] ploke-transform: add bm25_doc_meta schema and migration hookup
- [ ] ploke-db: upsert_bm25_doc_meta + avg length + stream active nodes
- [ ] ploke-embed: BM25 actor with two-pass rebuild, IndexBatch/Remove/Search commands
- [ ] ploke-embed: IndexerTask field bm25_tx + send IndexBatch at // in process_batch
- [ ] Replace DefaultHasher with TrackingHash or blake3 in BM25 metadata
- [ ] ploke-rag: hybrid search (dense + BM25) with RRF
- [ ] ploke-rag: graph expansion queries and hop decay
- [ ] Reranker: cross-encoder hook (Candle or external), integrate on top 30–50
- [ ] Token budget packer and pricing tracker
- [ ] TUI: commands to rebuild BM25, run hybrid search, display costs
- [ ] Periodic BM25 full rebuild trigger when avgdl drift exceeds threshold

Notes and definitions
- Upsert = “update if exists; insert if not.”
- RRF (Reciprocal Rank Fusion): For each result list, an item at rank r contributes 1/(k+r), k≈60, summed across lists; sort by the sum.

Open choices to confirm (defaults in parentheses)
- Enrich BM25 text with names/paths/signatures/doc-comments now? (yes)
- Reranker source: local Candle cross-encoder vs temporary external endpoint? (start external if easier, switch to local)
- Avgdl drift threshold to trigger rebuild? (2–5%)

How to resume after context reset
- Recreate the BM25 actor scaffolding and the IndexerTask send point at // in process_batch.
- Follow the checklist top-down; I will keep it updated and call out any blocking questions or schema changes as they arise.
