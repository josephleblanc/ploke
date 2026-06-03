//! Bottom run timeline: multi-track colored spans from passive run-record evidence.

use crate::ui::inspector::{tool_execution_name, tool_execution_status_label, turn_outcome_label};
use crate::ui::theme::{PaletteTokens, tokens_from_ui};
use crate::ui::view::GraphViewDiagnostics;
use eframe::egui::{self, Color32, Id, Rect, Sense, Stroke, Ui};
use ploke_records::run_record::{RunRecord, ToolResult, TurnOutcome, TurnRecord};
use ploke_tree::graph::Graph;
use ploke_tree::{ComparedRunArm, RunRecordStats};

const TRACK_LABEL_WIDTH: f32 = 76.0;
const TRACK_HEIGHT: f32 = 18.0;
const TRACK_GAP: f32 = 3.0;
const RULER_HEIGHT: f32 = 14.0;
const FOOTER_HEIGHT: f32 = 16.0;
const SPAN_ROUNDING: f32 = 2.0;
const MIN_SPAN_WIDTH_PX: f32 = 3.0;

#[derive(Debug, Clone)]
struct TimelineSpan {
    start: f32,
    end: f32,
    fill: Color32,
    tooltip: String,
}

#[derive(Debug, Clone)]
struct TimelineTrack {
    name: &'static str,
    spans: Vec<TimelineSpan>,
}

#[derive(Debug, Clone)]
struct TimelineModel {
    axis_label: String,
    tracks: Vec<TimelineTrack>,
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "timeline")
)]
pub(crate) fn render_bottom_timeline(
    ui: &mut Ui,
    graph: &Graph,
    diagnostics: Option<&GraphViewDiagnostics>,
    selection_synced: bool,
) {
    let panel_width = ui.max_rect().width();
    ui.set_min_width(panel_width);
    ui.set_width(panel_width);

    let tokens = tokens_from_ui(ui);
    let model = build_timeline_model(graph, tokens);
    ui.vertical(|ui| {
        ui.set_width(ui.available_width());
        if let Some(model) = model.as_ref() {
            render_tracks(ui, model);
            render_ruler(ui, model);
        } else {
            ui.label(
                egui::RichText::new(
                    "No run-record spans — load a graph with compressed run records",
                )
                .small()
                .weak(),
            );
        }
        ui.add_space(2.0);
        render_timeline_footer(ui, diagnostics, selection_synced, model.as_ref());
    });
}

fn render_timeline_footer(
    ui: &mut Ui,
    diagnostics: Option<&GraphViewDiagnostics>,
    selection_synced: bool,
    model: Option<&TimelineModel>,
) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), FOOTER_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
            if let Some(model) = model {
                ui.label(model.axis_label.as_str());
                ui.separator();
            }
            if let Some(diagnostics) = diagnostics {
                ui.label(format!(
                    "graph nodes={}, edges={}",
                    diagnostics.node_count, diagnostics.edge_count
                ));
            }
            ui.separator();
            ui.label(format!(
                "selection={}",
                if selection_synced {
                    "synced"
                } else {
                    "not_applicable"
                }
            ));
            ui.separator();
            ui.label("order=turn_index");
        },
    );
}

fn render_tracks(ui: &mut Ui, model: &TimelineModel) {
    let track_area_width = (ui.available_width() - TRACK_LABEL_WIDTH).max(80.0);
    for track in &model.tracks {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(TRACK_LABEL_WIDTH, TRACK_HEIGHT),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                    ui.label(
                        egui::RichText::new(track.name)
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                },
            );
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(track_area_width, TRACK_HEIGHT), Sense::hover());
            paint_track(ui, rect, &track.spans);
        });
        ui.add_space(TRACK_GAP);
    }
}

fn render_ruler(ui: &mut Ui, model: &TimelineModel) {
    let Some(turn_track) = model.tracks.first() else {
        return;
    };
    if turn_track.spans.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        ui.allocate_exact_size(egui::vec2(TRACK_LABEL_WIDTH, RULER_HEIGHT), Sense::hover());
        let track_area_width = (ui.available_width() - TRACK_LABEL_WIDTH).max(80.0);
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(track_area_width, RULER_HEIGHT), Sense::hover());
        let visuals = ui.visuals();
        let stroke = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
        ui.painter().hline(rect.x_range(), rect.bottom(), stroke);
        for (index, span) in turn_track.spans.iter().enumerate() {
            let x = rect.left() + rect.width() * span.start;
            ui.painter().vline(x, rect.y_range(), stroke);
            let tick_label = format!("T{}", index + 1);
            ui.painter().text(
                egui::pos2(x + 2.0, rect.top()),
                egui::Align2::LEFT_TOP,
                tick_label,
                egui::FontId::proportional(9.0),
                visuals.weak_text_color(),
            );
        }
    });
}

fn paint_track(ui: &mut Ui, rect: Rect, spans: &[TimelineSpan]) {
    let painter = ui.painter();
    let track_bg = ui.visuals().widgets.noninteractive.bg_fill;
    painter.rect_filled(rect, SPAN_ROUNDING, track_bg);
    painter.rect_stroke(
        rect,
        SPAN_ROUNDING,
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );

    let span_response_id = Id::new(("timeline_span", rect.min.x.to_bits(), rect.min.y.to_bits()));
    for (index, span) in spans.iter().enumerate() {
        let mut left = rect.left() + rect.width() * span.start;
        let mut right = rect.left() + rect.width() * span.end;
        if right - left < MIN_SPAN_WIDTH_PX {
            right = (left + MIN_SPAN_WIDTH_PX).min(rect.right());
        }
        left = left.clamp(rect.left(), rect.right());
        right = right.clamp(rect.left(), rect.right());
        if right <= left {
            continue;
        }
        let span_rect = Rect::from_min_max(
            egui::pos2(left, rect.top() + 1.0),
            egui::pos2(right, rect.bottom() - 1.0),
        );
        let response = ui.interact(span_rect, span_response_id.with(index), Sense::hover());
        painter.rect_filled(span_rect, SPAN_ROUNDING, span.fill);
        if response.hovered() {
            response.show_tooltip_ui(|ui| {
                ui.label(span.tooltip.as_str());
            });
        }
    }
}

fn build_timeline_model(graph: &Graph, tokens: PaletteTokens) -> Option<TimelineModel> {
    let (record, stats) = primary_run_record(graph)?;
    let mut tracks = Vec::new();
    let turn_track = build_turn_track(record, tokens);
    if turn_track.spans.is_empty() {
        return None;
    }
    tracks.push(turn_track);
    tracks.push(build_tool_track(record, tokens));
    if let Some(protocol_track) = build_protocol_track(graph) {
        if !protocol_track.spans.is_empty() {
            tracks.push(protocol_track);
        }
    }
    let axis_label = format!(
        "treatment · turns={} · tools={} ({})",
        stats.turn_count, stats.tool_call_count, record.manifest_id
    );
    Some(TimelineModel { axis_label, tracks })
}

fn primary_run_record(graph: &Graph) -> Option<(&RunRecord, &RunRecordStats)> {
    let evidence = graph.run_records()?;
    for refs in evidence.refs_by_branch.values() {
        for record_ref in refs
            .iter()
            .filter(|record_ref| record_ref.arm == ComparedRunArm::Treatment)
        {
            let record = evidence.index.get(&record_ref.record_key)?;
            let stats = evidence.stats.get(&record_ref.record_key)?;
            return Some((record, stats));
        }
    }
    evidence
        .index
        .iter()
        .next()
        .and_then(|(key, record)| evidence.stats.get(key).map(|stats| (record, stats)))
}

fn build_turn_track(record: &RunRecord, tokens: PaletteTokens) -> TimelineTrack {
    let weights = turn_weights(record);
    let total: f32 = weights.iter().sum::<f32>().max(1.0);
    let mut spans = Vec::new();
    let mut cursor = 0.0f32;
    for (turn, weight) in record.phases.agent_turns.iter().zip(weights.iter()) {
        let width = weight / total;
        let start = cursor;
        let end = cursor + width;
        cursor = end;
        spans.push(TimelineSpan {
            start,
            end,
            fill: turn_color(turn, tokens),
            tooltip: turn_tooltip(turn),
        });
    }
    TimelineTrack {
        name: "Turns",
        spans,
    }
}

fn build_tool_track(record: &RunRecord, tokens: PaletteTokens) -> TimelineTrack {
    let weights = turn_weights(record);
    let total: f32 = weights.iter().sum::<f32>().max(1.0);
    let mut spans = Vec::new();
    let mut cursor = 0.0f32;
    for (turn, turn_weight) in record.phases.agent_turns.iter().zip(weights.iter()) {
        let turn_width = turn_weight / total;
        let turn_start = cursor;
        let tool_weight_sum: f32 = turn
            .tool_calls
            .iter()
            .map(|tool| tool_weight(tool))
            .sum::<f32>()
            .max(1.0);
        let mut tool_cursor = 0.0f32;
        for tool in &turn.tool_calls {
            let tool_width = (tool_weight(tool) / tool_weight_sum) * turn_width;
            let start = turn_start + tool_cursor;
            let end = start + tool_width;
            tool_cursor += tool_width;
            spans.push(TimelineSpan {
                start,
                end,
                fill: tool_color(tool, tokens),
                tooltip: format!(
                    "turn {} · {} · {} · {}ms",
                    turn.turn_number,
                    tool_execution_name(tool),
                    tool_execution_status_label(tool),
                    tool.latency_ms
                ),
            });
        }
        cursor += turn_width;
    }
    TimelineTrack {
        name: "Tool calls",
        spans,
    }
}

fn build_protocol_track(graph: &Graph) -> Option<TimelineTrack> {
    let artifacts = graph.protocol_artifacts()?;
    if artifacts.index.is_empty() {
        return None;
    }
    let mut entries: Vec<_> = artifacts.index.values().collect();
    entries.sort_by_key(|artifact| artifact.created_at_ms);
    let count = entries.len().max(1) as f32;
    let mut spans = Vec::new();
    for (index, artifact) in entries.into_iter().enumerate() {
        let start = index as f32 / count;
        let end = (index as f32 + 1.0) / count;
        spans.push(TimelineSpan {
            start,
            end,
            fill: protocol_color(artifact.procedure_name.as_str()),
            tooltip: format!(
                "{} · subject={} · created_at_ms={}",
                artifact.procedure_name, artifact.subject_id, artifact.created_at_ms
            ),
        });
    }
    Some(TimelineTrack {
        name: "Protocol",
        spans,
    })
}

fn turn_weights(record: &RunRecord) -> Vec<f32> {
    record.phases.agent_turns.iter().map(turn_weight).collect()
}

fn turn_weight(turn: &TurnRecord) -> f32 {
    let latency_sum: u64 = turn
        .tool_calls
        .iter()
        .map(|tool| tool.latency_ms.max(1))
        .sum();
    if latency_sum > 0 {
        latency_sum as f32
    } else {
        1.0
    }
}

fn tool_weight(tool: &ploke_records::run_record::ToolExecutionRecord) -> f32 {
    tool.latency_ms.max(1) as f32
}

fn turn_color(turn: &TurnRecord, tokens: PaletteTokens) -> Color32 {
    match &turn.outcome {
        TurnOutcome::Error { .. } => tokens.error,
        TurnOutcome::Timeout { .. } => tokens.warning,
        TurnOutcome::ToolCalls { .. } => tokens.info,
        TurnOutcome::Content => tokens.success,
    }
}

fn tool_color(
    tool: &ploke_records::run_record::ToolExecutionRecord,
    tokens: PaletteTokens,
) -> Color32 {
    match &tool.result {
        ToolResult::Failed(_) => tokens.error,
        ToolResult::Completed(_) => tokens.success,
    }
}

fn protocol_color(procedure_name: &str) -> Color32 {
    let hash = procedure_name.bytes().fold(0u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(u32::from(byte))
    });
    let hue = (hash % 360) as f32;
    let [r, g, b] = hsl_to_rgb(hue, 0.45, 0.42);
    Color32::from_rgb(r, g, b)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = match h as i32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    ]
}

fn turn_tooltip(turn: &TurnRecord) -> String {
    let outcome = turn_outcome_label(&turn.outcome);
    let tools = turn.tool_calls.len();
    format!(
        "turn {} · outcome={} · tools={} · started={}",
        turn.turn_number, outcome, tools, turn.started_at
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_records::agent_turn::{ToolCompletedRecord, ToolRequestRecord};
    use ploke_records::run_record::{
        RunPhases, RunRecord, ToolExecutionRecord, ToolResult, TurnOutcome, TurnRecord,
    };
    use ploke_records::tool_contracts::ToolArgumentsJson;

    fn sample_record() -> RunRecord {
        RunRecord {
            schema_version: "run-record.v1".to_owned(),
            manifest_id: "manifest-test".to_owned(),
            metadata: ploke_records::run_record::RunMetadata {
                run_arm: ploke_records::run_record::RunArm {
                    id: "arm".to_owned(),
                    role: ploke_records::run_record::RunArmRole::Treatment,
                    command: "run".to_owned(),
                    execution: "test".to_owned(),
                },
                benchmark: ploke_records::run_record::BenchmarkMetadata {
                    instance_id: "instance".to_owned(),
                    repo_root: std::path::PathBuf::from("/tmp/repo"),
                    base_sha: None,
                    issue: None,
                },
                agent: ploke_records::run_record::AgentMetadata::default(),
                runtime: ploke_records::run_record::RuntimeMetadata::default(),
                budget: ploke_records::run_record::EvalBudget {
                    max_turns: 40,
                    max_tool_calls: 200,
                    wall_clock_secs: 1800,
                },
            },
            phases: RunPhases {
                agent_turns: vec![
                    TurnRecord {
                        turn_number: 1,
                        started_at: "t0".to_owned(),
                        ended_at: "t1".to_owned(),
                        db_timestamp_micros: 0,
                        issue_prompt: String::new(),
                        llm_request: None,
                        llm_response: None,
                        tool_calls: vec![ToolExecutionRecord {
                            request: ToolRequestRecord {
                                request_id: "req".to_owned(),
                                parent_id: "parent".to_owned(),
                                call_id: "call".to_owned(),
                                tool: "read_file".to_owned(),
                                arguments: ToolArgumentsJson::from("{}"),
                            },
                            result: ToolResult::Completed(ToolCompletedRecord {
                                request_id: "req".to_owned(),
                                parent_id: "parent".to_owned(),
                                call_id: "call".to_owned(),
                                tool: "read_file".to_owned(),
                                content: String::new(),
                                ui_payload: None,
                                latency_ms: 40,
                            }),
                            latency_ms: 40,
                        }],
                        outcome: TurnOutcome::ToolCalls { count: 1 },
                        agent_turn_artifact: None,
                    },
                    TurnRecord {
                        turn_number: 2,
                        started_at: "t1".to_owned(),
                        ended_at: "t2".to_owned(),
                        db_timestamp_micros: 1,
                        issue_prompt: String::new(),
                        llm_request: None,
                        llm_response: None,
                        tool_calls: Vec::new(),
                        outcome: TurnOutcome::Content,
                        agent_turn_artifact: None,
                    },
                ],
                ..RunPhases::default()
            },
            db_time_travel_index: Vec::new(),
            conversation: Vec::new(),
            timing: None,
        }
    }

    #[test]
    fn turn_and_tool_tracks_cover_the_unit_interval() {
        let record = sample_record();
        let tokens = PaletteTokens::tokyo_night();
        let turn_track = build_turn_track(&record, tokens);
        assert_eq!(turn_track.spans.len(), 2);
        assert!((turn_track.spans.last().expect("last").end - 1.0).abs() < 0.001);

        let tool_track = build_tool_track(&record, tokens);
        assert_eq!(tool_track.spans.len(), 1);
        assert!((tool_track.spans[0].end - tool_track.spans[0].start) > 0.0);
    }
}
