//! Segment-lane **bar profiles (v1)** — shared track and segment painting for operator bars.
//!
//! Visual contract (see [`operator-ui-policy.md`](../docs/style/operator-ui-policy.md)):
//! faint track background, muted gamma semantic fills (~0.7), saturated strokes, discrete
//! blocks with gaps for multi-segment lanes — not neon solid fills.
//!
//! ## Profiles (iterate spacing/hues here first)
//!
//! | Profile | Use | Layout |
//! |---------|-----|--------|
//! | [`BarProfile::SegmentLane`] | Bottom timeline (turns / tools / protocol) | Normalized multi-segment spans, 1–2px gaps |
//! | [`BarProfile::MetricFill`] | Analyst Snapshot metrics, protocol verdict rows | Single fill `value / section_max` |
//! | [`BarProfile::StackedOutcome`] | Tool-step outcome histogram (succeeded + failed) | Two normalized segments |
//! | [`BarProfile::EmptyTrack`] | Track shell only (no segments) | Faint lane + stroke |
//!
//! Call sites pass semantic [`egui::Color32`] from theme tokens; this module owns geometry only.

use eframe::egui::{self, Color32, Id, Painter, Rect, Sense, Stroke, StrokeKind, Ui};

/// Gap between adjacent segments in multi-segment lanes (px).
pub const SEGMENT_GAP_PX: f32 = 1.5;
/// Corner radius for lane track and segment blocks.
pub const SEGMENT_CORNER: f32 = 1.0;
/// Muted fill derived from saturated semantic stroke.
pub const SEMANTIC_FILL_GAMMA: f32 = 0.7;
/// Vertical inset inside the lane so segment strokes are not clipped.
pub const LANE_INSET_Y: f32 = 1.0;
/// Minimum width for a visible metric fill when value is positive.
pub const MIN_METRIC_FILL_WIDTH_PX: f32 = 2.0;
/// Minimum width for a timeline span before clamping.
pub const MIN_SPAN_WIDTH_PX: f32 = 3.0;
/// Horizontal inset for inspector metric bar tracks (pane edge; see `metric_row_board`).
pub const METRIC_TRACK_X_PADDING: f32 = 12.0;

/// Right gutter for row action buttons (copy, inspect, popout); same scale as metric tracks.
pub const PANE_LIST_TRAILING_INSET: f32 = METRIC_TRACK_X_PADDING;

/// Reserve [`PANE_LIST_TRAILING_INSET`] after trailing row actions so icons are not clipped at the pane edge.
pub fn add_pane_list_trailing_inset(ui: &mut egui::Ui) {
    ui.add_space(PANE_LIST_TRAILING_INSET);
}

/// One normalized segment in a multi-segment lane (`start`/`end` in \[0, 1\]).
#[derive(Debug, Clone)]
pub struct BarPaintSegment {
    pub start: f32,
    pub end: f32,
    pub semantic: Color32,
    pub tooltip: Option<String>,
}

/// Options for [`BarProfile::SegmentLane`] (timeline uses horizontal padding).
#[derive(Debug, Clone, Copy)]
pub struct SegmentLaneOpts {
    pub lane_height: f32,
    pub x_padding: f32,
    pub min_span_width: f32,
}

impl Default for SegmentLaneOpts {
    fn default() -> Self {
        Self {
            lane_height: 12.0,
            x_padding: 6.0,
            min_span_width: MIN_SPAN_WIDTH_PX,
        }
    }
}

/// Saturated stroke and gamma-muted fill for a semantic hue.
pub fn gamma_semantic_colors(semantic: Color32) -> (Color32, Color32) {
    (semantic.gamma_multiply(SEMANTIC_FILL_GAMMA), semantic)
}

/// Horizontal fill width for `value / section_max` capped at `track_width`; zero when `value <= 0`.
pub fn metric_fill_width(value: f32, section_max: f32, track_width: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }
    let section_max = section_max.max(1.0);
    let fill = track_width * (value / section_max);
    fill.min(track_width).max(MIN_METRIC_FILL_WIDTH_PX)
}

/// Content span inside a track rect after horizontal padding.
pub fn lane_content_x_span(track_rect: Rect, x_padding: f32) -> (f32, f32) {
    let left = track_rect.left() + x_padding;
    let width = (track_rect.width() - 2.0 * x_padding).max(1.0);
    (left, width)
}

/// Map normalized `t` ∈ \[0, 1\] to x inside padded track content.
pub fn lane_x_for_normalized(track_rect: Rect, t: f32, x_padding: f32) -> f32 {
    let (left, width) = lane_content_x_span(track_rect, x_padding);
    left + width * t
}

/// Clip rect and inner lane rect centered vertically in `track_rect`.
pub fn segment_lane_rects(track_rect: Rect, lane_height: f32, x_padding: f32) -> (Rect, Rect) {
    let clip_rect = track_rect;
    let lane_rect = Rect::from_min_max(
        egui::pos2(
            track_rect.left() + x_padding,
            track_rect.center().y - lane_height * 0.5,
        ),
        egui::pos2(
            track_rect.right() - x_padding,
            track_rect.center().y + lane_height * 0.5,
        ),
    );
    (clip_rect, lane_rect)
}

/// Left/right x for one normalized span with inter-segment gaps.
pub fn span_x_range(
    track_rect: Rect,
    span: &BarPaintSegment,
    index: usize,
    span_count: usize,
    x_padding: f32,
    min_span_width: f32,
    lane_rect: Rect,
) -> Option<(f32, f32)> {
    let mut left = lane_x_for_normalized(track_rect, span.start, x_padding);
    let mut right = lane_x_for_normalized(track_rect, span.end, x_padding);
    if right - left < min_span_width {
        right = (left + min_span_width).min(lane_rect.right());
    }
    left = left.clamp(lane_rect.left(), lane_rect.right());
    right = right.clamp(lane_rect.left(), lane_rect.right());
    let gap_half = SEGMENT_GAP_PX * 0.5;
    if index > 0 {
        left = (left + gap_half).min(right);
    }
    if index + 1 < span_count {
        right = (right - gap_half).max(left);
    }
    if right <= left {
        return None;
    }
    Some((left, right))
}

/// Segment block rect inside the lane for a span x-range.
pub fn segment_block_rect(lane_rect: Rect, left: f32, right: f32) -> Rect {
    Rect::from_min_max(
        egui::pos2(left, lane_rect.top() + LANE_INSET_Y),
        egui::pos2(right, lane_rect.bottom() - LANE_INSET_Y),
    )
}

/// Normalized \[start, end\] for the succeeded portion of a stacked outcome bar.
pub fn stacked_outcome_success_range(succeeded: usize, total: usize) -> (f32, f32) {
    if total == 0 || succeeded == 0 {
        return (0.0, 0.0);
    }
    let frac = succeeded as f32 / total as f32;
    (0.0, frac)
}

/// Normalized \[start, end\] for the failed portion (after succeeded).
pub fn stacked_outcome_failed_range(succeeded: usize, failed: usize, total: usize) -> (f32, f32) {
    if total == 0 || failed == 0 {
        return (0.0, 0.0);
    }
    let success_frac = succeeded as f32 / total as f32;
    let fail_frac = failed as f32 / total as f32;
    (success_frac, (success_frac + fail_frac).min(1.0))
}

pub fn paint_track_background(
    painter: &Painter,
    lane_rect: Rect,
    faint_bg: Color32,
    stroke: Color32,
) {
    painter.rect_filled(lane_rect, SEGMENT_CORNER, faint_bg);
    painter.rect_stroke(
        lane_rect,
        SEGMENT_CORNER,
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
}

pub fn paint_segment_block(painter: &Painter, segment_rect: Rect, semantic: Color32) {
    let (fill, stroke) = gamma_semantic_colors(semantic);
    painter.rect_filled(segment_rect, SEGMENT_CORNER, fill);
    painter.rect_stroke(
        segment_rect,
        SEGMENT_CORNER,
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
}

/// Paint a single metric fill segment from the left of `track_rect`.
pub fn paint_metric_fill_segment(
    painter: &Painter,
    track_rect: Rect,
    fill_width: f32,
    semantic: Color32,
) {
    if fill_width <= 0.0 {
        return;
    }
    let mut fill_rect = track_rect;
    fill_rect.set_width(fill_width.min(track_rect.width()));
    paint_segment_block(painter, fill_rect, semantic);
}

/// Faint lane track only ([`BarProfile::EmptyTrack`]).

/// Multi-segment timeline lane with hover tooltips.
pub fn paint_segment_lane(
    ui: &mut Ui,
    track_rect: Rect,
    spans: &[BarPaintSegment],
    opts: SegmentLaneOpts,
) {
    let (clip_rect, lane_rect) = segment_lane_rects(track_rect, opts.lane_height, opts.x_padding);
    let painter = ui.painter().with_clip_rect(clip_rect);
    let visuals = ui.visuals();
    paint_track_background(
        &painter,
        lane_rect,
        visuals.faint_bg_color,
        visuals.widgets.noninteractive.bg_stroke.color,
    );

    let span_response_id = Id::new((
        "segment_lane_span",
        track_rect.min.x.to_bits(),
        track_rect.min.y.to_bits(),
    ));
    for (index, span) in spans.iter().enumerate() {
        let Some((left, right)) = span_x_range(
            track_rect,
            span,
            index,
            spans.len(),
            opts.x_padding,
            opts.min_span_width,
            lane_rect,
        ) else {
            continue;
        };
        let segment_rect = segment_block_rect(lane_rect, left, right);
        let response = ui.interact(segment_rect, span_response_id.with(index), Sense::hover());
        paint_segment_block(&painter, segment_rect, span.semantic);
        if response.hovered() {
            if let Some(tooltip) = span.tooltip.as_deref() {
                response.show_tooltip_ui(|ui| {
                    ui.label(tooltip);
                });
            }
        }
    }
}

/// Metric fill using explicit pixel width (Analyst Snapshot / protocol verdict rows).
pub fn paint_metric_fill_bar(ui: &Ui, track_rect: Rect, fill_width: f32, semantic: Color32) {
    let painter = ui.painter().with_clip_rect(track_rect);
    paint_track_background(
        &painter,
        track_rect,
        ui.visuals().faint_bg_color,
        ui.visuals().widgets.noninteractive.bg_stroke.color,
    );
    paint_metric_fill_segment(&painter, track_rect, fill_width, semantic);
}

/// Stacked succeeded / failed tool-step histogram.
pub fn paint_stacked_outcome_bar(
    ui: &mut Ui,
    track_rect: Rect,
    total: usize,
    succeeded: usize,
    failed: usize,
    success_semantic: Color32,
    error_semantic: Color32,
) {
    if total == 0 {
        return;
    }
    let mut segments = Vec::new();
    if succeeded > 0 {
        let (start, end) = stacked_outcome_success_range(succeeded, total);
        segments.push(BarPaintSegment {
            start,
            end,
            semantic: success_semantic,
            tooltip: None,
        });
    }
    if failed > 0 {
        let (start, end) = stacked_outcome_failed_range(succeeded, failed, total);
        segments.push(BarPaintSegment {
            start,
            end,
            semantic: error_semantic,
            tooltip: None,
        });
    }
    let painter = ui.painter().with_clip_rect(track_rect);
    paint_track_background(
        &painter,
        track_rect,
        ui.visuals().faint_bg_color,
        ui.visuals().widgets.noninteractive.bg_stroke.color,
    );
    for (index, segment) in segments.iter().enumerate() {
        let Some((left, right)) = span_x_range(
            track_rect,
            segment,
            index,
            segments.len(),
            0.0,
            MIN_SPAN_WIDTH_PX,
            track_rect,
        ) else {
            continue;
        };
        let segment_rect = segment_block_rect(track_rect, left, right);
        paint_segment_block(&painter, segment_rect, segment.semantic);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Pos2;

    fn rect_width(left: f32, right: f32) -> f32 {
        right - left
    }

    #[test]
    fn metric_fill_width_scales_and_caps() {
        assert_eq!(metric_fill_width(0.0, 26.0, 100.0), 0.0);
        assert_eq!(metric_fill_width(26.0, 26.0, 100.0), 100.0);
        assert!((metric_fill_width(13.0, 26.0, 100.0) - 50.0).abs() < f32::EPSILON);
        assert_eq!(metric_fill_width(30.0, 20.0, 80.0), 80.0);
        assert_eq!(metric_fill_width(0.5, 26.0, 100.0), 2.0);
    }

    #[test]
    fn lane_content_x_span_respects_padding() {
        let track = Rect::from_min_max(Pos2::new(10.0, 0.0), Pos2::new(110.0, 12.0));
        let (left, width) = lane_content_x_span(track, 6.0);
        assert_eq!(left, 16.0);
        assert_eq!(width, 88.0);
    }

    #[test]
    fn lane_x_for_normalized_maps_endpoints() {
        let track = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 10.0));
        assert_eq!(lane_x_for_normalized(track, 0.0, 0.0), 0.0);
        assert_eq!(lane_x_for_normalized(track, 1.0, 0.0), 100.0);
        assert_eq!(lane_x_for_normalized(track, 0.5, 10.0), 50.0);
    }

    #[test]
    fn span_x_range_applies_gap_between_segments() {
        let track = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 12.0));
        let lane = track;
        let a = BarPaintSegment {
            start: 0.0,
            end: 0.5,
            semantic: Color32::WHITE,
            tooltip: None,
        };
        let b = BarPaintSegment {
            start: 0.5,
            end: 1.0,
            semantic: Color32::WHITE,
            tooltip: None,
        };
        let (l0, r0) = span_x_range(track, &a, 0, 2, 0.0, 3.0, lane).expect("a");
        let (l1, r1) = span_x_range(track, &b, 1, 2, 0.0, 3.0, lane).expect("b");
        assert!(r0 < l1);
        assert!(rect_width(l0, r0) > 40.0);
    }

    #[test]
    fn stacked_outcome_ranges_partition_unit_interval() {
        let (s0, s1) = stacked_outcome_success_range(7, 10);
        assert_eq!(s0, 0.0);
        assert!((s1 - 0.7).abs() < f32::EPSILON);
        let (f0, f1) = stacked_outcome_failed_range(7, 3, 10);
        assert!((f0 - 0.7).abs() < f32::EPSILON);
        assert!((f1 - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn gamma_semantic_colors_mutes_fill() {
        let semantic = Color32::from_rgb(200, 40, 40);
        let (fill, stroke) = gamma_semantic_colors(semantic);
        assert_eq!(stroke, semantic);
        assert!(fill.r() <= semantic.r());
    }
}
