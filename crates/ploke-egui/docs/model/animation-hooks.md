# Animation Hooks

Animation belongs in display and layout layers, not in the semantic
`ploke_tree::Graph`.

The useful split is:

```text
projection/filter = which nodes and edges are present
layout            = where nodes are placed
DisplayNode       = how nodes draw and hit-test
DisplayEdge       = how edges draw and hit-test
```

## DisplayNode

`egui_graphs` stores a display object on each widget node. A custom
`DisplayNode` can animate appearance without changing the domain graph.

Useful responsibilities:

- Fade nodes in/out.
- Animate radius or badge size.
- Draw selected/current artifacts differently.
- Draw compact labels such as `A1`.
- Return no shapes during an exit transition.
- Return `false` from hit testing when a node should not interact.

Sketch:

```rust
struct GraphNodeShape {
    opacity: f32,
    radius: f32,
}
```

The shape reads `NodeProps<GraphNode>` in `update`, then draws intermediate
visual state in `shapes`.

## DisplayEdge

`DisplayEdge` can animate edge opacity, stroke width, arrowheads, labels, or
path reveal.

Useful responsibilities:

- Fade patch edges in/out.
- Emphasize selected lineage edges.
- Draw edge labels only when appropriate.
- Return no shapes during an exit transition.
- Return `false` from hit testing when an edge should not interact.

For transitions, the edge payload or view animation cache can carry a
render-only progress value:

```text
t = 0.0: opacity 0, width 0
t = 1.0: opacity 1, width normal
```

## Visibility Contract

Custom display types can make an item invisible or unclickable, but that alone
does not remove it from the graph. If the node or edge remains in
`WidgetGraph`, layout and graph-level diagnostics still see it unless the
layout and diagnostics also explicitly filter it.

For artifact/lineage view switching, `ploke-egui` owns a coordinated
visibility layer:

```text
payload visible flag
  -> display skips drawing/hit testing
  -> layout skips hidden topology
  -> diagnostics count visible topology
```

This makes one superset `WidgetGraph` viable for artifact/lineage filters.

## Transition Flow

A view transition can work like this:

```text
1. Preserve stable domain-backed handles for nodes and edges.
2. Mutate layer visibility for the target view.
3. Preserve node positions and animation state by handle.
4. Let layout move visible nodes toward their target positions.
5. Let DisplayNode/DisplayEdge fade/move/reveal visual state.
6. Keep exiting items visible until their exit animation completes, then hide
   them from layout/draw/hit testing together.
```

This keeps view membership structural while allowing smooth visual changes.

## Constrained Force Layout

Artifact tree view should remain deterministic on the main axis:

```text
parent artifact above child artifact
generation/depth controls y
```

Force-directed behavior can still be useful within a generation row:

```text
y = fixed by depth
x = relaxed by sibling repulsion and parent-child attraction
```

This hybrid gives the graph a stable git-tree shape while allowing siblings to
spread out naturally.

Possible forces:

- Sibling repulsion within the same depth.
- Parent-child horizontal attraction.
- Edge-length pressure.
- Optional crossing or overlap penalties.

This belongs in an artifact-specific layout implementation, for example:

```text
src/ui/view/layout/artifact_tree.rs
```

Projection should decide artifact membership and edges. Layout should decide
positions. Display should decide animation and shape.
