# ploke-io Refactor Strategy — De-monolith lib.rs

Date: 2025-08-18

Motivation
- The current crates/ploke-io/src/lib.rs is monolithic and consumes significant context budget in discussions and reviews.
- We want to:
  - Improve maintainability, testability, and onboarding by modularizing features.
  - Keep public API stable (IoManagerHandle and IoManagerBuilder).
  - Enable incremental refactors without breaking the build.
  - Maintain a minimal context footprint in the chat while preserving a durable, on-disk roadmap.

High-level Approach
- Create a modular source layout with clear ownership boundaries per feature area.
- Move code in small, reviewable steps (1-2 modules per PR).
- Keep public API stable via re-exports from lib.rs after extraction.
- Maintain a rolling 2-log implementation log window for traceability and context minimization.

Target Module Layout (Incremental)
- src/handle.rs
  - Public handle and top-level API entry points.
  - IoManagerHandle { new(), builder(), get_snippets_batch(...), scan_changes_batch(...), shutdown() }
- src/builder.rs
  - IoManagerBuilder and effective limit resolution.
  - IoManagerBuilder { with_semaphore_permits, with_fd_limit, with_roots, build }
  - compute_fd_limit_from_inputs(...)
- src/actor.rs
  - IoManager (actor), message enums, and event loop.
  - IoManager { new(), new_with(), run(), handle_request(...) }
  - IoManagerMessage, IoRequest, OrderedRequest
- src/read.rs
  - Read-path helpers and batch processing.
  - read_file_to_string_abs, parse_tokens_from_str, extract_snippet_str
  - handle_read_snippet_batch, handle_read_snippet_batch_with_roots
  - process_file, process_file_with_roots
- src/scan.rs
  - Change scanning, bounded concurrency, ordering.
  - handle_scan_batch, handle_scan_batch_with_roots
  - check_file_hash, check_file_hash_with_roots
  - test_instrumentation for bounded concurrency tests (cfg(test))
- src/path_policy.rs
  - Path policy helpers and future canonicalization.
  - path_within_roots (future: canonicalization, symlink policy)
- src/errors.rs
  - Error types and mapping to ploke_error.
  - RecvError, IoError, impl From<IoError> for ploke_error::Error
- src/tests_skeleton.rs (comment-only inventory)
  - Consolidated inventory of existing tests mapped to modules, to guide migration.

Public API Stability
- Keep IoManagerHandle and IoManagerBuilder paths stable for downstream crates.
- lib.rs will re-export module items:
  - pub use crate::handle::IoManagerHandle;
  - pub use crate::builder::{IoManagerBuilder, compute_fd_limit_from_inputs};
- Internal APIs (pub(crate)) will be moved module-by-module without external breakage.

Refactor Plan — Steps and PRs
1) Extract pure helper functions and path policy
   - Move read_file_to_string_abs, parse_tokens_from_str, extract_snippet_str into src/read.rs.
   - Move path_within_roots into src/path_policy.rs.
   - Re-export from lib.rs with pub(crate) use to keep call-sites unchanged.

2) Errors and RecvError
   - Move RecvError, IoError, and the From<IoError> impl into src/errors.rs.
   - Update lib.rs to pub use crate::errors::{RecvError, IoError}.

3) Scan Path
   - Move handle_scan_batch, handle_scan_batch_with_roots, check_file_hash, check_file_hash_with_roots into src/scan.rs.
   - Move test_instrumentation into scan.rs under cfg(test).

4) Read Path
   - Move handle_read_snippet_batch, handle_read_snippet_batch_with_roots, process_file, process_file_with_roots into src/read.rs.

5) Builder and Limits
   - Move IoManagerBuilder and compute_fd_limit_from_inputs into src/builder.rs.
   - Keep IoManagerHandle::builder() delegating to the new location.

6) Actor
   - Move IoManager, IoManagerMessage, IoRequest, OrderedRequest into src/actor.rs.
   - Keep IoManagerHandle::new() implementation delegating to actor::IoManager creation.

7) Future: Watcher and Write Path
   - Introduce src/watcher/ and src/write/ directories feature-gated.
   - Define public APIs per production_plan.md once ploke-core types are introduced.

Testing Migration Plan
- Short term: Keep tests in lib.rs while moving code; import moved items via crate paths.
- Medium term: Split tests per module:
  - src/read.rs: snippet extraction, utf-8 boundaries, large file, partial failure, concurrency throttling (read path).
  - src/scan.rs: ordering, bounded concurrency, mixed changes, errors.
  - src/builder.rs: limit precedence and clamping tests.
  - src/actor.rs: shutdown during ops, send/recv errors.
  - src/path_policy.rs: roots enforcement, reject relative paths.
  - Permission tests stay under cfg(unix).
- Long term: Integration tests in tests/ as needed; property tests for write path later.

Context Budget Strategy (Important)
- Keep production_plan.md as the long-form roadmap, but add a concise “Current Status” and “Next 3 Tasks” section at the top.
- Maintain only the latest two implementation logs (2-log window). After adding a new log, git rm the oldest.
- Keep this refactor_strategy.md as the living index to the module layout and migration checklists.
- For chat-based context, reference:
  - docs/refactor_strategy.md (module map + next steps)
  - latest implementation log for “what changed”
  - keep code snippets limited to the module under change, not the full lib.rs.

Ownership Boundaries and Conventions
- Module internal visibility: use pub(crate) for cross-module calls, keep public API stable at crate root.
- Avoid cyclical dependencies; keep read/scan independent except for shared helpers.
- Tracing: consistent span names retained in moved functions; add module-targeted tracing where useful.
- Error mapping: do not alter ploke_error semantics while moving; changes should be explicit and reviewed.

On-disk Skeleton Files
- Added non-compiling (comment-only) skeletons under src/ to outline where code will land.
- These serve as anchors for future PRs without impacting build.

Next Steps
- Begin with Step 1 (helpers + path policy extraction).
- Update lib.rs with mod declarations and re-exports in a follow-up PR.

References
- docs/production_plan.md
- src/lib.rs (current impl)
- docs/implementation-log-008.md
