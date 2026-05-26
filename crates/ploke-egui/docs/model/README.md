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
- [source-process-graph.md](source-process-graph.md) explains what Prototype 1
  process the graph represents, which typed records feed it, and what nodes and
  edges should explain in the UI.
- [view-set-contract.md](view-set-contract.md) defines the intended graph view
  modes as node and edge set projections over `ploke_tree::Graph`.
- [default-view-contract.md](default-view-contract.md) defines the intended
  default app layout, current implementation status, and CLI-testable signals
  for the artifact-first graph view.
- [debugger-claim-workflow.md](debugger-claim-workflow.md) defines the
  evidence-first workflow for turning a developer-visible debugger claim into a
  typed graph-derived UI witness.
- [protocol-and-evaluation-data-locations.md](protocol-and-evaluation-data-locations.md)
  maps Prototype 1 evaluation artifacts, protocol evidence records, and
  `record.json.gz` paths to the current `ploke_tree::Graph` import boundary.
- [analyst-representation.md](analyst-representation.md) describes how run,
  protocol, tool-call, and patch facts should be shaped into analyst-facing
  summaries before the operator drills into raw evidence.
- [animation-hooks.md](animation-hooks.md) records how `DisplayNode`,
  `DisplayEdge`, and custom layout implementations can support transitions
  without making display state semantic authority.
