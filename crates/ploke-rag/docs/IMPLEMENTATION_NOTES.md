# ploke-rag — Crucial implementation notes (extracted from READMEs & AI_NOTES)

Purpose
- This document collects the *crucial* facts from the various README.md and AI_NOTES.md files across the workspace that are directly relevant to implementing the `ploke-rag` plan end-to-end (fusion, context assembly, BM25 lifecycle, persistence, observability, and tests).
- Use this as the single source of truth for design decisions that cross crate boundaries.

1) Actor / concurrency model (overall)
- ploke crates use a mixture of Tokio (async actors) and Rayon (parallel CPU work).
- ploke-db BM25 is implemented as an actor that receives `Bm25Cmd` messages via mpsc and replies via oneshot channels.
- All calls to BM25 should be done with an async-aware timeout (tokio::time::timeout) on the caller side; the actor itself runs on the tokio domain and is CPU-local.
- Retries/backoff must use tokio::time::sleep (non-blocking).

2) BM25 actor: commands & semantics (what ploke-rag depends on)
- Commands available (ploke-db `bm25_service`):
  - Search { query: String, top_k: usize, resp: oneshot::Sender<Vec<(Uuid, f32)>> }
  - IndexBatch { docs: Vec<DocData> }
  - Remove { ids: Vec<Uuid> }
  - Rebuild
  - FinalizeSeed { resp: oneshot::Sender<Result<(), String>> }
  - Status { resp: oneshot::Sender<Result<Bm25Status, DbError>> }
  - Save { path: PathBuf, resp: oneshot::Sender<Result<(), DbError>> }
  - Load { path: PathBuf, resp: oneshot::Sender<Result<(), DbError>> }
- Bm25Status enum values:
  - Uninitialized
  - Building
  - Ready { docs: usize }  // docs is the number of documents in in-memory scorer
  - Empty
  - Error(String)
- Behavioral guarantees:
  - Search is non-blocking and returns Vec<(Uuid, f32)>. It may return empty if index is Empty/Uninitialized.
  - IndexBatch / Remove update in-memory state and the actor's status (Ready/Empty).
  - Rebuild sets status=Building, repopulates index from DB, and then sets Ready/Empty or Error.
  - Save writes a tiny sidecar JSON with tokenizer version and doc_count (best-effort). Load currently triggers a Rebuild to ensure usable index (file advisory).
  - Ready.docs is reliable (driven by an in-memory HashSet), so ploke-rag can rely on this for readiness checks.

3) Persistence & save/load implications
- Current persistence in ploke-db is a lightweight sidecar (version + doc_count). It's not a full scorer serialization yet.
- Load is implemented as "rebuild from DB", so callers can treat Load as a warm-start hint (but not a strict fast-path until full serialization exists).
- When we later add a durable persistence format, ploke-db will replace the rebuild-with-load semantics. Keep the API surface the same in ploke-rag.

4) Error mapping & timeouts
- Map channel (send/recv) errors to RagError::Channel.
- Map DbError::Cozo(...) and other DB-level errors from Save/Load/Rebuild to RagError::Db (preserve message).
- Set a sensible default BM25 actor RPC timeout in ploke-rag (recommend 250ms).
- For status-checks on cold start, retry up to 2 times with short backoff (50ms, 100ms) before using lenient fallback.

5) Dense retrieval & embedding shape expectations
- ploke-db exposes dense HNSW search helpers; results are returned as Vec<EmbeddingData> wrappers.
- Embedding vector dimension used in the transform schema is mentioned as 384 (see schema snippets). Expect embedding Vec<f32> lengths to be 384 unless configured otherwise.
- ploke-db `QueryResult::to_embedding_nodes()` & Database::raw_query(...) utilities will be used to hydrate Uuid -> EmbeddingData for ranking/fusion.

6) Snippet fetching & change detection
- ploke-io provides IoManagerHandle with:
  - get_snippets_batch(requests: Vec<EmbeddingData>) -> Vec<Result<String, PlokeError>>
  - scan_changes_batch(files: Vec<FileData>) -> Result<Ok(Vec<Option<FileData>>), ...>
- EmbeddingData includes file_path, start_byte, end_byte, file_tracking_hash, node_tracking_hash.
- Use file_tracking_hash / TrackingHash to deduplicate or detect stale data when assembling context. The `ploke-io` actor will return ContentMismatch errors per-request where hashes differ.

7) Canonical IDs, TrackingHash and stability
- CanonId / TrackingHash are Uuid-based identifiers and are used broadly for:
  - deduplication
  - change detection
  - stable node identity across file reorganizations (CanonId) vs file-content tracking (TrackingHash)
- Use TrackingHash of files/nodes to avoid including stale snippets in context assembly.

8) ploke-transform & schema notes (DB contract)
- Schema creation helpers exist (ploke-transform::schema::create_schema_all).
- Primary nodes (function, struct, module...) include fields: id (Uuid), name, tracking_hash, span, embedding (optional).
- When writing embedding updates, Database::update_embeddings_batch takes Vec<(Uuid, Vec<f32>)>.
- Cozo query results encode linkages (syntax_edge, module relationships) that can be used for structural importance heuristics.

9) Tokenization & budgeting considerations
- `ploke-rag` should accept an abstract TokenCounter trait (in plans); tests should use a deterministic approximate counter to avoid coupling with external tokenizers.
- Budgeting must consider reserved tokens for system prompts, and ensure deterministic trimming/stitching of snippets.
- For deterministic tests, prefer counting bytes or words (documented test adapter) versus relying on external provider tokenizer.

10) Observability & tracing
- Add structured tracing fields on all BM25-related operations:
  - bm25_status, strict (bool), attempts, timeout_ms, fallback_used (bool), bm25_results.len(), dense_results.len()
- Emit events for persistence and rebuild: persistence_started/done, rebuild_started/done/failure (with duration).
- Tests should assert presence of these fields when needed (using tracing-test utilities).

11) Fusion: score normalization and MMR hooks
- Expect to normalize BM25 scores (positive floats, higher is better) and dense similarities (distance/score) into comparable scales before RRF or MMR.
- Start with min-max normalization per modality; make normalization pluggable.
- MMR uses cosine similarity on normalized dense vectors; missing embeddings should be treated as zero vector (similarity 0).

12) Tests & golden fixtures
- Use existing fixture DB items and `create_schema_all` in tests (ploke-db tests show usage).
- Unit tests:
  - BM25 status and behavior (Ready, Empty, Uninitialized, Error).
  - Lenient fallback: when BM25 empty/unready, dense search results are returned and fallback_used=true.
  - Strict variant: search_bm25_strict returns error on Empty/Uninitialized.
  - Save/Load: Save writes small sidecar (assert file exists & content), Load triggers Rebuild and results in Ready state.
- Integration tests:
  - Rebuild -> Search yields meaningful results without dense fallback.
  - Persistence round-trip (once full persistence exists; for now assert Load triggers the same effect as Rebuild).

13) Backwards compatibility
- Preserve existing search_bm25() semantics (lenient) and add strict variants and persistence as opt-in methods.
- Document behavior in RagService rustdocs so consumers understand state transitions.

14) Implementation checklist for ploke-rag (practical next steps)
- Implement BM25 client wrapper in ploke-rag that:
  - exposes async bm25_status(), bm25_search(lenient), bm25_search_strict(), bm25_save(), bm25_load()
  - enforces timeouts and retry/backoff on status/search per plan
  - maps errors to RagError consistently
  - emits tracing fields listed above
- Implement score normalization utilities and RRF + MMR modules that accept bm25 + dense results and embeddings.
- Implement context assembler using IoManagerHandle for snippet fetching and TrackingHash for dedup / staleness checks.
- Add unit tests using the DB fixture and a fake/mocked BM25 actor if needed (or use ploke-db actor with MemStorage).

References (where these facts came from)
- crates/ploke-io/README.md, crates/ploke-io/AI_NOTES.md (I/O actor semantics, change detection)
- crates/ingest/ploke-embed/README.md (embedding backends, dimensions)
- crates/ingest/syn_parser/README.md (module tree, parsed graph, ids)
- crates/ingest/ploke-transform/README.md (schema, create_schema_all)
- crates/ploke-db/AI_NOTES.md and tests in crates/ploke-db/src (DB contracts, QueryResult helpers)
- The new BM25 actor changes (Bm25Status, Save/Load, behavior) implemented in ploke-db and summarized in crates/ploke-rag/bm25_notes.md

If you want, I can now:
- Create the BM25 client wrapper (async helper) inside `crates/ploke-rag` implementing timeouts/retries and strict/lenient search modes.
- Start the fusion module (score normalization + RRF) and provide unit tests that use the existing ploke-db in-memory actor.

Which should I do next?
