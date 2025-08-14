Action required to implement next BM25 steps

Summary of next 2–3 steps
1) ploke-db: Add atomic persistence helpers
   - Implement on Database:
     - upsert_bm25_doc_meta_batch(docs: Vec<(uuid::Uuid, ploke_core::TrackingHash, String /*tokenizer_version*/, usize /*token_length*/)>)
2) ploke-embed bm25_service: Wire FinalizeSeed to real persistence
   - Modify the BM25 actor to optionally hold Arc<ploke_db::Database>.
   - On FinalizeSeed:
     - drain staged metadata via indexer.drain_staged_meta()
     - persist using the new Database helpers in one transaction
     - only then ack Ok(()), else return Err(String) with context.
3) ploke-tui: Implement avgdl recalculation
   - Add functionality to recalculate avgdl from staged metadata when triggered by user messages
   - Store avgdl in persistent system data file
   - Trigger recalculation before LLM response in terminal user interface

Is current code sufficient?
- Not yet. We need to edit the following file to add the required Database helpers:
  - crates/ploke-db/src/database.rs  ← Please add this file to the chat.
- Optionally, share the file where bm25_service::start is invoked so we can switch the call site to start_with_db and thread the Database handle into the BM25 actor.

Please add to chat
- crates/ploke-db/src/database.rs
- (Optional) The file that starts the BM25 actor (call site of bm25_service::start or start_default)

Status Update (2025-08-15):
- ✅ Step 1 completed: ploke-db Database helpers implemented
  - Added upsert_bm25_doc_meta_batch
- ✅ Step 2 completed: bm25_service updated with real persistence
  - BM25 actor now optionally holds Arc<Database>
  - FinalizeSeed drains staged metadata and persists atomically
- ✅ Integration completed: IndexerTask properly wired to BM25 service
  - Sends DocMeta during process_batch
  - Awaits FinalizeSeed acknowledgment before marking completion

Next steps:
- Add integration tests to verify end-to-end BM25 indexing and persistence
- Implement BM25 search functionality in the RAG module
- Add CLI commands for BM25 rebuild and hybrid search in TUI
- Implement avgdl recalculation triggered by user messages in TUI
