use eframe::egui::{Pos2, Shape, Vec2};
use petgraph::{EdgeType, stable_graph::IndexType};

use super::projection::GraphNode;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct GraphNodeShape {
    inner: egui_graphs::DefaultNodeShape,
    visible: bool,
}

impl From<egui_graphs::NodeProps<GraphNode>> for GraphNodeShape {
    fn from(node_props: egui_graphs::NodeProps<GraphNode>) -> Self {
        let visible = node_props.payload.visible();
        Self {
            inner: egui_graphs::DefaultNodeShape::from(node_props),
            visible,
        }
    }
}

impl<E, Ty, Ix> egui_graphs::DisplayNode<GraphNode, E, Ty, Ix> for GraphNodeShape
where
    E: Clone,
    Ty: EdgeType,
    Ix: IndexType,
{
    fn closest_boundary_point(&self, dir: Vec2) -> Pos2 {
        <egui_graphs::DefaultNodeShape as egui_graphs::DisplayNode<GraphNode, E, Ty, Ix>>::closest_boundary_point(&self.inner, dir)
    }

    fn shapes(&mut self, ctx: &egui_graphs::DrawContext) -> Vec<Shape> {
        if self.visible {
            <egui_graphs::DefaultNodeShape as egui_graphs::DisplayNode<GraphNode, E, Ty, Ix>>::shapes(&mut self.inner, ctx)
        } else {
            Vec::new()
        }
    }

    fn update(&mut self, state: &egui_graphs::NodeProps<GraphNode>) {
        self.visible = state.payload.visible();
        <egui_graphs::DefaultNodeShape as egui_graphs::DisplayNode<GraphNode, E, Ty, Ix>>::update(
            &mut self.inner,
            state,
        );
    }

    fn is_inside(&self, pos: Pos2) -> bool {
        self.visible
            && <egui_graphs::DefaultNodeShape as egui_graphs::DisplayNode<
                GraphNode,
                E,
                Ty,
                Ix,
            >>::is_inside(&self.inner, pos)
    }
}

impl GraphNodeShape {
    pub(super) fn set_radius(&mut self, radius: f32) {
        self.inner.radius = radius;
    }
}
