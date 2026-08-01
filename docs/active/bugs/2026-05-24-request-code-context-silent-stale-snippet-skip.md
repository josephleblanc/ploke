# `request_code_context` Can Hide Stale Snippet Failures

## Summary

`request_code_context` can plausibly report successful but incomplete context
when the database/file-hash state is stale after edits. The lower-level IO actor
does detect stale snippets as `ContentMismatch`, but RAG context assembly
defaults to non-strict IO and silently skips per-snippet IO failures. In a live
Prototype 1 loop this can look like the model simply failed to find relevant
code, when the real problem is that the retrieval path dropped stale snippets
without surfacing a tool-level stale-index failure.

## Current Status

Fixed at the model-facing retrieval boundary on 2026-05-25.

`ploke-rag` context assembly still defaults to lenient per-snippet IO, but it now
counts skipped snippet IO failures in `ContextStats::skipped_io_errors`.
`request_code_context` uses that count to return an explicit degraded-context
note and repair steps when stale snippets are skipped. A deliberately stale
fixture DB hash now proves that lenient assembly does not silently look clean:

```text
cargo test -p ploke-rag lenient_context_reports_skipped_stale_snippet_io --lib
cargo test -p ploke-tui stale_snippet_skips_are_model_visible_degraded_context --lib
```

Remaining risk: this does not by itself guarantee post-edit reindex freshness or
solve same-file proposal composition. It makes stale snippet loss visible to the
model/operator instead of only logged.

This is a separate follow-up to the earlier `NsPatch` post-apply rescan bug:
the non-semantic apply path now does trigger the rescan helper, but we still do
not have an end-to-end freshness witness proving that post-edit
`request_code_context` sees the current file state or reports stale state
explicitly.

## Affected Surface

- `crates/ploke-tui/src/tools/request_code_context.rs`
- `crates/ploke-rag/src/context/mod.rs`
- `crates/ploke-rag/src/core/mod.rs`
- `crates/ploke-io/src/actor.rs`
- `crates/ploke-tui/src/rag/editing.rs`
- `crates/ploke-tui/src/app_state/database.rs`
- Prototype 1 broad-harness/headless TUI runs that rely on follow-up context
  after an applied edit

## Current Evidence

`request_code_context` calls `rag.get_context(...)` directly:

- `crates/ploke-tui/src/tools/request_code_context.rs`

RAG context assembly defaults to lenient IO:

- `crates/ploke-rag/src/context/mod.rs`
  - `AssemblyPolicy::default()` sets `strict_io: false`.
  - When `io.get_snippets_batch(...)` returns a per-snippet error, the
    non-strict path logs `Skipping snippet ... due to IO error` and continues.

The IO actor does detect the stale file-hash condition:

- `crates/ploke-io/src/actor.rs`
  - `get_snippets_batch` compares the current `TrackingHash` against the
    database-provided `file_tracking_hash`.
  - On mismatch, it returns `IoError::ContentMismatch` for that request.

That means the stale-hash invariant exists at the IO boundary, but the
model-facing retrieval tool can currently turn the invariant violation into
missing context rather than a failed or explicitly degraded tool result.

The post-apply rescan path is also not a full freshness witness:

- `crates/ploke-tui/src/rag/editing.rs`
  - semantic and non-semantic approval paths now await `rescan_for_changes`.
- `crates/ploke-tui/src/app_state/database.rs`
  - `scan_for_change` emits `SystemEvent::ReIndex`, then sends changed files
    through the scan oneshot.
  - The function still has `TODO: Add validation step here.`

Current tests cover narrower signals:

- `ns_patch_approval_triggers_rescan_helper` proves the helper was invoked and
  the file changed, but not that `request_code_context` returned fresh content.
- `post_apply_rescan` checks for a SysInfo message containing scheduled/refreshed
  wording, not DB/RAG freshness.
- `semantic_approval_refreshes_file_hash_before_returning` is stronger for exact
  graph lookup after semantic approval, but it still does not exercise the
  `request_code_context` tool path.

## Git History Notes

The history looks like real progress layered on top of partial fixes:

- `7afa9055` added namespace patch write support with serious gaps: no per-file
  lock, direct `mpatch::apply_patch_to_file`, and the ns-patch write result
  returned the pre-write hash.
- `42101063` made real write-path progress by moving ns patch application to
  content preflight plus temp-write/fsync/rename, and by computing the returned
  file hash from `patch_result.new_content`.
- `e17e65b4` added rescan-helper plumbing, but it was still fire-and-forget and
  the regression only proved the helper was scheduled/counted.
- `e5699350` made approval await `rescan_for_changes`, which is meaningful
  progress, but the scan path still lacks a downstream validation step proving
  the context tool will see fresh snippets.
- `67f7894d` decoupled sparse context assembly from embedding rows. That is
  useful for sparse retrieval, but it makes the snippet-read path more important
  because valid sparse hits can still be dropped later by lenient snippet IO.

## Why It Matters

Prototype 1 is trying to collect evidence that repeated generations can
hill-climb on benchmark performance. If context retrieval silently drops stale
snippets, run traces can misattribute failures to model behavior or benchmark
difficulty when the actual failure is an indexing/freshness bug.

This also weakens run review: a tool call can appear to have completed
successfully while failing to retrieve code that exists in the target checkout.

## Expected Behavior

After an accepted edit changes a file, a follow-up `request_code_context` call
should do one of the following:

1. return snippets from the current post-edit file state, or
2. return a structured stale-index/tool failure that tells the model and
   operator to refresh/retry, or
3. return a successful result with explicit degraded-context diagnostics naming
   skipped stale snippets.

It should not silently succeed while omitting matching snippets because the DB
hash no longer matches the file.

## Regression Tests To Add

Add tests at the model-facing boundary:

1. semantic edit -> approve -> `request_code_context` for the edited item ->
   assert the returned snippet contains post-edit text.
2. ns patch edit -> approve -> `request_code_context` for the edited file/item
   -> assert the returned snippet contains post-edit text.
3. deliberately stale DB hash -> `request_code_context` -> assert the tool
   returns an explicit stale-index/degraded-context result instead of a clean
   success with silently skipped snippets.

The third test is the critical counterexample: it should fail if
`ContentMismatch` is only logged under `strict_io: false`.

## Related Reports

- [`2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md`](./2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md)
- [`2026-05-22-prototype1-google-post-apply-indexing-timeout.md`](./2026-05-22-prototype1-google-post-apply-indexing-timeout.md)
