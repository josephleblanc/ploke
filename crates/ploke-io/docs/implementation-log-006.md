# Implementation Log 006 — Enforce Basic Roots Path Policy

Date: 2025-08-18

Summary
- Enforced Phase 1 path policy basics when roots are configured:
  - Read path: requests for files outside configured roots short-circuit with InvalidInput.
  - Scan path: requests for files outside configured roots return InvalidInput.
- Internal structure changes:
  - IoManager now stores roots as Option<Arc<Vec<PathBuf>>> to avoid cloning on each request.
  - Added helper path_within_roots and new internal methods:
    - handle_read_snippet_batch_with_roots
    - process_file_with_roots
    - handle_scan_batch_with_roots
    - check_file_hash_with_roots
- Backward compatibility: existing public APIs and tests continue to work.

Rationale
- Implements a minimal, safe path policy as outlined in Phase 1 of docs/production_plan.md:
  - Reject traversal attempts outside configured roots early, before file access.
  - Keep full canonicalization and symlink policy for Phase 7.

Changes Made
- crates/ploke-io/src/lib.rs:
  - Store roots as Option<Arc<Vec<PathBuf>>> in IoManager.
  - Enforce roots in both read and scan paths.
  - Added path_within_roots helper.
  - No external API changes; builder remains the configuration surface.
- crates/ploke-io/docs/production_plan.md:
  - Updated Progress Update to reflect basic roots enforcement.

Tests/Verification
- Added unit test:
  - test_roots_enforcement_basic: verifies an in-roots file is allowed and an out-of-roots file is rejected with FileOperation(InvalidInput).
- Run: cargo test -p ploke-io
- All tests pass locally.

Impact/Risks
- Behavior change only when IoManager is built with roots; default behavior remains unchanged.
- Does not canonicalize paths or handle symlinks yet; that remains a Phase 7 deliverable.

Next Steps
- Phase 2 docs: Add “Configuration and Builder” documentation, including roots.
- Phase 7: Implement canonicalization and symlink policy to harden path checks.
