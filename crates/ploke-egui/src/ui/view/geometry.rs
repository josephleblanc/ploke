use eframe::egui::{Pos2, Rect, Vec2};

use super::style::{CurveStyle, EdgeStyle};

pub(super) fn endpoint_direction(
    rank_direction: f32,
    lateral_direction: f32,
    lateral_fraction: f32,
    style: CurveStyle,
) -> Vec2 {
    let arc_angle = lateral_fraction * style.endpoint_max_arc_fraction;
    Vec2::new(
        lateral_direction * arc_angle.sin(),
        rank_direction * arc_angle.cos(),
    )
}

pub(super) fn curve_points(start: Pos2, end: Pos2, style: CurveStyle) -> [Pos2; 4] {
    let delta = end - start;
    if delta == Vec2::ZERO {
        return [start, start, end, end];
    }

    let rank_direction = if delta.y >= 0.0 { 1.0 } else { -1.0 };
    let rank_distance = delta.y.abs().max(delta.x.abs() * 0.35);
    let handle = (rank_distance * style.rank_handle_fraction).max(style.min_handle);
    let handle = Vec2::new(0.0, handle * rank_direction);

    [start, start + handle, end - handle, end]
}

pub(super) fn self_loop_points(center: Pos2, radius: f32, style: CurveStyle) -> [Pos2; 4] {
    let radius = radius.max(1.0);
    let lift = (style.min_handle * 0.9).max(radius * 5.0);
    let spread = (style.min_handle * 0.6).max(radius * 3.5);
    let shoulder = radius * 0.8;
    let rise = radius * 0.45;
    let start = center + Vec2::new(shoulder, -rise);
    let end = center + Vec2::new(-shoulder, -rise);
    [
        start,
        start + Vec2::new(spread, -lift),
        end + Vec2::new(-spread, -lift),
        end,
    ]
}

pub(super) fn cubic_point(points: [Pos2; 4], t: f32) -> Pos2 {
    let mt = 1.0 - t;
    let weights = cubic_bezier_weights(mt, t);
    points
        .into_iter()
        .zip(weights)
        .fold(Pos2::ZERO, |point, (control, weight)| {
            point + control.to_vec2() * weight
        })
}

pub(super) fn cubic_tangent(points: [Pos2; 4], t: f32) -> Vec2 {
    let mt = 1.0 - t;
    let a = (points[1] - points[0]) * (3.0 * mt * mt);
    let b = (points[2] - points[1]) * (6.0 * mt * t);
    let c = (points[3] - points[2]) * (3.0 * t * t);
    a + b + c
}

fn cubic_bezier_weights(mt: f32, t: f32) -> [f32; 4] {
    [mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t]
}

pub(super) fn distance_to_curve(points: [Pos2; 4], point: Pos2, style: EdgeStyle) -> f32 {
    let mut distance = f32::MAX;
    let mut previous = points[0];
    let segments = style.curve.hit_segments.max(1);
    for step in 1..=segments {
        let current = cubic_point(points, step as f32 / segments as f32);
        distance = distance.min(distance_to_segment(previous, current, point));
        previous = current;
    }
    distance
}

pub(super) fn segments_intersect(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> bool {
    let ab_c = cross(b - a, c - a);
    let ab_d = cross(b - a, d - a);
    let cd_a = cross(d - c, a - c);
    let cd_b = cross(d - c, b - c);

    if nearly_zero(ab_c) && on_segment(a, b, c) {
        return true;
    }
    if nearly_zero(ab_d) && on_segment(a, b, d) {
        return true;
    }
    if nearly_zero(cd_a) && on_segment(c, d, a) {
        return true;
    }
    if nearly_zero(cd_b) && on_segment(c, d, b) {
        return true;
    }

    ab_c.signum() != ab_d.signum() && cd_a.signum() != cd_b.signum()
}

pub(super) fn curve_intersects_rect(points: [Pos2; 4], target: Rect, segments: usize) -> bool {
    let mut previous = points[0];
    for step in 1..=segments.max(1) {
        let current = cubic_point(points, step as f32 / segments.max(1) as f32);
        if segment_intersects_rect(previous, current, target) {
            return true;
        }
        previous = current;
    }
    false
}

pub(super) fn curve_interferes_rect(
    points: [Pos2; 4],
    target: Rect,
    margin: f32,
    segments: usize,
) -> bool {
    curve_intersects_rect(points, target.expand(margin.max(0.0)), segments)
}

pub(super) fn segment_intersects_rect(start: Pos2, end: Pos2, target: Rect) -> bool {
    if target.contains(start) || target.contains(end) {
        return true;
    }

    let top_left = target.left_top();
    let top_right = target.right_top();
    let bottom_right = target.right_bottom();
    let bottom_left = target.left_bottom();

    segments_intersect(start, end, top_left, top_right)
        || segments_intersect(start, end, top_right, bottom_right)
        || segments_intersect(start, end, bottom_right, bottom_left)
        || segments_intersect(start, end, bottom_left, top_left)
}

fn distance_to_segment(start: Pos2, end: Pos2, point: Pos2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_sq();
    if length_squared == 0.0 {
        return point.distance(start);
    }

    let t = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * t)
}

fn cross(left: Vec2, right: Vec2) -> f32 {
    left.x * right.y - left.y * right.x
}

fn nearly_zero(value: f32) -> bool {
    value.abs() <= 1.0e-5
}

fn on_segment(start: Pos2, end: Pos2, point: Pos2) -> bool {
    let min = Pos2::new(start.x.min(end.x), start.y.min(end.y));
    let max = Pos2::new(start.x.max(end.x), start.y.max(end.y));
    point.x >= min.x - 1.0e-5
        && point.x <= max.x + 1.0e-5
        && point.y >= min.y - 1.0e-5
        && point.y <= max.y + 1.0e-5
}
