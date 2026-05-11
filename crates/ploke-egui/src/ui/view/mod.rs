//! 2D graph widget and interaction state.

mod diagnostics;
mod edge;
mod geometry;
mod label;
mod layout;
mod order;
mod projection;
mod style;

use eframe::egui;

pub use style::{
    CurveStyle, EdgeLabelStyle, EdgeStyle, LabelStyle, LayoutStyle, StatusColors, ViewStyle,
};

use crate::graph::Graph as DomainGraph;

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
        self.diagnostics
    }

    pub fn show(&mut self, ui: &mut egui::Ui, graph: &DomainGraph) {
        let viewport = ui.available_size();
        if viewport_resized(self.last_viewport, viewport) {
            self.fit_next_frame = true;
            self.last_viewport = Some(viewport);
        }

        if self.cache.refresh(graph, self.view_style) {
            egui_graphs::set_layout_state(
                ui,
                layout::State {
                    triggered: false,
                    row_dist: self.view_style.layout.row_distance,
                    col_dist: self.view_style.layout.column_distance,
                },
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
                .diagnostics(graph, response.rect.size(), self.view_style, edge_labels);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphViewDiagnostics {
    pub node_count: usize,
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
    pub candidate_candidate: usize,
    pub candidate_history: usize,
    pub history_history: usize,
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
