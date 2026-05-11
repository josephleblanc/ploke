use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::{fs, path::PathBuf};

use eframe::egui::{self, Color32, RichText, ScrollArea};
use eframe::{App, Frame};
use ploke_tree::browser::{
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
    selected_group: Option<TreeGroupKey>,
    tree_open_override: Option<bool>,
    view_mode: ViewMode,
    status_message: String,
}

impl PlokeTreeApp {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(default_path: &str) -> Self {
        let mut app = Self {
            path_input: default_path.to_owned(),
            model: None,
            selected_index: None,
            selected_group: None,
            tree_open_override: None,
            view_mode: ViewMode::Timeline,
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
            selected_group: None,
            tree_open_override: None,
            view_mode: ViewMode::Timeline,
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
                self.selected_group = None;
                self.status_message = format!("Failed to read {}: {}", path.display(), err);
            }
        }
    }

    fn load_model_from_json(&mut self, json: &str, source: &str) {
        match serde_json::from_str::<PlaybackBrowserModel>(json) {
            Ok(model) => {
                let step_count = model.steps.len();
                self.selected_index = if step_count > 0 { Some(0) } else { None };
                self.selected_group = self
                    .selected_index
                    .and_then(|index| model.steps.get(index).map(tree_group_key));
                self.status_message = format!("Loaded {} steps from {}", step_count, source);
                self.model = Some(model);
            }
            Err(err) => {
                self.model = None;
                self.selected_index = None;
                self.selected_group = None;
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
                        self.selected_group = None;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Timeline,
    Tree,
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

            ui.horizontal_wrapped(|ui| {
                ui.label("View");
                ui.selectable_value(&mut self.view_mode, ViewMode::Timeline, "Timeline");
                ui.selectable_value(&mut self.view_mode, ViewMode::Tree, "Tree");
                if self.view_mode == ViewMode::Tree {
                    ui.separator();
                    ui.label("Projection from browser-model evidence");
                }
            });

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

        let tree_groups = self.model.as_ref().map(build_tree_groups);

        egui::SidePanel::right("detail_panel")
            .resizable(true)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.heading("Detail");
                ui.separator();
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some(step) = self.selected_step() {
                            let selected_group = if self.view_mode == ViewMode::Tree {
                                self.selected_group.as_ref().and_then(|key| {
                                    tree_groups.as_ref().and_then(|groups| {
                                        groups.iter().find(|group| &group.key == key)
                                    })
                                })
                            } else {
                                None
                            };
                            detail_panel(ui, step, selected_group);
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
            ui.heading(match self.view_mode {
                ViewMode::Timeline => "Timeline",
                ViewMode::Tree => "Tree",
            });
            ui.separator();

            match self.view_mode {
                ViewMode::Timeline => {
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
                                        self.selected_group = Some(tree_group_key(step));
                                    }

                                    step_summary_row(ui, step);
                                    ui.separator();
                                }
                            });
                    } else {
                        ui.label("Load a JSON model to view the timeline");
                    }
                }
                ViewMode::Tree => {
                    if let (Some(model), Some(groups)) = (self.model.as_ref(), tree_groups.as_ref())
                    {
                        ui.horizontal_wrapped(|ui| {
                            badge(
                                ui,
                                format!("groups {}", groups.len()),
                                Color32::from_rgb(228, 228, 228),
                            );
                            if ui.button("Expand all").clicked() {
                                self.tree_open_override = Some(true);
                            }
                            if ui.button("Collapse all").clicked() {
                                self.tree_open_override = Some(false);
                            }
                            ui.label(
                                "Grouped by explicit node, candidate, branch, and block/order fields.",
                            );
                        });
                        ui.separator();
                        let tree_open_override = self.tree_open_override.take();
                        ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for group in groups {
                                    let group_selected =
                                        self.selected_group.as_ref() == Some(&group.key);
                                    let group_id =
                                        ui.make_persistent_id(("tree_group", group.key.ui_id()));
                                    let mut state =
                                        egui::collapsing_header::CollapsingState::load_with_default_open(
                                            ui.ctx(),
                                            group_id,
                                            true,
                                        );
                                    if let Some(open) = tree_open_override {
                                        state.set_open(open);
                                    }

                                    let (_, header_response, _) = state
                                        .show_header(ui, |ui| {
                                            ui.horizontal_wrapped(|ui| {
                                                let response = ui.selectable_label(
                                                    group_selected,
                                                    RichText::new(group_title(group)).strong(),
                                                );
                                                badge(
                                                    ui,
                                                    format!("steps {}", group.step_indices.len()),
                                                    Color32::from_rgb(228, 228, 228),
                                                );
                                                badge(
                                                    ui,
                                                    format!("blocks {}", group.block_range_label()),
                                                    Color32::from_rgb(232, 224, 255),
                                                );
                                                if group.evaluation_count > 0 {
                                                    badge(
                                                        ui,
                                                        format!(
                                                            "eval {} keep {} reject {}",
                                                            group.evaluation_count,
                                                            group.kept_count,
                                                            group.rejected_count
                                                        ),
                                                        Color32::from_rgb(218, 243, 224),
                                                    );
                                                }
                                                if group.protocol_count > 0 {
                                                    badge(
                                                        ui,
                                                        format!(
                                                            "protocol {}",
                                                            group.protocol_item_count
                                                        ),
                                                        Color32::from_rgb(218, 232, 246),
                                                    );
                                                }
                                                response.clicked()
                                            })
                                            .inner
                                        })
                                        .body(|ui| {
                                            ui.horizontal_wrapped(|ui| {
                                                if !group.candidate_ids.is_empty() {
                                                    ui.monospace(format!(
                                                        "candidates: {}",
                                                        list_preview(&group.candidate_ids)
                                                    ));
                                                }
                                                if !group.branch_ids.is_empty() {
                                                    ui.monospace(format!(
                                                        "branches: {}",
                                                        list_preview(&group.branch_ids)
                                                    ));
                                                }
                                                if !group.target_relpaths.is_empty() {
                                                    ui.monospace(format!(
                                                        "targets: {}",
                                                        list_preview(&group.target_relpaths)
                                                    ));
                                                }
                                            });

                                            for index in &group.step_indices {
                                                if let Some(step) = model.steps.get(*index) {
                                                    let is_selected =
                                                        self.selected_index == Some(*index);
                                                    let response = ui.selectable_label(
                                                        is_selected,
                                                        tree_step_row_text(step),
                                                    );
                                                    if response.clicked() {
                                                        self.selected_index = Some(*index);
                                                        self.selected_group =
                                                            Some(group.key.clone());
                                                    }
                                                    step_summary_row(ui, step);
                                                }
                                            }
                                        });

                                    if header_response.inner {
                                        self.selected_index = Some(group.first_index);
                                        self.selected_group = Some(group.key.clone());
                                    }

                                    ui.separator();
                                }
                            });
                    } else {
                        ui.label("Load a JSON model to view the tree projection");
                    }
                }
            }
        });
    }
}

fn step_summary_row(ui: &mut egui::Ui, step: &PlaybackBrowserStep) {
    ui.horizontal_wrapped(|ui| {
        badge(
            ui,
            evidence_label(step.evidence),
            evidence_color(step.evidence),
        );
        if let Some(disposition) = step.evaluation.as_ref().map(|eval| eval.disposition) {
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
                format!("protocol {}", protocol_item_count(protocol)),
                Color32::from_rgb(218, 232, 246),
            );
        }
    });
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum TreeGroupKey {
    Node(String),
    Candidate(String),
    Branch(String),
    BlockCandidate {
        block_height: u64,
        candidate_index: usize,
    },
    Block {
        block_height: u64,
    },
}

struct TreeGroup {
    key: TreeGroupKey,
    step_indices: Vec<usize>,
    first_index: usize,
    first_block: u64,
    last_block: u64,
    evaluation_count: usize,
    kept_count: usize,
    rejected_count: usize,
    surface_count: usize,
    protocol_count: usize,
    protocol_item_count: usize,
    warning_count: usize,
    candidate_ids: Vec<String>,
    branch_ids: Vec<String>,
    target_relpaths: Vec<String>,
}

impl TreeGroup {
    fn new(key: TreeGroupKey, first_index: usize, block_height: u64) -> Self {
        Self {
            key,
            step_indices: Vec::new(),
            first_index,
            first_block: block_height,
            last_block: block_height,
            evaluation_count: 0,
            kept_count: 0,
            rejected_count: 0,
            surface_count: 0,
            protocol_count: 0,
            protocol_item_count: 0,
            warning_count: 0,
            candidate_ids: Vec::new(),
            branch_ids: Vec::new(),
            target_relpaths: Vec::new(),
        }
    }

    fn add_step(&mut self, index: usize, step: &PlaybackBrowserStep) {
        self.step_indices.push(index);
        self.first_index = self.first_index.min(index);
        self.first_block = self.first_block.min(step.block_height);
        self.last_block = self.last_block.max(step.block_height);
        self.warning_count += step.warning_count;

        push_unique(&mut self.candidate_ids, step.candidate_id.as_deref());
        push_unique(&mut self.branch_ids, step.branch_id.as_deref());
        if let Some(surface) = step.surface.as_ref() {
            self.surface_count += 1;
            push_unique(
                &mut self.target_relpaths,
                Some(surface.target_relpath.as_str()),
            );
        }

        if let Some(evaluation) = step.evaluation.as_ref() {
            self.evaluation_count += 1;
            match disposition_label(evaluation.disposition).as_str() {
                "keep" => self.kept_count += 1,
                "reject" => self.rejected_count += 1,
                _ => {}
            }
        }

        if let Some(protocol) = step.protocol.as_ref() {
            self.protocol_count += 1;
            self.protocol_item_count += protocol_item_count(protocol);
        }
    }

    fn block_range_label(&self) -> String {
        if self.first_block == self.last_block {
            format!("h{}", self.first_block)
        } else {
            format!("h{}-h{}", self.first_block, self.last_block)
        }
    }
}

impl TreeGroupKey {
    fn ui_id(&self) -> String {
        match self {
            TreeGroupKey::Node(node_id) => format!("node:{node_id}"),
            TreeGroupKey::Candidate(candidate_id) => format!("candidate:{candidate_id}"),
            TreeGroupKey::Branch(branch_id) => format!("branch:{branch_id}"),
            TreeGroupKey::BlockCandidate {
                block_height,
                candidate_index,
            } => format!("block-candidate:{block_height}:{candidate_index}"),
            TreeGroupKey::Block { block_height } => format!("block:{block_height}"),
        }
    }

    fn evidence_label(&self) -> &'static str {
        match self {
            TreeGroupKey::Node(_) => "node_id",
            TreeGroupKey::Candidate(_) => "candidate_id",
            TreeGroupKey::Branch(_) => "branch_id",
            TreeGroupKey::BlockCandidate { .. } => "block_height + candidate_index",
            TreeGroupKey::Block { .. } => "block_height",
        }
    }
}

fn build_tree_groups(model: &PlaybackBrowserModel) -> Vec<TreeGroup> {
    let mut groups = BTreeMap::<TreeGroupKey, TreeGroup>::new();
    for (index, step) in model.steps.iter().enumerate() {
        let key = tree_group_key(step);
        groups
            .entry(key.clone())
            .or_insert_with(|| TreeGroup::new(key, index, step.block_height))
            .add_step(index, step);
    }
    groups.into_values().collect()
}

fn tree_group_key(step: &PlaybackBrowserStep) -> TreeGroupKey {
    if let Some(node_id) = step.node_id.as_ref() {
        TreeGroupKey::Node(node_id.clone())
    } else if let Some(candidate_id) = step.candidate_id.as_ref() {
        TreeGroupKey::Candidate(candidate_id.clone())
    } else if let Some(branch_id) = step.branch_id.as_ref() {
        TreeGroupKey::Branch(branch_id.clone())
    } else if let Some(candidate_index) =
        step.order.as_ref().and_then(|order| order.candidate_index)
    {
        TreeGroupKey::BlockCandidate {
            block_height: step.block_height,
            candidate_index,
        }
    } else {
        TreeGroupKey::Block {
            block_height: step.block_height,
        }
    }
}

fn group_title(group: &TreeGroup) -> String {
    match &group.key {
        TreeGroupKey::Node(node_id) => format!("Node {}", node_id),
        TreeGroupKey::Candidate(candidate_id) => format!("Candidate {}", candidate_id),
        TreeGroupKey::Branch(branch_id) => format!("Branch {}", branch_id),
        TreeGroupKey::BlockCandidate {
            block_height,
            candidate_index,
        } => format!("Block h{} candidate slot {}", block_height, candidate_index),
        TreeGroupKey::Block { block_height } => format!("Block h{}", block_height),
    }
}

fn tree_step_row_text(step: &PlaybackBrowserStep) -> String {
    let kind = step
        .fine_kind
        .map(fine_kind_label)
        .unwrap_or("coarse".to_owned());
    let candidate = step
        .candidate_id
        .as_deref()
        .or(step.selected_candidate.as_deref())
        .unwrap_or("<no candidate>");
    format!("#{}  {}  {}", step.index, kind, candidate)
}

fn list_preview(values: &[String]) -> String {
    let mut preview = values
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if values.len() > 3 {
        preview.push_str(&format!(" +{}", values.len() - 3));
    }
    preview
}

fn push_unique(values: &mut Vec<String>, value: Option<&str>) {
    if let Some(value) = value {
        if !values.iter().any(|existing| existing == value) {
            values.push(value.to_owned());
        }
    }
}

fn protocol_item_count(protocol: &ploke_tree::browser::ProtocolSnapshot) -> usize {
    protocol.intent_segmentation_count
        + protocol.tool_call_review_count
        + protocol.segment_review_count
        + protocol.issue_detection_count
        + protocol.synthesis_count
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

fn detail_panel(ui: &mut egui::Ui, step: &PlaybackBrowserStep, group: Option<&TreeGroup>) {
    if let Some(group) = group {
        ui.label(RichText::new(group_title(group)).strong());
        detail_kv(ui, "grouping", group.key.evidence_label());
        detail_kv(ui, "steps", group.step_indices.len().to_string());
        detail_kv(ui, "blocks", group.block_range_label());
        detail_kv(
            ui,
            "evaluations",
            format!(
                "{} (keep {}, reject {})",
                group.evaluation_count, group.kept_count, group.rejected_count
            ),
        );
        detail_kv(ui, "surfaces", group.surface_count.to_string());
        detail_kv(ui, "protocol_items", group.protocol_item_count.to_string());
        detail_kv(ui, "warnings", group.warning_count.to_string());
        ui.separator();
        ui.label(RichText::new("Selected step").strong());
    }

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
