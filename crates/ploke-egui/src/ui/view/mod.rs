//! 2D graph widget and interaction state.

pub mod artifact_tree;
mod diagnostics;
mod edge;
mod effects;
mod geometry;
mod label;
mod layout;
mod node;
mod projection;
mod style;

use eframe::egui;
use eframe::egui::Vec2;
use ploke_tree::Graph as DomainGraph;
use serde::{Deserialize, Serialize};

pub use style::{
    CurveStyle, EdgeLabelStyle, EdgeStyle, LabelStyle, LayoutStyle, StatusColors, ViewStyle,
};

use projection::GraphViewCache;

const GRAPH_VIEW_ID: &str = "ploke-egui-run-graph";
const RESIZE_FIT_THRESHOLD: f32 = 8.0;

#[derive(Debug)]
pub struct GraphView {
    cache: GraphViewCache,
    id: String,
    interaction: egui_graphs::SettingsInteraction,
    navigation: egui_graphs::SettingsNavigation,
    style: egui_graphs::SettingsStyle,
    view_style: ViewStyle,
    last_viewport: Option<egui::Vec2>,
    fit_next_frame: bool,
    layout_state_pending: bool,
    diagnostics: Option<GraphViewDiagnostics>,
    mode: GraphViewMode,
    artifact_tree_filters: ArtifactTreeFilters,
}

impl Default for GraphView {
    fn default() -> Self {
        let view_style = ViewStyle::default();
        Self {
            cache: GraphViewCache::default(),
            id: GRAPH_VIEW_ID.to_owned(),
            interaction: egui_graphs::SettingsInteraction::new()
                .with_dragging_enabled(false)
                .with_node_selection_enabled(true)
                .with_edge_selection_enabled(true),
            navigation: navigation(view_style.layout.fit_padding, false),
            style: egui_graphs::SettingsStyle::new().with_labels_always(true),
            view_style,
            last_viewport: None,
            fit_next_frame: true,
            layout_state_pending: false,
            diagnostics: None,
            mode: GraphViewMode::ArtifactTree,
            artifact_tree_filters: ArtifactTreeFilters::default(),
        }
    }
}

impl GraphView {
    pub fn with_style(mut self, style: ViewStyle) -> Self {
        self.navigation = navigation(style.layout.fit_padding, false);
        self.view_style = style;
        self.fit_next_frame = true;
        self
    }

    pub fn with_edge_style(mut self, edge_style: EdgeStyle) -> Self {
        self.view_style.edge = edge_style;
        self
    }

    pub fn diagnostics(&self) -> Option<GraphViewDiagnostics> {
        self.diagnostics.clone()
    }

    pub fn selected_node_detail(&mut self, graph: &DomainGraph) -> Option<GraphSelectionDetail> {
        self.sync_projection(graph);
        self.cache.selected_node_detail()
    }

    pub fn selected_reference(&mut self, graph: &DomainGraph) -> Option<&GraphSelectionRef> {
        self.sync_projection(graph);
        self.cache.selected_reference()
    }

    pub fn selected_label(&self) -> Option<&str> {
        self.cache.selected_label()
    }

    pub fn selected_kind(&self) -> Option<&'static str> {
        self.cache.selected_kind()
    }

    pub fn selected_node(
        &mut self,
        graph: &DomainGraph,
    ) -> Option<(&GraphSelectionRef, &str, &'static str)> {
        self.sync_projection(graph);
        self.cache.selected_node()
    }

    pub fn select_reference(&mut self, graph: &DomainGraph, reference: &GraphSelectionRef) -> bool {
        self.sync_projection(graph);
        self.cache.select_reference(reference)
    }

    pub fn clear_selection(&mut self, graph: &DomainGraph) {
        self.sync_projection(graph);
        self.cache.clear_selection();
    }

    pub fn mode(&self) -> GraphViewMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: GraphViewMode) {
        if self.mode != mode {
            self.mode = mode;
            self.fit_next_frame = true;
        }
    }

    pub fn artifact_tree_filters(&self) -> ArtifactTreeFilters {
        self.artifact_tree_filters
    }

    pub fn set_hide_unconsidered_children(&mut self, hide: bool) {
        if self.artifact_tree_filters.hide_unconsidered_children != hide {
            self.artifact_tree_filters.hide_unconsidered_children = hide;
            self.fit_next_frame = true;
        }
    }

    pub fn contract_diagnostics(
        graph: &DomainGraph,
        mode: GraphViewMode,
        viewport_size: Vec2,
    ) -> Option<GraphViewDiagnostics> {
        profiling::scope!("ploke-egui.graph-view.contract-diagnostics");
        let view_style = ViewStyle::default();
        let mut cache = GraphViewCache::default();
        cache.refresh(graph, view_style, mode, ArtifactTreeFilters::default());
        cache.diagnostics(viewport_size, view_style, EdgeLabelDiagnostics::default())
    }

    #[cfg_attr(
        all(not(target_arch = "wasm32"), feature = "native-benchmark"),
        tracing::instrument(skip_all, name = "central_graph")
    )]
    pub fn show(&mut self, ui: &mut egui::Ui, graph: &DomainGraph) {
        profiling::scope!("ploke-egui.graph-view.show");
        let viewport = ui.available_size();
        if viewport_resized(self.last_viewport, viewport) {
            self.fit_next_frame = true;
            self.last_viewport = Some(viewport);
        }

        self.sync_projection(graph);
        if std::mem::take(&mut self.layout_state_pending) {
            let _span = tracing::trace_span!("central_graph_layout_state_restore").entered();
            egui_graphs::set_layout_state(
                ui,
                self.cache.layout_state(self.view_style),
                Some(self.id.clone()),
            );
        }

        let fit_now = std::mem::take(&mut self.fit_next_frame);
        {
            let _span = tracing::trace_span!("central_graph_navigation_prepare").entered();
            self.navigation = navigation(self.view_style.layout.fit_padding, fit_now);
        }

        let mut widget = {
            let _span = tracing::trace_span!("central_graph_widget_build").entered();
            egui_graphs::GraphView::<_, _, _, _, _, _, layout::State, layout::Lineage>::new(
                self.cache.graph_mut(),
            )
            .with_id(Some(self.id.clone()))
            .with_interactions(&self.interaction)
            .with_navigations(&self.navigation)
            .with_styles(&self.style)
        };

        label::reset_edge_label_diagnostics(ui.ctx());
        let response = {
            let _span = tracing::trace_span!("central_graph_widget_add").entered();
            ui.add(&mut widget)
        };
        let edge_labels = label::edge_label_diagnostics(ui.ctx());
        {
            let _span = tracing::trace_span!("central_graph_diagnostics_update").entered();
            self.diagnostics =
                self.cache
                    .diagnostics(response.rect.size(), self.view_style, edge_labels);
        }
    }

    #[cfg_attr(
        all(not(target_arch = "wasm32"), feature = "native-benchmark"),
        tracing::instrument(skip_all, name = "graph_projection_cache_refresh")
    )]
    fn sync_projection(&mut self, graph: &DomainGraph) {
        profiling::scope!("ploke-egui.graph-view.sync-projection");
        if self.cache.refresh(
            graph,
            self.view_style,
            self.mode,
            self.artifact_tree_filters,
        ) {
            self.layout_state_pending = true;
            self.fit_next_frame = true;
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArtifactTreeFilters {
    /// archaeology:artifact-child-consideration
    /// proof:docs/active/archaeology/ploke-tree-graph/artifact-child-consideration.md
    pub hide_unconsidered_children: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphViewDiagnostics {
    pub mode: GraphViewMode,
    pub node_count: usize,
    pub edge_count: usize,
    pub connectivity: GraphConnectivityDiagnostics,
    pub artifact_tree: artifact_tree::Shape,
    pub graph_size: egui::Vec2,
    pub viewport_size: egui::Vec2,
    pub aspect_ratio: f32,
    pub viewport_aspect_ratio: f32,
    pub fitted_size: egui::Vec2,
    pub fitted_fill: egui::Vec2,
    pub center_offset: egui::Vec2,
    pub edge_labels: EdgeLabelDiagnostics,
    pub readability: GraphReadabilityDiagnostics,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GraphViewMode {
    #[default]
    ArtifactTree,
    Lineage,
    ArtifactAndLineage,
    Empty,
}

impl GraphViewMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ArtifactTree => "artifact-tree",
            Self::Lineage => "lineage",
            Self::ArtifactAndLineage => "artifact-and-lineage",
            Self::Empty => "empty",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GraphConnectivityDiagnostics {
    pub component_count_before_anchoring: usize,
    pub hidden_record_count: usize,
    pub hidden_edge_count: usize,
    pub hidden_evidence_count: usize,
    pub hidden_operation_count: usize,
    pub hidden_unattached_component_count: usize,
    pub synthetic_anchors_visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphSelectionDetail {
    pub kind: String,
    pub label: String,
    pub detail: String,
    /// archaeology:selection-protocol-evidence
    /// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
    pub reference: GraphSelectionRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphSelectionRef {
    Artifact { key: String },
    RunForestNode { key: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EdgeLabelDiagnostics {
    pub label_count: usize,
    pub collision_count: usize,
    pub edge_intersection_count: usize,
    pub edge_collision_count: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GraphReadabilityDiagnostics {
    pub edge_edge_crossings: usize,
    pub edge_crossings_by_kind: EdgeCrossingsByKind,
    pub long_edge_count: usize,
    pub backtracking_edge_count: usize,
    pub selected_path_crossings: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EdgeCrossingsByKind {
    pub artifact_artifact: usize,
    pub candidate_candidate: usize,
    pub mixed: usize,
}

fn navigation(fit_padding: f32, fit_to_screen: bool) -> egui_graphs::SettingsNavigation {
    egui_graphs::SettingsNavigation::new()
        .with_fit_to_screen_enabled(fit_to_screen)
        .with_zoom_and_pan_enabled(true)
        .with_fit_to_screen_padding(fit_padding)
}

fn viewport_resized(previous: Option<egui::Vec2>, current: egui::Vec2) -> bool {
    let Some(previous) = previous else {
        return true;
    };

    (previous.x - current.x).abs() > RESIZE_FIT_THRESHOLD
        || (previous.y - current.y).abs() > RESIZE_FIT_THRESHOLD
}
