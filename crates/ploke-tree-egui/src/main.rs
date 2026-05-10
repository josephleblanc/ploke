#[cfg(not(target_arch = "wasm32"))]
use std::{fs, path::PathBuf};

use eframe::egui::{self, Color32, RichText, ScrollArea};
use eframe::{App, Frame};
use ploke_tree_browser::{
    BrowserGranularity, PlaybackBrowserModel, PlaybackBrowserStep, RunSummary,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
const WEB_CANVAS_ID: &str = "ploke-tree-canvas";
#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_MODEL_PATH: &str = "/tmp/browser-model.json";

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Ploke Tree Browser",
        options,
        Box::new(|_cc| Ok(Box::new(PlokeTreeApp::new(DEFAULT_MODEL_PATH)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn wasm_start() -> Result<(), wasm_bindgen::JsValue> {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        if let Err(err) = start_web_app().await {
            log::error!("failed to start web app: {err:?}");
        }
    });
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn start_web_app() -> Result<(), wasm_bindgen::JsValue> {
    let window =
        web_sys::window().ok_or_else(|| wasm_bindgen::JsValue::from_str("missing window"))?;
    let document = window
        .document()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("missing document"))?;
    let canvas = document
        .get_element_by_id(WEB_CANVAS_ID)
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("missing ploke-tree canvas"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    let runner = Box::leak(Box::new(eframe::WebRunner::new()));

    runner
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|_cc| Ok(Box::new(PlokeTreeApp::new()))),
        )
        .await
}

struct PlokeTreeApp {
    #[cfg(not(target_arch = "wasm32"))]
    path_input: String,
    #[cfg(target_arch = "wasm32")]
    json_input: String,
    model: Option<PlaybackBrowserModel>,
    selected_index: Option<usize>,
    status_message: String,
}

impl PlokeTreeApp {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(default_path: &str) -> Self {
        let mut app = Self {
            path_input: default_path.to_owned(),
            model: None,
            selected_index: None,
            status_message: String::from("Ready"),
        };
        app.load_model();
        app
    }

    #[cfg(target_arch = "wasm32")]
    fn new() -> Self {
        Self {
            json_input: String::new(),
            model: None,
            selected_index: None,
            status_message: String::from("Ready"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_model(&mut self) {
        let path = PathBuf::from(self.path_input.trim());
        match fs::read_to_string(&path) {
            Ok(contents) => self.load_model_from_json(&contents, &path.display().to_string()),
            Err(err) => {
                self.model = None;
                self.selected_index = None;
                self.status_message = format!("Failed to read {}: {}", path.display(), err);
            }
        }
    }

    fn load_model_from_json(&mut self, json: &str, source: &str) {
        match serde_json::from_str::<PlaybackBrowserModel>(json) {
            Ok(model) => {
                let step_count = model.steps.len();
                self.selected_index = if step_count > 0 { Some(0) } else { None };
                self.status_message = format!("Loaded {} steps from {}", step_count, source);
                self.model = Some(model);
            }
            Err(err) => {
                self.model = None;
                self.selected_index = None;
                self.status_message = format!("Failed to parse {}: {}", source, err);
            }
        }
    }

    fn load_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|input| input.raw.dropped_files.clone());
        for file in dropped_files {
            if let Some(bytes) = file.bytes {
                let filename = file
                    .name
                    .clone()
                    .split('/')
                    .next_back()
                    .unwrap_or("dropped.json")
                    .to_owned();
                match std::str::from_utf8(bytes.as_ref()) {
                    Ok(json_text) => self.load_model_from_json(json_text, &filename),
                    Err(err) => {
                        self.model = None;
                        self.selected_index = None;
                        self.status_message =
                            format!("Dropped file {} is not UTF-8: {}", filename, err);
                    }
                }
                return;
            }

            #[cfg(not(target_arch = "wasm32"))]
            if let Some(path) = file.path.as_ref() {
                self.path_input = path.display().to_string();
                self.load_model();
                return;
            }
        }
    }

    fn selected_step(&self) -> Option<&PlaybackBrowserStep> {
        let model = self.model.as_ref()?;
        let index = self.selected_index?;
        model.steps.get(index)
    }
}

impl App for PlokeTreeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        self.load_dropped_files(ctx);

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            #[cfg(not(target_arch = "wasm32"))]
            ui.horizontal_wrapped(|ui| {
                ui.label("JSON path");
                let path_edit =
                    ui.add(egui::TextEdit::singleline(&mut self.path_input).desired_width(360.0));
                let load_clicked = ui.button("Load").clicked();
                let enter_pressed =
                    path_edit.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                if load_clicked || enter_pressed {
                    self.load_model();
                }
            });

            #[cfg(target_arch = "wasm32")]
            ui.horizontal_wrapped(|ui| {
                ui.label("JSON text");
                let parse_clicked = ui.button("Parse").clicked();
                if parse_clicked {
                    let json = self.json_input.clone();
                    self.load_model_from_json(&json, "pasted text");
                }
            });

            #[cfg(target_arch = "wasm32")]
            ui.add(
                egui::TextEdit::multiline(&mut self.json_input)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .hint_text(
                        "Paste PlaybackBrowserModel JSON, or drop a JSON file onto this page",
                    ),
            );

            ui.label(RichText::new(&self.status_message).small());

            if let Some(model) = self.model.as_ref() {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.monospace(format!("schema: {}", model.schema_version));
                    ui.separator();
                    ui.label(format!(
                        "granularity: {}",
                        granularity_label(model.granularity)
                    ));
                    ui.separator();
                    ui.label(format!("steps: {}", model.step_count));
                    if let Some(summary) = model.run_summary.as_ref() {
                        ui.separator();
                        run_summary_bar(ui, summary);
                    }
                });
            }
        });

        egui::SidePanel::right("detail_panel")
            .resizable(true)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.heading("Step detail");
                ui.separator();
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some(step) = self.selected_step() {
                            detail_panel(ui, step);
                        } else if self.model.is_some() {
                            ui.label("No step selected");
                        } else {
                            ui.label("Load a browser model to inspect steps");
                        }
                    });
            });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            if let Some(model) = self.model.as_ref() {
                let diagnostics = model_diagnostics(model);
                // Protocol snapshots are an upstream enrichment slot. Keep absence
                // handling in diagnostics instead of adding explanatory UI copy here.
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("warnings: {}", model.warning_count));
                    ui.separator();
                    diagnostics_label(ui, "eval", diagnostics.evaluation);
                    ui.separator();
                    diagnostics_label(ui, "surface", diagnostics.surface);
                    ui.separator();
                    diagnostics_label(ui, "protocol", diagnostics.protocol);
                });
            } else {
                ui.label("No model loaded");
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Timeline");
            ui.separator();

            if let Some(model) = self.model.as_ref() {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (index, step) in model.steps.iter().enumerate() {
                            let is_selected = self.selected_index == Some(index);
                            let response =
                                ui.selectable_label(is_selected, timeline_row_text(step));
                            if response.clicked() {
                                self.selected_index = Some(index);
                            }

                            ui.horizontal_wrapped(|ui| {
                                badge(
                                    ui,
                                    evidence_label(step.evidence),
                                    evidence_color(step.evidence),
                                );
                                if let Some(disposition) =
                                    step.evaluation.as_ref().map(|eval| eval.disposition)
                                {
                                    badge(
                                        ui,
                                        disposition_label(disposition),
                                        disposition_color(disposition),
                                    );
                                }
                                if let Some(kind) = step.fine_kind {
                                    ui.monospace(format!("kind: {}", fine_kind_label(kind)));
                                }
                                ui.monospace(format!("h{}", step.block_height));
                                if let Some(label) = step.label.as_deref() {
                                    ui.monospace(label);
                                }
                                if let Some(surface) = step.surface.as_ref() {
                                    ui.monospace(format!("target {}", surface.target_relpath));
                                }
                                if let Some(protocol) = step.protocol.as_ref() {
                                    badge(
                                        ui,
                                        format!(
                                            "protocol {}",
                                            protocol.intent_segmentation_count
                                                + protocol.tool_call_review_count
                                                + protocol.segment_review_count
                                                + protocol.issue_detection_count
                                                + protocol.synthesis_count
                                        ),
                                        Color32::from_rgb(218, 232, 246),
                                    );
                                }
                            });

                            ui.separator();
                        }
                    });
            } else {
                ui.label("Load a JSON model to view the timeline");
            }
        });
    }
}

fn timeline_row_text(step: &PlaybackBrowserStep) -> String {
    let kind = step
        .fine_kind
        .map(fine_kind_label)
        .unwrap_or("coarse".to_owned());
    let label = step.label.as_deref().unwrap_or("<no label>");
    format!(
        "#{}  {}  h{}  {}  {}",
        step.index,
        kind,
        step.block_height,
        evidence_label(step.evidence),
        label
    )
}

fn detail_panel(ui: &mut egui::Ui, step: &PlaybackBrowserStep) {
    ui.label(RichText::new(timeline_row_text(step)).strong());
    ui.separator();

    if let Some(evaluation) = step.evaluation.as_ref() {
        detail_kv(
            ui,
            "eval_disposition",
            disposition_label(evaluation.disposition),
        );
        detail_kv(
            ui,
            "tool_calls_total",
            evaluation.tool_calls_total.to_string(),
        );
        detail_kv(
            ui,
            "tool_calls_failed",
            evaluation.tool_calls_failed.to_string(),
        );
        detail_kv(
            ui,
            "patch_attempted",
            evaluation.patch_attempted.to_string(),
        );
        detail_kv(
            ui,
            "patch_apply_state",
            evaluation.patch_apply_state.as_deref().unwrap_or("<none>"),
        );
        detail_kv(ui, "convergence", evaluation.convergence.to_string());
        detail_kv(
            ui,
            "oracle_eligible",
            evaluation.oracle_eligible.to_string(),
        );
        detail_kv(ui, "aborted", evaluation.aborted.to_string());
        if let Some(reasons) = evaluation.reasons.as_ref() {
            for (index, reason) in reasons.iter().enumerate() {
                detail_kv(ui, &format!("reason {}", index + 1), reason);
            }
        }
    } else {
        detail_kv(ui, "eval_disposition", "<none>");
    }

    detail_kv(
        ui,
        "target_relpath",
        step.surface
            .as_ref()
            .map(|surface| surface.target_relpath.as_str())
            .unwrap_or("<none>"),
    );

    ui.separator();
    ui.collapsing("Advanced", |ui| {
        detail_kv(ui, "id", &step.id);
        detail_kv(ui, "block_hash", &step.block_hash);
        detail_kv(ui, "node_id", step.node_id.as_deref().unwrap_or("<none>"));
        detail_kv(
            ui,
            "branch_id",
            step.branch_id.as_deref().unwrap_or("<none>"),
        );
        detail_kv(
            ui,
            "candidate_id",
            step.candidate_id.as_deref().unwrap_or("<none>"),
        );
        detail_kv(
            ui,
            "selected_candidate",
            step.selected_candidate.as_deref().unwrap_or("<none>"),
        );
        detail_kv(ui, "warning_count", step.warning_count.to_string());
        detail_kv(
            ui,
            "considered_candidate_count",
            step.considered_candidate_count.to_string(),
        );
        if let Some(evaluation) = step.evaluation.as_ref() {
            detail_kv(
                ui,
                "nonempty_valid_patch",
                evaluation.nonempty_valid_patch.to_string(),
            );
        }
    });

    ui.separator();
    ui.collapsing("Provenance", |ui| {
        detail_kv(ui, "evidence", evidence_label(step.evidence));
        detail_kv(
            ui,
            "fine_kind",
            step.fine_kind
                .as_ref()
                .map(fine_kind_label)
                .unwrap_or_else(|| "<none>".to_owned()),
        );
        detail_kv(
            ui,
            "order",
            step.order
                .as_ref()
                .map(|order| format!("{order:?}"))
                .unwrap_or_else(|| "<none>".to_owned()),
        );

        if let Some(surface) = step.surface.as_ref() {
            detail_kv(
                ui,
                "patch_id",
                surface.patch_id.as_deref().unwrap_or("<none>"),
            );
            detail_kv(
                ui,
                "source_content_hash",
                surface.source_content_hash.as_deref().unwrap_or("<none>"),
            );
            detail_kv(
                ui,
                "proposed_content_hash",
                surface.proposed_content_hash.as_deref().unwrap_or("<none>"),
            );
            detail_kv(
                ui,
                "source_state_id",
                surface.source_state_id.as_deref().unwrap_or("<none>"),
            );
        }
    });

    if let Some(protocol) = step.protocol.as_ref() {
        ui.separator();
        ui.collapsing("Protocol artifacts", |ui| {
            detail_kv(
                ui,
                "intent_segmentation_count",
                protocol.intent_segmentation_count.to_string(),
            );
            detail_kv(
                ui,
                "tool_call_review_count",
                protocol.tool_call_review_count.to_string(),
            );
            detail_kv(
                ui,
                "segment_review_count",
                protocol.segment_review_count.to_string(),
            );
            detail_kv(
                ui,
                "issue_detection_count",
                protocol.issue_detection_count.to_string(),
            );
            detail_kv(ui, "synthesis_count", protocol.synthesis_count.to_string());
            detail_kv(
                ui,
                "model_id",
                protocol.model_id.as_deref().unwrap_or("<none>"),
            );
            detail_kv(
                ui,
                "provider_slug",
                protocol.provider_slug.as_deref().unwrap_or("<none>"),
            );
        });
    }
}

fn run_summary_bar(ui: &mut egui::Ui, summary: &RunSummary) {
    ui.label(format!(
        "campaign: {} | nodes: {} | generations: {} | blocks: {} | evals: {} (kept {}, rejected {}) | journal: {}{}",
        summary.campaign_id,
        summary.node_count,
        summary.generation_count,
        summary.sealed_block_count,
        summary.evaluation_count,
        summary.evaluations_kept,
        summary.evaluations_rejected,
        summary.journal_entry_count,
        summary
            .terminal_status
            .as_deref()
            .map(|status| format!(" | terminal: {}", status))
            .unwrap_or_default(),
    ));
}

fn detail_kv(ui: &mut egui::Ui, key: &str, value: impl AsRef<str>) {
    ui.horizontal_wrapped(|ui| {
        ui.monospace(format!("{}:", key));
        ui.label(value.as_ref());
    });
}

fn badge(ui: &mut egui::Ui, text: String, fill: Color32) {
    ui.label(
        RichText::new(text)
            .background_color(fill)
            .color(Color32::BLACK)
            .monospace(),
    );
}

fn granularity_label(granularity: BrowserGranularity) -> &'static str {
    match granularity {
        BrowserGranularity::CoarseHistory => "coarse_history",
        BrowserGranularity::FineHistory => "fine_history",
    }
}

fn fine_kind_label(kind: impl std::fmt::Debug) -> String {
    format!("{kind:?}")
        .replace("ParentStarted", "parent_started")
        .replace("EvaluationRecorded", "evaluation_recorded")
        .replace("CandidateConsidered", "candidate_considered")
        .replace("SuccessorSelected", "successor_selected")
        .replace("HistoryEntryAdmitted", "history_entry_admitted")
        .replace("HistoryBlockSealed", "history_block_sealed")
        .replace("SuccessorReadyAck", "successor_ready_ack")
        .replace("SuccessorCompletion", "successor_completion")
}

fn evidence_label(evidence: impl std::fmt::Debug) -> String {
    match format!("{evidence:?}").as_str() {
        "Projection" => "projection".to_owned(),
        "TypedRecord" => "typed_record".to_owned(),
        "AdmittedHistory" => "admitted_history".to_owned(),
        "SealedHistory" => "sealed_history".to_owned(),
        other => other.to_lowercase(),
    }
}

fn evidence_color(evidence: impl std::fmt::Debug) -> Color32 {
    match format!("{evidence:?}").as_str() {
        "Projection" => Color32::from_rgb(214, 228, 255),
        "TypedRecord" => Color32::from_rgb(218, 243, 224),
        "AdmittedHistory" => Color32::from_rgb(252, 235, 200),
        "SealedHistory" => Color32::from_rgb(232, 224, 255),
        _ => Color32::from_rgb(228, 228, 228),
    }
}

fn disposition_label(disposition: impl std::fmt::Debug) -> String {
    match format!("{disposition:?}").as_str() {
        "Keep" => "keep".to_owned(),
        "Reject" => "reject".to_owned(),
        other => other.to_lowercase(),
    }
}

fn disposition_color(disposition: impl std::fmt::Debug) -> Color32 {
    match format!("{disposition:?}").as_str() {
        "Keep" => Color32::from_rgb(200, 240, 208),
        "Reject" => Color32::from_rgb(246, 207, 207),
        _ => Color32::from_rgb(228, 228, 228),
    }
}

#[derive(Clone, Copy, Default)]
struct DiagnosticsBucket {
    not_applicable: usize,
    expected_absent: usize,
    unavailable_upstream: usize,
}

#[derive(Clone, Copy, Default)]
struct ModelDiagnostics {
    evaluation: DiagnosticsBucket,
    surface: DiagnosticsBucket,
    protocol: DiagnosticsBucket,
}

fn model_diagnostics(model: &PlaybackBrowserModel) -> ModelDiagnostics {
    let protocol_any_present = model.steps.iter().any(|step| step.protocol.is_some());
    let mut diagnostics = ModelDiagnostics::default();

    for step in &model.steps {
        let candidate_context = step.node_id.is_some()
            || step.branch_id.is_some()
            || step.candidate_id.is_some()
            || step
                .label
                .as_deref()
                .is_some_and(|label| label.starts_with("candidate:node-"));

        classify_enrichment(
            &mut diagnostics.evaluation,
            candidate_context,
            step.evaluation.is_some(),
            step.branch_id.is_some(),
        );
        classify_enrichment(
            &mut diagnostics.surface,
            candidate_context,
            step.surface.is_some(),
            step.branch_id.is_some(),
        );
        classify_enrichment(
            &mut diagnostics.protocol,
            candidate_context,
            step.protocol.is_some(),
            step.node_id.is_some() && protocol_any_present,
        );
    }

    diagnostics
}

fn classify_enrichment(
    bucket: &mut DiagnosticsBucket,
    applicable: bool,
    present: bool,
    expected_context: bool,
) {
    if !applicable {
        bucket.not_applicable += 1;
    } else if present {
    } else if expected_context {
        bucket.expected_absent += 1;
    } else {
        bucket.unavailable_upstream += 1;
    }
}

fn diagnostics_label(ui: &mut egui::Ui, name: &str, bucket: DiagnosticsBucket) {
    ui.label(format!(
        "{} n/a {} | absent {} | unavailable {}",
        name, bucket.not_applicable, bucket.expected_absent, bucket.unavailable_upstream
    ));
}
