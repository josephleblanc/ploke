# Prototype1 Observe Child Success Sidecar Race

Status: open; current source has a channel-only C4 observation fix plus
historical regression guards, but live fresh-run verification is still pending.
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

`C4 -> C5` treated runner-result projections as terminal observation evidence
even though parent/child lifecycle state should be driven by the per-runtime
channel. Successful children additionally require treatment evidence, and that
treatment evidence only exists in the terminal channel `Result` payload.

The sidecar is reconstruction evidence. It should not advance C4 by itself.

## Fix

`ObserveChild` now keeps waiting when it sees a sidecar result or compatibility
`ResultWritten` projection without the terminal channel payload. It advances C4
only from `ToParent::Result`.

Regression test:

```bash
cargo test -p ploke-eval observe_child_waits_for_treatment_channel_result_after_success_sidecar -- --nocapture
```

The test writes a successful sidecar result first, delays the channel terminal
`Result`, and verifies `ObserveChild` waits for the treatment-bearing channel
payload.

Additional authority tests:

```bash
cargo test -p ploke-eval observe_child_times_out_on_success_sidecar_without_channel_result -- --nocapture
cargo test -p ploke-eval observe_child_ignores_result_written_projection_for_success -- --nocapture
cargo test -p ploke-eval observe_child_rejects_success_channel_result_without_treatment -- --nocapture
```

These tests prove that a sidecar alone does not advance C4, a compatibility
`ResultWritten` message does not advance C4, and a successful channel `Result`
without treatment evidence fails explicitly instead of entering comparison.

That synthetic test is necessary but no longer sufficient. A fixture-backed
historical transition/evidence regression now uses the actual sidecar/channel
shape from:

- Campaign: `p1-gemini35-flash-direct-15g2x3-20260525-035000`
- Node: `node-15006265e24b3b9b`
- Runtime: `8d4f99c6-c167-4c02-90d6-2055b174c34e`

Additional regression test:

```bash
cargo test -p ploke-eval observe_child_replays_node_150_success_sidecar_then_historical_treatment_channel -- --nocapture
```

This test writes the historical successful sidecar first, delays the exact
historical treatment-bearing channel record, and verifies `ObserveChild`
advances to a successful observation instead of failing with
`MissingTreatmentEvidence`.

The bug remains open until a fresh live run verifies the parent can complete
this observe/comparison path and continue without hitting the same blocker.

## Resume Notes

Do not treat a historical `MissingTreatmentEvidence` failure as evidence that
the child failed semantically. For this run, inspect doctor state first. If the
node remains resumable and the channel terminal result is present, a rebuilt
controller may be able to observe it. If the run has already admitted later
contradictory state, abandon and restart according to the loop-blocker workflow.
