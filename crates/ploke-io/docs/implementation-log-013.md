# Implementation Log 013 — Complete Watcher (Debounce/Coalesce) and Path Policy Hardening

Date: 2025-08-19

Summary
- Phase 4 completed:
  - Implemented debounced, coalesced file event broadcasting using an internal aggregator loop.
  - Extended FileChangeEvent with origin: Option<Uuid> for future echo suppression and old_path: Option<PathBuf> for rename pairs.
- Phase 7 progress:
  - normalize_against_roots now uses strict canonicalization (error on failure) to prevent reading outside configured roots via invalid/non-existent paths.

Rationale
- Debouncing/coalescing reduces noisy event storms and provides more stable signals downstream.
- Strict canonicalization closes gaps where best-effort fallbacks could allow surprising behavior or mismatches with roots.

Changes Made
- src/watcher.rs:
  - Added debounced aggregator with recv_timeout and flush logic keyed by path.
  - Introduced FileChangeEvent { path, kind, old_path, origin }.
  - Added mapping for rename pairs (Both/From/To) when provided by notify.
- src/path_policy.rs:
  - Added canonicalize_strict() and refactored normalize_against_roots() to use it.
- docs/production_plan.md:
  - Updated with Phase 4 completion and Phase 7 hardening notes.

Tests/Verification
- Default build remains unaffected (watcher is feature-gated).
- Manual verification:
  - cargo check -p ploke-io --features watcher
  - Run a small binary or test harness to touch files under roots and observe broadcast behavior with debouncing.
- Existing IO path tests remain green.

Impact/Risks
- FileChangeEvent now has additional fields; backward compatibility maintained for consumers matching on kind/path.
- Debounce tuning may need iteration; default leverages the configured poll interval as baseline.

Next Steps
- Phase 5 scaffolding: write path with per-file locks, atomic rename, and origin propagation into watcher.
- Further Phase 7: symlink policy configuration and error message redaction for user-facing contexts.

References
- docs/production_plan.md
- src/{watcher.rs,path_policy.rs}
