# Artifact Tree Default

This directory is the restart point for follow-up work on the default
`ploke-egui` tree graph view. It records the project vocabulary and product
target so workers do not reinterpret the graph as a generic record graph,
history timeline, or debug projection.

## Source Of Truth

The default tree graph view is an artifact-first tree/DAG projection over
`ploke_tree::graph::Graph`.

Primary default geometry when a typed run forest is available:

- nodes are Prototype 1 run-forest nodes: one visible node per scheduler
  node / materialized child candidate state;
- edges are scheduler `parent_node_id -> node_id` relations;
- lower-granularity artifact ids such as bounded text-file before/after ids
  appear in node detail, filters, or drilldown, not as the primary canvas
  topology;
- the initial parent appears uppermost and children appear below;
- edge labels are short patch handles such as `P1`, `P2`, `P3`;
- primary node labels are short stable display handles such as `A1`, `A2`,
  `A3`, never raw full ids.

History remains crucial, but it is not the canvas spine in the default view.
Use History for reveal order, dimming/stepping state, and highlighting the
current `Parent<Ruler>`. Do not collapse the default canvas into a sealed
History-block chain.

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

Current correction: `ploke_tree::Graph` now carries the existing
`RunForest` projection when built from a run record set. The default
`ArtifactTree` renderer uses that run forest as the primary topology when it is
present, and falls back to the lower-granularity artifact-id relation fold only
for graphs without run-forest records.

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

- Move the artifact-tree relation fold into `ploke-tree` as a borrowed
  projection consumed by egui.
- Keep full composed/all-record graph rendering behind an explicit debug mode.
- Add tests that default geometry contains Artifact nodes and patch/derivation
  edges, and does not contain HistoryBlock, tool, evidence, provider, or
  unattached synthetic nodes.
- Keep full raw ids available only in hover/details/debug text.
- Verify with the snapshot command against a real run root after code changes.
