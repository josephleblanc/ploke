use crate::ui::id_display::{self, CopyableExpandable, CopyableText};
use eframe::egui;

use super::call_review::{
    cached_hover_monospace_block, call_review_failure_label, confidence_emphasis, confidence_label,
    failure_emphasis, overall_verdict_emphasis, overall_verdict_label, scan_value_label,
};
use super::eval_protocol;
use super::fields::*;
use super::{InspectorRenderCache, show_inspector_collapsing};
#[cfg(not(target_arch = "wasm32"))]
use super::{render_decoded_tool_arguments, render_decoded_tool_result};

fn render_protocol_artifact_coordinate(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    artifact_key: &str,
    artifact: &ploke_records::protocol::Artifact,
) {
    cached_kv_artifact_file(ui, render_cache, "artifact", artifact_key);
    cached_kv_id(
        ui,
        render_cache,
        "procedure",
        artifact.procedure_name.as_str(),
    );
    cached_kv_id(ui, render_cache, "subject", artifact.subject_id.as_str());
    cached_kv_run_name(ui, render_cache, "run", artifact.run_id.as_str());
    cached_kv_u64(ui, render_cache, "created at ms", artifact.created_at_ms);
    if let Some(model) = artifact.model_id.as_deref() {
        cached_kv_id(ui, render_cache, "model", model);
    }
    if let Some(provider) = artifact.provider_slug.as_deref() {
        cached_kv_id(ui, render_cache, "provider", provider);
    }
}

pub(crate) fn render_tool_call_review_artifact(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    review_index: usize,
    artifact_key: &str,
    artifact: &ploke_records::protocol::Artifact,
    payload: &ploke_records::protocol::ToolCallReviewPayload,
) {
    let focal = &payload.input.focal;
    let title = format!(
        "call {} {} {:?}",
        focal.index, focal.tool_name, payload.output.overall
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt(("tool-call-review", artifact_key, review_index))
            .default_open(false),
        |ui| {
            render_tool_call_review_detail(
                ui,
                render_cache,
                artifact_key,
                artifact,
                payload,
                false,
            );
        },
    );
}

fn render_tool_call_review_detail_header(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    payload: &ploke_records::protocol::ToolCallReviewPayload,
    clearable: bool,
) {
    let focal = &payload.input.focal;
    ui.horizontal_wrapped(|ui| {
        cached_label(ui, render_cache, "selected call");
        let mut index_buffer = itoa::Buffer::new();
        cached_monospace_label(ui, render_cache, index_buffer.format(focal.index))
            .on_hover_text("Focal tool-call index in the reviewed neighborhood.");
        cached_monospace_label(ui, render_cache, focal.tool_name.as_str())
            .on_hover_text("Focal tool name from the protocol review.");
        scan_value_label(
            ui,
            render_cache,
            overall_verdict_label(payload.output.overall),
            overall_verdict_emphasis(payload.output.overall),
        )
        .on_hover_ui(|ui| {
            cached_label(ui, render_cache, "overall");
            cached_label(
                ui,
                render_cache,
                "Overall protocol verdict synthesized from usefulness, redundancy, and recoverability.",
            );
            ui.separator();
            cached_label(ui, render_cache, "LLM synthesis");
            cached_hover_monospace_block(
                ui,
                render_cache,
                payload.output.synthesis_rationale.as_str(),
            );
        });
        scan_value_label(
            ui,
            render_cache,
            confidence_label(payload.output.overall_confidence),
            confidence_emphasis(payload.output.overall_confidence),
        )
        .on_hover_text("Overall confidence assigned by the protocol review.");
        scan_value_label(
            ui,
            render_cache,
            call_review_failure_label(payload),
            failure_emphasis(payload),
        )
        .on_hover_text("Failure state for the focal call or calls in its review scope.");
        let mut latency_buffer = itoa::Buffer::new();
        cached_monospace_label(ui, render_cache, latency_buffer.format(focal.latency_ms))
            .on_hover_text("Protocol NeighborhoodCall.latency_ms for the focal tool call.");
        cached_monospace_label(ui, render_cache, "ms");
        cached_label(ui, render_cache, "scope");
        let mut scope_buffer = itoa::Buffer::new();
        cached_monospace_label(
            ui,
            render_cache,
            scope_buffer.format(payload.output.packet.total_calls_in_scope),
        )
        .on_hover_text("Number of calls included in the local analysis scope.");
        if clearable
            && ui
                .small_button("Clear")
                .on_hover_text("Clear the selected Eval & Protocol item.")
                .clicked()
        {
            eval_protocol::clear_selected_eval_protocol_call_review(ui);
        }
    });
}

pub(crate) fn render_tool_call_review_detail(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    artifact_key: &str,
    artifact: &ploke_records::protocol::Artifact,
    payload: &ploke_records::protocol::ToolCallReviewPayload,
    clearable: bool,
) {
    render_tool_call_review_detail_header(ui, render_cache, payload, clearable);
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("artifact provenance")
            .id_salt(("tool-call-review-provenance", artifact_key))
            .default_open(false),
        |ui| {
            render_protocol_artifact_coordinate(ui, render_cache, artifact_key, artifact);
        },
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("packet")
            .id_salt(("tool-call-review-packet", artifact_key))
            .default_open(false),
        |ui| {
            render_local_analysis_packet(ui, render_cache, &payload.output.packet);
        },
    );
    render_local_analysis_signals(ui, render_cache, &payload.output.signals);
    render_local_analysis_assessment(ui, render_cache, &payload.output);
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("reviewed neighborhood")
            .id_salt(("tool-call-review-neighborhood", artifact_key))
            .default_open(false),
        |ui| {
            for call in &payload.output.packet.calls {
                render_protocol_call_row(
                    ui,
                    render_cache,
                    ("tool-call-review-call", artifact_key),
                    call.index,
                    call.turn,
                    call.tool_name.as_str(),
                    call.tool_kind,
                    call.failed,
                    call.latency_ms,
                    call.summary.as_str(),
                    call.args_preview.as_str(),
                    call.result_preview.as_str(),
                    call.search_term.as_deref(),
                    call.path_hint.as_deref(),
                );
            }
        },
    );
}

pub(crate) fn render_tool_call_segment_review_artifact(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    review_index: usize,
    artifact_key: &str,
    artifact: &ploke_records::protocol::Artifact,
    payload: &ploke_records::protocol::ToolCallSegmentReviewPayload,
) {
    let segment = &payload.input.segment;
    let title = format!(
        "segment {} {:?} {:?}",
        segment.segment_index, segment.label, payload.output.overall
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt(("tool-call-segment-review", artifact_key, review_index))
            .default_open(false),
        |ui| {
            render_protocol_artifact_coordinate(ui, render_cache, artifact_key, artifact);
            ui.separator();
            cached_kv_usize(ui, render_cache, "segment", segment.segment_index);
            cached_kv_usize(ui, render_cache, "start call", segment.start_index);
            cached_kv_usize(ui, render_cache, "end call", segment.end_index);
            cached_kv_debug(ui, render_cache, "status", segment.status);
            cached_kv_debug(ui, render_cache, "label", segment.label);
            cached_kv_debug(ui, render_cache, "confidence", segment.confidence);
            render_copyable_text_preview(
                ui,
                render_cache,
                ("segment-review-rationale", artifact_key, review_index),
                "segment rationale",
                segment.rationale.as_str(),
            );
            render_segmentation_coverage(ui, render_cache, &payload.input.coverage);
            render_local_analysis_packet(ui, render_cache, &payload.output.packet);
            render_local_analysis_signals(ui, render_cache, &payload.output.signals);
            render_local_analysis_assessment(ui, render_cache, &payload.output);
            show_inspector_collapsing(
                ui,
                egui::CollapsingHeader::new("segment calls").default_open(false),
                |ui| {
                    for call in &segment.calls {
                        render_protocol_call_row(
                            ui,
                            render_cache,
                            ("tool-call-segment-review-call", artifact_key),
                            call.index,
                            call.turn,
                            call.tool_name.as_str(),
                            call.tool_kind,
                            call.failed,
                            call.latency_ms,
                            call.summary.as_str(),
                            call.args_preview.as_str(),
                            call.result_preview.as_str(),
                            call.search_term.as_deref(),
                            call.path_hint.as_deref(),
                        );
                    }
                },
            );
        },
    );
}

pub(crate) fn render_intent_segmentation_artifact(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    artifact_index: usize,
    artifact_key: &str,
    artifact: &ploke_records::protocol::Artifact,
    payload: &ploke_records::protocol::IntentSegmentationPayload,
) {
    let title = format!(
        "segmentation {} calls -> {} segments",
        payload.output.coverage.total_calls,
        payload.output.segments.len()
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((
                "tool-call-intent-segmentation",
                artifact_key,
                artifact_index,
            ))
            .default_open(false),
        |ui| {
            render_protocol_artifact_coordinate(ui, render_cache, artifact_key, artifact);
            render_segmentation_coverage(ui, render_cache, &payload.output.coverage);
            render_copyable_text_preview(
                ui,
                render_cache,
                ("intent-overall-rationale", artifact_key, artifact_index),
                "overall rationale",
                payload.output.overall_rationale.as_str(),
            );
            for segment in &payload.output.segments {
                let title = format!(
                    "segment {} {:?} calls {}..{}",
                    segment.segment_index, segment.label, segment.start_index, segment.end_index
                );
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new(title)
                        .id_salt(("intent-segment", artifact_key, segment.segment_index))
                        .default_open(false),
                    |ui| {
                        cached_kv_debug(ui, render_cache, "status", segment.status);
                        cached_kv_debug(ui, render_cache, "confidence", segment.confidence);
                        render_protocol_turn_span(ui, render_cache, segment.turns.as_slice());
                        render_copyable_text_preview(
                            ui,
                            render_cache,
                            (
                                "intent-segment-rationale",
                                artifact_key,
                                segment.segment_index,
                            ),
                            "rationale",
                            segment.rationale.as_str(),
                        );
                        for call in &segment.calls {
                            render_protocol_call_row(
                                ui,
                                render_cache,
                                ("intent-segment-call", artifact_key, segment.segment_index),
                                call.index,
                                call.turn,
                                call.tool_name.as_str(),
                                call.tool_kind,
                                call.failed,
                                call.latency_ms,
                                call.summary.as_str(),
                                call.args_preview.as_str(),
                                call.result_preview.as_str(),
                                call.search_term.as_deref(),
                                call.path_hint.as_deref(),
                            );
                        }
                    },
                );
            }
        },
    );
}

fn render_local_analysis_packet(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    packet: &ploke_protocol::LocalAnalysisPacket,
) {
    cached_kv_id(ui, render_cache, "target", packet.target_id.as_str());
    cached_kv_debug(ui, render_cache, "target kind", packet.target_kind);
    cached_kv_usize(ui, render_cache, "scope calls", packet.total_calls_in_scope);
    cached_kv_usize(ui, render_cache, "run calls", packet.total_calls_in_run);
    if let Some(index) = packet.focal_call_index {
        cached_kv_usize(ui, render_cache, "focal call", index);
    }
    if let Some(index) = packet.segment_index {
        cached_kv_usize(ui, render_cache, "segment", index);
    }
    if let Some(status) = packet.segment_status {
        cached_kv_debug(ui, render_cache, "segment status", status);
    }
    if let Some(label) = packet.segment_label {
        cached_kv_debug(ui, render_cache, "segment label", label);
    }
    render_protocol_turn_span(ui, render_cache, packet.turn_span.as_slice());
    render_copyable_text_preview(
        ui,
        render_cache,
        ("local-analysis-scope", packet.target_id.as_str()),
        "scope",
        packet.scope_summary.as_str(),
    );
}

fn render_local_analysis_signals(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    signals: &ploke_protocol::LocalAnalysisSignals,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("signals").default_open(false),
        |ui| {
            cached_kv_usize(ui, render_cache, "turns", signals.scope_turn_count);
            cached_kv_usize(
                ui,
                render_cache,
                "distinct tools",
                signals.distinct_tool_count,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "repeated tools",
                signals.repeated_tool_name_count,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "search calls",
                signals.search_calls_in_scope,
            );
            cached_kv_usize(ui, render_cache, "read calls", signals.read_calls_in_scope);
            cached_kv_usize(
                ui,
                render_cache,
                "browse calls",
                signals.browse_calls_in_scope,
            );
            cached_kv_usize(ui, render_cache, "edit calls", signals.edit_calls_in_scope);
            cached_kv_usize(
                ui,
                render_cache,
                "execute calls",
                signals.execute_calls_in_scope,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "failed calls",
                signals.failed_calls_in_scope,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "similar searches",
                signals.similar_search_neighbors,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "directory pivots",
                signals.directory_pivots,
            );
            cached_kv_optional_usize(
                ui,
                render_cache,
                "labeled segments",
                signals.labeled_segments_in_source,
            );
            cached_kv_optional_usize(
                ui,
                render_cache,
                "ambiguous segments",
                signals.ambiguous_segments_in_source,
            );
            cached_kv_optional_usize(
                ui,
                render_cache,
                "uncovered calls",
                signals.uncovered_calls_in_source,
            );
            if !signals.candidate_concerns.is_empty() {
                cached_label(ui, render_cache, "concerns");
                for concern in &signals.candidate_concerns {
                    cached_monospace_label(ui, render_cache, format!("{concern:?}").as_str());
                }
            }
        },
    );
}

fn render_local_analysis_assessment(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    assessment: &ploke_protocol::LocalAnalysisAssessment,
) {
    cached_kv_debug(ui, render_cache, "overall", assessment.overall);
    cached_kv_debug(
        ui,
        render_cache,
        "overall confidence",
        assessment.overall_confidence,
    );
    ui.separator();
    cached_kv_debug(
        ui,
        render_cache,
        "usefulness",
        assessment.usefulness.verdict,
    );
    cached_kv_debug(
        ui,
        render_cache,
        "redundancy",
        assessment.redundancy.verdict,
    );
    cached_kv_debug(
        ui,
        render_cache,
        "recoverability",
        assessment.recoverability.verdict,
    );
    render_copyable_text_preview(
        ui,
        render_cache,
        ("assessment-synthesis", assessment.packet.target_id.as_str()),
        "synthesis raw",
        assessment.synthesis_rationale.as_str(),
    );
    render_protocol_judgment(
        ui,
        render_cache,
        "usefulness",
        assessment.usefulness.verdict,
        assessment.usefulness.confidence,
        assessment.usefulness.rationale.as_str(),
    );
    render_protocol_judgment(
        ui,
        render_cache,
        "redundancy",
        assessment.redundancy.verdict,
        assessment.redundancy.confidence,
        assessment.redundancy.rationale.as_str(),
    );
    render_protocol_judgment(
        ui,
        render_cache,
        "recoverability",
        assessment.recoverability.verdict,
        assessment.recoverability.confidence,
        assessment.recoverability.rationale.as_str(),
    );
}

fn render_protocol_judgment(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    label: &str,
    verdict: impl std::fmt::Debug,
    confidence: impl std::fmt::Debug,
    rationale: &str,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(label).default_open(false),
        |ui| {
            cached_kv_debug(ui, render_cache, "verdict", verdict);
            cached_kv_debug(ui, render_cache, "confidence", confidence);
            render_copyable_text_preview(
                ui,
                render_cache,
                ("judgment-rationale", label, rationale),
                "rationale",
                rationale,
            );
        },
    );
}

fn render_segmentation_coverage(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    coverage: &ploke_protocol::SegmentationCoverage,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("coverage").default_open(false),
        |ui| {
            cached_kv_usize(ui, render_cache, "total calls", coverage.total_calls);
            cached_kv_usize(
                ui,
                render_cache,
                "labeled segments",
                coverage.labeled_segments,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "ambiguous segments",
                coverage.ambiguous_segments,
            );
            cached_kv_usize(ui, render_cache, "labeled calls", coverage.labeled_calls);
            cached_kv_usize(
                ui,
                render_cache,
                "ambiguous calls",
                coverage.ambiguous_calls,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "uncovered calls",
                coverage.uncovered_calls,
            );
        },
    );
}

fn render_protocol_turn_span(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turns: &[u32],
) {
    if turns.is_empty() {
        cached_kv_id(ui, render_cache, "turns", "none");
        return;
    }

    ui.horizontal(|ui| {
        cached_label(ui, render_cache, "turns");
        let mut buffer = itoa::Buffer::new();
        for turn in turns {
            cached_monospace_label(ui, render_cache, buffer.format(*turn));
        }
    });
}

fn render_protocol_call_row(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_salt: impl std::hash::Hash,
    index: usize,
    turn: u32,
    tool_name: &str,
    tool_kind: impl std::fmt::Debug,
    failed: bool,
    latency_ms: u64,
    summary: &str,
    args_preview: &str,
    result_preview: &str,
    search_term: Option<&str>,
    path_hint: Option<&str>,
) {
    let status = if failed { "failed" } else { "ok" };
    let title = format!("[{index}] {tool_name} {status}");
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt(("protocol-call-row", id_salt, index))
            .default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "tool", tool_name);
            cached_kv_u32(ui, render_cache, "turn", turn);
            cached_kv_debug(ui, render_cache, "kind", tool_kind);
            cached_kv_bool(ui, render_cache, "failed", failed);
            cached_kv_u64(ui, render_cache, "latency ms", latency_ms);
            if let Some(search_term) = search_term {
                cached_kv_text(ui, render_cache, "search term", search_term);
            }
            if let Some(path_hint) = path_hint {
                cached_kv_path(ui, render_cache, "path hint", path_hint);
            }
            render_copyable_text_preview(
                ui,
                render_cache,
                ("protocol-call-summary", tool_name, index, summary),
                "summary",
                summary,
            );
            render_protocol_preview_payload(
                ui,
                render_cache,
                ("protocol-call-args-preview", tool_name, index),
                "args preview",
                ProtocolPreviewKind::Arguments,
                tool_name,
                args_preview,
            );
            render_protocol_preview_payload(
                ui,
                render_cache,
                ("protocol-call-result-preview", tool_name, index),
                "result preview",
                ProtocolPreviewKind::Result,
                tool_name,
                result_preview,
            );
        },
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtocolPreviewKind {
    Arguments,
    Result,
}

fn render_protocol_preview_payload(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    label: &'static str,
    kind: ProtocolPreviewKind,
    tool_name: &str,
    preview: &str,
) {
    let preview_id = egui::Id::new(("protocol-preview-payload", label, id_source, preview));
    let can_show_fields = protocol_preview_has_typed_fields(render_cache, kind, tool_name, preview);
    let mode_id = ui.make_persistent_id(("protocol-preview-mode", preview_id));
    let mut show_fields = ui.data(|data| data.get_temp::<bool>(mode_id).unwrap_or(can_show_fields));
    if !can_show_fields {
        show_fields = false;
    }

    let copy_value = CopyableText::new(preview);
    ui.horizontal_wrapped(|ui| {
        cached_label(ui, render_cache, label);
        if ui
            .selectable_label(!show_fields, "Raw")
            .on_hover_text("Show raw preview text.")
            .clicked()
        {
            show_fields = false;
            ui.data_mut(|data| data.insert_temp(mode_id, show_fields));
        }
        if can_show_fields
            && ui
                .selectable_label(show_fields, "Fields")
                .on_hover_text("Show deserialized fields.")
                .clicked()
        {
            show_fields = true;
            ui.data_mut(|data| data.insert_temp(mode_id, show_fields));
        }
        id_display::copy_button(ui, &copy_value);
    });

    ui.indent(("protocol-preview-body", preview_id), |ui| {
        if show_fields {
            render_protocol_preview_typed_fields(ui, render_cache, kind, tool_name, preview);
        } else {
            render_copyable_multiline_body(ui, render_cache, preview, preview);
            if kind == ProtocolPreviewKind::Result && !can_show_fields {
                render_result_preview_fields_note(ui, render_cache);
            }
        }
    });
}

fn render_result_preview_fields_note(ui: &mut egui::Ui, render_cache: &mut InspectorRenderCache) {
    ui.horizontal_wrapped(|ui| {
        cached_label(ui, render_cache, "fields");
        cached_wrapped_monospace_label(
            ui,
            render_cache,
            "unavailable: this protocol artifact stores a truncated result_preview, not the full tool result. The typed Fields view needs the full run-record result or a primary-branch protocol data fix.",
        );
    });
}

fn protocol_preview_has_typed_fields(
    render_cache: &mut InspectorRenderCache,
    kind: ProtocolPreviewKind,
    tool_name: &str,
    preview: &str,
) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        match kind {
            ProtocolPreviewKind::Arguments => render_cache
                .tool_arguments("protocol-preview", tool_name, preview)
                .decoded()
                .is_some(),
            ProtocolPreviewKind::Result => render_cache
                .tool_result("protocol-preview", tool_name, preview)
                .decoded()
                .is_some(),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (render_cache, kind, tool_name, preview);
        false
    }
}

fn render_protocol_preview_typed_fields(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    kind: ProtocolPreviewKind,
    tool_name: &str,
    preview: &str,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        match kind {
            ProtocolPreviewKind::Arguments => {
                let decoded = render_cache.tool_arguments("protocol-preview", tool_name, preview);
                render_decoded_tool_arguments(ui, render_cache, decoded.as_ref());
            }
            ProtocolPreviewKind::Result => {
                let decoded = render_cache.tool_result("protocol-preview", tool_name, preview);
                render_decoded_tool_result(ui, render_cache, decoded.as_ref());
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (kind, tool_name, preview);
        cached_kv_id(ui, render_cache, "decode", "native_only");
    }
}

fn render_copyable_multiline_body(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    copy_text: &str,
    display_text: &str,
) {
    let copy_value = CopyableText::new(copy_text);
    let response = ui
        .add(
            egui::Label::new(render_cache.wrapped_monospace_galley(ui, display_text))
                .sense(egui::Sense::click()),
        )
        .on_hover_text(copy_value.hover_text(false, false));
    id_display::attach_copy_context_menu(&response, &copy_value);
}
