//! Evidence provenance lanes for operator-facing failure and decode labels.

use eframe::egui;
use ploke_records::run_record::{ToolExecutionRecord, ToolResult};

use crate::ui::theme::tokens_from_ui;

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
        let tokens = tokens_from_ui(ui);
        match self {
            Self::UiSnapshotLoad | Self::UiDecodeFailed | Self::RecordedHarnessFailure => {
                (tokens.error, tokens.badge_text)
            }
            Self::UiDecodeUnavailable => (tokens.warning, tokens.badge_text),
            Self::UiDecodeOk
            | Self::RecordedToolCompleted
            | Self::GraphWitnessGap
            | Self::RecordedTypedErrorWire
            | Self::RecordedRawPayload => (tokens.text_muted, tokens.text),
        }
    }

    pub fn lane_for_tool_result(tool: &ToolExecutionRecord) -> Self {
        match &tool.result {
            ToolResult::Completed(_) => Self::RecordedToolCompleted,
            ToolResult::Failed(_) => Self::RecordedHarnessFailure,
        }
    }
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
    use ploke_records::agent_turn::{
        ToolErrorCodeRecord, ToolErrorWireRecord, ToolLlmErrorPayloadRecord, ToolUiPayloadRecord,
        ToolVerbosityRecord,
    };
    use ploke_records::agent_turn::{ToolFailedRecord, ToolRequestRecord};
    use ploke_records::run_record::{ToolExecutionRecord, ToolResult};
    use ploke_records::tool_contracts::ToolName;

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
