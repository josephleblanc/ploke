# Implementation Log 015 — Write Request Wiring and Symlink Policy Builder Integration

Date: 2025-08-19

Summary
- Integrated Phase 5 scaffolding into the Io actor by adding a WriteSnippetBatch request variant and handler that delegates to the existing write::write_snippets_batch stub.
- Advanced Phase 7 by wiring SymlinkPolicy through IoManagerBuilder and IoManager; added normalize_against_roots_with_policy for policy-aware normalization.

Rationale
- Pre-wiring the write request path allows incremental implementation of the write workflow without disruptive API churn later.
- Carrying a symlink policy through the builder enables future tightening of path normalization without changing the public configuration surface.

Changes Made
- src/actor.rs:
  - Added IoRequest::WriteSnippetBatch and its handler.
  - IoManager now carries symlink_policy; propagated to read and scan paths.
  - handle_read_snippet_batch_with_roots/process_file_with_roots and handle_scan_batch_with_roots/check_file_hash_with_roots accept and apply symlink_policy.
- src/path_policy.rs:
  - Added normalize_against_roots_with_policy using strict canonicalization and policy-aware root checks.
- src/builder.rs:
  - Added builder field symlink_policy: Option<SymlinkPolicy> and with_symlink_policy(...) method.
  - IoManager::new_with now takes symlink_policy and stores it.
- docs/production_plan.md:
  - Added progress update for Phase 5 wiring and policy builder integration.

Tests/Verification
- Build-only verification at this step; write path remains a stub.
- Existing tests for read/scan continue to apply; no behavior changes unless symlink policy is configured.

Impact/Risks
- Internal enum expansion (IoRequest) increases match coverage; handler added to maintain exhaustiveness.
- Symlink policy is optional; default None preserves prior behavior.
- Future work will tighten policy semantics and add tests.

Next Steps
- Implement actual write path steps (splice + atomic rename + new hash) and add a public handle method.
- Enforce a strict symlink policy that prevents cross-root traversals and add unit tests.
- Maintain two-log window by removing the oldest log after committing this one.

References
- docs/production_plan.md
- src/{actor.rs, builder.rs, path_policy.rs, write.rs}

Progress Update — 2025-08-19 (Phase 5: Minimal Write Path)
- Implemented the initial write path in src/write.rs:
  - Validates absolute paths and reads UTF-8 content via existing helpers.
  - Verifies expected TrackingHash against actual before applying changes.
  - Enforces byte-range and UTF-8 char-boundary correctness.
  - Splices replacement bytes in-memory, computes new TrackingHash from tokens.
  - Performs atomic write using a temp file in the same directory, syncs, renames, and best-effort fsync of parent dir.
- IoRequest::WriteSnippetBatch now executes real writes via write::write_snippets_batch (still internal API).
- Not yet implemented:
  - Per-file async locking to prevent concurrent writes to the same file.
  - Watcher origin correlation and event emission.
  - Public Handle API and shared types in ploke-core.
- Next:
  - Add per-file locking and expose a public write_snippets_batch API once shared types land in ploke-core.
  - Enforce roots/symlink policy on write paths analogous to reads/scans.
