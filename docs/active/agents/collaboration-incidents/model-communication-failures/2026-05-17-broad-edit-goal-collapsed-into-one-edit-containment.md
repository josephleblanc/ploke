# 2026-05-17 broad edit goal collapsed into one-edit containment

## Trigger

The user rejected a proposed `ploke-eval` containment strategy for headless
TUI edits and said the agent did not understand the goal. The user clarified
that the objective is not one safe edit per attempt, but giving the model broad
freedom over as much of the codebase as possible in a relatively safe way.

## User-visible failure

The agent kept reducing the mechanism to "stop after one mutation" or a narrow
edit barrier. That response protected against stale staged proposals, but it
worked against the product goal: broad autonomous editing with safety rails,
batch admission, and reliable post-apply evidence.

## Touched code surface

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
- `crates/ploke-tui/src/rag/editing.rs`
- `docs/active/bugs/2026-05-17-headless-tui-staged-proposal-tool-result-lifecycle.md`
- `docs/active/bugs/2026-05-17-headless-tui-same-file-ns-patch-stale-anchor-retries.md`

No Rust code was changed during the failed explanation.

## What the agent did

The agent over-indexed on immediate containment and under-modeled the intended
workflow. It treated repeated tool calls after a staged patch as waste to stop,
instead of asking how to preserve a long, exploratory model turn while keeping
workspace mutation and evidence admission coherent.

## Skipped or overreached

- Skipped restating the core objective before proposing mechanics:
  broad edit freedom with safe admission.
- Overreached by collapsing the desired batch/edit-session semantics into a
  one-edit terminal outcome.
- Failed to distinguish model exploration freedom from workspace mutation
  admission. The model may need to inspect and plan broadly even if applied
  mutations must pass a stricter gate.

## Why this was risky

A one-edit containment strategy would make the broad harness much less useful.
It would reduce stale-anchor risk by cutting off exactly the behavior the
system is meant to study: multi-file, multi-step autonomous repair over a large
surface. It also risks teaching future agents to solve safety by shrinking the
task instead of preserving the task and adding the right admission boundary.

## Prevention rule

For broad-harness edit-surface work, state the objective before proposing a
gate:

```text
model freedom over a broad code surface -> staged workspace proposals ->
eval admission of allowed/disjoint mutations -> post-apply refresh evidence ->
only refreshed/applied workspace evidence counts for validation
```

Do not propose "stop after one edit" as the default fix unless the user asks
for a single-edit harness. Prefer batch admission and evidence barriers that
preserve broad model autonomy.

## Memory hypothesis

Recent stale-anchor failures bias the agent toward narrow containment. That is
useful for preventing loops, but it can erase the broader Prototype 1 goal of
testing broad autonomous edit behavior under controlled admission.
