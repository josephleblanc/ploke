//! eframe application shell for the operator graph UI.

use eframe::egui;
use ploke_tree::Graph;

#[cfg(not(target_arch = "wasm32"))]
use crate::diagnostics::SnapshotSink;
use crate::ui::view::{GraphView, GraphViewDiagnostics, GraphViewMode};

#[derive(Debug, Default)]
pub struct OperatorApp {
    graph: Graph,
    view: GraphView,
    #[cfg(not(target_arch = "wasm32"))]
    diagnostics_sink: Option<SnapshotSink>,
    #[cfg(not(target_arch = "wasm32"))]
    close_after_snapshot: bool,
    #[cfg(not(target_arch = "wasm32"))]
    diagnostics_error: Option<String>,
}

impl OperatorApp {
    pub fn new(graph: Graph) -> Self {
        Self {
            graph,
            view: GraphView::default(),
            #[cfg(not(target_arch = "wasm32"))]
            diagnostics_sink: None,
            #[cfg(not(target_arch = "wasm32"))]
            close_after_snapshot: false,
            #[cfg(not(target_arch = "wasm32"))]
            diagnostics_error: None,
        }
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_snapshot_sink(mut self, sink: SnapshotSink) -> Self {
        self.diagnostics_sink = Some(sink);
        self.close_after_snapshot = true;
        self
    }
}

impl eframe::App for OperatorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("run_navigation").show_inside(ui, |ui| {
            ui.heading("Run");
            render_mode_picker(ui, &mut self.view);
            render_graph_facts(ui, &self.graph);
            if let Some(selection) = self.view.selected_node_detail() {
                ui.separator();
                ui.heading("Selected");
                ui.label(format!("{} {}", selection.kind, selection.label));
                ui.monospace(selection.detail);
            }
            if let Some(diagnostics) = self.view.diagnostics() {
                ui.separator();
                render_diagnostics(ui, diagnostics);
            }
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(error) = &self.diagnostics_error {
                ui.separator();
                ui.label(error.as_str());
            }
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if graph_has_content(&self.graph) {
                self.view.show(ui, &self.graph);
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        "No graph records loaded. Pass --run-root with a Prototype 1 record root.",
                    );
                });
            }
        });

        #[cfg(not(target_arch = "wasm32"))]
        self.emit_diagnostics(ui.ctx());
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl OperatorApp {
    fn emit_diagnostics(&mut self, ctx: &egui::Context) {
        let Some(sink) = &mut self.diagnostics_sink else {
            return;
        };
        let Some(diagnostics) = self.view.diagnostics() else {
            return;
        };

        match sink.observe(diagnostics) {
            Ok(true) if self.close_after_snapshot => {
                self.diagnostics_sink = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Ok(_) => {}
            Err(error) => {
                self.diagnostics_error = Some(format!("Diagnostics write failed: {error}"));
                self.diagnostics_sink = None;
            }
        }
    }
}

fn render_mode_picker(ui: &mut egui::Ui, view: &mut GraphView) {
    ui.horizontal(|ui| {
        if ui
            .selectable_label(view.mode() == GraphViewMode::ArtifactTree, "Artifact tree")
            .clicked()
        {
            view.set_mode(GraphViewMode::ArtifactTree);
        }
        if ui
            .selectable_label(view.mode() == GraphViewMode::FullDebug, "Full debug")
            .clicked()
        {
            view.set_mode(GraphViewMode::FullDebug);
        }
    });
}

fn render_graph_facts(ui: &mut egui::Ui, graph: &Graph) {
    ui.label(format!("History blocks: {}", graph.history.blocks.len()));
    ui.label(format!("Candidates: {}", graph.candidates.candidates.len()));
    ui.label(format!("Artifacts: {}", graph.artifacts.artifacts.len()));
    ui.label(format!("Selections: {}", graph.selections.selections.len()));
    ui.label(format!("Runtimes: {}", graph.runtimes.runtimes.len()));
    ui.label(format!("Operations: {}", graph.operations.operations.len()));
    ui.label(format!("Evidence: {}", graph.evidence.attachments.len()));
}

fn graph_has_content(graph: &Graph) -> bool {
    graph.history.blocks.len()
        + graph.candidates.candidates.len()
        + graph.artifacts.artifacts.len()
        + graph.selections.selections.len()
        + graph.runtimes.runtimes.len()
        + graph.operations.operations.len()
        + graph.evidence.attachments.len()
        > 0
}

fn render_diagnostics(ui: &mut egui::Ui, diagnostics: GraphViewDiagnostics) {
    ui.label(format!("Mode: {}", diagnostics.mode.as_str()));
    ui.label(format!("View nodes: {}", diagnostics.node_count));
    ui.label(format!("View edges: {}", diagnostics.edge_count));
    ui.label(format!(
        "Components before anchors: {}",
        diagnostics.connectivity.component_count_before_anchoring
    ));
    ui.label(format!(
        "Hidden records: {}, hidden edges: {}, hidden evidence: {}, hidden operations: {}, unattached components: {}",
        diagnostics.connectivity.hidden_record_count,
        diagnostics.connectivity.hidden_edge_count,
        diagnostics.connectivity.hidden_evidence_count,
        diagnostics.connectivity.hidden_operation_count,
        diagnostics.connectivity.hidden_unattached_component_count
    ));
    ui.label(format!(
        "Synthetic anchors visible: {}",
        diagnostics.connectivity.synthetic_anchors_visible
    ));
    if diagnostics.mode == GraphViewMode::FullDebug {
        for root in diagnostics
            .connectivity
            .component_roots_before_anchoring
            .iter()
        {
            ui.label(format!("Component root: {} {}", root.kind, root.label));
        }
    }
    ui.label(format!(
        "Graph: {:.0} x {:.0}",
        diagnostics.graph_size.x, diagnostics.graph_size.y
    ));
    ui.label(format!("Aspect: {:.2}", diagnostics.aspect_ratio));
    ui.label(format!(
        "Fit fill: {:.0}% x {:.0}%",
        diagnostics.fitted_fill.x * 100.0,
        diagnostics.fitted_fill.y * 100.0
    ));
    ui.label(format!(
        "Edge labels: {}, label collisions: {}, edge intersections: {}, edge collisions: {}",
        diagnostics.edge_labels.label_count,
        diagnostics.edge_labels.collision_count,
        diagnostics.edge_labels.edge_intersection_count,
        diagnostics.edge_labels.edge_collision_count
    ));
    ui.label(format!(
        "Candidate clutter: {}",
        diagnostics
            .readability
            .edge_crossings_by_kind
            .candidate_candidate
    ));
    ui.label(format!(
        "Crossings: {}, artifact/artifact: {}, mixed: {}, long edges: {}, backtracking: {}, selected crossings: {}",
        diagnostics.readability.edge_edge_crossings,
        diagnostics
            .readability
            .edge_crossings_by_kind
            .artifact_artifact,
        diagnostics.readability.edge_crossings_by_kind.mixed,
        diagnostics.readability.long_edge_count,
        diagnostics.readability.backtracking_edge_count,
        diagnostics.readability.selected_path_crossings
    ));
}
