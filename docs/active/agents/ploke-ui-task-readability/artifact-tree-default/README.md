# Artifact Tree Default

This directory is the restart point for follow-up work on the default
`ploke-egui` tree graph view. It records the project vocabulary and product
target so workers do not reinterpret the graph as a generic record graph,
history timeline, or debug projection.

## Source Of Truth

The default tree graph view is an artifact-first tree/DAG projection over
`ploke_tree::graph::Graph`.

Primary default geometry:

- nodes are artifact identities from `ploke_tree::graph::artifact_tree::Tree<'_>`;
- edges are default visible artifact relations `P_H ∪ P_C`;
- sealed History opened-from context `P_O` and applied-patch provenance
  `P_B` stay available as relation inventory, inspector context, and
  diagnostics, but not as default visible edge geometry;
- scheduler/process facts such as node ids, generation, candidate ids, and
  parent/child runtime topology appear only as attached provenance, drilldown,
  or later step-through overlays;
- the initial parent artifact appears uppermost and derived/successor artifacts
  appear below;
- edge labels are short patch handles such as `P1`, `P2`, `P3`;
- primary node labels are short stable display handles such as `A1`, `A2`,
  `A3`, never raw full ids.

History remains crucial, but it is not the canvas spine in the default view.
Use History for reveal order, dimming/stepping state, and highlighting the
current `Parent<Ruler>`. Do not collapse the default canvas into a sealed
History-block chain.

Promotion continuity is also part of the default artifact identity fold:

- if a selected child Artifact later becomes the next Parent checkout, the
  selected child `ArtifactId` and the later next-parent base `ArtifactId`
  should display as one artifact node in this view;
- raw ids remain visible in the inspector and CLI diagnostics;
- this is a graph-owned quotient over child-plan continuity facts, not a
  renderer-local alias.

## Separation From Drilldown And Debug

The default view must not render every loaded record class. Evidence nodes,
provider attempts, tool calls, runtime detail, agent turns, unattached records,
unresolved components, and full record inventories belong in typed drilldown,
side diagnostics, or explicit debug/all-record modes.

The full composed graph may remain available as a debug surface, but its layout
quality is not the product target. The product target is the artifact tree that
makes branching improvement legible at a glance.

## Hard Boundary

`ploke_tree::graph::Graph` owns semantic meaning. `ploke-egui` may own
presentation, layout, labels, interaction state, diagnostics, and render-only
payloads. It must not copy semantic records, ids, refs, partial records, or
wrapper carriers out of `Graph` for later semantic use.

Allocated derived values are allowed only when they carry a clearly derived
semantic result, such as counts, sums, averages, layout coordinates,
readability metrics, or display labels. Wrapping semantic data in a new local
type such as `Option<SomeReport>` is not derivation; it is a blocker unless the
data remains reference-based.

## Resolved Code Mismatch

At the time this note was written, the code default was a `LineageOverview`
mode routed through `project_lineage_overview`. That is a history-spined
overview, not the intended artifact-tree default. That mismatch has since been
resolved: the default view is now `ArtifactTree`.

The remaining boundary issue is ownership. The current egui projection computes
artifact membership and classified patch/derivation edges directly from
`ploke_tree::Graph`. The canonical relation fold should move into `ploke-tree`
as a borrowed projection before richer drilldown, styling, or all-record layout
work relies on it.

Current correction: the default renderer should use
`ploke_tree::Graph::artifact_tree()` directly. Any future process/scheduler
step-through surface must be a borrowed projection over `&Graph`, not an owned
`RunForest`-style topology embedded alongside the graph.

## Evidence Links

- [`crates/ploke-egui/README.md`](../../../../../crates/ploke-egui/README.md)
  describes the initial view as the artifact tree, with edges as patches and
  History as the stepping/highlight authority.
- [`../2026-05-12_ploke-egui-artifact-view-handoff.md`](../../2026-05-12_ploke-egui-artifact-view-handoff.md)
  states the current thread goal: start with artifact-first tree geometry and
  defer runtime/tool/agent-turn detail to drilldown surfaces.
- [`../../plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md`](../../../plans/self-improvement-loop/typed-persistence-spine/ui-drilldown-contract.md)
  defines later typed drilldown expectations; it is not license to overload the
  default canvas with all debug detail.

## Follow-Up Tasks

- Keep the artifact-tree relation fold in `ploke-tree` as the borrowed default
  projection consumed by egui.
- Move any process/scheduler stepping surface to a borrowed iterator/cursor or
  schedule projection over `&Graph`.
- Keep full composed/all-record graph rendering behind an explicit debug mode.
- Add tests that default geometry contains Artifact nodes and artifact
  produced-child/history-successor edges, and does not switch to
  run-forest/scheduler nodes when scheduler records are present.
- Keep full raw ids available only in hover/details/debug text.
- Verify with the snapshot command against a real run root after code changes.
