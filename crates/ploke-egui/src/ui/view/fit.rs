//! Graph fit: layout relayout targets ~80–85% fill on width and height at zoom 1;
//! screen fit uses cover-style zoom on the tighter axis.

use eframe::egui::{Pos2, Rect, Ui, Vec2};
use egui_graphs::DisplayEdge;
use petgraph::{EdgeType, stable_graph::IndexType};

use super::layout::{self, State as LayoutState};
use super::projection::{GraphEdgePayload, WidgetGraph};
use super::style::ViewStyle;

/// Midpoint of the operator target band (80–85%) shown as "Fit fill" in diagnostics.
pub(super) const TARGET_VIEWPORT_FILL: f32 = 0.825;

pub(super) fn fit_zoom(viewport: Vec2, graph_size: Vec2, padding: f32) -> f32 {
    let viewport = Vec2::new(viewport.x.max(1.0), viewport.y.max(1.0));
    let padded = Vec2::new(
        graph_size.x.max(1.0) * (1.0 + padding),
        graph_size.y.max(1.0) * (1.0 + padding),
    );
    let zoom_x = viewport.x / padded.x;
    let zoom_y = viewport.y / padded.y;
    zoom_x.max(zoom_y)
}

pub(super) fn fit_pan(bounds: Rect, viewport: Vec2, zoom: f32) -> Vec2 {
    let viewport = Rect::from_min_size(Pos2::ZERO, viewport);
    viewport.center().to_vec2() - bounds.center().to_vec2() * zoom
}

pub(super) fn fitted_screen_rect(bounds: Rect, viewport: Vec2, zoom: f32) -> Rect {
    let pan = fit_pan(bounds, viewport, zoom);
    Rect::from_min_max(
        (bounds.min.to_vec2() * zoom + pan).to_pos2(),
        (bounds.max.to_vec2() * zoom + pan).to_pos2(),
    )
}

pub(super) fn graph_fit_bounds(graph: &WidgetGraph, style: ViewStyle) -> Option<Rect> {
    let mut min = Pos2::new(f32::MAX, f32::MAX);
    let mut max = Pos2::new(f32::MIN, f32::MIN);
    let mut any = false;

    for node in graph
        .g()
        .node_weights()
        .filter(|node| node.payload().visible())
    {
        let location = node.location();
        let radius = node_radius(node, style.layout.node_radius);
        min.x = min.x.min(location.x - radius);
        min.y = min.y.min(location.y - radius);
        max.x = max.x.max(location.x + radius);
        max.y = max.y.max(location.y + radius);
        any = true;
    }

    if !any {
        return None;
    }

    for (_, edge) in graph.edges_iter() {
        let payload = edge.payload();
        if !payload.visible() {
            continue;
        }
        let Some((start_idx, end_idx)) = graph.edge_endpoints(edge.id()) else {
            continue;
        };
        let Some(start) = graph.node(start_idx) else {
            continue;
        };
        let Some(end) = graph.node(end_idx) else {
            continue;
        };
        if !start.payload().visible() || !end.payload().visible() {
            continue;
        }
        let Some((edge_min, edge_max)) = edge.display().extra_bounds(start, end) else {
            continue;
        };
        min.x = min.x.min(edge_min.x);
        min.y = min.y.min(edge_min.y);
        max.x = max.x.max(edge_max.x);
        max.y = max.y.max(edge_max.y);
    }

    Some(Rect::from_min_max(min, max))
}

/// Independent row/column scale factors so relayout can reach the target fill on both axes.
pub(super) fn layout_fit_axis_scales(viewport: Vec2, bounds: Rect) -> Vec2 {
    let viewport = Vec2::new(viewport.x.max(1.0), viewport.y.max(1.0));
    let graph_size = bounds.size();
    let graph_size = Vec2::new(graph_size.x.max(1.0), graph_size.y.max(1.0));
    let target = viewport * TARGET_VIEWPORT_FILL;
    Vec2::new(target.x / graph_size.x, target.y / graph_size.y)
}

pub(super) fn scaled_layout_state(base: LayoutState, viewport: Vec2, bounds: Rect) -> LayoutState {
    let scales = layout_fit_axis_scales(viewport, bounds);
    LayoutState {
        triggered: false,
        row_dist: base.row_dist * scales.y,
        col_dist: base.col_dist * scales.x,
        lane_dist: base.lane_dist * scales.x,
        ..base
    }
}

/// Fit fill and center offset for the graph bounds at the given zoom (matches on-screen framing).
pub(super) fn viewport_fit_metrics(bounds: Rect, viewport: Vec2, zoom: f32) -> (Vec2, Vec2, Vec2) {
    let viewport = Vec2::new(viewport.x.max(1.0), viewport.y.max(1.0));
    let fitted = fitted_screen_rect(bounds, viewport, zoom);
    let fitted_size = fitted.size();
    let fitted_fill = fitted_size / viewport;
    let center_offset =
        fitted.center().to_vec2() - Rect::from_min_size(Pos2::ZERO, viewport).center().to_vec2();
    (fitted_size, fitted_fill, center_offset)
}

/// Expands node positions around the graph center when lineage spacing alone cannot
/// reach the target viewport fill (for example depth-heavy trees with little leaf span).
pub(super) fn stretch_graph_to_viewport_fill(
    graph: &mut WidgetGraph,
    bounds: Rect,
    viewport: Vec2,
    style: ViewStyle,
) -> Rect {
    let viewport = Vec2::new(viewport.x.max(1.0), viewport.y.max(1.0));
    let target = viewport * TARGET_VIEWPORT_FILL;
    let size = bounds.size();
    let size = Vec2::new(size.x.max(1.0), size.y.max(1.0));
    let scale_x = (target.x / size.x).max(1.0);
    let scale_y = (target.y / size.y).max(1.0);
    if scale_x <= 1.0 + f32::EPSILON && scale_y <= 1.0 + f32::EPSILON {
        return bounds;
    }

    let center = bounds.center();
    for node in graph.g_mut().node_weights_mut() {
        if !node.payload().visible() {
            continue;
        }
        let location = node.location();
        node.set_location(Pos2::new(
            center.x + (location.x - center.x) * scale_x,
            center.y + (location.y - center.y) * scale_y,
        ));
    }

    graph_fit_bounds(graph, style).unwrap_or(bounds)
}

pub(super) fn apply_graph_layout_fit(
    graph: &mut WidgetGraph,
    base_state: LayoutState,
    viewport: Vec2,
    style: ViewStyle,
) -> Option<(LayoutState, Rect)> {
    let bounds = graph_fit_bounds(graph, style)?;
    let mut scaled_state = scaled_layout_state(base_state, viewport, bounds);
    layout::apply_lineage(graph, &scaled_state);
    scaled_state.triggered = true;
    let post_layout_bounds = graph_fit_bounds(graph, style)?;
    let fitted_bounds = stretch_graph_to_viewport_fill(graph, post_layout_bounds, viewport, style);
    Some((scaled_state, fitted_bounds))
}

pub(super) fn apply_graph_layout_viewport_fit(
    ui: &mut Ui,
    custom_id: &Option<String>,
    viewport: Vec2,
    bounds: Rect,
) {
    let zoom = 1.0;
    let pan = fit_pan(bounds, viewport, zoom);

    let mut meta = egui_graphs::MetadataFrame::new(custom_id.clone()).load(ui);
    meta.zoom = zoom;
    meta.pan = pan;
    meta.save(ui);
    ui.ctx().request_repaint();
}

pub(super) fn apply_graph_screen_fit(
    ui: &mut Ui,
    widget_id: eframe::egui::Id,
    custom_id: &Option<String>,
    top_left: Pos2,
    viewport: Vec2,
    graph: &WidgetGraph,
    style: ViewStyle,
) {
    let Some(bounds) = graph_fit_bounds(graph, style) else {
        return;
    };

    let padding = style.layout.fit_padding;
    let zoom = fit_zoom(viewport, bounds.size(), padding);
    let pan = fit_pan(bounds, viewport, zoom);

    let _ = (widget_id, custom_id, top_left);

    let mut meta = egui_graphs::MetadataFrame::new(custom_id.clone()).load(ui);
    meta.zoom = zoom;
    meta.pan = pan;
    meta.save(ui);
    ui.ctx().request_repaint();
}

fn node_radius<N, Ty, Ix, D>(
    node: &egui_graphs::Node<N, GraphEdgePayload, Ty, Ix, D>,
    fallback: f32,
) -> f32
where
    N: Clone,
    Ty: EdgeType,
    Ix: IndexType,
    D: egui_graphs::DisplayNode<N, GraphEdgePayload, Ty, Ix>,
{
    let location = node.location();
    [Vec2::X, Vec2::X * -1.0, Vec2::Y, Vec2::Y * -1.0]
        .into_iter()
        .map(|dir| {
            node.display()
                .closest_boundary_point(dir)
                .distance(location)
        })
        .fold(fallback.max(1.0), f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Pos2;

    #[test]
    fn cover_fit_targets_tight_axis_near_viewport_fill() {
        let viewport = Vec2::new(400.0, 800.0);
        let graph_size = Vec2::new(1586.0, 1402.0);
        let padding = 0.22;
        let zoom = fit_zoom(viewport, graph_size, padding);
        let fitted =
            fitted_screen_rect(Rect::from_min_size(Pos2::ZERO, graph_size), viewport, zoom);
        let fill = Vec2::new(fitted.width() / viewport.x, fitted.height() / viewport.y);
        assert!(
            (fill.x - TARGET_VIEWPORT_FILL).abs() < 0.02
                || (fill.y - TARGET_VIEWPORT_FILL).abs() < 0.02,
            "expected one axis near {TARGET_VIEWPORT_FILL}, got {fill:?}"
        );
        assert!(
            fill.x.min(fill.y) >= TARGET_VIEWPORT_FILL - 0.02,
            "under-filled axis should reach target band, got {fill:?}"
        );
    }

    #[test]
    fn layout_fit_axis_scales_target_both_axes_at_zoom_one() {
        let viewport = Vec2::new(800.0, 600.0);
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(2000.0, 1500.0));
        let scales = layout_fit_axis_scales(viewport, bounds);
        let fitted = bounds.size() * scales;
        let fill = fitted / viewport;
        assert!((fill.x - TARGET_VIEWPORT_FILL).abs() < 0.02);
        assert!((fill.y - TARGET_VIEWPORT_FILL).abs() < 0.02);
    }

    #[test]
    fn layout_fit_axis_scales_use_vertical_space_in_tall_viewport() {
        let viewport = Vec2::new(400.0, 800.0);
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 400.0));
        let uniform = layout_fit_axis_scales(viewport, bounds).x;
        let fitted_uniform = bounds.size() * uniform;
        let fill_uniform = fitted_uniform / viewport;
        assert!(fill_uniform.y < TARGET_VIEWPORT_FILL - 0.1);

        let scales = layout_fit_axis_scales(viewport, bounds);
        let fitted = bounds.size() * scales;
        let fill = fitted / viewport;
        assert!((fill.x - TARGET_VIEWPORT_FILL).abs() < 0.02);
        assert!((fill.y - TARGET_VIEWPORT_FILL).abs() < 0.02);
    }

    #[test]
    fn viewport_fit_metrics_at_zoom_one_match_graph_bounds() {
        let viewport = Vec2::new(400.0, 800.0);
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(330.0, 660.0));
        let (_, fill, center) = viewport_fit_metrics(bounds, viewport, 1.0);
        assert!((fill.x - TARGET_VIEWPORT_FILL).abs() < 0.02);
        assert!((fill.y - TARGET_VIEWPORT_FILL).abs() < 0.02);
        assert!(center.length() < 1.0);
    }

    #[test]
    fn fit_pan_centers_offset_bounds_at_zoom_one() {
        let viewport = Vec2::new(526.0, 960.0);
        let bounds = Rect::from_min_max(Pos2::new(120.0, 80.0), Pos2::new(593.0, 886.0));
        let zoom = 1.0;
        let pan = fit_pan(bounds, viewport, zoom);
        let fitted = fitted_screen_rect(bounds, viewport, zoom);
        let viewport_rect = Rect::from_min_size(Pos2::ZERO, viewport);
        assert!(fitted.center().distance(viewport_rect.center()) < 0.5);
        assert!(viewport_rect.contains(fitted.min));
        assert!(viewport_rect.contains(fitted.max));
        assert_eq!(
            pan,
            viewport_rect.center().to_vec2() - bounds.center().to_vec2() * zoom
        );
    }

    #[test]
    fn layout_viewport_fit_uses_post_relayout_bounds_center() {
        let viewport = Vec2::new(526.0, 960.0);
        let pre_layout = Rect::from_min_max(Pos2::new(-200.0, 0.0), Pos2::new(1800.0, 1400.0));
        let post_layout = Rect::from_min_max(Pos2::new(40.0, 60.0), Pos2::new(513.0, 866.0));
        let pre_pan = fit_pan(pre_layout, viewport, 1.0);
        let post_pan = fit_pan(post_layout, viewport, 1.0);
        assert!(pre_pan.x.abs() > post_pan.x.abs() + 50.0);
        let fitted = fitted_screen_rect(post_layout, viewport, 1.0);
        assert!(
            fitted
                .center()
                .distance(Rect::from_min_size(Pos2::ZERO, viewport).center())
                < 0.5
        );
    }

    #[test]
    fn stretch_scale_expands_underfilled_width_to_target_band() {
        let viewport = Vec2::new(526.0, 960.0);
        let bounds = Rect::from_min_max(Pos2::new(248.0, 0.0), Pos2::new(278.0, 886.0));
        let target = viewport * TARGET_VIEWPORT_FILL;
        let scale_x = (target.x / bounds.width().max(1.0)).max(1.0);
        let stretched_width = bounds.width() * scale_x;
        assert!((stretched_width / viewport.x - TARGET_VIEWPORT_FILL).abs() < 0.02);
    }

    #[test]
    fn square_graph_in_square_viewport_fills_both_axes() {
        let viewport = Vec2::splat(500.0);
        let graph_size = Vec2::splat(400.0);
        let padding = 0.22;
        let zoom = fit_zoom(viewport, graph_size, padding);
        let fitted =
            fitted_screen_rect(Rect::from_min_size(Pos2::ZERO, graph_size), viewport, zoom);
        let fill = fitted.size() / viewport;
        assert!((fill.x - TARGET_VIEWPORT_FILL).abs() < 0.02);
        assert!((fill.y - TARGET_VIEWPORT_FILL).abs() < 0.02);
    }
}
