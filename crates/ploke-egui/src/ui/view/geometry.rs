use eframe::egui::{Pos2, Vec2};

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

fn distance_to_segment(start: Pos2, end: Pos2, point: Pos2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_sq();
    if length_squared == 0.0 {
        return point.distance(start);
    }

    let t = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * t)
}
