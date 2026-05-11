//! eframe application shell for the operator graph UI.

use eframe::egui;

use crate::graph::Graph;
use crate::ui::view::GraphView;

#[derive(Debug, Default)]
pub struct OperatorApp {
    graph: Graph,
    view: GraphView,
    summary: GraphSummary,
}

impl OperatorApp {
    pub fn new(graph: Graph) -> Self {
        Self {
            graph,
            view: GraphView::default(),
            summary: GraphSummary::default(),
        }
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }
}

impl eframe::App for OperatorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.summary.refresh(&self.graph);

        egui::Panel::left("run_navigation").show_inside(ui, |ui| {
            ui.heading("Run");
            ui.label(self.summary.candidates.as_str());
            ui.label(self.summary.candidate_edges.as_str());
            ui.label(self.summary.artifacts.as_str());
            ui.label(self.summary.artifact_edges.as_str());
            ui.label(self.summary.runtimes.as_str());
            ui.label(self.summary.operations.as_str());
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.view.show(ui, &self.graph);
        });
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
