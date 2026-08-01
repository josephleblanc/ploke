# Prototype 1 Max-Generation Handoff Persists Failed Successor

Status: source repair restored after rollback; focused and serial regressions
pass; fresh live validation pending.

## Broken Contract

When a selected child node is at `max_generations`, Prototype 1 must stop before
successor handoff and persist a stopped continuation, not spawn a successor that
immediately fails before child planning.

## Evidence

- Campaign: `p1-ui-1783521610`
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-ui-1783521610`
- Fresh walk reconstruction reached `R6` for parent
  `node-c7af383546e6eeff`, generation `3`, then blocked because generation 3 had
  reached `max_generations = 3`.
- The successor channel under
  `prototype1/nodes/node-c7af383546e6eeff/channels/804a61ac-4d5a-4604-a814-b94a8b606a64/child-to-parent.jsonl`
  records `successor_ready` followed by failed successor completion with the
  same cap detail.
- The transition journal records that handoff and failed completion.
- Rollback audit on 2026-07-16 confirmed commit `13bf7b9e4` was not an ancestor
  of `ba7bf5ba5`; both continuation branches had reverted to `>`.

## Source Trace

- Upstream cause:
  `cli_facing.rs::live_successor_continuation_decision` allowed handoff at
  equality.
- Downstream guard: `cli_facing.rs::resolve_parent_policy_budget` correctly
  rejects child planning when the parent generation is at or beyond the cap.
- Reconstruction then stops at `R6` because the capped successor already owns
  the parent identity and no stopped successor record exists.

## Docs/Policy Expectation

`max_generations` is the last generation that may be produced. A child selected
at that generation can be evaluated and sealed, but it must not be promoted
into another live parent turn.

## Current Repro Coverage

```text
cargo test -p ploke-eval generation_cap_stops_direct_child_handoff_at_max_generation -- --nocapture
```

The regression proves equality returns `StopMaxGenerations` and cannot authorize
successor handoff.

## Missing Repro / Validation

A fresh multi-generation walk campaign must reach a stopped terminal
continuation at the cap without spawning another successor.

## Fix Direction

Use `>=` at both authority-bearing continuation branches. Do not weaken the
downstream parent-budget guard or reinterpret the old failed successor as
successful completion.
