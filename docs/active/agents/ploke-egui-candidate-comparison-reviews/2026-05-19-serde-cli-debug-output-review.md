# Candidate Comparison Serde CLI Debug Output Review

Reviewer: `cc-serde-cli-reviewer`
Date: 2026-05-19
Scope: design review only; no source changes.

## Verdict

The right serialization boundary is a graph-backed inspector/debug snapshot, not
History JSON parsing in `ploke-egui` and not eframe persistence state.

`ploke-egui` already has the serde machinery needed to print Candidate
Comparison field values from the CLI:

- `eframe = { version = "0.34", features = ["persistence"] }` and workspace
  `serde` / `serde_json` are enabled in `crates/ploke-egui/Cargo.toml:11-21`.
- `cargo tree -p ploke-egui -e features` shows `eframe/persistence` enabling
  `eframe/serde`, `eframe/ron`, `egui/persistence`, and `egui/serde`.
- The app already uses eframe storage helpers for UI layout state:
  `OperatorApp::load` calls `eframe::get_value` and `App::save` calls
  `eframe::set_value` (`crates/ploke-egui/src/ui/app/mod.rs:201-204`,
  `crates/ploke-egui/src/ui/app/mod.rs:269-272`).
- The current dashboard state is serializable: `Pane` derives
  `Serialize`/`Deserialize` and can carry `PinnedInspector` and
  `InspectorSection` (`crates/ploke-egui/src/ui/dashboard/tiles.rs:8-19`),
  while `InspectorPanelSection::CandidateComparison` also derives
  `Serialize`/`Deserialize` (`crates/ploke-egui/src/ui/app/shell.rs:462-477`).

Use those eframe features for restoring UI pane/section state. For CLI field
verification, use the existing project serde path over typed records and typed
graph witnesses.

## Current Evidence Path

The source record already owns the selector data. `SelectionDecisionEntryRecord`
contains typed traversal, typed metrics, and optional typed formula data
(`crates/ploke-records/src/history/payload.rs:676-712`). The formula schema is
owned by `ploke-records::selection`: `SelectionFormulaRecord`,
`ScoreChildPropRecord`, and `ScoreChildPropRowRecord` all derive serde
(`crates/ploke-records/src/selection.rs:141-235`).

`ploke-tree` ingests that record into graph facts. `MetricIndex` carries formula
nodes keyed by `(selection_entry_id, metric_set_id)`
(`crates/ploke-tree/src/graph/types/selection.rs:115-123`,
`crates/ploke-tree/src/graph/types/selection.rs:183-209`). The ingestion path
validates that the formula `metric_set_id` matches the decision metrics and then
inserts `SelectionFormulaNode` under the selection-entry-scoped key
(`crates/ploke-tree/src/graph/build/selection.rs:343-363`).

`ploke-egui` imports only through typed graph loading:
`graph_from_run_root` builds `FsRunStore`, loads a typed record set, then calls
`Graph::from_records` (`crates/ploke-egui/src/import/mod.rs:13-24`). That is the
path the CLI debug output should use.

The Candidate Comparison witness is currently graph-backed but not part of the
serializable inspector snapshot. `CandidateComparisonSlot` resolves the metric
set and formula from `&Graph`, and `resolve_child` binds each child to
`CandidateNode`, `MetricCandidateNode`, and `ScoreChildPropRowRecord`
(`crates/ploke-egui/src/ui/inspector.rs:477-592`). The right-panel renderer
prints the formula summary and row fields directly from those witnesses
(`crates/ploke-egui/src/ui/app/shell.rs:1613-1815`,
`crates/ploke-egui/src/ui/app/shell.rs:1819-1885`).

The existing serializable `SelectionInspectorSnapshot` omits Candidate
Comparison. It derives `Serialize` and has a text renderer
(`crates/ploke-egui/src/ui/inspector.rs:1180-1265`), while snapshot construction
for run-forest and artifact selections lives at
`crates/ploke-egui/src/ui/inspector.rs:2195-2372`. The current serde test only
proves the existing snapshot fields serialize
(`crates/ploke-egui/src/ui/inspector.rs:4218-4230`).

## Recommended Boundary

Add one borrowed, serializable Candidate Comparison export projection in
`crates/ploke-egui/src/ui/inspector.rs`, next to `SelectionInspectorSnapshot`
and the existing witness types.

Recommended shape:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CandidateComparisonSnapshot<'a> {
    pub parent_node_id: &'a str,
    pub metric_set_id: Option<&'a str>,
    pub selection_entry_id: Option<&'a str>,
    pub formula: Option<CandidateComparisonFormulaSnapshot<'a>>,
    pub candidates: Vec<CandidateComparisonCandidateSnapshot<'a>>,
}
```

The exact type names can change, but the ownership rule should not:

- Source facts: `ploke-records::selection::{MetricSet, SelectionFormulaRecord,
  ScoreChildPropRecord, ScoreChildPropRowRecord}` and the graph nodes in
  `ploke-tree`.
- Semantic object: `CandidateComparisonSlot` plus resolved
  `CandidateComparisonCandidate<'_>` from `&ploke_tree::Graph`.
- Projection: a borrowed serializable snapshot produced from the same witnesses
  the renderer consumes.
- Renderer: CLI text/JSON in `crates/ploke-egui/src/cli/report.rs`, using
  `serde_json::to_writer_pretty` or `to_string_pretty` over the projection.

Do not put this in `eframe::Storage`. eframe persistence is app-state storage;
the debug output is a one-shot projection of typed graph facts. Persisting it
through eframe would blur UI layout state with source-data verification.

Do not make CLI code reconstruct formula semantics. `cli::report` should load
the graph, resolve the same selection as `--inspect-node`, ask the inspector for
the Candidate Comparison snapshot, and print it. It should not parse
`SelectionDecisionEntryRecord` files, walk History JSON, or recompute
`score_child_prop`.

## CLI Shape

The least-disruptive command shape is an extension of the existing inspector
surface:

```text
ploke-egui --features dev -- --run-root <prototype1-root> \
  --inspect-node <NODE> \
  --inspect-section candidate-comparison \
  --inspect-format json
```

Why this shape:

- `--inspect-node` already resolves visible labels or graph keys through
  `SelectionInspector::from_default_selector` (`crates/ploke-egui/src/cli/report.rs:11-15`,
  `crates/ploke-egui/src/ui/inspector.rs:766-780`).
- The Candidate Comparison section already has a stable enum value and title
  (`crates/ploke-egui/src/ui/app/shell.rs:462-495`).
- `cli::report` is already the thin renderer module for dev-only inspection
  surfaces (`crates/ploke-egui/src/cli/report.rs:1-24`).

An acceptable narrower first slice is:

```text
ploke-egui --features dev -- --run-root <prototype1-root> \
  --candidate-comparison <NODE> --candidate-comparison-format json
```

The general `--inspect-section/--inspect-format` version is preferable because
it avoids adding one command family per inspector section.

## Output Fields

The JSON should expose stable field names matching the UI-visible fields, but it
should preserve source grouping:

- `parent_node_id`
- `metric_set`: `metric_set_id`, `score_profile`, `imp_at_k.budget_k`,
  `imp_at_k.score_points_per_imp_point`, `imp_at_k.require_for_score`
- `formula`: `selection_entry_id`, `metric_set_id`, `kind`,
  `seed`, `top_m`, `lambda_millis`, `lambda`, `metric_inputs`, `oracle_mode`,
  `oracle_require_evidence`, `oracle_used_for_alpha`, `alpha_mid`,
  `total_weight`, `sample`, `sample_threshold`, `uniform_fallback_slot`,
  `selected_index`, `selected_candidate`
- `candidates[]`: child/candidate identity plus the exact
  `ScoreChildPropRowRecord` fields used by the renderer:
  `payload_index`, `candidate`, `node_id`, `branch_id`,
  `branch_disposition`, `base_outcome`, `outcome_points`,
  `operational_points`, `protocol_points`, `imp_at_k_delta`,
  `imp_at_k_score_excluded`, `performance`, `oracle_resolved`,
  `oracle_configured`, `oracle_rate`, `oracle_used_for_alpha`, `child_count`,
  `alpha`, `alpha_mid`, `exploitation`, `exploration`, `weight`,
  `cumulative_lower`, `cumulative_upper`, `sample_hit`, `selectable`,
  `exclusion_reason`, `performance_present`, `selection_input_present`,
  `decision_present`, `selected`
- metric drilldown attached to each candidate from `MetricCandidateNode`:
  `imp_at_k`, compared-run operational deltas, and compared-run protocol deltas.

The formula and row fields can be serialized directly from
`ScoreChildPropRecord` / `ScoreChildPropRowRecord` references. That gives CLI
verification the same serde names as the canonical record schema, while wrapper
fields provide the graph/UI binding context such as `child_node_id` and
`selected_by_selection_node`.

## Comparing CLI Output Against UI Witnesses

The comparison should be mechanical:

1. Resolve the selected node through the same path as the UI:
   `SelectionInspector::from_default_selector`.
2. Build `InspectorSections` through `InspectorCache::sections`.
3. Resolve `CandidateComparisonSlot` and each child through
   `CandidateComparisonSlot::resolve_child`.
4. Build the JSON projection from those resolved witnesses.
5. In tests, assert the JSON fields against the witness fields before any text
   rendering:
   - `formula.seed == score.record.seed`
   - `candidates[i].row.payload_index == row.payload_index`
   - `candidates[i].row.weight == row.weight`
   - `candidates[i].row.sample_hit == row.sample_hit`
   - `candidates[i].metric.imp_at_k.improvement == metric.imp_at_k.improvement`

The existing candidate test already proves the witness can expose formula rows:
`selected_child_artifact_exposes_candidate_comparison_from_parent_plan` asserts
`payload_index`, `performance`, `weight`, `sample_hit`, and metric drilldown
values (`crates/ploke-egui/src/ui/inspector.rs:4063-4216`). Extend that fixture
to serialize the new Candidate Comparison snapshot and assert the printed JSON
fields.

Do not compare against the egui widget text as the primary oracle. Widget text
is a renderer projection and can format, abbreviate, or cache values. The CLI
JSON should compare against the typed witness that both the renderer and CLI
export use.

## Test Strategy

Recommended focused tests:

- `ploke-egui`: extend the existing fixture test to assert the new
  Candidate Comparison snapshot serializes all formula and row fields.
- `ploke-egui`: add an args parse test for
  `--inspect-section candidate-comparison --inspect-format json`, following the
  existing CLI parse tests in `crates/ploke-egui/src/cli/mod.rs:219-260`.
- `ploke-egui`: add a pure report-render test that calls the report function
  with an in-memory graph and asserts JSON text contains the selected candidate,
  `metric_set_id`, `payload_index`, `weight`, and `sample_hit`.
- Existing `ploke-records` roundtrip coverage for `SelectionFormulaRecord`
  should remain the canonical schema roundtrip test; egui should not duplicate
  record ownership.

Test fixture JSON literals are acceptable for assertions, but production CLI
code should not inspect `serde_json::Value` or field-walk persisted records.

## Commands Run

```text
sed -n '1,220p' .orchestrator/workers/cc-serde-cli-reviewer.md
sed -n '1,220p' .codex/skills/ploke-egui-benchmarking/SKILL.md
sed -n '1,220p' .codex/skills/cli-surface-discipline/SKILL.md
sed -n '1,220p' .codex/skills/ploke-debugger-claim-workflow/SKILL.md
sed -n '1,220p' .codex/skills/ui-claim-archaeology/SKILL.md
rg -n "ploke-egui|Candidate Comparison|score_child_prop|serde|snapshot|export" /home/brasides/.codex/memories/MEMORY.md
rg -n "egui|eframe|serde|native-benchmark|contract-report|inspect-node|snapshot|export|debug|benchmark|bench" Cargo.toml crates/ploke-egui/Cargo.toml
rg -n "CandidateComparison|candidate_comparison|ScoreChildProp|SelectionFormula|ScoreChildPropFormula|formula|selection_entry_id|metric_set_id" crates/ploke-egui/src crates/ploke-tree/src crates/ploke-records/src crates/ploke-eval/src/successor_selection crates/ploke-eval/src/cli/prototype1_state docs/active/archaeology/ploke-tree-graph/score-child-prop-ui-spec.md
cargo tree -p ploke-egui -e features | rg "eframe|egui |egui_tiles|serde" | head -n 120
cargo test -p ploke-egui selection_snapshot_serializes_typed_schema 2>&1 | tail -n 80
cargo test -p ploke-egui selected_child_artifact_exposes_candidate_comparison_from_parent_plan 2>&1 | tail -n 80
cargo test -p ploke-egui benchmark 2>&1 | tail -n 120
git diff --no-index --check /dev/null docs/active/agents/ploke-egui-candidate-comparison-reviews/2026-05-19-serde-cli-debug-output-review.md
```

Outcomes:

- `cargo tree` confirmed serde/persistence feature availability through
  `eframe`.
- `selection_snapshot_serializes_typed_schema` passed.
- `selected_child_artifact_exposes_candidate_comparison_from_parent_plan`
  passed.
- `cargo test -p ploke-egui benchmark` passed: 15 passed, 0 failed.
- `git diff --no-index --check` printed no whitespace diagnostics. The command
  exits non-zero for a no-index diff even when whitespace is clean.

## Blockers / Caveats

- Current `SelectionInspectorSnapshot` has no `candidate_comparison` field, so
  current `--inspect-node` output cannot verify Candidate Comparison values.
- Current diagnostics snapshots can serialize selected inspector data, but they
  will not include Candidate Comparison until the inspector snapshot is extended
  (`crates/ploke-egui/src/diagnostics/mod.rs:48-90`,
  `crates/ploke-egui/src/ui/app/mod.rs:504-528`).
- Native interactive window behavior was not tested in this review.

## Changed Files

- `docs/active/agents/ploke-egui-candidate-comparison-reviews/2026-05-19-serde-cli-debug-output-review.md`
