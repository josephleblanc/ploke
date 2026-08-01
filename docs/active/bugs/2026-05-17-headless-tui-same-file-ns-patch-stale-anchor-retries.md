# 2026-05-17 Headless TUI Same-File `NsPatch` Stale Anchor Retries

## Summary

Prototype 1 headless TUI runs can repeatedly try to apply multiple
non-semantic patch proposals against the same file after an earlier proposal has
already changed that file. Each proposal carries the file hash observed when it
was staged. Once the first proposal mutates the file, later proposals against
the same path are anchored to stale content and `ploke-io` correctly rejects
them with `File content changed since indexing`.

This is a follow-on to
[`2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md`](./2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md).
The original bug was that `ploke-tui` did not actually rescan after a successful
non-semantic apply. The current failure has been observed in a checkout that
already contains that rescan fix.

## Affected Surface

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
- `crates/ploke-tui/src/rag/editing.rs`
- `crates/ploke-tui/src/rag/tools.rs`
- broad-harness live headless TUI runs in `ploke-eval`

## Concrete Failure

Observed during live Prototype 1 campaign
`p1-protocol-two-target-20260517-1`, parent worktree:

```text
/home/brasides/.ploke-eval/campaigns/p1-protocol-two-target-20260517-1/prototype1/workspaces/edit-harness/node-a30cc6cf0aaf9c60
```

The active worktree was based on:

```text
4934f5c1 prototype1: initializing gen 0 parent node-a30cc6cf0aaf9c60
94118e6c Add profiling and oracle evaluation support
```

The worktree was dirty in:

```text
crates/ploke-rag/src/lib.rs
```

The live stderr repeatedly alternated two request ids:

```text
44dd6d0a-215b-4e03-b6ad-b6fa476171dc
5712cf4c-216e-4858-92da-0c3ff51be3e6
```

Both failed against:

```text
crates/ploke-rag/src/lib.rs
```

with:

```text
File content changed since indexing
```

The dirty diff already showed the file had been changed from the indexed
version:

```diff
-const BM25_TIMEOUT_MS: u64 = 250;
+const BM25_TIMEOUT_MS: u64 = 50;
+
 const BM25_RETRY_BACKOFF_MS: [u64; 2] = [50, 100];
+
```

## Git History Evidence

The earlier stale-index bug was fixed by:

```text
e17e65b4 x
```

which added a non-semantic apply rescan path in
`crates/ploke-tui/src/rag/editing.rs`.

The later headless orchestration changes that exposed this failure mode are:

```text
4af3e2fd prototype1: apply broad harness edits after turn completion
a6264381 prototype1: apply allowed harness edits during turn
```

Those commits changed the headless adapter from one applied proposal ending the
attempt into an adapter that can accept and apply multiple proposals inside the
same attempt, then continue from the mutated workspace.

## Root Cause

`ploke-tui` correctly records an `expected_file_hash` when staging an
`NsWriteSnippetData`. `ploke-io` correctly rejects a write if the file no
longer matches that hash.

The missing guard is in the headless adapter. It tracks applied edits by
proposal id, not by touched file and staged file version. That lets distinct
proposal ids for the same path remain actionable after the first proposal has
mutated the file.

The failure is especially likely in headless repair flows because the adapter
can continue the same attempt after an applied edit or failed apply, and the
model can emit another `ns_patch` against the same file using stale context.

## Existing Test Evidence

`crates/ploke-tui/src/tools/tool_tests/patches.rs` already contains regression
coverage for the lower-level invariant: two same-file `ns_patch` proposals
staged against the same input hash cannot both be applied after the first one
changes the file.

That test proves the TUI/IO layer is enforcing the stale-anchor rule. The
missing coverage is at the `ploke-eval` headless adapter layer: once an allowed
proposal mutates a path, later same-path proposals in the same attempt should
not be treated as still admissible candidate work.

## Expected Behavior

After an allowed proposal applies in a headless attempt, the adapter should
prevent stale same-path proposals from being applied later in that same attempt.
Acceptable outcomes include:

1. end the attempt after the first applied source candidate;
2. mark pending same-path proposals stale/denied and require a new staged
   proposal after rescan; or
3. model touched-path/version state explicitly in the adapter and only admit a
   later proposal if it was staged against the post-apply file version.

The adapter must not repeatedly retry a proposal whose `expected_file_hash`
cannot match the current file content.

## Investigation Anchors

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
  - `run_attempt`
  - `apply_edit`
  - repair-turn handling around `pending_retry` and applied edits
- `crates/ploke-tui/src/rag/editing.rs`
  - non-semantic apply status and post-apply rescan
- `crates/ploke-tui/src/rag/tools.rs`
  - `NsWriteSnippetData` staging and `expected_file_hash`
- `crates/ploke-tui/src/tools/tool_tests/patches.rs`
  - same-file stale-anchor regression tests
