# Prototype 1 Foreground Child Recovery Misses Parent Comparison

Status: fixed in source; retained as regression guard and live-run evidence.

Original campaign:
`p1-gemini35-flash-direct-15g2x3-par2-20260526-051954`.

Second reproducing campaign:
`p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-spawnfix-20260608-081204`.

## Broken Contract

When a child runtime produces terminal treatment evidence, the parent must run
the observe/comparison transition before successor selection. A succeeded child
without `prototype1/evaluations/<branch-id>.json` is not selection-grade
evidence and must not be converted into an implicit reject.

That leaves a mechanically succeeded child without decision-grade
`SelectionInput`, so History traversal returns `none` and no successor handoff
is possible.

## Evidence

Campaign:

```text
p1-gemini35-flash-direct-15g2x3-par2-20260526-051954
```

Parent worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-par2-20260526-051954
```

Succeeded child:

```text
node_id = node-eb352af656260d42
runtime_id = 6177a521-ccd6-48df-ac1b-e3eab75149e0
branch_id = branch-657276832e71602a
slot = r29
commit = 66d7596f011fe595a12fc33c9918e64dc68adb5d
```

The terminal channel result exists and includes treatment evidence:

```text
/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260526-051954/prototype1/nodes/node-eb352af656260d42/channels/6177a521-ccd6-48df-ac1b-e3eab75149e0/child-to-parent.jsonl
```

The node result exists and reports success:

```text
/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260526-051954/prototype1/nodes/node-eb352af656260d42/results/6177a521-ccd6-48df-ac1b-e3eab75149e0.json
/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260526-051954/prototype1/nodes/node-eb352af656260d42/runner-result.json
```

The treatment closure reached eval and protocol complete:

```text
/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260526-051954-treatment-branch-657276832e71602a-1779803432311/closure-state.json
```

But the parent campaign has no comparison artifact:

```text
/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260526-051954/prototype1/evaluations/branch-657276832e71602a.json
```

That path is the file read by `load_evaluation_report()` in
`crates/ploke-eval/src/cli/prototype1_state/run/core.rs`. Since it is absent,
`reconstruct_terminal_outcomes()` builds the child outcome without
`evaluation_report` and without `selection_input`. `select_successor_for_profile`
then has no decision-grade current-generation candidate, so doctor and
`prototype1-step` report:

```text
phase = complete
notes = ["admitted successor selection resolved to none"]
```

The second reproducing campaign hit the same reader/classifier bug without
foreground child recovery. It admitted two generation-1 children:

```text
node-da6475e4299be9dc branch-f982221065f34a84
node-cc665038f3123f66 branch-709763bfbc7f99c9
```

Both child runner results succeeded and their child-to-parent channels contained
treatment evidence, but no parent comparison artifacts or `branches.json` were
written before the controller reported:

```text
phase = complete
notes = ["admitted successor selection resolved to none"]
```

After the source fix, replaying the same persisted campaign state with the fresh
binary re-entered observe, wrote both comparison artifacts, selected
`node-cc665038f3123f66`, and spawned successor runtime
`e2b0daad-1ad4-420a-bb6b-b8e571cfc917`:

```text
/home/brasides/.ploke-eval/campaigns/p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-spawnfix-20260608-081204/prototype1/evaluations/branch-f982221065f34a84.json
/home/brasides/.ploke-eval/campaigns/p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-spawnfix-20260608-081204/prototype1/evaluations/branch-709763bfbc7f99c9.json
/home/brasides/.ploke-eval/campaigns/p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-spawnfix-20260608-081204/prototype1/branches.json
/home/brasides/.ploke-eval/campaigns/p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-spawnfix-20260608-081204/prototype1/nodes/node-cc665038f3123f66/channels/e2b0daad-1ad4-420a-bb6b-b8e571cfc917/child-to-parent.jsonl
```

## Source Trace

The source boundary is
`crates/ploke-eval/src/cli/prototype1_state/run/core.rs`:

- `diagnose()` now classifies a `Succeeded` child with no branch evaluation
  report as `DiagnosedPhase::Observe`.
- `advance_child_phase()` and `resume_c4()` allow that exact repair case to
  replay the observe transition.
- `reconstruct_terminal_outcomes()` now rejects a succeeded child missing its
  branch evaluation report instead of fabricating `completed:Reject`.

## Current Repro Coverage

Focused regressions:

```text
cargo test -p ploke-eval succeeded_child -- --nocapture
```

The tests cover both halves of the contract:

- `succeeded_child_without_evaluation_stays_in_observe`
- `succeeded_child_with_evaluation_can_enter_selection`

Historical replay of the second reproducing campaign also validated the
transition path from observe to select to handoff with the real persisted child
evidence.

## Run Disposition

Treat the original foreground-recovery campaign as stop-use for
successor-promotion evidence. It is still a useful evidence source for child
fanout, detached-child process loss, foreground recovery behavior, and treatment
closure behavior, but it should not be used as a successful end-to-end
successor-handoff run.

The second reproducing campaign remained trustworthy enough for repair-and-resume
because the missing comparison artifacts could be produced from already-persisted
terminal treatment evidence, and selection had not admitted a successor before
the fix.

## Repair Notes

The likely source-level issue is not child treatment execution itself. The
normal parent-owned C5 path calls `compare_observed_child_treatment()` and writes
`prototype1/evaluations/<branch-id>.json`; the operator foreground recovery path
only reran the child invocation and wrote the terminal child result.

A regression should continue exercising the transition/evidence reader path for
this exact shape:

1. child channel has a treatment-bearing terminal success result;
2. node sidecar result is success;
3. parent comparison artifact is absent;
4. `prototype1-step` must write the comparison artifact from the terminal
   treatment evidence before selection, or block loudly instead of completing
   with `selection=none`.

Do not repair this by making selector readers treat success sidecars as
selection evidence. Success sidecars are not sufficient; the parent comparison
artifact or equivalent typed comparison report is the selection input boundary.
