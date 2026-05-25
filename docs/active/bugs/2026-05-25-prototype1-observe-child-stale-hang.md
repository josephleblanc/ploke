# 2026-05-25 Prototype 1 `observe_child` Stale Hang

Status: fixed in source with regression coverage.

## Symptom

Campaign `p1-gemini35-flash-direct-fresh-20260525-035030` reached
`observe_child` for node `node-4a657b2403d14770`, runtime
`c85fcad2-7f28-4cee-b470-a4781951e817`, then stopped making observable
progress. The transition journal had `observe_child:before` at
2026-05-24 23:17:22 PDT, no matching `observe_child:after`, and the
attempt-scoped result path was absent:

```text
prototype1/nodes/node-4a657b2403d14770/results/c85fcad2-7f28-4cee-b470-a4781951e817.json
```

Doctor still reported `phase=observe`, `status=running`, and no blockers.

## Broken Contract

`C4 -> C5` treated the child channel result as the only completion signal and
polled forever when the child stopped producing channel output and did not write
the attempt result. Replay classified the pending observe as generic
`ResultPending`, and doctor did not turn stale pending observe evidence into a
blocker.

## Fix

- `observe_child` now has a stale deadline from
  `[execution].observe_child_stale_after_secs`, with the same default used by
  journal replay callers that do not have an admitted profile.
- `observe_child` checks an attempt-scoped runner result path while polling, so
  a written result without a channel message is no longer invisible.
- Journal replay can classify a missing-result pending observe as
  `StaleOrHung`.
- Doctor adds a blocker for stale/hung pending observe states, including the
  runtime id, missing result path, pid liveness when known, and stream freshness
  when stream paths are recorded. The doctor threshold comes from the admitted
  `run-profile.toml`.

## Verification

Focused tests:

```text
cargo test -p ploke-eval replay_marks_missing_runner_result_as_stale_or_hung -- --nocapture
cargo test -p ploke-eval replay_observe_child_uses_default_stale_threshold -- --nocapture
cargo test -p ploke-eval stale_observe_before_adds_doctor_blocker -- --nocapture
```
