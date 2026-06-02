# ploke-egui Dependency Leverage Review

Scope: `crates/ploke-egui`, plus `crates/ploke-egui/Cargo.toml` and dependency APIs as needed. I did not inspect or edit `ploke-eval`.

## Findings

### High: custom tree layout duplicates `egui_graphs` hierarchical layout

`crates/ploke-egui/src/ui/view/layout.rs:11` defines a custom serializable layout state, and `layout.rs:30` through `layout.rs:151` implements `CenteredTree` with root discovery, visited tracking, recursive subtree placement, and parent-centering behavior.

`egui_graphs` already ships this concept: `LayoutStateHierarchical` / `LayoutHierarchical` are re-exported from `egui_graphs/src/lib.rs`, and the implementation has `center_parent` and `orientation` fields at `egui_graphs-0.30.0/src/layouts/hierarchical/layout.rs:27` through `layout.rs:39`. Its `Hierarchical` layout performs root discovery and component fallback at `layout.rs:60` through `layout.rs:109`, with recursive placement at `layout.rs:112` through `layout.rs:169`.

Risk: our graph view now owns layout algorithm maintenance without having first proven the crate-provided hierarchical layout is insufficient. This is exactly the part currently making the visual result bad, so custom code here has high design and regression cost.

Benefit of leveraging dependency: use `egui_graphs::LayoutHierarchical` with `center_parent: true`, `orientation: TopDown`, and our row/column distances first. If it is visually insufficient, keep only the missing semantic ordering/selected-path behavior in local code instead of owning the whole layout algorithm.

### High: bounds and fit diagnostics reimplement metadata that `egui_graphs` already tracks

`crates/ploke-egui/src/ui/view/diagnostics.rs:7` through `diagnostics.rs:33` computes graph size, aspect ratio, fitted size, and fill. `diagnostics.rs:36` through `diagnostics.rs:54` manually scans node locations to compute bounds.

`egui_graphs` already maintains per-widget metadata with zoom, pan, timing, and graph bounds. See `egui_graphs-0.30.0/src/metadata.rs:53` through `metadata.rs:68`, `MetadataFrame::graph_bounds` at `metadata.rs:146` through `metadata.rs:149`, and `get_metrics` at `egui_graphs-0.30.0/src/graph_view.rs:1316` through `graph_view.rs:1320`.

Risk: the current diagnostics can disagree with what the widget actually fit or drew. It only scans node centers plus a one-pixel pad, while `egui_graphs` expands bounds with node radius and edge `extra_bounds`. That makes this a weak visual regression signal.

Benefit of leveraging dependency: base viewport diagnostics on `egui_graphs` metadata where possible, including `graph_bounds`, `zoom`, `pan`, `last_step_time_ms`, and `last_draw_time_ms`. The reportable box would then describe the same geometry the widget uses for fit/pan/zoom.

### Medium: custom edge shape discards `egui_graphs` edge order and duplicates default edge behavior

`crates/ploke-egui/src/ui/view/edge.rs:15` through `edge.rs:24` defines `GraphEdgeShape`, but it does not store the `EdgeProps::order` value. `edge.rs:26` through `edge.rs:37` copies id, label, status, selected, and style, then ignores order. The drawing path implements custom cubic geometry, label layout, hit testing, and extra bounds at `edge.rs:116` through `edge.rs:210`.

`egui_graphs::DefaultEdgeShape` already stores `order`, selection state, width, tip, curve, loop size, and label at `egui_graphs-0.30.0/src/draw/displays_default/edge.rs:13` through `edge.rs:24`; it initializes those from `EdgeProps` at `edge.rs:26` through `edge.rs:39`; and it uses `order` to choose straight/curved rendering at `edge.rs:45` through `edge.rs:80`. `egui_graphs::Graph::add_edge_custom` assigns sibling edge order at `egui_graphs-0.30.0/src/graph.rs:243` through `graph.rs:307`.

Risk: parallel or opposite-direction edges can visually stack or curve inconsistently because our custom shape throws away the library’s ordering signal. We also own hit testing and extra bounds that the default shape already handles.

Benefit of leveraging dependency: either use `DefaultEdgeShape` plus `SettingsStyle` hooks where status-specific styling allows it, or preserve `EdgeProps::order` in `GraphEdgeShape` and make local edge drawing an extension of the library model instead of a replacement.

### Medium: interaction state exists in `egui_graphs`, but `ploke-egui` does not project it back to the operator UI

`crates/ploke-egui/src/ui/view/mod.rs:43` through `mod.rs:48` enables dragging, node selection, edge selection, and labels. `mod.rs:97` through `mod.rs:109` builds the widget and records only diagnostics after `ui.add`.

`egui_graphs::Graph` exposes selected and hovered state: `selected_nodes`, `selected_edges`, `hovered_node`, and `dragged_node` at `egui_graphs-0.30.0/src/graph.rs:395` through `graph.rs:424`. The optional `events` feature also exposes node/edge click, select, hover, pan, and zoom event payloads at `egui_graphs-0.30.0/src/events/event.rs:3` through `event.rs:93`, with `GraphView::with_event_sink` available at `egui_graphs-0.30.0/src/graph_view.rs:371` through `graph_view.rs:384`.

Risk: the app enables interactions but does not turn them into side-panel selection, hover details, or patch evidence drilldown. From the operator’s perspective, this makes the graph feel inert even when the widget is doing useful work internally.

Benefit of leveraging dependency: start by reading `selected_nodes` / `selected_edges` from the cached widget graph after `show`; only enable the `events` feature if we need event streams for pan/zoom telemetry or richer hover timing. This should connect existing interaction state to the `Graph` evidence/detail UI without inventing another event system.

### Medium: candidate ordering is a second graph traversal separate from the rendered graph

`crates/ploke-egui/src/ui/view/order.rs:8` through `order.rs:38` builds a parent-to-children map and visits candidates manually. `projection.rs:105` uses this order to add nodes to a `StableGraph`; then `layout.rs:47` through `layout.rs:68` independently discovers roots and remaining nodes from the rendered graph.

Risk: ordering and layout can drift because there are two traversals over two representations. This is especially risky as soon as selected-path dominance, generation grouping, or non-tree edges are added.

Benefit of leveraging dependency: after constructing the `StableGraph`, use `petgraph::algo::toposort` or the graph’s own roots/neighbor traversal as the single ordering source. `petgraph` provides `toposort` at `petgraph-0.8.3/src/algo/mod.rs:211` and path/connectivity helpers like `has_path_connecting` at `algo/mod.rs:369`. Use those to validate “execution spine is a DAG/tree-like projection” before layout, instead of trusting a separate pre-order pass.

### Low: dependency declarations include currently unused or stale surfaces

`crates/ploke-egui/Cargo.toml:16` declares `serde_json`, and `Cargo.toml:17` declares `tracing`. In compiled source, `tracing` does not appear to be used. `serde_json` appears only in `crates/ploke-egui/src/history_playback.rs:128`, but `history_playback.rs` is not included from `lib.rs:18` through `lib.rs:27` and still references non-existent local modules at `history_playback.rs:7` through `history_playback.rs:8`.

Risk: stale dependencies and uncompiled transplanted files make it harder to tell which crate boundaries are real. They also make dependency review noisier.

Benefit of leveraging dependency discipline: either wire `history_playback` deliberately into the crate’s actual module tree, or remove/defer the stale file and drop `serde_json` until a compiled path needs it. Drop `tracing` until there is a concrete instrumentation call site.

## Suggested order

1. Replace or benchmark `CenteredTree` against `egui_graphs::LayoutHierarchical` before further custom layout work.
2. Move diagnostics toward `egui_graphs` metadata so visual tests observe the same bounds/fit state the widget uses.
3. Preserve `EdgeProps::order` in `GraphEdgeShape`, or return to `DefaultEdgeShape` plus hooks if status coloring can be represented cleanly.
4. Surface selected/hovered widget graph state into the side panel.
5. Delete or explicitly defer stale dependency/file surfaces.

## Verification note

I ran `cargo check -p ploke-egui`; it passed. No implementation files were changed.
