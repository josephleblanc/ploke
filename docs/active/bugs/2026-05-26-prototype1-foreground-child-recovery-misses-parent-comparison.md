# Prototype 1 Foreground Child Recovery Misses Parent Comparison

Status: open blocker report from campaign
`p1-gemini35-flash-direct-15g2x3-par2-20260526-051954`.

## Broken Contract

When a detached child runtime dies under the operator environment, rerunning the
saved child invocation in the foreground can produce a terminal child-channel
`Result`, but the parent campaign can still reach terminal `complete` without
writing the parent comparison artifact that successor selection needs.

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

## Run Disposition

Treat this campaign as stop-use for successor-promotion evidence. It is still a
useful evidence source for child fanout, detached-child process loss, foreground
recovery behavior, and treatment closure behavior, but it should not be used as
a successful end-to-end successor-handoff run.

Recommended disposition: abandon-and-restart after fixing or documenting the
foreground recovery path.

## Repair Notes

The likely source-level issue is not child treatment execution itself. The
normal parent-owned C5 path calls `compare_observed_child_treatment()` and writes
`prototype1/evaluations/<branch-id>.json`; the operator foreground recovery path
only reran the child invocation and wrote the terminal child result.

A regression should exercise the transition/evidence reader path for this exact
shape:

1. child channel has a treatment-bearing terminal success result;
2. node sidecar result is success;
3. parent comparison artifact is absent;
4. `prototype1-step` must either write the comparison artifact from the terminal
   treatment evidence before selection, or block loudly instead of completing
   with `selection=none`.

Do not repair this by making selector readers treat success sidecars as
selection evidence. Success sidecars are not sufficient; the parent comparison
artifact or equivalent typed comparison report is the selection input boundary.
