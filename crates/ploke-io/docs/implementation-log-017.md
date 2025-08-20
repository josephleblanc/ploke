# Implementation Log 017 — Write API exposed + Path Policy enforced for writes

Date: 2025-08-19

Summary
- Exposed a public IoManagerHandle::write_snippets_batch API returning Vec<Result<WriteResult, PlokeError>> with channel errors mapped to IoError.
- Enforced roots/symlink policy on the write path:
  - IoManager passes configured roots and SymlinkPolicy into write::write_snippets_batch.
  - Each write request normalizes the path against configured roots (strict canonicalization); absolute-path requirement enforced with operation=write.
- Kept atomic write behavior: temp file, fsync, rename, best-effort parent fsync.

Rationale
- Advances Phase 5 by making the write pipeline callable from clients.
- Aligns with Phase 7 path policy goals to prevent writes outside configured roots.

Changes Made
- src/actor.rs: pass roots + symlink_policy to write_snippets_batch.
- src/write.rs:
  - process_one_write now normalizes/validates paths using normalize_against_roots(_with_policy) and enforces absolute paths for writes.
  - write_snippets_batch accepts roots + symlink_policy and forwards them to per-request processing.
- src/handle.rs: added public write_snippets_batch API.
  
Tests/Verification
- Compiles locally; next step is to add unit tests for write boundary checks and policy enforcement.
- Manual smoke: can call write_snippets_batch; errors surface as IoError when channel or path-policy failures occur.

Impact/Risks
- Behavior change: writes are rejected when outside configured roots or with non-absolute paths.
- No per-file locking yet; concurrent writes to the same file could race (planned follow-up).

Next Steps
- Add tests for write path: splice correctness, boundary validations, hash recomputation, and roots enforcement.
- Introduce per-file async locking keyed by normalized path.
- Optionally propagate watcher origin on write completion.

References
- docs/production_plan.md (Phase 5, Phase 7)
- src/{actor.rs, handle.rs, write.rs}
