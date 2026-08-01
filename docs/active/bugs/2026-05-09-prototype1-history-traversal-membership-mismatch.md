# Prototype 1 History Traversal Membership Mismatch

- date: 2026-05-09 local / 2026-05-10 UTC
- campaign: `p1-occurrence-selection-long-20260509-2`
- run root: `/home/brasides/.ploke-eval/campaigns/p1-occurrence-selection-long-20260509-2/prototype1`
- status: active

## Summary

The occurrence/membership identity shape is readable from sealed History through
`ploke-records`, `ploke-tree`, and the browser export, but the live Prototype 1
loop failed while trying to continue from the selected successor.

The final transition-journal record reports:

```text
kind=successor
runtime_id=c57a9f3f-5836-48c1-9b69-2dd01480db8c
node_id=node-05a4daa25b8b63c5
status=failed
detail=prototype1-state successor failed: batch selection is invalid: selected membership is absent from sealed considered set
```

This is the same semantic family as the issue the previous patch targeted, but
the observed failure is not a deserialization/projection miss. The read side now
accepts and projects the new IDs. The remaining failure appears to be in the
live traversal selection/sealing/handoff path in `ploke-eval`.

## Evidence

Typed loaders still accept the persisted records:

```text
ploke-records real_campaign_scheduler_json_roundtrips: ok
ploke-records real_campaign_node_json_roundtrips: ok
ploke-records real_campaign_transition_journal_parses_representative_entries: ok
```

`ploke-tree` loads the run and reports no parse errors before failing on stale
fixture-specific assertions:

```text
nodes=7 max_generation=2 roots=1
journal_lines=65 journal_parsed=65 journal_errors=0
history_blocks=1 history_entries=1 history_record_errors=0 history_json_errors=0
branches: source_node_count=2 branch_count=4 active_target_count=0
evaluations: file_count=4 parsed_count=4 keep_count=4 reject_count=0
```

Coarse playback over sealed History shows the selected membership is present in
the first sealed block:

```text
step_count=1 warnings=0
block_height=0
selected_candidate=candidate:node-05a4daa25b8b63c5:plan_index=1
selected_occurrence_id=01e7087ec0c9d7d0329b2616d197de7198cb783505322a820e2186818bc373df
selected_membership_id=259f1c24cf76bcf80f98c2aabb5da51c9935a23bc21a2335f24acc31c0aef09b
considered_candidate_count=2
```

Fine playback shows the IDs on both considered candidates and on the selected
successor:

```text
fine step_count=5
candidate_considered=2
successor_selected=1
history_entry_admitted=1
history_block_sealed=1
```

The browser export also carries the new identity fields and derives fine step
IDs from `candidate-membership:<membership_id>`.

The latest persisted observation evidence stopped around
`2026-05-09T18:28:40-07:00`; local inspection time was
`2026-05-09T21:51:40-07:00`. No newer run artifact was found during the check,
so the loop appears stopped after this failure.

## Failure Path

The error string is emitted from
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs` in
`SelectionSealMaterial::selected_payload()`. That method reconstructs a
`CandidateSetCommitment` from:

- `SelectionSealMaterial.considered`
- `SelectionSealMaterial.considered_sources`

Then it resolves `selected_membership_id` against that reconstructed set:

```text
selected membership is absent from sealed considered set
```

The selected membership itself comes from traversal selection in
`ParentRuntime::select_successor()`, where `SelectionSealMaterial` is assembled
from `traversal_selection::select(...)`.

Relevant code surfaces:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `SelectionSealMaterial::selected_payload`
  - `SelectionSealMaterial::candidate_set_commitment`
  - `ParentRuntime::select_successor`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
  - `SelectionDecisionEntry::new_with_traversal_identity`
  - `SelectionDecisionEntry::selected_payload_by_identity`
  - `CandidateSetCommitment`
- `crates/ploke-eval/src/successor_selection/traversal.rs`
  - traversal candidates carry `candidate_set_root` and
    `candidate_set_membership`

## Current Analysis

The previous patch correctly extended the read/projection surface:

- `ploke-records` deserializes `selected_occurrence_id`,
  `selected_membership_id`, and `candidate_set.memberships`.
- `ploke-tree` coarse and fine playback preserve those fields.
- `ploke-tree-browser` exports those fields for the frontend.

The live failure is later in the causal chain. It looks like traversal selection
chooses a membership minted under one candidate-set commitment, but
`SelectionSealMaterial::selected_payload()` validates it against a candidate set
reconstructed from a different considered/source-class basis.

Likely causes to check:

1. Traversal selection is preserving `selected_membership_id` from a historical
   candidate set, but the seal material reconstructs a fresh mixed
   history/current-generation candidate set before handoff.
2. `considered_sources` passed into `SelectionSealMaterial` does not match the
   source classes used when the selected membership was minted.
3. The selected payload is present by candidate label, but the selected
   membership is root-scoped and therefore invalid after the considered set is
   normalized or recomputed.
4. The validator is correct, and the selection layer should mint a new
   membership for the final set being sealed rather than carrying a membership
   from an input set.

The important invariant: `CandidateMembershipId` is not a global candidate
identity. It is scoped to the candidate-set root that produced it. Any live
handoff path that rebuilds or merges candidate sets must either preserve the
exact root/membership pair or remint the selected membership from the final
sealed set.

## Implementation Note

Updated 2026-05-09:

- `crates/ploke-eval/src/successor_selection/traversal.rs` now separates
  source-set membership evidence from final decision-set membership with typed
  role markers.
- Traversal candidate grading may use source-set membership evidence, but the
  returned `Selection.selected_occurrence_id` and `Selection.selected_membership_id`
  are derived from a freshly reconstructed final decision candidate set.
- Regression test:
  `successor_selection::traversal::tests::traversal_seals_membership_from_final_decision_set`
  selects a historical candidate from a mixed history/current universe, proves
  the old source membership is not reused, proves the selected membership
  resolves in the final decision set, and constructs a sealable
  `SelectionDecisionEntry`.

## Related Projection Gap

`history playback` and browser export show occurrence/membership IDs correctly,
but `history selection-show --format json` still renders the older candidate
view without those fields. That made the failure harder to inspect:

```text
considered_total=2
considered[].occurrence_id=null
considered[].membership_id=null
```

This is a projection gap, not the live failure itself. It should be fixed so the
selection debug view exposes:

- selected occurrence ID
- selected membership ID
- candidate-set root
- per-considered occurrence/membership IDs

## Remaining Follow-Up

1. Trace `selected_membership_id` with its `candidate_set_root` through:
   traversal candidate construction, selection, `SelectionSealMaterial`, and
   `SelectionDecisionEntry`.
2. Update `history selection-show` to expose the new identity fields so this
   class of mismatch can be diagnosed without relying on fine playback only.
3. Relax or split the stale `ploke-tree` ignored real-run tests so they remain
   useful for arbitrary active campaigns instead of asserting old fixture
   counts.
