# `ploke-io`

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


`ploke-io` centralizes filesystem effects behind an actor-style service running on a dedicated Tokio runtime thread. It supports hash-verified snippet reads, change scans, full-file reads, file creation, and atomic snippet writes with path-policy enforcement.

## Responsibilities

- Spawn an I/O actor and expose a cloneable `IoManagerHandle`.
- Enforce bounded concurrency from OS file-descriptor heuristics or explicit builder settings.
- Normalize paths against configured roots and symlink policies.
- Read UTF-8 safe snippets with content-hash verification.
- Scan files for tracking-hash changes.
- Apply atomic writes through temp-write/fsync/rename and serialize same-file writes.
- Optionally broadcast watcher events when built with the `watcher` feature.

## Key Files

- [`crates/ploke-io/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-io/src/lib.rs) — crate docs, public exports, request/response types.
- [`crates/ploke-io/src/handle.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-io/src/handle.rs) — `IoManagerHandle`, public async methods, builder entry point.
- [`crates/ploke-io/src/actor.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-io/src/actor.rs) — `IoManager`, `IoManagerMessage`, `IoRequest`, actor event loop and request dispatch.
- [`crates/ploke-io/src/read.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-io/src/read.rs) — full-file/snippet read helpers.
- [`crates/ploke-io/src/write.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-io/src/write.rs) — atomic snippet and namespace diff write helpers.
- [`crates/ploke-io/src/path_policy.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-io/src/path_policy.rs) — root containment and symlink policy logic.

## Public API

- `IoManagerHandle::new()` and `IoManagerHandle::builder()` — start the actor with defaults or custom root/concurrency/watcher config.
- `get_snippets_batch(requests)` — ordered batch of hash-verified snippet reads.
- `read_full_verified(file_path, expected_hash, namespace)` — read and verify whole-file content.
- `read_file(ReadFileRequest)` — workspace file read with path policy and truncation limits.
- `scan_changes_batch(requests)` — report changed files by recomputed tracking hash.
- `write_snippets_batch` / namespace write variants — apply atomic edits.
- `update_roots` and watcher subscription methods when the feature is enabled.

## Internal Structure

`IoManagerHandle::new` creates an `mpsc` channel, spawns a thread, builds a current-thread Tokio runtime, and runs `IoManager::run`. The actor receives `IoManagerMessage::{Request, UpdateRoots, Shutdown}`. Each request type carries a `oneshot` responder, so callers await a single structured result while the actor controls concurrency, path policy, and spawn boundaries.

## Dependencies

- **Uses:** `tokio`, `futures`, `quote`, `syn`, `ploke-core`, `ploke-error`, filesystem APIs, optional `notify` watcher support.
- **Used by:** `ploke-tui` tools, `ploke-embed` indexing, and `ploke-rag` snippet assembly.

## Notable Patterns / Gotchas

- When roots are configured, paths must be absolute and contained by root policy; `DenyCrossRoot` is the default when roots are set.
- `ReadFileRequest.range` is currently acknowledged in actor code, but server-side slicing is still marked TODO in the read-file path.
- Writes to the same file are serialized in-process; do not bypass this with direct filesystem writes in tool code.
