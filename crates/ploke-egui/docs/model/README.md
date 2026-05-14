# ploke-egui Graph Model Notes

This directory records the working model for how `ploke-egui` turns a
`ploke_tree::Graph` into an interactive graph UI.

## Documents

- [graph-pipeline.md](graph-pipeline.md) explains the current `DomainGraph ->
  RawGraph -> WidgetGraph -> egui_graphs::GraphView` stack, where nodes and
  edges live, and where visibility should be decided.
- [run-graph-crosswalk.md](run-graph-crosswalk.md) collects the typed record,
  `ploke-tree`, and UI graph vocabulary so entity and edge meanings stay
  canonical across docs and implementation.
- [view-set-contract.md](view-set-contract.md) defines the intended graph view
  modes as node and edge set projections over `ploke_tree::Graph`.
- [default-view-contract.md](default-view-contract.md) defines the intended
  default app layout, current implementation status, and CLI-testable signals
  for the artifact-first graph view.
- [animation-hooks.md](animation-hooks.md) records how `DisplayNode`,
  `DisplayEdge`, and custom layout implementations can support transitions
  without making display state semantic authority.
