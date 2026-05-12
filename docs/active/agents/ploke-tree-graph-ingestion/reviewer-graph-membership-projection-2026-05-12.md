# Review: Scoped Membership Projection Repair

Reviewer: `reviewer-graph`
Date: 2026-05-12

## Scope

Reviewed the scoped membership projection repair against the assigned acceptance:

- set-scoped membership identity is preserved
- no vector-index matching remains
- report only; no source edits

Changed files by this review:

- `docs/active/agents/ploke-tree-graph-ingestion/reviewer-graph-membership-projection-2026-05-12.md`

## Findings

### 1. Fine playback still exposes a lossy public membership projection

Severity: substantive

Evidence:

- `crates/ploke-tree/src/playback/fine.rs:31` maps richer `FineHistoryStep` values into `RunPlayback<Fine>`.
- `crates/ploke-tree/src/playback/fine.rs:35-43` drops `candidate_set_root` when constructing `FineStep`.
- `crates/ploke-tree/src/playback/fine.rs:190-323` computes root-scoped IDs for borrowed fine playback, but `FineStepRef` still has no `candidate_set_root` field.

Impact:

`fine_history_steps_from_sealed_history` and the fine browser model preserve the root, but the existing public `fine_run_playback_from_sealed_history` and borrowed fine playback APIs still expose only `membership_id`. A caller using the normal `RunPlayback<Fine>` path cannot recover the required `{ candidate_set_root, membership_id }` pair except by parsing the formatted step id. That leaves the original projection gap partially open for non-browser consumers and violates the handoff requirement that fine playback preserve set-scoped candidate membership identity before UI/detail keys rely on membership IDs.

Expected repair:

Carry the root through the common fine playback DTOs, or replace the bare membership field with a typed/set-scoped projection at the playback boundary. The browser should not need a side-channel `FineHistoryStep` path to avoid losing the root.

### 2. Duplicate bare candidate labels can be assigned the selected membership

Severity: substantive

Evidence:

- `crates/ploke-tree/src/playback/fine.rs:335-352` gives selected membership/occurrence precedence when `selected_applies_to` is true.
- `crates/ploke-tree/src/playback/fine.rs:388-391` treats `selected_candidate == candidate.candidate` as sufficient when the payload lacks coordinate/selection-input identity.
- `crates/ploke-tree/src/graph/build/selection/membership.rs:63-83` has the same selected-membership precedence in graph ingestion.
- `crates/ploke-tree/src/graph/build/selection/membership.rs:53-54` has the same selected-candidate fallback.
- `crates/ploke-tree/src/graph/build/selection.rs:202-220` then records the returned membership and set-scoped key on the candidate node.

Impact:

If a selection contains two considered payloads with the same `SubjectRefRecord` and no coordinate or selection-input identity, `selected_applies_to` is true for both rows. The selected membership lookup then resolves the same selected `membership_id` for each matching label, so the graph/fine projection can attach the selected membership key to more than one considered payload. That is not vector-index matching, but it is still not membership-safe: `CandidateSetMembershipRecord` binds `payload_hash`, and the projection is ignoring that binding when labels collide.

Expected repair:

For selected-member precedence, require a payload-unique relation such as payload hash, coordinate, occurrence id, or another typed identity. If only the candidate label is available and multiple considered payloads share it, project the membership as ambiguous/missing rather than assigning the selected membership to every matching row. Add regression coverage for duplicate bare candidate labels with distinct candidate-set memberships and one selected membership.

## Verification

Commands run:

- `cargo test -p ploke-tree candidate_membership --features projection 2>&1 | tail -n 60`
  - passed: 3 tests
- `cargo test -p ploke-tree membership --features projection 2>&1 | tail -n 60`
  - passed: 12 tests
- `cargo test -p ploke-tree same_membership_id_under_different_candidate_set_roots_keeps_both_nodes --features projection 2>&1 | tail -n 60`
  - passed: 1 test

Residual test gap:

Current tests cover reordered memberships, no vector-index fallback, ambiguous duplicate labels in the non-selected path, and same membership id under different roots. They do not cover duplicate bare selected-candidate labels where selected-membership precedence can attach one membership to multiple payload rows.
