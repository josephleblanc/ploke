# Prototype 1 Late Child Result Recovery Corrupts Node State

Status: open blocker; current campaign is stop-use for loop progress.

Campaign:

```text
p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113
```

Worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113
```

## Broken Contract

When a child result arrives after the parent `observe_child` timeout, the
controller must either reconcile the terminal child evidence through the
parent comparison path or block with a clear recovery instruction; it must not
leave contradictory child-success and successor-failure evidence, and a later
parent re-entry must not downgrade terminal child node state.

## Evidence

The gen2 child that exposed the race:

```text
node_id = node-7809adc3fc6e4aad
runtime_id = a7691248-60cb-419f-9a3a-52670fcb7e08
branch_id = branch-3e52c562999775da
treatment = p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113-treatment-branch-3e52c562999775da-1781026510878
```

The parent successor runtime failed before the child result arrived:

```text
/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113/prototype1/nodes/node-0dae679bb16a4604/channels/d38b78ed-724e-4536-9235-47fc67933c18/child-to-parent.jsonl
```

That channel records `successor_completion.status = failed` at
`2026-06-09T17:55:11.290602501Z` with:

```text
TimedOutWaitingForResult {
  node_id: "node-7809adc3fc6e4aad",
  runtime_id: a7691248-60cb-419f-9a3a-52670fcb7e08,
  waited_ms: 1200065,
  runner_result_path: ".../node-7809adc3fc6e4aad/results/a7691248-60cb-419f-9a3a-52670fcb7e08.json"
}
```

The child result arrived about 43 seconds later:

```text
/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113/prototype1/nodes/node-7809adc3fc6e4aad/channels/a7691248-60cb-419f-9a3a-52670fcb7e08/child-to-parent.jsonl
/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113/prototype1/nodes/node-7809adc3fc6e4aad/results/a7691248-60cb-419f-9a3a-52670fcb7e08.json
/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113/prototype1/nodes/node-7809adc3fc6e4aad/runner-result.json
```

The terminal child record reported success:

```json
{
  "status": "succeeded",
  "disposition": "succeeded",
  "branch_id": "branch-3e52c562999775da",
  "recorded_at": "2026-06-09T17:55:54.420975107+00:00"
}
```

But parent decision records remained incomplete:

```text
/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113/prototype1/branches.json
/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113/prototype1/evaluations/
```

`branches.json` contained gen2 records for `branch-a2091b86f5682087` and
`branch-69b1c2bee940922c`, but no `branch-3e52c562999775da` entry. The
evaluations directory likewise had no `branch-3e52c562999775da.json`.

The treatment closure for the late child was also not selection-grade:

```text
/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113-treatment-branch-3e52c562999775da-1781026510878/closure-state.json
```

It recorded eval complete for both benchmark instances, protocol complete for
`BurntSushi__ripgrep-2209`, and protocol missing for
`BurntSushi__ripgrep-2295`.

An attempted recovery command then made the campaign less trustworthy:

```text
/home/brasides/.ploke-eval/worktrees/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113/target/debug/ploke-eval \
  loop prototype1-state \
  --campaign p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113 \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-20260609-092113 \
  --handoff-invocation .../node-0dae679bb16a4604/invocations/d38b78ed-724e-4536-9235-47fc67933c18.json \
  --stop-after complete \
  --format json
```

The recovery attempt appended a new parent start and repeated materialize/build
records for all three existing gen2 children. It then failed at
`prototype1_child_artifact_commit` because sandboxed `git add` could not create
linked-worktree index locks under `/home/brasides/code/ploke/.git/worktrees/`.

The attempted recovery also rewrote gen2 `node.json` terminal states back to
`binary_built` even though each `runner-result.json` still reports success:

```text
node-1ce9b90304f3c980 node.json status=binary_built; runner-result status=succeeded
node-cbbb6061cbc68f81 node.json status=binary_built; runner-result status=succeeded
node-7809adc3fc6e4aad node.json status=binary_built; runner-result status=succeeded
```

The final successor channel now contains two failure records:

1. the original late-result timeout at `prototype1_state_complete`;
2. the failed recovery at `prototype1_child_artifact_commit`.

The failed recovery did not spawn duplicate child runtimes: no post-11:00 gen2
child invocations, runtime channels, or runner result files were created. The
post-11:00 writes were limited to duplicate parent/successor/materialize/build
journal records, three rewritten `runner-request.json` files, three rebuilt
`bin/ploke-eval` files, three downgraded `node.json` files, and the parent
successor channel append.

## Source Trace

The child observation contract is implemented in
`crates/ploke-eval/src/cli/prototype1_state/c4.rs`. `ObserveChild` polls the
per-runtime child channel until a treatment-bearing result arrives, then writes
the `observe_child:after` journal entry and advances to comparison. If the
profile deadline passes first, it returns `TimedOutWaitingForResult`.

The timeout policy is documented in
`crates/ploke-eval/docs/knobs/timeouts.md`: child result observation polls the
per-runtime child channel, while result files are reconstruction evidence, not
lifecycle authority. The admitted run profile controls the deadline with
`execution.observe_child_stale_after_secs`, documented in
`crates/ploke-eval/docs/prototype1-run-profile.md`.

There is already a narrower diagnostic repair shape in
`crates/ploke-eval/src/cli/prototype1_state/run/core.rs`:

- `needs_terminal_observe()` keeps a `Succeeded` child with no evaluation report
  in `DiagnosedPhase::Observe`.
- `resume_c4()` accepts that succeeded-without-evaluation shape.
- `advance_one_child()` can run `ObserveChild` and
  `compare_observed_child_treatment()` for the repaired child.

The failed command did not use that narrow diagnostic path. Direct
`prototype1-state --handoff-invocation --stop-after complete` re-entered a
full parent turn, resolved the existing child plan again, and re-ran
materialize/build before failing. That path is not idempotent over a failed
successor with already-terminal child result evidence.

## Docs / Policy Expectation

`stop_after = complete` is documented as running evaluation, selection, and
handoff. It should not be a destructive or status-downgrading replay over
already-terminal children.

The timeout docs say channel results are lifecycle authority and result files
are reconstruction evidence. Once the lifecycle authority arrives late, the
operator-facing recovery path must either reconcile that authority through the
same comparison boundary or block before any further paid or state-mutating
loop work.

## Current Repro Coverage

Existing tests cover adjacent pieces but not this incident:

- `observe_child` no longer waits forever on missing terminal evidence.
- `observe_child` no longer treats a success sidecar as terminal before the
  treatment-bearing channel result.
- a succeeded child without a branch evaluation stays in observe for diagnostic
  repair.

Those tests do not cover a result that arrives after a recorded
`TimedOutWaitingForResult`, and they do not cover direct `prototype1-state`
re-entry over a failed successor with terminal child evidence.

Initial guard coverage added on 2026-06-09:

```text
RUSTFLAGS=-Awarnings cargo test -p ploke-eval terminal_child_blocks_reentry -- --nocapture
```

That regression constructs a planned child, writes a persisted nonterminal
`node.json` plus a terminal successful `runner-result.json`, then calls the
production `run_planned_child` direct parent path. The expected result is a
structured `InvalidBatchSelection` before materialize/build and before any
node-status rewrite.

Historical replay coverage added on 2026-06-09:

```text
RUSTFLAGS=-Awarnings cargo test -p ploke-eval historical_late_child_result_blocks_direct_reentry -- --nocapture
```

Fixture artifacts are copied from the stopped campaign under:

```text
crates/ploke-eval/src/tests/fixtures/prototype1-late-child-result/
```

The replay consumes the recorded child plan, transition journal, node
projection, runner result, per-runtime result, and child channel for
`node-7809adc3fc6e4aad`. It parses the historical journal through
`PrototypeJournal::replay_observe_child_at`, verifies the `observe_child:before`
entry plus terminal runner result classifies as
`TerminalResultWrittenUnobserved(Succeeded)`, verifies the branch evaluation is
absent, then calls production `run_planned_child`. The expected result is a
structured `InvalidBatchSelection` before materialize/build and without new
transition-journal entries.

Focused validation after adding the replay:

```text
RUSTFLAGS=-Awarnings cargo test -p ploke-eval terminal_child_blocks_reentry -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval historical_late_child_result_blocks_direct_reentry -- --nocapture
```

Both focused tests passed with one matching test each.

Run-review scout fan-out also confirmed the persisted artifact chain:

- authority/runtime scout: the child runner/channel evidence is terminal
  success, while `observe_child:after`, branch registry, branch evaluation, and
  sealed History evidence are absent for `branch-3e52c562999775da`;
- eval/protocol scout: both treatment eval run roots exist, protocol artifacts
  exist only for `BurntSushi__ripgrep-2209`, protocol is missing for
  `BurntSushi__ripgrep-2295`, and the treatment closure is partial despite the
  terminal child result.

Run-review fan-in classified this as blocker-repair, not provider failure. The
concrete execution path is:

```text
prototype1-state
  -> leaf_runner_argv
  -> Prototype1RunnerCommand::run
  -> execute_prototype1_runner_invocation
  -> run_prototype1_resolved_branch_treatment
  -> advance_eval_closure
  -> advance_protocol_closure
  -> build_prototype1_treatment_evidence
```

The artifact chain is:

```text
invocations/a7691248-60cb-419f-9a3a-52670fcb7e08.json
  -> child-to-parent.jsonl ready/evaluating/Result
  -> runner-result.json status=succeeded
  -> transition-journal.jsonl observe_child:before without observe_child:after
  -> no branches.json / evaluations / History refs for branch-3e52c562999775da
```

The fan-in also separated mechanical child success from treatment usefulness:
the treatment closure is eval-complete for both benchmark instances but
protocol-partial, with protocol complete for `BurntSushi__ripgrep-2209` and
missing for `BurntSushi__ripgrep-2295`.

## Missing Repro / Validation

Add a step-level regression with this historical shape:

1. child channel has `ready` and `evaluating`;
2. parent successor channel records `TimedOutWaitingForResult`;
3. child later writes a treatment-bearing terminal `Result` and
   `runner-result.json`;
4. branch comparison and evaluation for that child are absent;
5. recovery must take an explicit append-only reconciliation path or block with
   a clear diagnostic.

The regression should fail if a recovery path re-runs materialize/build for
terminal children or rewrites a terminal `node.json` state to a nonterminal
state.

Remaining validation gap after the new replay: a purpose-built repair or
diagnostic command still needs coverage that proves it reconciles the late
terminal child result through the parent comparison/evaluation boundary without
re-running child prep. The new guard prevents direct destructive re-entry; it
does not yet implement recovery of the missing branch-level evaluation or the
missing protocol artifact for one treatment instance.

Additional missing coverage from run-review fan-in:

- a repair-path replay that consumes the historical terminal channel and emits
  `observe_child:after` plus branch/evaluation/History evidence;
- a regression for required-procedure protocol partial/missing treatment
  evidence, either blocking success-shaped treatment promotion or recording the
  partial protocol state in branch evaluation;
- an end-to-end replay through diagnosis/continue for the stopped campaign,
  beyond the current direct re-entry guard.

Current automated evidence:

- `historical_late_child_result_blocks_direct_reentry` replays the real stopped
  campaign artifacts through typed node, runner-result, channel, and transition
  journal readers, then proves direct child-path re-entry blocks before
  materialize/build when the late terminal result has no branch evaluation;
- `terminal_child_blocks_reentry` covers the synthetic terminal-runner-result
  mismatch case and proves the command does not rewrite the persisted node or
  create a build target;
- `succeeded_child_without_evaluation_blocks_direct_reentry` covers the
  selection-boundary case where both node and runner result say `succeeded` but
  no branch evaluation report exists;
- `child_runtime_transitions_emit_authority_trace` now asserts the structured
  `PrototypeJournal` child records instead of treating string-formatted tracing
  output as test evidence.

Focused validation on 2026-06-09:

```text
RUSTFLAGS=-Awarnings cargo test -p ploke-eval historical_late_child_result_blocks_direct_reentry -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval terminal_child_blocks_reentry -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval succeeded_child_without_evaluation_blocks_direct_reentry -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval child_ -- --nocapture
```

## Fix Direction

Own the fix at the Prototype 1 state-transition layer, not in selection readers.

Required properties:

- terminal node status must be monotonic unless a repair command explicitly
  creates a new runtime/attempt identity;
- direct `prototype1-state --handoff-invocation --stop-after complete` must not
  re-materialize or rebuild children that already have terminal runtime
  results;
- late treatment-bearing channel results need a deterministic reconciliation
  command or diagnosis phase that writes the missing parent comparison/eval
  records from the existing evidence;
- if protocol closure is partial, the controller should preserve that as the
  branch disposition input or run the missing protocol explicitly before
  selection, rather than fabricating selection-grade evidence;
- sandboxed or linked-worktree `.git/worktrees/*/index.lock` failures should be
  preflighted before state-mutating child prep when the command would write
  through a linked worktree.

Do not repair this by making selector readers treat `runner-result.json` alone
as selection evidence. The parent comparison artifact, or an equivalent typed
comparison report, remains the selection input boundary.

## Disposition

Abandon-and-restart for loop-progress purposes. The campaign is still useful as
evidence for late child results, partial protocol closure, and non-idempotent
parent re-entry, but it now contains contradictory required state:

- successor completion failed;
- late child runner results succeeded;
- branch/evaluation records for `branch-3e52c562999775da` are absent;
- the later recovery attempt rewrote terminal node statuses back to
  `binary_built`;
- no gen2 selection or successor handoff was sealed.

Resume this exact campaign only if a purpose-built repair command can prove,
append-only, that it reconstructs the missing comparison/evaluation without
rerunning child prep or rewriting terminal states. Otherwise start a fresh
campaign after the regression and source fix.

## Related Bugs

- [`2026-05-25-prototype1-observe-child-stale-hang.md`](./2026-05-25-prototype1-observe-child-stale-hang.md)
- [`2026-05-25-prototype1-observe-child-success-sidecar-race.md`](./2026-05-25-prototype1-observe-child-success-sidecar-race.md)
- [`2026-05-26-prototype1-foreground-child-recovery-misses-parent-comparison.md`](./2026-05-26-prototype1-foreground-child-recovery-misses-parent-comparison.md)
- [`2026-06-08-prototype1-successor-handoff-stale-parent-identity.md`](./2026-06-08-prototype1-successor-handoff-stale-parent-identity.md)
