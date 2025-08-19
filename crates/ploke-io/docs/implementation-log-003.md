# Implementation Log 003 — Extract IO Helpers and Add Tests

Date: 2025-08-18

Summary
- Extracted helper functions to improve readability and reuse in the read and scan paths:
  - read_file_to_string_abs(path) -> Result<String, IoError>
  - parse_tokens_from_str(content, path) -> Result<TokenStream, IoError>
  - extract_snippet_str(content, start, end, path) -> Result<String, IoError>
- Refactored IoManager::process_file and IoManager::check_file_hash to use these helpers.
- Added tests:
  - test_large_file_snippet_extraction: validates UTF-8 safe slicing on a ~2MB file.
  - test_mixed_batch_hash_mismatch_per_request: verifies per-request hash validation on same-file mixed batch.

Rationale
- Aligns with Phase 1 of docs/production_plan.md to harden the read path, avoid duplication, and ensure per-request correctness.

Changes Made
- crates/ploke-io/src/lib.rs:
  - Added helper functions (module-level).
  - Updated process_file() to use read_file_to_string_abs, parse_tokens_from_str, and extract_snippet_str.
  - Updated check_file_hash() to use read_file_to_string_abs and parse_tokens_from_str.
  - Added two new unit tests.

Tests/Verification
- Run: cargo test -p ploke-io
- Expect all tests to pass, including new ones.

Impact/Risks
- Pure refactor for readability and maintainability; error variants preserved.
- Helper functions centralize policy (absolute path, UTF-8, range and boundary checks).

Next Steps
- Phase 1: Consider adding a test for semaphore exhaustion under extremely small limits (optional; behavior already covered by existing throttling tests).
- Phase 2: Introduce IoManagerBuilder with configurable roots and limits as outlined in production_plan.md.
