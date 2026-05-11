//! eframe application shell for the operator graph UI.

use eframe::egui;

#[cfg(not(target_arch = "wasm32"))]
use crate::diagnostics::SnapshotSink;
use crate::graph::Graph;
use crate::ui::view::{GraphView, GraphViewDiagnostics};

#[derive(Debug, Default)]
pub struct OperatorApp {
    graph: Graph,
    view: GraphView,
    summary: GraphSummary,
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
            summary: GraphSummary::default(),
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
        self.summary.refresh(&self.graph);
        self.summary.refresh_diagnostics(self.view.diagnostics());

        egui::Panel::left("run_navigation").show_inside(ui, |ui| {
            ui.heading("Run");
            ui.label(self.summary.candidates.as_str());
            ui.label(self.summary.candidate_edges.as_str());
            ui.label(self.summary.artifacts.as_str());
            ui.label(self.summary.artifact_edges.as_str());
            ui.label(self.summary.runtimes.as_str());
            ui.label(self.summary.operations.as_str());
            if self.summary.diagnostics.is_some() {
                ui.separator();
                ui.label(self.summary.view_nodes.as_str());
                ui.label(self.summary.graph_size.as_str());
                ui.label(self.summary.aspect.as_str());
                ui.label(self.summary.fit_fill.as_str());
                ui.label(self.summary.edge_labels.as_str());
                ui.label(self.summary.readability.as_str());
            }
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(error) = &self.diagnostics_error {
                ui.separator();
                ui.label(error.as_str());
            }
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.view.show(ui, &self.graph);
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

#[derive(Debug, Default)]
struct GraphSummary {
    counts: Option<GraphCounts>,
    candidates: String,
    candidate_edges: String,
    artifacts: String,
    artifact_edges: String,
    runtimes: String,
    operations: String,
    diagnostics: Option<GraphViewDiagnostics>,
    view_nodes: String,
    graph_size: String,
    aspect: String,
    fit_fill: String,
    edge_labels: String,
    readability: String,
}

impl GraphSummary {
    fn refresh(&mut self, graph: &Graph) {
        let counts = GraphCounts::from(graph);
        if self.counts == Some(counts) {
            return;
        }

        self.counts = Some(counts);
        self.candidates = format!("Candidates: {}", counts.candidates);
        self.candidate_edges = format!("Candidate edges: {}", counts.candidate_edges);
        self.artifacts = format!("Artifacts: {}", counts.artifacts);
        self.artifact_edges = format!("Artifact edges: {}", counts.artifact_edges);
        self.runtimes = format!("Runtimes: {}", counts.runtimes);
        self.operations = format!("Operations: {}", counts.operations);
    }

    fn refresh_diagnostics(&mut self, diagnostics: Option<GraphViewDiagnostics>) {
        if self.diagnostics == diagnostics {
            return;
        }

        self.diagnostics = diagnostics;
        let Some(diagnostics) = diagnostics else {
            self.view_nodes.clear();
            self.graph_size.clear();
            self.aspect.clear();
            self.fit_fill.clear();
            self.edge_labels.clear();
            self.readability.clear();
            return;
        };

        self.view_nodes = format!("View nodes: {}", diagnostics.node_count);
        self.graph_size = format!(
            "Graph: {:.0} x {:.0}",
            diagnostics.graph_size.x, diagnostics.graph_size.y
        );
        self.aspect = format!("Aspect: {:.2}", diagnostics.aspect_ratio);
        self.fit_fill = format!(
            "Fit fill: {:.0}% x {:.0}%",
            diagnostics.fitted_fill.x * 100.0,
            diagnostics.fitted_fill.y * 100.0
        );
        self.edge_labels = format!(
            "Edge labels: {}, label collisions: {}, edge intersections: {}, edge collisions: {}",
            diagnostics.edge_labels.label_count,
            diagnostics.edge_labels.collision_count,
            diagnostics.edge_labels.edge_intersection_count,
            diagnostics.edge_labels.edge_collision_count
        );
        self.readability = format!(
            "Crossings: {}, candidate/history: {}, long edges: {}, backtracking: {}, selected crossings: {}",
            diagnostics.readability.edge_edge_crossings,
            diagnostics
                .readability
                .edge_crossings_by_kind
                .candidate_history,
            diagnostics.readability.long_edge_count,
            diagnostics.readability.backtracking_edge_count,
            diagnostics.readability.selected_path_crossings
        );
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct GraphCounts {
    candidates: usize,
    candidate_edges: usize,
    artifacts: usize,
    artifact_edges: usize,
    runtimes: usize,
    operations: usize,
}

impl From<&Graph> for GraphCounts {
    fn from(graph: &Graph) -> Self {
        Self {
            candidates: graph.candidate_count(),
            candidate_edges: graph.candidate_edge_count(),
            artifacts: graph.artifact_count(),
            artifact_edges: graph.artifact_edge_count(),
            runtimes: graph.runtime_count(),
            operations: graph.operation_count(),
        }
    }
}
