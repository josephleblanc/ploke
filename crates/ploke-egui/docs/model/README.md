# ploke-egui Graph Model Notes

This directory records the working model for how `ploke-egui` turns a
`ploke_tree::Graph` into an interactive inspection UI. Upstream run logs,
records, reports, and report-like projections belong to the graph/import side;
these docs start from the loaded `Graph` and specify how the UI answers
operator drilldown questions without becoming a second graph model.

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
