# Prototype 1 Walk Reconstruction Rejects Fresh Missing Baseline

Status: source repair restored after rollback; focused and serial regressions
pass; fresh walk resume pending.

## Broken Contract

Durable walk reconstruction must rebuild the latest safe typestate boundary
from existing evidence. A fresh campaign with parent-start evidence but no
baseline eval yet reconstructs to `R5`; it must not validate child-spawn
baseline evidence before `r5_to_r6` runs.

## Evidence

- Campaign: `p1-ui-1783521610`
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-ui-1783521610`
- `walk start --until r5` observed parent-start evidence, then rejected the
  wholly missing closure row as though baseline execution had completed.
- The transition journal contained the expected `parent_started` evidence.
- `closure-state.json` still had `eval.status = "missing"` and an instance
  `eval_status = "missing"`, which is the expected pre-baseline setup shape.
- Rollback audit on 2026-07-16 confirmed commit `fde746c0e` and its regression
  were absent from current HEAD.

## Source Trace

`WalkController` reconstruction reaches `load_parent_baseline` after recovering
parent-start evidence. That reader treated the existence of
`closure-state.json` as baseline evidence and called the strict
`complete_baseline_from_closure` validator at the wrong phase.

## Docs/Policy Expectation

Missing evidence returns the latest safe state plus blockers; reconstruction
does not replay side effects or fabricate downstream evidence. Here the latest
safe state is `R5`, and only the live `r5_to_r6` edge owns baseline execution.

## Current Repro Coverage

```text
cargo test -p ploke-eval load_parent_baseline_treats_fresh_missing_closure_as_not_ready -- --nocapture
```

The test proves a generation-zero closure whose eval summary and instance rows
are wholly missing is treated as absent baseline evidence.

## Missing Repro / Validation

Restart a fresh walk server after R5 reconstruction and advance the live
`r5_to_r6` edge successfully.

## Fix Direction

Keep `complete_baseline_from_closure` strict. Only the wholly missing
pre-baseline shape returns `Ok(None)` from `load_parent_baseline`; failed,
partial, complete-but-invalid, and inconsistent projections remain errors.
