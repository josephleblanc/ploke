use eframe::egui::{Pos2, Rect, Vec2};
use petgraph::{
    stable_graph::NodeIndex,
    visit::{EdgeRef, IntoEdgeReferences},
};
use ploke_records::branch::TreatmentBranchStatus;

use crate::graph::{EdgeKind, Graph};

use super::geometry::{cubic_point, curve_points, segments_intersect};
use super::projection::WidgetGraph;
use super::style::ViewStyle;
use super::{
    EdgeCrossingsByKind, EdgeLabelDiagnostics, GraphReadabilityDiagnostics, GraphViewDiagnostics,
};

const LONG_EDGE_MEDIAN_MULTIPLE: f32 = 2.0;

pub(super) fn graph_diagnostics(
    semantic_graph: &Graph,
    graph: &WidgetGraph,
    viewport_size: Vec2,
    style: ViewStyle,
    edge_labels: EdgeLabelDiagnostics,
) -> Option<GraphViewDiagnostics> {
    let bounds = node_bounds(graph)?;
    let graph_size = bounds.size();
    let viewport_size = Vec2::new(viewport_size.x.max(1.0), viewport_size.y.max(1.0));
    let graph_size = Vec2::new(graph_size.x.max(1.0), graph_size.y.max(1.0));
    let padded = graph_size * (1.0 + style.layout.fit_padding);
    let zoom = (viewport_size.x / padded.x).min(viewport_size.y / padded.y);
    let fitted = fit_bounds(bounds, viewport_size, zoom);
    let fitted_size = fitted.size();
    let fitted_fill = Vec2::new(
        fitted_size.x / viewport_size.x,
        fitted_size.y / viewport_size.y,
    );

    Some(GraphViewDiagnostics {
        node_count: graph.g().node_count(),
        graph_size,
        viewport_size,
        aspect_ratio: graph_size.x / graph_size.y,
        viewport_aspect_ratio: viewport_size.x / viewport_size.y,
        fitted_size,
        fitted_fill,
        center_offset: fitted.center() - Rect::from_min_size(Pos2::ZERO, viewport_size).center(),
        edge_labels,
        readability: readability_diagnostics(semantic_graph, graph, style),
    })
}

fn readability_diagnostics<'a>(
    semantic_graph: &'a Graph,
    graph: &WidgetGraph,
    style: ViewStyle,
) -> GraphReadabilityDiagnostics {
    let edges = edge_curves(semantic_graph, graph, style);
    let mut crossings = GraphReadabilityDiagnostics {
        long_edge_count: long_edge_count(&edges),
        backtracking_edge_count: backtracking_edge_count(&edges),
        ..GraphReadabilityDiagnostics::default()
    };

    for (index, edge) in edges.iter().enumerate() {
        for other in edges.iter().skip(index + 1) {
            if edge.shares_endpoint(other) || !curves_cross(edge, other, style) {
                continue;
            }

            crossings.edge_edge_crossings += 1;
            crossings
                .edge_crossings_by_kind
                .record(edge.kind, other.kind);
            if edge.is_selected_path() || other.is_selected_path() {
                crossings.selected_path_crossings += 1;
            }
        }
    }

    crossings
}

#[derive(Debug, Clone, Copy)]
struct EdgeCurve<'a> {
    source: NodeIndex,
    target: NodeIndex,
    kind: &'a EdgeKind,
    status: TreatmentBranchStatus,
    points: [Pos2; 4],
}

impl EdgeCurve<'_> {
    fn shares_endpoint(self, other: &Self) -> bool {
        self.source == other.source
            || self.source == other.target
            || self.target == other.source
            || self.target == other.target
    }

    fn is_selected_path(self) -> bool {
        matches!(self.kind, EdgeKind::HistorySuccession { .. })
            || self.status == TreatmentBranchStatus::Selected
    }

    fn chord_length(self) -> f32 {
        self.points[0].distance(self.points[3])
    }

    fn backtracks(self) -> bool {
        self.points[3].y <= self.points[0].y + 1.0
    }
}

fn edge_curves<'a>(
    semantic_graph: &'a Graph,
    graph: &WidgetGraph,
    style: ViewStyle,
) -> Vec<EdgeCurve<'a>> {
    graph
        .g()
        .edge_references()
        .filter_map(|edge| {
            let semantic_edge = semantic_graph.edge(&edge.weight().payload().id)?;
            let start = graph.g().node_weight(edge.source())?.location();
            let end = graph.g().node_weight(edge.target())?.location();
            if start == end {
                return None;
            }

            Some(EdgeCurve {
                source: edge.source(),
                target: edge.target(),
                kind: semantic_edge.kind(),
                status: semantic_edge.status(),
                points: curve_points(start, end, style.edge.curve),
            })
        })
        .collect()
}

fn long_edge_count(edges: &[EdgeCurve<'_>]) -> usize {
    let Some(median) = median_edge_length(edges) else {
        return 0;
    };
    let threshold = median * LONG_EDGE_MEDIAN_MULTIPLE;
    edges
        .iter()
        .filter(|edge| edge.chord_length() > threshold)
        .count()
}

fn median_edge_length(edges: &[EdgeCurve<'_>]) -> Option<f32> {
    if edges.is_empty() {
        return None;
    }

    let mut lengths = edges
        .iter()
        .map(|edge| edge.chord_length())
        .collect::<Vec<_>>();
    lengths.sort_by(|left, right| left.total_cmp(right));
    Some(lengths[lengths.len() / 2])
}

fn backtracking_edge_count(edges: &[EdgeCurve<'_>]) -> usize {
    edges.iter().filter(|edge| edge.backtracks()).count()
}

fn curves_cross(left: &EdgeCurve<'_>, right: &EdgeCurve<'_>, style: ViewStyle) -> bool {
    let segments = style.edge.curve.hit_segments.max(1);
    let mut left_start = left.points[0];
    for left_step in 1..=segments {
        let left_end = cubic_point(left.points, left_step as f32 / segments as f32);
        let mut right_start = right.points[0];
        for right_step in 1..=segments {
            let right_end = cubic_point(right.points, right_step as f32 / segments as f32);
            if segments_intersect(left_start, left_end, right_start, right_end) {
                return true;
            }
            right_start = right_end;
        }
        left_start = left_end;
    }
    false
}

fn node_bounds(graph: &WidgetGraph) -> Option<Rect> {
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

    Some(Rect::from_min_max(
        min - Vec2::splat(1.0),
        max + Vec2::splat(1.0),
    ))
}

fn fit_bounds(bounds: Rect, viewport_size: Vec2, zoom: f32) -> Rect {
    let viewport = Rect::from_min_size(Pos2::ZERO, viewport_size);
    let center = bounds.center().to_vec2();
    let pan = viewport.center().to_vec2() - center * zoom;
    Rect::from_min_max(
        (bounds.min.to_vec2() * zoom + pan).to_pos2(),
        (bounds.max.to_vec2() * zoom + pan).to_pos2(),
    )
}

impl EdgeCrossingsByKind {
    fn record(&mut self, left: &EdgeKind, right: &EdgeKind) {
        match (left, right) {
            (EdgeKind::CandidateTransition, EdgeKind::CandidateTransition) => {
                self.candidate_candidate += 1;
            }
            (EdgeKind::HistorySuccession { .. }, EdgeKind::HistorySuccession { .. }) => {
                self.history_history += 1;
            }
            (EdgeKind::CandidateTransition, EdgeKind::HistorySuccession { .. })
            | (EdgeKind::HistorySuccession { .. }, EdgeKind::CandidateTransition) => {
                self.candidate_history += 1;
            }
            (EdgeKind::ArtifactPatch { .. }, _) | (_, EdgeKind::ArtifactPatch { .. }) => {}
        }
    }
}
