# ploke-tree Graph Ingestion

Agent-facing coordination packet for building `ploke-tree::Graph` from
Prototype 1 run records without spreading semantics across browser, egui, or
`ploke-eval` code.

Start here when delegating work on the graph import boundary:

- [`orchestration.md`](orchestration.md)
  Rules for main-thread orchestration, worker reports, retries, commits, and
  hard edit boundaries.
- [`implementation-lanes.md`](implementation-lanes.md)
  Proposed module split and parallel worker lanes for `RunRecordSet`, graph
  types, graph builders, and future record families.
- [`2026-05-11_3a2ea733-persisted-surface-survey.md`](2026-05-11_3a2ea733-persisted-surface-survey.md)
  Fresh metadata/source survey of persisted `.ploke-eval` files, worktree
  exclusions, current loader coverage, and unresolved loader rows.
- [`../2026-05-11_ploke-tree-graph-ingestion-inventory.md`](../2026-05-11_ploke-tree-graph-ingestion-inventory.md)
  Current source-family tracker: what is loaded into `RunRecordSet`, what is
  ingested into `Graph`, and what is still missing.
- [`../2026-05-11_ploke-egui-graph-import-boundary-handoff.md`](../2026-05-11_ploke-egui-graph-import-boundary-handoff.md)
  Current restart handoff for the graph/import boundary and UI direction.

Core model:

- `ploke-records` owns passive persisted record shapes.
- `ploke-tree` owns typed run loading, read-side indexing, and
  `ploke-tree::Graph`.
- `ploke-egui` and browser-facing code are projections over the graph, not
  alternate semantic authorities.
- `History` is the primary ordering spine. Other records attach evidence,
  secondary order, diagnostics, or unresolved ambiguity.
- The graph interface is intended to become comprehensive for loop
  investigation. Every persisted loop-relevant surface should eventually be
  reachable through `Graph`, even when represented only as metadata, evidence,
  a locator, a digest, or an explicit ambiguity rather than as a rendered node
  or edge.
- Support current emitted run shapes only. Older path shapes are useful audit
  evidence, but they do not create migration or compatibility obligations. If
  an older run has a file that the current emitted shape does not, treat that
  file as unsupported unless the user explicitly reauthorizes legacy support.
