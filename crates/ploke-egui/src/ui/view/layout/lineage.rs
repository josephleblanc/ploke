use std::collections::HashSet;

use eframe::egui::Pos2;
use petgraph::{
    Direction::{Incoming, Outgoing},
    EdgeType,
    stable_graph::{IndexType, NodeIndex},
    visit::EdgeRef,
};

use super::{State, sorted_nodes};

const ROOT_GAP_MULTIPLE: f32 = 1.5;

#[derive(Debug, Default)]
pub(in crate::ui::view) struct Lineage {
    state: State,
}

impl Lineage {
    pub(super) fn apply<N, E, Ty, Ix, Dn, De>(
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
        let roots = sorted_nodes(graph.g().node_indices().filter(|node| {
            state.node_visible(*node)
                && graph
                    .g()
                    .edges_directed(*node, Incoming)
                    .filter(|edge| state.edge_visible(edge.id()))
                    .all(|edge| !state.node_visible(edge.source()))
        }));
        let mut visited = HashSet::new();
        let mut cursor = Cursor::default();

        for root in roots {
            if visited.contains(&root) {
                continue;
            }

            place_lineage(graph, &mut visited, root, 0, &mut cursor, state);
            cursor.advance_root_gap(state);
        }

        let remaining = sorted_nodes(
            graph
                .g()
                .node_indices()
                .filter(|node| state.node_visible(*node)),
        );
        for node in remaining {
            if visited.contains(&node) {
                continue;
            }

            place_lineage(graph, &mut visited, node, 0, &mut cursor, state);
            cursor.advance_root_gap(state);
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
    cursor: &mut Cursor,
    state: &State,
) -> Placement
where
    N: Clone,
    E: Clone,
    Ty: EdgeType,
    Ix: IndexType,
    Dn: egui_graphs::DisplayNode<N, E, Ty, Ix>,
    De: egui_graphs::DisplayEdge<N, E, Ty, Ix, Dn>,
{
    visited.insert(node);

    let children = sorted_nodes(
        graph
            .g()
            .edges_directed(node, Outgoing)
            .filter(|edge| state.edge_visible(edge.id()))
            .map(|edge| edge.target()),
    )
    .into_iter()
    .filter(|child| state.node_visible(*child))
    .filter(|child| !visited.contains(child))
    .collect::<Vec<_>>();

    let placement = if children.is_empty() {
        cursor.take_leaf(state)
    } else {
        let mut child_placements = Vec::with_capacity(children.len());
        for child in children {
            child_placements.push(place_lineage(
                graph,
                visited,
                child,
                depth + 1,
                cursor,
                state,
            ));
        }

        Placement::parent(&child_placements)
    };

    graph.g_mut()[node].set_location(placement.location(depth, state));
    placement
}

#[derive(Debug, Default)]
struct Cursor {
    next_leaf: usize,
}

impl Cursor {
    fn take_leaf(&mut self, state: &State) -> Placement {
        let slot = self.next_leaf;
        self.next_leaf = self.next_leaf.saturating_add(1);
        let columns = state.max_columns.max(1);
        Placement {
            x: (slot % columns) as f32 * state.col_dist,
            min_lane: slot / columns,
            max_lane: slot / columns,
        }
    }

    fn advance_root_gap(&mut self, state: &State) {
        let gap = (ROOT_GAP_MULTIPLE.ceil() as usize).max(1);
        self.next_leaf = self.next_leaf.saturating_add(gap);
        let columns = state.max_columns.max(1);
        let remainder = self.next_leaf % columns;
        if remainder > columns.saturating_sub(gap + 1) {
            self.next_leaf = self.next_leaf.saturating_add(columns - remainder);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Placement {
    x: f32,
    min_lane: usize,
    max_lane: usize,
}

impl Placement {
    fn parent(children: &[Self]) -> Self {
        let x = children.iter().map(|child| child.x).sum::<f32>() / children.len() as f32;
        let min_lane = children
            .iter()
            .map(|child| child.min_lane)
            .min()
            .expect("parent placement needs at least one child");
        let max_lane = children
            .iter()
            .map(|child| child.max_lane)
            .max()
            .expect("parent placement needs at least one child");
        Self {
            x,
            min_lane,
            max_lane,
        }
    }

    fn location(self, depth: usize, state: &State) -> Pos2 {
        Pos2::new(
            self.x,
            depth as f32 * state.row_dist + self.min_lane as f32 * state.lane_dist,
        )
    }
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
            lane_dist: 160.0,
            max_columns: 8,
            visibility_filter_active: false,
            visible_nodes: Vec::new(),
            visible_edges: Vec::new(),
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

    #[test]
    fn lineage_layout_reserves_leaf_span_for_nested_subtrees() {
        let mut raw = StableGraph::<&'static str, (), Directed>::default();
        let parent = raw.add_node("parent");
        let left = raw.add_node("left-subtree");
        let right = raw.add_node("right-leaf");
        let left_first = raw.add_node("left-first");
        let left_second = raw.add_node("left-second");
        let left_third = raw.add_node("left-third");

        raw.add_edge(parent, left, ());
        raw.add_edge(parent, right, ());
        raw.add_edge(left, left_first, ());
        raw.add_edge(left, left_second, ());
        raw.add_edge(left, left_third, ());

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
            lane_dist: 160.0,
            max_columns: 8,
            visibility_filter_active: false,
            visible_nodes: Vec::new(),
            visible_edges: Vec::new(),
        };

        Lineage::apply(&mut graph, &state);

        let left_x = graph.g().node_weight(left).unwrap().location().x;
        let right_x = graph.g().node_weight(right).unwrap().location().x;
        let left_first_x = graph.g().node_weight(left_first).unwrap().location().x;
        let left_third_x = graph.g().node_weight(left_third).unwrap().location().x;
        let parent_x = graph.g().node_weight(parent).unwrap().location().x;

        assert_eq!(left_x, (left_first_x + left_third_x) / 2.0);
        assert!(right_x - left_third_x >= state.col_dist);
        assert_eq!(parent_x, (left_x + right_x) / 2.0);
    }

    #[test]
    fn lineage_layout_wraps_wide_leaf_sets_into_lanes() {
        let mut raw = StableGraph::<&'static str, (), Directed>::default();
        let parent = raw.add_node("parent");
        let leaves = (0..9)
            .map(|_| {
                let leaf = raw.add_node("leaf");
                raw.add_edge(parent, leaf, ());
                leaf
            })
            .collect::<Vec<_>>();

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
            lane_dist: 160.0,
            max_columns: 4,
            visibility_filter_active: false,
            visible_nodes: Vec::new(),
            visible_edges: Vec::new(),
        };

        Lineage::apply(&mut graph, &state);

        let bounds = node_bounds(&graph).expect("test graph has nodes");
        assert!(bounds.width() <= state.col_dist * (state.max_columns - 1) as f32 + 1.0);
        assert!(bounds.height() >= state.row_dist + state.lane_dist * 2.0);

        let first_leaf_y = graph.g().node_weight(leaves[0]).unwrap().location().y;
        let wrapped_leaf_y = graph.g().node_weight(leaves[4]).unwrap().location().y;
        assert_eq!(wrapped_leaf_y - first_leaf_y, state.lane_dist);
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
