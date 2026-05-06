# 2026-05-06 History Traversal Invariant Review

Status: reviewed the active workspace changes around History traversal candidate selection and the current-generation bridge. I found no confirmed duplicate-candidate regression in `History::candidates`; the main open correctness gap is still historical Runtime/Artifact hydration before handoff. No tests were run for this review.

Post-review update: after this report was written, the generation-local scope construction was tightened from an ad hoc CLI helper into typed History scope construction: `Scope<Generation>` and `ScopeFor<Generation>` in [history.rs:3593](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L3593), implemented by the parent-turn selection carrier in [cli_facing.rs:5832](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L5832). The findings below remain oriented around the same invariants.

## Findings

### High: Historical selections still do not have a full structural Runtime/Artifact hydration path

Type: residual design risk / correctness gap, not a newly introduced confirmed bug.

The `mod.rs` model says a successor runtime must be built from the selected Artifact and verify sealed material before becoming the next Parent: [mod.rs:109](../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs#L109), [mod.rs:140](../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs#L140), [mod.rs:221](../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs#L221). The History module still records that whole-artifact and runtime/build identities need canonical refs in several paths: [history.rs:369](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L369).

The live handoff validation for a historical traversal selection checks node id, branch id, selected payload presence, sealed candidate evidence, and a non-empty `primary_runtime_id`: [cli_facing.rs:5844](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L5844), [cli_facing.rs:5905](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L5905). That prevents the worst unresolvable historical handoff case, and there is a focused test for missing runtime identity: [cli_facing.rs:8207](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L8207).

However, after this validation the parent still loads a mutable node record by `candidate_node_id` and calls the successor handoff path with that node id and the current repo root: [cli_facing.rs:6367](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L6367), [cli_facing.rs:6451](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L6451). A non-empty historical runtime id is only evidence that a runtime was observed; it is not yet a typed `Artifact -> Runtime -> Parent` hydration carrier. This leaves the historical Runtime/Artifact hydration gap open: a historical candidate can be selected by admitted History evidence, but the handoff path is not yet structurally forced to hydrate the exact admitted Artifact/Runtime from that evidence.

Next step: introduce or require a historical selection handoff carrier that resolves sealed candidate evidence to an admitted Artifact identity and runtime hydration target before `spawn_and_handoff_prototype1_successor`.

### Medium: Traversal source provenance is structural in memory but not sealed in the decision entry

Type: residual design risk.

The current-generation bridge is conceptually compatible with the `mod.rs` model while it remains a selector input: `History::candidates` reads admitted sealed blocks with block/entry provenance [history.rs:885](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L885), while fresh local children are projected through `ParentSelection::current_generation_candidates` [cli_facing.rs:5661](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L5661) and added to a typed union as `TraversalCandidateSource::CurrentGeneration` [traversal.rs:156](../../../crates/ploke-eval/src/successor_selection/traversal.rs#L156), [traversal.rs:209](../../../crates/ploke-eval/src/successor_selection/traversal.rs#L209). The selector records whether the winner came from current generation state: [traversal.rs:129](../../../crates/ploke-eval/src/successor_selection/traversal.rs#L129).

The durable `SelectionDecisionEntry`, however, stores one `scope` and a flat `Vec<EvaluationPayload>`: [history.rs:3233](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L3233), [history.rs:3244](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L3244), [history.rs:3252](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L3252). The in-memory `selected_from_generation_outcomes` flag lives in `SelectionSealMaterial` [cli_facing.rs:348](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L348) and is used before sealing [cli_facing.rs:6438](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L6438), but it is not itself part of `SelectionDecisionEntry`.

This does not currently create duplicate candidate evidence because traversal entries are filtered out of later candidate reads. It does mean a sealed traversal decision that mixed historical and current candidates does not durably preserve the source union as structure. Later auditors must infer source from prior History and payload shape, or rely on the procedure-level exclusion. That is weaker than the evidence-surface goal of preserving candidate source provenance.

Next step: persist candidate-source provenance for traversal considered sets, or split traversal evidence into explicit historical and current-generation slices while keeping the selector's comparable `EvaluationPayload` view as a projection.

### Medium: `history:all_admitted_candidates` is preserved by filtering traversal decisions, but its meaning is now narrower and should stay explicit

Type: confirmed invariant preservation with terminology risk.

`SelectionScope::all_admitted_candidates` includes generation-local scopes by definition: [history.rs:3593](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L3593), [history.rs:3610](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L3610). Before the filter, a sealed traversal decision with scope `history:all_admitted_candidates` could be read later as another source of candidates, replaying its entire considered set. The current change prevents that by requiring candidate-contributing decisions to use the base generation-local successor procedure: [history.rs:895](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L895), [history.rs:3341](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L3341). The new test covers the replay case: [history.rs:5609](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L5609).

This preserves the useful meaning of `history:all_admitted_candidates` as all admitted candidate-bearing local selection material, not all payloads that happen to appear inside any admitted decision. That is the correct direction for avoiding duplicate evidence and traversal self-amplification. It should be documented in code or type names if the scope string remains broad, because the phrase "all admitted candidates" can otherwise be misread as including traversal-considered projections.

Next step: make the candidate-source predicate explicit in public docs or type vocabulary, for example "all admitted candidate sources" rather than "all admitted decision payloads".

### Low: Current-generation bridge preserves the generation-local default, but integration coverage is incomplete

Type: test gap.

The default generation-local path is still separate. `Prototype1SuccessorSelection::GenerationLocal` calls `generation_selection` and only materializes generation-local seal material when a successor is actually handed off: [cli_facing.rs:6331](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L6331), [cli_facing.rs:6428](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L6428). The traversal path explicitly opts into History plus current-generation candidates: [cli_facing.rs:6336](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L6336), [cli_facing.rs:5784](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L5784).

There is a unit test that traversal scores current-generation candidates with historical candidates: [traversal.rs:559](../../../crates/ploke-eval/src/successor_selection/traversal.rs#L559). There is also the no-reingestion History test cited above. I did not run tests in this review. Missing coverage remains for a full CLI-facing traversal selection where the winner is current-generation, the sealed entry includes the mixed considered set, and a later `History::candidates(history:all_admitted_candidates)` still returns only admitted candidate sources. Missing coverage also remains for the historical winner path with a runtime id present but no resolvable admitted Artifact hydration target.

Next step: add focused integration tests for those two paths before relying on history traversal for longer campaigns.

## Direct Questions

### Is the current-generation bridge conceptually compatible with the `mod.rs` model?

Yes, with the provenance caveat above. Current local candidates are comparable because they are projected into the same `EvaluationPayload` decision-grade shape and tagged as current-generation traversal sources before scoring. They are not read from `History::candidates`, so they are not treated as already admitted History at selector input time. If a current-generation candidate wins, handoff validation treats it as selected from current generation outcomes rather than requiring historical runtime identity.

The compatibility weak point is durability: the sealed traversal decision does not preserve the current-vs-history source union as first-class sealed structure. The procedure filter prevents later re-feeding, but the record shape still collapses source provenance into a flat considered payload list.

### Does filtering traversal decisions preserve or weaken `history:all_admitted_candidates`?

It preserves the intended invariant. Traversal decisions are admitted History entries, but they are decisions over candidate evidence, not new candidate-source evidence. Filtering them out prevents traversal projections from being promoted into future candidate sources and avoids duplicate candidate evidence. The only weakening is terminological: the scope string needs to be understood as all admitted candidate-source decisions, not every admitted decision that contains candidates.

## Review Frame

Surface Request: Review History traversal and current-generation bridge against Prototype 1 History invariants.

Causal Chain: `mod.rs` conceptual model -> History entries/blocks/candidate projection -> traversal candidate union -> sealed `SelectionDecisionEntry` -> handoff/runtime hydration.

Concern: The implementation must preserve History as authority over admitted evidence while allowing fresh local children to be compared without pretending they are already admitted History.

Evidence Surface: `SelectionInput` binding, candidate-set commitments/proofs, `SelectionDecisionEntry` procedure/scope, candidate source provenance, Crown/History claims in `mod.rs`, runtime/artifact hydration evidence.

Existing Algebra: Artifact, Runtime, Parent/Ruling, Crown/Locked, History, Block, Entry, `SelectionDecisionEntry`, `EvaluationPayload`, `HistoryCandidates`.

Missing Structure: remaining places where current projections, historical admissions, Crown authority, or runtime hydration are conflated by convention rather than structure.

Transformation: audit and document invariant preservation/gaps; do not patch code.

Projection: report file with findings, evidence, and next steps.

Preservation Check: no overclaiming History authority, no duplicate candidate evidence, no unhydrated historical handoff, no regression of generation-local default.

## Verification

Commands inspected:

- `git status --short`
- `git diff -- crates/ploke-eval/src/cli/prototype1_state/history.rs crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs crates/ploke-eval/src/successor_selection/traversal.rs`

No test suite or targeted test was run for this review.
