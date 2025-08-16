# BM25 integration notes for ploke-rag

This document summarizes the BM25 actor semantics implemented in ploke-db for use by ploke-rag.

Actor commands
- Status { resp } -> Result<Bm25Status, DbError>
  - Returns one of: Uninitialized, Building, Ready { docs }, Empty, Error(String).
  - Ready.docs is driven by a HashSet of indexed ids in-memory and is accurate after IndexBatch/Remove/Rebuild.
- Search { query, top_k, resp } -> Vec<(Uuid, f32)>
  - Non-blocking; returns zero results if index is Empty/Uninitialized.
- IndexBatch { docs } / Remove { ids }
  - Mutate in-memory state and update status to Ready/Empty accordingly. Upserts also write BM25 doc-meta to Cozo.
- Rebuild
  - Sets status=Building, rebuilds from DB, then sets Ready{docs}/Empty or Error on failure.
- Save { path } / Load { path }
  - Save writes a small JSON sidecar with tokenizer version + doc_count (no scorer serialization yet).
  - Load currently triggers a Rebuild from DB to ensure a usable index; file is advisory for now.

Operational guidance for ploke-rag
- Lenient fallback (default)
  - If Search returns empty and Status != Ready, perform dense search and set fallback_used=true in tracing.
  - On Status=Building/Uninitialized, optionally retry Search with short backoff (e.g., 50ms/100ms) up to 2 attempts.
- Strict variant
  - If Search returns empty and Status is not Ready or Ready{docs:0} (Empty), return an error to the caller (no dense fallback).
- Persistence
  - bm25_save(path) and bm25_load(path) are available as call-throughs:
    - Save is best-effort and cheap.
    - Load ensures a usable index by internally rebuilding from DB; when a persistent format is added, Load will short-circuit.
- Tracing fields to include in ploke-rag
  - bm25_status, strict, attempts, timeout_ms (client-side), fallback_used, bm25_results, dense_results.
- Timeouts
  - The actor itself is CPU-local and generally fast; enforce request timeouts in ploke-rag using tokio::time::timeout (e.g., 250ms default).
- Error mapping
  - Channel send/recv -> RagError::Channel
  - DbError::Cozo(...) from Save/Load/Rebuild is surfaced -> RagError::Db via From.

Future improvements hook points
- When persistence format is implemented in ploke-db, Load will reconstruct scorer/embedder from disk and avoid DB rebuild.
- ploke-rag may introduce a background warm-up path that calls Rebuild once at service start and uses Status for readiness gating.
