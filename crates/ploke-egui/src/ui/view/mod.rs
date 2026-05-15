//! 2D graph widget and interaction state.

pub mod artifact_tree;
mod diagnostics;
mod edge;
mod geometry;
mod label;
mod layout;
mod node;
mod projection;
mod style;

use eframe::egui;
use eframe::egui::Vec2;
use ploke_tree::Graph as DomainGraph;

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
    diagnostics: Option<GraphViewDiagnostics>,
    mode: GraphViewMode,
}

impl Default for GraphView {
    fn default() -> Self {
        let view_style = ViewStyle::default();
        Self {
            cache: GraphViewCache::default(),
            id: GRAPH_VIEW_ID.to_owned(),
            interaction: egui_graphs::SettingsInteraction::new()
                .with_dragging_enabled(true)
                .with_node_selection_enabled(true)
                .with_edge_selection_enabled(true),
            navigation: navigation(view_style.layout.fit_padding, false),
            style: egui_graphs::SettingsStyle::new().with_labels_always(true),
            view_style,
            last_viewport: None,
            fit_next_frame: true,
            diagnostics: None,
            mode: GraphViewMode::ArtifactTree,
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

    pub fn selected_node_detail(&self) -> Option<GraphSelectionDetail> {
        self.cache.selected_node_detail()
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

    pub fn contract_diagnostics(
        graph: &DomainGraph,
        mode: GraphViewMode,
        viewport_size: Vec2,
    ) -> Option<GraphViewDiagnostics> {
        let view_style = ViewStyle::default();
        let mut cache = GraphViewCache::default();
        cache.refresh(graph, view_style, mode);
        cache.diagnostics(viewport_size, view_style, EdgeLabelDiagnostics::default())
    }

    pub fn show(&mut self, ui: &mut egui::Ui, graph: &DomainGraph) {
        let viewport = ui.available_size();
        if viewport_resized(self.last_viewport, viewport) {
            self.fit_next_frame = true;
            self.last_viewport = Some(viewport);
        }

        if self.cache.refresh(graph, self.view_style, self.mode) {
            egui_graphs::set_layout_state(
                ui,
                self.cache.layout_state(self.view_style),
                Some(self.id.clone()),
            );
            self.fit_next_frame = true;
        }

        let fit_now = std::mem::take(&mut self.fit_next_frame);
        self.navigation = navigation(self.view_style.layout.fit_padding, fit_now);

        let mut widget =
            egui_graphs::GraphView::<_, _, _, _, _, _, layout::State, layout::Lineage>::new(
                self.cache.graph_mut(),
            )
            .with_id(Some(self.id.clone()))
            .with_interactions(&self.interaction)
            .with_navigations(&self.navigation)
            .with_styles(&self.style);

        label::reset_edge_label_diagnostics(ui.ctx());
        let response = ui.add(&mut widget);
        let edge_labels = label::edge_label_diagnostics(ui.ctx());
        self.diagnostics =
            self.cache
                .diagnostics(response.rect.size(), self.view_style, edge_labels);
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSelectionDetail {
    pub kind: String,
    pub label: String,
    pub detail: String,
    pub reference: GraphSelectionRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
