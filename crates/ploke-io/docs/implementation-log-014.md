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
