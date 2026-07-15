use eframe::egui::{self, Color32, RichText, ScrollArea, Ui};
use ploke_eval::walk_client::{
    ArtifactBody, ArtifactFile, CompletedEvaluationTrace, EvaluationRunCoordinate,
    EvaluationTraceIndex, EvaluationTraceSnapshot, EvaluationTraceState, LocalAnalysisAssessment,
    LocalAnalysisTargetKind, ToolCallNeighborhood, ToolExecutionRecord, ToolResult, TraceEvidence,
    TraceSource,
};

pub(in crate::app) struct TracePanel<'a> {
    pub(in crate::app) index: Option<&'a EvaluationTraceIndex>,
    pub(in crate::app) selected: &'a mut Option<usize>,
    pub(in crate::app) snapshot: Option<&'a EvaluationTraceSnapshot>,
    pub(in crate::app) index_pending: bool,
    pub(in crate::app) trace_pending: bool,
}

#[derive(Debug, Default)]
pub(in crate::app) struct TraceAction {
    pub(in crate::app) refresh: bool,
    pub(in crate::app) load: Option<EvaluationRunCoordinate>,
}

impl TracePanel<'_> {
    pub(in crate::app) fn show(self, ui: &mut Ui) -> TraceAction {
        let mut action = TraceAction::default();
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("COMPLETED-RUN INSPECTION")
                    .strong()
                    .color(Color32::LIGHT_BLUE),
            )
            .on_hover_text(
                "The run registry discovers completed runs. Exact detail falls back to lifecycle-only evidence if a run is no longer completed. Mutable in-flight traces are intentionally excluded.",
            );
            ui.separator();
            ui.label("read-only");
            if ui
                .add_enabled(!self.index_pending, egui::Button::new("Refresh Completed Runs"))
                .clicked()
            {
                action.refresh = true;
            }
            if self.index_pending {
                ui.label(RichText::new("loading run index...").color(Color32::YELLOW));
            }
        });
        ui.label(
            "Inspect final prompts, provider exchanges, tool calls, and protocol adjudications without opening run files.",
        );
        ui.add_space(8.0);

        let Some(index) = self.index else {
            ui.separator();
            ui.add_space(8.0);
            ui.label(RichText::new("No completed-run index loaded").color(Color32::GRAY));
            return action;
        };

        index_header(ui, index);
        ui.add_space(8.0);
        if index.runs.is_empty() {
            ui.label(
                "The authoritative run registry contains no completed runs for this campaign.",
            );
            return action;
        }

        let selected_text = self
            .selected
            .and_then(|selected| index.runs.get(selected))
            .map(run_label)
            .unwrap_or_else(|| "Choose a completed run".to_string());
        ui.horizontal_wrapped(|ui| {
            ui.label("completed run");
            let width = (ui.available_width() - 140.0).clamp(220.0, 520.0);
            egui::ComboBox::from_id_salt("completed_trace_run")
                .width(width)
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for (position, entry) in index.runs.iter().enumerate() {
                        if ui
                            .selectable_value(self.selected, Some(position), run_label(entry))
                            .changed()
                        {
                            action.load = Some(entry.coordinate.clone());
                        }
                    }
                });
            if let Some(selected) = *self.selected
                && let Some(entry) = index.runs.get(selected)
                && ui
                    .add_enabled(
                        !self.trace_pending,
                        egui::Button::new(
                            self.snapshot
                                .filter(|snapshot| snapshot.coordinate == entry.coordinate)
                                .map_or("Load Trace", |_| "Reload Trace"),
                        ),
                    )
                    .clicked()
            {
                action.load = Some(entry.coordinate.clone());
            }
            if self.trace_pending {
                ui.label(RichText::new("loading trace...").color(Color32::YELLOW));
            }
        });
        ui.add_space(8.0);
        ui.separator();

        let selected = self
            .selected
            .and_then(|position| index.runs.get(position))
            .map(|entry| &entry.coordinate);
        match (selected, self.snapshot) {
            (None, _) => {
                ui.label("Choose a completed run to load its sealed trace.");
            }
            (Some(_), None) => {
                ui.label(if self.trace_pending {
                    "Loading the selected sealed trace..."
                } else {
                    "Press Load Trace to inspect the selected run."
                });
            }
            (Some(coordinate), Some(snapshot)) if &snapshot.coordinate != coordinate => {
                ui.colored_label(
                    Color32::YELLOW,
                    "The visible trace belongs to the previous selection and has been hidden.",
                );
            }
            (Some(_), Some(snapshot)) => {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| render_snapshot(ui, snapshot));
            }
        }
        action
    }
}

fn index_header(ui: &mut Ui, index: &EvaluationTraceIndex) {
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("campaign: {}", index.campaign));
        ui.separator();
        ui.label(format!("runs: {}", index.runs.len()));
        ui.separator();
        ui.label(format!("authority: {:?}", index.authority));
        ui.separator();
        ui.label(format!(
            "session revision: {}",
            index.version.journal_revision()
        ));
        ui.separator();
        ui.label(format!("protocol: {}", index.epoch.protocol_version));
    });
    ui.label(
        RichText::new(index.instances_root.display().to_string())
            .monospace()
            .color(Color32::GRAY),
    );
}

fn run_label(entry: &ploke_eval::walk_client::EvaluationRunEntry) -> String {
    let role = format!("{:?}", entry.registration.value.frozen_spec.run_role).to_lowercase();
    format!(
        "{role} · {} · {}",
        entry.coordinate.instance, entry.coordinate.run_id
    )
}

fn render_snapshot(ui: &mut Ui, snapshot: &EvaluationTraceSnapshot) {
    ui.add_space(8.0);
    ui.heading(format!(
        "{} / {}",
        snapshot.coordinate.instance, snapshot.coordinate.run_id
    ));
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("campaign: {}", snapshot.coordinate.campaign));
        ui.separator();
        ui.label(format!("authority: {:?}", snapshot.authority));
        ui.separator();
        ui.label(format!(
            "session revision: {}",
            snapshot.version.journal_revision()
        ));
        ui.separator();
        ui.label(format!("protocol: {}", snapshot.epoch.protocol_version));
    });
    ui.add_space(8.0);

    match &snapshot.trace {
        EvaluationTraceState::NotCompleted { registration } => {
            ui.colored_label(
                Color32::YELLOW,
                format!(
                    "LIFECYCLE EVIDENCE ONLY — run is no longer completed: {:?}",
                    registration.value.lifecycle.execution_status
                ),
            );
            render_source(ui, &registration.source);
        }
        EvaluationTraceState::Completed { trace } => {
            ui.label(
                RichText::new("SEALED COMPLETED EVIDENCE")
                    .strong()
                    .color(Color32::LIGHT_BLUE),
            );
            render_completed(ui, trace);
        }
    }
}

fn render_completed(ui: &mut Ui, trace: &CompletedEvaluationTrace) {
    let run = &trace.run.value;
    ui.horizontal_wrapped(|ui| {
        ui.label(format!(
            "role: {:?}",
            trace.registration.value.frozen_spec.run_role
        ));
        ui.separator();
        ui.label(format!("turns: {}", run.turn_count()));
        ui.separator();
        ui.label(format!("tool calls: {}", run.tool_call_count()));
        ui.separator();
        ui.label(format!("failed: {}", run.failed_tool_call_count()));
        if let Some(model) = run.metadata.agent.model_id.as_deref() {
            ui.separator();
            ui.label(format!("model: {model}"));
        }
        if let Some(provider) = run.metadata.agent.provider.as_deref() {
            ui.separator();
            ui.label(format!("provider: {provider}"));
        }
    });

    egui::CollapsingHeader::new("Evidence sources and SHA-256")
        .default_open(false)
        .show(ui, |ui| {
            for source in trace.sources() {
                render_source(ui, source);
            }
        });

    if let Some(turn) = trace.turn.as_ref() {
        egui::CollapsingHeader::new("Sealed parent-turn summary")
            .default_open(true)
            .show(ui, |ui| {
                ui.label(format!("model: {}", turn.value.0.selected_model));
                if let Some(route) = turn.value.0.model_route.as_ref() {
                    ui.label(format!(
                        "route: {} / {}",
                        route.router,
                        route.provider_slug.as_deref().unwrap_or("-")
                    ));
                }
                text_block(ui, "issue prompt", &turn.value.0.issue_prompt);
                if !turn.value.0.llm_prompt.is_empty() {
                    egui::CollapsingHeader::new(format!(
                        "Recorded LLM prompt ({} messages)",
                        turn.value.0.llm_prompt.len()
                    ))
                    .show(ui, |ui| {
                        for (index, message) in turn.value.0.llm_prompt.iter().enumerate() {
                            text_block(
                                ui,
                                &format!("message {index} · {:?}", message.role),
                                &message.content,
                            );
                        }
                    });
                }
                if let Some(response) = turn.value.0.llm_response.as_deref() {
                    text_block(ui, "recorded LLM response", response);
                }
                egui::CollapsingHeader::new(format!(
                    "Observed event timeline ({})",
                    turn.value.0.events.len()
                ))
                .show(ui, |ui| {
                    for (index, event) in turn.value.0.events.iter().enumerate() {
                        let kind = serde_json::to_value(event)
                            .ok()
                            .and_then(|value| {
                                value
                                    .as_object()
                                    .and_then(|object| object.keys().next().cloned())
                            })
                            .unwrap_or_else(|| "event".to_string());
                        egui::CollapsingHeader::new(format!("{index} · {kind}")).show(ui, |ui| {
                            if let Ok(raw) = serde_json::to_string_pretty(event) {
                                text_block(ui, "event JSON", &raw);
                            }
                        });
                    }
                });
                if let Some(final_message) = turn.value.0.final_assistant_message.as_ref() {
                    text_block(
                        ui,
                        "final assistant message preview",
                        &final_message.content_preview,
                    );
                }
                render_source(ui, &turn.source);
            });
    } else {
        ui.colored_label(
            Color32::GRAY,
            "No sealed parent-turn summary was registered.",
        );
    }

    render_exchanges(ui, trace);
    render_turns(ui, trace);
    render_protocol(ui, trace);
}

fn render_exchanges(ui: &mut Ui, trace: &CompletedEvaluationTrace) {
    let Some(exchanges) = trace.exchanges.as_ref() else {
        ui.colored_label(
            Color32::GRAY,
            "No provider-response sidecar was registered.",
        );
        return;
    };
    egui::CollapsingHeader::new(format!(
        "Provider exchanges in physical order ({})",
        exchanges.value.len()
    ))
    .default_open(true)
    .show(ui, |ui| {
        render_source(ui, &exchanges.source);
        for exchange in &exchanges.value {
            let response = exchange.record.response();
            let provider = response
                .provider
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "-".to_string());
            egui::CollapsingHeader::new(format!(
                "line {} · response {} · {} · {}",
                exchange.source_line,
                exchange.record.response_index().get(),
                response.model,
                provider
            ))
            .id_salt(("model_exchange", exchange.source_line))
            .show(ui, |ui| {
                for choice in &response.choices {
                    if let Some(message) = choice.message.as_ref() {
                        if let Some(content) = message.content.as_deref() {
                            text_block(ui, "assistant content", content);
                        }
                        if let Some(calls) = message.tool_calls.as_ref() {
                            for call in calls {
                                ui.label(format!(
                                    "tool: {} ({})",
                                    call.function.name.as_str(),
                                    call.call_id
                                ));
                                text_block(ui, "arguments", &pretty_json(&call.function.arguments));
                            }
                        }
                    }
                }
                egui::CollapsingHeader::new("normalized provider envelope").show(ui, |ui| {
                    if let Ok(raw) = serde_json::to_string_pretty(response) {
                        text_block(ui, "response JSON", &raw);
                    }
                });
            });
        }
    });
}

fn render_turns(ui: &mut Ui, trace: &CompletedEvaluationTrace) {
    let mut call_index = 0usize;
    egui::CollapsingHeader::new(format!(
        "Agent turns and tool execution ({})",
        trace.run.value.phases.agent_turns.len()
    ))
    .default_open(true)
    .show(ui, |ui| {
        for turn in &trace.run.value.phases.agent_turns {
            let start = call_index;
            let end = start + turn.tool_calls.len();
            egui::CollapsingHeader::new(format!(
                "Turn {} · {:?} · {} tool call(s)",
                turn.turn_number,
                turn.outcome,
                turn.tool_calls.len()
            ))
            .id_salt(("agent_turn", turn.turn_number))
            .default_open(turn.turn_number == 1)
            .show(ui, |ui| {
                text_block(ui, "issue prompt", &turn.issue_prompt);
                ui.label(format!("started: {}", turn.started_at));
                ui.label(format!("ended: {}", turn.ended_at));
                if let Some(request) = turn.llm_request.as_ref() {
                    egui::CollapsingHeader::new(format!(
                        "LLM request · {} · {} message(s)",
                        request.model,
                        request.messages.len()
                    ))
                    .show(ui, |ui| {
                        for (index, message) in request.messages.iter().enumerate() {
                            text_block(
                                ui,
                                &format!("message {index} · {:?}", message.role),
                                &message.content,
                            );
                        }
                    });
                }
                if let Some(response) = turn.llm_response.as_ref() {
                    text_block(ui, "recorded LLM response", &response.content);
                }
                for (offset, call) in turn.tool_calls.iter().enumerate() {
                    render_tool_call(ui, call, start + offset, &trace.protocol);
                }
            });
            call_index = end;
        }
    });
}

fn render_tool_call(
    ui: &mut Ui,
    call: &ToolExecutionRecord,
    index: usize,
    protocol: &[TraceEvidence<ArtifactFile>],
) {
    let status = match &call.result {
        ToolResult::Completed(_) => "completed",
        ToolResult::Failed(_) => "failed",
    };
    egui::CollapsingHeader::new(format!("[{index}] {} · {status}", call.request.tool))
        .id_salt(("tool_call", index))
        .show(ui, |ui| {
            ui.label(format!("call id: {}", call.request.call_id));
            ui.label(format!("latency: {} ms", call.latency_ms));
            text_block(
                ui,
                "arguments",
                &pretty_json(call.request.arguments.as_str()),
            );
            match &call.result {
                ToolResult::Completed(result) => text_block(ui, "result", &result.content),
                ToolResult::Failed(result) => {
                    ui.colored_label(Color32::LIGHT_RED, "tool failed");
                    text_block(ui, "error", &result.error);
                }
            }
            render_call_reviews(ui, protocol, index, &call.request.tool);
        });
}

fn render_call_reviews(
    ui: &mut Ui,
    protocol: &[TraceEvidence<ArtifactFile>],
    index: usize,
    tool: &str,
) {
    let mut candidates = 0usize;
    let mut linked = 0usize;
    for evidence in protocol {
        let ArtifactBody::ToolCallReview(payload) = &evidence.value.artifact.body else {
            continue;
        };
        let input_index = payload.input.focal.index;
        let output_index = payload.output.packet.focal_call_index;
        if input_index != index && output_index != Some(index) {
            continue;
        }
        candidates += 1;
        if review_matches(&payload.input, &payload.output, index, tool) {
            linked += 1;
            egui::CollapsingHeader::new(format!(
                "Tool-call review · {:?} · {:?}",
                payload.output.overall, payload.output.overall_confidence
            ))
            .id_salt(("call_review", index, evidence.value.artifact.created_at_ms))
            .default_open(true)
            .show(ui, |ui| {
                render_review(ui, &payload.output);
                render_source(ui, &evidence.source);
            });
        } else {
            ui.colored_label(
                Color32::LIGHT_RED,
                format!(
                    "Review identity mismatch: input call {input_index} ({}) / output call {} / run call {index} ({tool})",
                    payload.input.focal.tool_name,
                    output_index
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string())
                ),
            );
            render_source(ui, &evidence.source);
        }
    }
    if candidates == 0 {
        ui.colored_label(Color32::GRAY, "No tool-call review artifact for this call.");
    } else if linked == 0 {
        ui.colored_label(
            Color32::LIGHT_RED,
            "Review evidence exists but does not validate against this run call.",
        );
    }
}

fn review_matches(
    input: &ToolCallNeighborhood,
    output: &LocalAnalysisAssessment,
    index: usize,
    tool: &str,
) -> bool {
    if input.focal.index != index
        || input.focal.tool_name != tool
        || input.subject_id != output.packet.subject_id
        || output.packet.target_kind != LocalAnalysisTargetKind::FocalCall
        || output.packet.target_id != format!("call:{index}")
        || output.packet.scope_summary
            != format!("focal call [{index}] {tool} in bounded neighborhood")
        || output.packet.focal_call_index != Some(index)
        || input.total_calls_in_run != output.packet.total_calls_in_run
        || input.before.len() + 1 + input.after.len() != output.packet.total_calls_in_scope
        || output.packet.turn_span.as_slice() != [input.turn.turn]
        || output.packet.segment_index.is_some()
        || output.packet.segment_status.is_some()
        || output.packet.segment_label.is_some()
    {
        return false;
    }
    output.packet.calls.iter().eq(input.all_calls())
}

fn render_run_link(
    ui: &mut Ui,
    input: &ToolCallNeighborhood,
    output: &LocalAnalysisAssessment,
    call: Option<&ToolExecutionRecord>,
) {
    match call {
        Some(call) if review_matches(input, output, input.focal.index, &call.request.tool) => {
            ui.colored_label(
                Color32::LIGHT_GREEN,
                format!(
                    "linked to displayed run call [{}] {}",
                    input.focal.index, call.request.tool
                ),
            );
        }
        Some(call) => {
            ui.colored_label(
                Color32::LIGHT_RED,
                format!(
                    "review identity does not validate against displayed run call [{}] '{}' (review tool '{}')",
                    input.focal.index, call.request.tool, input.focal.tool_name
                ),
            );
        }
        None => {
            ui.colored_label(
                Color32::YELLOW,
                format!(
                    "not linked in this view: no direct run-record call at global index {}",
                    input.focal.index
                ),
            );
        }
    }
}

fn render_review(ui: &mut Ui, review: &LocalAnalysisAssessment) {
    ui.label(format!("recorded scope: {}", review.packet.scope_summary));
    ui.label(format!(
        "overall: {:?} ({:?})",
        review.overall, review.overall_confidence
    ));
    text_block(ui, "synthesis", &review.synthesis_rationale);
    ui.label(format!(
        "usefulness: {:?} ({:?})",
        review.usefulness.verdict, review.usefulness.confidence
    ));
    text_block(ui, "usefulness rationale", &review.usefulness.rationale);
    ui.label(format!(
        "redundancy: {:?} ({:?})",
        review.redundancy.verdict, review.redundancy.confidence
    ));
    text_block(ui, "redundancy rationale", &review.redundancy.rationale);
    ui.label(format!(
        "recoverability: {:?} ({:?})",
        review.recoverability.verdict, review.recoverability.confidence
    ));
    text_block(
        ui,
        "recoverability rationale",
        &review.recoverability.rationale,
    );
    if !review.signals.candidate_concerns.is_empty() {
        ui.label(format!(
            "concerns: {}",
            review
                .signals
                .candidate_concerns
                .iter()
                .map(|concern| format!("{concern:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}

fn render_protocol(ui: &mut Ui, trace: &CompletedEvaluationTrace) {
    egui::CollapsingHeader::new(format!(
        "All typed protocol artifacts ({})",
        trace.protocol.len()
    ))
    .default_open(false)
    .show(ui, |ui| {
        if trace.protocol.is_empty() {
            ui.colored_label(
                Color32::GRAY,
                "No typed protocol artifacts were sealed for this run.",
            );
        }
        for evidence in &trace.protocol {
            let artifact = &evidence.value.artifact;
            egui::CollapsingHeader::new(format!(
                "{} · {} · {}",
                artifact.procedure_name,
                artifact.created_at_ms,
                artifact.model_id.as_deref().unwrap_or("-")
            ))
            .id_salt((
                "protocol_artifact",
                artifact.created_at_ms,
                &evidence.value.path,
            ))
            .show(ui, |ui| {
                ui.label(format!("subject: {}", artifact.subject_id));
                ui.label(format!("run: {}", artifact.run_id));
                ui.label(format!(
                    "provider: {}",
                    artifact.provider_slug.as_deref().unwrap_or("-")
                ));
                match &artifact.body {
                    ArtifactBody::ToolCallReview(payload) => {
                        ui.label(format!(
                            "focal call: input={} output={}",
                            payload.input.focal.index,
                            payload
                                .output
                                .packet
                                .focal_call_index
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "-".to_string())
                        ));
                        render_run_link(
                            ui,
                            &payload.input,
                            &payload.output,
                            run_call(trace, payload.input.focal.index),
                        );
                        render_review(ui, &payload.output);
                    }
                    ArtifactBody::ToolCallSegmentReview(payload) => {
                        ui.label(format!(
                            "segment: {:?}",
                            payload.output.packet.segment_index
                        ));
                        render_review(ui, &payload.output);
                    }
                    _ => {
                        ui.label(
                            "Typed artifact retained; use its source path for the full payload.",
                        );
                    }
                }
                render_source(ui, &evidence.source);
            });
        }
    });
}

fn run_call(trace: &CompletedEvaluationTrace, index: usize) -> Option<&ToolExecutionRecord> {
    trace
        .run
        .value
        .phases
        .agent_turns
        .iter()
        .flat_map(|turn| &turn.tool_calls)
        .nth(index)
}

fn render_source(ui: &mut Ui, source: &TraceSource) {
    ui.group(|ui| {
        ui.label(format!("{:?}", source.kind));
        ui.label(RichText::new(source.path.display().to_string()).monospace());
        ui.label(
            RichText::new(format!("sha256: {}", source.content_sha256))
                .monospace()
                .color(Color32::GRAY),
        );
    });
}

fn text_block(ui: &mut Ui, label: &str, text: &str) {
    ui.label(RichText::new(format!("{label}:")).strong());
    ui.add(egui::Label::new(RichText::new(text).monospace()).wrap());
}

fn pretty_json(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_link_requires_exact_global_call_identity() {
        let input: ToolCallNeighborhood = serde_json::from_value(serde_json::json!({
            "subject_id": "org__repo-1",
            "total_calls_in_run": 2,
            "total_calls_in_turn": 2,
            "turn": {
                "turn": 1,
                "tool_count": 2,
                "failed_tool_count": 0,
                "patch_proposed": false,
                "patch_applied": false
            },
            "before": [],
            "focal": neighborhood_call(1, "read_file"),
            "after": []
        }))
        .expect("tool-call neighborhood");
        let mut output = assessment(1, "read_file");

        assert!(review_matches(&input, &output, 1, "read_file"));
        output.packet.target_id = "call:0".to_string();
        assert!(!review_matches(&input, &output, 1, "read_file"));
        output.packet.target_id = "call:1".to_string();
        output.packet.scope_summary = "corrupted scope".to_string();
        assert!(!review_matches(&input, &output, 1, "read_file"));
        output.packet.scope_summary =
            "focal call [1] read_file in bounded neighborhood".to_string();
        output.packet.calls[0].summary = "corrupted summary".to_string();
        assert!(!review_matches(&input, &output, 1, "read_file"));
        output.packet.calls[0] = input.focal.clone();
        output.packet.total_calls_in_run = 3;
        assert!(!review_matches(&input, &output, 1, "read_file"));
        output.packet.total_calls_in_run = 2;
        output.packet.focal_call_index = Some(0);
        assert!(!review_matches(&input, &output, 1, "read_file"));
        output.packet.focal_call_index = Some(1);
        assert!(!review_matches(&input, &output, 1, "cargo"));
    }

    #[test]
    fn missing_review_remains_visible_on_an_expandable_tool_call() {
        use egui_kittest::{Harness, kittest::Queryable};

        let call = tool_execution();
        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(900.0, 500.0))
            .build_ui(move |ui| render_tool_call(ui, &call, 0, &[]));

        harness.get_by_label("[0] read_file · completed").click();
        harness.run();

        assert_eq!(
            harness
                .get_all_by_label("No tool-call review artifact for this call.")
                .count(),
            1
        );
        assert_eq!(harness.get_all_by_label("pub fn fixture() {}").count(), 1);
    }

    #[test]
    fn mismatch_is_red() {
        use egui_kittest::{Harness, kittest::Queryable};

        let input: ToolCallNeighborhood = serde_json::from_value(serde_json::json!({
            "subject_id": "org__repo-1",
            "total_calls_in_run": 2,
            "total_calls_in_turn": 2,
            "turn": {
                "turn": 1,
                "tool_count": 2,
                "failed_tool_count": 0,
                "patch_proposed": false,
                "patch_applied": false
            },
            "before": [],
            "focal": neighborhood_call(1, "read_file"),
            "after": []
        }))
        .expect("tool-call neighborhood");
        let mut output = assessment(1, "read_file");
        output.packet.total_calls_in_run = 3;
        let call = tool_execution();
        let harness = Harness::builder()
            .with_size(egui::Vec2::new(900.0, 240.0))
            .build_ui(move |ui| render_run_link(ui, &input, &output, Some(&call)));

        assert_eq!(
            harness
                .get_all_by_label(
                    "review identity does not validate against displayed run call [1] 'read_file' (review tool 'read_file')"
                )
                .count(),
            1
        );
    }

    #[test]
    fn evidence_source_keeps_full_path_and_hash_visible() {
        use egui_kittest::{Harness, kittest::Queryable};

        let source: TraceSource = serde_json::from_value(serde_json::json!({
            "kind": "protocol_artifact",
            "path": "/tmp/protocol/review.json",
            "content_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        }))
        .expect("trace source");
        let harness = Harness::builder()
            .with_size(egui::Vec2::new(900.0, 240.0))
            .build_ui(move |ui| render_source(ui, &source));

        assert_eq!(
            harness
                .get_all_by_label("/tmp/protocol/review.json")
                .count(),
            1
        );
        assert_eq!(
            harness
                .get_all_by_label(
                    "sha256: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                )
                .count(),
            1
        );
    }

    fn neighborhood_call(index: usize, tool: &str) -> serde_json::Value {
        serde_json::json!({
            "index": index,
            "turn": 1,
            "tool_name": tool,
            "tool_kind": "read",
            "failed": false,
            "latency_ms": 7,
            "summary": "read source",
            "args_preview": "src/lib.rs",
            "result_preview": "source"
        })
    }

    fn tool_execution() -> ToolExecutionRecord {
        serde_json::from_value(serde_json::json!({
            "request": {
                "request_id": "request-1",
                "parent_id": "parent-1",
                "call_id": "call-1",
                "tool": "read_file",
                "arguments": {"path": "src/lib.rs"}
            },
            "result": {
                "status": "Completed",
                "request_id": "request-1",
                "parent_id": "parent-1",
                "call_id": "call-1",
                "tool": "read_file",
                "content": "pub fn fixture() {}",
                "ui_payload": null,
                "latency_ms": 7
            },
            "latency_ms": 7
        }))
        .expect("tool execution")
    }

    fn assessment(index: usize, tool: &str) -> LocalAnalysisAssessment {
        serde_json::from_value(serde_json::json!({
            "packet": {
                "subject_id": "org__repo-1",
                "target_kind": "focal_call",
                "target_id": format!("call:{index}"),
                "scope_summary": format!("focal call [{index}] {tool} in bounded neighborhood"),
                "total_calls_in_scope": 1,
                "total_calls_in_run": 2,
                "turn_span": [1],
                "focal_call_index": index,
                "calls": [neighborhood_call(index, tool)]
            },
            "signals": {
                "scope_turn_count": 1,
                "repeated_tool_name_count": 0,
                "distinct_tool_count": 1,
                "search_calls_in_scope": 0,
                "read_calls_in_scope": 1,
                "browse_calls_in_scope": 0,
                "edit_calls_in_scope": 0,
                "execute_calls_in_scope": 0,
                "failed_calls_in_scope": 0,
                "similar_search_neighbors": 0,
                "directory_pivots": 0
            },
            "usefulness": {
                "verdict": "key_progress",
                "confidence": "high",
                "rationale": "It found the target."
            },
            "redundancy": {
                "verdict": "distinct",
                "confidence": "high",
                "rationale": "It was the first read."
            },
            "recoverability": {
                "verdict": "no_recovery_needed",
                "confidence": "high",
                "rationale": "The call succeeded."
            },
            "overall": "focused_progress",
            "overall_confidence": "high",
            "synthesis_rationale": "The read directly advanced the task."
        }))
        .expect("local analysis assessment")
    }
}
