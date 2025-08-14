Action required to implement next BM25 steps

Summary of next 2–3 steps
1) ploke-db: Add atomic persistence helpers
   - Implement on Database:
     - upsert_bm25_doc_meta_batch(docs: Vec<(uuid::Uuid, ploke_core::TrackingHash, String /*tokenizer_version*/, usize /*token_length*/)>)
     - set_bm25_avgdl(avgdl: f32)
     - Prefer a single transactional helper that commits both doc meta and avgdl atomically.
2) ploke-embed bm25_service: Wire FinalizeSeed to real persistence
   - Modify the BM25 actor to optionally hold Arc<ploke_db::Database>.
   - On FinalizeSeed:
     - compute avgdl via indexer.compute_avgdl_from_staged()
     - drain staged metadata via indexer.drain_staged_meta()
     - persist using the new Database helpers in one transaction
     - only then ack Ok(()), else return Err(String) with context.
   - Add a new constructor start_with_db(avgdl: f32, db: std::sync::Arc<ploke_db::Database>) while keeping the existing start/start_default for backward compatibility.
3) Optional: expose tokenizer_version
   - You can import ploke_db::bm25_index::TOKENIZER_VERSION directly during persistence (getter not strictly needed).

Is current code sufficient?
- Not yet. We need to edit the following file to add the required Database helpers:
  - crates/ploke-db/src/database.rs  ← Please add this file to the chat.
- Optionally, share the file where bm25_service::start is invoked so we can switch the call site to start_with_db and thread the Database handle into the BM25 actor.

Please add to chat
- crates/ploke-db/src/database.rs
- (Optional) The file that starts the BM25 actor (call site of bm25_service::start or start_default)

Status Update (2025-08-15):
- ✅ Step 1 completed: ploke-db Database helpers implemented
  - Added upsert_bm25_doc_meta_batch, set_bm25_avgdl, and upsert_bm25_data_atomic
- ✅ Step 2 completed: bm25_service updated with real persistence
  - BM25 actor now optionally holds Arc<Database>
  - FinalizeSeed computes avgdl and persists metadata atomically
  - Added start_with_db constructor for database-enabled BM25 service
- ✅ Integration completed: IndexerTask properly wired to BM25 service
  - Sends DocMeta during process_batch
  - Awaits FinalizeSeed acknowledgment before marking completion

Next steps:
- Add integration tests to verify end-to-end BM25 indexing and persistence
- Implement BM25 search functionality in the RAG module
- Add CLI commands for BM25 rebuild and hybrid search in TUI
