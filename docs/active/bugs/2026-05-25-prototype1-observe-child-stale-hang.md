# 2026-05-25 Prototype 1 `observe_child` Stale Hang

Status: fixed in source with regression coverage. Follow-up coverage now also
blocks acknowledged-but-dead children before the parent starts `observe_child`.

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
- Follow-up: the default observe-child stale threshold is now 1200 seconds so
  slow but live child eval/protocol runs are less likely to be misclassified as
  stale solely because protocol adjudication exceeded the previous 10-minute
  default.
- Follow-up: doctor now also blocks the pre-observe case where a node is
  `Running`, the spawn journal recorded an acknowledged child PID, no
  `observe_child:before` exists yet, the attempt-scoped runner result is absent,
  and the PID is no longer visible. Without this, the next `prototype1-step`
  could enter observe and wait the full stale threshold even though the child
  process was already gone.

## 2026-05-25 Follow-up Evidence

Campaign `p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824` reached
`phase=observe` for child `node-b85ef46248b84eb4`, runtime
`d7f47451-ac79-4ab0-a980-e36bb10fc71f`. The journal recorded spawn,
ready, evaluating, and spawn-observed/acknowledged records for PID `2479313`,
but there was no `observe_child:before`, no
`nodes/node-b85ef46248b84eb4/results/d7f47451-ac79-4ab0-a980-e36bb10fc71f.json`,
and no live `ploke-eval` process. The treatment campaign had only setup
artifacts and closure state still reported eval evidence as missing.

Before the follow-up fix, doctor still reported no blockers because the stale
logic only handled a pending `observe_child:before`. After rebuilding source,
doctor reports:

```text
phase=blocked
blocker: node 'node-b85ef46248b84eb4' is running for runtime
'd7f47451-ac79-4ab0-a980-e36bb10fc71f', but child pid 2479313 is not visible
and no runner result exists at
.../nodes/node-b85ef46248b84eb4/results/d7f47451-ac79-4ab0-a980-e36bb10fc71f.json
```

Disposition for this campaign: stop advancing it for loop evidence. The child
did not produce treatment evidence, protocol evidence, or a runner result, so
there is no trustworthy child result to compare or select.

## Verification

Focused tests:

```text
cargo test -p ploke-eval replay_marks_missing_runner_result_as_stale_or_hung -- --nocapture
cargo test -p ploke-eval replay_observe_child_uses_default_stale_threshold -- --nocapture
cargo test -p ploke-eval stale_observe_before_adds_doctor_blocker -- --nocapture
cargo test -p ploke-eval dead_acknowledged_running_child_adds_doctor_blocker_before_observe -- --nocapture
```
