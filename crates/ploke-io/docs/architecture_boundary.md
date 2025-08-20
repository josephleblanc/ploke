# ploke-io Architecture Boundaries and Responsibilities

Date: 2025-08-18

Purpose
This document records architecture decisions and responsibility boundaries for ploke-io in the broader ploke system. It will guide crate design and APIs for robust, concurrent read/write file operations, watcher integration, and multi-agent workflows, while preserving a small memory footprint and clear ownership lines across crates.

Context
- ploke is an LLM-driven code generation and refactoring tool for Rust.
- The codebase is parsed into an in-memory Cozo relational vector-graph with minimal footprint.
- The DB stores node metadata and hashes, not the code text; snippets are read on-demand from disk.
- Embeddings (dense vectors + bm25) enable hybrid retrieval for RAG and reranking.
- Near-term: enable LLM-proposed file edits, reversible changes, in-chat editing and linting, and IDE integration.
- Mid-term: agent system supporting multiple concurrent agents (autonomous or mixed-initiative) operating over the same workspace.

High-Level Principles
- Isolation: File I/O runs in a dedicated actor with its own Tokio runtime, never blocking caller runtimes.
- Determinism: APIs enforce clear preconditions (path policy, UTF-8 boundaries, hash checks) and return typed errors.
- Atomicity: Writes are atomic (temp file + fsync + rename + fsync parent) to prevent partial reads and ensure watcher coherence.
- Concurrency with fairness: Bounded parallelism via semaphores plus per-path locks to avoid stampedes and preserve progress.
- Minimal footprint: No caching of full snippets in DB; optional LRU caches in the I/O layer are bounded and evictable.
- Observability: Trace spans and structured logs for lock contention, I/O latencies, and coalesced watcher events.
- Clear boundaries: IO focuses on bytes, UTF-8 safety, hashing verification, atomicity, and events; higher-level policy belongs elsewhere.

Division of Responsibilities (Crate Matrix)
- ploke-io (this crate)
  - Owns: on-demand snippet reads; future snippet-based writes; scan-for-changes; file watcher; per-process concurrency limits; per-file locking; atomic writes; event emission; path normalization/policy enforcement; IoError → ploke_error mapping; builder/configuration; tracing.
  - Uses: syn-based tokenization only to compute or verify TrackingHash (policy-dependent and optionally cached).
  - Does not own: AST rewriting, linting, formatting, graph queries, RAG retrieval, git commit policies, agent orchestration logic.

- ploke-core
  - Owns: core domain types (EmbeddingData, FileData, ChangedFileData, TrackingHash, event structs), shared constants, ID types.
  - Provides: stable cross-crate types for IO requests/responses and events.

- syn_parser (ingest)
  - Owns: discovery/parsing pipelines, canonical item identities, optional helpers for node anchoring beyond raw byte ranges.

- ploke-db
  - Owns: persistence in Cozo, graph schema, queries, indexing, and versions/metadata consistent with IO events.

- ploke-rag
  - Owns: embedding generation, hybrid retrieval (HNSW + bm25), reranking strategies, prompt assembly helpers.

- ploke-tui (and future IDE adapters)
  - Owns: UI, in-chat editing workflows, preview/diff rendering, lint surfacing, approval gates, multi-agent management UX.

- git integration layer (elsewhere)
  - Owns: staging/commits/reverts, branch policies, conflict resolution, diff generation beyond basic splice preview.

Core IO Capabilities and Policies

1) Read Path (On-Demand Snippet Reads)
- Input: Vec<EmbeddingData> (file path, byte range, expected file_tracking_hash, node_tracking_hash, ids/names).
- Behavior:
  - group-by-path, read file once per path (bounded by semaphore), decode UTF-8, parse to tokens (if verification requires), compute actual TrackingHash.
  - verify per-request against actual hash (not just the first request).
  - enforce: 0 <= start <= end <= file_len; is_char_boundary(start/end).
  - return snippets in original order, with typed errors per-request.
- Ownership: ploke-io fully owns correctness (bounds, UTF-8, per-request verification) and error mapping.
- Performance: optional bounded cache for (path, mtime, size) → (bytes, tokens, hash). Cache invalidated by watcher or mtime change.
- Non-goals: no AST-level selection; byte/UTF-8 safe slicing only.

2) Write Path (Snippet-Based Writes) [Planned]
- Input: Vec<WriteSnippetData> (extends read request with new_snippet and origin/correlation id).
- Behavior:
  - normalize path; acquire per-file in-process async mutex; acquire OS advisory lock (flock/Windows exclusive share).
  - read current bytes; UTF-8 decode; compute actual TrackingHash; compare with expected; fail with ContentMismatch on mismatch.
  - resolve target byte range (either direct offsets or via node/content anchor resolved by higher layers and passed concretely).
  - enforce UTF-8 boundaries at splice edges.
  - in-memory splice; write temp file (0600), fsync; atomic rename; fsync parent directory.
  - compute new TrackingHash; emit FileChangeEvent with origin; return new hash and delta (start, removed_len, added_len).
- Ownership: ploke-io owns locking, atomicity, hash verification, and event emission, not policy decisions about when to write.
- Non-goals: formatting, linting, or AST-aware transformations.

3) Watcher Path (File Events)
- Runtime: runs inside IoManager runtime; exposes broadcast subscription.
- Policy:
  - Only watch configured roots; normalize/canonicalize paths; reject traversal outside roots.
  - Debounce and coalesce bursts; drop oldest with warnings under backpressure.
  - Include origin in events to suppress self-echo for a TTL (correlation id from write requests).
- Event type (cross-crate via ploke-core):
  - FileChangeEvent { path, kind, new_tracking_hash: Option<TrackingHash>, old_tracking_hash: Option<TrackingHash>, timestamp, origin: Option<Uuid> }
  - FileEventKind { Created, Modified, Removed, Renamed(from, to), PermissionChanged, Unknown }
- Ownership: ploke-io owns steady-state emission and lifecycle; consumers decide reactions (re-index, refresh UI, rerun lint).

4) Change Scanning
- Input: Vec<FileData>; Output: Vec<Option<ChangedFileData>> preserving input order.
- Behavior:
  - bounded concurrency via semaphore; compute fresh TrackingHash from tokens; compare with expected; return changed paths.
  - Potential optimization: buffer_unordered(limit) instead of spawning all and serially acquiring permits.
- Ownership: ploke-io owns the efficient and correct per-file verification.

Security, Path Policy, and Safety
- Path normalization: canonicalize within configured roots; reject relative paths or .. components that escape roots.
- Symlinks: default follow to target inside root; optionally deny crossing filesystem boundaries.
- Permissions: pre-check readable/writable; map PermissionDenied deterministically.
- Line endings/BOM: preserve as-is; no normalization unless explicitly requested by caller.
- Large files: start with in-memory splice; optionally feature-gate streaming edit for very large files later.

Concurrency Model and Limits
- Dedicated single-threaded tokio runtime in the actor OS thread; I/O workloads are async and bounded by semaphore.
- Semaphore limit:
  - default derived from rlimit (min(100, soft/3)) with env override PLOKE_IO_FD_LIMIT, clamped to a safe range.
- Per-file locks:
  - in-process: async mutex keyed by canonical path.
  - cross-process: OS advisory lock (flock/Windows). On lock failure, return deterministic error and/or retry with backoff (configurable).
- Fairness: ensure readers are not starved by writers—use queueing order or fair semaphore where applicable.

Error Model and Mapping
- ploke-io defines IoError variants for boundary checks, locking, atomic write steps, watcher failures, and shutdown.
- Mapping to ploke_error:
  - ContentMismatch → FatalError::ContentMismatch
  - FileOperation (read/write/rename/fsync) → FatalError::FileOperation
  - Utf8 → FatalError::Utf8
  - ParseError (hash computation) → FatalError::SyntaxError (or Internal if policy prefers)
  - Shutdown/Channel errors → InternalError::... (consistent workspace policy)
- Goal: keep end-user facing errors actionable; keep internals (e.g., lock contention details) primarily in logs.

APIs and Builder
- Handle: IoManagerHandle (Clone)
  - get_snippets_batch(Vec<EmbeddingData>) -> Result<Vec<Result<String, PlokeError>>, RecvError>
  - scan_changes_batch(Vec<FileData>) -> Result<Result<Vec<Option<ChangedFileData>>, PlokeError>, IoError>
  - write_snippets_batch(Vec<WriteSnippetData>) -> Result<Vec<Result<WriteResult, PlokeError>>, IoError> [new]
  - subscribe_file_events() -> broadcast::Receiver<FileChangeEvent> [new]
  - shutdown()
- Builder: IoManagerBuilder [new]
  - with_fd_limit(usize), with_semaphore_permits(usize)
  - with_roots(Vec<PathBuf>)
  - enable_watcher(bool).with_debounce(Duration)
  - with_cache_limits(bytes: usize, entries: usize)
  - with_locking_policy(enum)
  - build() -> IoManagerHandle
- Types (preferably in ploke-core):
  - WriteSnippetData, WriteResult, FileChangeEvent, FileEventKind, LockError variants if needed cross-crate.

Agent System Integration
- Agents call ploke-io through IoManagerHandle for reads/writes and subscribe to file events.
- Concurrency: multiple agents can target same files; per-file locks serialize writes; stale hashes raise ContentMismatch.
- Origin correlation id:
  - Each write carries origin: Uuid. The watcher tags events with the same origin to allow agent-side echo suppression.
- Reversibility:
  - ploke-io emits enough metadata (old/new hashes, deltas) for higher layers to stage diffs via git; actual commit/revert belongs to the git layer.
- In-chat editing and linting:
  - ploke-io provides read-on-demand and write atomics; linting orchestration runs above, with pre-apply checks calling read APIs for snapshotting and post-apply watchers to re-trigger analyzers.

Performance Considerations
- Caching:
  - Optional LRU caches for (path, mtime, size) → (bytes, tokens, hash) to avoid redundant reads/parses.
  - Invalidated by watcher signals or mtime change; bounded memory with metrics.
- Memmap (optional feature):
  - Consider memmap2 for read path on large files; validate UTF-8 slicing semantics carefully.
- Batch ordering:
  - Pre-allocate result vec by index to avoid collect-sort overhead.
- Backpressure:
  - Watcher uses bounded broadcast; on overload, coalesce and log.

Testing Strategy
- Unit tests: boundary checks (UTF-8, out-of-range), per-request hash verification, permission errors, zero-length edits, multi-byte Unicode.
- Integration tests:
  - large files, high concurrency, watcher debounce/coalesce, cross-process lock contention (where feasible).
  - atomicity fault injection: simulate failures between temp write and rename (best-effort).
- Property tests: splice correctness preserves valid UTF-8 and expected deltas.
- Performance tests: throughput under mixed read/write workloads; cache effectiveness.

Migration and Compatibility
- Maintain current read APIs; add write and watcher APIs behind a version bump or feature flag if needed.
- Align IoError → ploke_error mapping with workspace policy before shipping write/watcher features.
- Document path policy and environment overrides (PLOKE_IO_FD_LIMIT, etc.).

Non-Goals
- Git operations, AST-level transforms, lint execution, reranking, or UI orchestration—these belong to other crates.

Summary (Decision)
- Keep ploke-io responsible for low-level file safety (read/write/scan/watch), concurrency, atomicity, and correctness checks (UTF-8 and hash verification), with clean async APIs and strong observability.
- Push higher-level semantics (AST rewrites, lint orchestration, git commits, agent strategies) out of ploke-io, using events and typed results to integrate cleanly.
- Introduce write and watcher features with per-file locks and atomic rename workflow, using origin-based correlation to avoid echo.
- Provide a builder for configuration and optional bounded caches for performance while preserving minimal steady-state memory usage.
