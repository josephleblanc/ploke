# Agent 28: Combined Child Evidence Review

Date: 2026-05-06

Scope:
- Reviewed reports `19`, `23`, `24`, and `25`.
- Report `26` was requested but was not present in `docs/active/agents/prototype1-hyperagents-design-2026-05-06/`.
- Reviewed changed source in `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`, `history_preview.rs`, `metrics.rs`, and `mod.rs`, plus narrow adjacent selection and live-loop source.
- Source code was not modified.

## Findings

### High: metrics can undo child-evidence ambiguity protection by rebuilding a last-writer-wins branch map

`ChildEvidenceSet` now treats ambiguous branch placement conservatively: conflicting joins remove the key from the indirect join map (`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:382`-`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:410`), and later branch-only sources are refused through `node_for` (`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:501`-`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:527`). That satisfies the Agent 23/24 requirement inside the evidence layer.

`metrics::Assembly::apply_child_evidence` then rebuilds its own `branch_to_node` map from every child branch and evaluation (`crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1184`-`crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1190`). The map is a `BTreeMap<String, String>`, so if two children retain direct evidence for the same branch id, the later child silently wins. Raw evaluation and selection folds then use that map to attach branch-scoped evidence to a row (`crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1323`-`crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1325`, `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1346`-`crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1360`).

This means the metrics projection can re-place the exact branch-only evidence that `ChildEvidenceSet` correctly left unplaced. The result is not History authority corruption, but it is a correctness blocker for treating metrics as a faithful projection of combined child evidence.

Minimal fix: make metrics preserve the evidence-layer ambiguity result. Track branch ids that appear under more than one child and refuse to insert them into `Assembly::branch_to_node`, or expose an unambiguous branch-to-node projection from `ChildEvidenceSet` and consume only that. Add a metrics regression test with two child node records sharing one branch id plus a branch-only evaluation; the evaluation must remain unattached and produce a diagnostic rather than landing on the last child.

### Medium: evidence now depends on successor selection and can project selection inputs from conflicted evidence

`prototype1_state::evidence` imports successor-selection types directly (`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:16`) and exposes `ChildEvidenceSet::selection_inputs()` (`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:38`-`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:50`). That crosses the boundary from source grouping into policy-input construction. The live controller already has a direct adapter from a completed child report to `SelectionInput` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7020`-`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7045`), while the new evidence adapter builds the same successor-selection type from degraded preview evidence.

The bigger correctness issue is that `ChildEvidence::selection_input()` does not account for existing child diagnostics. Conflicting `parent_node_id`, `generation`, `branch_id`, or branch metadata are recorded on `child.diagnostics` by `merge_child`/`merge_branch` (`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:530`-`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:571`, `crates/ploke-eval/src/cli/prototype1_state/evidence.rs:618`-`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:650`), but the selection projection can still return `Ok(SelectionInput)` if the first retained values satisfy required fields (`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:69`-`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:189`). It also uses the first evaluation matching the retained branch id (`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:95`-`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:99`) without diagnosing duplicate/conflicting evaluations for the same branch.

Minimal fix: move this adapter out of `prototype1_state::evidence` into a selection-facing projection module, or keep a neutral evidence-local projection type and convert to `SelectionInput` at the successor-selection boundary. The projection should fail when the child has warning diagnostics relevant to identity, generation, branch, or branch metadata, and should require exactly one usable evaluation for the selected branch unless duplicates are proven equivalent. Add tests for conflict diagnostics causing `SelectionProjectionError` and duplicate branch evaluations not selecting first-by-order.

### Low: metrics diagnostics drop source refs from child evidence diagnostics

Evidence diagnostics carry `source_ref` (`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:300`-`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:306`), but metrics flattens them to strings without the source reference (`crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1127`-`crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1131`, `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1194`-`crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1198`). That makes the operator-facing projection less actionable than the source evidence.

Minimal fix: include `diagnostic.source_ref` when present, or serialize metrics diagnostics as structured records instead of plain strings.

## Notes

No source treatment count bug found in the summary path. `EvidenceProjection::from_set` de-duplicates by source ref before counting class/treatment pairs (`crates/ploke-eval/src/cli/prototype1_state/metrics.rs:255`-`crates/ploke-eval/src/cli/prototype1_state/metrics.rs:298`), so evaluation sources appearing both in `child.documents` and `child.evaluations` do not double-count treatments.

No direct History/Crown authority escalation was found. The new metrics and evidence surfaces remain read-only projections over preview evidence. The main authority risk is future misuse of degraded evidence as successor-selection input before ambiguity and provenance diagnostics become hard failures.

## Verification

Passed:

```text
cargo check -p ploke-eval
cargo test -p ploke-eval --lib prototype1_state::evidence
cargo test -p ploke-eval --lib prototype1_state::metrics::tests::dashboard_exposes_child_evidence_projection_summary
```

Existing warnings remain, mostly dead-code warnings in Prototype 1 History scaffolding and existing `syn_parser` warnings.
