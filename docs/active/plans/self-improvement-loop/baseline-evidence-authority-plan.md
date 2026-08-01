# Baseline Evidence Authority Plan

Pickup plan for the Prototype 1 failure where generation-2 child evaluations
rejected valid treatments because the baseline arm had no complete run record.

## Surface Request

Fix the live-loop shape that allowed child fanout and treatment comparison to
run with `baseline_record_path: null`.

## Causal Chain

```text
Parent artifact P
  -> baseline/control evaluation for P under policy E
  -> Baseline<P, E, Complete>
  -> children C_i generated from P
  -> treatment evaluation for C_i under policy E
  -> compare Baseline<P, E, Complete> against Treatment<C_i, E, Complete>
  -> select successor
  -> selected child artifact becomes next Parent artifact
```

The current failure collapsed this into:

```text
campaign closure-state.json exists
  -> assume baseline/control evaluation exists
```

That is not a valid implication.

## Current Failure Evidence

Live campaign:

```text
p1-smoke-3x4-edit-surface-20260510-3
```

Observed facts:

- `closure-state.json` for the baseline campaign was updated at
  `2026-05-10T19:43:32Z`.
- It still reported `eval.status: missing`.
- Its only instance row had `artifacts.record_path: null`.
- Generation-2 treatment evaluations then rejected with
  `missing_baseline_record`.
- At least one treatment had useful treatment metrics and
  `nonempty_valid_patch: true`, so the reject was caused by missing baseline
  evidence, not necessarily by a bad child patch.

Relevant code surfaces:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `ensure_prototype1_baseline_closure_state` only checks that the
    closure-state projection exists and parses.
  - the legacy controller path runs `advance_eval_closure` before target
    selection.
  - the `tui-edit-surface` child-plan path bypasses that controller and can
    proceed with only a stale/missing baseline closure projection.
- `crates/ploke-eval/src/cli/prototype1_process.rs`
  - child evaluation loads baseline closure state and compares against the
    treatment closure state.

## Intended Model

### Initial Parent

The root parent artifact has no previous treatment evidence.

Correct shape:

```text
Artifact A0
  -> Parent<A0, Baseline<Missing>>
  -> run baseline/control eval for A0
  -> Parent<A0, Baseline<Complete>>
  -> child fanout
```

The initial parent may spawn children only after the baseline/control arm has
produced complete evidence for the active instance and eval policy.

### Following Parents

A successor parent is selected from children that were already evaluated as
treatments.

Correct shape:

```text
Parent<F, Baseline<Complete>>
  -> child B evaluated as Treatment<B, E, Complete>
  -> select B
  -> promote Treatment<B, E, Complete> as Baseline<B, E, Complete>
  -> Parent<B, Baseline<Complete>>
  -> child fanout
```

The new parent should normally inherit its baseline evidence from the selected
child's completed treatment evidence. It should not ask the original campaign
baseline projection to stand in for the selected artifact's baseline.

If selected-child treatment evidence is missing or policy-incompatible, the
successor may not proceed to child fanout. It must either fail before spawning
children or explicitly run/reconstruct the missing baseline for that selected
artifact.

## Invariants

- `Parent<P>` may spawn children only with `Baseline<P, E, Complete>`.
- `Baseline<P, E, Complete>` must identify:
  - parent artifact identity `P`
  - instance id
  - eval/protocol policy identity `E`
  - run record path
  - derived metrics used by selection
- `closure-state.json` is a projection/cache. It may help locate evidence, but
  its existence is not authority.
- A treatment record for selected child `B` can become baseline evidence for
  `B` only if it was produced under the expected evaluation policy.
- Child evaluation should compare:

```text
Baseline<ParentArtifact, EvalPolicy, Complete>
Treatment<ChildArtifact, EvalPolicy, Complete>
```

not:

```text
CampaignClosureState
TreatmentClosureState
```

## Structural Naming Constraint

Do not encode this fix as a long helper name like
`ensure_fresh_complete_baseline_closure_state_for_current_parent`.

The missing object is structural. Prefer carriers shaped around role, artifact,
policy, and state, for example:

```rust
Baseline<P, Missing>
Baseline<P, Complete>
Parent<P, Baseline<Complete>>
Treatment<C, Complete>
```

The exact Rust spelling can differ, but the type boundary must prevent treating
`ClosureState<FileExists>` as `Baseline<Complete>`.

## Implementation Plan

1. Add a typed baseline evidence carrier.

   It should be constructed from authoritative run/evaluation artifacts, not
   from the mere existence of `closure-state.json`.

2. Add an initial-parent transition.

   Root parent startup should move from baseline-missing to baseline-complete
   before child fanout. The transition can call the existing
   `advance_eval_closure`/`advance_protocol_closure` machinery, but the result
   must be checked for a complete record path before proceeding.

3. Add a successor-parent transition.

   When a child is selected, carry the selected child treatment evidence through
   the selection/handoff record. Successor startup should establish
   `Baseline<SelectedArtifact, Complete>` from that evidence before it can enter
   child fanout.

4. Move comparison inputs away from campaign-global baseline lookup.

   Child evaluation should receive or resolve baseline evidence for the active
   parent artifact. It should not load the original campaign closure state and
   assume that row is the correct baseline for every later parent.

5. Fail early on missing evidence.

   If the baseline evidence cannot be completed or promoted, fail before
   spawning child runners. Do not allow `missing_baseline_record` to appear as a
   per-child reject caused by parent setup state.

6. Keep `scheduler.json` demoted.

   Do not repair this by pushing more authority into `scheduler.json`. That file
   is a mutable projection. Live health should be projected from node records,
   runner results, transition journal entries, and typed History/selection
   records.

## Test Plan

- Unit test: `ensure_prototype1_baseline_closure_state` or its replacement must
  reject a closure projection with `eval.status: missing` and
  `record_path: null` when asked for `Baseline<Complete>`.
- Unit test: initial parent cannot produce a child plan when baseline evidence
  is missing.
- Unit test: selected child treatment evidence can be promoted into successor
  baseline evidence only when artifact id and eval policy match.
- Regression test: a treatment with `nonempty_valid_patch: true` must not be
  rejected as `missing_baseline_record` when complete parent baseline evidence
  exists.
- Projection test: operator health/status code must not use `scheduler.json` as
  the authoritative live node status source.

## Next Slice

Start by finding the smallest place to introduce the baseline carrier around:

```text
crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs
crates/ploke-eval/src/cli/prototype1_process.rs
```

Then patch the `tui-edit-surface` path first, because that is the path that
bypassed the legacy baseline-advance controller and produced the live failure.
