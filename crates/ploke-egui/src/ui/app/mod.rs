//! eframe application shell for the operator graph UI.

pub(crate) mod layout;
mod shell;

use eframe::egui;
use ploke_tree::Graph;

#[cfg(not(target_arch = "wasm32"))]
use crate::diagnostics::{
    GraphIdentity, RunSnapshot, SnapshotObservation, SnapshotSink, artifact_component_breakdown,
    write_manual_snapshot,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::import::graph_from_run_root;
#[cfg(not(target_arch = "wasm32"))]
use crate::run_picker::RunPicker;
use crate::ui::inspector::SelectionInspector;
use crate::ui::view::{ArtifactTreeFilters, GraphView, GraphViewDiagnostics, GraphViewMode};

#[derive(Debug, Default)]
pub struct OperatorApp {
    graph: Graph,
    view: GraphView,
    #[cfg(not(target_arch = "wasm32"))]
    run_picker: RunPicker,
    #[cfg(not(target_arch = "wasm32"))]
    run_error: Option<String>,
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
            run_picker: RunPicker::from_default_root(),
            #[cfg(not(target_arch = "wasm32"))]
            run_error: None,
            #[cfg(not(target_arch = "wasm32"))]
            diagnostics_sink: None,
            #[cfg(not(target_arch = "wasm32"))]
            close_after_snapshot: false,
            #[cfg(not(target_arch = "wasm32"))]
            diagnostics_error: None,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_with_run_picker(graph: Graph, run_picker: RunPicker) -> Self {
        Self {
            graph,
            view: GraphView::default(),
            run_picker,
            run_error: None,
            diagnostics_sink: None,
            close_after_snapshot: false,
            diagnostics_error: None,
        }
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    pub fn with_mode(mut self, mode: GraphViewMode) -> Self {
        self.view.set_mode(mode);
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_snapshot_sink(mut self, sink: SnapshotSink) -> Self {
        self.diagnostics_sink = Some(sink);
        self.close_after_snapshot = true;
        self
    }

    fn current_run_name(&self) -> Option<String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            return self
                .run_picker
                .selected_run()
                .map(|run| run.name)
                .filter(|name| !name.is_empty());
        }

        #[cfg(target_arch = "wasm32")]
        {
            None
        }
    }
}

impl eframe::App for OperatorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let top_graph_has_content = graph_has_content(&self.graph);

        egui::Panel::top("top_strip")
            .default_size(layout::TOP_STRIP_HEIGHT)
            .show_inside(ui, |ui| {
                shell::render_top_strip(
                    ui,
                    self.view.mode(),
                    self.current_run_name().as_deref(),
                    top_graph_has_content,
                );
            });

        egui::Panel::left("run_navigation")
            .default_size(layout::LEFT_SIDEBAR_WIDTH)
            .max_size(layout::LEFT_SIDEBAR_MAX_WIDTH)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        #[cfg(not(target_arch = "wasm32"))]
                        self.render_run_picker(ui);
                        render_mode_picker(ui, &mut self.view);
                        render_quick_filters(ui, &mut self.view);
                        render_graph_facts(ui, &self.graph);
                        if let Some(diagnostics) = self.view.diagnostics() {
                            ui.separator();
                            #[cfg(not(target_arch = "wasm32"))]
                            if let Some(run) = self.run_snapshot() {
                                ui.label(format!("Run: {}", run.name));
                            }
                            render_diagnostics(ui, &diagnostics);
                            #[cfg(not(target_arch = "wasm32"))]
                            self.render_diagnostics_export(ui, diagnostics);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some(error) = &self.diagnostics_error {
                            ui.separator();
                            ui.label(error.as_str());
                        }
                    });
            });

        let graph_has_content = graph_has_content(&self.graph);
        let selected_detail = self.view.selected_node_detail(&self.graph);

        egui::Panel::right("selection_inspector")
            .default_size(layout::RIGHT_INSPECTOR_WIDTH)
            .max_size(layout::RIGHT_INSPECTOR_MAX_WIDTH)
            .show_inside(ui, |ui| {
                let selected_inspector = selected_detail
                    .as_ref()
                    .map(|selection| SelectionInspector::from_graph(&self.graph, selection));
                shell::render_right_inspector(
                    ui,
                    selected_detail.as_ref(),
                    selected_inspector.as_ref(),
                );
            });

        egui::Panel::bottom("timeline")
            .default_size(layout::BOTTOM_TIMELINE_HEIGHT)
            .show_inside(ui, |ui| {
                shell::render_bottom_timeline(
                    ui,
                    self.view.diagnostics().as_ref(),
                    selected_detail.as_ref(),
                );
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if graph_has_content {
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
    fn render_run_picker(&mut self, ui: &mut egui::Ui) {
        if let Some(path) = self.run_picker.show(ui) {
            match graph_from_run_root(&path) {
                Ok(graph) => {
                    let mode = self.view.mode();
                    self.graph = graph;
                    self.view = GraphView::default();
                    self.view.set_mode(mode);
                    self.run_error = None;
                }
                Err(error) => {
                    self.run_error = Some(format!("Run import failed: {error}"));
                }
            }
        }

        if let Some(error) = &self.run_error {
            ui.label(error);
        }
    }

    fn emit_diagnostics(&mut self, ctx: &egui::Context) {
        let Some(diagnostics) = self.view.diagnostics() else {
            return;
        };
        let run = self.run_snapshot();
        let graph_identity = GraphIdentity::from_graph(&self.graph, &diagnostics);
        let selected = self.view.selected_node_detail(&self.graph);
        let selected_inspector = selected.as_ref().map(|selection| {
            SelectionInspector::from_graph(&self.graph, selection).snapshot(selection)
        });
        let observation = SnapshotObservation::new(diagnostics)
            .with_graph_has_content(graph_has_content(&self.graph))
            .with_hide_unconsidered_children(
                self.view.artifact_tree_filters().hide_unconsidered_children,
            )
            .with_run_error(self.run_error.clone())
            .with_run(run)
            .with_graph_identity(graph_identity)
            .with_selected(selected.as_ref())
            .with_selected_inspector(selected_inspector)
            .with_artifact_components(artifact_component_breakdown(&self.graph));

        let Some(sink) = &mut self.diagnostics_sink else {
            return;
        };
        match sink.observe(observation) {
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

    fn render_diagnostics_export(&mut self, ui: &mut egui::Ui, diagnostics: GraphViewDiagnostics) {
        if ui.button("Write diagnostics").clicked() {
            let run = self.run_snapshot();
            let graph_identity = GraphIdentity::from_graph(&self.graph, &diagnostics);
            let selected = self.view.selected_node_detail(&self.graph);
            let selected_inspector = selected.as_ref().map(|selection| {
                SelectionInspector::from_graph(&self.graph, selection).snapshot(selection)
            });
            let observation = SnapshotObservation::new(diagnostics)
                .with_graph_has_content(graph_has_content(&self.graph))
                .with_hide_unconsidered_children(
                    self.view.artifact_tree_filters().hide_unconsidered_children,
                )
                .with_run_error(self.run_error.clone())
                .with_run(run)
                .with_graph_identity(graph_identity)
                .with_selected(selected.as_ref())
                .with_selected_inspector(selected_inspector)
                .with_artifact_components(artifact_component_breakdown(&self.graph));
            match write_manual_snapshot(observation) {
                Ok(path) => {
                    self.diagnostics_error =
                        Some(format!("Diagnostics written: {}", path.display()));
                }
                Err(error) => {
                    self.diagnostics_error = Some(format!("Diagnostics write failed: {error}"));
                }
            }
        }
    }

    fn run_snapshot(&self) -> Option<RunSnapshot> {
        self.run_picker.selected_run().map(|run| RunSnapshot {
            name: run.name,
            path: run.path.display().to_string(),
        })
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
            .selectable_label(view.mode() == GraphViewMode::Lineage, "Lineage")
            .clicked()
        {
            view.set_mode(GraphViewMode::Lineage);
        }
        if ui
            .selectable_label(view.mode() == GraphViewMode::ArtifactAndLineage, "Both")
            .clicked()
        {
            view.set_mode(GraphViewMode::ArtifactAndLineage);
        }
        if ui
            .selectable_label(view.mode() == GraphViewMode::Empty, "None")
            .clicked()
        {
            view.set_mode(GraphViewMode::Empty);
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

fn render_quick_filters(ui: &mut egui::Ui, view: &mut GraphView) {
    let ArtifactTreeFilters {
        hide_unconsidered_children,
    } = view.artifact_tree_filters();
    let mut hide = hide_unconsidered_children;
    if ui
        .checkbox(&mut hide, "Hide children excluded from selection")
        .changed()
    {
        view.set_hide_unconsidered_children(hide);
    }
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

fn render_diagnostics(ui: &mut egui::Ui, diagnostics: &GraphViewDiagnostics) {
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
