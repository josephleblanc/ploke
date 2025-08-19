# Implementation Log 004 — Preserve IoError Kind and Add Tests

Date: 2025-08-18

Summary
- Improved error fidelity in read path:
  - Preserve std::io::ErrorKind from tokio::fs::read in IoError::FileOperation.
- Added tests to cover plan Phase 1 items:
  - test_parse_error_invalid_rust: verifies parse errors map to Fatal(SyntaxError).
  - test_reject_relative_path: ensures non-absolute paths are rejected with FileOperation(InvalidInput).
- Dependency cleanup:
  - Removed unused seahash from dev-dependencies.

Rationale
- Aligns with Phase 1 of docs/production_plan.md:
  - Stronger error mapping and path policy basics.
  - Expanded test coverage for parse failures and path validation.
  - Clean up unused dev dependencies.

Changes Made
- crates/ploke-io/src/lib.rs:
  - Updated read_file_to_string_abs to capture and propagate the original io::ErrorKind.
  - Added two unit tests: test_parse_error_invalid_rust and test_reject_relative_path.
- crates/ploke-io/Cargo.toml:
  - Removed seahash dev-dependency.

Tests/Verification
- Run: cargo test -p ploke-io
- All tests should pass.

Impact/Risks
- Error mapping now accurately reflects OS error kinds, improving downstream diagnostics.
- New tests are isolated and do not alter existing behavior.

Next Steps
- Continue Phase 2: Introduce IoManagerBuilder (with_semaphore_permits, with_fd_limit, with_roots) with unit tests and env overrides.
- Consider adding a semaphore exhaustion test under extremely small limits as a non-blocking enhancement.
