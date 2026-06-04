use crate::ui::eval_protocol::EvalProtocolDashboard;
use eframe::egui;
use ploke_tree::Graph;

use super::context_strip::render_run_evidence_context_strip;
use super::fields::*;
use super::{
    InspectorRenderCache, format_f64, render_cached_code_block, render_patch_artifact_details,
    render_run_record_tool_steps, render_token_usage, response_finish_reason_label,
    show_inspector_collapsing, turn_outcome_elapsed_secs, turn_outcome_error, turn_outcome_label,
    turn_outcome_tool_count,
};
use crate::ui::text::style::inspector_error_text_color;

fn fresh_kv_grid_row(ui: &mut egui::Ui, key: &str, value: &str, monospace_value: bool) {
    ui.label(key);
    if monospace_value {
        ui.monospace(value);
    } else {
        ui.label(value);
    }
    ui.end_row();
}

pub(crate) fn render_run_level_llm_trace_for_graph(
    ui: &mut egui::Ui,
    graph: &Graph,
    render_cache: &mut InspectorRenderCache,
) {
    let dashboard = EvalProtocolDashboard::from_graph(graph);
    let Some(run_records) = dashboard.run_records() else {
        cached_kv_id(ui, render_cache, "run llm trace", "not_available");
        return;
    };
    if run_records.index.is_empty() {
        cached_kv_id(ui, render_cache, "run llm trace", "none");
        return;
    }

    render_run_evidence_context_strip(
        ui,
        render_cache.run_evidence_source_label(),
        render_cache.run_evidence_catalog_error(),
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Run LLM Trace").default_open(true),
        |ui| {
            let mut records = itoa::Buffer::new();
            let mut turn_count = itoa::Buffer::new();
            egui::Grid::new("run-llm-trace-summary")
                .num_columns(2)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    fresh_kv_grid_row(ui, "records", records.format(run_records.index.len()), true);
                    if let Some(turns) = dashboard.run_records_total_turn_count() {
                        fresh_kv_grid_row(ui, "turns", turn_count.format(turns), true);
                    } else {
                        fresh_kv_grid_row(ui, "turns", "not_recorded", true);
                    }
                });
            if run_records.index.len() == 1 {
                let (record_key, record) = run_records.index.iter().next().expect("len checked");
                render_run_record_llm_trace_metadata_chips(ui, render_cache, record_key, record);
                render_run_record_llm_trace_turns(
                    ui,
                    render_cache,
                    "agent-trace",
                    record_key,
                    record,
                );
            } else {
                for (record_index, (record_key, record)) in run_records.index.iter().enumerate() {
                    render_run_record_llm_trace_for_record(
                        ui,
                        render_cache,
                        "agent-trace",
                        record_index,
                        record_key,
                        record,
                    );
                }
            }
        },
    );
}

fn render_run_record_llm_trace_metadata_chips(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    record_key: &str,
    record: &ploke_records::run_record::RunRecord,
) {
    egui::Grid::new(("run-llm-trace-metadata", record_key))
        .num_columns(2)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            ui.label("record");
            cached_path_value(
                ui,
                render_cache,
                ("agent-trace-record", record_key),
                record_key,
            );
            ui.end_row();
            fresh_kv_grid_row(ui, "manifest", record.manifest_id.as_str(), true);
            fresh_kv_grid_row(
                ui,
                "instance",
                record.metadata.benchmark.instance_id.as_str(),
                true,
            );
            if let Some(model) = record.metadata.agent.model_id.as_deref() {
                fresh_kv_grid_row(ui, "model", model, true);
            }
            if let Some(provider) = record.metadata.agent.provider.as_deref() {
                fresh_kv_grid_row(ui, "provider", provider, true);
            }
            let mut turns = itoa::Buffer::new();
            fresh_kv_grid_row(
                ui,
                "turns",
                turns.format(record.phases.agent_turns.len()),
                true,
            );
        });
}

fn render_run_record_llm_trace_turns(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    record: &ploke_records::run_record::RunRecord,
) {
    for (turn_index, turn) in record.phases.agent_turns.iter().enumerate() {
        render_run_record_turn_llm_trace(
            ui,
            render_cache,
            scope,
            record_key,
            turn_index,
            record,
            turn,
            true,
        );
    }
}

fn render_run_record_llm_trace_for_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_index: usize,
    record_key: &str,
    record: &ploke_records::run_record::RunRecord,
) {
    let instance_id = record.metadata.benchmark.instance_id.as_str();
    let manifest_id = record.manifest_id.as_str();
    let title = if instance_id == manifest_id {
        manifest_id.to_owned()
    } else {
        format!("{instance_id} {manifest_id}")
    };
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((scope, "run-record-llm", record_key, record_index))
            .default_open(record_index == 0),
        |ui| {
            render_run_record_llm_trace_metadata_chips(ui, render_cache, record_key, record);
            render_run_record_llm_trace_turns(ui, render_cache, scope, record_key, record);
        },
    );
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::INSPECTOR_AGENT_TRACE_LLM_TRACE)]
pub(crate) fn render_run_record_turn_llm_trace(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    record: &ploke_records::run_record::RunRecord,
    turn: &ploke_records::run_record::TurnRecord,
    include_tool_steps: bool,
) {
    if turn_index > 0 {
        ui.separator();
    }
    render_run_record_turn_llm_trace_summary(ui, render_cache, turn);
    if include_tool_steps {
        render_run_record_tool_steps(ui, render_cache, turn.tool_calls.as_slice());
    }
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("timing & outcome")
            .id_salt((scope, "run-record-turn-timing", record_key, turn_index))
            .default_open(false),
        |ui| {
            render_run_record_turn_llm_trace_timing(ui, render_cache, turn);
        },
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("LLM & artifacts")
            .id_salt((scope, "run-record-turn-llm-bundle", record_key, turn_index))
            .default_open(false),
        |ui| {
            render_run_record_turn_llm_trace_bundle(
                ui,
                render_cache,
                scope,
                record_key,
                turn_index,
                record,
                turn,
            );
        },
    );
}

fn render_run_record_turn_llm_trace_summary(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turn: &ploke_records::run_record::TurnRecord,
) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                cached_label(ui, render_cache, "turn");
                let mut turn_number = itoa::Buffer::new();
                cached_monospace_label(ui, render_cache, turn_number.format(turn.turn_number));
                cached_label(ui, render_cache, "outcome");
                cached_monospace_label(ui, render_cache, turn_outcome_label(&turn.outcome));
                cached_label(ui, render_cache, "tool steps");
                let mut tool_steps = itoa::Buffer::new();
                cached_monospace_label(ui, render_cache, tool_steps.format(turn.tool_calls.len()));
                let failed_tools = turn
                    .tool_calls
                    .iter()
                    .filter(|tool| {
                        matches!(
                            tool.result,
                            ploke_records::run_record::ToolResult::Failed(_)
                        )
                    })
                    .count();
                if failed_tools > 0 {
                    cached_label(ui, render_cache, "failed");
                    let mut failed = itoa::Buffer::new();
                    ui.label(
                        egui::RichText::new(failed.format(failed_tools))
                            .monospace()
                            .color(inspector_error_text_color(ui)),
                    );
                }
                if let Some(count) = turn_outcome_tool_count(&turn.outcome) {
                    cached_label(ui, render_cache, "outcome tools");
                    let mut outcome_tools = itoa::Buffer::new();
                    cached_monospace_label(ui, render_cache, outcome_tools.format(count));
                }
            });
            if let Some(message) = turn_outcome_error(&turn.outcome) {
                cached_label(ui, render_cache, "outcome error");
                cached_wrapped_monospace_label(ui, render_cache, message);
            }
        });
}

fn render_run_record_turn_llm_trace_timing(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turn: &ploke_records::run_record::TurnRecord,
) {
    cached_kv_text(ui, render_cache, "started", turn.started_at.as_str());
    cached_kv_text(ui, render_cache, "ended", turn.ended_at.as_str());
    cached_kv_i64(ui, render_cache, "db micros", turn.db_timestamp_micros);
    if let Some(message) = turn_outcome_error(&turn.outcome) {
        cached_kv_text(ui, render_cache, "outcome error", message);
    }
    if let Some(elapsed) = turn_outcome_elapsed_secs(&turn.outcome) {
        cached_kv_u64(ui, render_cache, "elapsed secs", elapsed);
    }
}

fn render_run_record_turn_llm_trace_bundle(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    record: &ploke_records::run_record::RunRecord,
    turn: &ploke_records::run_record::TurnRecord,
) {
    if let Some(request) = turn.llm_request.as_ref() {
        render_llm_request_record(ui, render_cache, scope, record_key, turn_index, request);
    } else {
        cached_kv_id(ui, render_cache, "llm request", "missing");
    }
    if let Some(response) = turn.llm_response.as_ref() {
        render_llm_response_record(
            ui,
            render_cache,
            scope,
            "turn-response",
            record_key,
            turn_index,
            response,
        );
    } else {
        cached_kv_id(ui, render_cache, "llm response", "missing");
    }
    render_agent_turn_artifact_trace(ui, render_cache, scope, record_key, turn_index, turn);
    if let Some(model) = record.metadata.agent.model_id.as_deref() {
        cached_kv_id(ui, render_cache, "record model", model);
    }
}

fn render_llm_request_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    request: &ploke_records::run_record::ChatRequestRecord,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("llm request").default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "model", request.model.as_str());
            cached_kv_usize(ui, render_cache, "messages", request.messages.len());
            for (message_index, message) in request.messages.iter().enumerate() {
                render_request_message_record(
                    ui,
                    render_cache,
                    scope,
                    record_key,
                    turn_index,
                    message_index,
                    message,
                );
            }
        },
    );
}

fn render_request_message_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    message_index: usize,
    message: &ploke_records::agent_turn::RequestMessageRecord,
) {
    let title = format!(
        "message {} {}",
        message_index + 1,
        request_role_label(message.role)
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((scope, "llm-message", record_key, turn_index, message_index))
            .default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "role", request_role_label(message.role));
            if let Some(tool_call_id) = message.tool_call_id.as_deref() {
                cached_kv_id(ui, render_cache, "tool call id", tool_call_id);
            }
            cached_label(ui, render_cache, "content");
            render_cached_code_block(
                ui,
                render_cache,
                (
                    scope,
                    "llm-message-content",
                    record_key,
                    turn_index,
                    message_index,
                ),
                message.content.as_str(),
            );
            if let Some(tool_calls) = message.tool_calls.as_ref() {
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new("provider tool calls").default_open(false),
                    |ui| {
                        cached_kv_usize(ui, render_cache, "tool calls", tool_calls.len());
                        for (tool_index, tool_call) in tool_calls.iter().enumerate() {
                            render_provider_tool_call_record(
                                ui,
                                render_cache,
                                scope,
                                record_key,
                                turn_index,
                                message_index,
                                tool_index,
                                tool_call,
                            );
                        }
                    },
                );
            }
        },
    );
}

fn render_provider_tool_call_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    message_index: usize,
    tool_index: usize,
    tool_call: &ploke_records::agent_turn::ProviderToolCallRecord,
) {
    let title = format!(
        "tool call {} {}",
        tool_index + 1,
        tool_call.function.name.as_str()
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((
                scope,
                "provider-tool-call",
                record_key,
                turn_index,
                message_index,
                tool_index,
            ))
            .default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "call id", tool_call.call_id.as_str());
            cached_kv_id(ui, render_cache, "tool", tool_call.function.name.as_str());
            cached_label(ui, render_cache, "arguments");
            render_cached_code_block(
                ui,
                render_cache,
                (
                    scope,
                    "provider-tool-call-arguments",
                    record_key,
                    turn_index,
                    message_index,
                    tool_index,
                ),
                tool_call.function.arguments.as_str(),
            );
        },
    );
}

fn render_llm_response_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    id_kind: &'static str,
    record_key: &str,
    turn_index: usize,
    response: &ploke_records::agent_turn::LlmResponseRecord,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("llm response")
            .id_salt((scope, id_kind, record_key, turn_index))
            .default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "model", response.model.as_str());
            if let Some(reason) = response.finish_reason.as_ref() {
                cached_kv_id(
                    ui,
                    render_cache,
                    "finish",
                    response_finish_reason_label(reason),
                );
            }
            if let Some(usage) = response.usage {
                render_token_usage(ui, render_cache, usage);
            }
            if let Some(metadata) = response.metadata.as_ref() {
                cached_kv_text(ui, render_cache, "cost", format_f64(metadata.cost).as_str());
                cached_kv_text(
                    ui,
                    render_cache,
                    "tokens/sec",
                    format!("{:.3}", metadata.performance.tokens_per_second).as_str(),
                );
            }
            cached_label(ui, render_cache, "content");
            render_cached_code_block(
                ui,
                render_cache,
                (scope, id_kind, "content", record_key, turn_index),
                response.content.as_str(),
            );
        },
    );
}

fn render_agent_turn_artifact_trace(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    turn: &ploke_records::run_record::TurnRecord,
) {
    let Some(artifact) = turn.agent_turn_artifact.as_ref() else {
        cached_kv_id(ui, render_cache, "agent-turn artifact", "not_recorded");
        return;
    };

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("agent-turn artifact")
            .id_salt((scope, "agent-turn-artifact", record_key, turn_index))
            .default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "task", artifact.task_id.as_str());
            cached_kv_id(
                ui,
                render_cache,
                "selected model",
                artifact.selected_model.as_str(),
            );
            cached_kv_id(
                ui,
                render_cache,
                "user message",
                artifact.user_message_id.as_str(),
            );
            cached_kv_usize(ui, render_cache, "events", artifact.events.len());
            if let Some(prompt_debug) = artifact.prompt_debug.as_deref() {
                cached_label(ui, render_cache, "prompt debug");
                render_cached_code_block(
                    ui,
                    render_cache,
                    (scope, "agent-turn-prompt-debug", record_key, turn_index),
                    prompt_debug,
                );
            }
            if !artifact.llm_prompt.is_empty() {
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new("artifact llm prompt").default_open(false),
                    |ui| {
                        cached_kv_usize(ui, render_cache, "messages", artifact.llm_prompt.len());
                        for (message_index, message) in artifact.llm_prompt.iter().enumerate() {
                            render_request_message_record(
                                ui,
                                render_cache,
                                scope,
                                record_key,
                                turn_index,
                                message_index,
                                message,
                            );
                        }
                    },
                );
            }
            if let Some(response) = artifact.llm_response.as_deref() {
                cached_label(ui, render_cache, "artifact llm response");
                render_cached_code_block(
                    ui,
                    render_cache,
                    (scope, "agent-turn-llm-response", record_key, turn_index),
                    response,
                );
            }
            if let Some(final_message) = artifact.final_assistant_message.as_ref() {
                render_message_snapshot(ui, render_cache, "final assistant", final_message);
            }
            if let Some(terminal) = artifact.terminal_record.as_ref() {
                render_turn_finished_record(ui, render_cache, "terminal", terminal);
            }
            render_patch_artifact_details(
                ui,
                render_cache,
                scope,
                "agent-turn",
                record_key,
                turn_index,
                &artifact.patch_artifact,
            );
            render_observed_turn_events(
                ui,
                render_cache,
                scope,
                record_key,
                turn_index,
                artifact.events.as_slice(),
            );
        },
    );
}

fn render_observed_turn_events(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    events: &[ploke_records::agent_turn::ObservedTurnEventRecord],
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("observed events").default_open(false),
        |ui| {
            cached_kv_usize(ui, render_cache, "events", events.len());
            for (event_index, event) in events.iter().enumerate() {
                render_observed_turn_event(
                    ui,
                    render_cache,
                    scope,
                    record_key,
                    turn_index,
                    event_index,
                    event,
                );
            }
        },
    );
}

fn render_observed_turn_event(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    event_index: usize,
    event: &ploke_records::agent_turn::ObservedTurnEventRecord,
) {
    use ploke_records::agent_turn::ObservedTurnEventRecord;

    let title = format!(
        "event {} {}",
        event_index + 1,
        observed_turn_event_label(event)
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((
                scope,
                "observed-turn-event",
                record_key,
                turn_index,
                event_index,
            ))
            .default_open(false),
        |ui| match event {
            ObservedTurnEventRecord::DebugCommand(command) => {
                render_cached_code_block(
                    ui,
                    render_cache,
                    (scope, "debug-command", record_key, turn_index, event_index),
                    command,
                );
            }
            ObservedTurnEventRecord::LlmEvent(event) => {
                render_cached_code_block(
                    ui,
                    render_cache,
                    (scope, "llm-event", record_key, turn_index, event_index),
                    event,
                );
            }
            ObservedTurnEventRecord::LlmResponse(response) => {
                render_llm_response_record(
                    ui,
                    render_cache,
                    scope,
                    "event-llm-response",
                    record_key,
                    turn_index,
                    response,
                );
            }
            ObservedTurnEventRecord::ToolRequested(request) => {
                cached_kv_id(ui, render_cache, "request", request.request_id.as_str());
                cached_kv_id(ui, render_cache, "parent", request.parent_id.as_str());
                cached_kv_id(ui, render_cache, "call", request.call_id.as_str());
                cached_kv_id(ui, render_cache, "tool", request.tool.as_str());
                cached_label(ui, render_cache, "arguments");
                render_cached_code_block(
                    ui,
                    render_cache,
                    (
                        scope,
                        "event-tool-request",
                        record_key,
                        turn_index,
                        event_index,
                    ),
                    request.arguments.as_str(),
                );
            }
            ObservedTurnEventRecord::ToolCompleted(completed) => {
                cached_kv_id(ui, render_cache, "request", completed.request_id.as_str());
                cached_kv_id(ui, render_cache, "parent", completed.parent_id.as_str());
                cached_kv_id(ui, render_cache, "call", completed.call_id.as_str());
                cached_kv_id(ui, render_cache, "tool", completed.tool.as_str());
                cached_kv_u64(ui, render_cache, "latency ms", completed.latency_ms);
                cached_label(ui, render_cache, "content");
                render_cached_code_block(
                    ui,
                    render_cache,
                    (
                        scope,
                        "event-tool-completed",
                        record_key,
                        turn_index,
                        event_index,
                    ),
                    completed.content.as_str(),
                );
            }
            ObservedTurnEventRecord::ToolFailed(failed) => {
                cached_kv_id(ui, render_cache, "request", failed.request_id.as_str());
                cached_kv_id(ui, render_cache, "parent", failed.parent_id.as_str());
                cached_kv_id(ui, render_cache, "call", failed.call_id.as_str());
                if let Some(tool) = failed.tool.as_deref() {
                    cached_kv_id(ui, render_cache, "tool", tool);
                }
                cached_kv_u64(ui, render_cache, "latency ms", failed.latency_ms);
                cached_label(ui, render_cache, "error");
                render_cached_code_block(
                    ui,
                    render_cache,
                    (
                        scope,
                        "event-tool-failed",
                        record_key,
                        turn_index,
                        event_index,
                    ),
                    failed.error.as_str(),
                );
            }
            ObservedTurnEventRecord::MessageUpdated(message) => {
                render_message_snapshot(ui, render_cache, "message", message);
            }
            ObservedTurnEventRecord::TurnFinished(finished) => {
                render_turn_finished_record(ui, render_cache, "finished", finished);
            }
        },
    );
}

fn request_role_label(role: ploke_records::agent_turn::RequestRoleRecord) -> &'static str {
    match role {
        ploke_records::agent_turn::RequestRoleRecord::User => "user",
        ploke_records::agent_turn::RequestRoleRecord::Assistant => "assistant",
        ploke_records::agent_turn::RequestRoleRecord::System => "system",
        ploke_records::agent_turn::RequestRoleRecord::Tool => "tool",
    }
}

fn observed_turn_event_label(
    event: &ploke_records::agent_turn::ObservedTurnEventRecord,
) -> &'static str {
    match event {
        ploke_records::agent_turn::ObservedTurnEventRecord::DebugCommand(_) => "debug_command",
        ploke_records::agent_turn::ObservedTurnEventRecord::LlmEvent(_) => "llm_event",
        ploke_records::agent_turn::ObservedTurnEventRecord::LlmResponse(_) => "llm_response",
        ploke_records::agent_turn::ObservedTurnEventRecord::ToolRequested(_) => "tool_requested",
        ploke_records::agent_turn::ObservedTurnEventRecord::ToolCompleted(_) => "tool_completed",
        ploke_records::agent_turn::ObservedTurnEventRecord::ToolFailed(_) => "tool_failed",
        ploke_records::agent_turn::ObservedTurnEventRecord::MessageUpdated(_) => "message_updated",
        ploke_records::agent_turn::ObservedTurnEventRecord::TurnFinished(_) => "turn_finished",
    }
}

fn render_message_snapshot(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    label: &str,
    message: &ploke_records::agent_turn::MessageSnapshotRecord,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(label).default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "id", message.id.as_str());
            cached_kv_id(ui, render_cache, "kind", message.kind.as_str());
            cached_kv_id(ui, render_cache, "status", message.status.as_str());
            if let Some(tool_call_id) = message.tool_call_id.as_deref() {
                cached_kv_id(ui, render_cache, "tool call", tool_call_id);
            }
            cached_kv_usize(ui, render_cache, "content len", message.content_len);
            cached_label(ui, render_cache, "preview");
            cached_wrapped_monospace_label(ui, render_cache, message.content_preview.as_str());
        },
    );
}

fn render_turn_finished_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    label: &str,
    finished: &ploke_records::agent_turn::TurnFinishedRecord,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(label).default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "session", finished.session_id.as_str());
            cached_kv_id(ui, render_cache, "request", finished.request_id.as_str());
            cached_kv_id(ui, render_cache, "parent", finished.parent_id.as_str());
            cached_kv_id(
                ui,
                render_cache,
                "assistant message",
                finished.assistant_message_id.as_str(),
            );
            cached_kv_id(ui, render_cache, "outcome", finished.outcome.as_str());
            if let Some(error_id) = finished.error_id.as_deref() {
                cached_kv_id(ui, render_cache, "error", error_id);
            }
            cached_kv_u32(ui, render_cache, "attempts", finished.attempts);
            cached_label(ui, render_cache, "summary");
            cached_wrapped_monospace_label(ui, render_cache, finished.summary.as_str());
        },
    );
}
