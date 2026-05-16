use std::cell::Cell;
use std::sync::Arc;

use eframe::egui::{
    Color32, FontId, Galley, Id, LayerId, Order, Pos2, Shape, Stroke, Vec2,
    epaint::{CubicBezierShape, TextShape},
};
use petgraph::{EdgeType, stable_graph::IndexType};

use super::geometry::{curve_points, distance_to_curve, endpoint_direction, self_loop_points};
use super::label::{EdgeLabelInput, place_edge_label, record_edge_label};
use super::projection::GraphEdgePayload;
use super::style::{CurveStyle, EdgeStyle};

#[derive(Debug, Clone)]
pub(super) struct GraphEdgeShape {
    label: Arc<str>,
    label_visible: bool,
    color: Color32,
    selected: bool,
    visible: bool,
    style: EdgeStyle,
    curve: Cell<Option<EdgeCurve>>,
    label_galley: Option<EdgeLabelGalley>,
}

impl From<egui_graphs::EdgeProps<GraphEdgePayload>> for GraphEdgeShape {
    fn from(edge: egui_graphs::EdgeProps<GraphEdgePayload>) -> Self {
        let visible = edge.payload.visible();
        Self {
            label: edge.payload.label,
            label_visible: edge.payload.label_visible,
            color: edge.payload.color,
            selected: edge.selected,
            visible,
            style: edge.payload.style,
            curve: Cell::new(None),
            label_galley: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct EdgeCurve {
    key: EdgeCurveKey,
    points: [Pos2; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct EdgeCurveKey {
    start: Pos2,
    end: Pos2,
    style: CurveStyle,
}

impl EdgeCurveKey {
    fn new(start: Pos2, end: Pos2, style: CurveStyle) -> Self {
        Self { start, end, style }
    }
}

impl GraphEdgeShape {
    fn curve(&self, start: Pos2, end: Pos2) -> [Pos2; 4] {
        let key = EdgeCurveKey::new(start, end, self.style.curve);
        match self.curve.get() {
            Some(curve) if curve.key == key => curve.points,
            _ => {
                let points = curve_points(start, end, self.style.curve);
                self.curve.set(Some(EdgeCurve { key, points }));
                points
            }
        }
    }

    fn label_galley(&mut self, ctx: &egui_graphs::DrawContext, color: Color32) -> Arc<Galley> {
        let key = EdgeLabelGalleyKey {
            font_size: self.style.label.font_size,
            color,
        };
        match &self.label_galley {
            Some(cached) if cached.key == key => cached.galley.clone(),
            _ => {
                let galley = ctx.ctx.fonts_mut(|fonts| {
                    fonts.layout_no_wrap(
                        self.label.to_string(),
                        FontId::monospace(self.style.label.font_size),
                        color,
                    )
                });
                self.label_galley = Some(EdgeLabelGalley {
                    key,
                    galley: galley.clone(),
                });
                galley
            }
        }
    }

    /// archaeology:artifact-relations
    /// proof:docs/active/archaeology/ploke-tree-graph/artifact-relations.md
    fn curve_for_nodes<N, Ty, Ix, D>(
        &self,
        start: &egui_graphs::Node<N, GraphEdgePayload, Ty, Ix, D>,
        end: &egui_graphs::Node<N, GraphEdgePayload, Ty, Ix, D>,
    ) -> [Pos2; 4]
    where
        N: Clone,
        Ty: EdgeType,
        Ix: IndexType,
        D: egui_graphs::DisplayNode<N, GraphEdgePayload, Ty, Ix>,
    {
        if start.location() == end.location() {
            return self_loop_points(start.location(), node_radius(start), self.style.curve);
        }

        let (start_point, end_point) = attachment_points(start, end, self.style.curve);
        self.curve(start_point, end_point)
    }
}

#[derive(Debug, Clone)]
struct EdgeLabelGalley {
    key: EdgeLabelGalleyKey,
    galley: Arc<Galley>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct EdgeLabelGalleyKey {
    font_size: f32,
    color: Color32,
}

impl<N, Ty, Ix, D> egui_graphs::DisplayEdge<N, GraphEdgePayload, Ty, Ix, D> for GraphEdgeShape
where
    N: Clone,
    Ty: EdgeType,
    Ix: IndexType,
    D: egui_graphs::DisplayNode<N, GraphEdgePayload, Ty, Ix>,
{
    fn shapes(
        &mut self,
        start: &egui_graphs::Node<N, GraphEdgePayload, Ty, Ix, D>,
        end: &egui_graphs::Node<N, GraphEdgePayload, Ty, Ix, D>,
        ctx: &egui_graphs::DrawContext,
    ) -> Vec<Shape> {
        if !self.visible {
            return Vec::new();
        }
        let curve = self.curve_for_nodes(start, end);
        let screen_curve = curve.map(|point| ctx.meta.canvas_to_screen_pos(point));

        let color = self.color;
        let stroke_width = if self.selected {
            self.style.selected_width
        } else {
            self.style.normal_width
        };
        let mut shapes = Vec::with_capacity(1);
        shapes.push(
            CubicBezierShape::from_points_stroke(
                screen_curve,
                false,
                Color32::TRANSPARENT,
                Stroke::new(stroke_width, color),
            )
            .into(),
        );

        if self.label_visible {
            let galley = self.label_galley(ctx, color);
            let placement = place_edge_label(EdgeLabelInput {
                curve: screen_curve,
                start_node: ctx.meta.canvas_to_screen_pos(start.location()),
                end_node: ctx.meta.canvas_to_screen_pos(end.location()),
                start_radius: screen_curve[0]
                    .distance(ctx.meta.canvas_to_screen_pos(start.location())),
                end_radius: screen_curve[3].distance(ctx.meta.canvas_to_screen_pos(end.location())),
                text_size: galley.rect.size(),
                padding: Vec2::splat(3.0),
                gap: self.style.label.gap,
                stroke_width,
            });
            record_edge_label(ctx.ctx, placement.background, screen_curve);
            let label_painter = ctx.ctx.layer_painter(LayerId::new(
                Order::Foreground,
                Id::new("ploke-egui.edge-labels"),
            ));
            label_painter.add(Shape::rect_filled(
                placement.background,
                2.0,
                Color32::from_black_alpha(180),
            ));
            label_painter.add(TextShape::new(placement.text_pos, galley, color));
        }

        shapes
    }

    fn update(&mut self, state: &egui_graphs::EdgeProps<GraphEdgePayload>) {
        if self.label.as_ref() != state.payload.label.as_ref() {
            self.label = state.payload.label.clone();
            self.curve.set(None);
            self.label_galley = None;
        }
        self.label_visible = state.payload.label_visible;
        if self.style.label != state.payload.style.label || self.color != state.payload.color {
            self.label_galley = None;
        }
        self.color = state.payload.color;
        self.selected = state.selected;
        self.visible = state.payload.visible();
        self.style = state.payload.style;
    }

    fn is_inside(
        &self,
        start: &egui_graphs::Node<N, GraphEdgePayload, Ty, Ix, D>,
        end: &egui_graphs::Node<N, GraphEdgePayload, Ty, Ix, D>,
        pos: Pos2,
    ) -> bool {
        if !self.visible {
            return false;
        }
        distance_to_curve(self.curve_for_nodes(start, end), pos, self.style)
            <= self.style.hit_tolerance
    }

    fn extra_bounds(
        &self,
        start: &egui_graphs::Node<N, GraphEdgePayload, Ty, Ix, D>,
        end: &egui_graphs::Node<N, GraphEdgePayload, Ty, Ix, D>,
    ) -> Option<(Pos2, Pos2)> {
        if !self.visible {
            return None;
        }
        let curve = self.curve_for_nodes(start, end);
        let min = Pos2::new(
            curve.iter().map(|point| point.x).fold(f32::MAX, f32::min),
            curve.iter().map(|point| point.y).fold(f32::MAX, f32::min),
        );
        let max = Pos2::new(
            curve.iter().map(|point| point.x).fold(f32::MIN, f32::max),
            curve.iter().map(|point| point.y).fold(f32::MIN, f32::max),
        );
        Some((min, max))
    }
}

fn node_radius<N, Ty, Ix, D>(node: &egui_graphs::Node<N, GraphEdgePayload, Ty, Ix, D>) -> f32
where
    N: Clone,
    Ty: EdgeType,
    Ix: IndexType,
    D: egui_graphs::DisplayNode<N, GraphEdgePayload, Ty, Ix>,
{
    node.display()
        .closest_boundary_point(Vec2::X)
        .distance(node.location())
        .max(1.0)
}

fn attachment_points<N, Ty, Ix, D>(
    start: &egui_graphs::Node<N, GraphEdgePayload, Ty, Ix, D>,
    end: &egui_graphs::Node<N, GraphEdgePayload, Ty, Ix, D>,
    style: CurveStyle,
) -> (Pos2, Pos2)
where
    N: Clone,
    Ty: EdgeType,
    Ix: IndexType,
    D: egui_graphs::DisplayNode<N, GraphEdgePayload, Ty, Ix>,
{
    let delta = end.location() - start.location();
    let rank_direction = if delta.y >= 0.0 { 1.0 } else { -1.0 };
    let lateral_direction = delta.x.signum();
    let rank_distance = delta.y.abs().max(1.0);
    let lateral_fraction = (delta.x.abs() / rank_distance).min(1.0);

    let start_direction =
        endpoint_direction(rank_direction, lateral_direction, lateral_fraction, style);
    let end_direction =
        endpoint_direction(-rank_direction, -lateral_direction, lateral_fraction, style);

    (
        start.display().closest_boundary_point(start_direction),
        end.display().closest_boundary_point(end_direction),
    )
}
