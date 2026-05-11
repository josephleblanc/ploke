use eframe::egui::{Context, Id, Pos2, Rect, Vec2};

use super::EdgeLabelDiagnostics;
use super::geometry::{cubic_point, cubic_tangent, segments_intersect};

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
                if !hits_endpoint(background, input) && !Curve(input.curve).intersects(background) {
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
        frame.labels.push(rect);
        frame.edges.push(curve);
    });
}

pub(super) fn edge_label_diagnostics(ctx: &Context) -> EdgeLabelDiagnostics {
    let frame = ctx.data_mut(|data| data.get_temp::<EdgeLabelFrame>(label_frame_id()));
    let Some(frame) = frame else {
        return EdgeLabelDiagnostics::default();
    };

    EdgeLabelDiagnostics {
        label_count: frame.labels.len(),
        collision_count: label_collision_count(&frame.labels),
        edge_intersection_count: edge_intersection_count(&frame),
        edge_collision_count: 0,
    }
}

#[derive(Debug, Clone, Default)]
struct EdgeLabelFrame {
    labels: Vec<Rect>,
    edges: Vec<[Pos2; 4]>,
}

fn label_frame_id() -> Id {
    Id::new(EDGE_LABEL_FRAME_ID)
}

fn label_collision_count(rects: &[Rect]) -> usize {
    let mut count = 0;
    for (index, rect) in rects.iter().enumerate() {
        for other in rects.iter().skip(index + 1) {
            if rect.intersects(*other) {
                count += 1;
            }
        }
    }
    count
}

fn edge_intersection_count(frame: &EdgeLabelFrame) -> usize {
    let mut count = 0;
    for rect in &frame.labels {
        for curve in &frame.edges {
            if Curve(*curve).intersects(*rect) {
                count += 1;
            }
        }
    }
    count
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

trait Intersects<T> {
    fn intersects(&self, target: T) -> bool;
}

#[derive(Debug, Clone, Copy)]
struct Curve([Pos2; 4]);

#[derive(Debug, Clone, Copy)]
struct Segment {
    start: Pos2,
    end: Pos2,
}

impl Intersects<Rect> for Curve {
    fn intersects(&self, target: Rect) -> bool {
        let mut previous = self.0[0];
        for step in 1..=COLLISION_SEGMENTS {
            let current = cubic_point(self.0, step as f32 / COLLISION_SEGMENTS as f32);
            if (Segment {
                start: previous,
                end: current,
            })
            .intersects(target)
            {
                return true;
            }
            previous = current;
        }
        false
    }
}

impl Intersects<Rect> for Segment {
    fn intersects(&self, target: Rect) -> bool {
        if target.contains(self.start) || target.contains(self.end) {
            return true;
        }

        let top_left = target.left_top();
        let top_right = target.right_top();
        let bottom_right = target.right_bottom();
        let bottom_left = target.left_bottom();

        segments_intersect(self.start, self.end, top_left, top_right)
            || segments_intersect(self.start, self.end, top_right, bottom_right)
            || segments_intersect(self.start, self.end, bottom_right, bottom_left)
            || segments_intersect(self.start, self.end, bottom_left, top_left)
    }
}
