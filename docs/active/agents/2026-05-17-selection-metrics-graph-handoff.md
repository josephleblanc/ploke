# Selection Metrics Graph Handoff

Date: 2026-05-17

## Summary

Selection-time `imp@k` evidence now starts in `ploke-eval` during successor traversal, is sealed into `SelectionDecisionEntry` schema v4, is mirrored by passive `ploke-records` DTOs, and is indexed by `ploke-tree::Graph` as metric evidence. This slice does not render the metrics in `ploke-egui`.

## Implemented Path

- `ploke-eval::successor_selection::metrics` owns the active carriers:
  `Policy`, `Set`, `Candidate`, and `ImpAtK`.
- `run-profile.toml` accepts:
  - `[selection.metrics] persist`, `score_profile`
  - `[selection.metrics.imp_at_k] enabled`, `budget_k`, `archive_scope`,
    `score_points_per_imp_point`, `require_for_score`
- Traversal computes the metric set after History and current-generation candidates are merged, before `frontier_max` / `score_child_prop` choose.
- `score_points_per_imp_point = 0` is persist-only.
- Nonzero `score_points_per_imp_point` adds `imp@k.improvement * points` to the traversal performance score.
- Incomplete `imp@k` contributes zero unless `require_for_score = true`, in which case the candidate is excluded from decision-grade metric scoring.
- `SelectionDecisionEntry` now requires `metrics` and writes schema v4.
- The metric set is bound to `considered_order_hash` and candidate-set root.
- `ploke-tree::Graph` has a passive `MetricIndex`; `SelectionNode` carries the metric set id, and metric candidate rows stay keyed by metric set id + payload index while preserving payload hash, occurrence id, and membership id.

## Verified Surfaces

- `cargo test -p ploke-eval run_profile_metrics_config`
- `cargo test -p ploke-eval imp_at_k`
- `cargo test -p ploke-eval score_child_prop`
- `cargo test -p ploke-records selection_metrics`
- `cargo test -p ploke-tree selection_metrics`

Additional focused check:

- `cargo test -p ploke-records selection_decision`

## Remaining UI Work

- Add borrowed graph/UI witnesses for metric rows before rendering anything in `ploke-egui`.
- Render metric set identity from `SelectionNode` first, then candidate-local `imp@k` rows through typed drilldown.
- Keep the metric rows passive: graph/UI code should not recompute `imp@k`, reinterpret run-profile policy, or upgrade metric evidence into selection authority.

## Notes For The UI/Graph Thread

- Use `Graph.metrics.sets` for set-level facts.
- Use `Graph.metrics.candidates` for candidate-local metric rows.
- Treat `GraphWarningKind::SelectionMetricBindingMismatch` as a bad persisted-evidence warning, not as a renderer failure.
- `projected payload visible` is separate from `drawable in the current renderer/readability path`; no egui visibility was verified in this slice.
