# Prototype 1 Successor Handoff Leaves Stale Parent Identity

Status: fixed in source; live campaign stopped-use after blocker.

Campaign:

```text
p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-spawnfix-20260608-081204
```

Worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-spawnfix-20260608-081204
```

## Broken Contract

A successor Artifact must carry the selected parent identity in
`.ploke/prototype1/parent_identity.json` before the handoff seals and starts the
next runtime. The checkout identity, sealed successor identity, and runtime
identity must all name the same parent coordinate.

## Evidence

The first handoff selected generation-1 parent `node-cc665038f3123f66` and
recorded successor runtime `e2b0daad-1ad4-420a-bb6b-b8e571cfc917`, but the active
checkout and selected branch still carried the generation-0 identity:

```text
/home/brasides/.ploke-eval/worktrees/p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-spawnfix-20260608-081204/.ploke/prototype1/parent_identity.json
parent_id = node-b2296f2600bcca11
generation = 0
branch_id = prototype1-parent-p1-guided-surface-g35flash-p25flash-5g1x2-a2-pr1-spawnfix-20260608-081204-gen0
```

The selected checkout HEAD also contained the same stale identity:

```text
git show HEAD:.ploke/prototype1/parent_identity.json
parent_id = node-b2296f2600bcca11
generation = 0
```

But the transition journal recorded the handoff to generation 1:

```text
kind = active_checkout_advanced
kind = successor_handoff
kind = parent_started
kind = resource phase=parent_start generation=1 node_id=node-cc665038f3123f66
```

The same old-binary successor then advanced into generation 2 before the run was
stopped. It materialized and started:

```text
node-297ca23d5a8dcea0 generation=2 status=running
node-a2b6b659ade64e8a generation=2 status=running
```

Both gen2 child channels had only `ready` and `evaluating` records when the
remaining runner processes were terminated. No terminal gen2 treatment result was
accepted.

## Source Trace

The source boundary was
`crates/ploke-eval/src/cli/prototype1_process.rs::install_committed_successor_artifact`.
Before the fix it switched the active checkout to the selected child artifact
branch, recorded `ActiveCheckoutAdvanced`, and measured the successor surface
without first writing the selected parent identity into the checkout.

The fix writes `selected_parent_identity`, commits
`.ploke/prototype1/parent_identity.json` with the canonical identity commit
message, validates the active checkout against that identity, and only then
measures the installed successor Artifact tree.

## Docs/Policy Expectation

`crates/ploke-eval/src/cli/prototype1_state/prototype1_generation_audit.md`
states that the parent identity is a fixed-path file committed into every parent
Artifact branch, and that a successor Artifact contains
`parent_identity.json` for `Parent(k+1)`.

## Current Repro Coverage

Focused regression:

```text
cargo test -p ploke-eval successor_install_commits_selected_parent_identity -- --nocapture
```

Observed result:

```text
test cli::prototype1_process::tests::successor_install_commits_selected_parent_identity ... ok
```

The existing observe/selection regression also passed:

```text
cargo test -p ploke-eval succeeded_child -- --nocapture
```

## Run Disposition

Treat the campaign above as stopped-use for successor-chain evidence after the
first handoff. It remains useful as an evidence source for the two source bugs,
the gen1 comparison repair, and the stale identity handoff failure, but it should
not be treated as a trustworthy multi-generation run.

The live processes stopped during evidence freeze were:

```text
718810  prototype1-state successor for node-cc665038f3123f66
811194  prototype1-runner for node-a2b6b659ade64e8a
811216  prototype1-runner for node-297ca23d5a8dcea0
```

## Missing Validation

A fresh campaign using a binary built from the fixed source should verify that
the next selected branch has a generation-1 identity commit at HEAD and that a
plain doctor/status invocation no longer falls back to the generation-0 child
plan after handoff.
