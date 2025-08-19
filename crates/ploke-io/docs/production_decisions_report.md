# Production Decisions Report — ploke-io

Date: 2025-08-19

Purpose
This document enumerates key decisions required to reach production readiness. For each item, we outline background context and the implications of possible choices. These decisions will lock down core behavior, error policy, and public surface stability across read/scan/watch/write paths.

1) Symlink Policy Semantics and Default
Background
- Path normalization exists with a placeholder SymlinkPolicy (Follow | DenyCrossRoot).
- Current behavior uses strict canonicalization and then checks containment under configured roots.

Implications
- Follow: Allows symlink traversal; must ensure traversal cannot escape configured roots. Requires canonical, symlink-aware comparisons. More flexible for repos using symlinks.
- DenyCrossRoot: Blocks traversals that would exit or re-enter roots via symlinks; stricter, reduces risk of writing outside intended boundaries. May break legitimate symlink-heavy setups.
- Decision affects read/scan/write uniformly; must be documented and tested (including chained symlinks, broken links).

2) Cross-Process Locking Strategy (Writes)
Background
- Per-file async locks serialize writes within this process only.
- No cross-process coordination yet.

Implications
- Add OS advisory locks (flock/fcntl on Unix; CreateFile sharing modes on Windows):
  - Pros: Prevents concurrent writers across processes.
  - Cons: Portability complexity, potential deadlock/timeout policies to define, added syscalls overhead.
- Keep as-is (process-local only) for v1 and document behavior:
  - Simpler implementation; risk of races if multiple processes write the same file.

3) Write Event Origin Correlation and Echo Suppression
Background
- Watcher emits OS-derived events. Write pipeline also broadcasts “Modified” events (feature-gated).
- Without origin correlation, subscribers may react to their own writes, causing loops.

Implications
- Introduce an origin/correlation id on write requests and propagate to emitted events:
  - Pros: Subscribers can suppress own-origin events deterministically.
  - Cons: API surface change across crates; must standardize in ploke-core.
- Decide whether synthetic write events should always be emitted or only when OS events are not seen.

4) Observability Scope (Tracing and Metrics)
Background
- Minimal tracing is present; metrics are not yet included.

Implications
- Rich structured tracing (path, size, duration, result codes):
  - Pros: Faster triage, production insights.
  - Cons: Potential PII exposure in logs (paths). Requires redaction policy and log-volume management.
- Metrics (counters/histograms), feature-gated:
  - Pros: Light overhead monitoring, SLOs.
  - Cons: Additional dependency and surface; needs runtime config.

5) Platform Support and MSRV
Background
- notify, tokio, and fsync/rename semantics vary across platforms.
- No MSRV pinned yet.

Implications
- Define target OSes (Linux, macOS, Windows) and explicit MSRV (e.g., 1.76+):
  - Pros: Predictable compilation and CI matrix.
  - Cons: Limits contributor environments and dependency upgrades.
- Adjust durability steps (fsync parent) and watcher expectations per OS; document caveats.

6) Concurrency and FD Limit Policy
Background
- Effective concurrency derived from NOFILE soft limit; override via env or builder with clamping.

Implications
- Finalize precedence and clamp ranges (currently 4..=1024), and environment variable naming:
  - Pros: Stable behavior and documentation.
  - Cons: May need tuning in unusual environments (containers, CI).

7) Write Durability Guarantees
Background
- Atomic temp-write + fsync + rename + best-effort parent fsync.

Implications
- Strong durability (fsync parent mandatory, treat failures as errors) vs best-effort:
  - Stronger guarantees increase latency and potential failures on exotic filesystems.
  - Best-effort improves performance but weakens guarantees after crashes.
- Must document exact guarantees per platform and recommended deployment guidance.

8) Watcher Configuration Defaults
Background
- Debounce default ~250ms; broadcast channel capacity fixed; behavior under load unspecified.

Implications
- Choose default debounce and channel size; define behavior on overflow (drop newest/oldest/backpressure):
  - Low debounce → lower latency but more events; higher CPU.
  - High debounce → fewer events but possible missed intermediate states.
- Determine whether to watch only configured roots or allow dynamic subscriptions.

9) Error Policy and Mapping (IoError -> ploke_error)
Background
- Channel/shutdown mapped to Internal; file/parse errors mapped to Fatal variants.

Implications
- Decide classification for permission-denied, path-policy violations, and durability failures:
  - Fatal vs Internal affects retry semantics and upstream behavior.
- Redaction policy for error messages (paths and internals) to avoid leaking sensitive details.

10) Ownership of Public Types Across Crates
Background
- FileChangeEvent/FileEventKind and WriteResult/WriteSnippetData are (or will be) in ploke-core.

Implications
- Keeping types in ploke-core:
  - Pros: Single source of truth for shared models, lower duplication.
  - Cons: Tighter coupling and version alignment; requires disciplined release process.
- If types remain in ploke-io, other crates depend on ploke-io for models (less desirable).

11) Non-Existent Paths on Writes (File Creation Policy)
Background
- Current strict canonicalization requires existing files; write path may need to create new files.

Implications
- Allow creation:
  - Normalize/validate based on canonicalized parent directory within roots; ensure parent exists.
  - Decide default permissions (e.g., 0o600) and ownership considerations.
- Disallow creation in v1:
  - Simpler; restricts write API use-cases (only in-place edits).

12) Permissions and File Mode Policy (Temp and Final Files)
Background
- Temp file creation uses OS defaults; rename preserves original target perms if it exists.

Implications
- Set explicit permissions for temp and new files (e.g., 0o600) for security:
  - Pros: Predictable security posture.
  - Cons: Platform differences (Windows ACLs), potential need to preserve or copy attributes.
- Consider preserving metadata (mtime, perms, ownership) when replacing.

13) Optional Caching Layer
Background
- Proposed bounded LRU to speed repeated reads; invalidation via watcher events.

Implications
- Include in v1:
  - Pros: Performance gains on repeated access patterns.
  - Cons: Complexity and cache coherency risk; requires metrics and eviction tests.
- Defer:
  - Simpler release; may leave performance on the table for some workloads.

14) API Stability and Feature Flags
Background
- Multiple features (watcher, potential metrics); evolving builder/config.

Implications
- Mark APIs as unstable in 0.x or stabilize specific surfaces:
  - Pros: Manage expectations; plan deprecation paths.
  - Cons: Slower iteration when stabilized too early.
- Decide default features (none vs watcher on by default).

15) Watcher Backend Strategy and Polling Fallback
Background
- notify picks a recommended backend per OS; polling intervals can be configured.

Implications
- Rely on defaults vs enforce polling fallback in certain environments:
  - Polling increases CPU and latency but is more predictable across filesystems/containers.
- Expose configuration for backend/polling at builder-level if necessary.

16) Read-While-Write Policy
Background
- Writes are serialized per-file within this process; reads are not currently blocked.

Implications
- Block reads when a file is under write:
  - Pros: Eliminates stale reads during in-process writes.
  - Cons: Increased coupling and potential throughput reduction.
- Allow reads to proceed:
  - Pros: Higher parallelism.
  - Cons: Possible transient inconsistencies for subscribers relying on strict ordering.

Decision Summary: Next Steps
- Choose defaults and policies for items (1), (2), (3), (5), (6), (7), (8), (9), (11), (12), (14), and (16).
- Align cross-crate type ownership (10) and finalize public API surfaces.
- Document chosen policies in docs/production_plan.md and README, and add targeted tests.

References
- docs/production_plan.md
- docs/production_readiness_report.md
- src/{path_policy.rs, write.rs, watcher.rs, actor.rs, builder.rs}
