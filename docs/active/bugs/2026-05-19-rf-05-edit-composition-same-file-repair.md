# RF-05 Edit Composition And Same-File Repair Failure

## Summary

Repeated headless TUI edits against the same file can pass through multiple
proposal and repair cycles without a strong composition, invalidation, or
rejection boundary. In observed broad-harness traces, this allowed stale or
partial non-semantic patch attempts to create malformed intermediate Rust before
later attempts either repaired the file or left a broken candidate artifact.

This is the active bug report for taxonomy item RF-05 in
[`replay-regression-failure-taxonomy.md`](../agents/run-reviews/2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/replay-regression-failure-taxonomy.md).

## Affected Surface

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-tui/src/rag/editing.rs`
- `crates/ploke-io` non-semantic patch application
- Prototype 1 broad-harness headless TUI runs

## Concrete Failure Modes

- Same-file repair loops can leave malformed Rust in the candidate workspace.
- Partial non-semantic patches can require multiple repair attempts, increasing
  the chance of stale anchors, duplicate fragments, or restore markers.
- A trace may eventually compile after churn, but the system lacks an invariant
  that prevents malformed intermediate states from becoming committed
  artifacts.

## Observed Examples

Observed in the `p1-smoke-broad-harness-1x3-20260519-1` trace review:

- `node-a87394840086768d-r2` left malformed `fs.rs` with restore markers,
  missing declarations, and delimiter errors.
- `node-a87394840086768d-r7` repaired `fs.rs` after many proposal events before
  eventually passing visible checks.
- `node-81bd26e4b6222d08-r6` repaired a new `headless_runtime.rs` after
  duplicate definitions, an unclosed delimiter, and partial patch failure.
- `node-81bd26e4b6222d08-r3` had 16 proposal events around one module file and
  repeated compile failures.

## Existing Related Bugs

- [`2026-04-18-multi-edit-apply-result-accounting.md`](./2026-04-18-multi-edit-apply-result-accounting.md)
  tracks same-file multi-edit semantics and per-edit result accounting inside a
  single proposal.
- [`2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md`](./2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md)
  tracks missing rescan after non-semantic patch apply.
- [`2026-05-17-headless-tui-same-file-ns-patch-stale-anchor-retries.md`](./2026-05-17-headless-tui-same-file-ns-patch-stale-anchor-retries.md)
  tracks stale same-file proposal retries after a prior proposal has already
  changed the file.
- [`2026-05-17-headless-tui-staged-proposal-tool-result-lifecycle.md`](./2026-05-17-headless-tui-staged-proposal-tool-result-lifecycle.md)
  tracks staged proposal success being replayed to the model before apply
  outcome is settled.

RF-05 is narrower than "same-file edits are risky" and broader than one stale
hash rejection. The missing invariant is that repeated same-file repair attempts
must settle into one coherent candidate state before the artifact can be
submitted or materialized.

## Regression Coverage

The fixed-contract replay test is:

```text
cargo test -p ploke-eval recorded_replay_rejects_stale_same_file_repair_after_first_apply -- --ignored --nocapture
```

Tracked marker:

```text
regr:samefile:19-05-26_06-42
```

Location:

```text
crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs
```

The provider tape asks for one valid `non_semantic_patch` edit and then a stale
same-file repair against the pre-apply content. The fixed behavior is:

1. the first edit applies;
2. the stale repair fails before staging or applying a second proposal;
3. the next provider request receives a rejection for the stale repair call;
4. the final workspace remains at the first valid edit and contains no repair
   artifacts.

The test is currently expected-failing because the stale same-file repair still
applies as a second proposal instead of being rejected or invalidated.

## Expected Behavior

Repeated same-file edits in a headless attempt should be handled by one of these
settled paths:

1. compose the same-file edits into a single safe write plan before apply;
2. invalidate stale same-file repair attempts and require a fresh proposal after
   rescan;
3. reject stale or partial same-file repairs before they can stage, apply, or be
   counted as a candidate artifact.

The fixed contract is not that the model may only make one edit. The contract is
that broad model autonomy cannot materialize malformed same-file churn as an
accepted candidate state.

## Investigation Anchors

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
  - `recorded_replay_rejects_stale_same_file_repair_after_first_apply`
  - `run_attempt`
  - proposal application and repair-turn handling
- `crates/ploke-tui/src/rag/tools.rs`
  - `apply_ns_code_edit_tool`
  - non-semantic patch staging and proposal creation
- `crates/ploke-tui/src/rag/editing.rs`
  - proposal approval, apply status, and rescan behavior
- `crates/ploke-io`
  - non-semantic diff application and file hash expectations
