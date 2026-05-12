use eframe::egui::{Context, Id, Pos2, Rect, Vec2};

use super::EdgeLabelDiagnostics;
use super::geometry::{cubic_point, cubic_tangent, curve_intersects_rect};

const CANDIDATE_T: [f32; 7] = [0.50, 0.42, 0.58, 0.34, 0.66, 0.26, 0.74];
const CLEARANCE_SCALE: [f32; 4] = [1.0, 1.35, 1.7, 2.1];
const EDGE_LABEL_FRAME_ID: &str = "ploke-egui.edge-label-frame";
const COLLISION_SEGMENTS: usize = 16;

#[derive(Debug, Clone, Copy)]
pub(super) struct EdgeLabelPlacement {
    pub text_pos: Pos2,
    pub background: Rect,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct EdgeLabelInput {
    pub curve: [Pos2; 4],
    pub start_node: Pos2,
    pub end_node: Pos2,
    pub start_radius: f32,
    pub end_radius: f32,
    pub text_size: Vec2,
    pub padding: Vec2,
    pub gap: f32,
    pub stroke_width: f32,
}

pub(super) fn place_edge_label(input: EdgeLabelInput) -> EdgeLabelPlacement {
    let label_size = input.text_size + input.padding * 2.0;
    let endpoint_t = endpoint_clearance(
        input.curve,
        label_size,
        input.start_radius,
        input.end_radius,
    );
    for t in CANDIDATE_T {
        if t <= endpoint_t || t >= 1.0 - endpoint_t {
            continue;
        }

        let preferred_side = preferred_side(input.curve, t);
        for side in [preferred_side, -preferred_side] {
            for clearance_scale in CLEARANCE_SCALE {
                let background = label_rect(
                    input.curve,
                    t,
                    side,
                    label_size,
                    input.gap,
                    input.stroke_width,
                    clearance_scale,
                );
                if !hits_endpoint(background, input)
                    && !curve_intersects_rect(input.curve, background, COLLISION_SEGMENTS)
                {
                    return EdgeLabelPlacement {
                        text_pos: background.min + input.padding,
                        background,
                    };
                }
            }
        }
    }

    let preferred_side = preferred_side(input.curve, 0.5);
    let background = label_rect(
        input.curve,
        0.5,
        preferred_side,
        label_size,
        input.gap,
        input.stroke_width,
        CLEARANCE_SCALE[CLEARANCE_SCALE.len() - 1],
    );
    EdgeLabelPlacement {
        text_pos: background.min + input.padding,
        background,
    }
}

pub(super) fn reset_edge_label_diagnostics(ctx: &Context) {
    ctx.data_mut(|data| data.insert_temp(label_frame_id(), EdgeLabelFrame::default()));
}

pub(super) fn record_edge_label(ctx: &Context, rect: Rect, curve: [Pos2; 4]) {
    ctx.data_mut(|data| {
        let frame = data.get_temp_mut_or_default::<EdgeLabelFrame>(label_frame_id());
        frame.placements.push(EdgeLabelGeometry { rect, curve });
    });
}

pub(super) fn edge_label_diagnostics(ctx: &Context) -> EdgeLabelDiagnostics {
    let frame = ctx.data_mut(|data| data.get_temp::<EdgeLabelFrame>(label_frame_id()));
    let Some(frame) = frame else {
        return EdgeLabelDiagnostics::default();
    };

    EdgeLabelDiagnostics {
        label_count: frame.placements.len(),
        collision_count: label_collision_count(&frame.placements),
        edge_intersection_count: edge_intersection_count(&frame),
        edge_collision_count: edge_collision_count(&frame),
    }
}

#[derive(Debug, Clone, Default)]
struct EdgeLabelFrame {
    placements: Vec<EdgeLabelGeometry>,
}

#[derive(Debug, Clone, Copy)]
struct EdgeLabelGeometry {
    rect: Rect,
    curve: [Pos2; 4],
}

fn label_frame_id() -> Id {
    Id::new(EDGE_LABEL_FRAME_ID)
}

fn label_collision_count(placements: &[EdgeLabelGeometry]) -> usize {
    let mut count = 0;
    for (index, placement) in placements.iter().enumerate() {
        for other in placements.iter().skip(index + 1) {
            if placement.rect.intersects(other.rect) {
                count += 1;
            }
        }
    }
    count
}

fn edge_intersection_count(frame: &EdgeLabelFrame) -> usize {
    let mut count = 0;
    for (index, placement) in frame.placements.iter().enumerate() {
        for other in frame.placements.iter().enumerate() {
            if index == other.0 {
                continue;
            }
            if curve_intersects_rect(other.1.curve, placement.rect, COLLISION_SEGMENTS) {
                count += 1;
            }
        }
    }
    count
}

fn edge_collision_count(frame: &EdgeLabelFrame) -> usize {
    frame
        .placements
        .iter()
        .filter(|placement| {
            curve_intersects_rect(placement.curve, placement.rect, COLLISION_SEGMENTS)
        })
        .count()
}

fn preferred_side(curve: [Pos2; 4], t: f32) -> f32 {
    let lateral = curve[3].x - curve[0].x;
    let normal_x = curve_normal(curve, t).x;
    if lateral.abs() <= f32::EPSILON || normal_x.abs() <= f32::EPSILON {
        return 1.0;
    }

    lateral.signum() * normal_x.signum()
}

fn endpoint_clearance(
    curve: [Pos2; 4],
    label_size: Vec2,
    start_radius: f32,
    end_radius: f32,
) -> f32 {
    let edge_length = curve[0].distance(curve[3]).max(1.0);
    let node_clearance = start_radius.max(end_radius) + label_size.x.max(label_size.y) * 0.5 + 6.0;
    (node_clearance / edge_length).min(0.45)
}

fn label_rect(
    curve: [Pos2; 4],
    t: f32,
    side: f32,
    size: Vec2,
    gap: f32,
    stroke_width: f32,
    clearance_scale: f32,
) -> Rect {
    let anchor = cubic_point(curve, t);
    let normal = curve_normal(curve, t);
    let clearance = (rect_extent_along(size, normal) + gap + stroke_width) * clearance_scale;
    Rect::from_center_size(anchor + normal * clearance * side, size)
}

fn rect_extent_along(size: Vec2, direction: Vec2) -> f32 {
    size.x * 0.5 * direction.x.abs() + size.y * 0.5 * direction.y.abs()
}

fn curve_normal(curve: [Pos2; 4], t: f32) -> Vec2 {
    let tangent = usable_tangent(curve, t);
    Vec2::new(-tangent.y, tangent.x).normalized()
}

fn usable_tangent(curve: [Pos2; 4], t: f32) -> Vec2 {
    let tangent = cubic_tangent(curve, t);
    if tangent.length_sq() > f32::EPSILON {
        tangent
    } else {
        curve[3] - curve[0]
    }
}

fn hits_endpoint(rect: Rect, input: EdgeLabelInput) -> bool {
    let radius = input.start_radius + input.gap;
    let start = Rect::from_center_size(input.start_node, Vec2::splat(radius * 2.0));
    if rect.intersects(start) {
        return true;
    }

    let radius = input.end_radius + input.gap;
    let end = Rect::from_center_size(input.end_node, Vec2::splat(radius * 2.0));
    rect.intersects(end)
}
