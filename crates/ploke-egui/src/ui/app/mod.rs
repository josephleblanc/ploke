//! eframe application shell for the operator graph UI.

pub(crate) mod layout;
pub(crate) mod shell;

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "dev",
    feature = "native-benchmark"
))]
use std::time::Instant;

use eframe::egui;
use ploke_tree::Graph;
#[cfg(not(target_arch = "wasm32"))]
use ploke_tree::GraphSnapshot;

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "dev",
    feature = "native-benchmark"
))]
use crate::benchmark::{
    BenchmarkAction, BenchmarkActionReport, BenchmarkController, BenchmarkInspectorSection,
    BenchmarkSelectionTarget, BenchmarkWriteResult, InspectorSectionPhase, InspectorSequenceStage,
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
use crate::ui::charts;
use crate::ui::diff::PatchDiffCache;
#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "dev",
    feature = "native-benchmark"
))]
use crate::ui::inspector::default_selections;
use crate::ui::inspector::{GraphRevision, InspectorCache, SelectionInspector};
use crate::ui::view::{ArtifactTreeFilters, GraphView, GraphViewDiagnostics, GraphViewMode};

#[derive(Debug)]
pub struct OperatorApp {
    graph: Graph,
    graph_revision: GraphRevision,
    view: GraphView,
    inspector_cache: InspectorCache,
    inspector_render_cache: shell::InspectorRenderCache,
    patch_diff_cache: PatchDiffCache,
    dashboard_tree: egui_tiles::Tree<crate::ui::dashboard::tiles::Pane>,
    #[cfg(not(target_arch = "wasm32"))]
    run_picker: RunPicker,
    #[cfg(not(target_arch = "wasm32"))]
    run_error: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    graph_snapshot: Option<GraphSnapshot>,
    #[cfg(not(target_arch = "wasm32"))]
    graph_snapshot_path: String,
    #[cfg(not(target_arch = "wasm32"))]
    graph_snapshot_label: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    graph_snapshot_status: Option<String>,
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
    benchmark_inspector_section: Option<BenchmarkInspectorSection>,
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    benchmark_inspector_exclusive: bool,
}

impl OperatorApp {
    pub fn new(graph: Graph) -> Self {
        let dashboard_tree = crate::ui::dashboard::tiles::create_default_tree_for_graph(&graph);
        Self {
            graph,
            graph_revision: GraphRevision::default(),
            view: GraphView::default(),
            inspector_cache: InspectorCache::default(),
            inspector_render_cache: shell::InspectorRenderCache::default(),
            patch_diff_cache: PatchDiffCache::default(),
            dashboard_tree,
            #[cfg(not(target_arch = "wasm32"))]
            run_picker: RunPicker::from_default_root(),
            #[cfg(not(target_arch = "wasm32"))]
            run_error: None,
            #[cfg(not(target_arch = "wasm32"))]
            graph_snapshot: None,
            #[cfg(not(target_arch = "wasm32"))]
            graph_snapshot_path: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            graph_snapshot_label: None,
            #[cfg(not(target_arch = "wasm32"))]
            graph_snapshot_status: None,
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
            benchmark_inspector_section: None,
            #[cfg(all(
                not(target_arch = "wasm32"),
                feature = "dev",
                feature = "native-benchmark"
            ))]
            benchmark_inspector_exclusive: false,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_with_run_picker(graph: Graph, run_picker: RunPicker) -> Self {
        let dashboard_tree = crate::ui::dashboard::tiles::create_default_tree_for_graph(&graph);
        Self {
            graph,
            graph_revision: GraphRevision::default(),
            view: GraphView::default(),
            inspector_cache: InspectorCache::default(),
            inspector_render_cache: shell::InspectorRenderCache::default(),
            patch_diff_cache: PatchDiffCache::default(),
            dashboard_tree,
            run_picker,
            run_error: None,
            graph_snapshot: None,
            graph_snapshot_path: String::new(),
            graph_snapshot_label: None,
            graph_snapshot_status: None,
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
            benchmark_inspector_section: None,
            #[cfg(all(
                not(target_arch = "wasm32"),
                feature = "dev",
                feature = "native-benchmark"
            ))]
            benchmark_inspector_exclusive: false,
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

    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_graph_snapshot(mut self, path: PathBuf, snapshot: GraphSnapshot) -> Self {
        self.graph_snapshot_path = path.display().to_string();
        self.graph_snapshot_label = Some(snapshot_label(path.as_path()));
        self.graph_snapshot = Some(snapshot);
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

    pub fn load(&mut self, storage: &dyn eframe::Storage) {
        if let Some(tree) = eframe::get_value(storage, "ploke-egui-dashboard") {
            self.dashboard_tree = tree;
        }
        if crate::ui::dashboard::tiles::prefers_eval_protocol_pane(&self.graph) {
            self.dashboard_tree =
                crate::ui::dashboard::tiles::create_default_tree_for_graph(&self.graph);
        }
    }

    fn current_run_name(&self) -> Option<&str> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            return self
                .run_picker
                .selected_run_name()
                .or(self.graph_snapshot_label.as_deref())
                .filter(|name| !name.is_empty());
        }

        #[cfg(target_arch = "wasm32")]
        {
            None
        }
    }

    #[cfg_attr(
        all(not(target_arch = "wasm32"), feature = "native-benchmark"),
        tracing::instrument(skip_all, name = "run_navigation")
    )]
    fn render_run_navigation_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                #[cfg(not(target_arch = "wasm32"))]
                self.render_run_picker(ui);
                #[cfg(not(target_arch = "wasm32"))]
                self.render_graph_snapshot_controls(ui);
                #[cfg(not(target_arch = "wasm32"))]
                ui.separator();
                render_mode_picker(ui, &mut self.view);
                render_quick_filters(ui, &mut self.view);
                render_tile_picker(ui, &mut self.dashboard_tree, &self.graph);

                ui.separator();
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
    }
}

impl eframe::App for OperatorApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "ploke-egui-dashboard", &self.dashboard_tree);
    }

    #[cfg_attr(
        all(not(target_arch = "wasm32"), feature = "native-benchmark"),
        tracing::instrument(skip_all, name = "frame_update")
    )]
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
        {
            let _span = tracing::trace_span!("egui_panel_top_strip_layout").entered();
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
        }
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
        {
            let _span = tracing::trace_span!("egui_panel_run_navigation_layout").entered();
            egui::Panel::left("run_navigation")
                .default_size(layout::LEFT_SIDEBAR_WIDTH)
                .max_size(layout::LEFT_SIDEBAR_MAX_WIDTH)
                .show_inside(ui, |ui| {
                    profiling::scope!("ploke-egui.frame.run-navigation");
                    self.render_run_navigation_panel(ui);
                });
        }
        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        self.record_benchmark_component("run_navigation", run_navigation_start);

        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        let timeline_start = Instant::now();
        {
            let _span = tracing::trace_span!("egui_panel_timeline_layout").entered();
            egui::Panel::bottom("timeline")
                .default_size(layout::BOTTOM_TIMELINE_HEIGHT)
                .show_inside(ui, |ui| {
                    profiling::scope!("ploke-egui.frame.timeline");
                    let selection_synced = self.view.selected_node(&self.graph).is_some();
                    shell::render_bottom_timeline(
                        ui,
                        self.view.diagnostics().as_ref(),
                        selection_synced,
                    );
                });
        }
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
        {
            let _span = tracing::trace_span!("egui_panel_central_layout").entered();
            egui::CentralPanel::default().show_inside(ui, |ui| {
                profiling::scope!("ploke-egui.frame.central");

                let mut actions = Vec::new();
                let mut behavior = crate::ui::dashboard::tiles::TreeBehavior {
                    graph: &self.graph,
                    view: &mut self.view,
                    inspector_cache: &mut self.inspector_cache,
                    inspector_render_cache: &mut self.inspector_render_cache,
                    patch_diff_cache: &mut self.patch_diff_cache,
                    graph_revision: self.graph_revision,
                    actions: &mut actions,
                    #[cfg(all(
                        not(target_arch = "wasm32"),
                        feature = "dev",
                        feature = "native-benchmark"
                    ))]
                    benchmark_inspector_section: self.benchmark_inspector_section,
                    #[cfg(all(
                        not(target_arch = "wasm32"),
                        feature = "dev",
                        feature = "native-benchmark"
                    ))]
                    benchmark_inspector_exclusive: self.benchmark_inspector_exclusive,
                };

                self.dashboard_tree.ui(&mut behavior, ui);

                for action in actions {
                    use crate::ui::dashboard::tiles::{Pane, TreeAction};
                    match action {
                        TreeAction::Pin(reference) => {
                            let pane = Pane::PinnedInspector(reference);
                            let id = self.dashboard_tree.tiles.insert_pane(pane);
                            if let Some(root) = self.dashboard_tree.root {
                                if let Some(egui_tiles::Tile::Container(container)) =
                                    self.dashboard_tree.tiles.get_mut(root)
                                {
                                    container.add_child(id);
                                }
                            } else {
                                self.dashboard_tree.root = Some(id);
                            }
                        }
                        TreeAction::PinSection(reference, section) => {
                            let pane = Pane::InspectorSection(reference, section);
                            let id = self.dashboard_tree.tiles.insert_pane(pane);
                            if let Some(root) = self.dashboard_tree.root {
                                if let Some(egui_tiles::Tile::Container(container)) =
                                    self.dashboard_tree.tiles.get_mut(root)
                                {
                                    container.add_child(id);
                                }
                            } else {
                                self.dashboard_tree.root = Some(id);
                            }
                        }
                        TreeAction::Remove(tile_id) => {
                            self.dashboard_tree.tiles.remove(tile_id);
                        }
                    }
                }
            });
        }
        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        self.record_benchmark_component("central_graph", central_start);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _span = tracing::trace_span!("emit_diagnostics").entered();
            self.emit_diagnostics(ui.ctx());
        }
        #[cfg(all(
            not(target_arch = "wasm32"),
            feature = "dev",
            feature = "native-benchmark"
        ))]
        let capture_start = Instant::now();
        {
            let _span = tracing::trace_span!("benchmark_finish_frame").entered();
            profiling::finish_frame!();
        }
        #[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
        {
            let _span = tracing::trace_span!("puffin_capture").entered();
            self.emit_puffin_capture(ui.ctx());
        }
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
                    self.run_picker.record_loaded_graph(&path, &graph);
                    self.replace_graph(graph);
                    self.graph_snapshot = None;
                    self.graph_snapshot_label = None;
                    self.run_error = None;
                    self.graph_snapshot_status = None;
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

    fn render_graph_snapshot_controls(&mut self, ui: &mut egui::Ui) {
        profiling::scope!("ploke-egui.render-graph-snapshot-controls");
        ui.separator();
        ui.label("Graph snapshot");
        ui.text_edit_singleline(&mut self.graph_snapshot_path);
        ui.horizontal(|ui| {
            if ui.button("Load").clicked() {
                self.load_graph_snapshot_from_control();
            }
            if ui.button("Export").clicked() {
                self.export_graph_snapshot_from_control();
            }
        });
        if let Some(status) = &self.graph_snapshot_status {
            ui.label(status.as_str());
        }
    }

    fn load_graph_snapshot_from_control(&mut self) {
        let Some(path) = snapshot_control_path(&self.graph_snapshot_path) else {
            self.graph_snapshot_status = Some("Graph snapshot path is empty".to_owned());
            return;
        };

        match GraphSnapshot::read_json(&path) {
            Ok(snapshot) => {
                let graph = snapshot.graph();
                self.replace_graph(graph);
                self.run_picker.clear_selection();
                self.run_error = None;
                self.graph_snapshot_label = Some(snapshot_label(path.as_path()));
                self.graph_snapshot = Some(snapshot);
                self.graph_snapshot_status =
                    Some(format!("Graph snapshot loaded: {}", path.display()));
            }
            Err(error) => {
                self.graph_snapshot_status = Some(format!("Graph snapshot load failed: {error}"));
            }
        }
    }

    fn export_graph_snapshot_from_control(&mut self) {
        let Some(path) = snapshot_control_path(&self.graph_snapshot_path) else {
            self.graph_snapshot_status = Some("Graph snapshot path is empty".to_owned());
            return;
        };

        let result = if let Some(snapshot) = &self.graph_snapshot {
            snapshot.write_json(&path)
        } else if let Some(run) = self.run_picker.selected_run() {
            GraphSnapshot::from_run_root(&run.path).and_then(|snapshot| snapshot.write_json(&path))
        } else {
            self.graph_snapshot_status =
                Some("Select a run or load a graph snapshot before export".to_owned());
            return;
        };

        match result {
            Ok(()) => {
                self.graph_snapshot_path = path.display().to_string();
                self.graph_snapshot_status =
                    Some(format!("Graph snapshot exported: {}", path.display()));
            }
            Err(error) => {
                self.graph_snapshot_status = Some(format!("Graph snapshot export failed: {error}"));
            }
        }
    }

    fn replace_graph(&mut self, graph: Graph) {
        let mode = self.view.mode();
        self.graph = graph;
        self.dashboard_tree =
            crate::ui::dashboard::tiles::create_default_tree_for_graph(&self.graph);
        self.graph_revision = self.graph_revision.next();
        self.view = GraphView::default();
        self.view.set_mode(mode);
        self.inspector_cache = InspectorCache::default();
        self.inspector_render_cache = shell::InspectorRenderCache::default();
        self.patch_diff_cache = PatchDiffCache::default();
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

#[cfg(not(target_arch = "wasm32"))]
fn snapshot_control_path(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn snapshot_label(path: &Path) -> String {
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .map(str::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::fs;

    use ploke_records::ids::CampaignId;
    use ploke_records::scheduler::SchedulerStateRecord;
    use ploke_tree::{
        AgentTurnRecordSet, PassiveEvidence, RunForestInput, RunRecordSet, TransitionJournal,
    };

    use super::*;

    #[test]
    fn graph_snapshot_controls_load_and_export_snapshot() {
        let dir = std::env::temp_dir().join(format!(
            "ploke-egui-snapshot-controls-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create test dir");
        let input = dir.join("input.json");
        let output = dir.join("output.json");
        GraphSnapshot::from_records(empty_record_set())
            .write_json(&input)
            .expect("write input snapshot");

        let picker = RunPicker::from_root_deferred(dir.clone());
        let mut app = OperatorApp::new_with_run_picker(crate::demo::sample_graph(), picker);
        app.graph_snapshot_path = input.display().to_string();
        app.load_graph_snapshot_from_control();

        assert!(app.graph_snapshot.is_some());
        assert_eq!(
            app.graph
                .forest
                .as_ref()
                .expect("loaded forest")
                .campaign
                .campaign_id,
            "campaign-ui"
        );

        app.graph_snapshot_path = output.display().to_string();
        app.export_graph_snapshot_from_control();
        let exported = GraphSnapshot::read_json(&output).expect("read exported snapshot");
        assert_eq!(
            exported
                .graph()
                .forest
                .as_ref()
                .expect("exported forest")
                .campaign
                .campaign_id,
            "campaign-ui"
        );

        fs::remove_dir_all(&dir).expect("remove test dir");
    }

    fn empty_record_set() -> RunRecordSet {
        RunRecordSet {
            forest_input: RunForestInput {
                scheduler: SchedulerStateRecord {
                    schema_version: "prototype1-scheduler.v1".to_owned(),
                    campaign_id: CampaignId("campaign-ui".to_owned()),
                    updated_at: "2026-05-20T00:00:00Z".to_owned(),
                    policy: Default::default(),
                    frontier_node_ids: Vec::new(),
                    completed_node_ids: Vec::new(),
                    failed_node_ids: Vec::new(),
                    last_continuation_decision: None,
                    nodes: Vec::new(),
                },
                node_records: Vec::new(),
                parent_identity: None,
                successor_ready: Vec::new(),
                successor_completion: Vec::new(),
                passive_evidence: PassiveEvidence::default(),
            },
            history_blocks: Vec::new(),
            transition_journal: TransitionJournal::default(),
            agent_turn_records: AgentTurnRecordSet::default(),
        }
    }
}

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "dev",
    feature = "native-benchmark"
))]
impl OperatorApp {
    fn benchmark_begin_frame(&mut self) {
        let _span = tracing::trace_span!("benchmark_frame_begin").entered();
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
        let _span = tracing::trace_span!("benchmark_frame_end").entered();
        let Some(benchmark) = &mut self.benchmark else {
            return;
        };
        let _end_frame_span = tracing::trace_span!("benchmark_end_frame").entered();
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
        let _span = tracing::trace_span!("benchmark_apply_action").entered();
        let mut report = BenchmarkActionReport::for_action(action);
        self.benchmark_inspector_section = None;
        self.benchmark_inspector_exclusive = false;
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
                inspector_section,
                reset_patch_cache,
            } => {
                self.benchmark_inspector_section = inspector_section;
                if let Some(section) = inspector_section {
                    report
                        .notes
                        .push(format!("forced_inspector_section={}", section.as_str()));
                }
                if reset_patch_cache {
                    self.patch_diff_cache = PatchDiffCache::default();
                    report.notes.push("patch_diff_cache_reset=true".to_owned());
                }
                self.apply_benchmark_selection(&mut report);
            }
            BenchmarkAction::InspectorSequence { stage } => match stage {
                InspectorSequenceStage::Warmup => {
                    report
                        .notes
                        .push("sequence_stage_action=warmup_idle".to_owned());
                }
                InspectorSequenceStage::SelectArtifact => {
                    report
                        .notes
                        .push("sequence_stage_action=select_artifact".to_owned());
                    self.apply_benchmark_selection(&mut report);
                }
                InspectorSequenceStage::OpenSection(section) => {
                    self.benchmark_inspector_section = Some(section);
                    self.benchmark_inspector_exclusive = true;
                    report.notes.push(format!(
                        "sequence_stage_action=force_only_{}",
                        section.as_str()
                    ));
                }
            },
            BenchmarkAction::InspectorSectionPhase {
                section,
                target,
                phase,
            } => match phase {
                InspectorSectionPhase::IdleBeforeSelection => {
                    self.view.clear_selection(&self.graph);
                    if section == BenchmarkInspectorSection::PatchDebug {
                        self.patch_diff_cache = PatchDiffCache::default();
                        report.notes.push("patch_diff_cache_reset=true".to_owned());
                    }
                    report
                        .notes
                        .push("phase_sequence_action=idle_unselected".to_owned());
                }
                InspectorSectionPhase::SelectNode => {
                    self.benchmark_inspector_exclusive = true;
                    self.apply_benchmark_selection_target(target, &mut report);
                    report
                        .notes
                        .push(format!("phase_sequence_action=select_{}", target.as_str()));
                }
                InspectorSectionPhase::ExpandSection => {
                    self.benchmark_inspector_section = Some(section);
                    self.benchmark_inspector_exclusive = true;
                    report
                        .notes
                        .push(format!("phase_sequence_action=expand_{}", section.as_str()));
                }
                InspectorSectionPhase::CollapseSection => {
                    self.benchmark_inspector_exclusive = true;
                    report.notes.push(format!(
                        "phase_sequence_action=collapse_{}",
                        section.as_str()
                    ));
                }
                InspectorSectionPhase::UnselectNode => {
                    self.view.clear_selection(&self.graph);
                    report
                        .notes
                        .push("phase_sequence_action=unselect_node".to_owned());
                }
                InspectorSectionPhase::IdleSelectedCollapsed
                | InspectorSectionPhase::IdleExpanded
                | InspectorSectionPhase::IdleCollapsed
                | InspectorSectionPhase::IdleAfterUnselect => {}
            },
        }
        report
    }

    fn apply_benchmark_selection(&mut self, report: &mut BenchmarkActionReport) {
        self.apply_benchmark_selection_target(BenchmarkSelectionTarget::Primary, report);
    }

    fn apply_benchmark_selection_target(
        &mut self,
        target: BenchmarkSelectionTarget,
        report: &mut BenchmarkActionReport,
    ) {
        let selections = default_selections(&self.graph);
        let primary = preferred_benchmark_selection(&self.graph, &selections);
        let alternate = primary
            .as_ref()
            .and_then(|primary| {
                selections.iter().find(|selection| {
                    selection_key(&selection.reference) != selection_key(&primary.reference)
                        && matches!(
                            SelectionInspector::from_graph(&self.graph, selection),
                            SelectionInspector::Artifact(artifact) if !artifact.patches.is_empty()
                        )
                })
            })
            .cloned()
            .or_else(|| {
                primary.as_ref().and_then(|primary| {
                    selections
                        .iter()
                        .find(|selection| {
                            selection_key(&selection.reference) != selection_key(&primary.reference)
                        })
                        .cloned()
                })
            });

        let (selection, fallback) = match target {
            BenchmarkSelectionTarget::Primary => (primary, None),
            BenchmarkSelectionTarget::Alternate => {
                let fallback = alternate
                    .is_none()
                    .then(|| "no_alternate_visible_artifact_selection".to_owned());
                (alternate.or(primary), fallback)
            }
        };

        let Some(selection) = selection else {
            report.fallback = Some("no_visible_artifact_selection".to_owned());
            return;
        };

        if let Some(fallback) = fallback {
            report.fallback = Some(fallback);
        }
        report
            .notes
            .push(format!("selection_target={}", target.as_str()));
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
fn preferred_benchmark_selection(
    graph: &Graph,
    selections: &[crate::ui::view::GraphSelectionDetail],
) -> Option<crate::ui::view::GraphSelectionDetail> {
    let first = selections.first().cloned();
    selections
        .iter()
        .find(|selection| {
            matches!(
                SelectionInspector::from_graph(graph, selection),
                SelectionInspector::Artifact(artifact) if !artifact.patches.is_empty()
            )
        })
        .cloned()
        .or(first)
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

fn render_tile_picker(
    ui: &mut egui::Ui,
    tree: &mut egui_tiles::Tree<crate::ui::dashboard::tiles::Pane>,
    graph: &Graph,
) {
    ui.separator();
    ui.label("Tiles");
    ui.horizontal_wrapped(|ui| {
        use crate::ui::dashboard::tiles::Pane;

        let options = [
            Pane::Graph,
            Pane::EvalProtocol,
            Pane::Inspector,
            Pane::ArtifactDistribution,
            Pane::RecentActivity,
            Pane::StatsSummary,
            Pane::Diagnostics,
        ];

        for pane in options {
            if ui.button(pane.title()).clicked() {
                let id = tree.tiles.insert_pane(pane);
                if let Some(root) = tree.root {
                    if let Some(egui_tiles::Tile::Container(container)) = tree.tiles.get_mut(root) {
                        container.add_child(id);
                    }
                } else {
                    tree.root = Some(id);
                }
            }
        }

        if ui.button("🔄 Reset Layout").clicked() {
            *tree = crate::ui::dashboard::tiles::create_default_tree_for_graph(graph);
        }
    });
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

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "diagnostics")
)]
pub(crate) fn render_diagnostics(ui: &mut egui::Ui, diagnostics: &GraphViewDiagnostics) {
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
