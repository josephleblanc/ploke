# 2026-05-15 `NsPatch` Apply Missing Rescan Causes Stale-Index Retries

## Summary

In the non-semantic apply path, `ploke-tui` reports that a workspace rescan has
been scheduled after edits are applied, but it does not actually trigger the
rescan helper. Subsequent `ns_patch` proposals in the same live session can
keep targeting the pre-edit indexed state, which produces repeated
`NsContentMismatch` / "File content changed since indexing" failures against the
already-mutated file.

## Affected Surface

- `crates/ploke-tui/src/rag/editing.rs`
- broad-harness live headless TUI runs in `ploke-eval`

## Concrete Failure

Observed during live Prototype 1 broad-harness campaign
`p1-broad-harness-minimaxm25-20260515-7`, slot `node-ecba5b72929bd127-r6`.

The slot worktree became dirty in:

- `crates/ploke-tree/src/lib.rs`

After that first applied non-semantic change, later `ns_patch` calls against the
same file failed repeatedly with errors of the form:

- `File content changed since indexing: .../crates/ploke-tree/src/lib.rs`

The repeated failure is consistent with `ploke-io` rejecting a patch whose
`expected_file_hash` no longer matches the current file content.

## Root Cause

The non-semantic apply branch in `crates/ploke-tui/src/rag/editing.rs`:

- emits `"Scheduled rescan of workspace after applying edits"`
- persists proposals
- but does **not** call `rescan_for_changes(...)`

By contrast, the semantic apply path does call `rescan_for_changes(...)` after a
successful apply.

This leaves the live session operating on stale indexed content after an
`NsPatch` mutation, even though the system message implies the workspace was
refreshed.

## Why It Matters

- repeated `ns_patch` retries can never converge once they keep referencing the
  stale pre-edit file hash
- the operator sees terminal-flooding repeated failures
- broad-harness runs waste slot budget on retries that should have been
  invalidated immediately after the first apply

This gets much worse in headless Prototype 1 runs because `ploke-eval` currently
raises the headless TUI tool-call chain limit and repair limits far above normal
interactive defaults.

## Expected Behavior

After a successful non-semantic apply:

1. the workspace should actually be rescanned before the session continues, or
2. the session should stop accepting further same-turn `ns_patch` proposals
   against stale indexed state.

The system must not claim a rescan was scheduled when no rescan was actually
triggered.
