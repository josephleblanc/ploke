# Candidate Comparison Popout Chart Review

Reviewer: `cc-chart-reviewer`  
Date: 2026-05-19  
Scope: future popout charts for `score_child_prop` selection data in `ploke-egui`

## Findings And Recommendations

1. The popout should be keyed by the current selection formula identity, not by `metric_set_id` alone.
   `ploke-tree` now indexes formula values by `(selection_entry_id, metric_set_id)` in `SelectionFormulaKey`, and the egui lookup follows that same key through `selection_formula_for_selection` (`crates/ploke-tree/src/graph/types/selection.rs:183`, `crates/ploke-egui/src/ui/inspector.rs:2044`). A chart opened from Candidate Comparison should therefore bind to the selected `CandidateComparisonSlot` and its resolved `SelectionFormulaNode`, not to a global metric-set view.

2. The first chart implementation should use immediate-mode `egui::Painter` charts inside a popout, not a new chart dependency.
   `ploke-egui` currently depends on `eframe`, `egui`, `egui_extras`, `egui_graphs`, and `egui_tiles`, with no `egui_plot` or plotting crate present (`crates/ploke-egui/Cargo.toml:11`). There is a local `ui::charts` module, but it is currently an unused example horizontal bar helper over `&[(&str, f32)]` (`crates/ploke-egui/src/ui/charts/mod.rs:6`, `crates/ploke-egui/src/ui/charts/bar.rs:4`). Treat it as a painter starting point, not as the semantic model for Candidate Comparison. The required charts are categorical bars, cumulative intervals, and compact heatmaps; these are simpler and more controllable with painter rectangles, lines, and hover hitboxes than with a general plot library. Add `egui_plot` only after a concrete need appears for zoomable numeric axes.

3. Keep typed graph witnesses as the source and make chart rows render-only.
   The existing witness path is `CandidateComparisonSlot -> CandidateComparisonChildSlot -> CandidateComparisonCandidate<'g> -> CandidateSelectorFormula<'g>`, borrowing `ScoreChildPropRowRecord` from the graph-backed formula (`crates/ploke-egui/src/ui/inspector.rs:477`, `crates/ploke-egui/src/ui/inspector.rs:577`). The chart may build a short-lived render list for layout, but it should borrow row, metric, and child references for the current frame. Do not introduce persisted chart DTOs, JSON readers, or cached semantic rows.

4. Recommended popout layout: coordinated small multiples, not one overloaded plot.
   Put candidates on a shared vertical axis in the same order as the current comparison table, then stack chart bands:
   - weighted sampling interval band
   - score decomposition band
   - alpha/exploitation/exploration/weight band
   - metric drilldown heatmap band
   This preserves side-by-side comparison between comparable nodes without forcing unrelated units onto one axis.

## Chart Forms

### Side-By-Side Candidate Comparison

Use a horizontal bar matrix with one row per candidate. Candidate labels should come from the already-resolved row candidate or candidate subject, matching the current renderer fallback from `candidate.subject.value` to child branch candidate id (`crates/ploke-egui/src/ui/app/shell.rs:1839`). Recommended columns:

- `performance`
- `weight`
- `alpha`
- `exploitation`
- `exploration`
- `child_count`

For each numeric cell, render a bar normalized within the current formula row set. Keep raw values in hover text and the table, because normalization makes cross-formula comparisons unsafe. Mark the selected row with the existing `selected` value carried by `ScoreChildPropRowRecord` and the UI witness (`crates/ploke-records/src/selection.rs:235`, `crates/ploke-egui/src/ui/inspector.rs:551`).

### Sample Threshold And Cumulative Weight

Use a single cumulative interval chart:

- x-axis: `0..total_weight`
- candidate segment: `[cumulative_lower, cumulative_upper]`
- vertical marker: `sample_threshold`
- highlight: `sample_hit`
- fallback annotation: `uniform_fallback_slot` when present

The exact fields are persisted on `ScoreChildPropRecord` and `ScoreChildPropRowRecord` (`crates/ploke-records/src/selection.rs:169`, `crates/ploke-records/src/selection.rs:224`). The upstream calculation constructs `sample_threshold`, `uniform_fallback_slot`, and cumulative ranges from the replayed selection formula (`crates/ploke-eval/src/successor_selection/traversal.rs:654`, `crates/ploke-eval/src/successor_selection/traversal.rs:1953`). This chart is the clearest way to show why the selected candidate won under weighted sampling.

### Score Decomposition

Use stacked horizontal bars per candidate:

- `outcome_points`
- `operational_points`
- `protocol_points`
- `imp_at_k_delta`

The row already stores all four components plus `performance` and `imp_at_k_score_excluded` (`crates/ploke-records/src/selection.rs:197`). If `imp_at_k_delta` is negative, render it as a diverging segment around zero. If `imp_at_k_score_excluded` is true, show the imp@k segment as a hatched or muted overlay rather than adding it into the total. This keeps the chart faithful to the actual selector equation rather than the table-only summary.

### Alpha, Exploitation, Exploration, And Weight

Use small multiples rather than a dual-axis plot:

- candidate rank vs `alpha`
- candidate rank vs `exploitation`
- candidate rank vs `exploration`
- candidate rank vs final `weight`

The formula uses normalized alpha, sigmoid exploitation, exploration penalty, then final `weight = exploitation * exploration` (`crates/ploke-eval/src/successor_selection/traversal.rs:1902`, `crates/ploke-eval/src/successor_selection/traversal.rs:1929`). The chart should draw `alpha_mid` as a horizontal reference line for the alpha and exploitation bands. Avoid combining `child_count` and `weight` on one numeric axis; use `child_count` as hover text or a small bar because it has a different unit.

### Metric Drilldowns

Use a compact heatmap or diverging bar grid keyed by candidate row:

- `imp@k improvement`, `baseline_score`, `best_descendant_score`
- `tool_failures_delta`
- `patch_failures_delta`
- `valid_patch`
- `converged`
- `oracle_eligible`
- `protocol_reviewed_delta`
- `protocol_missing_delta`

The current table already renders these fields from `MetricCandidateNode.imp_at_k` and `ComparedRunMetricNode` helpers (`crates/ploke-egui/src/ui/app/shell.rs:1882`, `crates/ploke-tree/src/graph/types/selection.rs:155`). For multiple compared runs per candidate, default the popout to an aggregate row with an expandable per-run detail band. Do not flatten compared-run details into a single score unless the selection formula itself used that aggregate.

## Implementation Boundaries

- Source truth remains `ploke-records -> ploke-tree::Graph -> CandidateComparisonSlot/CandidateComparisonCandidate<'g> -> renderer`.
- The chart renderer may create a frame-local render projection such as `Vec<ChartRow<'g>>`, but each row should borrow `ScoreChildPropRowRecord`, `MetricCandidateNode`, and `ChildPlanChildRecord`. This projection is layout data, not a semantic source.
- If caching is needed, cache only geometry or formatted render-boundary artifacts keyed by selection entry id, metric set id, graph revision, chart mode, and available rectangle size.
- Do not parse History JSON, benchmark JSON, or CLI output to populate chart fields. The current formula row already comes through `SelectionFormulaNode` (`crates/ploke-tree/src/graph/types/selection.rs:202`).
- If extending `crates/ploke-egui/src/ui/charts`, change its input away from `&[(&str, f32)]` before connecting it to selection data. A string/f32 tuple API would erase selected/sample/excluded state and lose the borrowed graph witness.
- Add a dedicated benchmark scenario only when the popout is implemented. Current benchmark scenarios cover inspector sections and modes, but not Candidate Comparison or a popout chart (`crates/ploke-egui/src/benchmark.rs:77`).

## Performance And UX Risks

- The current Candidate Comparison renderer calls `slot.resolve_child(graph, child)` per row and does graph searches/fallback matching in the render path (`crates/ploke-egui/src/ui/app/shell.rs:1709`, `crates/ploke-egui/src/ui/inspector.rs:512`). A popout chart should avoid repeating those searches in each chart band; resolve once per frame into borrowed rows and pass slices to subcharts.
- Avoid per-frame `format!` for every numeric cell in chart labels. The table currently formats floats through `format_f64` (`crates/ploke-egui/src/ui/app/shell.rs:1964`); chart axes should prefer cached labels or hover-only formatting.
- Do not make the default right panel denser. Put charts behind an explicit popout/open control so the current table remains the detailed audit surface.
- Keep the popout scroll/zoom model simple. The candidate count is expected to be small per selection, so dense bars and hover details are preferable to pan/zoom plots.
- Add accessibility-safe encodings: selected row outline, sample-hit icon/marker, and text hover details. Color alone is insufficient for selected/sample/excluded states.

## Commands Run

- `sed -n '1,220p' .orchestrator/workers/cc-chart-reviewer.md`
- `sed -n '1,220p' .codex/skills/ploke-egui-benchmarking/SKILL.md`
- `sed -n '1,220p' .codex/skills/ploke-debugger-claim-workflow/SKILL.md`
- `sed -n '1,220p' .codex/skills/ui-claim-archaeology/SKILL.md`
- `sed -n '1,220p' crates/ploke-egui/docs/model/debugger-claim-workflow.md`
- `rg -n "egui_plot|plot|chart|serde|puffin|native-benchmark|egui" crates/ploke-egui/Cargo.toml Cargo.toml crates/*/Cargo.toml`
- `rg -n "CandidateComparison|SelectionFormula|ScoreChildProp|score_child_prop|formula|cumulative|exploitation|exploration|metric_set" crates/ploke-egui/src crates/ploke-tree/src crates/ploke-records/src crates/ploke-eval/src/successor_selection crates/ploke-eval/src/cli/prototype1_state`
- `rg -n "Plot|BarChart|Line|Points|egui_plot|plotters|puffin|Frame|frame_time|fps|repaint|serialize|Deserialize|Serialize" crates/ploke-egui/src crates/ploke-egui/docs`
- `nl -ba crates/ploke-egui/src/ui/inspector.rs | sed -n '470,600p'`
- `nl -ba crates/ploke-egui/src/ui/inspector.rs | sed -n '1870,2065p'`
- `nl -ba crates/ploke-egui/src/ui/app/shell.rs | sed -n '1620,1885p'`
- `nl -ba crates/ploke-egui/src/ui/app/shell.rs | sed -n '1880,1975p'`
- `nl -ba crates/ploke-tree/src/graph/types/selection.rs | sed -n '100,240p'`
- `nl -ba crates/ploke-records/src/selection.rs | sed -n '128,285p'`
- `nl -ba crates/ploke-eval/src/successor_selection/traversal.rs | sed -n '601,765p'`
- `nl -ba crates/ploke-eval/src/successor_selection/traversal.rs | sed -n '1827,1975p'`
- `nl -ba crates/ploke-egui/src/benchmark.rs | sed -n '77,180p'`
- `nl -ba crates/ploke-egui/src/benchmark.rs | sed -n '1020,1190p'`
- `nl -ba crates/ploke-egui/src/perf.rs | sed -n '90,185p'`
- `find crates/ploke-egui/src/ui/charts -maxdepth 2 -type f -printf '%p\n' | sort`
- `nl -ba crates/ploke-egui/src/ui/charts/mod.rs | sed -n '1,120p'`
- `nl -ba crates/ploke-egui/src/ui/charts/bar.rs | sed -n '1,160p'`
- `nl -ba crates/ploke-egui/src/ui/app/mod.rs | sed -n '1,55p'`
- `find docs/active/agents/ploke-egui-candidate-comparison-reviews -maxdepth 1 -type f -printf '%f\n' 2>/dev/null | sort`
- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120` passed: 15 passed, 0 failed.
- `git diff --check -- docs/active/agents/ploke-egui-candidate-comparison-reviews/2026-05-19-popout-chart-review.md` passed.

## Changed Files

- `docs/active/agents/ploke-egui-candidate-comparison-reviews/2026-05-19-popout-chart-review.md`

## Blockers

None for the review. The implementation blocker is design choice: choose painter-first charts or approve a new plot dependency before coding the popout.
