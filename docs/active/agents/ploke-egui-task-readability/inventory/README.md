# Inventory

Inventory policy for task-readability and artifact-view graph coverage.

The active source-family tracker is still
[`../../2026-05-11_ploke-tree-graph-ingestion-inventory.md`](../../2026-05-11_ploke-tree-graph-ingestion-inventory.md).
It is useful but likely stale in places. Do not rely on it without checking
current code.

## Refresh Scope

For this thread, refresh only coverage that affects:

- artifact nodes,
- patch/derivation edges,
- candidate branches,
- selected/promoted lineage,
- History reveal/highlight order,
- near-term inspector joins for patch, candidate, selection, and evaluation
  evidence.

Do not broaden the inventory into provider, tool-call, database-context, or
timeline work unless a drilldown slice is explicitly opened.

## Classification

When a gap appears, classify it as one of:

- `graph-semantic-gap`
  A real execution/archive relation is missing from `ploke-tree::Graph`.
- `typed-loader-gap`
  A typed record exists or should exist, but `RunRecordSet` does not load it.
- `egui-projection-gap`
  `ploke-tree::Graph` has the fact, but the artifact view does not project it.
- `view-diagnostic-gap`
  The fact is geometry/readability only and belongs in `ploke-egui`.
- `deferred-drilldown`
  Not needed for first artifact-tree readability.

## Update Rule

When `Graph::from_records` starts consuming a new family for artifact-view work,
update the source-family tracker in the same change. The update must say:

- typed input record or loader,
- whether the graph creates a core relation or evidence attachment,
- graph object joined to,
- ambiguity preserved when the join fails.
