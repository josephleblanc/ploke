# Default View Contract

This document defines the intended default `ploke-egui` view as a testable UI
contract. It is a rendering contract over `ploke_tree::Graph`, not a new source
of graph meaning.

The default view should answer the first operator question:

```text
What artifact lineage is selected or promoted, what nearby alternatives exist,
and where should I drill next?
```

## Question Sources

This contract is downstream of the existing UI/browser question docs. When the
default view needs to justify a panel, check, or drilldown, start from these
files:

- `docs/active/plans/self-improvement-loop/frontend-questions.md`
  Broad observability questions: loop progress, benchmark trajectory, causal
  order, evidence/authority, runtime diagnosis, benchmark direction, and
  interaction model.
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md`
  Precise answer contracts for lineage, candidate frontier, successor
  selection, patch diff, tool calls, provider attempts, timeline concurrency,
  locus analysis, composability, and mergeability.
- `docs/active/agents/ploke-ui-task-readability/plan.md`
  Current readability-wave objective: answer which artifact lineage is selected
  or promoted, what nearby alternatives exist, and where to drill next.
- `docs/active/agents/2026-05-09_egui-wasm-observability-plan.md`
  Older egui/wasm observability plan with the operator-question table for live
  state, loop health, exit state, generation count, metrics, active ruler,
  improvement, patch rationale and impact, and failure/log location.
- `docs/workflow/evalnomicon/drafts/observability/run-tree-browser-design.md`
  Earlier run-tree browser layout sketch: left run/filter pane, central run
  tree, right detail pane, and bottom timeline lane.

## Status Legend

- Implemented: present in current `ploke-egui` behavior.
- Partial: present, but not in the intended location or not yet complete enough
  to satisfy the contract.
- Missing: not currently implemented.
- Blocked: the UI frame is known, but the needed graph-owned answer object,
  typed loader, or relation is not available yet.
- Not applicable: the frame or row is valid, but the currently loaded run or
  selected item has no source facts for it.

## Layout Regions

The default native layout should be:

```text
+---------------------------------------------------------------+
| Top strip: run, mode, load/schema status, quick filters        |
+----------------+------------------------------+---------------+
| Left sidebar   |                              | Right inspector|
| navigation     |        ArtifactTree canvas   | selected item  |
| filters        |                              | drill next     |
+----------------+------------------------------+---------------+
| Bottom lane: compact timeline / causal order                  |
+---------------------------------------------------------------+
```

Current implementation:

```text
+----------------+----------------------------------------------+
| Left panel     |                                              |
| run picker     |              graph canvas                    |
| mode picker    |                                              |
| counts         |                                              |
| selected node  |                                              |
| diagnostics    |                                              |
+----------------+----------------------------------------------+
```

Default width budget:

- The initial native window is `800px` wide.
- The left sidebar default width is `200px`.
- The left sidebar maximum width is `240px`.
- The center graph canvas default width is `600px`.
- Therefore the graph canvas starts at `75%` of the initial window width.
- Contract: the graph canvas keeps at least `70%` of the initial window width
  even when the left sidebar is at its maximum width.

## Question To Frame Map

The frame layout is not only visual. Each region owns a class of operator
questions from the source docs, and missing answers should appear as explicit
status rows instead of empty space or inferred facts.

| Question family | Primary frame | First visible answer | Source facts | Current status |
|---|---|---|---|---|
| Run load/progress | Top strip, left sidebar | loaded/empty/failed, generation/node counts | `&Graph`, run picker state, `RunForest` when present | Partial: counts exist in the left panel; top strip is missing. |
| Artifact lineage / selected path | Center canvas, right inspector | visible tree, selected node identity, parent/child relation | `Graph.forest`, `Graph.artifact_tree()` fallback, History marks | Partial: canvas exists; inspector is missing. |
| Nearby alternatives | Center canvas, right inspector | sibling nodes, candidate/branch refs, hidden/visible status | scheduler nodes, candidate/branch graph indices | Partial: visible siblings exist for run-forest topology; candidate drilldown is missing. |
| Successor selection | Right inspector | selected candidate/member, considered count, candidate-set root | `Graph.selections`, `Graph.candidates` | Partial/blocked: graph carries selection facts; no inspector answer path yet. |
| Evidence and authority | Right inspector | typed record refs, evidence strength, missing/not-applicable/failed | `Graph.evidence`, warnings, History records | Missing: diagnostic shape exists; inspector rows do not. |
| Patch/surface detail | Right inspector, later drilldown | target path, patch id, base/derived artifact, source state | scheduler node facts, branch/candidate surface evidence | Partial: run-forest detail text has fields; structured inspector rows are missing. |
| Evaluation/tool/provider diagnosis | Right inspector, bottom timeline | evaluation status, tool-call/provider availability rows | evaluation records, agent-turn/tool/provider projections | Blocked/partial by row: some records load as evidence, but graph-owned answer objects are incomplete. |
| Causal order and concurrency | Bottom timeline | compact sealed order, runtime/selection/evaluation spans | History, scheduler/runtime/evaluation/tool span refs | Missing: timeline frame and span projection are not implemented. |
| Drill next | Right inspector | available/blocked/missing drilldown list | typed drilldown contract rows | Missing: no drilldown availability table in the app yet. |

The `ploke-tree-egui` browser view is the precedent for the right-frame and
timeline presentation: scrollable detail rows, grouped sections, evaluation /
surface / protocol summaries, and compact step ordering. `ploke-egui` should
reuse that presentation shape, but the source is `&ploke_tree::Graph` plus a
selected graph reference, not copied `PlaybackBrowserModel` snapshots.

## Region Contracts

### Top Strip

Purpose:

- Keep run-level controls visible without competing with graph inspection.
- Show whether the loaded run is usable before the operator reads the graph.

Should contain:

- run selector;
- graph mode selector;
- load/schema status;
- quick filters for evidence strength, transition kind, and failure class.

Questions supported:

- Is the run loaded and compatible with the current schema?
- Which view mode am I inspecting?
- Am I filtering the default artifact view?

Testable signals:

- `layout.top_strip.present = true`
- `controls.run_selector.present = true`
- `controls.mode_selector.present = true`
- `status.load_state in {loaded, empty, failed}`
- `filters.quick.present = true`

Status:

- Partial: run selector and mode selector exist, but they are in the left panel.
- Missing: load/schema status as a first-class field.
- Missing: quick filters.

### Left Sidebar

Purpose:

- Provide navigation and coarse filtering while leaving the graph central.
- Summarize run contents without becoming a semantic authority.

Should contain:

- run selector if no top strip exists yet;
- artifact/candidate/lineage filters;
- run record counts derived from `&Graph`;
- high-level graph diagnostics.

Questions supported:

- What broad record classes are loaded?
- Are alternatives visible or hidden by the current view?
- Is the graph unreadable because of clutter, crossings, or hidden components?

Testable signals:

- `layout.left_sidebar.present = true`
- `layout.width_budget.left_sidebar_width_logical_px > 0`
- `layout.width_budget.left_sidebar_max_width_logical_px <= 240`
- `layout.width_budget.center_canvas_width_percent >= 70`
- `facts.history_blocks.count >= 0`
- `facts.artifacts.count >= 0`
- `facts.candidates.count >= 0`
- `diagnostics.readability.present = true`
- `filters.artifact_tree.present = true`

Status:

- Implemented: left panel exists.
- Implemented: run picker exists in native builds.
- Implemented: mode picker exists.
- Implemented: record counts are shown from `&Graph`.
- Implemented: graph diagnostics are shown when available.
- Missing: artifact/candidate/lineage filters.

### Center Canvas

Purpose:

- Make the `ArtifactTree` the primary default surface.
- Show artifacts and patch/derivation relations, with History acting as
  reveal/highlight state rather than as the canvas spine.

Should contain:

- Artifact nodes as the primary visible nodes;
- patch/derivation edges as the primary visible edges;
- current ruler or latest selected successor highlight;
- nearby alternatives visible as artifact branches;
- pan, zoom, drag, hover, and selection behavior.

Questions supported:

- Which artifact is currently selected or promoted?
- What artifact path led here?
- What sibling or nearby alternative artifacts exist?
- Is the default canvas showing product graph structure rather than debug
  record inventory?

Testable signals:

- `view.mode = "artifact-tree"` by default.
- `canvas.present = true`
- `canvas.primary_node_set = A`
- `canvas.primary_edge_set = P_H union P_B`
- `canvas.synthetic_anchors_visible = false`
- `canvas.debug_nodes_visible = false`
- `canvas.artifact_nodes.count > 0` for non-empty artifact runs.
- `canvas.patch_edges.count > 0` when patch/derivation relations exist.
- `canvas.ruler_highlight.count <= 1` until multi-ruler semantics are defined.

Status:

- Implemented: default `GraphViewMode` is `ArtifactTree`.
- Implemented: central graph canvas exists.
- Implemented: ArtifactTree hides synthetic anchors.
- Implemented: diagnostics report whether synthetic anchors are visible.
- Implemented: diagnostics report `A`, `P_H`, `P_B`, total visible nodes, total
  visible edges, weak components, roots, orphan artifacts, and ruler highlight
  count.
- Implemented: contract checks include artifact node-set reporting, artifact
  edge-set reporting, ruler-highlight reporting, and component reporting.
- Partial: these diagnostics are still computed from the current egui
  projection. The canonical relation fold should move into `ploke-tree`.
- Missing: CLI-readable assertion that debug-node exclusion is sourced from the
  graph-owned projection rather than from egui-local membership inference.

### Right Inspector

Purpose:

- Give selected-node or selected-edge answers without crowding the graph.
- Make "where should I drill next?" explicit.

Should contain:

- selected artifact or edge summary;
- lineage path summary;
- candidate set or successor-selection summary when available;
- evidence refs and typed record ids;
- missing/not-applicable/failed diagnostics for unavailable drilldowns.

Questions supported:

- What exactly did I select?
- Why was this successor selected over nearby candidates?
- Which typed record supports this answer?
- Which drilldown is still missing, malformed, or not applicable?

Testable signals:

- `layout.right_inspector.present = true`
- `selection.detail.present = true when selected`
- `selection.record_refs.present = true when available`
- `selection.drilldown_candidates.present = true`
- `selection.unavailable_reason in {missing, not_applicable, failed}`

Status:

- Partial: selected-node detail exists in the left panel.
- Missing: separate right inspector.
- Missing: typed record refs in the inspector.
- Missing: drilldown-candidate list.
- Missing: unavailable-reason classification.

### Bottom Timeline

Purpose:

- Keep causal order visible while the canvas stays artifact-first.
- Show History/runtime timing as a secondary lane, not as the default graph
  spine.

Should contain:

- compact History order;
- parent, child, successor, selection, evaluation, tool, and provider spans as
  typed projections when available;
- hover/selection sync with the center canvas;
- explicit evidence strength for timestamp-only ordering.

Questions supported:

- What is the sealed History order?
- Which phases happened before, after, or in parallel?
- Where do runtime-local events join admitted History?

Testable signals:

- `layout.bottom_timeline.present = true`
- `timeline.compact = true`
- `timeline.synced_selection = true`
- `timeline.spans.count >= 0`
- `timeline.order_strength in {sealed, causal, timestamp, projection}`

Status:

- Missing: bottom timeline.
- Missing: timeline span projection in `ploke-egui`.
- Missing: selection sync between timeline and graph.

## Default View Invariants

These are the minimum invariants a CLI or snapshot test should be able to check
without a human opening the UI:

```text
DefaultView :=
  Layout(top?, left, center, right?, bottom?)
  + Center.mode = ArtifactTree
  + Center.nodes = A
  + Center.edges = P_H union P_B
  + Center.nodes disjoint D
  + Center.nodes disjoint SYN
  + History is highlight/reveal state, not the default canvas spine
```

Required pass/fail checks:

- The default mode is `ArtifactTree`.
- The default center canvas receives at least `70%` of the initial native window
  width.
- A non-empty artifact run renders artifact nodes in the center canvas.
- The default canvas does not render synthetic anchor nodes.
- The default canvas does not render runtime/tool/provider/debug records as
  primary nodes.
- Record counts and graph diagnostics remain render-only projections from
  `&Graph`.
- The selected item, if any, can be described textually.
- Missing drilldowns are reported as missing/not-applicable/failed, not hidden
  behind empty panels.

Current status:

- Implemented: default mode check is possible from existing diagnostics.
- Implemented: synthetic anchor visibility is reported by existing diagnostics.
- Implemented: node/edge counts are reported by semantic set for `A`, `P_H`,
  and `P_B`.
- Implemented: selected detail is represented in the default-view diagnostic
  shape when selection exists.
- Partial: layout-region presence is reported for major regions, but tests
  still need specific real-run coverage.
- Implemented: pass/fail contract report exists for the current diagnostic
  shape.
- Partial: the report still reflects egui-local artifact membership until the
  graph-owned projection exists.
- Missing: unavailable drilldown classification.

## CLI Feedback Direction

The current `--features dev -- --contract-report` path prints a text
default-view contract report without opening a native window. This is the
portable feedback path for headless development environments.

The `--features dev -- --snapshot` path still writes graph diagnostics JSON and
text from the live egui app, but it requires a working native display.

The next diagnostic shape should be a textual or JSON contract report:

```text
DefaultViewContractReport {
  layout: LayoutReport,
  controls: ControlReport,
  center: ArtifactTreeReport,
  inspector: InspectorReport,
  timeline: TimelineReport,
  findings: Vec<Finding>,
}
```

The report should be produced from the same render path as the app. It may
record UI layout facts, visibility facts, readability metrics, and render-only
labels. It must not become a semantic mirror of `ploke_tree::Graph`.

Useful commands should eventually include:

```text
cargo run -p ploke-egui --features dev -- --run-root <prototype1-root> --contract-report
cargo run -p ploke-egui --features dev -- --run-root <prototype1-root> --snapshot
cargo test -p ploke-egui --features dev default_view_contract
```

The contract-report command exposes the projection contract without opening the
UI. The snapshot command exposes what the rendered app did when a native display
is available. The test command enforces stable contract expectations for
synthetic fixtures and selected real-run fixtures.
