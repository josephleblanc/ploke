use eframe::egui::{Color32, Pos2, Shape, Vec2};
use petgraph::{EdgeType, stable_graph::IndexType};

use super::effects::{self, NodeVisualEffect};
use super::projection::GraphNode;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct GraphNodeShape {
    inner: egui_graphs::DefaultNodeShape,
    effect: NodeVisualEffect,
    visible: bool,
}

impl From<egui_graphs::NodeProps<GraphNode>> for GraphNodeShape {
    fn from(node_props: egui_graphs::NodeProps<GraphNode>) -> Self {
        let visible = node_props.payload.visible();
        let effect = node_props.payload.effect();
        Self {
            inner: egui_graphs::DefaultNodeShape::from(node_props),
            effect,
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
        let _span = tracing::trace_span!("egui_graphs_node_shape_layout").entered();
        if self.visible {
            let effect = self.effect.with_interaction(
                self.inner.selected,
                self.inner.hovered,
                self.inner.dragged,
            );
            if effect.needs_repaint() {
                ctx.ctx
                    .request_repaint_after(effects::EFFECT_REPAINT_INTERVAL);
            }

            let center = ctx.meta.canvas_to_screen_pos(self.inner.pos);
            let radius = ctx.meta.canvas_to_screen_size(self.inner.radius);
            let color = self.node_color(ctx);
            let time = ctx.ctx.input(|input| input.time);
            let mut shapes = effects::node_backdrop_shapes(center, radius, color, effect, time);
            shapes.extend(<egui_graphs::DefaultNodeShape as egui_graphs::DisplayNode<
                GraphNode,
                E,
                Ty,
                Ix,
            >>::shapes(&mut self.inner, ctx));
            shapes
        } else {
            Vec::new()
        }
    }

    fn update(&mut self, state: &egui_graphs::NodeProps<GraphNode>) {
        let _span = tracing::trace_span!("egui_graphs_node_update").entered();
        self.visible = state.payload.visible();
        self.effect = state.payload.effect();
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

    fn node_color(&self, ctx: &egui_graphs::DrawContext) -> Color32 {
        self.inner.color.unwrap_or_else(|| {
            ctx.ctx
                .global_style()
                .visuals
                .widgets
                .inactive
                .fg_stroke
                .color
        })
    }
}
