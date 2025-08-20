# Implementation Log 002 — Update Unit Tests to Match Production Plan

Date: 2025-08-18

Summary
- Updated local unit tests to align with Phase 1 behavior from docs/production_plan.md:
  - Per-request hash verification now uses the request namespace; tests compute hashes with the same namespace.
  - Fixed ordering/count assumptions in handle_read_snippet_batch test by flattening embedding groups.
  - Kept expected error variants (OutOfRange → Fatal(FileOperation), InvalidCharBoundary → SyntaxError) intact.

Rationale
- The IO read path now verifies a file’s TrackingHash per request using the provided namespace.
- Previous tests computed hashes with a different namespace (PROJECT_NAMESPACE_UUID) or placeholder paths, causing ContentMismatch.
- Ensuring tests generate hashes with the same namespace restores intended behavior and validates current design.

Changes Made
- crates/ploke-io/src/lib.rs (tests):
  - Added helper tracking_hash_with_path_ns(content, path, namespace).
  - Updated tests to use namespace-aware hash helper where appropriate:
    - test_invalid_byte_ranges
    - test_multi_byte_unicode_boundaries
    - test_partial_failure_handling (valid requests; non-existent file path updated)
    - test_seek_errors
    - test_zero_length_snippet
    - test_exact_semaphore_limit
    - test_concurrency_throttling
  - Refactored test_handle_read_snippet_batch to:
    - Flatten embedding groups into a single Vec<EmbeddingData>.
    - Compare snippet count against flattened length.
    - Zip results with flattened inputs for per-item assertions.

Tests/Verification
- Run: cargo test -p ploke-io
- Expect previously failing tests to pass with the updated per-request hash logic.

Impact/Risks
- Tests now depend on correct namespace propagation; future changes to hashing policy must update helpers accordingly.
- No production code behavior changes in this step.

Next Steps
- Continue Phase 1 test additions for large-file slicing and mixed-batch hash mismatches.
- Consider removing seahash examples from docs to avoid confusion.
