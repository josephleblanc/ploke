mod lineage;

use petgraph::stable_graph::{EdgeIndex, IndexType, NodeIndex};
use serde::{Deserialize, Serialize};

pub(super) use lineage::Lineage;

const DEFAULT_MAX_COLUMNS: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct State {
    pub(super) triggered: bool,
    pub(super) row_dist: f32,
    pub(super) col_dist: f32,
    pub(super) lane_dist: f32,
    pub(super) max_columns: usize,
    pub(super) visibility_filter_active: bool,
    pub(super) visible_nodes: Vec<usize>,
    pub(super) visible_edges: Vec<usize>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            triggered: false,
            row_dist: 280.0,
            col_dist: 132.0,
            lane_dist: 164.0,
            max_columns: DEFAULT_MAX_COLUMNS,
            visibility_filter_active: false,
            visible_nodes: Vec::new(),
            visible_edges: Vec::new(),
        }
    }
}

impl egui_graphs::LayoutState for State {}

impl State {
    pub(super) fn node_visible<Ix>(&self, node: NodeIndex<Ix>) -> bool
    where
        Ix: IndexType,
    {
        !self.visibility_filter_active || self.visible_nodes.binary_search(&node.index()).is_ok()
    }

    pub(super) fn edge_visible<Ix>(&self, edge: EdgeIndex<Ix>) -> bool
    where
        Ix: IndexType,
    {
        !self.visibility_filter_active || self.visible_edges.binary_search(&edge.index()).is_ok()
    }
}

pub(super) fn sorted_nodes<Ix>(nodes: impl Iterator<Item = NodeIndex<Ix>>) -> Vec<NodeIndex<Ix>>
where
    Ix: IndexType,
{
    let mut nodes = nodes.collect::<Vec<_>>();
    nodes.sort_by_key(|node| node.index());
    nodes
}

pub(super) fn apply_lineage<N, E, Ty, Ix, Dn, De>(
    graph: &mut egui_graphs::Graph<N, E, Ty, Ix, Dn, De>,
    state: &State,
) where
    N: Clone,
    E: Clone,
    Ty: petgraph::EdgeType,
    Ix: IndexType,
    Dn: egui_graphs::DisplayNode<N, E, Ty, Ix>,
    De: egui_graphs::DisplayEdge<N, E, Ty, Ix, Dn>,
{
    Lineage::apply(graph, state);
}
