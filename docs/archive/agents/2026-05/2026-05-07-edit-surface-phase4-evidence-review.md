# Phase 4 Edit-Surface Evidence Review

Date: 2026-05-07

Scope: current uncommitted changes in:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`

Context read:

- `docs/archive/agents/2026-05/2026-05-07-edit-surface-implementation-handoff.md`
- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-implementation-plan.md`
- `docs/workflow/evalnomicon/drafts/2026-05-07-prototype1-edit-surface-model.md`

## Summary

The live checked-edit construction path is pointed in the right direction:
`CheckedSurfaceEdit` comes from the backend-owned check/apply path, then
`surface_evidence_from_checked_edit` derives `SurfaceEvidence` from that carrier
and stores it on both `ChildFiles` and the sealed `CandidateArtifact` payload.
Legacy serde compatibility also looks okay because the new fields are optional
with `serde(default, skip_serializing_if = "Option::is_none")`.

I would not land the patch as-is. There are two correctness gaps that can let
requested edit-surface generation or selected candidate evidence silently drift
away from checked surface evidence, and one replay-data gap that leaves History
short of the Phase 4 authority-chain target.

## Findings

### High: Existing child plans can bypass the requested TUI edit-surface generator

`resolve_child_plan` now accepts an existing child-plan file even when the
command requests `Prototype1CandidateGenerator::TuiEditSurface`
(`cli_facing.rs:5497-5519`). The previous explicit fail-closed error for
existing plans was removed. The new post-load validation calls
`validate_deterministic_surface_evidence`, but that function returns `Ok(())`
for any child whose `resolved.branch.synthesized_spec_id` is not the deterministic
TUI producer (`cli_facing.rs:923-926`).

That means a stale or legacy child plan with non-TUI children can be reused under
`--candidate-generator tui-edit-surface` without any checked surface evidence.
This is the exact silent downgrade the patch comments say deterministic
edit-surface candidates must avoid (`history.rs:2678-2681`).

Impact: a run can claim to use the bounded edit-surface generator while actually
selecting from an existing non-surface plan. History would then have no durable
surface evidence because no checked edit happened for that plan.

Expected fix shape: when the command requests `tui-edit-surface`, either reject
any pre-existing child plan unless every child is from `TUI_EDIT_SURFACE_PRODUCER_ID`
and has valid surface evidence, or force regeneration through the checked
surface path.

### High: SurfaceEvidence is not re-bound to candidate artifact identity before sealing or handoff

The direct live construction is good: `child_files_from_checked_edit` copies
`patch_id`, `base_artifact_id`, and `derived_artifact_id` from
`CheckedSurfaceEdit` into the node/resolved branch (`cli_facing.rs:516-526`),
then builds `SurfaceEvidence` from the same checked carrier
(`cli_facing.rs:527-575`).

The later acceptance paths do not preserve that binding. The deterministic
evidence validator only checks producer id and target path
(`cli_facing.rs:934-955`). `candidate_artifact_from_outcome` attaches any
`SurfaceEvidence` present on the outcome without checking it against the node or
resolved branch (`cli_facing.rs:6373-6385`). `select_artifact_for_handoff` only
checks that deterministic TUI artifacts have some surface evidence
(`cli_facing.rs:6407-6413`).

The candidate record already has identities to compare against:
`TreatmentBranchNode` carries `patch_id` and `derived_artifact_id`
(`branch_registry.rs:43-59`), and the node carries the base/patch/derived
artifact ids populated from the checked edit. None of these are checked against:

- `surface.base.artifact_id`
- `surface.after.artifact_id`
- `surface.patch_id`
- `surface.source_content_hash`
- `surface.proposed_content_hash`
- `surface.policy`
- `surface.touches_digest` / `surface.delta_digest`

Impact: a persisted `ChildFiles` message or historical candidate payload can
carry surface evidence for the same target path but a different base artifact,
derived artifact, patch, or content hash. Replay/scoring would then attribute
the selected candidate to the wrong delta without re-reading projections.

Expected fix shape: add one authoritative binding check before sealing and
handoff, ideally near `CandidateArtifact::with_surface` or as a private
`SurfaceEvidence` validation method used by `ChildFiles` and
`CandidateArtifact`. The check should compare surface base/after/patch/content
fields to node/resolved branch identity and fail closed for deterministic TUI
candidates.

### Medium: Durable data is enough for v1 material-delta display, but not enough to replay the authority chain

The new `SurfaceEvidence` fields cover useful material evidence: producer id,
proposal id, run id, policy, target path, base/after artifact refs, patch id,
content hashes, touches, replacement hashes, a touches digest, a delta digest,
and checked/applied statuses (`history.rs:2683-2700`). That is likely enough to
show and re-score a deterministic single-file splice without reading mutable
branch projection files.

It is not enough to replay the Phase 4 authority chain described in the plan and
model. The Phase 4 plan asks for generator runtime id, graph rule ids/digests,
projection artifact id/hash, graph bounds digest, canonical targets, material
spans, check result, derived artifact id, and later child evidence
(`2026-05-07-prototype1-edit-surface-implementation-plan.md:355-381`). The
model also calls out readable/writable grant identifiers or commitments,
proposal/request/call ids, edit mode, touched spans, hashes, derived artifact,
and validation result (`docs/workflow/evalnomicon/drafts/2026-05-07-prototype1-edit-surface-model.md:586-598`).

The current patch does not persist a grant id/commitment, graph rule digests,
projection identity, graph bounds digest, edit mode, request/call id, or actual
generator runtime id. `producer_id` and `run_id` are useful, but they are not the
same as the parent/runtime authority that generated and admitted the transition.

Impact: future replay can diagnose "this material delta was carried on the
candidate", but cannot prove why the surface bounds admitted it or revise
semantic/code-graph attribution without returning to derived projections or
adapter assumptions.

Expected fix shape: either narrow the Phase 4 claim to "material single-file
splice evidence only" or add durable commitments for the surface grant, graph
projection, graph rules/bounds, edit mode, and generator runtime/coordinate.

### Medium: Visibility still lets crate-local code mint arbitrary checked/applied evidence

`SurfaceEvidence`, `SurfaceArtifactRef`, and `SurfaceTouch` expose their fields
as `pub(crate)` (`history.rs:2683-2719`), and `CandidateArtifact::with_surface`
accepts any `SurfaceEvidence` (`history.rs:2819-2822`). The status enums also
have only successful states, so constructing the struct is equivalent to
claiming "checked" and "applied".

This is not as bad as a public API, but it is still broader than the authority
model in the Prototype 1 guidance. The durable record should be the projection
of an allowed checked/apply transition, not a bag of crate-visible status
fields. The current constructor computes digests, but crate-local callers can
bypass it and create inconsistent digests or status claims.

Expected fix shape: keep the evidence fields private, expose accessors, and make
the only ordinary constructor derive evidence from the checked backend carrier
or from a validation function that recomputes digests and binds the candidate
artifact identity.

## Serde Compatibility

Legacy compatibility looks acceptable for the touched storage boundaries:

- `ChildFiles.surface` is optional and skipped when absent (`parent.rs:171-177`).
- `CandidateArtifact.surface` is optional and skipped when absent
  (`history.rs:2797-2803`).
- Existing candidate artifacts without `surface` should deserialize with
  `surface: None`.

The new `SurfaceEvidence` itself has required fields, but it is nested under an
optional field, so old records are not forced to contain it.

## Positive Notes

- `surface_evidence_from_checked_edit` derives its fields from
  `CheckedSurfaceEdit` and the checked `ArtifactDelta`, not from rendered CLI
  output (`cli_facing.rs:534-575`).
- `ChildFiles` carries the evidence through the child plan (`parent.rs:171-177`,
  `parent.rs:233-255`).
- `CandidateArtifact` carries the evidence in the sealed selection payload
  (`history.rs:2797-2803`).
- `selection-show` exposes evidence presence, proposal id, and delta id for
  operator diagnosis (`history_preview.rs:853-878`).

## Verification

Commands run:

```bash
cargo fmt --all -- --check
/bin/bash -lc 'cargo check -p ploke-eval 2>&1 | tail -n 80'
/bin/bash -lc 'cargo test -p ploke-eval prototype1_state --lib 2>&1 | tail -n 20'
/bin/bash -lc 'cargo test -p ploke-eval successor_selection --lib 2>&1 | tail -n 20'
```

Results:

- `cargo fmt --all -- --check`: passed with no output.
- `cargo check -p ploke-eval`: passed; the useful tail contained existing
  warnings only.
- `prototype1_state` filtered tests: passed, `192 passed; 0 failed`.
- `successor_selection` filtered tests: passed, `20 passed; 0 failed`.

## Main-Thread Resolution

After this review, the main thread patched the two High findings before commit:

- `resolve_child_plan` now validates requested `tui-edit-surface` child plans
  with `validate_requested_tui_surface_child`, which rejects non-deterministic
  producer entries instead of accepting legacy plan children without surface
  evidence.
- Surface evidence is now re-bound before current-generation sealing and before
  selected Artifact handoff. The binding check verifies producer, policy, target,
  source/proposed content hashes, base Artifact id, derived Artifact id, patch id,
  operation target, generation target, proposal/apply id, and recomputed
  touches/delta digests.

The Medium findings remain scoped future work:

- Persisting the full graph grant/projection/bounds authority chain is still not
  part of this Phase 4 slice.
- Surface evidence fields remain crate-visible pending a smaller visibility/API
  cleanup.

Follow-up verification after the patch:

```bash
cargo fmt --all
/bin/bash -lc 'cargo check -p ploke-eval 2>&1 | tail -n 80'
/bin/bash -lc 'cargo test -p ploke-eval prototype1_state --lib 2>&1 | tail -n 40'
/bin/bash -lc 'cargo test -p ploke-eval successor_selection --lib 2>&1 | tail -n 20'
```

Results:

- `cargo check -p ploke-eval`: passed with existing warnings only.
- `prototype1_state` filtered tests: passed, `194 passed; 0 failed`.
- `successor_selection` filtered tests: passed, `20 passed; 0 failed`.
