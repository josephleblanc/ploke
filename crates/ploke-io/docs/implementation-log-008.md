# Implementation Log 008 — Tests for Scan Ordering and Bounded Concurrency

Date: 2025-08-18

Summary
- Added unit tests for Phase 3 acceptance:
  - test_scan_changes_preserves_input_order: validates deterministic ordering of results aligns with input indices and distinguishes changed vs unchanged files.
  - test_scan_changes_bounded_concurrency: validates observed maximum concurrent scan operations do not exceed configured semaphore permits using internal test instrumentation.

Rationale
- Completes Phase 3 acceptance by verifying behavior, not just implementation.
- Ensures future regressions in ordering or concurrency limits are caught by tests.

Changes Made
- crates/ploke-io/src/lib.rs:
  - Inserted two new #[tokio::test] functions in tests module for ordering and concurrency checks.
- crates/ploke-io/docs/production_plan.md:
  - Updated progress to reflect test completion for Phase 3.

Tests/Verification
- Run: cargo test -p ploke-io
  - All tests pass locally; new tests exercise scan path and test-only instrumentation.

Impact/Risks
- None to runtime behavior; instrumentation is compiled only in test builds.
- Test timing uses modest async delay; should be stable under CI.

Next Steps
- Phase 4: Begin watcher integration design and behind-a-flag scaffolding.
- Phase 7: Plan canonicalization and symlink policy for roots enforcement hardening.
