# Default View Contract

This document defines the intended default `ploke-egui` view as a testable UI
contract. It is a read-only inspection contract over `ploke_tree::Graph`, not a
new source of graph meaning and not a participant in successor selection.

`ploke-eval` runs, logs, typed records, reports, and report-like projections are
the upstream material. They may be loaded or folded by other crates before
`ploke-egui` sees them, but the UI contract begins at `&ploke_tree::Graph`.

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
  order, source/projection confidence, runtime diagnosis, benchmark direction,
  and interaction model.
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md`
  Precise answer contracts for lineage, candidate frontier, successor
  selection, patch diff, tool calls, provider attempts, timeline concurrency,
  locus analysis, composability, and mergeability.
- `docs/active/agents/ploke-ui-task-readability/plan.md`
  Current readability-wave objective: answer which artifact lineage is selected
  or promoted, what nearby alternatives exist, and where to drill next.
- `docs/active/agents/2026-05-09_egui-wasm-observability-plan.md`
  Older egui/wasm observability plan from the `ploke-tree-egui` /
  browser-model iteration. It is useful for question inventory and presentation
  density, but not as the current source model.
- `docs/workflow/evalnomicon/drafts/observability/run-tree-browser-design.md`
  Earlier run-tree browser layout sketch: left run/filter pane, central run
  tree, right detail pane, and bottom timeline lane. Treat it as prior art for
  frames and drilldown questions, not as an instruction to copy browser-owned
  snapshots into `ploke-egui`.

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
+-----------------------------------------------------------------------+
| top strip: run + mode                                                 |
+----------------+----------------------------------+-------------------+
| left controls  |                                  | right inspector   |
| run picker     |          graph canvas            | selected summary  |
| mode picker    |                                  | identity/details  |
| counts         |                                  | drill next        |
| diagnostics    |                                  |                   |
+----------------+----------------------------------+-------------------+
| bottom timeline shell: order/status/selection sync                    |
+-----------------------------------------------------------------------+
```

Default width budget:

- The initial native window is `1280px` wide.
- The left sidebar default width is `200px`.
- The left sidebar maximum width is `240px`.
- The right inspector default width is `300px`.
- The right inspector maximum width is `360px`.
- The center graph canvas default width is `780px`.
- Therefore the graph canvas starts at about `61%` of the initial window width.
- Contract: with both side panels visible, the graph canvas remains the largest
  region and keeps at least `50%` of the initial window width even when both
  side panels are at their maximum width.

## Question To Frame Map

The frame layout is not only visual. Each region owns a class of operator
questions from the source docs, and missing answers should appear as explicit
status rows instead of empty space or inferred facts.

| Question family | Primary frame | First visible answer | Source facts | Current status |
|---|---|---|---|---|
| Run load/progress | Top strip, left sidebar | loaded/empty/failed, generation/node counts | `&Graph`, run picker state, `RunForest` when present | Partial: counts exist in the left panel and top strip; richer generation summary is missing. |
| Artifact lineage / selected path | Center canvas, right inspector | visible tree, selected node identity, parent/child relation | `Graph.forest`, `Graph.artifact_tree()` fallback, History marks | Partial: canvas and inspector shell exist; structured lineage rows are incomplete. |
| Nearby alternatives | Center canvas, right inspector | sibling nodes, candidate/branch refs, hidden/visible status | scheduler nodes, candidate/branch graph indices | Partial: visible siblings exist for run-forest topology; candidate drilldown is missing. |
| Successor selection | Right inspector | selected candidate/member, considered count, candidate-set root | `Graph.selections`, `Graph.candidates` | Partial/blocked: graph carries selection facts; no inspector answer path yet. |
| Source/projection status | Right inspector | typed source refs, graph-build warnings, missing/not-applicable/failed | graph refs, warnings, History/report-derived records | Missing: diagnostic shape exists; inspector rows do not. |
| Patch/surface detail | Right inspector, later drilldown | target path, patch id, base/derived artifact, source state | scheduler node facts, branch/candidate surface evidence | Partial: run-forest detail text has fields; structured inspector rows are missing. |
| Evaluation/tool/provider diagnosis | Right inspector, bottom timeline | evaluation status, tool-call/provider availability rows | evaluation records, agent-turn/tool/provider projections | Blocked/partial by row: some records load as graph sources, but graph-owned answer objects are incomplete. |
| Causal order and concurrency | Bottom timeline | compact sealed order, runtime/selection/evaluation spans | History, scheduler/runtime/evaluation/tool span refs | Missing: timeline frame and span projection are not implemented. |
| Drill next | Right inspector | available/blocked/missing drilldown list | typed drilldown contract rows | Missing: no drilldown availability table in the app yet. |

The older `ploke-tree-egui` / browser view is precedent for presentation shape:
scrollable detail rows, grouped sections, evaluation / surface / protocol
summaries, and compact step ordering. `ploke-egui` should reuse those row
priorities where they still answer the same question, but the source is
`&ploke_tree::Graph` plus a selected graph reference, not copied
`PlaybackBrowserModel` snapshots.

## Region Contracts

### Top Strip

Purpose:

- Keep run-level controls visible without competing with graph inspection.
- Show whether the loaded run is usable before the operator reads the graph.

Should contain:

- run selector;
- graph mode selector;
- load/schema status;
- quick filters for source/projection status, transition kind, and failure
  class.

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
- Summarize run contents without becoming a semantic source.

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
- `layout.width_budget.right_inspector_width_logical_px > 0`
- `layout.width_budget.right_inspector_max_width_logical_px <= 360`
- `layout.width_budget.center_canvas_width_percent >= 50`
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
- Show the run-forest parent/child topology when available, with artifact,
  patch, branch, candidate, and History facts available as detail, filter,
  mark, or drilldown material.

Should contain:

- `F` run-forest nodes as the primary visible nodes when `Graph.forest` is
  present and non-empty;
- `E_F` run-forest parent edges as primary visible edges when `Graph.forest`
  is present and non-empty;
- fallback `A` Artifact nodes and `P_H union P_B` patch/derivation edges only
  when run-forest records are absent;
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
- `graph_identity.forest_nodes` reports the loaded `Graph.forest` node count.
- `graph_identity.default_visible_nodes` reports the rendered default
  projection node count.
- `graph_identity.artifact_tree_nodes`, `graph_identity.artifact_tree_P_H`,
  and `graph_identity.artifact_tree_P_B` report the borrowed artifact-tree
  relation counts even when the default canvas is using `F`.
- `graph_identity.visible_node_fingerprint` is stable for the visible default
  node key set, so CLI and app-written diagnostics can be compared directly.
- `canvas.primary_node_set = F` when `F` is non-empty.
- `canvas.primary_edge_set = E_F` when `F` is non-empty.
- `canvas.primary_node_set = A` only for fallback graphs without `F`.
- `canvas.primary_edge_set = P_H union P_B` only for fallback graphs without
  `F`.
- `canvas.synthetic_anchors_visible = false`
- `canvas.debug_nodes_visible = false`
- `canvas.run_forest_nodes.count > 0` for non-empty real Prototype 1 runs
  loaded through `RunRecordSet`.
- `canvas.artifact_nodes.count > 0` for fallback graphs without run-forest
  records.
- `canvas.patch_edges.count > 0` when fallback patch/derivation relations
  exist.
- `canvas.ruler_highlight.count <= 1` until multi-ruler semantics are defined.

Status:

- Implemented: default `GraphViewMode` is `ArtifactTree`.
- Implemented: central graph canvas exists.
- Implemented: ArtifactTree hides synthetic anchors.
- Implemented: diagnostics report whether synthetic anchors are visible.
- Implemented: diagnostics report `F`, `A`, `E_F`, `P_H`, `P_B`, total visible
  nodes, total visible edges, weak components, roots, orphan artifacts, and
  ruler highlight count.
- Implemented: contract checks include artifact node-set reporting, artifact
  edge-set reporting, ruler-highlight reporting, and component reporting.
- Partial: run-forest topology is graph-owned through `Graph.forest`; fallback
  artifact relation diagnostics are still computed from the current graph
  projection path.
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
- typed source refs and record ids;
- missing/not-applicable/failed diagnostics for unavailable drilldowns.

First-frame content contract:

| Section | Question answered | First slice content | Source | Status |
|---|---|---|---|---|
| Summary | What exactly did I select? | display label and selected graph kind | selected widget payload plus borrowed `Graph` lookup | Implemented for selected nodes. |
| Identity | Which generation, parent, child, artifact, and candidate does this belong to? | run-forest node id, parent node, child nodes, candidate id, source/base/derived artifact ids, patch id when present | `Graph.forest` / `TreeNode` refs; fallback `Graph.artifact_tree()` refs | Implemented for run-forest nodes and artifact fallback nodes. |
| Surface | What changed, and where? | target path, patch id, source state id, base artifact, derived artifact | scheduler node fields and candidate/branch surface evidence | Partial: core fields exist on run-forest nodes; patch diff body is deferred. |
| Lineage | What path led here? | immediate parent/child relation, ancestry availability status | `E_F` parent edges, fallback `P_H`/`P_B` edges | Partial: immediate graph topology edges exist; full ancestry path is deferred. |
| Artifact relations | Which artifact-id edges are known for this selection? | separate incoming/outgoing artifact edge rows, distinct from run-forest `E_F` rows | `Graph.artifact_tree()` and run-forest base/derived artifact refs | Partial: fallback artifact selections show `P_H`/`P_B`; run-forest selections show `P_B` only when base and derived artifact ids are both present. |
| Selection | Why was this successor selected over nearby candidates? | selected candidate/member refs, candidate-set root, considered count, decision outcome | `Graph.selections`, `Graph.candidates` | Partial/blocked: graph carries many facts, but no borrowed inspector answer path exists. |
| Candidate set | From among which candidates? | candidate set root, membership ids, selected membership, unavailable reason if source/decision roles are ambiguous | `CandidateMembershipKey`, `SelectionNode` | Blocked until a graph-owned answer object preserves source-set vs decision-set roles. |
| Source refs | Which typed records or report-derived facts support this displayed answer? | source record refs, graph warnings, and source counts | `TreeNode` refs, artifact source refs, graph warnings | Partial: record-ref rows exist where the graph projection exposes them; source/projection classification is deferred. |
| Drill next | What can I inspect next? | availability rows for lineage, candidate set, patch diff, eval actions, tool calls, provider attempts | typed drilldown contract row status | Partial: static rows exist; typed availability is deferred. |

Questions supported:

- What exactly did I select?
- Why was this successor selected over nearby candidates?
- Which typed record or report-derived fact supports this displayed answer?
- Which drilldown is still missing, malformed, or not applicable?

Testable signals:

- `layout.right_inspector.present = true`
- `selection.detail.present = true when selected`
- `--inspect-node A1` prints the same graph-resolved inspector projection
  without opening the GUI.
- `selection.record_refs.present = true when available`
- `selection.drilldown_candidates.present = true`
- `selection.unavailable_reason in {missing, not_applicable, failed}`

Status:

- Implemented: separate right inspector frame exists.
- Implemented: selected widget payload carries a `GraphSelectionRef`.
- Implemented: selected-node inspection resolves to a typed borrowed
  `SelectionInspector<'_>` over `Graph` facts before any render strings are
  produced.
- Implemented: egui/text/JSON use a downstream `SelectionInspectorSnapshot`;
  this snapshot is not a semantic carrier.
- Implemented: run-forest selections expose explicit parent/child node rows and
  keep graph topology edges separate from artifact-id edge rows.
- Partial: typed record refs are present for run-forest source refs and artifact
  source counts; full source/projection locator rows are deferred.
- Partial: drilldown-candidate shell rows exist; typed availability is deferred.
- Partial: unavailable-reason classification is visible for first shell rows.

Implementation rule:

```text
selected widget payload
  -> GraphSelectionRef
  -> borrowed typed Graph inspector answer
  -> render-only snapshot / rows
```

The inspector may allocate labels for display, but it must not store copied
semantic ids, records, or partial records as UI state. If a row cannot be
resolved by a typed graph relation or graph-loaded source ref, show `missing`,
`blocked`, or `not_applicable` with the relevant answer-contract id.

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
- explicit source/projection status for timestamp-only ordering.

First-frame content contract:

| Section | Question answered | First slice content | Source | Status |
|---|---|---|---|---|
| Sealed order | What is the admitted History order? | compact block/entry order and selected step marker | `Graph.history` | Missing frame; graph has History facts. |
| Run forest order | Which parent produced which children? | generation bands and selected node highlight | `Graph.forest` | Missing frame; canvas uses these facts. |
| Runtime/selection joins | Where do runtime-local events join admitted History? | join availability rows for selection, handoff, evaluation | History, scheduler, runtime, selection indices | Missing/partial depending row. |
| Evaluation/tool spans | Where did the child spend time, and what failed? | evaluation/tool/provider span availability status | evaluation, agent-turn, provider attempt projections | Blocked/partial: typed facts exist in places, but no graph-owned span projection is wired to egui. |
| Source/projection status | Which order is causal vs timestamp/projection? | span badge: sealed, causal, timestamp, or projection | typed span refs and source refs | Missing. |

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

- Implemented: bottom timeline frame exists.
- Missing: timeline span projection in `ploke-egui`.
- Partial: selection status is reflected in the timeline shell; visual span
  highlighting is missing.

Implementation rule:

The bottom lane is not a log dump. Live or completed runs may produce logs,
reports, and typed records upstream, but `ploke-egui` should see them through
`ploke_tree::Graph` or a graph-owned span projection. Timestamps may help draw
spans, but they must not be the only source of causality or nesting.

## Frame-First Implementation Slices

The UI shell should be built before every row is fully backed, so future
drilldowns land in stable frames instead of accumulating in whichever panel
currently exists.

| Slice | Scope | Done when | Explicit non-goals |
|---|---|---|---|
| `shell.frames` | Top strip, left sidebar, center canvas, right inspector, bottom timeline frames exist with stable sizes. | Diagnostics report each frame's presence and the graph remains the visual center. | No semantic drilldown joins beyond existing selected detail. |
| `inspector.selection-shell` | Selected detail moves from left sidebar to right inspector. | Selecting a graph node shows Summary, Identity, and Drill Next sections. | No candidate comparison, tool-call viewer, or patch diff body. |
| `inspector.run-forest-node` | Run-forest node rows are resolved from `&Graph.forest`. | Node id, generation, parent node, branch, candidate, base/patch/derived artifacts render as structured rows. | No copied browser step model. |
| `inspector.selection-replay-status` | Selection/candidate rows show available/blocked/missing status. | `ui.successor.selection` and `ui.selection.candidate.set` rows say what can and cannot be proven from graph-owned facts. | No source-set/decision-set flattening. |
| `timeline.shell` | Bottom lane exists and can show empty/not-applicable/blocked status. | Contract report exposes timeline frame presence and span counts. | No timestamp-only causal reconstruction. |
| `timeline.first-spans` | Compact spans for History/run-forest order. | Selecting a graph node can highlight a corresponding timeline row/span. | No provider/tool span detail until typed span refs exist. |

The earlier `ploke-tree-egui` work is the reference for presentation density
and row grouping. Its browser snapshots are not the source model for these
slices. When a browser-field idea is useful, port the row shape and field
priority, then bind it to `&Graph` or a graph-owned answer object.

## Default View Invariants

These are the minimum invariants a CLI or snapshot test should be able to check
without a human opening the UI:

```text
DefaultView :=
  Layout(top?, left, center, right?, bottom?)
  + Center.mode = ArtifactTree
  + if F non-empty:
      Center.nodes = F
      Center.edges = E_F
    else:
      Center.nodes = A
      Center.edges = P_H union P_B
  + Center.nodes disjoint D
  + Center.nodes disjoint SYN
  + History is highlight/reveal state, not the default canvas spine
```

Required pass/fail checks:

- The default mode is `ArtifactTree`.
- The default center canvas is the largest horizontal region and receives at
  least `50%` of the initial native window width with both side panels visible.
- A non-empty real Prototype 1 run renders run-forest nodes in the center
  canvas; fallback artifact-only graphs render artifact nodes.
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
- Implemented: node/edge counts are reported by semantic set for `F`, `A`,
  `E_F`, `P_H`, and `P_B`.
- Implemented: selected detail is represented in the default-view diagnostic
  shape when selection exists.
- Implemented: layout-region presence is reported for the top, left, center,
  right, and bottom frames.
- Implemented: pass/fail contract report exists for the current diagnostic
  shape.
- Partial: the run-forest path reports graph-owned topology; fallback
  artifact-only membership and relation diagnostics still depend on the current
  projection path.
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
