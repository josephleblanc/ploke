# Preconditions and Design Assessment for Editing and File-Watching Features

Date: 2025-08-18
Crate: ploke-io

Overview
This document outlines the preconditions, design choices, and responsibilities necessary to implement:
1) File editing via snippet-based writes with integrity checks and robust behavior when new content is shorter/longer than the target byte range.
2) File watching that emits typed events for downstream consumers.
It also surveys what ploke-io should own to support a multi-agent framework concurrently reading/writing in the same workspace, assuming git integration elsewhere in the ploke project.

Current Context
- ploke-io provides a non-blocking actor with bounded concurrency and content verification via token-based TrackingHash.
- Batch snippet reads validate the whole-file TrackingHash and enforce UTF-8-safe slicing.
- Error handling bridges to the shared ploke-error types; some alignments remain pending.
- No write operations or watchers exist yet; adding them must preserve actor isolation and correctness.

1) File Editing Feature (Snippet-Based Write)
Goal
Accept a request structurally similar to EmbeddingData, plus the new snippet content, and write it into the specified byte range of a file.

Core Preconditions
- Addressing policy: Decide and standardize addressing semantics for edits.
  - Byte range with UTF-8 boundary enforcement (status quo for reads).
  - Content-defined addressing by node_tracking_hash (stronger resilience to nearby edits).
  - Recommended: Require both for safety:
    - File-level content verification using file_tracking_hash for the current on-disk content.
    - Node-level anchoring via node_tracking_hash or a content “anchor” (start/end tokens or sentinel lines).
- Integrity checks:
  - Must compute the actual file TrackingHash once per target file and compare it per-request (not only against the first request).
  - If anchoring by byte range, enforce:
    - 0 <= start <= end <= file_len
    - is_char_boundary(start) and is_char_boundary(end)
  - If anchoring by node/content signature, resolve anchors to a concrete byte range before write; fail if not uniquely resolvable.
    - USER NOTE: We are planning to implement a second kind of NodeId, `CanonId`, which will not depend on the byte range of the node contents in the target files, but rather on: crate_namespace, file_path, logical_item_path (e.g. canonical path), item_kind (e.g. function, struct), cfg (cfg attributes).
- Path policy and permissions:
  - Reject relative paths or paths containing ..; optionally require canonicalized absolute paths within a repository root.
  - Pre-check read/write permissions; map to deterministic ploke-error variants.
- Concurrency and locking:
  - Require a cross-process locking strategy before writes. Options:
    - Advisory file lock (flock on Unix, CreateFile with exclusive sharing on Windows).
    - Lock-file per path at .ploke/locks/<hash(path)>.lock to standardize across platforms.
  - Within the Io actor, also guard per-path with an async mutex (per-process mutual exclusion).
- Atomic write strategy:
  - Read entire file into memory, apply validated splice, write the new file atomically:
    - Write to temporary path (same directory) with 0600 perms.
    - fsync temp file, then rename over the original (atomic on most platforms).
    - fsync parent directory (POSIX) to persist rename; on Windows fall back to best-effort.
  - This avoids partial reads by other processes and helps watchers produce coherent events.
- Git integration boundary:
  - ploke-io should not commit; it should expose hooks and event metadata so upstream can stage/commit via the git crate.
  - Optionally include a “write origin” correlation ID to suppress self-notifications in the watcher.
- Error policy:
  - Maintain IoError -> ploke_error mapping consistency.
  - Detect and return ContentMismatch if the file changed (current hash != expected).
  - Return Utf8 and boundary errors for invalid ranges.
  - On lock acquisition failure or shutdown, return deterministic errors.

Handling Snippet Length Mismatch
- New snippet shorter than target range:
  - Byte-oriented splice: content_before + new_snippet + content_after.
  - This shifts subsequent bytes; callers relying on prior absolute ranges must refresh positions. Provide the new file TrackingHash in response so callers can re-derive positions.
  - Optional: return a delta of the change (start, removed_len, added_len) to help upstream adjust cached offsets.
- New snippet longer than target range:
  - Same splice logic; ensure resulting content remains valid UTF-8.
  - Maintain char-boundary correctness at splice boundaries (both ends).
  - If downstream systems depend on stable positions, they must re-query after write.
- Optional resilience upgrade:
  - Content-anchored edits: locate node boundaries via node_tracking_hash/tokens; this reduces accidental misplacement when nearby text shifted since indexing (as long as the node hash still matches).

Suggested Write Request Type (sketch)
- WriteSnippetData:
  - file_path: PathBuf
  - file_tracking_hash: TrackingHash
  - node_tracking_hash: TrackingHash (or optional anchor spec)
  - start_byte: usize
  - end_byte: usize
  - new_snippet: String
  - id: Uuid
  - name: String
  - namespace: Uuid
  - origin: Option<Uuid> // correlation ID to suppress watcher echo
- Behavior:
  - Validate, lock, verify hash, splice, atomic write, compute new TrackingHash, emit watcher event with origin, return new hash + optional delta.

2) File Watching Feature (Event Emission)
Goal
Emit typed events when files change, to trigger re-indexing or reconcile conflicting writes.

Core Preconditions
- Dependency: Use notify crate (modern version) for cross-platform watching.
- Runtime integration:
  - Run the watcher inside the IoManager actor runtime thread to simplify concurrency and lifecycle.
  - Provide a subscription API (tokio::sync::broadcast is a good fit) that allows multiple consumers.
- Event model and trait:
  - Define a domain-level event type usable across crates:
    - enum FileEventKind { Created, Modified, Removed, Renamed(PathBuf, PathBuf), PermissionChanged, Unknown }
    - struct FileChangeEvent { path: PathBuf, kind: FileEventKind, new_tracking_hash: Option<TrackingHash>, old_tracking_hash: Option<TrackingHash>, timestamp: SystemTime, origin: Option<Uuid> }
  - Trait bound example for consumers:
    - trait HandlesFileEvent: Send + Sync { fn on_file_event(&self, evt: &FileChangeEvent); }
  - Note: Tracking hashes are optional because not all events warrant reads/parsing; avoid heavy work in the watcher thread—defer to consumers or a separate batching task.
- Debounce and coalescing:
  - Filesystems can emit bursts; add configurable debounce and de-duplication windows.
  - Preserve last event per path within window; coalesce Renamed sequences when possible.
- Correlation with internal writes:
  - Include origin Uuid on edits; watcher filters events with same origin for a short TTL to avoid echo loops.
- Backpressure:
  - Use bounded channels to avoid unbounded memory; drop oldest or coalesce aggressively under load while logging warnings.
- Security and path policy:
  - Enforce same path normalization rules as writes/reads; restrict to configured roots.
- Shutdown:
  - Gracefully stop watcher on IoManager shutdown.

3) Multi-Agent Concurrency in Same Workspace
Assumptions
- Multiple agents/processes may concurrently read/write; ploke has git integration handled by another crate.
- We need consistency, atomicity, and a predictable conflict model.

What ploke-io Should Own
- Cross-process and in-process locking:
  - Adopt a lock hierarchy:
    - Per-file lock (advisory OS lock + in-process async mutex).
    - Optional global write gate for high-stakes operations (rare).
  - Lock ordering by canonicalized path to avoid deadlocks across multi-file edits.
- Atomic write discipline:
  - Temp file + fsync + atomic rename + fsync(parent).
  - Emit watcher events with origin to allow reconcilers to ignore self or to aggregate related changes.
- Hash verification and conflict detection:
  - On every write, require expected file_tracking_hash to match current.
  - If mismatch, fail with ContentMismatch and include current hash so caller can rebase.
- Version metadata:
  - Return new TrackingHash post-write; optionally include a monotonically increasing version (logical clock) persisted in extended attributes or a sidecar DB if desired by higher-level orchestration.
- Integration points for git:
  - Provide hooks/callbacks or just typed events so the git integration layer can stage/commit.
  - Optional “pre-commit preview” mode: dry-run edit that produces a diff without applying, for agent negotiation.
- Read/Write fairness and starvation:
  - Readers should not be blocked indefinitely by write bursts; use semaphore + fair locks or queueing to ensure progress.
- Observability:
  - Trace spans for write/lock operations.
  - Emit structured events for lock contention, slow I/O, and coalesced watcher drops.

What ploke-io Should Not Own
- Git branching/merging/commit policies, conflict resolution, or repository-level transactions.
- Higher-level diff/patch semantics beyond validated splice writes; richer AST-aware rewriters belong in other crates.

Atomic Edit Workflow (Recommended)
1) Normalize and validate path; acquire per-file lock (OS + async).
2) Read file bytes once; check UTF-8 decode.
3) Compute actual TrackingHash; compare with expected.
4) Resolve final byte range (from anchors or given offsets); enforce char boundaries.
5) Apply splice in memory; validate UTF-8.
6) Write to temp file; fsync; atomically rename; fsync parent directory.
7) Compute new TrackingHash; release locks.
8) Emit FileChangeEvent with origin; return new hash and delta.

Edge Cases and Policies
- Zero-length files or zero-length edits:
  - Allow empty insertions (start == end) and deletions (new_snippet == "").
- CRLF vs LF normalization:
  - Decide policy: preserve existing line endings; do not normalize unless explicitly requested.
- Permissions and read-only files:
  - Surface PermissionDenied with path; do not attempt chmod.
- Large files:
  - For very large files, a streaming splice would avoid full buffering; start with in-memory splice for simplicity and gate streaming behind a feature later.
- Symlinks:
  - Decide whether to follow symlinks; default: resolve and operate on target, but deny edits across filesystem boundaries if policy forbids it.

Minimum Viable API Sketches
- IoRequest::WriteSnippetBatch { requests: Vec<WriteSnippetData>, responder: oneshot::Sender<Vec<Result<WriteResult, PlokeError>>> }
- WriteResult { new_file_tracking_hash: TrackingHash, delta: (usize /*start*/, usize /*removed_len*/, usize /*added_len*/) }
- Subscription:
  - IoManagerHandle::subscribe_file_events() -> broadcast::Receiver<FileChangeEvent>

Testing Plan (Essentials)
- Happy-path edit: shorter and longer replacements; verify re-read matches expected; watch event received.
- ContentMismatch on stale hash.
- Boundary errors for invalid UTF-8 slicing.
- PermissionDenied path.
- Concurrency: two writers targeting same file; second should block then apply or fail based on hash mismatch.
- Watcher echo suppression using origin.
- Atomicity: simulate crash between temp write and rename (fault injection); ensure original remains intact.

Open Questions for Product/Architecture
- Should IO verify node-level hashes on write, or should that belong to higher layers?
  - IO should verify, report errors upstream with ctx
- Should we support a patch/diff API instead of splice by offsets?
- Do we allow policy-driven normalization (formatting, newline, BOM) at IO-layer, or leave it to higher layers?
- Do we need a global transaction spanning multiple files (two-phase commit) for coordinated agent edits?

Short-Term Implementation Steps (Suggested)
- Add per-request hash verification in current read path (already planned).
- Introduce origin/correlation ID concept in request types and event model.
- Implement per-file async mutex registry and OS-level advisory locks.
- Add WriteSnippetData and IoRequest::WriteSnippetBatch, using atomic write workflow.
- Integrate notify-based watcher with broadcast subscriptions and coalescing.
- Ensure IoError/PlokeError mappings include consistent variants for lock/rename/fsync failures.

Conclusion
With strict path normalization, per-request hash verification, atomic rename writes, and a typed watcher emitting debounced events, ploke-io can safely support snippet writes and multi-agent concurrency. ploke-io should own low-level consistency (locking, atomicity, hash checks, event emission) and expose well-typed APIs for higher layers to orchestrate git operations, AST-aware edits, and conflict policies.
