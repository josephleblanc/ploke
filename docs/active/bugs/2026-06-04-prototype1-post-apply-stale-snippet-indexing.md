# Bug: Prototype 1 post-apply indexing can request stale snippets

## Status

Fixed in source on 2026-06-04 with recorded replay/tape regression coverage.
A fresh live `prototype1-state` run is still needed to prove the warning is gone
from campaign logs under real provider timing.

The regression test is:

```text
cargo test -p ploke-eval recorded_replay_truncating_patch_removes_stale_snippet_rows -- --nocapture
```

It passed locally after the fix.

## Broken Contract

After a headless TUI edit applies and post-apply refresh completes, changed-file
descendants that no longer exist in the candidate workspace must be removed from
the database before indexing asks IO for snippets.

Indexing must not emit stale-snippet warnings like:

```text
WARN ploke_embed::indexer: Snippet error: Fatal(ContentMismatch { name: "assemble_run_forest", ... })
WARN ploke_embed::indexer: Snippet error: Fatal(FileOperation { operation: "read", ... error: "Byte range ... exceeds file length 296" })
```

Those warnings mean the DB still contains rows whose file hash or byte spans
belong to the pre-edit file state.

## Evidence

Observed campaign:

```text
/home/brasides/.ploke-eval/campaigns/p1-g31pro-p25flash-10g5c-a2-patch5-eval3-explore-20260604-141932
```

Observed child request:

```text
prototype1/messages/edit-harness-request/node-a987f5c9318791d5-r2.json
```

The request targeted this candidate workspace:

```text
prototype1/workspaces/edit-harness/node-a987f5c9318791d5-r2
```

That workspace contained a dirty truncating edit:

```text
M crates/ploke-tree/src/lib.rs
M crates/ploke-tree/src/tests.rs
```

The affected `tests.rs` file had been shortened to 296 bytes while stale DB
rows still carried byte spans from the previous, much larger file. The warnings
then came from snippet extraction for rows that should have been retracted before
post-apply indexing.

The request-declared submitted result path for `r2` was absent, so this report
does not claim a persisted `r2` result/tape exists for exact replay of that
slot. A later slot in the same campaign did preserve replayable headless
artifacts, but this bug's minimal regression uses a controlled recorded tape
that reproduces the same stale-row shape.

## Source Trace

The warning is logged per stale snippet request in:

```text
crates/ingest/ploke-embed/src/indexer/mod.rs
```

The IO actor detects the two concrete stale states:

```text
crates/ploke-io/src/actor.rs
```

It returns `ContentMismatch` when the current file tracking hash differs from
the DB row's expected hash, and it returns a `FileOperation` error when the DB
row's byte range no longer fits inside the current file.

The post-apply refresh path is:

```text
headless TUI tape/live edit
-> apply non_semantic_patch
-> settle_staged_batch
-> wait_for_refresh
-> scan_for_change_target
```

The pre-fix scan path removed embedded vectors for changed files but did not
remove all stale descendant rows before transform/indexing. That left deleted or
truncated symbols available for later snippet extraction.

The source fix is in:

```text
crates/ploke-tui/src/app_state/database.rs
crates/ploke-db/src/database.rs
```

The DB now retracts changed-file descendants, including associated method rows,
before the refreshed parsed graph is transformed and indexed.

## Repro Coverage

The recorded replay regression is:

```text
crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs
```

Test:

```text
recorded_replay_truncating_patch_removes_stale_snippet_rows
```

The test:

1. creates a real headless TUI canary workspace;
2. overwrites `src/lib.rs` with a source file containing `stale_index_canary`;
3. lets the runtime index that function row;
4. installs a recorded provider response tape;
5. runs the production `run_attempt` path;
6. applies a recorded `non_semantic_patch` that removes `stale_index_canary` and
   shortens `src/lib.rs`;
7. asserts the attempt reaches `HeadlessTerminal::Applied`;
8. asserts the final file no longer contains `stale_index_canary`;
9. asserts the database no longer contains a `stale_index_canary` function row
   after post-apply refresh.

This is intentionally an upstream-state assertion. It does not capture the WARN
line directly; it proves the stale DB row that would make the indexer request a
stale snippet is retracted before indexing can reach that bad state.

## Missing Validation

Remaining validation before closing the live-run risk:

```text
Run a fresh live prototype1-state campaign with truncating edits or ordinary
child patch generation, then inspect the campaign logs for absence of
ploke_embed::indexer Snippet error: Fatal(ContentMismatch ...) and byte-range
FileOperation warnings.
```

The fix should not suppress the warning, make IO lenient, or ignore snippet
errors. The warning is valid when stale rows reach IO. The durable fix is to
prevent stale rows from surviving post-apply refresh.

## Related Reports

- [`2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md`](./2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md)
- [`2026-05-24-request-code-context-silent-stale-snippet-skip.md`](./2026-05-24-request-code-context-silent-stale-snippet-skip.md)
- [`2026-05-22-prototype1-google-post-apply-indexing-timeout.md`](./2026-05-22-prototype1-google-post-apply-indexing-timeout.md)
