# Test Coverage Addendum — 2025-08-19

Purpose
- Complement the existing docs/test_coverage.md with prioritized tests and identified gaps for ploke-io.

Scope
- Read, Scan, Write, Watcher, Path Policy, Error Mapping, Concurrency/FD Limits, and Observability.

Prioritized Tests (High → Medium → Low)

High Priority
- Read path
  - Per-request hash mismatch in mixed batch (unchanged + changed) returns deterministic errors per index.
  - UTF-8 boundary checks: start_byte and end_byte on char boundaries succeed; off-boundary fails with InvalidCharBoundary.
  - Very large file slicing (~2MB) maintains performance and correctness (already present; keep).
  - Absolute-path enforcement error when given relative paths with and without roots configured.
- Scan path
  - Preserves input order for mixed outcomes (Some/None) with bounded concurrency; concurrency cap observed via test instrumentation.
  - Error propagation precedence: first error by input-order is returned (deterministic).
  - Root normalization + symlink policy applied consistently before scanning; rejects outside-root.
- Write path
  - Splice correctness for multiple non-overlapping edits (separate requests on same file) serialized by per-file lock.
  - Hash verification: ContentMismatch when expected hash differs from actual.
  - Range validation: OutOfRange (end beyond len, reversed range), InvalidCharBoundary (split UTF-8).
  - Atomicity: temp file created in same dir; rename replaces original; parent fsync best-effort (fault-injection where possible).
  - Roots enforcement: write outside configured roots is rejected; absolute path required without roots.
  - Returned new hash equals recomputed hash from resulting file.
- Error mapping
  - IoError variants map to ploke_error::Error::Fatal or ::Internal as specified (Recv → Internal, OutOfRange → Fatal FileOperation, ParseError → Fatal SyntaxError, etc).

Medium Priority
- Watcher (feature = "watcher")
  - Debounce/coalescing precedence: Created then Modified coalesce to Created within window; Remove supersedes lower precedence events.
  - Rename events: Both/From/To backends produce normalized Renamed events with optional old_path.
  - Write integration: successful write emits Modified event; basic end-to-end with subscribe_file_events.
- Path policy / SymlinkPolicy
  - DenyCrossRoot: symlink escaping root is rejected on read/scan/write.
  - Follow: lexical containment check passes within root, then strict canonicalization; rejects traversal outside root.
- Concurrency / Limits
  - FD limit precedence and clamping: builder > env > soft/3 > default 50 (unit tests in builder; extend with env-var isolation).
  - Exact semaphore limit respected under heavy load without exceeding permits (already present for scan; add for read/write).

Low Priority
- Observability
  - Tracing spans exist for major operations (read, scan, write) and include key fields (path, bytes, result) when enabled.
- Performance sanity
  - Micro-benchmark guidance (Criterion) to ensure no regressions for read batches and change scans (future work).
- Cross-process behavior (documentation only for now)
  - Note lack of cross-process write serialization; potential advisory lock experiments behind a feature (future).

Gaps and Proposed Additions
- Write: overlapping edits ordering semantics (current per-batch behavior is independent per request; document and test).
- Error redaction: user-facing messages vs internal logs for sensitive absolute paths (add tests to assert redacted messages when applicable).
- Symlink chains and broken symlinks: explicit tests for policy behavior and strict canonicalization failures.
- Non-existent paths in reads/scans/writes: consistent InvalidInput mapping; verify messages.
- Watcher integration with multiple roots: events tagged with the observed root (optional future enhancement).

Test Organization
- Keep unit tests colocated: src/{read.rs, scan.rs, write.rs, watcher.rs} with #[cfg(test)] modules.
- Feature-gated watcher tests under #[cfg(all(test, feature = "watcher"))].
- Use tempfile for isolated file system effects; prefer deterministic content and stable ranges.

Execution
- Core suite:
  - cargo test -p ploke-io
- With watcher:
  - cargo test -p ploke-io --features watcher

Maintenance
- Update this document as tests are added.
- Link from docs/production_plan.md if milestones shift coverage priorities.
