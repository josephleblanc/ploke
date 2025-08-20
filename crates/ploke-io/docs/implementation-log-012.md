# Implementation Log 012 — Observability Spans and Watcher Logging Hygiene

Date: 2025-08-19

Summary
- Added lightweight tracing spans across critical I/O paths to improve observability without changing behavior.
- Replaced eprintln! in the watcher with tracing::warn! and added warning logs on watcher errors.

Rationale
- Early observability helps validate concurrency limits, identify hotspots, and diagnose failures in production-like runs.
- Align logging with the crate’s tracing setup for consistent output and structured fields.

Changes Made
- src/actor.rs:
  - Added tracing spans in:
    - handle_request arms (io_read_snippet_batch, io_scan_change_batch)
    - process_file (io_process_file)
    - handle_scan_batch (io_handle_scan_batch)
    - check_file_hash (io_check_file_hash)
- src/watcher.rs:
  - Replaced eprintln! with tracing::warn! and added a warning log for general watcher errors.

Tests/Verification
- No functional changes; existing tests should continue to pass.
- Run with RUST_LOG or tracing subscriber enabled to see spans and warnings.

Impact/Risks
- Minimal; spans add negligible overhead. Logging changes unify output.

Next Steps
- Phase 4: iterate on watcher debouncing/coalescing and add origin correlation fields.
- Phase 7: continue path policy hardening (canonicalization/symlink policy).
- Maintain the 2-log window by removing the oldest implementation log after committing this change.

References
- docs/production_plan.md
- src/{actor.rs,watcher.rs}
