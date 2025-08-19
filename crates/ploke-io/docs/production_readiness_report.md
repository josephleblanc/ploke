# Production Readiness Assessment — ploke-io

Date: 2025-08-19

Overview
ploke-io is an async I/O actor providing concurrent reads, change scanning, optional watcher integration, and an atomic write path. This report summarizes current status, remaining work to reach production readiness, risks, and blockers.

Current Status
- Read Path (Phases 1/3): 
  - Per-request hash verification, UTF-8-safe slicing, strict absolute path + roots enforcement.
  - Bounded concurrency with deterministic ordering; tests in place.
- Builder & Configuration (Phase 2):
  - IoManagerBuilder with semaphore permits, fd-limit precedence/clamp, roots, watcher toggles.
- Watcher (Phase 4):
  - Feature-gated; debouncing + coalescing; broadcast API; tests for create/modify/remove/rename.
- Write Path (Phase 5):
  - End-to-end splice with atomic temp-write + fsync + rename; per-file async locks; tests green.
- Path Policy (Phase 7):
  - Strict canonicalization; placeholder SymlinkPolicy plumbed; enforcement present for roots.
- Docs & Process:
  - Implementation logs and production plan under docs; error mapping aligned to ploke_error.

Key Items Required for Full Production Readiness
- Symlink Policy Finalization (Phase 7):
  - Decide semantics: Follow vs DenyCrossRoot; ensure comparisons are on canonical paths and add tests (traversals, chained symlinks).
- Cross-process Coordination (Optional):
  - Evaluate OS advisory locks (flock/fcntl on Unix, CreateFile semantics on Windows) to avoid cross-process write races; make feature-gated if added.
- Origin Correlation for Writes (Phase 4/5):
  - Propagate a correlation id from write requests through actor to emitted watcher events to enable echo suppression by subscribers.
- Observability (Phase 8):
  - Add structured tracing for read/scan/write with durations and result codes; optional simple metrics (counters/histograms) behind a feature gate.
- CI and Release Hygiene (Phase 10):
  - CI matrix covering default and watcher feature; fmt, clippy (with reasonable allow list), tests.
  - MSRV definition; dependency audit; CHANGELOG and versioning.
- Documentation (Phase 9):
  - Public README examples for builder, watcher subscription, write semantics and failure modes; error model mapping examples.
- Caching Layer (Phase 6, optional):
  - If needed for performance, add bounded LRU with watcher-based invalidation; otherwise defer.

Risks and Unknowns
- Cross-platform differences:
  - rename + fsync semantics vary (Windows vs Unix); ensure expectations documented and tested.
  - notify backend behaviors differ; maintain debounce/coalesce logic robustly.
- Performance and memory:
  - Large-file handling under high concurrency may pressure memory; consider memmap (feature-gated) or streaming reads later.
- Error surface:
  - Ensure error kinds are stable across crates; continue aligning IoError -> ploke_error mapping.

Missing Information / Blockers
- ploke-core type stabilization:
  - Confirm final shapes of WriteSnippetData and WriteResult and any future watcher event types to avoid churn.
- Policy decisions:
  - Final decision on SymlinkPolicy behavior and defaults; whether OS advisory locks are required for v1.
- Platform scope:
  - Confirm target OS list and MSRV; Windows-specific file and watcher behavior expectations.

Recommendations and Rough Timeline
- Week 1:
  - Land symlink policy enforcement + tests; add origin correlation id; extend documentation on write semantics.
- Week 2:
  - Add tracing fields and minimal metrics; build CI matrix incl. watcher; define MSRV; perform dependency audit.
- Optional Week 3:
  - Evaluate OS advisory lock feature; add behind a flag if adopted. Consider caching layer if needed by benchmarks.

Conclusion
The crate is close to production readiness for the read/scan/watch/write core. The primary remaining work is policy finalization, observability, and release hygiene. No hard technical blockers, but cross-crate type stabilization and policy decisions are required to freeze the public surface.
