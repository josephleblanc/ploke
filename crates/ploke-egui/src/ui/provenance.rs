//! Evidence provenance lanes for operator-facing failure and decode labels.

use eframe::egui;
use ploke_records::run_record::{ToolExecutionRecord, ToolResult};

use crate::ui::theme::{PaletteTokens, tokens_from_ui};

/// Which layer produced the evidence the operator is reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceLane {
    /// Snapshot fetch / JSON import / graph build in egui.
    UiSnapshotLoad,
    /// WASM cannot decode tool args/results (not a harness failure).
    UiDecodeUnavailable,
    /// Native JSON decode of tool args/results failed.
    UiDecodeFailed,
    /// Native typed decode succeeded.
    UiDecodeOk,
    /// Failure recorded when the agent ran (RunRecord / ToolUiPayload).
    RecordedHarnessFailure,
    /// Tool step completed in the export.
    RecordedToolCompleted,
    /// Typed `ToolErrorWire` on the tool UI payload.
    RecordedTypedErrorWire,
    /// Raw error or content payload from the export.
    RecordedRawPayload,
    /// Typed payload exists but graph/export is partial.
    GraphWitnessGap,
}

impl EvidenceLane {
    pub const fn chip_text(self) -> &'static str {
        match self {
            Self::UiSnapshotLoad => "UI · snapshot load",
            Self::UiDecodeUnavailable => "UI · decode (WASM)",
            Self::UiDecodeFailed => "UI · decode failed",
            Self::UiDecodeOk => "decode ok",
            Self::RecordedHarnessFailure => "Recorded · tool failed",
            Self::RecordedToolCompleted => "Recorded · completed",
            Self::RecordedTypedErrorWire => "Recorded · typed error wire",
            Self::RecordedRawPayload => "Recorded · raw payload",
            Self::GraphWitnessGap => "Graph · not in export",
        }
    }

    pub const fn tooltip(self) -> &'static str {
        match self {
            Self::UiSnapshotLoad => {
                "The browser or native UI failed to fetch or import this graph snapshot; tool rows below are not trustworthy until load succeeds."
            }
            Self::UiDecodeUnavailable => {
                "WASM build cannot run native tool decoders; raw JSON is still valid export evidence—the harness did not fail because of WASM."
            }
            Self::UiDecodeFailed => {
                "Native UI could not decode persisted tool JSON into typed fields; see raw payload for the recorded bytes."
            }
            Self::UiDecodeOk => {
                "Native UI decoded persisted tool JSON into typed fields for scanability."
            }
            Self::RecordedHarnessFailure => {
                "Failure recorded when the agent ran the tool; trust this headline and typed error wire over UI decode labels."
            }
            Self::RecordedToolCompleted => "Tool step completed in the exported run record.",
            Self::RecordedTypedErrorWire => {
                "Structured tool error exported with the run record (user + system + LLM wire)."
            }
            Self::RecordedRawPayload => {
                "Raw error or result bytes from the export, not reinterpreted by the UI."
            }
            Self::GraphWitnessGap => {
                "Expected graph witness missing from this snapshot; UI cannot prove the fact from the loaded graph."
            }
        }
    }

    pub fn chip_colors(self, ui: &egui::Ui) -> (egui::Color32, egui::Color32) {
        self.chip_colors_from_tokens(tokens_from_ui(ui))
    }

    /// Fill + label colors for lane chips (semantic fill + contrasting label).
    pub fn chip_colors_from_tokens(self, tokens: PaletteTokens) -> (egui::Color32, egui::Color32) {
        let fill = match self {
            Self::UiSnapshotLoad | Self::UiDecodeFailed | Self::RecordedHarnessFailure => {
                tokens.error
            }
            Self::UiDecodeUnavailable | Self::GraphWitnessGap => tokens.warning,
            Self::UiDecodeOk | Self::RecordedToolCompleted => tokens.success,
            Self::RecordedTypedErrorWire => tokens.warning,
            Self::RecordedRawPayload => tokens.accent,
        };
        (fill, chip_label_for_fill(fill, tokens))
    }

    pub fn lane_for_tool_result(tool: &ToolExecutionRecord) -> Self {
        match &tool.result {
            ToolResult::Completed(_) => Self::RecordedToolCompleted,
            ToolResult::Failed(_) => Self::RecordedHarnessFailure,
        }
    }
}

fn chip_label_for_fill(fill: egui::Color32, tokens: PaletteTokens) -> egui::Color32 {
    let badge = wcag_contrast_ratio(tokens.badge_text, fill);
    let body = wcag_contrast_ratio(tokens.text, fill);
    if badge > body {
        tokens.badge_text
    } else {
        tokens.text
    }
}

fn wcag_contrast_ratio(foreground: egui::Color32, background: egui::Color32) -> f32 {
    let fg = relative_luminance(foreground);
    let bg = relative_luminance(background);
    let (hi, lo) = if fg > bg { (fg, bg) } else { (bg, fg) };
    (hi + 0.05) / (lo + 0.05)
}

fn relative_luminance(color: egui::Color32) -> f32 {
    fn channel_luminance(channel: u8) -> f32 {
        let c = channel as f32 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel_luminance(color.r())
        + 0.7152 * channel_luminance(color.g())
        + 0.0722 * channel_luminance(color.b())
}

/// Export anchors for run-record tool step provenance popovers.
#[derive(Debug, Clone, Copy)]
pub struct RunRecordProvenanceCtx<'a> {
    pub manifest_id: &'a str,
    pub record_path: &'a str,
}

/// Operator-facing provenance for the inspect popover (no graph re-parse).
#[derive(Debug, Clone, Copy)]
pub struct ProvenanceDetail<'a> {
    pub lane: EvidenceLane,
    pub headline_field: Option<&'static str>,
    pub call_id: Option<&'a str>,
    pub tool_name: Option<&'a str>,
    pub step_index: Option<usize>,
    pub manifest_id: Option<&'a str>,
    pub record_path: Option<&'a str>,
    pub snapshot_label: Option<&'a str>,
    pub load_error: Option<&'a str>,
}

/// Which export field drives the harness headline for a failed tool.
pub fn headline_field_label(tool: &ToolExecutionRecord) -> Option<&'static str> {
    let ToolResult::Failed(result) = &tool.result else {
        return None;
    };
    if let Some(payload) = result.ui_payload.as_ref() {
        if let Some(error) = payload.error.as_ref() {
            if !error.user.is_empty() {
                return Some("error.user");
            }
        }
        if !payload.summary.is_empty() {
            return Some("ui_payload.summary");
        }
    }
    Some("result.error")
}

/// Prefer `error.user`, then `ui_payload.summary`, then `result.error`.
pub fn tool_failure_headline(tool: &ToolExecutionRecord) -> &str {
    let ToolResult::Failed(result) = &tool.result else {
        return "";
    };
    if let Some(payload) = result.ui_payload.as_ref() {
        if let Some(error) = payload.error.as_ref() {
            if !error.user.is_empty() {
                return error.user.as_str();
            }
        }
        if !payload.summary.is_empty() {
            return payload.summary.as_str();
        }
    }
    result.error.as_str()
}

pub fn detail_for_tool_failure<'a>(
    tool: &'a ToolExecutionRecord,
    step_index: usize,
    record_ctx: Option<RunRecordProvenanceCtx<'a>>,
) -> ProvenanceDetail<'a> {
    ProvenanceDetail {
        lane: EvidenceLane::lane_for_tool_result(tool),
        headline_field: headline_field_label(tool),
        call_id: Some(tool.request.call_id.as_str()),
        tool_name: Some(tool.request.tool.as_str()),
        step_index: Some(step_index),
        manifest_id: record_ctx.map(|ctx| ctx.manifest_id),
        record_path: record_ctx.map(|ctx| ctx.record_path),
        snapshot_label: None,
        load_error: None,
    }
}

pub fn detail_for_lane<'a>(
    lane: EvidenceLane,
    record_ctx: Option<RunRecordProvenanceCtx<'a>>,
) -> ProvenanceDetail<'a> {
    detail_for_lane_at_step(lane, record_ctx, None, None, None)
}

pub fn detail_for_lane_at_step<'a>(
    lane: EvidenceLane,
    record_ctx: Option<RunRecordProvenanceCtx<'a>>,
    call_id: Option<&'a str>,
    tool_name: Option<&'a str>,
    step_index: Option<usize>,
) -> ProvenanceDetail<'a> {
    ProvenanceDetail {
        lane,
        headline_field: None,
        call_id,
        tool_name,
        step_index,
        manifest_id: record_ctx.map(|ctx| ctx.manifest_id),
        record_path: record_ctx.map(|ctx| ctx.record_path),
        snapshot_label: None,
        load_error: None,
    }
}

pub fn detail_for_snapshot_load<'a>(
    load_error: &'a str,
    source_label: Option<&'a str>,
) -> ProvenanceDetail<'a> {
    ProvenanceDetail {
        lane: EvidenceLane::UiSnapshotLoad,
        headline_field: None,
        call_id: None,
        tool_name: None,
        step_index: None,
        manifest_id: None,
        record_path: None,
        snapshot_label: source_label,
        load_error: Some(load_error),
    }
}

pub fn render_evidence_lane_chip_with_inspect(
    ui: &mut egui::Ui,
    lane: EvidenceLane,
    detail: ProvenanceDetail<'_>,
    id_salt: impl std::hash::Hash + Copy,
) {
    ui.horizontal(|ui| {
        render_evidence_lane_chip(ui, lane);
        render_provenance_inspect_button(ui, id_salt, detail);
    });
}

pub fn render_provenance_inspect_button(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    detail: ProvenanceDetail<'_>,
) {
    let popup_id = ui.make_persistent_id(("provenance-inspect", id_salt));
    let response = provenance_inspect_icon_button(ui);
    if response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
    }
    let _ = egui::popup_below_widget(
        ui,
        popup_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| render_provenance_popover(ui, detail),
    );
}

fn provenance_inspect_icon_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
    let response = response.on_hover_text("Inspect error provenance");
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        ui.painter().rect_filled(rect, 2.0, visuals.weak_bg_fill);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "i",
            egui::FontId::proportional(12.0),
            visuals.text_color(),
        );
    }
    response
}

fn render_provenance_popover(ui: &mut egui::Ui, detail: ProvenanceDetail<'_>) {
    ui.set_max_width(420.0);
    ui.heading("Error provenance");
    ui.separator();
    ui.label(egui::RichText::new("Lane").strong());
    render_evidence_lane_chip(ui, detail.lane);
    ui.label(detail.lane.tooltip());
    ui.separator();
    ui.label(egui::RichText::new("Trust").strong());
    ui.label(trust_line_for_lane(detail.lane));
    if let Some(field) = detail.headline_field {
        ui.separator();
        ui.label(egui::RichText::new("Headline source").strong());
        ui.monospace(field);
    }
    ui.separator();
    ui.label(egui::RichText::new("Export anchor").strong());
    render_export_anchor_rows(ui, detail);
    ui.separator();
    ui.label(egui::RichText::new("jq hint").strong());
    let jq_hint = jq_hint_template(detail);
    render_wrapped_monospace_block(ui, &jq_hint);
    render_provenance_copy_row(ui, "Copy jq hint", jq_hint);
    ui.separator();
    ui.label(egui::RichText::new("Environment").strong());
    ui.horizontal(|ui| {
        ui.label("Host:");
        ui.monospace(host_label());
        ui.label("Decode:");
        ui.monospace(decode_capability_label());
    });
}

fn trust_line_for_lane(lane: EvidenceLane) -> &'static str {
    match lane {
        EvidenceLane::UiSnapshotLoad => {
            "Trust the snapshot load layer: fix import/URL before interpreting tool rows below."
        }
        EvidenceLane::UiDecodeUnavailable => {
            "Trust export JSON and recorded harness fields; WASM decode limits are not harness failures."
        }
        EvidenceLane::UiDecodeFailed => {
            "Trust raw persisted tool JSON when native decode fails; harness headline is separate."
        }
        EvidenceLane::UiDecodeOk => {
            "Native decode is for scanability; recorded harness and export payloads remain authoritative."
        }
        EvidenceLane::RecordedHarnessFailure => {
            "Trust the recorded harness failure and typed error wire over UI decode labels."
        }
        EvidenceLane::RecordedToolCompleted => {
            "Trust the exported completed tool result and UI payload witnesses."
        }
        EvidenceLane::RecordedTypedErrorWire => {
            "Trust structured tool error wire from the export (user line first in drilldown)."
        }
        EvidenceLane::RecordedRawPayload => {
            "Trust persisted raw bytes; the UI does not reinterpret them."
        }
        EvidenceLane::GraphWitnessGap => {
            "Do not infer missing facts; load a richer snapshot or disk witness."
        }
    }
}

fn render_export_anchor_rows(ui: &mut egui::Ui, detail: ProvenanceDetail<'_>) {
    if let Some(label) = detail.snapshot_label {
        ui.horizontal(|ui| {
            ui.label("Source:");
            ui.monospace(label);
        });
    }
    if let Some(error) = detail.load_error {
        ui.horizontal(|ui| {
            ui.label("Load error:");
            ui.label(error);
        });
    }
    if let Some(call_id) = detail.call_id {
        ui.horizontal(|ui| {
            ui.label("call_id:");
            ui.monospace(call_id);
            render_provenance_copy_row(ui, "Copy call_id", call_id.to_owned());
        });
    }
    if let Some(tool) = detail.tool_name {
        ui.horizontal(|ui| {
            ui.label("tool:");
            ui.monospace(tool);
        });
    }
    if let Some(step) = detail.step_index {
        ui.horizontal(|ui| {
            ui.label("step:");
            ui.monospace((step + 1).to_string());
        });
    }
    if let Some(manifest_id) = detail.manifest_id {
        ui.horizontal(|ui| {
            ui.label("manifest_id:");
            ui.monospace(manifest_id);
        });
    }
    if let Some(record_path) = detail.record_path {
        ui.horizontal(|ui| {
            ui.label("record_path:");
            ui.monospace(record_path_tail(record_path));
            render_provenance_copy_row(ui, "Copy record_path", record_path.to_owned());
        });
    }
    if detail.call_id.is_none()
        && detail.tool_name.is_none()
        && detail.step_index.is_none()
        && detail.manifest_id.is_none()
        && detail.record_path.is_none()
        && detail.snapshot_label.is_none()
        && detail.load_error.is_none()
    {
        ui.label(egui::RichText::new("—").weak());
    }
}

fn record_path_tail(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn jq_hint_template(detail: ProvenanceDetail<'_>) -> String {
    let record_key = detail
        .record_path
        .map(record_path_tail)
        .unwrap_or("<record>");
    // Multiline clipboard text: jq accepts newline-separated filters; one logical
    // segment per line so the popover can lay out without galley wrap mid-token.
    let mut hint = format!(
        "jq '…run_records.index[\"{record_key}\"]\n  .phases.agent_turns[]\n  .tool_calls[]"
    );
    if detail.lane == EvidenceLane::RecordedHarnessFailure {
        hint.push_str("\n  | select(.result.Failed?)");
    }
    if let Some(call_id) = detail.call_id {
        hint.push_str(&format!(
            "\n  | select(.request.call_id==\n    \"{call_id}\")"
        ));
    }
    hint.push_str("\n  …'");
    hint
}

fn render_wrapped_monospace_block(ui: &mut egui::Ui, text: &str) {
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
    let color = crate::ui::theme::tokens_from_ui(ui).text;
    ui.vertical(|ui| {
        for line in text.split('\n') {
            let galley =
                ui.fonts_mut(|fonts| fonts.layout_no_wrap(line.to_owned(), font_id.clone(), color));
            ui.add(egui::Label::new(galley).selectable(true));
        }
    });
}

fn render_provenance_copy_row(ui: &mut egui::Ui, label: &str, value: String) {
    if ui.small_button(label).clicked() {
        ui.ctx().copy_text(value);
    }
}

pub fn render_evidence_lane_chip(ui: &mut egui::Ui, lane: EvidenceLane) {
    let (fill, text_color) = lane.chip_colors(ui);
    ui.label(
        egui::RichText::new(lane.chip_text())
            .background_color(fill)
            .color(text_color)
            .monospace()
            .size(11.0),
    )
    .on_hover_text(lane.tooltip());
}

#[cfg(target_arch = "wasm32")]
pub const fn host_label() -> &'static str {
    "WASM"
}

#[cfg(not(target_arch = "wasm32"))]
pub const fn host_label() -> &'static str {
    "native"
}

#[cfg(target_arch = "wasm32")]
pub const fn decode_capability_label() -> &'static str {
    "limited (WASM)"
}

#[cfg(not(target_arch = "wasm32"))]
pub const fn decode_capability_label() -> &'static str {
    "full (native)"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::NamedScheme;
    use ploke_records::agent_turn::{
        ToolErrorCodeRecord, ToolErrorWireRecord, ToolLlmErrorPayloadRecord, ToolUiPayloadRecord,
        ToolVerbosityRecord,
    };
    use ploke_records::agent_turn::{ToolFailedRecord, ToolRequestRecord};
    use ploke_records::run_record::{ToolExecutionRecord, ToolResult};
    use ploke_records::tool_contracts::ToolName;

    fn wcag_contrast_ratio(foreground: egui::Color32, background: egui::Color32) -> f32 {
        super::wcag_contrast_ratio(foreground, background)
    }

    /// Regression: success fill + `text` label was unreadable on Tokyo Night (list_dir completed).
    #[test]
    fn recorded_tool_completed_chip_uses_badge_text_on_tokyo_night() {
        let tokens = NamedScheme::TokyoNight.tokens();
        let (fill, text) = EvidenceLane::RecordedToolCompleted.chip_colors_from_tokens(tokens);
        let old_ratio = wcag_contrast_ratio(tokens.text, fill);
        let new_ratio = wcag_contrast_ratio(text, fill);
        assert_eq!(
            text, tokens.badge_text,
            "light success fill needs dark badge label"
        );
        assert!(
            new_ratio >= 4.5,
            "completed tool chip should be clearly readable: {new_ratio:.2}"
        );
        assert!(
            new_ratio > old_ratio * 1.5,
            "regression: old pair {old_ratio:.2}, new {new_ratio:.2}"
        );
    }

    /// Regression: `text_muted` fill + `text` label was unreadable on Tokyo Night (user report).
    #[test]
    fn recorded_typed_error_wire_contrast_beats_muted_pair_on_tokyo_night() {
        let tokens = NamedScheme::TokyoNight.tokens();
        let (fill, text) = EvidenceLane::RecordedTypedErrorWire.chip_colors_from_tokens(tokens);
        let old_ratio = wcag_contrast_ratio(tokens.text, tokens.text_muted);
        let new_ratio = wcag_contrast_ratio(text, fill);
        assert!(
            new_ratio >= 4.5,
            "typed error wire chip should be clearly readable: {new_ratio:.2}"
        );
        assert!(
            new_ratio > old_ratio * 2.0,
            "regression: old pair {old_ratio:.2}, new {new_ratio:.2}"
        );
    }

    #[test]
    fn evidence_lane_chip_colors_meet_contrast_on_all_schemes() {
        // 11px monospace chips: WCAG AA large-text minimum (3:1), not 4.5 body text.
        const MIN_CONTRAST: f32 = 3.0;
        let lanes = [
            EvidenceLane::RecordedTypedErrorWire,
            EvidenceLane::RecordedRawPayload,
            EvidenceLane::RecordedToolCompleted,
            EvidenceLane::UiDecodeOk,
            EvidenceLane::GraphWitnessGap,
        ];
        for scheme in [
            NamedScheme::TokyoNight,
            NamedScheme::Dracula,
            NamedScheme::GruvboxDark,
            NamedScheme::OneDark,
            NamedScheme::GruvboxLight,
            NamedScheme::OneLight,
        ] {
            let tokens = scheme.tokens();
            for lane in lanes {
                let (fill, text) = lane.chip_colors_from_tokens(tokens);
                let ratio = wcag_contrast_ratio(text, fill);
                assert!(
                    ratio >= MIN_CONTRAST,
                    "lane {:?} on {:?}: contrast {ratio:.2} < {MIN_CONTRAST} (fill={fill:?}, text={text:?})",
                    lane,
                    scheme,
                );
            }
        }
    }

    #[test]
    fn evidence_lane_chip_text_is_stable() {
        assert_eq!(
            EvidenceLane::UiDecodeUnavailable.chip_text(),
            "UI · decode (WASM)"
        );
        assert_eq!(
            EvidenceLane::RecordedHarnessFailure.chip_text(),
            "Recorded · tool failed"
        );
        assert_eq!(
            EvidenceLane::UiSnapshotLoad.chip_text(),
            "UI · snapshot load"
        );
    }

    fn failed_tool(user: Option<&str>, summary: &str, error: &str) -> ToolExecutionRecord {
        let ui_payload = ToolUiPayloadRecord {
            tool: ToolName::RequestCodeContext,
            call_id: "c1".to_owned(),
            request_id: None,
            proposal_id: None,
            summary: summary.to_owned(),
            fields: vec![],
            details: None,
            verbosity: ToolVerbosityRecord::Normal,
            error: user.map(|user| ToolErrorWireRecord {
                user: user.to_owned(),
                system: "sys".to_owned(),
                llm: ToolLlmErrorPayloadRecord {
                    ok: false,
                    tool: ToolName::RequestCodeContext,
                    code: ToolErrorCodeRecord::Io,
                    field: None,
                    expected: None,
                    received: None,
                    message: "m".to_owned(),
                    snippet: None,
                    retry_hint: None,
                    retry_context: None,
                },
            }),
            error_code: None,
        };
        ToolExecutionRecord {
            request: ToolRequestRecord {
                request_id: "r1".to_owned(),
                parent_id: "p1".to_owned(),
                call_id: "c1".to_owned(),
                tool: "request_code_context".to_owned(),
                arguments: "{}".into(),
            },
            result: ToolResult::Failed(ToolFailedRecord {
                request_id: "r1".to_owned(),
                parent_id: "p1".to_owned(),
                call_id: "c1".to_owned(),
                tool: Some("request_code_context".to_owned()),
                error: error.to_owned(),
                ui_payload: Some(ui_payload),
                latency_ms: 1,
            }),
            latency_ms: 1,
        }
    }

    #[test]
    fn headline_field_label_matches_headline_precedence() {
        let with_user = failed_tool(Some("operator message"), "summary line", "raw err");
        assert_eq!(headline_field_label(&with_user), Some("error.user"));

        let summary_only = failed_tool(None, "summary line", "raw err");
        assert_eq!(
            headline_field_label(&summary_only),
            Some("ui_payload.summary")
        );

        let no_payload = ToolExecutionRecord {
            request: ToolRequestRecord {
                request_id: "r1".to_owned(),
                parent_id: "p1".to_owned(),
                call_id: "c1".to_owned(),
                tool: "t".to_owned(),
                arguments: "{}".into(),
            },
            result: ToolResult::Failed(ToolFailedRecord {
                request_id: "r1".to_owned(),
                parent_id: "p1".to_owned(),
                call_id: "c1".to_owned(),
                tool: None,
                error: "raw err".to_owned(),
                ui_payload: None,
                latency_ms: 0,
            }),
            latency_ms: 0,
        };
        assert_eq!(headline_field_label(&no_payload), Some("result.error"));
    }

    #[test]
    fn detail_for_tool_failure_sets_lane_and_headline_field() {
        let tool = failed_tool(Some("operator message"), "summary", "raw");
        let ctx = RunRecordProvenanceCtx {
            manifest_id: "manifest-1",
            record_path: "/runs/baseline/record.json.gz",
        };
        let detail = detail_for_tool_failure(&tool, 2, Some(ctx));
        assert_eq!(detail.lane, EvidenceLane::RecordedHarnessFailure);
        assert_eq!(detail.headline_field, Some("error.user"));
        assert_eq!(detail.call_id, Some("c1"));
        assert_eq!(detail.step_index, Some(2));
        assert_eq!(detail.manifest_id, Some("manifest-1"));
    }

    #[test]
    fn detail_for_lane_ui_decode_unavailable_has_no_headline_field() {
        let detail = detail_for_lane(EvidenceLane::UiDecodeUnavailable, None);
        assert_eq!(detail.lane, EvidenceLane::UiDecodeUnavailable);
        assert_eq!(detail.headline_field, None);
        assert!(EvidenceLane::UiDecodeUnavailable.tooltip().contains("WASM"));
    }

    #[test]
    fn jq_hint_harness_failure_is_multiline_not_chained() {
        let tool = failed_tool(Some("operator message"), "summary", "raw");
        let detail = detail_for_tool_failure(
            &tool,
            1,
            Some(RunRecordProvenanceCtx {
                manifest_id: "manifest-1",
                record_path: "/runs/baseline/record.json.gz",
            }),
        );
        let hint = jq_hint_template(detail);
        assert!(hint.contains("\n  | select(.result.Failed?)"));
        assert!(
            !hint.contains(".tool_calls[] | "),
            "path and filters should not share one wrapped line: {hint}"
        );
        for line in hint.lines() {
            assert!(
                !line.contains(" | select(.result.Failed?) | "),
                "filters must not chain on one line: {line}"
            );
        }
    }

    #[test]
    fn jq_hint_decode_lane_omits_failed_filter_but_stays_multiline() {
        let detail = detail_for_lane_at_step(
            EvidenceLane::UiDecodeFailed,
            Some(RunRecordProvenanceCtx {
                manifest_id: "manifest-1",
                record_path: "/runs/baseline/record.json.gz",
            }),
            Some("function-call-abc"),
            None,
            Some(0),
        );
        let hint = jq_hint_template(detail);
        assert!(!hint.contains(".result.Failed?"));
        assert!(hint.contains("\n  .phases.agent_turns[]"));
        assert!(hint.contains("\n  | select(.request.call_id==\n    \"function-call-abc\")"));
    }

    #[test]
    fn tool_failure_headline_prefers_user_then_summary_then_error() {
        let with_user = failed_tool(Some("operator message"), "summary line", "raw err");
        assert_eq!(tool_failure_headline(&with_user), "operator message");

        let summary_only = failed_tool(None, "summary line", "raw err");
        assert_eq!(tool_failure_headline(&summary_only), "summary line");

        let no_payload = ToolExecutionRecord {
            request: ToolRequestRecord {
                request_id: "r1".to_owned(),
                parent_id: "p1".to_owned(),
                call_id: "c1".to_owned(),
                tool: "t".to_owned(),
                arguments: "{}".into(),
            },
            result: ToolResult::Failed(ToolFailedRecord {
                request_id: "r1".to_owned(),
                parent_id: "p1".to_owned(),
                call_id: "c1".to_owned(),
                tool: None,
                error: "raw err".to_owned(),
                ui_payload: None,
                latency_ms: 0,
            }),
            latency_ms: 0,
        };
        assert_eq!(tool_failure_headline(&no_payload), "raw err");
    }
}
