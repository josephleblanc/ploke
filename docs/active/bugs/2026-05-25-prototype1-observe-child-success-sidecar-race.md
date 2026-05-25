# Prototype1 Observe Child Success Sidecar Race

Status: fixed in source; verify on the next fresh or safely resumed run.
Severity: loop blocker.

## Symptom

Prototype 1 failed during child completion with:

```text
database setup failed during 'prototype1_state_complete':
Transition(MissingTreatmentEvidence { node_id: "node-15006265e24b3b9b" })
```

This occurred twice during the same live run class, after successful treatment
children had already written attempt results.

## Evidence

- Campaign: `p1-gemini35-flash-direct-15g2x3-20260525-035000`
- Node: `node-15006265e24b3b9b`
- Runtime: `8d4f99c6-c167-4c02-90d6-2055b174c34e`
- Transition journal had `observe_child:before` for this runtime and a later
  child `result_written` entry, but no durable `observe_child:after` for this
  node before the parent failed.
- `nodes/node-15006265e24b3b9b/results/<runtime>.json` contained only the
  successful runner result sidecar.
- `nodes/node-15006265e24b3b9b/channels/<runtime>/child-to-parent.jsonl`
  contained the full terminal `result` message with both `runner_result` and
  `treatment`.

The sidecar and channel records were written about 9 ms apart. The sidecar was
visible first.

## Broken Contract

`C4 -> C5` treated the successful sidecar runner-result projection as a terminal
observation even though successful children require treatment evidence, and the
treatment evidence only exists in the terminal channel `Result` payload.

The sidecar is sufficient to observe failed children, but not successful
children.

## Fix

`crates/ploke-eval/src/cli/prototype1_state/c4.rs` now keeps waiting when it
sees a successful sidecar result without a channel terminal payload. Failed
sidecar results still complete through the existing fallback path.

Regression test:

```bash
cargo test -p ploke-eval observe_child_waits_for_treatment_channel_result_after_success_sidecar -- --nocapture
```

The test writes a successful sidecar result first, delays the channel terminal
`Result`, and verifies `ObserveChild` waits for the treatment-bearing channel
payload instead of failing with `MissingTreatmentEvidence`.

## Resume Notes

Do not treat a historical `MissingTreatmentEvidence` failure as evidence that
the child failed semantically. For this run, inspect doctor state first. If the
node remains resumable and the channel terminal result is present, a rebuilt
controller may be able to observe it. If the run has already admitted later
contradictory state, abandon and restart according to the loop-blocker workflow.
