use std::{f32::consts::TAU, time::Duration};

use eframe::egui::{
    Color32, Pos2, Shape, Stroke,
    epaint::{CircleShape, CubicBezierShape},
};

pub(super) const EFFECT_REPAINT_INTERVAL: Duration = Duration::from_millis(33);

const PULSE_SECONDS: f64 = 1.45;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum NodeVisualEffect {
    #[default]
    None,
    Glow,
    Pulse,
}

impl NodeVisualEffect {
    pub(super) fn merge(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    pub(super) fn with_interaction(self, selected: bool, hovered: bool, dragged: bool) -> Self {
        if selected || dragged {
            Self::Pulse
        } else if hovered {
            self.merge(Self::Glow)
        } else {
            self
        }
    }

    pub(super) fn needs_repaint(self) -> bool {
        matches!(self, Self::Pulse)
    }

    fn rank(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Glow => 1,
            Self::Pulse => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum EdgeVisualEffect {
    #[default]
    None,
    Glow,
    Pulse,
}

impl EdgeVisualEffect {
    pub(super) fn merge(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    pub(super) fn with_interaction(self, selected: bool) -> Self {
        if selected { Self::Pulse } else { self }
    }

    pub(super) fn needs_repaint(self) -> bool {
        matches!(self, Self::Pulse)
    }

    fn rank(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Glow => 1,
            Self::Pulse => 2,
        }
    }
}

pub(super) fn node_backdrop_shapes(
    center: Pos2,
    radius: f32,
    color: Color32,
    effect: NodeVisualEffect,
    time: f64,
) -> Vec<Shape> {
    if effect == NodeVisualEffect::None {
        return Vec::new();
    }

    let pulse = pulse_unit(time);
    let (outer_alpha, inner_alpha, radius_boost) = match effect {
        NodeVisualEffect::None => unreachable!(),
        NodeVisualEffect::Glow => (0.15, 0.24, 0.0),
        NodeVisualEffect::Pulse => (0.10 + 0.10 * pulse, 0.18 + 0.14 * pulse, 3.0 * pulse),
    };

    vec![
        circle(
            center,
            radius + 12.0 + radius_boost * 1.7,
            color,
            outer_alpha,
        ),
        circle(center, radius + 6.0 + radius_boost, color, inner_alpha),
    ]
}

pub(super) fn edge_backdrop_shapes(
    points: [Pos2; 4],
    color: Color32,
    stroke_width: f32,
    effect: EdgeVisualEffect,
    time: f64,
) -> Vec<Shape> {
    if effect == EdgeVisualEffect::None {
        return Vec::new();
    }

    let pulse = pulse_unit(time);
    let (outer_alpha, inner_alpha, width_boost) = match effect {
        EdgeVisualEffect::None => unreachable!(),
        EdgeVisualEffect::Glow => (0.12, 0.24, 0.0),
        EdgeVisualEffect::Pulse => (0.08 + 0.10 * pulse, 0.18 + 0.12 * pulse, 2.5 * pulse),
    };

    vec![
        bezier(
            points,
            stroke_width + 12.0 + width_boost * 2.0,
            color,
            outer_alpha,
        ),
        bezier(points, stroke_width + 5.0 + width_boost, color, inner_alpha),
    ]
}

fn circle(center: Pos2, radius: f32, color: Color32, alpha: f32) -> Shape {
    CircleShape {
        center,
        radius,
        fill: tint(color, alpha),
        stroke: Stroke::new(0.0, Color32::TRANSPARENT),
    }
    .into()
}

fn bezier(points: [Pos2; 4], width: f32, color: Color32, alpha: f32) -> Shape {
    CubicBezierShape::from_points_stroke(
        points,
        false,
        Color32::TRANSPARENT,
        Stroke::new(width, tint(color, alpha)),
    )
    .into()
}

fn tint(color: Color32, alpha: f32) -> Color32 {
    let [red, green, blue, _] = color.to_array();
    let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

fn pulse_unit(time: f64) -> f32 {
    let normalized = (time.rem_euclid(PULSE_SECONDS) / PULSE_SECONDS) as f32;
    (normalized * TAU).sin() * 0.5 + 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_effect_interaction_priority_preserves_pulse() {
        assert_eq!(
            NodeVisualEffect::Glow.with_interaction(true, false, false),
            NodeVisualEffect::Pulse
        );
        assert_eq!(
            NodeVisualEffect::Pulse.with_interaction(false, true, false),
            NodeVisualEffect::Pulse
        );
    }

    #[test]
    fn edge_effect_merge_keeps_strongest_effect() {
        assert_eq!(
            EdgeVisualEffect::Glow.merge(EdgeVisualEffect::Pulse),
            EdgeVisualEffect::Pulse
        );
        assert_eq!(
            EdgeVisualEffect::Pulse.merge(EdgeVisualEffect::Glow),
            EdgeVisualEffect::Pulse
        );
    }

    #[test]
    fn pulse_unit_stays_in_bounds() {
        for time in [0.0, 0.25, 1.45, 13.8] {
            let pulse = pulse_unit(time);
            assert!((0.0..=1.0).contains(&pulse));
        }
    }
}
