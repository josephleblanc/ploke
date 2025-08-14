<!-- Update: Document actual repository snapshot (commit 843d16d) and mark completed/partial steps.
     Rationale: keep the planning doc in sync with verified code paths in ploke-rag and ploke-tui.
     NOTE: This file summarizes what is observed in code added to the conversation; some components
     (ploke-embed bm25 actor internals, ploke-db commits) must still be inspected directly to verify. -->

Condensed essentials and immediate plan for BM25 + hybrid retrieval

What we're building
- Add an in-memory BM25 indexer to complement dense HNSW for hybrid retrieval.
- Keep BM25 postings in memory; persist only small per-doc metadata (token_length, tracking_hash, tokenizer_version) in Cozo.
- Integrate BM25 indexing alongside the existing dense embedding IndexerTask, driven by the same snippet batches.
- Provide a hybrid search that fuses BM25 and dense results, expands via the code graph, reranks, and packs to a token budget.

High-level summary of recent repository changes (commit 843d16d)
- ploke-rag:
  - Added RagService that holds a handle (mpsc::Sender<Bm25Cmd>) to the BM25 actor started via bm25_service::start_default.
  - Implemented search_bm25(query, top_k) that sends Bm25Cmd::Search and awaits a oneshot response.
  - Implemented bm25_rebuild() that sends Bm25Cmd::Rebuild.
  - Implemented hybrid_search(...) as a placeholder that currently delegates to BM25-only search.
  Evidence: crates/ploke-rag/src/lib.rs

- ploke-tui:
  - Added ploke-rag as a dependency and initialize RagService in try_main; store Arc<RagService> in AppState.rag when available.
  - Added StateCommand variants for Bm25Rebuild, Bm25Search { query, top_k }, and HybridSearch { query, top_k }.
  - Updated state_manager to handle the new StateCommand variants by calling RagService APIs and forwarding results to the UI as SysInfo messages.
  Evidence: crates/ploke-tui/src/lib.rs and crates/ploke-tui/src/app_state/mod.rs; Cargo.toml updated to depend on ploke-rag.

Status summary (what is verified from the code provided)
- Completed / Verified:
  - RagService construction and BM25 command senders (search_bm25, bm25_rebuild) — implemented and wired.
  - TUI wiring:
    - RagService is created in try_main and stored in AppState.rag when initialization succeeds.
    - StateCommand includes BM25 and Hybrid variants.
    - state_manager executes RagService methods and posts human-readable SysInfo messages with results or errors.
  - TUI dependency: ploke-tui/Cargo.toml references ploke-rag.

- Partially completed / Needs verification:
  - hybrid_search orchestration inside ploke-rag:
    - Present as a placeholder that calls BM25-only search. Dense search (HNSW via ploke-db), RRF fusion, graph expansion, and reranking are not implemented yet.
  - BM25 actor internals (ploke-embed/bm25_service):
    - RagService assumes existence of bm25_service::start_default and Bm25Cmd variants (Search/Rebuild), but the actor's persistence semantics (FinalizeSeed behavior, transactional upsert of bm25_doc_meta) were not present in the files inspected here. The planning doc previously stated FinalizeSeed & DB helpers exist; that must be verified by inspecting ploke-embed and ploke-db source.
  - IndexerTask → BM25 wiring:
    - The plan expects IndexerTask::process_batch to compute token lengths, prepare DocMeta, send IndexBatch commands, and await FinalizeSeed on completion. The presence and correctness of that wiring must be confirmed by reviewing ploke-embed's indexer code.

- Not started / Missing (from inspected files)
  - Dense + sparse fusion (RRF) and reranker implementation (ploke-rag).
  - End-to-end integration tests that validate FinalizeSeed atomic persistence and hybrid_search correctness.
  - Rich TUI results pane (currently results surfaced as SysInfo messages only).

Concrete mapping of "Immediate follow-ups" to current status
1) Wire TUI commands to a RAG orchestration service
   - Status: Completed (ploke-tui now depends on ploke-rag, RagService is initialized in try_main, StateCommand variants and state_manager wiring done).
   - Remaining: polish UI presentation and error events.

2) Implement hybrid_search in ploke-rag
   - Status: Partially implemented (placeholder delegating to BM25-only).
   - Remaining: call dense search (ploke-db HNSW helpers), collect BM25 results, fuse with RRF, apply optional reranker, return packed/ranked Vec<(Uuid, score)>.

3) BM25 service/actor (ploke-embed) and FinalizeSeed persistence
   - Status: Unverified in this snapshot — planning notes claim FinalizeSeed and Database::upsert_bm25_doc_meta_batch exist, but those modules were not provided here for inspection.
   - Action required: inspect ploke-embed's bm25_service implementation and ploke-db persistence helpers to confirm they:
     - Stage per-doc metadata during IndexBatch.
     - On FinalizeSeed, perform an atomic upsert of bm25_doc_meta using the Database helper and return detailed errors on failure.

4) Wire IndexerTask to BM25 for commit semantics
   - Status: Unverified — ensure process_batch computes token_length, sends DocMeta, and that IndexerTask awaits FinalizeSeed before marking indexing as completed.

5) Integration tests and CI
   - Status: Not implemented. Add e2e tests to assert persistence and retrieval behavior.

Recommended next steps (developer-ready)
- Immediate verification tasks
  1. Provide the following files so I can inspect/modify as needed:
     - crates/ploke-embed/src/bm25_service.rs (or the bm25_index/bm25_service module)
     - crates/ploke-embed/src/indexer.rs or the file containing IndexerTask::process_batch
     - crates/ploke-db/src/index/bm25_doc_meta.rs or the Database helper implementations (Database::upsert_bm25_doc_meta_batch)
     Please drop crates/ploke-tui/src/app_state/mod.rs from the conversation to free context if you need to (you previously indicated willingness to do so).
  2. If the BM25 actor FinalizeSeed is not performing a single transactional DB upsert, implement Database::upsert_bm25_doc_meta_batch and call it inside FinalizeSeed; return detailed errors so IndexerTask can fail the whole run on persist errors.

- Medium-term development
  - Implement dense+BM25 fusion (RRF), reranker, and packing logic in ploke-rag.
  - Add integration tests that run a small index, call FinalizeSeed, and assert DB state and hybrid search outputs.

Change rationale
- The file now reflects verified repository state and clearly separates "verified by reading code" from "planning notes that must be validated by inspecting other modules".
- Asking for the specific files next will let me complete the "Immediate follow-ups" by either finishing the actor persistence wiring or adjusting the TUI and RagService facades.

Suggested shell commands
```bash
git add docs/plans/bm25/overview.md && git commit -m "docs(bm25): update status after wiring Rag/TUI"
```

Request for next access
- Please add the following files to the chat (or confirm dropping crates/ploke-tui/src/app_state/mod.rs to free context):
  - crates/ploke-embed/src/bm25_service.rs
  - crates/ploke-embed/src/indexer.rs
  - crates/ploke-db/src/index/mod.rs
  Once provided I will:
  - Verify FinalizeSeed behavior and implement DB transactional upsert if needed.
  - Complete hybrid_search by integrating dense HNSW calls and RRF fusion or provide a precise plan + patch.

Immediate follow-ups (concrete steps)
 1) Wire TUI commands to a RAG orchestration service
    - Add dependency: ploke-tui -> ploke-rag
    - Initialize RagService in ploke-tui/src/lib.rs::try_main and store a handle in shared state (AppState) or via a dedicated RagHandle channel.
    - Add new StateCommand variants: Bm25Rebuild and HybridSearch { query: String, top_k: usize }.
    - On these commands, spawn tasks calling RagService::{search_bm25, hybrid_search} and forward results to UI via AppEvent::Rag or AddMessageImmediate.
    Confidence: 0.6
    Files likely needed: crates/ploke-tui/src/app_state/mod.rs, crates/ploke-tui/src/lib.rs, crates/ploke-tui/Cargo.toml, crates/ploke-rag/src/lib.rs
    rg tips:
      - rg -n "enum StateCommand" crates/ploke-tui
      - rg -n "ContextManager" crates/ploke-tui/src

 2) Implement hybrid_search in ploke-rag
    - Combine BM25 results from RagService::search_bm25 with dense results from Cozo/HNSW, fuse with RRF, and return ranked Uuids (optionally include snippet previews).
    Confidence: 0.7
    Files: crates/ploke-rag/src/lib.rs, and possibly ploke-db helpers.

 3) Display search results in TUI
    - Render a lightweight results pane or inject results as SysInfo messages (filename and score).
    Confidence: 0.9
    Files: crates/ploke-tui/src/app/mod.rs

 4) Implement BM25 rebuild pathway
    - In bm25_service, expose a Rebuild command; in RagService, call Rebuild; on ack, update UI.
    Confidence: 0.6
    Files: ploke-embed bm25_service (ask to add file), crates/ploke-rag/src/lib.rs, crates/ploke-tui/src/app_state/mod.rs

 5) Add integration tests
    - Verify that 'bm25 rebuild' stages and persists metadata and that 'hybrid <query>' returns expected documents.
    Confidence: 0.5
    Files: tests across ploke-rag and ploke-db; may need new fixtures.

Progress update - 2025-08-14/5
 - Partial step 1 implemented (safe wiring)
   - Added dependency ploke-tui -> ploke-rag
   - Introduced AppState.rag: Option<Arc<RagService>> to hold a shared handle to RAG orchestration
   - Initialized RagService in ploke-tui/src/lib.rs::try_main using Arc<IndexerTask> and Database
   - Updated RagService to accept Arc<IndexerTask> to avoid moving the task out of AppState
 - Next wiring steps
   - Add StateCommand variants: Bm25Rebuild, Bm25Search { query, top_k }, HybridSearch { query, top_k }
   - In state_manager, on those commands, call RagService methods and forward results to UI via AppEvent::Rag or SysInfo messages
 - Deferred improvements
   - Consolidate EventBus priority handling (field in AppEvent vs separate channels)
   - Add richer error propagation from RagService into UI via Error events
   - Provide thin facade for BM25 rebuild/search to decouple TUI from service protocol
