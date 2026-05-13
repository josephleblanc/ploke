# ploke-egui Graph Model Notes

This directory records the working model for how `ploke-egui` turns a
`ploke_tree::Graph` into an interactive graph UI.

## Documents

- [graph-pipeline.md](graph-pipeline.md) explains the current `DomainGraph ->
  RawGraph -> WidgetGraph -> egui_graphs::GraphView` stack, where nodes and
  edges live, and where visibility should be decided.
- [animation-hooks.md](animation-hooks.md) records how `DisplayNode`,
  `DisplayEdge`, and custom layout implementations can support transitions
  without making display state semantic authority.
