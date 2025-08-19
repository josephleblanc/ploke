# Implementation Log 001 — Phase 1: Begin Read Path Hardening

Date: 2025-08-18

Summary
- Implemented key Phase 1 items from docs/production_plan.md:
  - Pre-allocated result ordering for read path (no post-sort; fill-by-index).
  - Per-request file hash verification (no longer rely on first request’s hash).
  - Added PLOKE_IO_FD_LIMIT env override with clamping (4..=1024).
  - Adopted bounded backpressure in scan path via buffer_unordered.
  - Basic path policy: reject non-absolute paths (builder-based roots policy to follow).
  - Restored From<RecvError> for IoError for consistent error chaining.
  - Removed unused tokio-stream dependency.

Rationale
- Improves correctness and determinism in read results.
- Aligns concurrency and resource usage with configurable, safe limits.
- Sets groundwork for broader configuration surface and path policy in Phase 2/7.

Changes Made
- crates/ploke-io/src/lib.rs:
  - IoManager::new: add env override for FD limit with clamp.
  - IoManager::handle_read_snippet_batch: switch to pre-allocated Vec<Option<Result<...>>> and fill by index.
  - IoManager::process_file: add absolute-path check and per-request hash verification.
  - IoManager::handle_scan_batch: replace spawn-all/join_all with buffer_unordered.
  - IoManager::check_file_hash: add absolute-path check.
  - impl From<RecvError> for IoError.
- crates/ploke-io/Cargo.toml:
  - Remove tokio-stream dependency (unused).

Tests/Verification
- Existing tests should continue to pass; changes preserve error mappings and boundaries.
- Manual sanity: concurrency limit obeys PLOKE_IO_FD_LIMIT; non-absolute paths rejected early.

Impact/Risks
- Ordering now strictly index-driven; any missing task results surface as InternalError::InvalidState.
- Absolute-path requirement may surface new errors if callers pass relative paths (intended per plan).

Next Steps
- Phase 1:
  - Extract helper functions (read/parse/extract) to simplify process_file.
  - Add tests for per-request hash mismatch in mixed batches and large-file slicing.
  - Consider removing other unused deps and align doc examples (seahash removal).
- Phase 2:
  - Introduce IoManagerBuilder with roots and other config.
