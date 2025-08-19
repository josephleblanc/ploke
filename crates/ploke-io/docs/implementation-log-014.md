# Implementation Log 014 — Watcher Tests, Unreachable Pattern Fixes, and Phase 5/7 Scaffolding

Date: 2025-08-19

Summary
- Fixed unreachable pattern warnings in watcher by reordering and narrowing match arms.
- Added watcher tests (feature-gated) covering Created/Modified/Removed and Renamed events.
- Introduced initial scaffolding for Phase 5 (write path) and Phase 7 (symlink policy).

Rationale
- Ensures correct event classification and eliminates compiler warnings.
- Validates watcher functionality under debouncing.
- Prepares codebase for upcoming write-path and security policy work.

Changes Made
- src/watcher.rs:
  - map_notify_events: removed Name(_) from broad Modify arm; added explicit Modify(Any) arm; kept rename arms specific.
  - map_event_kind: handled rename variants before broad Modify kinds; added Name(_) and Any arms explicitly.
  - Added async tests under cfg(feature="watcher").
- src/path_policy.rs:
  - Added SymlinkPolicy enum and path_within_roots_with_policy placeholder.
- src/write.rs:
  - Added internal scaffolding: WriteSnippetData, WriteResult, and write_snippets_batch stub.
- docs/production_plan.md:
  - Added progress update reflecting these changes.

Tests/Verification
- cargo test -p ploke-io --features watcher
  - All existing tests pass; new watcher tests pass locally.
  - No unreachable pattern warnings remain.

Impact/Risks
- Behavior unchanged for consumers; watcher event mapping clarified.
- Tests rely on filesystem notifications; debouncing and platform differences mitigated via timeouts.

Next Steps
- Wire SymlinkPolicy into builder configuration; enforce in normalize_against_roots.
- Begin implementing write path per Phase 5: per-file locking and atomic rename workflow.
- Maintain two-log window by removing the oldest log (implementation-log-012.md).

References
- docs/production_plan.md
- src/{watcher.rs, path_policy.rs, write.rs}

Progress Update — 2025-08-19 (Phase 5 wiring + Symlink policy in builder)
- Phase 5 scaffolding integrated into actor:
  - Added IoRequest::WriteSnippetBatch with handler delegating to write::write_snippets_batch (stub).
  - This prepares the actor message surface for the upcoming write path without exposing a public API yet.
- Phase 7 builder wiring:
  - IoManagerBuilder gained with_symlink_policy(...) and carries an optional SymlinkPolicy.
  - IoManager now stores an optional symlink_policy and enforces it when roots are configured.
  - Added normalize_against_roots_with_policy(...) which performs strict canonicalization and root checks using the policy hook.
- Notes:
  - Current SymlinkPolicy behavior still delegates to path_within_roots; a stricter policy will be implemented in a follow-up.
  - Backwards compatibility preserved; defaults remain unchanged unless explicitly configured.
