System/subsystem: BM25 sparse indexing and hybrid retrieval pipeline (ploke-embed actor + ploke-db indexer)

Rubric (6 categories)
- Correctness and atomicity: Ensure per-doc metadata and avgdl are computed correctly and committed in an all-or-nothing Finalize.
- Concurrency and backpressure: Actor must handle IndexBatch bursts without unbounded growth; Finalize must quiesce producers.
- Persistence integration: Use ploke-db helpers to perform a single atomic upsert for bm25_doc_meta and persist avgdl.
- Observability and error reporting: Structured logs and error propagation paths for IndexBatch/Finalize/Search.
- Performance and memory: Tokenization and staging avoid excessive allocations; bounded staging with spill policy later.
- API ergonomics and extensibility: Command API is minimal but supports evolution (Finalize, Rebuild, Remove, Search).

What was implemented in this patch
- ploke-db Bm25Indexer now stages per-doc metadata (tracking_hash, token_length) while indexing.
- Bm25Indexer exposes compute_avgdl_from_staged and drain_staged_meta for Finalize.
- ploke-embed bm25_service FinalizeSeed now computes avgdl and drains staged metadata (ready for DB persistence).
- Fixed a schema comment typo that would break compilation in ploke-transform/schema/meta.rs.

Next 2–3 steps with confidence and file needs
1) ploke-db: Add atomic upsert_bm25_doc_meta_batch and avgdl persistence helpers
   - Implement Database::upsert_bm25_doc_meta_batch(docs: Vec<(Uuid, TrackingHash, String /*tokenizer_version*/, usize /*token_length*/)>)
   - Implement Database::set_bm25_avgdl(avgdl: f32) or store in a config/meta relation.
   - Confidence: 0.8
   - Needs access to: crates/ploke-db/src/database.rs (please add this file to the chat so I can edit it)

2) ploke-embed bm25_service: Wire real persistence into FinalizeSeed
   - Pass an Arc<Database> into bm25_service::start(...) and, on FinalizeSeed, call the new ploke-db helpers with drained staged metadata and tokenizer_version="code_tokenizer_v1".
   - Handle and surface DB errors; only ack success on full commit; otherwise send Err with context.
   - Confidence: 0.7
   - Files to edit (already available): crates/ingest/ploke-embed/src/indexer/mod.rs
   - Also requires step (1) to be implemented.

3) Rebuild flow and backpressure guard
   - Add Rebuild logic to stream active primary nodes, compute avgdl in two passes when needed, and enforce a bounded staging buffer or spill-to-disk policy.
   - Confidence: 0.6
   - Likely new module/file for spill buffer configuration; may need read accessors in ploke-db to stream active nodes.

Notes
- Tokenizer version is currently fixed to "code_tokenizer_v1" at the send site; when persisting, include this in the upsert batch.
- Once DB helpers exist, FinalizeSeed should be updated to perform a single atomic commit and only then clear staged buffers.
