use eframe::egui::{Rect, Vec2};

use super::GraphViewDiagnostics;
use super::projection::WidgetGraph;
use super::style::ViewStyle;

pub(super) fn graph_diagnostics(
    graph: &WidgetGraph,
    viewport_size: Vec2,
    style: ViewStyle,
) -> Option<GraphViewDiagnostics> {
    let bounds = node_bounds(graph)?;
    let graph_size = bounds.size();
    let viewport_size = Vec2::new(viewport_size.x.max(1.0), viewport_size.y.max(1.0));
    let graph_size = Vec2::new(graph_size.x.max(1.0), graph_size.y.max(1.0));
    let padded = graph_size * (1.0 + style.layout.fit_padding);
    let zoom = (viewport_size.x / padded.x).min(viewport_size.y / padded.y);
    let fitted_size = graph_size * zoom;
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
        center_offset: Vec2::ZERO,
    })
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
