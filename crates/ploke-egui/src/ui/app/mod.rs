//! eframe application shell for the operator graph UI.

pub(crate) mod layout;
mod shell;

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "dev",
    feature = "native-benchmark"
))]
use std::time::Instant;

use eframe::egui;
use ploke_tree::Graph;

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "dev",
    feature = "native-benchmark"
))]
use crate::benchmark::{
    BenchmarkAction, BenchmarkActionReport, BenchmarkController, BenchmarkWriteResult,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::diagnostics::{
    GraphIdentity, RunSnapshot, SnapshotObservation, SnapshotSink, artifact_component_breakdown,
    write_manual_snapshot,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::import::graph_from_run_root;
#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
use crate::perf::{PuffinCapture, PuffinCaptureStatus};
#[cfg(not(target_arch = "wasm32"))]
use crate::run_picker::RunPicker;
use crate::ui::diff::PatchDiffCache;
#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "dev",
    feature = "native-benchmark"
))]
use crate::ui::inspector::default_selections;
use crate::ui::inspector::{GraphRevision, InspectorCache, SelectionInspector};
use crate::ui::view::{ArtifactTreeFilters, GraphView, GraphViewDiagnostics, GraphViewMode};

#[derive(Debug, Default)]
pub struct OperatorApp {
    graph: Graph,
    graph_revision: GraphRevision,
    view: GraphView,
    inspector_cache: InspectorCache,
    patch_diff_cache: PatchDiffCache,
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
    #[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
    puffin_capture: Option<PuffinCapture>,
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    benchmark: Option<BenchmarkController>,
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    benchmark_patch_debug_open: bool,
}

impl OperatorApp {
    pub fn new(graph: Graph) -> Self {
        Self {
            graph,
            graph_revision: GraphRevision::default(),
            view: GraphView::default(),
            inspector_cache: InspectorCache::default(),
            patch_diff_cache: PatchDiffCache::default(),
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
            #[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
            puffin_capture: None,
            #[cfg(all(
                not(target_arch = "wasm32"),
                feature = "dev",
                feature = "native-benchmark"
            ))]
            benchmark: None,
            #[cfg(all(
                not(target_arch = "wasm32"),
                feature = "dev",
                feature = "native-benchmark"
            ))]
            benchmark_patch_debug_open: false,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_with_run_picker(graph: Graph, run_picker: RunPicker) -> Self {
        Self {
            graph,
            graph_revision: GraphRevision::default(),
            view: GraphView::default(),
            inspector_cache: InspectorCache::default(),
            patch_diff_cache: PatchDiffCache::default(),
            run_picker,
            run_error: None,
            diagnostics_sink: None,
            close_after_snapshot: false,
            diagnostics_error: None,
            #[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
            puffin_capture: None,
            #[cfg(all(
                not(target_arch = "wasm32"),
                feature = "dev",
                feature = "native-benchmark"
            ))]
            benchmark: None,
            #[cfg(all(
                not(target_arch = "wasm32"),
                feature = "dev",
                feature = "native-benchmark"
            ))]
            benchmark_patch_debug_open: false,
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

    #[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
    pub fn with_puffin_capture(mut self, capture: PuffinCapture) -> Self {
        self.puffin_capture = Some(capture);
        self
    }

    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    pub fn with_benchmark(mut self, benchmark: BenchmarkController) -> Self {
        self.benchmark = Some(benchmark);
        self
    }

    fn current_run_name(&self) -> Option<&str> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            return self
                .run_picker
                .selected_run_name()
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
        profiling::scope!("ploke-egui.frame");
        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        self.benchmark_begin_frame();
        let top_graph_has_content = graph_has_content(&self.graph);

        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        let top_strip_start = Instant::now();
        egui::Panel::top("top_strip")
            .default_size(layout::TOP_STRIP_HEIGHT)
            .show_inside(ui, |ui| {
                profiling::scope!("ploke-egui.frame.top-strip");
                shell::render_top_strip(
                    ui,
                    self.view.mode(),
                    self.current_run_name(),
                    top_graph_has_content,
                );
            });
        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        self.record_benchmark_component("top_strip", top_strip_start);

        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        let run_navigation_start = Instant::now();
        egui::Panel::left("run_navigation")
            .default_size(layout::LEFT_SIDEBAR_WIDTH)
            .max_size(layout::LEFT_SIDEBAR_MAX_WIDTH)
            .show_inside(ui, |ui| {
                profiling::scope!("ploke-egui.frame.run-navigation");
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
                            #[cfg(all(
                                not(target_arch = "wasm32"),
                                feature = "dev",
                                feature = "native-benchmark"
                            ))]
                            let diagnostics_start = Instant::now();
                            render_diagnostics(ui, &diagnostics);
                            #[cfg(all(
                                not(target_arch = "wasm32"),
                                feature = "dev",
                                feature = "native-benchmark"
                            ))]
                            self.record_benchmark_component("diagnostics", diagnostics_start);
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
        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        self.record_benchmark_component("run_navigation", run_navigation_start);

        let graph_has_content = graph_has_content(&self.graph);
        let selected_node = self.view.selected_node(&self.graph);
        let selected_reference = selected_node.map(|(reference, _, _)| reference);
        let selected_kind = selected_node.map(|(_, _, kind)| kind);
        let selected_label = selected_node.map(|(_, label, _)| label);
        let selected_sections =
            self.inspector_cache
                .sections(&self.graph, self.graph_revision, selected_reference);
        let selection_synced = selected_reference.is_some();

        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        let inspector_start = Instant::now();
        egui::Panel::right("selection_inspector")
            .default_size(layout::RIGHT_INSPECTOR_WIDTH)
            .max_size(layout::RIGHT_INSPECTOR_MAX_WIDTH)
            .show_inside(ui, |ui| {
                profiling::scope!("ploke-egui.frame.selection-inspector");
                #[cfg(all(
                    not(target_arch = "wasm32"),
                    feature = "dev",
                    feature = "native-benchmark"
                ))]
                let patch_debug_open = self.benchmark_patch_debug_open;
                #[cfg(not(all(
                    not(target_arch = "wasm32"),
                    feature = "dev",
                    feature = "native-benchmark"
                )))]
                let patch_debug_open = false;
                shell::render_right_inspector(
                    ui,
                    &self.graph,
                    selected_kind,
                    selected_label,
                    selected_sections,
                    &mut self.patch_diff_cache,
                    patch_debug_open,
                );
            });
        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        self.record_benchmark_component("selection_inspector", inspector_start);

        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        let timeline_start = Instant::now();
        egui::Panel::bottom("timeline")
            .default_size(layout::BOTTOM_TIMELINE_HEIGHT)
            .show_inside(ui, |ui| {
                profiling::scope!("ploke-egui.frame.timeline");
                shell::render_bottom_timeline(
                    ui,
                    self.view.diagnostics().as_ref(),
                    selection_synced,
                );
            });
        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        self.record_benchmark_component("timeline", timeline_start);

        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        let central_start = Instant::now();
        egui::CentralPanel::default().show_inside(ui, |ui| {
            profiling::scope!("ploke-egui.frame.central");
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
        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        self.record_benchmark_component("central_graph", central_start);

        #[cfg(not(target_arch = "wasm32"))]
        self.emit_diagnostics(ui.ctx());
        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        let capture_start = Instant::now();
        profiling::finish_frame!();
        #[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
        self.emit_puffin_capture(ui.ctx());
        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        {
            self.record_benchmark_component("capture_overhead", capture_start);
            self.benchmark_end_frame(ui.ctx());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl OperatorApp {
    fn render_run_picker(&mut self, ui: &mut egui::Ui) {
        profiling::scope!("ploke-egui.render-run-picker");
        if let Some(path) = self.run_picker.show(ui) {
            match graph_from_run_root(&path) {
                Ok(graph) => {
                    let mode = self.view.mode();
                    self.graph = graph;
                    self.graph_revision = self.graph_revision.next();
                    self.view = GraphView::default();
                    self.view.set_mode(mode);
                    self.inspector_cache = InspectorCache::default();
                    self.patch_diff_cache = PatchDiffCache::default();
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
        profiling::scope!("ploke-egui.emit-diagnostics");
        if self.diagnostics_sink.is_none() {
            return;
        }
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

    #[cfg(feature = "profile-with-puffin")]
    fn emit_puffin_capture(&mut self, ctx: &egui::Context) {
        let Some(capture) = &mut self.puffin_capture else {
            return;
        };
        let diagnostics = self.view.diagnostics();
        match capture.observe_frame(diagnostics.as_ref()) {
            Ok(PuffinCaptureStatus::Complete { close }) => {
                if close {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Ok(PuffinCaptureStatus::Collecting { .. } | PuffinCaptureStatus::AlreadyComplete) => {}
            Err(error) => {
                self.diagnostics_error = Some(format!("Puffin capture failed: {error}"));
                self.puffin_capture = None;
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

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "dev",
    feature = "native-benchmark"
))]
impl OperatorApp {
    fn benchmark_begin_frame(&mut self) {
        let action = self
            .benchmark
            .as_mut()
            .and_then(BenchmarkController::begin_frame);
        if let Some(action) = action {
            let report = self.apply_benchmark_action(action);
            if let Some(benchmark) = &mut self.benchmark {
                benchmark.record_action(report);
            }
        }
    }

    fn record_benchmark_component(&mut self, component: &'static str, start: Instant) {
        if let Some(benchmark) = &mut self.benchmark {
            benchmark.record_component(component, elapsed_ns(start));
        }
    }

    fn benchmark_end_frame(&mut self, ctx: &egui::Context) {
        let Some(benchmark) = &mut self.benchmark else {
            return;
        };
        match benchmark.end_frame() {
            Ok(Some(result)) => {
                print_benchmark_result(&result);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Ok(None) => {
                ctx.request_repaint();
            }
            Err(error) => {
                self.diagnostics_error = Some(format!("Benchmark write failed: {error}"));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn apply_benchmark_action(&mut self, action: BenchmarkAction) -> BenchmarkActionReport {
        let mut report = BenchmarkActionReport::for_action(action);
        self.benchmark_patch_debug_open = false;
        match action {
            BenchmarkAction::None => {}
            BenchmarkAction::SetMode(mode) => {
                self.view.set_mode(mode);
                report.notes.push(format!("mode={}", mode.as_str()));
            }
            BenchmarkAction::ToggleHideUnconsideredChildren => {
                let current = self.view.artifact_tree_filters().hide_unconsidered_children;
                self.view.set_hide_unconsidered_children(!current);
                report
                    .notes
                    .push(format!("hide_unconsidered_children={}", !current));
            }
            BenchmarkAction::SelectArtifact {
                patch_debug_open,
                reset_patch_cache,
            } => {
                self.benchmark_patch_debug_open = patch_debug_open;
                if reset_patch_cache {
                    self.patch_diff_cache = PatchDiffCache::default();
                    report.notes.push("patch_diff_cache_reset=true".to_owned());
                }
                self.apply_benchmark_selection(&mut report);
            }
        }
        report
    }

    fn apply_benchmark_selection(&mut self, report: &mut BenchmarkActionReport) {
        let selections = default_selections(&self.graph);
        let first = selections.first().cloned();
        let with_patch = selections.iter().find(|selection| {
            matches!(
                SelectionInspector::from_graph(&self.graph, selection),
                SelectionInspector::Artifact(artifact) if !artifact.patches.is_empty()
            )
        });

        let (selection, fallback) = if let Some(selection) = with_patch.cloned() {
            (Some(selection), None)
        } else {
            (
                first,
                Some("no_visible_artifact_with_patch_slots".to_owned()),
            )
        };

        let Some(selection) = selection else {
            report.fallback = Some("no_visible_artifact_selection".to_owned());
            return;
        };

        if let Some(fallback) = fallback {
            report.fallback = Some(fallback);
        }
        report.target_label = Some(selection.label.clone());
        report.target_key = Some(selection_key(&selection.reference));
        if !self
            .view
            .select_reference(&self.graph, &selection.reference)
        {
            report
                .notes
                .push("selection_target_not_visible_in_current_mode".to_owned());
        }
    }
}

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "dev",
    feature = "native-benchmark"
))]
fn selection_key(reference: &crate::ui::view::GraphSelectionRef) -> String {
    match reference {
        crate::ui::view::GraphSelectionRef::Artifact { key }
        | crate::ui::view::GraphSelectionRef::RunForestNode { key } => key.clone(),
    }
}

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "dev",
    feature = "native-benchmark"
))]
fn print_benchmark_result(result: &BenchmarkWriteResult) {
    println!("Benchmark report written: {}", result.report_path.display());
    println!(
        "Benchmark summary written: {}",
        result.readme_path.display()
    );
}

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "dev",
    feature = "native-benchmark"
))]
fn elapsed_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().try_into().unwrap_or(u64::MAX)
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
