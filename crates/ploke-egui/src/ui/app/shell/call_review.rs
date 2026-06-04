use crate::ui::id_display::{self, CopyableText};
use crate::ui::render::text::*;
use crate::ui::text::style as text_style;
use crate::ui::theme::{PaletteTokens, tokens_from_ui};
use eframe::egui;
use ploke_protocol::Confidence;
use ploke_records::protocol::ArtifactBody;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::cache::{effective_eval_pane_content_width, eval_pane_content_width};
use super::eval_protocol::{
    selected_eval_protocol_call_review_key, selected_tool_call_review,
    set_selected_eval_protocol_call_review,
};
use super::fields::*;
use super::run_dashboard::{
    RunDashboardValueTone, effective_tone_for_local_analysis_signal_metric,
};
use super::{InspectorRenderCache, show_inspector_collapsing};

const CALL_REVIEW_SCAN_HOVER_PREVIEW_BYTES: usize = 220;
const CALL_REVIEW_ASSESSMENT_SPOTLIGHT_INNER_MARGIN: i8 = 8;
const CALL_REVIEW_SPOTLIGHT_SIGNALS_NARROW_WIDTH: f32 = 400.0;
const CALL_REVIEW_SPOTLIGHT_RATIONALE_LABEL_INDENT: f32 = 4.0;
const CALL_REVIEW_SPOTLIGHT_CONFIDENCE_SEGMENT_WIDTH: f32 = 10.0;
const CALL_REVIEW_SPOTLIGHT_CONFIDENCE_SEGMENT_HEIGHT: f32 = 4.0;
const CALL_REVIEW_SPOTLIGHT_CONFIDENCE_SEGMENT_GAP: f32 = 2.0;

/// Body width inside assessment spotlight frames: visible eval column, not scroll min-width.
fn call_review_assessment_spotlight_content_width(ui: &egui::Ui, pane_content_width: f32) -> f32 {
    effective_eval_pane_content_width(ui)
        .min(pane_content_width)
        .max(1.0)
}

fn call_review_assessment_spotlight_row_width(ui: &egui::Ui, content_width: f32) -> f32 {
    call_review_assessment_spotlight_content_width(ui, content_width)
}

/// Short spotlight chip prefix; full metric name stays in hover text.
pub(crate) fn call_review_spotlight_signal_metric_short_label(metric: &str) -> &str {
    match metric {
        "repeated tools" => "repeated",
        "distinct tools" => "distinct",
        "similar searches" => "similar",
        "directory pivots" => "pivots",
        "scope turns" => "turns",
        "search calls" => "search",
        "read calls" => "read",
        "browse calls" => "browse",
        "edit calls" => "edit",
        "execute calls" => "execute",
        "failed calls" => "failed",
        "uncovered calls" => "uncovered",
        "labeled segments" => "labeled",
        "ambiguous segments" => "ambiguous",
        other => other,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CallReviewFilter {
    All,
    FailedScope,
    Mixed,
    Recoverable,
    Redundant,
    LowConfidence,
}

impl CallReviewFilter {
    const ALL: [Self; 6] = [
        Self::All,
        Self::FailedScope,
        Self::Mixed,
        Self::Recoverable,
        Self::Redundant,
        Self::LowConfidence,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::FailedScope => "scope failures",
            Self::Mixed => "mixed",
            Self::Recoverable => "recoverable",
            Self::Redundant => "redundant",
            Self::LowConfidence => "low confidence",
        }
    }

    fn hover_text(self) -> &'static str {
        match self {
            Self::All => "Show every reviewed tool call.",
            Self::FailedScope => "Show reviews whose local analysis scope includes failed calls.",
            Self::Mixed => "Show reviews whose overall verdict is Mixed.",
            Self::Recoverable => "Show reviews whose overall verdict is RecoverableDetour.",
            Self::Redundant => "Show reviews whose overall verdict is RedundantThrash.",
            Self::LowConfidence => "Show reviews whose overall confidence is Low.",
        }
    }

    fn count(self, counts: CallReviewScanCounts) -> usize {
        match self {
            Self::All => counts.total,
            Self::FailedScope => counts.failed_scope,
            Self::Mixed => counts.mixed,
            Self::Recoverable => counts.recoverable,
            Self::Redundant => counts.redundant,
            Self::LowConfidence => counts.low_confidence,
        }
    }

    fn matches(self, payload: &ploke_records::protocol::ToolCallReviewPayload) -> bool {
        match self {
            Self::All => true,
            Self::FailedScope => payload.output.signals.failed_calls_in_scope > 0,
            Self::Mixed => payload.output.overall == ploke_protocol::OverallVerdict::Mixed,
            Self::Recoverable => {
                payload.output.overall == ploke_protocol::OverallVerdict::RecoverableDetour
            }
            Self::Redundant => {
                payload.output.overall == ploke_protocol::OverallVerdict::RedundantThrash
            }
            Self::LowConfidence => {
                payload.output.overall_confidence == ploke_protocol::Confidence::Low
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct CallReviewSort {
    column: CallReviewSortColumn,
    direction: SortDirection,
}

impl CallReviewSort {
    fn toggle_column(&mut self, column: CallReviewSortColumn) {
        if self.column == column {
            self.direction = self.direction.toggled();
        } else {
            self.column = column;
            self.direction = SortDirection::Asc;
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum CallReviewSortColumn {
    #[default]
    Call,
    Tool,
    Outcome,
    Confidence,
    Failed,
    LatencyMs,
    Scope,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl SortDirection {
    fn toggled(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }

    fn indicator(self) -> &'static str {
        match self {
            Self::Asc => "^",
            Self::Desc => "v",
        }
    }

    fn apply(self, ordering: Ordering) -> Ordering {
        match self {
            Self::Asc => ordering,
            Self::Desc => ordering.reverse(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CallReviewScanCounts {
    total: usize,
    failed_scope: usize,
    mixed: usize,
    recoverable: usize,
    redundant: usize,
    low_confidence: usize,
}

#[derive(Debug, Default)]
pub(super) struct CallReviewScanOrderCache {
    pub(super) key: Option<CallReviewScanOrderKey>,
    pub(super) rows: Arc<[String]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CallReviewScanOrderKey {
    filter: CallReviewFilter,
    sort: CallReviewSort,
    artifact_count: usize,
    artifact_rows_hash: u64,
}

impl CallReviewScanOrderKey {
    pub(super) fn new(
        protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
        filter: CallReviewFilter,
        sort: CallReviewSort,
    ) -> Self {
        Self {
            filter,
            sort,
            artifact_count: protocol_artifacts.index.len(),
            artifact_rows_hash: protocol_call_review_rows_hash(protocol_artifacts),
        }
    }
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::EVAL_PROTOCOL_CALL_REVIEW_SCAN)]
pub(crate) fn render_call_review_scan(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
) {
    let counts = call_review_scan_counts(protocol_artifacts);
    let filter_id = ui.make_persistent_id("eval-protocol-call-review-filter");
    let mut filter = ui
        .data(|data| data.get_temp::<CallReviewFilter>(filter_id))
        .unwrap_or_else(|| failure_first_call_review_filter(counts));
    let sort_id = ui.make_persistent_id("eval-protocol-call-review-sort");
    let mut sort = ui
        .data(|data| data.get_temp::<CallReviewSort>(sort_id))
        .unwrap_or_default();

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Call Review Scan").default_open(true),
        |ui| {
            let scan_content_width = eval_pane_content_width(ui);
            ui.set_max_width(scan_content_width);
            cached_kv_usize(ui, render_cache, "call reviews", counts.total);
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(scan_content_width);
                cached_label(ui, render_cache, "slice");
                for candidate in CallReviewFilter::ALL {
                    let label = candidate.label();
                    let count = candidate.count(counts);
                    let response = ui
                        .selectable_label(filter == candidate, label)
                        .on_hover_text(candidate.hover_text());
                    if response.clicked() {
                        filter = candidate;
                        ui.data_mut(|data| data.insert_temp(filter_id, filter));
                    }
                    let mut buffer = itoa::Buffer::new();
                    cached_monospace_label(ui, render_cache, buffer.format(count));
                }
            });

            render_call_review_reasoning_spotlight(ui, render_cache, protocol_artifacts);

            let matching = filter.count(counts);
            if matching == 0 {
                cached_kv_text(ui, render_cache, "index", "no matching call reviews");
                return;
            }

            ui.separator();
            egui::Grid::new("eval-protocol-call-review-scan-grid")
                .num_columns(7)
                .striped(true)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    let header_background = ui.painter().add(egui::Shape::Noop);
                    let mut header_rect = egui::Rect::NOTHING;
                    let mut sort_changed = false;
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "call",
                            "Focal tool-call index in the run.",
                            CallReviewSortColumn::Call,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "tool",
                            "Focal tool name from the protocol review.",
                            CallReviewSortColumn::Tool,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "outcome",
                            "Overall protocol review verdict for the focal call.",
                            CallReviewSortColumn::Outcome,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "confidence",
                            "Overall confidence assigned by the protocol review.",
                            CallReviewSortColumn::Confidence,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "failed",
                            "Whether the focal call failed, or another call in scope failed.",
                            CallReviewSortColumn::Failed,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "latency ms",
                            "Protocol NeighborhoodCall.latency_ms: focal tool-call latency in milliseconds. This is not split into model wait versus local/tool time yet.",
                            CallReviewSortColumn::LatencyMs,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "scope",
                            "Number of calls in the local analysis scope.",
                            CallReviewSortColumn::Scope,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    ui.end_row();
                    if sort_changed {
                        ui.data_mut(|data| data.insert_temp(sort_id, sort));
                    }
                    paint_call_review_scan_header_background(
                        ui,
                        header_background,
                        header_rect.expand2(egui::vec2(4.0, 2.0)),
                    );

                    let selected_call_review = selected_eval_protocol_call_review_key(ui);
                    let row_order = render_cache.call_review_scan_order(
                        protocol_artifacts,
                        filter,
                        sort,
                    );
                    for artifact_key in row_order.iter() {
                        let Some(artifact) = protocol_artifacts.index.get(artifact_key.as_str())
                        else {
                            continue;
                        };
                        let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
                            continue;
                        };
                        let selected =
                            selected_call_review.as_deref() == Some(artifact_key.as_str());
                        render_call_review_scan_row(
                            ui,
                            render_cache,
                            artifact_key.as_str(),
                            payload,
                            selected,
                        );
                    }
                });
        },
    );
}

fn failure_first_call_review_filter(counts: CallReviewScanCounts) -> CallReviewFilter {
    if counts.failed_scope > 0 {
        CallReviewFilter::FailedScope
    } else if counts.redundant > 0 {
        CallReviewFilter::Redundant
    } else if counts.mixed > 0 {
        CallReviewFilter::Mixed
    } else if counts.low_confidence > 0 {
        CallReviewFilter::LowConfidence
    } else {
        CallReviewFilter::All
    }
}

fn call_review_scan_counts(
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
) -> CallReviewScanCounts {
    let mut counts = CallReviewScanCounts::default();
    for artifact in protocol_artifacts.index.values() {
        let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
            continue;
        };
        counts.total += 1;
        if CallReviewFilter::FailedScope.matches(payload) {
            counts.failed_scope += 1;
        }
        if CallReviewFilter::Mixed.matches(payload) {
            counts.mixed += 1;
        }
        if CallReviewFilter::Recoverable.matches(payload) {
            counts.recoverable += 1;
        }
        if CallReviewFilter::Redundant.matches(payload) {
            counts.redundant += 1;
        }
        if CallReviewFilter::LowConfidence.matches(payload) {
            counts.low_confidence += 1;
        }
    }
    counts
}

pub(super) fn build_call_review_scan_order(
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
    filter: CallReviewFilter,
    sort: CallReviewSort,
) -> Arc<[String]> {
    let mut rows = Vec::new();
    for (artifact_key, artifact) in &protocol_artifacts.index {
        let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
            continue;
        };
        if filter.matches(payload) {
            rows.push(artifact_key.clone());
        }
    }

    rows.sort_by(|left_key, right_key| {
        compare_call_review_artifact_keys(protocol_artifacts, left_key, right_key, sort)
    });
    Arc::from(rows)
}

fn compare_call_review_artifact_keys(
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
    left_key: &str,
    right_key: &str,
    sort: CallReviewSort,
) -> Ordering {
    let Some(left) = protocol_artifacts.index.get(left_key) else {
        return Ordering::Greater;
    };
    let Some(right) = protocol_artifacts.index.get(right_key) else {
        return Ordering::Less;
    };
    let ArtifactBody::ToolCallReview(left) = &left.body else {
        return Ordering::Greater;
    };
    let ArtifactBody::ToolCallReview(right) = &right.body else {
        return Ordering::Less;
    };

    sort.direction
        .apply(compare_call_review_payloads(left, right, sort.column))
        .then_with(|| left.input.focal.index.cmp(&right.input.focal.index))
        .then_with(|| left.input.focal.tool_name.cmp(&right.input.focal.tool_name))
        .then_with(|| left_key.cmp(right_key))
}

fn compare_call_review_payloads(
    left: &ploke_records::protocol::ToolCallReviewPayload,
    right: &ploke_records::protocol::ToolCallReviewPayload,
    column: CallReviewSortColumn,
) -> Ordering {
    match column {
        CallReviewSortColumn::Call => left.input.focal.index.cmp(&right.input.focal.index),
        CallReviewSortColumn::Tool => left.input.focal.tool_name.cmp(&right.input.focal.tool_name),
        CallReviewSortColumn::Outcome => overall_verdict_rank(left.output.overall)
            .cmp(&overall_verdict_rank(right.output.overall)),
        CallReviewSortColumn::Confidence => confidence_rank(left.output.overall_confidence)
            .cmp(&confidence_rank(right.output.overall_confidence)),
        CallReviewSortColumn::Failed => failure_rank(left).cmp(&failure_rank(right)),
        CallReviewSortColumn::LatencyMs => left
            .input
            .focal
            .latency_ms
            .cmp(&right.input.focal.latency_ms),
        CallReviewSortColumn::Scope => left
            .output
            .packet
            .total_calls_in_scope
            .cmp(&right.output.packet.total_calls_in_scope),
    }
}

fn protocol_call_review_rows_hash(
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    for (key, artifact) in &protocol_artifacts.index {
        key.hash(&mut hasher);
        if let ArtifactBody::ToolCallReview(payload) = &artifact.body {
            1u8.hash(&mut hasher);
            payload.input.focal.index.hash(&mut hasher);
            payload.input.focal.tool_name.hash(&mut hasher);
            overall_verdict_rank(payload.output.overall).hash(&mut hasher);
            confidence_rank(payload.output.overall_confidence).hash(&mut hasher);
            failure_rank(payload).hash(&mut hasher);
            payload.input.focal.latency_ms.hash(&mut hasher);
            payload.output.packet.total_calls_in_scope.hash(&mut hasher);
        } else {
            0u8.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn call_review_scan_header_cell(
    ui: &mut egui::Ui,
    label: &'static str,
    hover_text: &'static str,
    column: CallReviewSortColumn,
    sort: &mut CallReviewSort,
    sort_changed: &mut bool,
) -> egui::Response {
    let active = sort.column == column;
    ui.horizontal(|ui| {
        let response = ui
            .add(egui::Button::new(text_style::inspector_table_header(ui, label)).frame(false))
            .on_hover_ui(|ui| {
                ui.label(hover_text);
                ui.label(call_review_sort_hover_text(active, sort.direction));
            });
        if response.clicked() {
            sort.toggle_column(column);
            *sort_changed = true;
        }
        if sort.column == column {
            ui.label(text_style::inspector_table_sort_indicator(
                ui,
                sort.direction.indicator(),
            ));
        }
    })
    .response
}

fn call_review_sort_hover_text(active: bool, direction: SortDirection) -> &'static str {
    if !active {
        "Click to sort by this column."
    } else {
        match direction {
            SortDirection::Asc => "Sorted ascending. Click to sort descending.",
            SortDirection::Desc => "Sorted descending. Click to sort ascending.",
        }
    }
}

fn include_response_rect(rect: &mut egui::Rect, response: &egui::Response) {
    *rect = rect.union(response.rect);
}

fn paint_call_review_scan_header_background(
    ui: &egui::Ui,
    shape: egui::layers::ShapeIdx,
    rect: egui::Rect,
) {
    ui.painter().set(
        shape,
        egui::Shape::rect_filled(rect, 0.0, text_style::inspector_table_header_background(ui)),
    );
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::EVAL_PROTOCOL_CALL_REVIEW_ROW)]
fn render_call_review_scan_row(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    artifact_key: &str,
    payload: &ploke_records::protocol::ToolCallReviewPayload,
    selected: bool,
) {
    let background = ui.painter().add(egui::Shape::Noop);
    let rail = ui.painter().add(egui::Shape::Noop);
    let mut row_rect = egui::Rect::NOTHING;
    let mut row_hovered = false;
    let mut row_clicked = false;
    let focal = &payload.input.focal;
    let mut index_buffer = itoa::Buffer::new();
    let response = cached_monospace_cell(ui, render_cache, index_buffer.format(focal.index));
    let response = call_review_scan_reason_hover(
        response,
        render_cache,
        "focal call summary",
        focal.summary.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    let response = cached_monospace_cell(ui, render_cache, focal.tool_name.as_str());
    let response = call_review_scan_reason_hover(
        response,
        render_cache,
        "LLM synthesis",
        payload.output.synthesis_rationale.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    let response = scan_value_cell(
        ui,
        render_cache,
        overall_verdict_label(payload.output.overall),
        overall_verdict_emphasis(payload.output.overall),
    );
    let response = call_review_scan_reason_hover(
        response,
        render_cache,
        "LLM synthesis",
        payload.output.synthesis_rationale.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    let response = scan_value_cell(
        ui,
        render_cache,
        confidence_label(payload.output.overall_confidence),
        confidence_emphasis(payload.output.overall_confidence),
    );
    let response = call_review_scan_reason_hover(
        response,
        render_cache,
        "LLM synthesis",
        payload.output.synthesis_rationale.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    let response = scan_value_cell(
        ui,
        render_cache,
        call_review_failure_label(payload),
        failure_emphasis(payload),
    );
    let response = call_review_scan_reason_hover(
        response,
        render_cache,
        "LLM recoverability rationale",
        payload.output.recoverability.rationale.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    let mut latency_buffer = itoa::Buffer::new();
    let response = cached_monospace_cell(ui, render_cache, latency_buffer.format(focal.latency_ms));
    let response = call_review_scan_two_part_hover(
        response,
        render_cache,
        "focal call summary",
        focal.summary.as_str(),
        "LLM synthesis",
        payload.output.synthesis_rationale.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    let mut scope_buffer = itoa::Buffer::new();
    let response = cached_monospace_cell(
        ui,
        render_cache,
        scope_buffer.format(payload.output.packet.total_calls_in_scope),
    );
    let response = call_review_scan_two_part_hover(
        response,
        render_cache,
        "scope summary",
        payload.output.packet.scope_summary.as_str(),
        "LLM synthesis",
        payload.output.synthesis_rationale.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    ui.end_row();

    let row_hovered = row_hovered || call_review_scan_row_contains_pointer(ui, row_rect);
    if row_clicked {
        set_selected_eval_protocol_call_review(ui, artifact_key);
    }

    let fill = if selected {
        Some(text_style::inspector_table_selected_row_background(ui))
    } else if row_hovered {
        Some(text_style::inspector_table_hovered_row_background(ui))
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().set(
            background,
            egui::Shape::rect_filled(row_rect.expand2(egui::vec2(4.0, 1.0)), 0.0, fill),
        );
    }
    if selected {
        let rail_rect = egui::Rect::from_min_max(
            egui::pos2(row_rect.left() - 5.0, row_rect.top() - 1.0),
            egui::pos2(row_rect.left() - 2.0, row_rect.bottom() + 1.0),
        );
        ui.painter().set(
            rail,
            egui::Shape::rect_filled(
                rail_rect,
                0.0,
                text_style::inspector_table_selected_row_rail(ui),
            ),
        );
    }
}

fn call_review_scan_row_contains_pointer(ui: &egui::Ui, row_rect: egui::Rect) -> bool {
    if !row_rect.is_positive() {
        return false;
    }
    let hit_rect = row_rect.expand2(egui::vec2(4.0, 2.0));
    ui.rect_contains_pointer(hit_rect)
}

fn include_call_review_scan_cell(
    row_rect: &mut egui::Rect,
    row_hovered: &mut bool,
    row_clicked: &mut bool,
    response: egui::Response,
) {
    include_response_rect(row_rect, &response);
    *row_hovered |= response.hovered();
    *row_clicked |= response.clicked();
}

fn cached_monospace_cell(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, CachedTextKind::Monospace);
    ui.add(egui::Label::new(galley).sense(egui::Sense::click()))
}

fn scan_value_cell(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
    emphasis: ScanValueEmphasis,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, scan_value_cached_text_kind(emphasis));
    ui.add(egui::Label::new(galley).sense(egui::Sense::click()))
}

fn call_review_scan_reason_hover(
    response: egui::Response,
    render_cache: &mut InspectorRenderCache,
    title: &'static str,
    reasoning: &str,
) -> egui::Response {
    response.on_hover_ui(|ui| {
        cached_label(ui, render_cache, title);
        cached_hover_monospace_preview(ui, render_cache, reasoning);
        cached_label(
            ui,
            render_cache,
            "Click to show the full reasoning in the right inspector.",
        );
    })
}

fn call_review_scan_two_part_hover(
    response: egui::Response,
    render_cache: &mut InspectorRenderCache,
    first_title: &'static str,
    first_body: &str,
    second_title: &'static str,
    second_body: &str,
) -> egui::Response {
    response.on_hover_ui(|ui| {
        cached_label(ui, render_cache, first_title);
        cached_hover_monospace_preview(ui, render_cache, first_body);
        ui.separator();
        cached_label(ui, render_cache, second_title);
        cached_hover_monospace_preview(ui, render_cache, second_body);
        cached_label(
            ui,
            render_cache,
            "Click to show the full reasoning in the right inspector.",
        );
    })
}

pub(crate) fn cached_hover_monospace_block(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, CachedTextKind::MonospaceHover);
    ui.add(egui::Label::new(galley))
}

fn cached_hover_monospace_preview(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let (preview, truncated) = bounded_utf8_prefix(text, CALL_REVIEW_SCAN_HOVER_PREVIEW_BYTES);
    let response = cached_hover_monospace_block(ui, render_cache, preview);
    if truncated {
        cached_label(
            ui,
            render_cache,
            "Preview truncated to keep table hover responsive.",
        );
    }
    response
}

fn bounded_utf8_prefix(text: &str, max_bytes: usize) -> (&str, bool) {
    if text.len() <= max_bytes {
        return (text, false);
    }

    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScanValueEmphasis {
    Normal,
    Warn,
    Error,
}

pub(crate) fn scan_value_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
    emphasis: ScanValueEmphasis,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, scan_value_cached_text_kind(emphasis));
    ui.add(egui::Label::new(galley))
}

/// Compact chip text: `dimension: value` (e.g. `usefulness: none`).
pub(crate) fn format_assessment_dimension_chip<'a>(
    dimension: &str,
    value: &str,
    buffer: &'a mut String,
) -> &'a str {
    buffer.clear();
    buffer.reserve(dimension.len() + 2 + value.len());
    buffer.push_str(dimension);
    buffer.push(':');
    buffer.push(' ');
    buffer.push_str(value);
    buffer.as_str()
}

pub(crate) fn render_assessment_dimension_chip(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    dimension: &str,
    value: &str,
    emphasis: ScanValueEmphasis,
    tooltip: &str,
    label_buffer: &mut String,
) -> egui::Response {
    let text = format_assessment_dimension_chip(dimension, value, label_buffer);
    scan_value_label(ui, render_cache, text, emphasis).on_hover_text(tooltip)
}

pub(crate) const ASSESSMENT_CHIP_OUTCOME_TOOLTIP: &str =
    "Overall outcome verdict synthesized from usefulness, redundancy, and recoverability.";
pub(crate) const ASSESSMENT_CHIP_CONFIDENCE_TOOLTIP: &str =
    "Confidence assigned to the overall outcome verdict.";
pub(crate) const ASSESSMENT_CHIP_USEFULNESS_TOOLTIP: &str =
    "Usefulness: whether the focal call advanced the run. \"none\" is no value (not missing data).";
pub(crate) const ASSESSMENT_CHIP_REDUNDANCY_TOOLTIP: &str =
    "Redundancy: whether the call repeated or overlapped prior work in scope.";
pub(crate) const ASSESSMENT_CHIP_RECOVERABILITY_TOOLTIP: &str =
    "Recoverability: whether a clear next step exists if the call was a detour or failure.";

fn scan_value_cached_text_kind(emphasis: ScanValueEmphasis) -> CachedTextKind {
    match emphasis {
        ScanValueEmphasis::Normal => CachedTextKind::Monospace,
        ScanValueEmphasis::Warn => CachedTextKind::MonospaceWarn,
        ScanValueEmphasis::Error => CachedTextKind::MonospaceError,
    }
}

pub(crate) fn call_review_failure_label(
    payload: &ploke_records::protocol::ToolCallReviewPayload,
) -> &'static str {
    if payload.input.focal.failed {
        "focal"
    } else if payload.output.signals.failed_calls_in_scope > 0 {
        "scope"
    } else {
        "ok"
    }
}

fn failure_rank(payload: &ploke_records::protocol::ToolCallReviewPayload) -> u8 {
    if payload.input.focal.failed {
        2
    } else if payload.output.signals.failed_calls_in_scope > 0 {
        1
    } else {
        0
    }
}

pub(crate) fn failure_emphasis(
    payload: &ploke_records::protocol::ToolCallReviewPayload,
) -> ScanValueEmphasis {
    if payload.input.focal.failed {
        ScanValueEmphasis::Error
    } else if payload.output.signals.failed_calls_in_scope > 0 {
        ScanValueEmphasis::Warn
    } else {
        ScanValueEmphasis::Normal
    }
}

pub(crate) fn overall_verdict_label(verdict: ploke_protocol::OverallVerdict) -> &'static str {
    match verdict {
        ploke_protocol::OverallVerdict::FocusedProgress => "focused",
        ploke_protocol::OverallVerdict::UsefulExploration => "useful",
        ploke_protocol::OverallVerdict::RecoverableDetour => "recoverable",
        ploke_protocol::OverallVerdict::RedundantThrash => "redundant",
        ploke_protocol::OverallVerdict::Mixed => "mixed",
        ploke_protocol::OverallVerdict::Unclear => "unclear",
    }
}

fn overall_verdict_rank(verdict: ploke_protocol::OverallVerdict) -> u8 {
    match verdict {
        ploke_protocol::OverallVerdict::FocusedProgress => 0,
        ploke_protocol::OverallVerdict::UsefulExploration => 1,
        ploke_protocol::OverallVerdict::RecoverableDetour => 2,
        ploke_protocol::OverallVerdict::RedundantThrash => 3,
        ploke_protocol::OverallVerdict::Mixed => 4,
        ploke_protocol::OverallVerdict::Unclear => 5,
    }
}

pub(crate) fn overall_verdict_emphasis(
    verdict: ploke_protocol::OverallVerdict,
) -> ScanValueEmphasis {
    match verdict {
        ploke_protocol::OverallVerdict::FocusedProgress
        | ploke_protocol::OverallVerdict::UsefulExploration => ScanValueEmphasis::Normal,
        ploke_protocol::OverallVerdict::RecoverableDetour
        | ploke_protocol::OverallVerdict::Mixed
        | ploke_protocol::OverallVerdict::Unclear => ScanValueEmphasis::Warn,
        ploke_protocol::OverallVerdict::RedundantThrash => ScanValueEmphasis::Error,
    }
}

pub(crate) fn confidence_label(confidence: ploke_protocol::Confidence) -> &'static str {
    match confidence {
        ploke_protocol::Confidence::Low => "low",
        ploke_protocol::Confidence::Medium => "medium",
        ploke_protocol::Confidence::High => "high",
    }
}

fn confidence_rank(confidence: ploke_protocol::Confidence) -> u8 {
    match confidence {
        ploke_protocol::Confidence::Low => 0,
        ploke_protocol::Confidence::Medium => 1,
        ploke_protocol::Confidence::High => 2,
    }
}

pub(crate) fn confidence_emphasis(confidence: ploke_protocol::Confidence) -> ScanValueEmphasis {
    match confidence {
        ploke_protocol::Confidence::Low => ScanValueEmphasis::Warn,
        ploke_protocol::Confidence::Medium | ploke_protocol::Confidence::High => {
            ScanValueEmphasis::Normal
        }
    }
}

pub(crate) fn usefulness_verdict_label(verdict: ploke_protocol::UsefulnessVerdict) -> &'static str {
    match verdict {
        ploke_protocol::UsefulnessVerdict::KeyProgress => "key",
        ploke_protocol::UsefulnessVerdict::HelpfulButNonEssential => "helpful",
        ploke_protocol::UsefulnessVerdict::LowValue => "low",
        ploke_protocol::UsefulnessVerdict::NoValue => "none",
        ploke_protocol::UsefulnessVerdict::Unclear => "unclear",
    }
}

pub(crate) fn usefulness_verdict_emphasis(
    verdict: ploke_protocol::UsefulnessVerdict,
) -> ScanValueEmphasis {
    match verdict {
        ploke_protocol::UsefulnessVerdict::KeyProgress
        | ploke_protocol::UsefulnessVerdict::HelpfulButNonEssential => ScanValueEmphasis::Normal,
        ploke_protocol::UsefulnessVerdict::LowValue
        | ploke_protocol::UsefulnessVerdict::Unclear => ScanValueEmphasis::Warn,
        ploke_protocol::UsefulnessVerdict::NoValue => ScanValueEmphasis::Error,
    }
}

pub(crate) fn redundancy_verdict_label(verdict: ploke_protocol::RedundancyVerdict) -> &'static str {
    match verdict {
        ploke_protocol::RedundancyVerdict::Distinct => "distinct",
        ploke_protocol::RedundancyVerdict::Overlapping => "overlap",
        ploke_protocol::RedundancyVerdict::RedundantRepeat => "repeat",
        ploke_protocol::RedundancyVerdict::SearchThrash => "thrash",
        ploke_protocol::RedundancyVerdict::Unclear => "unclear",
    }
}

pub(crate) fn redundancy_verdict_emphasis(
    verdict: ploke_protocol::RedundancyVerdict,
) -> ScanValueEmphasis {
    match verdict {
        ploke_protocol::RedundancyVerdict::Distinct
        | ploke_protocol::RedundancyVerdict::Overlapping => ScanValueEmphasis::Normal,
        ploke_protocol::RedundancyVerdict::Unclear => ScanValueEmphasis::Warn,
        ploke_protocol::RedundancyVerdict::RedundantRepeat
        | ploke_protocol::RedundancyVerdict::SearchThrash => ScanValueEmphasis::Error,
    }
}

pub(crate) fn recoverability_verdict_label(
    verdict: ploke_protocol::RecoverabilityVerdict,
) -> &'static str {
    match verdict {
        ploke_protocol::RecoverabilityVerdict::NoRecoveryNeeded => "ok",
        ploke_protocol::RecoverabilityVerdict::ClearNextStep => "clear",
        ploke_protocol::RecoverabilityVerdict::PartialNextStep => "partial",
        ploke_protocol::RecoverabilityVerdict::NoClearRecovery => "blocked",
        ploke_protocol::RecoverabilityVerdict::Unclear => "unclear",
    }
}

pub(crate) fn recoverability_verdict_emphasis(
    verdict: ploke_protocol::RecoverabilityVerdict,
) -> ScanValueEmphasis {
    match verdict {
        ploke_protocol::RecoverabilityVerdict::NoRecoveryNeeded
        | ploke_protocol::RecoverabilityVerdict::ClearNextStep => ScanValueEmphasis::Normal,
        ploke_protocol::RecoverabilityVerdict::PartialNextStep
        | ploke_protocol::RecoverabilityVerdict::Unclear => ScanValueEmphasis::Warn,
        ploke_protocol::RecoverabilityVerdict::NoClearRecovery => ScanValueEmphasis::Error,
    }
}

struct CallReviewAssessmentSpotlightConfig {
    title: &'static str,
    artifact_cache_key: &'static str,
    empty_selection_message: &'static str,
    not_available_label: &'static str,
    not_applicable_label: &'static str,
    show_synthesis_prose: bool,
}

const CALL_REVIEW_REASONING_SPOTLIGHT: CallReviewAssessmentSpotlightConfig =
    CallReviewAssessmentSpotlightConfig {
        title: "LLM Reasoning Spotlight",
        artifact_cache_key: "call-review-spotlight-artifact",
        empty_selection_message: "Select a call row to pin its LLM reasoning here.",
        not_available_label: "reasoning",
        not_applicable_label: "reasoning",
        show_synthesis_prose: false,
    };

const RUN_SYNTHESIS_SPOTLIGHT: CallReviewAssessmentSpotlightConfig =
    CallReviewAssessmentSpotlightConfig {
        title: "Run synthesis",
        artifact_cache_key: "run-synthesis-spotlight-artifact",
        empty_selection_message: "Select a call row in Call Review Scan to pin run synthesis here.",
        not_available_label: "synthesis",
        not_applicable_label: "synthesis",
        show_synthesis_prose: true,
    };

#[ploke_egui_macros::profile_scope(crate::allocation::scope::EVAL_PROTOCOL_RUN_SYNTHESIS_SPOTLIGHT)]
pub(crate) fn render_run_synthesis_spotlight(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
) {
    render_call_review_assessment_spotlight(
        ui,
        render_cache,
        protocol_artifacts,
        RUN_SYNTHESIS_SPOTLIGHT,
    );
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::EVAL_PROTOCOL_CALL_REVIEW_SPOTLIGHT)]
pub(super) fn render_call_review_reasoning_spotlight(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
) {
    render_call_review_assessment_spotlight(
        ui,
        render_cache,
        protocol_artifacts,
        CALL_REVIEW_REASONING_SPOTLIGHT,
    );
}

fn render_call_review_assessment_spotlight(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
    config: CallReviewAssessmentSpotlightConfig,
) {
    let selected_artifact_key = selected_eval_protocol_call_review_key(ui);
    let pane_content_width = eval_pane_content_width(ui);
    ui.add_space(4.0);
    egui::Frame::group(ui.style())
        .fill(ui.visuals().widgets.active.weak_bg_fill)
        .stroke(egui::Stroke::new(
            1.0,
            crate::ui::theme::tokens_from_ui(ui).accent,
        ))
        .inner_margin(egui::Margin::same(
            CALL_REVIEW_ASSESSMENT_SPOTLIGHT_INNER_MARGIN,
        ))
        .show(ui, |ui| {
            let content_width =
                call_review_assessment_spotlight_content_width(ui, pane_content_width);
            ui.set_max_width(content_width);
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(content_width);
                cached_label(ui, render_cache, config.title);
                if let Some(artifact_key) = selected_artifact_key.as_deref() {
                    cached_artifact_file_value(
                        ui,
                        render_cache,
                        (config.artifact_cache_key, artifact_key),
                        artifact_key,
                    );
                }
            });

            let Some((_artifact_key, payload)) = selected_tool_call_review(ui, protocol_artifacts)
            else {
                let Some(artifact_key) = selected_artifact_key.as_deref() else {
                    cached_label(ui, render_cache, config.empty_selection_message);
                    return;
                };
                cached_kv_artifact_file(ui, render_cache, "selected", artifact_key);
                if protocol_artifacts.index.get(artifact_key).is_some() {
                    cached_kv_id(
                        ui,
                        render_cache,
                        config.not_applicable_label,
                        "not_applicable",
                    );
                } else {
                    cached_kv_id(
                        ui,
                        render_cache,
                        config.not_available_label,
                        "not_available",
                    );
                }
                return;
            };

            let output = &payload.output;
            let focal = &payload.input.focal;
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(content_width);
                cached_label(ui, render_cache, "call");
                let mut index_buffer = itoa::Buffer::new();
                cached_monospace_label(ui, render_cache, index_buffer.format(focal.index));
                cached_monospace_label(ui, render_cache, focal.tool_name.as_str());
                scan_value_label(
                    ui,
                    render_cache,
                    overall_verdict_label(output.overall),
                    overall_verdict_emphasis(output.overall),
                );
                scan_value_label(
                    ui,
                    render_cache,
                    confidence_label(output.overall_confidence),
                    confidence_emphasis(output.overall_confidence),
                );
                scan_value_label(
                    ui,
                    render_cache,
                    call_review_failure_label(payload),
                    failure_emphasis(payload),
                );
            });

            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(content_width);
                render_call_review_spotlight_verdict_card(
                    ui,
                    render_cache,
                    "usefulness",
                    usefulness_verdict_label(output.usefulness.verdict),
                    usefulness_verdict_emphasis(output.usefulness.verdict),
                    output.usefulness.confidence,
                );
                render_call_review_spotlight_verdict_card(
                    ui,
                    render_cache,
                    "redundancy",
                    redundancy_verdict_label(output.redundancy.verdict),
                    redundancy_verdict_emphasis(output.redundancy.verdict),
                    output.redundancy.confidence,
                );
                render_call_review_spotlight_verdict_card(
                    ui,
                    render_cache,
                    "recoverability",
                    recoverability_verdict_label(output.recoverability.verdict),
                    recoverability_verdict_emphasis(output.recoverability.verdict),
                    output.recoverability.confidence,
                );
            });

            ui.add_space(4.0);
            render_call_review_spotlight_signals(ui, render_cache, content_width, &output.signals);

            if config.show_synthesis_prose {
                ui.add_space(4.0);
                if call_review_spotlight_has_structured_assessment(output) {
                    render_call_review_spotlight_structured_synthesis_note(ui, render_cache);
                } else if !output.synthesis_rationale.trim().is_empty() {
                    render_call_review_spotlight_synthesis_prose(
                        ui,
                        render_cache,
                        content_width,
                        output.synthesis_rationale.as_str(),
                    );
                }
            }

            ui.add_space(2.0);
            render_call_review_spotlight_rationale_bullet(
                ui,
                render_cache,
                content_width,
                "Usefulness",
                output.usefulness.rationale.as_str(),
            );
            render_call_review_spotlight_rationale_bullet(
                ui,
                render_cache,
                content_width,
                "Redundancy",
                output.redundancy.rationale.as_str(),
            );
            render_call_review_spotlight_rationale_bullet(
                ui,
                render_cache,
                content_width,
                "Recoverability",
                output.recoverability.rationale.as_str(),
            );
        });
}

/// Typed verdicts, signals, and dimension rationales are shown in the spotlight; the
/// mechanized `synthesis_rationale` string duplicates that wire-format summary.
fn call_review_spotlight_has_structured_assessment(
    output: &ploke_protocol::LocalAnalysisAssessment,
) -> bool {
    call_review_spotlight_has_dimension_rationale(output)
        || call_review_spotlight_synthesis_is_mechanized_wire_format(
            output.synthesis_rationale.as_str(),
        )
}

fn call_review_spotlight_has_dimension_rationale(
    output: &ploke_protocol::LocalAnalysisAssessment,
) -> bool {
    !output.usefulness.rationale.trim().is_empty()
        || !output.redundancy.rationale.trim().is_empty()
        || !output.recoverability.rationale.trim().is_empty()
}

fn call_review_spotlight_synthesis_is_mechanized_wire_format(synthesis: &str) -> bool {
    synthesis.starts_with("usefulness=") && synthesis.contains("branch rationales:")
}

fn render_call_review_spotlight_structured_synthesis_note(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
) {
    cached_label(ui, render_cache, "Structured summary from call review");
}

fn render_call_review_spotlight_synthesis_prose(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    content_width: f32,
    synthesis: &str,
) {
    let copy_value = CopyableText::new(synthesis);
    let row_width = call_review_assessment_spotlight_row_width(ui, content_width);
    ui.horizontal_top(|ui| {
        let bullet = ui.label(egui::RichText::new("•").weak());
        let prose_width = (row_width - bullet.rect.width() - ui.spacing().item_spacing.x).max(1.0);
        ui.vertical(|ui| {
            ui.set_max_width(prose_width);
            ui.horizontal(|ui| {
                cached_label(ui, render_cache, "synthesis");
                id_display::copy_button(ui, &copy_value);
            });
            let response = render_inspector_wrapped_prose_in_width(
                ui,
                render_cache,
                synthesis,
                prose_width,
                egui::Sense::click(),
            );
            id_display::attach_copy_context_menu(&response, &copy_value);
        });
    });
}

fn render_call_review_spotlight_verdict_card(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    title: &str,
    verdict: &str,
    verdict_emphasis: ScanValueEmphasis,
    confidence: Confidence,
) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.set_min_width(88.0);
            ui.vertical(|ui| {
                cached_label(ui, render_cache, title);
                scan_value_label(ui, render_cache, verdict, verdict_emphasis);
                render_call_review_spotlight_verdict_bar(ui, verdict_emphasis, confidence);
            });
        });
}

const SPOTLIGHT_VERDICT_BAR_TIER_COUNT: usize = 3;
const SPOTLIGHT_VERDICT_BAR_INACTIVE_GAMMA: f32 = 0.38;

/// Active segment index for the three-tier under-bar (concerning → neutral → good).
pub(crate) fn spotlight_verdict_bar_active_tier(verdict_emphasis: ScanValueEmphasis) -> usize {
    match verdict_emphasis {
        ScanValueEmphasis::Error => 0,
        ScanValueEmphasis::Warn => 1,
        ScanValueEmphasis::Normal => 2,
    }
}

fn spotlight_verdict_bar_tier_emphasis(tier: usize) -> ScanValueEmphasis {
    match tier {
        0 => ScanValueEmphasis::Error,
        1 => ScanValueEmphasis::Warn,
        _ => ScanValueEmphasis::Normal,
    }
}

/// Right-most (good) tier fill — theme semantic success, not body text color.
pub(crate) fn spotlight_verdict_bar_good_tier_fill(tokens: PaletteTokens) -> egui::Color32 {
    tokens.success
}

/// Base segment color for a tier; error/warn match [`scan_value_label`]; good tier uses success.
pub(crate) fn spotlight_verdict_bar_tier_color(
    ui: &egui::Ui,
    tier_emphasis: ScanValueEmphasis,
) -> egui::Color32 {
    match tier_emphasis {
        ScanValueEmphasis::Error => text_style::inspector_error_text_color(ui),
        ScanValueEmphasis::Warn => text_style::inspector_warn_text_color(ui),
        ScanValueEmphasis::Normal => spotlight_verdict_bar_good_tier_fill(tokens_from_ui(ui)),
    }
}

fn spotlight_verdict_bar_segment_fill(
    ui: &egui::Ui,
    tier_emphasis: ScanValueEmphasis,
    active: bool,
) -> egui::Color32 {
    let base = spotlight_verdict_bar_tier_color(ui, tier_emphasis);
    if active {
        base
    } else {
        base.gamma_multiply(SPOTLIGHT_VERDICT_BAR_INACTIVE_GAMMA)
    }
}

fn render_call_review_spotlight_verdict_bar(
    ui: &mut egui::Ui,
    verdict_emphasis: ScanValueEmphasis,
    confidence: Confidence,
) {
    let active_tier = spotlight_verdict_bar_active_tier(verdict_emphasis);
    let segment_count = SPOTLIGHT_VERDICT_BAR_TIER_COUNT as f32;
    let width = segment_count * CALL_REVIEW_SPOTLIGHT_CONFIDENCE_SEGMENT_WIDTH
        + (segment_count - 1.0) * CALL_REVIEW_SPOTLIGHT_CONFIDENCE_SEGMENT_GAP;
    let hover = format!(
        "Assessor confidence: {} · bar highlights verdict severity tier",
        confidence_label(confidence)
    );
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, CALL_REVIEW_SPOTLIGHT_CONFIDENCE_SEGMENT_HEIGHT),
        egui::Sense::hover(),
    );
    response.on_hover_text(hover);

    let mut left = rect.left();
    for tier in 0..SPOTLIGHT_VERDICT_BAR_TIER_COUNT {
        let segment_rect = egui::Rect::from_min_size(
            egui::pos2(left, rect.top()),
            egui::vec2(
                CALL_REVIEW_SPOTLIGHT_CONFIDENCE_SEGMENT_WIDTH,
                CALL_REVIEW_SPOTLIGHT_CONFIDENCE_SEGMENT_HEIGHT,
            ),
        );
        let fill = spotlight_verdict_bar_segment_fill(
            ui,
            spotlight_verdict_bar_tier_emphasis(tier),
            tier == active_tier,
        );
        ui.painter().rect_filled(segment_rect, 1.0, fill);
        left += CALL_REVIEW_SPOTLIGHT_CONFIDENCE_SEGMENT_WIDTH
            + CALL_REVIEW_SPOTLIGHT_CONFIDENCE_SEGMENT_GAP;
    }
}

fn render_call_review_spotlight_signals(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    content_width: f32,
    signals: &ploke_protocol::LocalAnalysisSignals,
) {
    let row_width = call_review_assessment_spotlight_row_width(ui, content_width);
    let narrow = row_width < CALL_REVIEW_SPOTLIGHT_SIGNALS_NARROW_WIDTH;
    ui.allocate_ui_with_layout(
        egui::vec2(row_width, 0.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            ui.set_max_width(row_width);
            ui.set_width(row_width);
            ui.horizontal(|ui| {
                ui.set_max_width(row_width);
                cached_label(ui, render_cache, "signals");
            });
            let mut render_chips = |ui: &mut egui::Ui| {
                ui.set_max_width(row_width);
                ui.set_width(row_width);
                render_call_review_spotlight_signal_chip(
                    ui,
                    render_cache,
                    "repeated tools",
                    signals.repeated_tool_name_count,
                );
                render_call_review_spotlight_signal_chip(
                    ui,
                    render_cache,
                    "distinct tools",
                    signals.distinct_tool_count,
                );
                render_call_review_spotlight_signal_chip(
                    ui,
                    render_cache,
                    "similar searches",
                    signals.similar_search_neighbors,
                );
                render_call_review_spotlight_signal_chip(
                    ui,
                    render_cache,
                    "directory pivots",
                    signals.directory_pivots,
                );
                render_call_review_spotlight_signal_chip(
                    ui,
                    render_cache,
                    "scope turns",
                    signals.scope_turn_count,
                );
                render_call_review_spotlight_signal_chip(
                    ui,
                    render_cache,
                    "search calls",
                    signals.search_calls_in_scope,
                );
                render_call_review_spotlight_signal_chip(
                    ui,
                    render_cache,
                    "read calls",
                    signals.read_calls_in_scope,
                );
                render_call_review_spotlight_signal_chip(
                    ui,
                    render_cache,
                    "browse calls",
                    signals.browse_calls_in_scope,
                );
                render_call_review_spotlight_signal_chip(
                    ui,
                    render_cache,
                    "edit calls",
                    signals.edit_calls_in_scope,
                );
                render_call_review_spotlight_signal_chip(
                    ui,
                    render_cache,
                    "execute calls",
                    signals.execute_calls_in_scope,
                );
                render_call_review_spotlight_signal_chip(
                    ui,
                    render_cache,
                    "failed calls",
                    signals.failed_calls_in_scope,
                );
                if let Some(value) = signals.uncovered_calls_in_source {
                    render_call_review_spotlight_signal_chip(
                        ui,
                        render_cache,
                        "uncovered calls",
                        value,
                    );
                }
                if let Some(value) = signals.labeled_segments_in_source {
                    render_call_review_spotlight_signal_chip(
                        ui,
                        render_cache,
                        "labeled segments",
                        value,
                    );
                }
                if let Some(value) = signals.ambiguous_segments_in_source {
                    render_call_review_spotlight_signal_chip(
                        ui,
                        render_cache,
                        "ambiguous segments",
                        value,
                    );
                }
            };
            if narrow {
                render_chips(ui);
            } else {
                ui.horizontal_wrapped(render_chips);
            }
        },
    );
}

fn scan_emphasis_for_local_analysis_signal_metric(metric: &str, value: usize) -> ScanValueEmphasis {
    match effective_tone_for_local_analysis_signal_metric(metric, value) {
        RunDashboardValueTone::Warn => ScanValueEmphasis::Warn,
        RunDashboardValueTone::Error => ScanValueEmphasis::Error,
        RunDashboardValueTone::Neutral | RunDashboardValueTone::Positive => {
            ScanValueEmphasis::Normal
        }
    }
}

fn render_call_review_spotlight_signal_chip(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    metric: &str,
    value: usize,
) {
    let short_metric = call_review_spotlight_signal_metric_short_label(metric);
    let mut count_buffer = itoa::Buffer::new();
    let count = count_buffer.format(value);
    let mut chip_buffer = String::new();
    let chip_text = format_assessment_dimension_chip(short_metric, count, &mut chip_buffer);
    let emphasis = scan_emphasis_for_local_analysis_signal_metric(metric, value);
    let hover = format!("{metric}: {value}");
    ui.group(|ui| {
        scan_value_label(ui, render_cache, chip_text, emphasis).on_hover_text(hover);
    });
}

fn render_call_review_spotlight_rationale_bullet(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    content_width: f32,
    category: &str,
    rationale: &str,
) {
    let copy_value = CopyableText::new(rationale);
    let row_width = call_review_assessment_spotlight_row_width(ui, content_width);
    ui.horizontal_top(|ui| {
        ui.add_space(CALL_REVIEW_SPOTLIGHT_RATIONALE_LABEL_INDENT);
        let bullet = ui.label(egui::RichText::new("•").weak());
        let content_width =
            (row_width - bullet.rect.width() - ui.spacing().item_spacing.x).max(1.0);
        ui.vertical(|ui| {
            ui.set_max_width(content_width);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(category).strong());
                id_display::copy_button(ui, &copy_value);
            });
            let response = render_inspector_wrapped_prose_in_width(
                ui,
                render_cache,
                rationale,
                content_width,
                egui::Sense::click(),
            );
            id_display::attach_copy_context_menu(&response, &copy_value);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::{
        bounded_utf8_prefix, call_review_spotlight_has_dimension_rationale,
        call_review_spotlight_has_structured_assessment,
        call_review_spotlight_signal_metric_short_label,
        call_review_spotlight_synthesis_is_mechanized_wire_format, confidence_label,
        format_assessment_dimension_chip, recoverability_verdict_emphasis,
        recoverability_verdict_label, redundancy_verdict_emphasis, redundancy_verdict_label,
        spotlight_verdict_bar_active_tier, spotlight_verdict_bar_good_tier_fill,
        usefulness_verdict_emphasis, usefulness_verdict_label,
    };
    use ploke_protocol::{
        Confidence, LocalAnalysisAssessment, LocalAnalysisPacket, LocalAnalysisSignals,
        LocalAnalysisTargetKind, RecoverabilityAssessment, RecoverabilityVerdict,
        RedundancyAssessment, RedundancyVerdict, UsefulnessAssessment, UsefulnessVerdict,
    };

    fn minimal_local_analysis_assessment(
        synthesis_rationale: &str,
        usefulness_rationale: &str,
    ) -> LocalAnalysisAssessment {
        LocalAnalysisAssessment {
            packet: LocalAnalysisPacket {
                subject_id: "test".to_string(),
                target_kind: LocalAnalysisTargetKind::FocalCall,
                target_id: "0".to_string(),
                scope_summary: String::new(),
                total_calls_in_scope: 1,
                total_calls_in_run: 1,
                turn_span: vec![0],
                focal_call_index: Some(0),
                segment_index: None,
                segment_status: None,
                segment_label: None,
                calls: Vec::new(),
            },
            signals: LocalAnalysisSignals {
                scope_turn_count: 1,
                repeated_tool_name_count: 0,
                distinct_tool_count: 1,
                search_calls_in_scope: 0,
                read_calls_in_scope: 0,
                browse_calls_in_scope: 0,
                edit_calls_in_scope: 0,
                execute_calls_in_scope: 0,
                failed_calls_in_scope: 0,
                similar_search_neighbors: 0,
                directory_pivots: 0,
                labeled_segments_in_source: None,
                ambiguous_segments_in_source: None,
                uncovered_calls_in_source: None,
                candidate_concerns: Vec::new(),
            },
            usefulness: UsefulnessAssessment {
                verdict: UsefulnessVerdict::HelpfulButNonEssential,
                confidence: Confidence::High,
                rationale: usefulness_rationale.to_string(),
            },
            redundancy: RedundancyAssessment {
                verdict: RedundancyVerdict::Distinct,
                confidence: Confidence::High,
                rationale: String::new(),
            },
            recoverability: RecoverabilityAssessment {
                verdict: RecoverabilityVerdict::NoRecoveryNeeded,
                confidence: Confidence::High,
                rationale: String::new(),
            },
            overall: ploke_protocol::OverallVerdict::FocusedProgress,
            overall_confidence: Confidence::High,
            synthesis_rationale: synthesis_rationale.to_string(),
        }
    }

    #[test]
    fn call_review_spotlight_structured_assessment_when_dimension_rationale_present() {
        let output = minimal_local_analysis_assessment("ignored", "branch rationale prose");
        assert!(call_review_spotlight_has_dimension_rationale(&output));
        assert!(call_review_spotlight_has_structured_assessment(&output));
    }

    #[test]
    fn call_review_spotlight_structured_assessment_when_synthesis_is_mechanized_wire_format() {
        let wire = "usefulness=HelpfulButNonEssential (High); redundancy=Distinct (High); \
recoverability=NoRecoveryNeeded (High). signals: repeated_tool_name_count=0, \
distinct_tool_count=1, similar_search_neighbors=0, directory_pivots=0, \
uncovered_calls_in_source=None. branch rationales: usefulness='' redundancy='' recoverability=''.";
        let output = minimal_local_analysis_assessment(wire, "");
        assert!(!call_review_spotlight_has_dimension_rationale(&output));
        assert!(call_review_spotlight_synthesis_is_mechanized_wire_format(
            wire
        ));
        assert!(call_review_spotlight_has_structured_assessment(&output));
    }

    #[test]
    fn call_review_spotlight_raw_synthesis_fallback_when_unstructured_only() {
        let output = minimal_local_analysis_assessment("Operator-only synthesis note.", "");
        assert!(!call_review_spotlight_has_structured_assessment(&output));
    }

    #[test]
    fn call_review_spotlight_signal_metric_short_labels_match_dashboard() {
        assert_eq!(
            call_review_spotlight_signal_metric_short_label("scope turns"),
            "turns"
        );
        assert_eq!(
            call_review_spotlight_signal_metric_short_label("search calls"),
            "search"
        );
        assert_eq!(
            call_review_spotlight_signal_metric_short_label("directory pivots"),
            "pivots"
        );
    }

    #[test]
    fn assessment_dimension_chip_label_includes_dimension_prefix() {
        let mut buffer = String::new();
        assert_eq!(
            format_assessment_dimension_chip("usefulness", "none", &mut buffer),
            "usefulness: none"
        );
        assert_eq!(
            format_assessment_dimension_chip("outcome", "recoverable", &mut buffer),
            "outcome: recoverable"
        );
    }

    #[test]
    fn call_review_hover_preview_respects_utf8_boundaries() {
        let (short, truncated) = bounded_utf8_prefix("focused progress", 220);
        assert_eq!(short, "focused progress");
        assert!(!truncated);

        let text = "abcdéfg";
        let (prefix, truncated) = bounded_utf8_prefix(text, 5);
        assert_eq!(prefix, "abcd");
        assert!(truncated);
    }

    #[test]
    fn call_review_spotlight_verdict_labels_are_short() {
        assert_eq!(
            usefulness_verdict_label(UsefulnessVerdict::HelpfulButNonEssential),
            "helpful"
        );
        assert_eq!(
            redundancy_verdict_label(RedundancyVerdict::Distinct),
            "distinct"
        );
        assert_eq!(
            recoverability_verdict_label(RecoverabilityVerdict::NoRecoveryNeeded),
            "ok"
        );
    }

    #[test]
    fn call_review_spotlight_confidence_labels_match_protocol() {
        assert_eq!(confidence_label(Confidence::Low), "low");
        assert_eq!(confidence_label(Confidence::Medium), "medium");
        assert_eq!(confidence_label(Confidence::High), "high");
    }

    #[test]
    fn spotlight_verdict_bar_good_tier_fill_uses_success_semantic() {
        use crate::ui::theme::NamedScheme;
        for scheme in [
            NamedScheme::TokyoNight,
            NamedScheme::Dracula,
            NamedScheme::GruvboxLight,
        ] {
            let tokens = scheme.tokens();
            assert_eq!(
                spotlight_verdict_bar_good_tier_fill(tokens),
                tokens.success,
                "{scheme:?}"
            );
        }
    }

    #[test]
    fn spotlight_verdict_bar_active_tier_tracks_usefulness_emphasis() {
        use ploke_protocol::UsefulnessVerdict::{
            HelpfulButNonEssential, KeyProgress, LowValue, NoValue, Unclear,
        };
        assert_eq!(
            spotlight_verdict_bar_active_tier(usefulness_verdict_emphasis(NoValue)),
            0
        );
        assert_eq!(
            spotlight_verdict_bar_active_tier(usefulness_verdict_emphasis(LowValue)),
            1
        );
        assert_eq!(
            spotlight_verdict_bar_active_tier(usefulness_verdict_emphasis(Unclear)),
            1
        );
        assert_eq!(
            spotlight_verdict_bar_active_tier(usefulness_verdict_emphasis(HelpfulButNonEssential)),
            2
        );
        assert_eq!(
            spotlight_verdict_bar_active_tier(usefulness_verdict_emphasis(KeyProgress)),
            2
        );
    }

    #[test]
    fn spotlight_verdict_bar_active_tier_tracks_redundancy_emphasis() {
        use ploke_protocol::RedundancyVerdict::{
            Distinct, Overlapping, RedundantRepeat, SearchThrash, Unclear,
        };
        assert_eq!(
            spotlight_verdict_bar_active_tier(redundancy_verdict_emphasis(RedundantRepeat)),
            0
        );
        assert_eq!(
            spotlight_verdict_bar_active_tier(redundancy_verdict_emphasis(SearchThrash)),
            0
        );
        assert_eq!(
            spotlight_verdict_bar_active_tier(redundancy_verdict_emphasis(Unclear)),
            1
        );
        assert_eq!(
            spotlight_verdict_bar_active_tier(redundancy_verdict_emphasis(Distinct)),
            2
        );
        assert_eq!(
            spotlight_verdict_bar_active_tier(redundancy_verdict_emphasis(Overlapping)),
            2
        );
    }

    #[test]
    fn spotlight_verdict_bar_active_tier_tracks_recoverability_emphasis() {
        use ploke_protocol::RecoverabilityVerdict::{
            ClearNextStep, NoClearRecovery, NoRecoveryNeeded, PartialNextStep, Unclear,
        };
        assert_eq!(
            spotlight_verdict_bar_active_tier(recoverability_verdict_emphasis(NoClearRecovery)),
            0
        );
        assert_eq!(
            spotlight_verdict_bar_active_tier(recoverability_verdict_emphasis(PartialNextStep)),
            1
        );
        assert_eq!(
            spotlight_verdict_bar_active_tier(recoverability_verdict_emphasis(Unclear)),
            1
        );
        assert_eq!(
            spotlight_verdict_bar_active_tier(recoverability_verdict_emphasis(NoRecoveryNeeded)),
            2
        );
        assert_eq!(
            spotlight_verdict_bar_active_tier(recoverability_verdict_emphasis(ClearNextStep)),
            2
        );
    }
}
