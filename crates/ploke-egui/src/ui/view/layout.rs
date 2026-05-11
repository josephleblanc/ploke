use std::collections::HashSet;

use eframe::egui::Pos2;
use petgraph::{
    Direction::{Incoming, Outgoing},
    EdgeType,
    stable_graph::{IndexType, NodeIndex},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct State {
    pub(super) triggered: bool,
    pub(super) row_dist: f32,
    pub(super) col_dist: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            triggered: false,
            row_dist: 240.0,
            col_dist: 90.0,
        }
    }
}

impl egui_graphs::LayoutState for State {}

#[derive(Debug, Default)]
pub(super) struct Lineage {
    state: State,
}

impl Lineage {
    fn apply<N, E, Ty, Ix, Dn, De>(
        graph: &mut egui_graphs::Graph<N, E, Ty, Ix, Dn, De>,
        state: &State,
    ) where
        N: Clone,
        E: Clone,
        Ty: EdgeType,
        Ix: IndexType,
        Dn: egui_graphs::DisplayNode<N, E, Ty, Ix>,
        De: egui_graphs::DisplayEdge<N, E, Ty, Ix, Dn>,
    {
        let roots = sorted_nodes(graph.g().externals(Incoming));
        let mut visited = HashSet::new();
        let mut next_root = 0usize;

        for root in roots {
            if visited.contains(&root) {
                continue;
            }

            let root_x = next_root as f32 * state.col_dist * 2.0;
            place_lineage(graph, &mut visited, root, 0, root_x, state);
            next_root = next_root.saturating_add(1);
        }

        let remaining = sorted_nodes(graph.g().node_indices());
        for node in remaining {
            if visited.contains(&node) {
                continue;
            }

            let root_x = next_root as f32 * state.col_dist * 2.0;
            place_lineage(graph, &mut visited, node, 0, root_x, state);
            next_root = next_root.saturating_add(1);
        }
    }
}

impl egui_graphs::Layout<State> for Lineage {
    fn from_state(state: State) -> impl egui_graphs::Layout<State> {
        Self { state }
    }

    fn next<N, E, Ty, Ix, Dn, De>(
        &mut self,
        graph: &mut egui_graphs::Graph<N, E, Ty, Ix, Dn, De>,
        _ui: &egui::Ui,
    ) where
        N: Clone,
        E: Clone,
        Ty: EdgeType,
        Ix: IndexType,
        Dn: egui_graphs::DisplayNode<N, E, Ty, Ix>,
        De: egui_graphs::DisplayEdge<N, E, Ty, Ix, Dn>,
    {
        if self.state.triggered {
            return;
        }

        Self::apply(graph, &self.state);
        self.state.triggered = true;
    }

    fn state(&self) -> State {
        self.state.clone()
    }
}

fn place_lineage<N, E, Ty, Ix, Dn, De>(
    graph: &mut egui_graphs::Graph<N, E, Ty, Ix, Dn, De>,
    visited: &mut HashSet<NodeIndex<Ix>>,
    node: NodeIndex<Ix>,
    depth: usize,
    x: f32,
    state: &State,
) where
    N: Clone,
    E: Clone,
    Ty: EdgeType,
    Ix: IndexType,
    Dn: egui_graphs::DisplayNode<N, E, Ty, Ix>,
    De: egui_graphs::DisplayEdge<N, E, Ty, Ix, Dn>,
{
    visited.insert(node);
    graph.g_mut()[node].set_location(Pos2::new(x, depth as f32 * state.row_dist));

    let children = sorted_nodes(graph.g().neighbors_directed(node, Outgoing))
        .into_iter()
        .filter(|child| !visited.contains(child))
        .collect::<Vec<_>>();

    for (index, child) in children.iter().enumerate() {
        let child_x = x + centered_offset(index, children.len(), state.col_dist);
        place_lineage(graph, visited, *child, depth + 1, child_x, state);
    }
}

fn sorted_nodes<Ix>(nodes: impl Iterator<Item = NodeIndex<Ix>>) -> Vec<NodeIndex<Ix>>
where
    Ix: IndexType,
{
    let mut nodes = nodes.collect::<Vec<_>>();
    nodes.sort_by_key(|node| node.index());
    nodes
}

fn centered_offset(index: usize, count: usize, spacing: f32) -> f32 {
    let center = (count.saturating_sub(1)) as f32 / 2.0;
    (index as f32 - center) * spacing
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Pos2, Rect, Vec2};
    use petgraph::{Directed, stable_graph::StableGraph};

    use super::*;

    #[test]
    fn lineage_layout_uses_depth_before_leaf_width() {
        let mut raw = StableGraph::<&'static str, (), Directed>::default();
        let parent = raw.add_node("parent");
        let left = raw.add_node("left");
        let selected = raw.add_node("selected");
        let right = raw.add_node("right");
        let selected_left = raw.add_node("selected-left");
        let selected_right = raw.add_node("selected-right");

        raw.add_edge(parent, left, ());
        raw.add_edge(parent, selected, ());
        raw.add_edge(parent, right, ());
        raw.add_edge(selected, selected_left, ());
        raw.add_edge(selected, selected_right, ());

        let mut graph: egui_graphs::Graph<
            &str,
            (),
            Directed,
            petgraph::stable_graph::DefaultIx,
            egui_graphs::DefaultNodeShape,
            egui_graphs::DefaultEdgeShape,
        > = egui_graphs::to_graph(&raw);
        let state = State {
            triggered: false,
            row_dist: 240.0,
            col_dist: 90.0,
        };

        Lineage::apply(&mut graph, &state);

        let bounds = node_bounds(&graph).expect("test graph has nodes");
        let parent_x = graph.g().node_weight(parent).unwrap().location().x;
        let left_x = graph.g().node_weight(left).unwrap().location().x;
        let right_x = graph.g().node_weight(right).unwrap().location().x;
        assert_eq!(parent_x, (left_x + right_x) / 2.0);
        assert!(bounds.height() > bounds.width());

        let viewport = Vec2::new(900.0, 600.0);
        let fitted = fit_bounds(bounds, viewport, 0.18);
        let viewport_rect = Rect::from_min_size(Pos2::ZERO, viewport);
        assert!(viewport_rect.contains(fitted.min));
        assert!(viewport_rect.contains(fitted.max));
        assert!(fitted.center().distance(viewport_rect.center()) < 0.5);
    }

    fn node_bounds<N, E, Ty, Ix, Dn, De>(
        graph: &egui_graphs::Graph<N, E, Ty, Ix, Dn, De>,
    ) -> Option<Rect>
    where
        N: Clone,
        E: Clone,
        Ty: EdgeType,
        Ix: IndexType,
        Dn: egui_graphs::DisplayNode<N, E, Ty, Ix>,
        De: egui_graphs::DisplayEdge<N, E, Ty, Ix, Dn>,
    {
        let mut nodes = graph.g().node_weights();
        let first = nodes.next()?.location();
        let mut min = first;
        let mut max = first;

        for node in nodes {
            let location = node.location();
            min.x = min.x.min(location.x);
            min.y = min.y.min(location.y);
            max.x = max.x.max(location.x);
            max.y = max.y.max(location.y);
        }

        Some(Rect::from_min_max(min, max))
    }

    fn fit_bounds(bounds: Rect, viewport: Vec2, padding: f32) -> Rect {
        let graph_size = (bounds.max - bounds.min) * (1.0 + padding);
        let zoom = (viewport.x / graph_size.x.max(1.0)).min(viewport.y / graph_size.y.max(1.0));
        let pan = viewport / 2.0 - bounds.center().to_vec2() * zoom;
        Rect::from_min_max(
            (bounds.min.to_vec2() * zoom + pan).to_pos2(),
            (bounds.max.to_vec2() * zoom + pan).to_pos2(),
        )
    }
}
