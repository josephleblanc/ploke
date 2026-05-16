use eframe::egui::{Pos2, Rect, Vec2};
use petgraph::{
    stable_graph::NodeIndex,
    visit::{EdgeRef, IntoEdgeReferences},
};

use super::geometry::{cubic_point, curve_points, segments_intersect, self_loop_points};
use super::projection::{GraphEdgePayload, ViewEdgeKind, WidgetGraph};
use super::style::ViewStyle;
use super::{
    EdgeCrossingsByKind, EdgeLabelDiagnostics, GraphConnectivityDiagnostics,
    GraphReadabilityDiagnostics, GraphViewDiagnostics, GraphViewMode, artifact_tree,
};

const LONG_EDGE_MEDIAN_MULTIPLE: f32 = 2.0;
const RANK_SPACING_MEDIAN_MULTIPLE: f32 = 2.0;

pub(super) fn graph_diagnostics(
    graph: &WidgetGraph,
    viewport_size: Vec2,
    style: ViewStyle,
    edge_labels: EdgeLabelDiagnostics,
    connectivity: GraphConnectivityDiagnostics,
    artifact_tree: artifact_tree::Shape,
    mode: GraphViewMode,
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
        mode,
        node_count: graph
            .g()
            .node_weights()
            .filter(|node| node.payload().visible())
            .count(),
        edge_count: graph
            .g()
            .edge_weights()
            .filter(|edge| edge.payload().visible())
            .count(),
        connectivity,
        artifact_tree,
        graph_size,
        viewport_size,
        aspect_ratio: graph_size.x / graph_size.y,
        viewport_aspect_ratio: viewport_size.x / viewport_size.y,
        fitted_size,
        fitted_fill,
        center_offset: fitted.center() - Rect::from_min_size(Pos2::ZERO, viewport_size).center(),
        edge_labels,
        readability: readability_diagnostics(graph, style),
    })
}

fn readability_diagnostics(graph: &WidgetGraph, style: ViewStyle) -> GraphReadabilityDiagnostics {
    let edges = edge_curves(graph, style);
    let median_rank_gap = median_node_rank_gap(graph);
    let mut crossings = GraphReadabilityDiagnostics {
        long_edge_count: long_edge_count(&edges, median_rank_gap),
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
struct EdgeCurve {
    source: NodeIndex,
    target: NodeIndex,
    kind: ViewEdgeKind,
    salient: bool,
    points: [Pos2; 4],
}

impl EdgeCurve {
    fn shares_endpoint(self, other: &Self) -> bool {
        self.source == other.source
            || self.source == other.target
            || self.target == other.source
            || self.target == other.target
    }

    fn is_selected_path(self) -> bool {
        self.salient
    }

    fn chord_length(self) -> f32 {
        self.points[0].distance(self.points[3])
    }

    fn rank_distance(self) -> f32 {
        (self.points[3].y - self.points[0].y).abs()
    }

    fn backtracks(self) -> bool {
        self.points[3].y <= self.points[0].y + 1.0
    }
}

/// archaeology:artifact-relations
/// proof:docs/active/archaeology/ploke-tree-graph/artifact-relations.md
fn edge_curves(graph: &WidgetGraph, style: ViewStyle) -> Vec<EdgeCurve> {
    graph
        .g()
        .edge_references()
        .filter_map(|edge| {
            let payload: &GraphEdgePayload = edge.weight().payload();
            if !payload.visible() {
                return None;
            }
            let source = graph.g().node_weight(edge.source())?;
            let target = graph.g().node_weight(edge.target())?;
            if !source.payload().visible() || !target.payload().visible() {
                return None;
            }
            let start = source.location();
            let end = target.location();
            let points = if start == end {
                self_loop_points(start, style.layout.node_radius, style.edge.curve)
            } else {
                curve_points(start, end, style.edge.curve)
            };

            Some(EdgeCurve {
                source: edge.source(),
                target: edge.target(),
                kind: payload.kind,
                salient: payload.color == style.edge.colors.selected,
                points,
            })
        })
        .collect()
}

fn long_edge_count(edges: &[EdgeCurve], median_rank_gap: Option<f32>) -> usize {
    let Some(edge_median) = median_edge_length(edges) else {
        return 0;
    };
    let edge_threshold = edge_median * LONG_EDGE_MEDIAN_MULTIPLE;
    let rank_threshold = median_rank_gap
        .or_else(|| median_rank_distance(edges))
        .map(|median| median * RANK_SPACING_MEDIAN_MULTIPLE);
    edges
        .iter()
        .filter(|edge| {
            edge.chord_length() > edge_threshold
                || rank_threshold.is_some_and(|threshold| edge.rank_distance() > threshold)
        })
        .count()
}

fn median_node_rank_gap(graph: &WidgetGraph) -> Option<f32> {
    let mut ranks = graph
        .g()
        .node_weights()
        .filter(|node| node.payload().visible())
        .map(|node| node.location().y)
        .filter(|rank| rank.is_finite())
        .collect::<Vec<_>>();
    if ranks.len() < 2 {
        return None;
    }

    ranks.sort_by(|left, right| left.total_cmp(right));
    ranks.dedup_by(|left, right| (*left - *right).abs() <= 1.0);
    let mut gaps = ranks
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .filter(|gap| *gap > 1.0)
        .collect::<Vec<_>>();
    if gaps.is_empty() {
        return None;
    }

    gaps.sort_by(|left, right| left.total_cmp(right));
    Some(gaps[gaps.len() / 2])
}

fn median_edge_length(edges: &[EdgeCurve]) -> Option<f32> {
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

fn median_rank_distance(edges: &[EdgeCurve]) -> Option<f32> {
    let mut distances = edges
        .iter()
        .map(|edge| edge.rank_distance())
        .filter(|distance| *distance > 1.0)
        .collect::<Vec<_>>();
    if distances.is_empty() {
        return None;
    }

    distances.sort_by(|left, right| left.total_cmp(right));
    Some(distances[distances.len() / 2])
}

fn backtracking_edge_count(edges: &[EdgeCurve]) -> usize {
    edges.iter().filter(|edge| edge.backtracks()).count()
}

fn curves_cross(left: &EdgeCurve, right: &EdgeCurve, style: ViewStyle) -> bool {
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
    let mut nodes = graph
        .g()
        .node_weights()
        .filter(|node| node.payload().visible());
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
    fn record(&mut self, left: ViewEdgeKind, right: ViewEdgeKind) {
        #[cfg(not(test))]
        {
            let _ = (left, right);
            self.artifact_artifact += 1;
        }

        #[cfg(test)]
        match (left, right) {
            (
                ViewEdgeKind::HistoryArtifact
                | ViewEdgeKind::HistoryOpenedFrom
                | ViewEdgeKind::ArtifactPatch,
                ViewEdgeKind::HistoryArtifact
                | ViewEdgeKind::HistoryOpenedFrom
                | ViewEdgeKind::ArtifactPatch,
            ) => {
                self.artifact_artifact += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Color32, Pos2};
    use petgraph::{Directed, stable_graph::StableGraph};
    use std::sync::Arc;

    use super::*;
    use crate::ui::view::GraphSelectionRef;
    use crate::ui::view::projection::{GraphLayerMask, GraphNode};

    #[test]
    fn selected_path_crossings_ignore_unselected_artifact_patch_edges() {
        let style = ViewStyle::default();
        let graph = crossing_graph(
            edge("P1", ViewEdgeKind::ArtifactPatch, Color32::WHITE, style),
            edge("P2", ViewEdgeKind::ArtifactPatch, Color32::WHITE, style),
        );

        let diagnostics = readability_diagnostics(&graph, ViewStyle::default());

        assert_eq!(diagnostics.edge_edge_crossings, 1);
        assert_eq!(diagnostics.selected_path_crossings, 0);
    }

    #[test]
    fn selected_path_crossings_follow_history_edge_render_emphasis() {
        let style = ViewStyle::default();
        let graph = crossing_graph(
            edge(
                "P1",
                ViewEdgeKind::HistoryArtifact,
                style.edge.colors.selected,
                style,
            ),
            edge("P2", ViewEdgeKind::ArtifactPatch, Color32::WHITE, style),
        );

        let diagnostics = readability_diagnostics(&graph, style);

        assert_eq!(diagnostics.edge_edge_crossings, 1);
        assert_eq!(diagnostics.selected_path_crossings, 1);
    }

    #[test]
    fn selected_path_crossings_follow_selected_patch_render_emphasis() {
        let style = ViewStyle::default();
        let graph = crossing_graph(
            edge(
                "P1",
                ViewEdgeKind::ArtifactPatch,
                style.edge.colors.selected,
                style,
            ),
            edge("P2", ViewEdgeKind::ArtifactPatch, Color32::WHITE, style),
        );

        let diagnostics = readability_diagnostics(&graph, style);

        assert_eq!(diagnostics.edge_edge_crossings, 1);
        assert_eq!(diagnostics.selected_path_crossings, 1);
    }

    #[test]
    fn long_edge_count_includes_rank_spacing_outliers() {
        let edges = [
            test_edge(0, Pos2::new(0.0, 0.0), Pos2::new(200.0, 50.0)),
            test_edge(1, Pos2::new(0.0, 0.0), Pos2::new(-200.0, 50.0)),
            test_edge(2, Pos2::new(0.0, 0.0), Pos2::new(0.0, 120.0)),
        ];

        assert_eq!(long_edge_count(&edges, Some(50.0)), 1);
    }

    #[test]
    fn edge_curves_include_self_loops() {
        let style = ViewStyle::default();
        let mut raw = StableGraph::<GraphNode, GraphEdgePayload, Directed>::default();
        let node = raw.add_node(node("loop"));
        raw.add_edge(
            node,
            node,
            edge(
                "P1",
                ViewEdgeKind::HistoryOpenedFrom,
                style.edge.colors.opened_from,
                style,
            ),
        );

        let mut graph: WidgetGraph = egui_graphs::to_graph_custom(
            &raw,
            |node| {
                node.set_label(String::new());
            },
            |_edge| {},
        );
        graph.g_mut()[node].set_location(Pos2::new(0.0, 0.0));

        let curves = edge_curves(&graph, style);

        assert_eq!(curves.len(), 1);
    }

    fn crossing_graph(left: GraphEdgePayload, right: GraphEdgePayload) -> WidgetGraph {
        let mut raw = StableGraph::<GraphNode, GraphEdgePayload, Directed>::default();
        let left_start = raw.add_node(node("left-start"));
        let left_end = raw.add_node(node("left-end"));
        let right_start = raw.add_node(node("right-start"));
        let right_end = raw.add_node(node("right-end"));

        raw.add_edge(left_start, left_end, left);
        raw.add_edge(right_start, right_end, right);

        let mut graph: WidgetGraph = egui_graphs::to_graph_custom(
            &raw,
            |node| {
                node.set_label(String::new());
            },
            |_edge| {},
        );
        graph.g_mut()[left_start].set_location(Pos2::new(0.0, 0.0));
        graph.g_mut()[left_end].set_location(Pos2::new(100.0, 100.0));
        graph.g_mut()[right_start].set_location(Pos2::new(100.0, 0.0));
        graph.g_mut()[right_end].set_location(Pos2::new(0.0, 100.0));
        graph
    }

    fn node(label: &'static str) -> GraphNode {
        GraphNode::Artifact {
            label: Arc::from(label),
            detail: Arc::from(label),
            reference: GraphSelectionRef::Artifact {
                key: label.to_owned(),
            },
            color: Color32::WHITE,
            layers: GraphLayerMask::ARTIFACT,
            visible: true,
        }
    }

    fn edge(
        label: &'static str,
        kind: ViewEdgeKind,
        color: Color32,
        style: ViewStyle,
    ) -> GraphEdgePayload {
        GraphEdgePayload {
            label: Arc::from(label),
            label_visible: true,
            color,
            style: style.edge,
            kind,
            layers: GraphLayerMask::ARTIFACT,
            visible: true,
        }
    }

    fn test_edge(index: usize, start: Pos2, end: Pos2) -> EdgeCurve {
        EdgeCurve {
            source: NodeIndex::new(index * 2),
            target: NodeIndex::new(index * 2 + 1),
            kind: ViewEdgeKind::ArtifactPatch,
            salient: false,
            points: [start, start, end, end],
        }
    }
}
