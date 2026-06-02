# Score Child Prop UI Spec

## Goal

The `Candidate Comparison` inspector section should show the actual
`score_child_prop` selector equation and every field used to choose the selected
candidate. The UI claim is not just "candidate A had better metrics"; it is:

```text
this selected candidate won under the sealed `score_child_prop` traversal using
these persisted inputs, this equation, this sample, and these per-candidate
weights.
```

The implementation must keep the same debugger-claim boundary used elsewhere:
typed records -> `ploke_tree::Graph` -> borrowed inspector witness -> render-only
egui rows. `ploke-egui` must not reread History JSON, scrape CLI text, or run a
second selector. The debugger workflow explicitly requires typed graph facts and
borrowed witnesses before rendering (`crates/ploke-egui/docs/model/debugger-claim-workflow.md:23-28`,
`crates/ploke-egui/docs/model/debugger-claim-workflow.md:36-60`).

## Current State

`Candidate Comparison` already exists in the right panel. It renders a parent
node, planned child count, metric policy, selected marker, `imp@k`, and a few
operational/protocol deltas (`crates/ploke-egui/src/ui/app/shell.rs:1578-1658`).
The borrowed witness is `CandidateComparisonSlot` and
`CandidateComparisonCandidate` (`crates/ploke-egui/src/ui/inspector.rs:474-555`).

The graph already ingests sealed selection metric rows into
`MetricSetNode`, `MetricCandidateNode`, and `ComparedRunMetricNode`
(`crates/ploke-tree/src/graph/types/selection.rs:113-176`). During graph build,
`selection.metrics` is bound to the selection entry, then compared-run metric
inputs are copied from the same considered payload's sealed evidence
(`crates/ploke-tree/src/graph/build/selection.rs:283-364`).

What is missing is the selector-equation layer. The current graph/UI path does
not carry `performance`, `oracle_resolved`, `oracle_configured`, `child_count`,
`alpha`, `alpha_mid`, `exploitation`, `exploration`, `weight`, `total_weight`,
`sample`, or the selected cumulative range. Those are the fields a developer
needs to audit why one candidate was selected.

## Authority Trail

The active run profile maps the selected strategy, metric input mode, oracle
mode, and metric policy into `ploke-eval`'s active selection strategy
(`crates/ploke-eval/src/cli/prototype1_state/profile.rs:263-310`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:969-980`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:8225-8287`).

`ParentSelection::select_successor` loads History candidates, adds current
generation candidates, calls `traversal_selection::select_with_policy`, and
then stores the selector result in `SelectionSealMaterial`
(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:8629-8695`).
`SelectionSealMaterial` owns `considered`, `considered_sources`, `traversal`,
and `metrics`, and seals them through
`SelectionDecisionEntry::new_with_traversal_identity_metrics`
(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:769-887`).

The sealed History record stores those fields in
`SelectionDecisionEntryRecord`, including `considered`, `considered_sources`,
`candidate_set`, `traversal`, `metrics`, and `decision`
(`crates/ploke-records/src/history/payload.rs:685-710`). The History
constructor validates that selection metrics are bound to the same considered
order hash and candidate-set root as the decision input
(`crates/ploke-eval/src/cli/prototype1_state/history.rs:5038-5075`,
`crates/ploke-eval/src/cli/prototype1_state/history.rs:5249-5267`).

## Selector Equation To Display

The UI should render this equation exactly for `score_child_prop`:

```text
outcome_points =
  10000 if decision.outcome == Accepted
   5000 if decision.outcome == ExploreFrom
   2500 if decision.outcome == Stop and branch_disposition == Reject
      0 otherwise

operational_points =
  sum(child_metrics.selection_quality_points() for each selection comparison)

protocol_points =
  if traversal.metrics includes protocol:
    sum(treatment_protocol.selection_delta_points(baseline_protocol))
  else:
    0

imp_at_k_delta =
  if imp@k scoring is enabled and complete:
    improvement * score_points_per_imp_point
  else:
    0, or exclude the candidate when require_for_score is true

performance =
  outcome_points + operational_points + protocol_points + imp_at_k_delta

alpha =
  normalized oracle rate when relative oracle scores differ,
  otherwise normalized performance

alpha_mid =
  mean(top_m highest alpha values)

exploitation =
  sigmoid(lambda * (alpha - alpha_mid))

exploration =
  1 / (1 + child_count)

weight =
  exploitation * exploration

sample =
  sha256(seed, domain string, considered payload hashes, candidate ids)[0..8]
  interpreted as a unit interval f64

selected =
  first row whose cumulative weight reaches sample * total_weight,
  with uniform slot fallback when total_weight <= 0 or not finite
```

Citations for the equation:

- Outcome points, operational child metric points, optional protocol points, and
  `imp@k` score delta are assembled in `performance_score` and
  `performance_score_with_set`
  (`crates/ploke-eval/src/successor_selection/traversal.rs:1008-1049`).
- Operational `selection_quality_points` is
  `oracle_eligible * 900`, else `convergence * 500`, else
  `nonempty_valid_patch * 200`, plus patch attempted, minus tool failures,
  partial patch failures, abort, and repair-loop penalties
  (`crates/ploke-eval/src/operational_metrics.rs:172-189`).
- Protocol `selection_points` and `selection_delta_points` include reviewed
  calls/segments, crosswalk counts, focused-progress counts, missing/skipped
  reviews, anchor mismatches, and review-signal penalties
  (`crates/ploke-eval/src/metric.rs:109-160`).
- `imp@k` score delta is gated by policy, row presence, incomplete reasons, and
  `require_for_score`; complete rows add
  `improvement * score_points_per_imp_point`
  (`crates/ploke-eval/src/successor_selection/metrics.rs:72-106`).
- `imp@k` rows themselves carry start identity, evaluator/eval set, budget,
  baseline score, best descendant score, improvement, descendant counts, and
  incomplete reasons (`crates/ploke-eval/src/successor_selection/metrics.rs:276-345`,
  `crates/ploke-records/src/selection.rs:223-283`).
- Alpha, `alpha_mid`, exploitation, exploration, and weight are computed in
  `score_child_prop_weights_with_set`
  (`crates/ploke-eval/src/successor_selection/traversal.rs:1482-1597`).
- Sampling is deterministic over seed, a strategy domain string, payload
  hashes, and candidate ids; selected index is the weighted cumulative row
  (`crates/ploke-eval/src/successor_selection/traversal.rs:1633-1670`).
- Relative oracle scoring uses resolved/configured instance counts, validates
  configured/evaluated instances, and resolves a rate
  (`crates/ploke-eval/src/successor_selection/traversal.rs:1052-1216`).

## Required UI Fields

Header fields:

- `selection_entry_id`
- `procedure_or_policy`
- `scope`
- `strategy = score_child_prop`
- `seed`
- `selected_source`
- `selected_candidate`
- `selected_occurrence_id`
- `selected_membership_id`
- `candidate_set_root`
- `considered_order_hash`
- `metric_set_id`
- `metric_inputs`
- `oracle_mode`
- `oracle_require_evidence`
- `top_m`
- `lambda_millis` and rendered `lambda`
- `total_weight`
- `sample`
- `sample_threshold = sample * total_weight` when `total_weight > 0` and finite
- `uniform_fallback_slot` when `total_weight <= 0` or not finite
- `selected_index`
- `selected_candidate_from_replay`
- `selectable_count`

Metric policy fields:

- `persist`
- `score_profile`
- `imp_at_k.enabled`
- `imp_at_k.budget_k`
- `imp_at_k.archive_scope`
- `imp_at_k.score_points_per_imp_point`
- `imp_at_k.require_for_score`

Per-candidate row fields:

- identity: `payload_index`, `payload_hash`, `candidate`,
  `occurrence_id`, `membership_id`, `node_id`, `branch_id`,
  `branch_disposition`, candidate source
- selected state: selected marker, selected by subject/occurrence/membership
- decision fields: `base_outcome`, `selected_branch_id`, decision rationale
- equation fields: `outcome_points`, `operational_points`,
  `protocol_points`, `imp_at_k_delta`, `performance`
- oracle fields: `oracle_resolved`, `oracle_configured`, `oracle_rate`,
  `oracle_used_for_alpha`
- ranking fields: `child_count`, `alpha`, `alpha_mid`, `exploitation`,
  `exploration`, `weight`, cumulative lower/upper bound, sample hit marker
- selection inclusion fields: `selectable`, `exclusion_reason`,
  `performance_present`, `selection_input_present`, `decision_present`,
  `imp_at_k_score_excluded`
- `imp@k` fields: start node/branch/generation, parent node, primary runtime,
  evaluator, eval set, budget, baseline score, best descendant score,
  improvement, descendant count, scored descendant count, incomplete reasons
- operational input fields from each compared run:
  `instance_id`, `status`, baseline/treatment tool failures, partial patch
  failures, `patch_attempted`, valid patch, convergence, oracle eligible, abort
  flags
- protocol input fields from each compared run:
  baseline/treatment reviewed calls, reviewed segments, missing calls, missing
  segments, skipped segment reviews, anchor mismatches, calls with segment
  crosswalk, calls without segment crosswalk, and focused-progress/review-signal
  counts used by `selection_delta_points`

When a field was not persisted for an older selection entry, the UI must render
`not_recorded` instead of recomputing a best-effort value.

## Record And Graph Shape

The preferred implementation is to persist the selector replay rows at the same
authority boundary that writes `SelectionSealMaterial.metrics`, not to
recompute them in `ploke-egui`.

Required record-level shape:

- Add a strategy-specific formula payload under the selection decision entry
  record, not under `MetricSet` alone. `MetricSet.id` is derived from considered
  order, candidate-set root, metric policy, and metric rows; it does not include
  traversal seed, `top_m`, `lambda_millis`, metric-input mode, oracle mode, or
  `require_evidence`. Those traversal fields affect the formula, so the formula
  must be selection-entry scoped.
- A concrete shape should live under `ploke_records::selection` or
  `ploke_records::history`, then be referenced from
  `SelectionDecisionEntryRecord`, for example:

```rust
pub struct SelectionFormulaRecord {
    pub selection_entry_id: EntryId,
    pub metric_set_id: HistoryHash,
    pub formula: FormulaRecord,
}
```

- The strategy-specific payload should remain structural, for example:

```rust
pub enum FormulaRecord {
    ScoreChildProp(ScoreChildPropRecord),
}
```

- `ScoreChildPropRecord` should carry the header/global fields already exposed
  by `ScoreChildPropReplay`: `seed`, `top_m`, `lambda`, `metric_inputs`,
  `oracle_mode`, `oracle_require_evidence`, `total_weight`, `sample`,
  `selected_index`, `selected_candidate`, and rows
  (`crates/ploke-eval/src/successor_selection/traversal.rs:387-419`,
  `crates/ploke-eval/src/successor_selection/traversal.rs:469-507`).
- Each row should carry at least the current replay fields plus the missing
  equation decomposition: `outcome_points`, `operational_points`,
  `protocol_points`, `imp_at_k_delta`, `selectable`, `exclusion_reason`,
  cumulative lower/upper bound, and whether the sample landed in that range.
- The record should still reference `MetricSet`, because `MetricSet` is the
  selection-time metric evidence sealed beside one decision
  (`crates/ploke-records/src/selection.rs:128-139`). It must not use
  `metric_set_id` as the only formula identity.
  Do not make `ploke-egui` depend on `ploke-eval` to call
  `replay_score_child_prop`.

Required graph-level shape:

- Add a selection-formula index keyed by `selection_entry_id`, or by the pair
  `(selection_entry_id, metric_set_id)`. Formula rows should be keyed by
  `(selection_entry_id, payload_index)` or
  `(selection_entry_id, metric_set_id, payload_index)`.
- Extend `MetricCandidateNode` or add a side index keyed by
  `(selection_entry_id, metric_set_id, payload_index)` for per-candidate formula
  rows.
- Keep `ComparedRunMetricNode` as the source for operational/protocol input
  drilldown (`crates/ploke-tree/src/graph/types/selection.rs:150-176`).
- Preserve the binding checks already present in graph ingestion:
  `metric_set.considered_order_hash` and `candidate_set_root` must match the
  selection decision before the UI treats the formula as authoritative
  (`crates/ploke-tree/src/graph/build/selection.rs:289-320`).

Required egui witness:

- Replace the current reduced `CandidateComparisonCandidate.metric` use with a
  borrowed selector-equation witness that joins:

```text
SelectionNode
  -> MetricSetNode
  -> selection-entry-scoped FormulaRecord::ScoreChildProp
  -> formula row by payload_index
  -> MetricCandidateNode
  -> ComparedRunMetricNode[]
```

- The renderer may still be tabular, but the row object must stay render-only.
  The semantic object is the graph-backed formula witness, not a UI row.

## Display Layout

Use a three-part section:

1. Formula summary:
   Show the equation text, strategy parameters, seed/sample/threshold, selected
   candidate, and whether the selector used performance alpha or oracle alpha.

2. Candidate weight table:
   One row per considered candidate with selected marker, candidate identity,
   performance, alpha, exploitation, exploration, weight, cumulative range, and
   sample-hit marker.

3. Candidate drilldown:
   A collapsible block per row with the score decomposition:
   outcome points, operational points, protocol points, `imp@k` delta,
   compared-run inputs, `imp@k` row, oracle evidence, and decision rationale.

## Tests

Add or update tests at the typed boundary before widget tests:

- `ploke-eval`: a `score_child_prop` selection test that asserts the persisted
  formula record matches `score_child_prop_weights_with_set`, including
  `sample`, `alpha_mid`, per-row weights, cumulative ranges, and selected row.
- `ploke-records`: serde roundtrip for the formula record under
  `selection::MetricSet`.
- `ploke-tree`: graph ingestion test proving formula rows are bound to the same
  `metric_set_id` and `payload_index` as `MetricCandidateNode`.
- `ploke-egui`: focused inspector test proving a selected child Artifact exposes
  the full formula witness, not just `imp@k` and compared-run deltas.

The current focused tests prove the reduced path:
`selected_child_artifact_exposes_candidate_comparison_from_parent_plan` in
`crates/ploke-egui/src/ui/inspector.rs` and
`selection_metrics_ingestion_preserves_metric_bindings` in
`crates/ploke-tree/src/graph/build/selection/tests.rs`. They should be extended
rather than replaced.

## Non-Goals

- Do not display a reconstructed equation from ad hoc UI code.
- Do not parse History JSON or protocol artifact JSON in `ploke-egui`.
- Do not treat `compared_runs` alone as proof of selector weights.
- Do not make a candidate-specific graph selection mode before the inspector
  witness exists.
- Do not claim live native UI verification from focused tests.

## Acceptance Criteria

- A user can select an Artifact or run-forest node and see why the selected
  candidate won under `score_child_prop`.
- Every numeric field in the equation is either read from a sealed typed record
  or displayed as `not_recorded`.
- The UI shows both the equation and the actual values substituted into it.
- The selected row can be audited by recomputing the weighted-sample threshold
  from the displayed fields.
- No selector semantics are implemented in `ploke-egui`; formula semantics are
  persisted at selection time or graph-owned before rendering.
