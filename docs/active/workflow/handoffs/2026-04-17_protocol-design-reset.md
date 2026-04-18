# 2026-04-17 Protocol Design Reset

- date: 2026-04-17
- task title: Protocol design reset after long-running campaign pass
- task description: Preserve the restart-critical state after the campaign-backed `closure advance all` run, with emphasis on why protocol advancement was so expensive, what hard blockers appeared, and why the next pass should shift from raw operator execution toward design and tool-improvement work.
- related planning files:
  - [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md)
  - [2026-04-16_eval-closure-formal-sketch.md](../../agents/2026-04-16_eval-closure-formal-sketch.md)
  - [2026-04-15_ploke-protocol-control-note.md](../../agents/2026-04-15_ploke-protocol-control-note.md)

## Restart-Critical State

- Latest fresh closure recompute reached:
  - eval: `221` success, `18` fail, `0` missing
  - protocol: `72` full, `21` partial, `8` fail, `120` missing
- Eval closure is now operationally done for this slice; protocol is the active frontier.
- The campaign command `closure advance all` is not a fixed-point loop. It does one eval pass and then one large protocol frontier walk.

## Why The Protocol Pass Took So Long

- The outer scheduler in `ploke-eval` originally walked selected protocol runs serially.
- Per run, protocol work is not just:
  - intent segmentation
  - call review
  - segment review
- Each call review and each segment review is itself a fork/merge procedure with three adjudication branches:
  - usefulness
  - redundancy
  - recoverability
- So one protocol run can fan out into many more model calls than the CLI surface suggests.
- This is finite, but it is still expensive enough to take hours when driven over a large frontier.

## Hard Blockers Observed

- Some protocol rows are true failures, not just “still missing”.
- The clearest repeated failure was:
  - `missing field "label"` while loading stored intent-segmentation artifacts
- This indicates a reader/writer schema mismatch:
  - the protocol layer allows ambiguous segments without labels
  - the aggregate reader currently expects `label` to be present
- Operational consequence:
  - some protocol rows will not converge merely by letting the current frontier walk continue longer

## Design Shift For Next Restart

- Do not resume from a pure “run more protocol coverage” mindset.
- Treat the existing protocol harvest as evidence about:
  - procedure cost
  - scheduler shape
  - artifact schema compatibility
  - how local analysis outputs should improve tools rather than only count as coverage
- The next useful design thread is:
  - how to use usefulness / redundancy / recoverability judgments to improve tool behavior and operator surfaces
  - how protocol scheduling should work as a bounded runtime queue instead of a long serial sweep

## Local Code State To Remember

- The local worktree now includes a `ploke-eval` scheduler change that moves protocol advancement toward bounded concurrency with a worker queue and `max_concurrency`.
- That change lives in `crates/ploke-eval/` and is intentionally kept out of `ploke-protocol`, which should remain the typed procedure library rather than own campaign scheduling.
- This is implementation progress, not yet the final design answer.

## First Move When Back At The Machine

1. If the old protocol pass is still running, stop it.
2. Recompute closure fresh.
3. Use the recomputed snapshot as the stable baseline for the design-oriented restart.
4. Start from the question:
   - how should protocol analysis outputs improve tools and workflow, not just fill coverage cells?
