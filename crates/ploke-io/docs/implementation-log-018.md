# Implementation Log 018 — Per-file write locks + watcher write events + tests

Date: 2025-08-19

Summary
- Completed Phase 5: write path now includes per-file async locks to serialize writes per normalized path, atomic temp-write + fsync + rename, and best-effort parent directory fsync.
- Exposed IoManagerHandle::write_snippets_batch as a public API; results return new TrackingHash.
- Integrated Phase 4 watcher: on successful writes (when feature enabled), broadcast a Modified event for the written path.
- Added unit tests for splice correctness, range validation, and roots enforcement; all tests pass with and without watcher feature.

Rationale
- Per-file locking prevents intra-process races on the same file.
- Broadcasting write events creates a foundation for eventual cache invalidation and downstream listeners.
- Tests ensure core correctness and guard against regressions.

Changes Made
- src/write.rs: introduced per-file lock registry (DashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>), path normalization on writes, and end-to-end write pipeline; returns WriteResult with new hash.
- src/actor.rs: wired WriteSnippetBatch to write_snippets_batch; broadcasts watcher events on success (feature-gated).
- src/handle.rs: added public write_snippets_batch and subscribe_file_events.
- src/builder.rs: builder wires watcher channel into actor; roots pre-canonicalized.
- docs/production_plan.md: updated progress to mark Phase 5 complete and outline next steps.
- tests: added write_tests.rs covering splice/hash recompute, invalid ranges, and roots enforcement.

Tests/Verification
- cargo test -p ploke-io: 32 passed, 2 ignored.
- cargo test -p ploke-io --features watcher: 34 passed, 2 ignored.
- Manual: Verified write modifies content and returned hash matches recomputed hash.

Impact/Risks
- Per-file locks are process-local; cross-process serialization is not enforced (consider optional OS-level advisory locks).
- Watcher write events are best-effort and may duplicate/lag OS signals; downstream should de-duplicate.
- Path normalization is strict; missing files or unresolved paths under roots are rejected.

Next Steps
- Phase 7: finalize and enforce SymlinkPolicy across read/scan/write; add tests for symlink traversal.
- Phase 4: add origin correlation in write path and propagate into watcher events for echo suppression.
- Phase 8: add structured tracing fields for write ops (path, bytes, duration, result); consider simple metrics.
- Phase 10: prepare CI matrix including watcher feature; MSRV and dependency audit.

References
- docs/production_plan.md
- src/{actor.rs, builder.rs, handle.rs, write.rs, watcher.rs}
- src/write_tests.rs
