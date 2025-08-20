# Implementation Log 007 — Deterministic Ordering for Scan Changes

Date: 2025-08-18

Summary
- Preserve input order for scan_changes_batch results while keeping bounded concurrency.
- Implemented index-tracking with pre-allocated output vector.
- Deterministic error propagation: returns the first error by input order if any file fails.

Rationale
- Completes the ordering requirement in Phase 3 of docs/production_plan.md.
- Avoids nondeterministic result ordering inherent in buffer_unordered collection.

Changes Made
- crates/ploke-io/src/lib.rs:
  - Refactored IoManager::handle_scan_batch and IoManager::handle_scan_batch_with_roots:
    - Stream items carry (idx, result) tuples.
    - Results are written into a pre-allocated Vec<Option<ChangedFileData>> by index.
    - If any error occurs, the first error by input index is returned to keep behavior deterministic.
- crates/ploke-io/docs/production_plan.md:
  - Progress updated to note Phase 3 ordering completion for scan path.

Tests/Verification
- Manual reasoning: bounded concurrency unchanged; ordering now aligns with input indices.
- Run: cargo test -p ploke-io
  - Existing tests remain green.

Impact/Risks
- Behavior change only affects the scan path ordering (now deterministic).
- Error propagation may differ in which specific error bubbles up if multiple files fail; selection is now by lowest input index for determinism.

Next Steps
- Phase 3: Add unit tests to verify scan ordering and concurrency behavior.
- Phase 7: Extend roots policy with canonicalization and symlink handling.
