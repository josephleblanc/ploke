use crate::ui::render::text::*;
use crate::ui::text::style as text_style;
use eframe::egui;
use ploke_records::protocol::ArtifactBody;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::fields::*;
use super::{
    InspectorRenderCache, render_call_review_reasoning_spotlight,
    selected_eval_protocol_call_review_key, set_selected_eval_protocol_call_review,
    show_inspector_collapsing,
};

const CALL_REVIEW_SCAN_HOVER_PREVIEW_BYTES: usize = 220;

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
        .unwrap_or(CallReviewFilter::All);
    let sort_id = ui.make_persistent_id("eval-protocol-call-review-sort");
    let mut sort = ui
        .data(|data| data.get_temp::<CallReviewSort>(sort_id))
        .unwrap_or_default();

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Call Review Scan").default_open(true),
        |ui| {
            cached_kv_usize(ui, render_cache, "call reviews", counts.total);
            ui.horizontal_wrapped(|ui| {
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
    ui.ctx()
        .pointer_hover_pos()
        .is_some_and(|pos| hit_rect.contains(pos))
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

#[cfg(test)]
mod tests {
    use super::bounded_utf8_prefix;

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
}
