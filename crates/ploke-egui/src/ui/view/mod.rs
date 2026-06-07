//! 2D graph widget and interaction state.

pub mod artifact_tree;
mod diagnostics;
mod edge;
mod effects;
mod fit;
mod geometry;
mod label;
mod layout;
mod node;
mod projection;
mod style;

use eframe::egui;
use eframe::egui::{Id, LayerId, Order, Rect, UiBuilder, Vec2};
use ploke_tree::Graph as DomainGraph;
use serde::{Deserialize, Serialize};

use crate::allocation::scope;

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
    viewport_layout_pending: bool,
    diagnostics: Option<GraphViewDiagnostics>,
    mode: GraphViewMode,
    artifact_tree_filters: ArtifactTreeFilters,
    external_selection: Option<GraphSelectionDetail>,
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
            navigation: navigation(view_style.layout.fit_padding),
            style: egui_graphs::SettingsStyle::new().with_labels_always(true),
            view_style,
            last_viewport: None,
            fit_next_frame: true,
            layout_state_pending: false,
            viewport_layout_pending: false,
            diagnostics: None,
            mode: GraphViewMode::ArtifactTree,
            artifact_tree_filters: ArtifactTreeFilters::default(),
            external_selection: None,
        }
    }
}

impl GraphView {
    pub fn with_style(mut self, style: ViewStyle) -> Self {
        self.navigation = navigation(style.layout.fit_padding);
        self.view_style = style;
        self.fit_next_frame = true;
        self
    }

    pub fn with_edge_style(mut self, edge_style: EdgeStyle) -> Self {
        self.view_style.edge = edge_style;
        self
    }

    pub(crate) fn view_style_mut(&mut self) -> &mut ViewStyle {
        &mut self.view_style
    }

    pub(crate) fn invalidate_projection_cache(&mut self) {
        self.cache = GraphViewCache::default();
        self.layout_state_pending = true;
        self.fit_next_frame = true;
    }

    /// Re-runs lineage at the current spacing, then screen-fits on the next frame.
    pub(crate) fn request_graph_layout_fit(&mut self, ui: &mut egui::Ui) -> bool {
        let view_style = self.view_style;
        let mut layout_state = self.cache.layout_state(view_style);
        let persisted = egui_graphs::get_layout_state::<layout::State>(ui, Some(self.id.clone()));
        layout_state.row_dist = persisted.row_dist;
        layout_state.col_dist = persisted.col_dist;
        layout_state.lane_dist = persisted.lane_dist;
        layout_state.max_columns = persisted.max_columns;
        if fit::relayout_at_spacing(self.cache.graph_mut(), &layout_state, view_style).is_none() {
            return false;
        }
        layout_state.triggered = true;
        self.cache.invalidate_layout_diagnostics();
        self.fit_next_frame = true;
        egui_graphs::set_layout_state(ui, layout_state, Some(self.id.clone()));
        true
    }

    pub fn diagnostics(&self) -> Option<GraphViewDiagnostics> {
        self.diagnostics.clone()
    }

    pub fn selected_node_detail(&mut self, graph: &DomainGraph) -> Option<GraphSelectionDetail> {
        self.sync_projection(graph);
        self.external_selection
            .clone()
            .or_else(|| self.cache.selected_node_detail())
    }

    pub fn selected_reference(&mut self, graph: &DomainGraph) -> Option<&GraphSelectionRef> {
        self.sync_projection(graph);
        if let Some(detail) = &self.external_selection {
            return Some(&detail.reference);
        }
        self.cache.selected_reference()
    }

    pub fn selected_label(&mut self, graph: &DomainGraph) -> Option<&str> {
        self.sync_projection(graph);
        if let Some(detail) = &self.external_selection {
            return Some(detail.label.as_str());
        }
        self.cache.selected_label()
    }

    pub fn selected_kind(&mut self, graph: &DomainGraph) -> Option<&'static str> {
        self.sync_projection(graph);
        if let Some(detail) = &self.external_selection {
            return Some(selection_kind(&detail.reference));
        }
        self.cache.selected_kind()
    }

    pub fn selected_node(
        &mut self,
        graph: &DomainGraph,
    ) -> Option<(&GraphSelectionRef, &str, &'static str)> {
        self.sync_projection(graph);
        if let Some(detail) = &self.external_selection {
            return Some((
                &detail.reference,
                detail.label.as_str(),
                selection_kind(&detail.reference),
            ));
        }
        self.cache.selected_node()
    }

    pub fn set_external_selection(&mut self, graph: &DomainGraph, detail: GraphSelectionDetail) {
        self.sync_projection(graph);
        self.cache.clear_selection();
        self.external_selection = Some(detail);
    }

    pub fn select_reference(&mut self, graph: &DomainGraph, reference: &GraphSelectionRef) -> bool {
        self.sync_projection(graph);
        if matches!(reference, GraphSelectionRef::Selection { .. }) {
            self.cache.clear_selection();
            if let Some(detail) =
                crate::ui::app::shell::selection_detail_for_reference(graph, reference)
            {
                self.external_selection = Some(detail);
                return true;
            }
            return false;
        }
        self.external_selection = None;
        self.cache.select_reference(reference)
    }

    pub fn clear_selection(&mut self, graph: &DomainGraph) {
        self.sync_projection(graph);
        self.external_selection = None;
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
        cache.diagnostics(
            viewport_size,
            1.0,
            view_style,
            EdgeLabelDiagnostics::default(),
        )
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
            let _span = tracing::trace_span!(scope::CENTRAL_GRAPH_LAYOUT_STATE_RESTORE).entered();
            egui_graphs::reset_metadata(ui, Some(self.id.clone()));
            egui_graphs::set_layout_state(
                ui,
                self.cache.layout_state(self.view_style),
                Some(self.id.clone()),
            );
            self.viewport_layout_pending = true;
        }

        let fit_now = std::mem::take(&mut self.fit_next_frame);
        {
            let _span = tracing::trace_span!(scope::CENTRAL_GRAPH_NAVIGATION_PREPARE).entered();
            self.navigation = navigation(self.view_style.layout.fit_padding);
        }

        label::reset_edge_label_diagnostics(ui.ctx());
        let custom_id = Some(self.id.clone());
        let mut widget = {
            let _span = tracing::trace_span!(scope::CENTRAL_GRAPH_WIDGET_BUILD).entered();
            egui_graphs::GraphView::<_, _, _, _, _, _, layout::State, layout::Lineage>::new(
                self.cache.graph_mut(),
            )
            .with_id(Some(self.id.clone()))
            .with_interactions(&self.interaction)
            .with_navigations(&self.navigation)
            .with_styles(&self.style)
        };

        let response = {
            let _span = tracing::trace_span!(scope::CENTRAL_GRAPH_WIDGET_ADD).entered();
            ui.add(&mut widget)
        };
        if std::mem::take(&mut self.viewport_layout_pending) {
            let view_style = self.view_style;
            let base_state = self.cache.layout_state(view_style);
            if let Some(layout_state) = fit::prepare_viewport_layout(
                self.cache.graph_mut(),
                base_state,
                view_style,
                response.rect.size(),
            ) {
                self.cache.invalidate_layout_diagnostics();
                egui_graphs::set_layout_state(ui, layout_state, Some(self.id.clone()));
                self.fit_next_frame = true;
            }
        } else if fit_now {
            fit::apply_graph_screen_fit(
                ui,
                response.id,
                &custom_id,
                response.rect.left_top(),
                response.rect.size(),
                self.cache.graph_mut(),
                self.view_style,
            );
        }
        let graph_rect = response.rect;
        let view_style = self.view_style;
        let relayout_available =
            fit::graph_fit_bounds(self.cache.graph_mut(), view_style).is_some();
        if show_graph_layout_fit_button(ui, graph_rect, relayout_available) {
            let _ = self.request_graph_layout_fit(ui);
        }
        let edge_labels = label::edge_label_diagnostics(ui.ctx());
        {
            let _span = tracing::trace_span!(scope::CENTRAL_GRAPH_DIAGNOSTICS_UPDATE).entered();
            let viewport_zoom = egui_graphs::MetadataFrame::new(custom_id.clone())
                .load(ui)
                .zoom;
            self.diagnostics = self.cache.diagnostics(
                response.rect.size(),
                viewport_zoom,
                self.view_style,
                edge_labels,
            );
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
    Artifact {
        key: String,
    },
    RunForestNode {
        key: String,
    },
    /// archaeology:selection-protocol-evidence
    /// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
    Selection {
        entry_id: String,
    },
}

fn selection_kind(reference: &GraphSelectionRef) -> &'static str {
    match reference {
        GraphSelectionRef::Artifact { .. } => "artifact",
        GraphSelectionRef::RunForestNode { .. } => "run-forest-node",
        GraphSelectionRef::Selection { .. } => "selection",
    }
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

const GRAPH_RELAYOUT_BUTTON_MARGIN: f32 = 8.0;
/// Reserve space for the egui_tiles tab close control in the pane chrome.
const GRAPH_RELAYOUT_TILE_CLOSE_RESERVE: f32 = 32.0;
const GRAPH_RELAYOUT_BUTTON_SIZE: Vec2 = Vec2::new(52.0, 24.0);

fn show_graph_layout_fit_button(
    ui: &mut egui::Ui,
    graph_rect: Rect,
    relayout_available: bool,
) -> bool {
    if graph_rect.width() < GRAPH_RELAYOUT_BUTTON_SIZE.x + GRAPH_RELAYOUT_BUTTON_MARGIN
        || graph_rect.height() < GRAPH_RELAYOUT_BUTTON_SIZE.y + GRAPH_RELAYOUT_BUTTON_MARGIN
    {
        return false;
    }

    let button_rect = Rect::from_min_size(
        egui::pos2(
            graph_rect.right()
                - GRAPH_RELAYOUT_BUTTON_SIZE.x
                - GRAPH_RELAYOUT_BUTTON_MARGIN
                - GRAPH_RELAYOUT_TILE_CLOSE_RESERVE,
            graph_rect.top() + GRAPH_RELAYOUT_BUTTON_MARGIN,
        ),
        GRAPH_RELAYOUT_BUTTON_SIZE,
    );

    let tokens = crate::ui::theme::tokens_from_ui(ui);
    let hover = "Resize node layout to pane";
    let disabled_hover = "No visible graph nodes to fit";
    let text_color = if relayout_available {
        tokens.text
    } else {
        tokens.text.gamma_multiply(0.45)
    };
    let fill = tokens.panel.gamma_multiply(1.35);
    let stroke = egui::Stroke::new(2.0, tokens.accent);
    let layer_id = LayerId::new(Order::Tooltip, Id::new("ploke-egui.graph-relayout-button"));

    let button = ui
        .scope_builder(
            UiBuilder::new()
                .id_salt("graph-relayout-button")
                .max_rect(button_rect)
                .layer_id(layer_id),
            |ui| {
                ui.set_min_size(GRAPH_RELAYOUT_BUTTON_SIZE);
                egui::Frame::new()
                    .fill(fill)
                    .stroke(stroke)
                    .inner_margin(egui::Margin::symmetric(6, 3))
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        let label = egui::RichText::new("↻ Layout").color(text_color);
                        if relayout_available {
                            ui.button(label).on_hover_text(hover)
                        } else {
                            ui.add_enabled(false, egui::Button::new(label))
                                .on_hover_text(disabled_hover)
                        }
                    })
                    .inner
            },
        )
        .inner;

    button.clicked() && relayout_available
}

fn navigation(fit_padding: f32) -> egui_graphs::SettingsNavigation {
    egui_graphs::SettingsNavigation::new()
        .with_fit_to_screen_enabled(false)
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
