# CONCISE SUMMARY — implementer brief (for GPT-5)

Core facts (most important)
- BM25 actor (ploke-db) exposes: Search, IndexBatch, Remove, Rebuild, FinalizeSeed, Status, Save, Load.
- Bm25Status: Uninitialized | Building | Ready{docs} | Empty | Error(String).
- Persistence: Save → small JSON sidecar (version + doc_count). Load currently triggers Rebuild (warm-start hint, not fast restore).
- Status guarantee: Ready.docs is accurate (in-memory HashSet). Search may return empty when Uninitialized/Empty.

ploke-rag responsibilities (must implement)
- BM25 client wrapper with:
  - async bm25_status(), bm25_save(path), bm25_load(path)
  - search_bm25_lenient(query, k): timeout 250ms, if BM25 empty/unready -> dense search fallback (trace fallback_used=true)
  - search_bm25_strict(query, k): error if BM25 empty or not Ready (no fallback)
  - timeouts/retries: default 250ms RPC timeout; on cold start retry status/search up to 2 times with backoff 50ms,100ms (use tokio::time)
  - consistent error mapping: channel→RagError::Channel, DbError→RagError::Db

Fusion & ranking essentials
- Normalize scores per-modality using min-max (configurable).
- RRF: fused(u) = w_bm25 / (k + rank_bm25(u)) + w_dense / (k + rank_dense(u)); stable tie-break by UUID asc.
- MMR: greedy selection using cosine similarity on normalized embeddings; missing embeddings → zero vector (sim=0).
- Provide functions: normalize_scores, rrf_fuse, mmr_select.

Context assembly essentials
- TokenCounter trait (injected) for deterministic budgeting in tests.
- Use IoManagerHandle to fetch snippets; dedupe by TrackingHash; stitch contiguous ranges by file.
- Token budget: reserve tokens for system prompt; enforce per-part/per-file caps deterministically.
- Output AssembledContext with ordered ContextParts and stats.

Observability (mandatory span fields)
- bm25_status, strict (bool), attempts, timeout_ms, fallback_used (bool), bm25_results_len, dense_results_len, duration_ms, persist_bytes (when available)

Tests (priority)
- Unit: status responses, lenient fallback triggers, strict rejects on Empty/Uninitialized, timeout + retry behavior (inject actor delays/mocks).
- Integration: Rebuild→Search yields results (no dense fallback); Save writes sidecar file; Load triggers Rebuild → Ready.

Important implementation notes
- Treat Save as cheap best-effort; Load currently equals Rebuild (ploke-db behavior).
- Rely on Ready.docs for readiness checks.
- Use tokio::time::timeout and tokio::time::sleep for sleeps/backoff.
- Keep existing lenient search API for backward compatibility; add strict + persistence methods as opt-in.

Minimal deliverables for next steps
1. Add BM25 client wrapper in crates/ploke-rag (timeouts, retries, strict/lenient).
2. Implement score normalization + RRF + MMR utilities.
3. Implement context assembler (TokenCounter trait + IoManager integration).
4. Add unit/integration tests described above and emit tracing fields.

Commands
```bash
git add crates/ploke-rag/CONCISE_SUMMARY_FOR_GPT5.md
git commit -m "docs: add concise implementer summary for GPT-5"
```
