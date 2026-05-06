# Agent 29: Combined Review Fixes

Date: 2026-05-06

Scope:
- Implemented focused fixes from report `28-combined-child-evidence-review.md`.
- Modified `crates/ploke-eval/src/cli/prototype1_state/evidence.rs` and `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`.
- Preserved the evidence-layer/read-only boundary; no History admission, scoring, archive traversal, or CLI exposure was wired.

## Changes

### Metrics Branch Placement

Metrics now consumes an explicit unambiguous branch placement projection from `ChildEvidenceSet`.

If a branch id appears under more than one child, metrics records the branch as ambiguous, removes it from the branch-to-node map, and refuses later branch-scoped attachment for evaluations and mutable selection projections. Raw document ingestion also goes through the same guarded branch insertion path, so later node/result records cannot rebuild a last-writer-wins placement.

Added a regression test with two child node records sharing one branch id plus a branch-only evaluation. The evaluation remains unattached to both rows and metrics retains diagnostics explaining the ambiguous branch and unattached evaluation.

Metrics diagnostics now include child evidence `source_ref` values where available.

### Selection Projection

Selection projection now fails when retained child evidence has warning diagnostics for:
- `parent_node_id`
- `generation`
- `branch_id`
- branch metadata: `candidate_id`, `source_state_id`, or `target_relpath`

Selection projection also requires exactly one evaluation record for the selected branch. Duplicate branch evaluations now produce a projection failure instead of selecting the first record by iteration order.

Added regression coverage for conflicted identity/generation/branch evidence, branch metadata conflicts, and duplicate evaluations.

## Deferred Boundary Work

The direct `prototype1_state::evidence -> successor_selection` import remains. Moving the adapter into a selection-facing projection module is still the cleaner API boundary, but that change would touch ownership outside this focused patch. The correctness guard is now fail-closed for known child evidence conflicts and duplicate evaluations, so the remaining concern is API shape rather than silent acceptance of conflicted degraded evidence.

## Verification

Passed:

```text
cargo fmt --all
cargo check -p ploke-eval
cargo test -p ploke-eval --lib prototype1_state::evidence
cargo test -p ploke-eval --lib prototype1_state::metrics::tests::dashboard_exposes_child_evidence_projection_summary
cargo test -p ploke-eval --lib prototype1_state::metrics::tests::branch_only_evaluation_is_not_attached_to_ambiguous_child_branch
```

Existing warnings remain, mostly in `syn_parser` and Prototype 1 History scaffolding.
