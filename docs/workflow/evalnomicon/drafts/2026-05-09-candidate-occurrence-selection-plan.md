# Candidate Occurrence Selection Plan

Prototype 1 successor selection currently treats a candidate label
(`SubjectRef`) as the selected identity. That is too weak for the History model:
the same candidate subject can appear more than once as distinct observed
runtime/artifact occurrences. Selection should choose a concrete occurrence
under a lineage authority context, then project legacy labels only for display
and compatibility.

## Problem

The failing shape was:

```text
History payload:  candidate:node-b744d87d056c7bb5:plan_index=2
Current payload:  candidate:node-b744d87d056c7bb5:plan_index=2
Selected label:   candidate:node-b744d87d056c7bb5:plan_index=2
```

Those two payloads may be different occurrences: different runtime, artifact
evidence, History source, or candidate-set membership. But the sealed selection
entry only names the selected candidate by `SubjectRef`, so History cannot know
which payload was selected. `SelectionDecisionEntry` is right to reject this as
ambiguous.

Self-selection is not the bug. The Hyperagents/DGM-H style archive may select an
existing archive member again. The bug is collapsing occurrence identity into a
string label before sealing the decision.

## Existing Pieces

- `CandidateCoordinate` already carries the candidate occurrence fields:
  `node_id`, `parent_node_id`, `branch_id`, `generation`, `plan_index`, and
  `primary_runtime_id`.
- `CandidateArtifact` carries artifact-backed candidate material.
- `SuccessorRef` carries runtime plus artifact for sealed successor handoff.
- `LineageId`, `LineageState`, `StoreHead`, and `LineageKey` carry History
  authority and head state.
- `CandidateSetCommitment` and `CandidateSetMembership` already provide a
  sealed considered-set membership surface.
- `SubjectRef`, `CandidateRef`, `candidate_node_id`, and `selected_branch_id`
  are compatibility/display projections, not durable occurrence identity.

## Target Model

Introduce two content-addressed identifiers using existing `HistoryHash`
machinery and SHA-256 domain-separated preimages:

```text
CandidateOccurrenceId = hash("prototype1.history.candidate_occurrence.v1", ...)
CandidateMembershipId = hash("prototype1.history.candidate_membership.v1", ...)
```

`CandidateOccurrenceId` identifies one observed candidate occurrence. Its
preimage should include the lineage authority context and stable occurrence
facts available at selection time:

- `LineageId`
- `CandidateCoordinate`
- `ArtifactRef` when present
- runtime/actor reference when present
- History/current-generation source class

`CandidateMembershipId` binds an occurrence to one sealed considered set:

```text
CandidateMembershipId = hash(CandidateOccurrenceId, CandidateSetRoot)
```

Selection should choose an occurrence or membership, not a `SubjectRef`.

## Implementation Direction

1. Add typed occurrence identity in the History/selection boundary.

   Prefer small newtypes over strings:

   ```rust
   CandidateOccurrenceId(HistoryHash)
   CandidateMembershipId(HistoryHash)
   ```

   Add constructors from typed preimages and keep formatting at the edge.

2. Make traversal candidates occurrence-bearing.

   Traversal items should carry:

   ```text
   occurrence_id
   coordinate
   payload
   source
   candidate_set_root / membership when available
   artifact/runtime refs when available
   ```

   Scoring may still group by node, parent node, generation, or performance, but
   the selected item must remain occurrence-addressed.

3. Change sealed selection to validate selected occurrence membership.

   `SelectionDecisionEntry` should locate the selected payload by occurrence or
   membership identity. The old `selected_candidate: SubjectRef` may remain as a
   derived compatibility field, but it must not be the authority lookup.

4. Preserve compatibility without normalizing around strings.

   Existing CLI, playback, passive records, and handoff code may continue to
   show `SubjectRef`, node id, and branch id. Those values should be derived
   from the selected occurrence and checked for consistency.

5. Keep self-selection policy separate.

   Do not ban active-parent self-selection as part of this fix. Once selection
   identity is occurrence-based, self-selection can be allowed or disallowed by
   explicit selection policy without corrupting History admission.

## Required Tests

- Two considered payloads with the same `SubjectRef` but different occurrence
  IDs can seal when a specific occurrence is selected.
- Two considered payloads with the same occurrence ID are rejected.
- A selected occurrence absent from the considered set is rejected.
- A selected membership ID with the wrong candidate-set root is rejected.
- The selected occurrence projects to the expected `SubjectRef`, node id,
  branch id, generation, artifact, and runtime.
- Existing traversal replay remains deterministic for the same candidate set.

## Non-Goals

- Do not make `LineageId` itself the candidate key.
- Do not stuff the full artifact/runtime/history hypergraph into
  `CandidateCoordinate`.
- Do not deduplicate by string label as the durable fix.
- Do not weaken History admission checks to accept ambiguous selected labels.
