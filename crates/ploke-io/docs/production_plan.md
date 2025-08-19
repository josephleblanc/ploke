# ploke-io Production Plan

Purpose
Define the concrete, phased roadmap and procedures to take ploke-io from the current state to a production-ready, configurable IO actor supporting robust read paths, change scanning, optional watcher integration, and atomic write operations, with strong observability, safety, and testing.

Scope and Goals
- Production-grade read path:
  - Per-request file hash verification (vs. first-request only).
  - Correct, pre-allocated result ordering and UTF-8 safe slicing.
  - Bounded concurrency driven by a well-defined limit.
- Configurability via IoManagerBuilder with sane defaults and env overrides.
- Efficient change scanning with bounded concurrency/backpressure semantics.
- Optional file watcher with debouncing and broadcast events.
- Planned write path with per-file locking and atomic rename workflow.
- Strong error model and mapping to ploke_error.
- Security and path policy; observability and metrics.
- Documentation, examples, and extensive tests.

Non-Goals
- AST-level rewrites, lint orchestration, git operations, or agent strategies.
- Unbounded caching or complex cross-process orchestration beyond OS advisory locks.

Key Deliverables (APIs and Types)
- IoManagerHandle (existing):
  - get_snippets_batch(Vec<EmbeddingData>) -> Result<Vec<Result<String, PlokeError>>, RecvError>
  - scan_changes_batch(Vec<FileData>) -> Result<Result<Vec<Option<ChangedFileData>>, PlokeError>, IoError>
  - shutdown()
- New APIs:
  - subscribe_file_events() -> broadcast::Receiver<FileChangeEvent> [watcher]
  - write_snippets_batch(Vec<WriteSnippetData>) -> Result<Vec<Result<WriteResult, PlokeError>>, IoError> [writes]
- Builder:
  - IoManagerBuilder with methods:
    - with_semaphore_permits(usize)
    - with_fd_limit(usize) and env override: PLOKE_IO_FD_LIMIT (clamped e.g., 4..=1024)
    - with_roots(Vec<PathBuf>) [path policy]
    - enable_watcher(bool).with_debounce(Duration)
    - with_cache_limits(bytes: usize, entries: usize) [optional future]
    - with_locking_policy(enum) [future]
    - build() -> IoManagerHandle
- Cross-crate types (preferably in ploke-core; coordinate changes):
  - WriteSnippetData, WriteResult
  - FileChangeEvent, FileEventKind
  - Potential LockError types if needed upstream.

Phased Implementation Plan

Phase 1: Read Path Hardening and Hygiene (High Priority)
- Correctness:
  - Compute actual TrackingHash once per file and verify per-request.
  - Replace sort-based reordering with a pre-allocated Vec<Option<Result<...>>> filled by index.
  - Extract helpers:
    - read_file_to_string(path) -> Result<String, IoError>
    - parse_tokens(content, path) -> Result<TokenStream, IoError>
    - extract_snippet(content, start, end, path) -> Result<String, IoError>
- Error model:
  - Reinstate impl From<RecvError> for IoError (consistent chain).
  - Map shutdown/channel errors to Internal error policy consistent with ploke_error.
- Concurrency limit:
  - Add PLOKE_IO_FD_LIMIT env override with clamp; default to min(100, soft/3) or 50 on error.
- Change scanning:
  - Use bounded concurrency (e.g., futures::stream::iter(...).buffer_unordered(N)) guided by permits for better backpressure semantics.
- Security/path basics:
  - Add simple path checks: reject non-absolute paths or attempts to escape configured roots when roots are set.
- Dependency cleanup:
  - Remove tokio-stream if unused; align README example with TrackingHash, not seahash.
- Tests:
  - Per-request hash mismatch mixed batch.
  - Parse error file (invalid Rust).
  - Very large file (e.g., >1MB) snippet extraction.
  - Semaphore exhaustion with very small limit.
  - Shutdown with no active ops.
- Acceptance:
  - All existing tests pass; new tests added and green.
  - Benchmarks show no regression in typical workloads.
  - README updated for hashing consistency (no seahash sample).

Phase 2: IoManagerBuilder and Configuration Surface
- Implement IoManagerBuilder with defaults mirroring current behavior.
- Expose: with_semaphore_permits, with_fd_limit, with_roots.
- Move runtime bootstrapping to builder; preserve IoManagerHandle::new() as a convenient default.
- Acceptance:
  - Backward compatible API maintained.
  - Unit tests for builder options and env overrides.
  - Documentation section “Configuration and Builder” added.

Phase 3: Change Scanning Backpressure and Result Ordering
- Replace spawn-all pattern with bounded stream in scan path (if not completed in Phase 1).
- Preserve input order in output (Vec<Option<ChangedFileData>>) deterministically.
- Acceptance:
  - New tests covering ordering and concurrency behavior.
  - Tracing spans show bounded concurrency in action.

Phase 4: Watcher Integration (Optional Feature)
- Add notify-based watcher within actor runtime:
  - Debounce and coalesce events; bounded broadcast channel.
  - subscribe_file_events() -> broadcast::Receiver<FileChangeEvent).
  - Origin correlation id for echo suppression (for future writes).
- Acceptance:
  - Integration tests for watcher events under file changes.
  - Documented configuration and operational guidance.
  - Backpressure handling documented; coalescing behavior deterministic.

Phase 5: Write Path with Atomic Renames
- API: write_snippets_batch(Vec<WriteSnippetData>) -> Result<Vec<Result<WriteResult, PlokeError>>, IoError>
- Behavior:
  - Normalize path; per-file async mutex; OS advisory lock.
  - Read current bytes; UTF-8; compute actual TrackingHash; verify expected; ContentMismatch on mismatch.
  - Splice in memory; write temp (0600), fsync; atomic rename; fsync parent.
  - Compute new TrackingHash; emit FileChangeEvent with origin; return new hash and delta.
- Acceptance:
  - Property tests: splice preserves valid UTF-8 and expected ranges.
  - Fault injection tests (best-effort) around rename/fsync steps.
  - Docs: write semantics, failure modes, and safety guarantees.

Phase 6: Optional Caching Layer
- Bounded LRU keyed by (canonical_path, mtime, size) → (bytes, tokens, hash).
- Invalidation on watcher event or mtime change.
- Feature-gated if necessary; metrics to observe hit rates.
- Acceptance:
  - Benchmarks demonstrate improvements on repeated-access workloads.
  - Clear eviction policy and memory bounds; disabled by default if uncertain.

Phase 7: Security, Path Policy, and Safety
- Canonicalize paths against configured roots.
- Symlink policy: follow or deny across boundaries based on config.
- Permissions/error mapping deterministic; avoid leaking sensitive absolute paths in user-facing messages where not needed.
- Acceptance:
  - Tests for traversal attempts, symlink edge cases, permission-denied paths.
  - Documentation of policy with examples.

Phase 8: Observability and Performance
- Tracing:
  - Consistent span naming for major operations; structured fields for path, size, duration, result.
  - ErrorLayer for span traces on errors.
- Metrics:
  - Optionally expose basic counters/histograms behind a feature gate.
- Performance:
  - Pre-allocate collections; consider memmap2 for large reads (feature-gated).
- Acceptance:
  - Documented tracing fields; examples for enabling logs.
  - No regressions; optional features behind flags.

Phase 9: Documentation and Examples
- README:
  - Replace hashing example with TrackingHash-based demonstration consistent with ploke-core.
  - Add builder and watcher examples (feature gated).
- API docs:
  - Document IoManagerMessage, OrderedRequest, and helper functions.
- Acceptance:
  - Docs validated; examples compile under doc tests if feasible.

Phase 10: Release Readiness
- Dependency audit; MSRV set and documented.
- CI: fmt, clippy (deny(warnings) where feasible), tests, optional features matrix.
- Versioning and CHANGELOG entries.
- Acceptance:
  - All gates pass; semver-respecting release; migration notes where needed.

Procedures to Follow During Implementation

Implementation Logs (2-log window)
- For each applied change (or cohesive set), add a new file: crates/ploke-io/docs/implementation-log-NNN.md
  - NNN is zero-padded (000, 001, 002, …).
  - Include: Title, Date, Summary, Rationale, Changes Made, Tests/Verification, Impact/Risks, Next Steps (with references to this plan sections).
- Maintain a 2-log window:
  - After adding a new log, if more than two logs exist, remove the oldest (git rm).
  - Keep links in README to the latest logs if desired.

Change Hygiene
- Keep PRs small and scoped per phase subsection.
- Add or update tests in the same PR as the change.
- Run: cargo fmt, cargo clippy, cargo test (workspace), and ensure no warnings in changed code where practical.
- Update documentation alongside code changes.

Cross-Crate Coordination
- Introduce new shared types (WriteSnippetData, WriteResult, FileChangeEvent, FileEventKind) in ploke-core first.
- Align error mapping with ploke-error; prefer Internal error variants for shutdown/channel unless Fatal is clearly policy.
- Keep changes backward-compatible across crates or coordinate version bumps.

Configuration Policy
- Defaults: safe and minimal (no watcher, no cache, dynamic concurrency limit).
- Env override PLOKE_IO_FD_LIMIT respected and clamped.
- Document all config knobs under “Configuration and Builder”.

Security and Path Policy
- Normalize/canonicalize paths within configured roots.
- Reject traversal outside roots.
- Clearly document symlink handling and permission errors.

Testing Strategy (Summary)
- Unit tests: boundaries, per-request hash verification, UTF-8 errors, permission denied, shutdown.
- Integration tests: large files, high concurrency, watcher debounce/coalesce, write atomicity (future).
- Property tests: splice correctness for write path.
- Performance tests: basic throughput under mixed read workloads; cache effectiveness (future).

Risks and Mitigations
- Error variant alignment: coordinate with ploke-error to avoid mismatches.
- Caching invalidation complexity: start small and optional.
- Watcher platform differences: feature-gate and test per-OS as feasible.

Release Checklist
- All phases marked complete or feature-gated off.
- CI green, docs updated, examples valid.
- CHANGELOG and semantic version bump.
- Post-release monitoring plan.

References
- docs/architecture_boundary.md
- docs/code_review/2025-08-18.md
- src/lib.rs (current implementation and tests)

Progress Update — 2025-08-18
- Phase 2 started: IoManagerBuilder introduced with with_semaphore_permits, with_fd_limit (env override PLOKE_IO_FD_LIMIT, clamped 4..=1024), with_roots, and build().
- Backward-compatible: IoManagerHandle::new() retained; added IoManagerHandle::builder().
- Unit tests added for limit computation precedence and clamping.
- Path policy basics enforced: when roots are configured, read and scan requests for files outside the roots are rejected early with InvalidInput errors (Phase 1).
- Phase 3: scan_changes_batch preserves input order deterministically while maintaining bounded concurrency; unit tests added to verify ordering and to measure bounded concurrency under load using internal test instrumentation.
```rust
// from crates/ploke-core/src/mod.rs
#[derive(Debug, Clone)]
pub struct EmbeddingData {
    pub id: Uuid,
    pub name: String,
    pub file_path: PathBuf,
    pub file_tracking_hash: TrackingHash,
    pub start_byte: usize,
    pub end_byte: usize,
    pub node_tracking_hash: TrackingHash,
    pub namespace: Uuid,
}

// TODO: Make these Typed Ids, and put the typed id definitions into ploke-core
#[derive(Debug, Clone)]
pub struct FileData {
    /// Uuid is of the owner file-level module
    pub id: Uuid,
    pub namespace: Uuid,
    pub file_tracking_hash: TrackingHash,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ChangedFileData {
    /// Uuid is of the owner file-level module
    pub id: Uuid,
    pub namespace: Uuid,
    pub old_tracking_hash: TrackingHash,
    pub new_tracking_hash: TrackingHash,
    pub file_path: PathBuf,
}

impl ChangedFileData {
    pub fn from_file_data(value: FileData, new_tracking_hash: TrackingHash) -> Self {
        let FileData {
            id,
            namespace,
            file_tracking_hash,
            file_path,
        } = value;
        Self {
            id,
            namespace,
            old_tracking_hash: file_tracking_hash,
            new_tracking_hash,
            file_path,
        }
    }
}
```

Progress Update — 2025-08-19
- Phase 1/3 hygiene sustained:
  - Read path: per-request hash verification, UTF-8 safe slicing, absolute path requirement, and roots enforcement using best-effort canonicalization of paths and configured roots are in place.
  - Scan path: bounded concurrency via buffer_unordered with deterministic output ordering; test instrumentation verifies concurrency caps.
  - Error model: IoError carries RecvError via From; mapping to ploke_error uses Internal for channel/shutdown.
- Phase 2: IoManagerBuilder shipped with precedence logic and env clamp; builder tests in place.
- Docs/Process:
  - Added Implementation Log 010 with “Conversation Context Management” to keep our active chat context small while continuing the plan.
  - Next steps: begin watcher design scaffolding (feature-gated).

Notes
- To maintain a two-log window, keep implementation-log-009.md and -010.md. Remove older logs as described in Implementation Log 010.
- For continued work, we will add/remove files to the active chat as needed to minimize token usage, per Implementation Log 010 guidance.

Progress Update — 2025-08-19 (Watcher scaffolding)
- Phase 4 initial scaffolding:
  - Added feature-gated watcher module using notify with a background thread broadcasting FileChangeEvent via tokio::broadcast.
  - IoManagerBuilder: enable_watcher(bool) and with_watcher_debounce(Duration).
  - IoManagerHandle: subscribe_file_events() available when built with the "watcher" feature.
  - Cargo: new optional dependency "notify" and feature "watcher".
- Process: maintain the 2-log window by removing the oldest implementation log after committing 011.

Progress Update — 2025-08-19 (Phase 4 completion + Phase 7 hardening)
- Phase 4 (Watcher) completed:
  - Implemented debouncing and basic coalescing of notify events using an internal aggregator with configurable debounce interval.
  - Added origin correlation field (origin: Option<Uuid>) in FileChangeEvent for future echo suppression during writes.
  - Added old_path: Option<PathBuf> to FileChangeEvent to represent rename pairs when available.
- Phase 7 (Path policy) hardening:
  - normalize_against_roots now uses strict canonicalization that fails when the path cannot be resolved, preventing reads outside roots via non-existent or invalid paths.
  - Continued using canonicalized comparisons against configured roots; builder pre-canonicalizes provided roots.

Next
- Write-path (Phase 5) scaffolding: per-file locking API and origin propagation into watcher events.
- Additional Phase 7 items: configurable symlink policy and improved error redaction for user-facing messages.

Progress Update — 2025-08-19 (Watcher tests + Warnings fixed + Phase 5/7 scaffolding)
- Fixed unreachable pattern warnings in watcher match arms by:
  - Handling rename variants before broader Modify matches.
  - Removing ModifyKind::Name(_) from broad Modify arms and adding a specific ModifyKind::Any arm after rename handling.
- Added feature-gated watcher tests validating Created/Modified/Removed and Renamed events with debouncing.
- Phase 5 scaffolding: introduced src/write.rs (internal stubs) for future write-path work.
- Phase 7 scaffolding: added SymlinkPolicy enum and placeholder policy-aware root checks for future symlink handling.
- Next: wire symlink policy into builder configuration and enforce it in normalize_against_roots.
