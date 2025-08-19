# Implementation Log 009 — Refactor Strategy and Skeleton Modules

Date: 2025-08-18

Summary
- Added a written refactor strategy to decompose crates/ploke-io/src/lib.rs into cohesive modules.
- Created non-compiling (comment-only) skeleton files outlining future module boundaries and function/test placement.

Rationale
- Reduce context footprint in chat by anchoring plan and inventories on disk.
- Enable small, incremental PRs to move code with stable public API.

Changes Made
- docs/refactor_strategy.md: strategy, module map, migration steps, context budget guidance.
- src/actor.rs: actor/message types and method inventories (comment-only).
- src/handle.rs: IoManagerHandle API inventory (comment-only).
- src/builder.rs: IoManagerBuilder and compute_fd_limit_from_inputs inventory (comment-only).
- src/read.rs: read path helpers and batch processing inventory (comment-only).
- src/scan.rs: scan path and test instrumentation inventory (comment-only).
- src/errors.rs: errors and mapping inventory (comment-only).
- src/path_policy.rs: path policy inventory (comment-only).
- src/tests_skeleton.rs: consolidated test inventory to guide migration.

Tests/Verification
- No runtime code moved yet; build should remain unchanged.
- Skeleton files are comment-only and do not affect compilation.

Impact/Risks
- None immediately; future steps must coordinate re-exports and visibility to avoid breaking API.

Next Steps
- Proceed with Step 1: extract helpers and path policy into the new modules and wire them from lib.rs with pub(crate) re-exports.
- Maintain 2-log window: remove the oldest log after this addition (keep 008 and 009).

References
- docs/production_plan.md
- docs/implementation-log-008.md
- docs/refactor_strategy.md
