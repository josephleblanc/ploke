# Graph Pipeline

`ploke_tree::Graph` is the semantic source available to `ploke-egui`.
`ploke-egui` should treat it as the owner of graph meaning for inspection and
should not turn copied labels, raw ids, or details into a second semantic
model. Upstream logs, records, and reports from `ploke-eval` must be loaded or
folded into typed graph material before the UI uses them.

## Current Layers

The current rendering path is:

```text
ploke_tree::Graph
  -> graph-owned named projection
  -> project_*()
  -> RawGraph
  -> to_widget_graph()
  -> WidgetGraph
  -> egui_graphs::GraphView
  -> egui painter
```

The source graph enters `projection.rs` as:

```rust
use ploke_tree::Graph as DomainGraph;
```

`DomainGraph` owns typed collections such as artifacts, history blocks,
candidates, selections, operations, source/projection attachments, and
warnings.

The required layer is a graph-owned projection for semantic view membership.
For the default canvas, that object should expose the run-forest topology when
available, or the fallback artifact node set and classified artifact edge sets,
before egui allocates labels, layout coordinates, or widget payloads.

`RawGraph` is a local projection graph:

```rust
type RawGraph = StableGraph<GraphNode, GraphEdgePayload, Directed>;
```

It answers: which nodes and edges should this view expose?

`WidgetGraph` is the mutable `egui_graphs` adapter:

```rust
egui_graphs::Graph<GraphNode, GraphEdgePayload, ...>
```

It answers: what does the widget need to draw, lay out, select, drag, and
hit-test?

## Widget Nodes

Each `egui_graphs::Node` contains both payload and display state:

```text
Node {
  props: NodeProps<GraphNode> {
    payload: GraphNode,
    label: String,
    selected: bool,
    dragged: bool,
    hovered: bool,
    color: Option<Color32>,
    location: Pos2,
  },
  display: impl DisplayNode,
}
```

In current code:

```rust
type WidgetNode = egui_graphs::Node<
    GraphNode,
    GraphEdgePayload,
    Directed,
    DefaultIx,
    GraphNodeShape,
>;
```

So:

```text
GraphNode = ploke-egui payload for the projected node
DisplayNode = drawing and hit-test implementation for that node
```

## View Membership

`egui_graphs` 0.30.0 has no first-class hide/show property that excludes a
node or edge from layout, drawing, hit testing, and diagnostics. Its node props
carry payload, label, selected, dragged, hovered, color, and location. Its edge
props carry payload, order, selected, and label. The drawer iterates all node
indices and all edge indices.

`ploke-egui` therefore owns visibility explicitly in its projected payloads.
The artifact/lineage product views share one superset `WidgetGraph`, and view
mode changes mutate node/edge visibility rather than rebuilding a different
semantic graph.

```text
DomainGraph
  -> superset projection for artifact/lineage layers
  -> WidgetGraph with layer masks and visible flags
  -> GraphViewMode mutates visible flags
```

Because `egui_graphs` does not provide visibility semantics, every local layer
that observes the widget graph must respect `visible`:

- `GraphNodeShape` returns no shapes and no hit-test region for hidden nodes.
- `GraphEdgeShape` returns no shapes and no hit-test region for hidden edges.
- `layout::State` carries visible node/edge index sets so layout ignores hidden
  topology.
- Diagnostics count and measure visible nodes/edges.
- Selection detail ignores hidden selected nodes.

## Multiple Views

Multiple graph views can read the same `DomainGraph`:

```rust
artifact_view.show(ui, &graph);
debug_view.show(ui, &graph);
```

Each `GraphView` should own its own UI state: mode, visibility filter, layout,
pan/zoom, selection, diagnostics, expansion state, and animation state. The
single `DomainGraph` remains the semantic source.

The current product direction is:

```text
DomainGraph -> one artifact/lineage WidgetGraph
GraphViewMode::ArtifactTree        -> show artifact layer
GraphViewMode::Lineage             -> show lineage layer
GraphViewMode::ArtifactAndLineage  -> show both layers
GraphViewMode::Empty               -> show neither layer
```

All live graph modes use the artifact/lineage projection. Record-level graph
statistics should be exposed as bounded diagnostics derived from `&DomainGraph`,
not by rendering every loaded record as one large debug graph.

## Artifact Tree Invariant

The default `ArtifactTree` mode is the git-tree product surface. It must render
artifact patch edges from parent artifact to child artifact so the root parent
is above its descendants in the top-down layout. The latest selected successor
artifact on the primary lineage is the next `Parent<Ruler>` and must be visually
highlighted as the current ruler candidate. The previous parent remains visible
as the source of the patch edge, not as the highlighted node.

## Current Problem Area

The current cache owns a full `WidgetGraph`:

```rust
struct GraphViewCache {
    graph: WidgetGraph,
    ...
}
```

That widget graph currently carries `GraphNode` payloads with owned
`Arc<str>` labels/details. This is convenient for `egui_graphs`, but it risks
turning copied display text into a second semantic source.

The cleaner direction is:

```text
GraphView cache:
  superset widget adapter plus UI-only state keyed by domain-backed handles

DomainGraph:
  semantic ids, labels, details, records

render/build pass:
  rejoin UI state with &DomainGraph and keep copied text render-only
```

If a renderer requires owned text, that ownership is limited to the renderer
call or widget payload. Inspector and graph projection rows must stay borrowed
or typed; do not introduce generic owned string rows as a convenience layer.
