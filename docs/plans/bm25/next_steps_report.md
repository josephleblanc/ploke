<!-- Reason: Update plan to stage all BM25 doc changes and avoid incremental DB upserts.
     - Explain the EmbeddingData fields and the new approach to avoid cloning snippet Strings.
     - Note required API changes: IndexBatch will carry borrowed &str snippets; process_batch will compute token lengths.
-->
System/subsystem: BM25 sparse indexing and hybrid retrieval pipeline (ploke-embed actor + ploke-db indexer)

Rubric (6 categories)
- Correctness and atomicity: Ensure per-doc metadata and avgdl are computed correctly and committed in an all-or-nothing Finalize. All per-node metadata upserts will be performed in a single atomic batch during Finalize (no incremental per-doc upserts from the BM25 actor).
- Concurrency and backpressure: Actor must handle IndexBatch bursts without unbounded growth; Finalize must quiesce producers. Staging reduces DB contention during IndexBatch bursts.
- Persistence integration: Use ploke-db helpers to perform a single atomic upsert for bm25_doc_meta (batch) and persist avgdl in one transactional operation.
- Observability and error reporting: Structured logs and error propagation paths for IndexBatch/Finalize/Search. Finalize must return rich errors on DB failure so the orchestrator can fail the run.
- Performance and memory: Tokenization and staging avoid unnecessary allocations; token length is computed once in process_batch and primitive metadata is sent to BM25 actor; bounded staging with spill policy later.
- API ergonomics and extensibility: Command API is minimal but supports evolution (Finalize, Rebuild, Remove, Search). Bm25Indexer APIs are changed to accept borrowed snippet references (&str) to minimize copying.

What was implemented in this patch
- ploke-db Bm25Indexer now stages per-doc metadata (tracking_hash, token_length) while indexing in memory.
- Bm25Indexer exposes compute_avgdl_from_staged and drain_staged_meta for Finalize.
- ploke-embed bm25_service FinalizeSeed now computes avgdl and drains staged metadata (ready for DB persistence).
- Fixed a schema comment typo that would break compilation in ploke-transform/schema/meta.rs.

Key design change (this update)
- We will no longer upsert snippets or per-node metadata incrementally into the database from the BM25 actor.
  Instead, the actor will stage all changes (tracking_hash, token_length) and on FinalizeSeed the actor
  will hand the entire staged set to ploke-db for a single atomic batch upsert and a single avgdl persistence. This makes the
  Finalize operation the single source of truth and preserves an "all-or-nothing" commit boundary.
- The tokenizer_version is now a constant field on the Bm25Indexer struct rather than being passed with each IndexBatch command.

EmbeddingData structure (relevant to integration)
- Note: valid_data in indexer/mod.rs is Vec<EmbeddingData>, with fields:
  ```rust
  #[derive(Debug, Clone)]
  pub struct EmbeddingData {
      pub id: Uuid,
      pub name: String,
      pub file_path: PathBuf,
      pub file_tracking_hash: TrackingHash,
      pub start_byte: usize,
      pub end_byte: usize,
      pub node_tracking_hash: TrackingHash,
      pub namespace: Uuid
  }
  ```
- Use these fields (especially `id` and `node_tracking_hash`) instead of cloning Strings from valid_snippets when constructing IndexBatch payloads.

Performance tradeoff and flow change
- Rather than cloning a String from valid_snippets to send owned Strings through IndexBatch, process_batch will:
  1) Zip valid_data (EmbeddingData) with valid_snippets (Vec<String>) to produce (EmbeddingData, &str) pairs.
  2) Compute token length for each snippet in process_batch (using CodeTokenizer::count_tokens_in_code).
  3) Send IndexBatch using DocMeta containing only metadata (tracking_hash, token_length) where Bm25Indexer methods accept DocMeta. Because process_batch is already bottlenecked by dense embedding computation, the cost of token counting here is an acceptable tradeoff for removing allocations and copies.
- The BM25 actor will continue staging metadata (tracking_hash, token_length), but will no longer attempt to persist snippets or metadata incrementally. FinalizeSeed will:
  - compute avgdl from staged metadata (or accept an avgdl computed upstream),
  - drain the staged metadata into a Vec<(Uuid, TrackingHash, String /*tokenizer_version*/, usize /*token_length*/)> (note tokenizer_version is now a constant field on Bm25Indexer),
  - call ploke-db::Database::upsert_bm25_doc_meta_batch(...) and ploke-db::Database::set_bm25_avgdl(avgdl) inside a single DB transaction (atomic),
  - only then acknowledge success to the orchestrator.

Next 2–3 steps with confidence and file needs
1) ploke-db: Add atomic upsert_bm25_doc_meta_batch and avgdl persistence helpers
   - Implement Database::upsert_bm25_doc_meta_batch(docs: Vec<(Uuid, TrackingHash, String /*tokenizer_version*/, usize /*token_length*/)>)
     - The helper must perform batch upsert in a single transaction and be tolerant of existing rows (upsert semantics).
   - Implement Database::set_bm25_avgdl(avgdl: f32) and provide a wrapper that can commit both doc meta and avgdl together atomically.
   - Confidence: 0.85
   - Needs access to: crates/ploke-db/src/database.rs (I will edit this to implement the helpers once you confirm)

2) ploke-embed bm25_service: Wire real persistence into FinalizeSeed
   - Modify bm25_service::start to accept an Arc<Database> (or closure) and, on FinalizeSeed, prepare the drained metadata and call the new ploke-db helpers.
   - The actor should only send an Ok(()) response when the DB transaction succeeds; any DB error should be sent back as Err(String) with context so IndexerTask can fail the run.
   - Ensure FinalizeSeed quiesces producers or drains further staged docs before committing so the staged set is consistent.
   - Confidence: 0.75
   - Files to edit (already available): crates/ingest/ploke-embed/src/indexer/mod.rs

3) IndexerTask/process_batch update and Bm25Indexer API change
   - In process_batch:
     - Zip valid_data (Vec<EmbeddingData>) with valid_snippets (Vec<String>), compute token_length per snippet, and create a docs list prepared for the BM25 actor using DocMeta.
     - Send IndexBatch with docs: Vec<(Uuid, DocMeta)>.
   - Update Bm25Indexer methods to accept DocMeta instead of &str for indexing, e.g.:
     - pub fn index_batch(&mut self, batch: Vec<(Uuid, DocMeta)>)
     - pub fn stage_doc_meta(&mut self, id: Uuid, meta: DocMeta)
   - Also ensure any tests and callers are updated to create owned Strings only where necessary (e.g., for persistence during Finalize the actor will own small tokenizer_version Strings; snippet text will be borrowed for indexing only).
   - Confidence: 0.8
   - Files to edit: crates/ingest/ploke-embed/src/indexer/mod.rs, crates/ingest/ploke-db/src/bm25_index/mod.rs (tests), and ploke-db persistence helpers.

Notes
- Tokenizer version is now a constant field on Bm25Indexer; when persisting, include this in the upsert batch.
- The change to pass DocMeta to Bm25Indexer avoids an allocation per snippet at the cost of computing token_length earlier; because embedding generation is the bottleneck, this is a good tradeoff.
- Finalize must be the single point where metadata is durably persisted so the system can reason about avgdl and consistent metadata snapshots.
