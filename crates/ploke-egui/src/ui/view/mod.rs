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
    layout_viewport_fit_remaining: u8,
    layout_viewport_fit_viewport: Option<Vec2>,
    layout_state_pending: bool,
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
            layout_viewport_fit_remaining: 0,
            layout_viewport_fit_viewport: None,
            layout_state_pending: false,
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

    /// Recomputes lineage spacing from the graph pane size and centers at zoom 1.
    pub(crate) fn request_graph_layout_fit(&mut self, ui: &mut egui::Ui, viewport: Vec2) {
        let view_style = self.view_style;
        let base_state = self.cache.layout_state(view_style);
        let Some((scaled_state, _post_layout_bounds)) =
            fit::apply_graph_layout_fit(self.cache.graph_mut(), base_state, viewport, view_style)
        else {
            return;
        };
        self.cache.invalidate_layout_diagnostics();
        egui_graphs::set_layout_state(ui, scaled_state, Some(self.id.clone()));
        self.layout_viewport_fit_viewport = Some(viewport);
        // Re-apply after widget draw so egui_graphs pan/top-left compensation cannot leave a stale offset.
        self.layout_viewport_fit_remaining = 2;
        ui.ctx().request_repaint();
    }

    fn apply_pending_graph_layout_viewport_fit(
        &mut self,
        ui: &mut egui::Ui,
        custom_id: &Option<String>,
        viewport: Vec2,
    ) {
        if self.layout_viewport_fit_remaining == 0 {
            return;
        }

        let view_style = self.view_style;
        let Some(bounds) = fit::graph_fit_bounds(self.cache.graph_mut(), view_style) else {
            self.layout_viewport_fit_remaining = 0;
            self.layout_viewport_fit_viewport = None;
            return;
        };
        fit::apply_graph_layout_viewport_fit(ui, custom_id, viewport, bounds);
        self.layout_viewport_fit_remaining -= 1;
        if self.layout_viewport_fit_remaining == 0 {
            self.layout_viewport_fit_viewport = None;
        }
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
            egui_graphs::set_layout_state(
                ui,
                self.cache.layout_state(self.view_style),
                Some(self.id.clone()),
            );
        }

        let fit_now = std::mem::take(&mut self.fit_next_frame);
        {
            let _span = tracing::trace_span!(scope::CENTRAL_GRAPH_NAVIGATION_PREPARE).entered();
            self.navigation = navigation(self.view_style.layout.fit_padding);
        }

        label::reset_edge_label_diagnostics(ui.ctx());
        let custom_id = Some(self.id.clone());
        if fit_now {
            fit::apply_graph_screen_fit(
                ui,
                ui.auto_id_with("graph-fit-prep"),
                &custom_id,
                ui.max_rect().min,
                viewport,
                self.cache.graph_mut(),
                self.view_style,
            );
        }

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
        if fit_now {
            // egui_graphs first-frame fit can overwrite pre-add metadata; re-apply after draw.
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
        if let Some(viewport) = self.layout_viewport_fit_viewport {
            self.apply_pending_graph_layout_viewport_fit(ui, &custom_id, viewport);
        }
        let graph_rect = response.rect;
        let view_style = self.view_style;
        let relayout_available =
            fit::graph_fit_bounds(self.cache.graph_mut(), view_style).is_some();
        if show_graph_layout_fit_button(ui, graph_rect, relayout_available) {
            let viewport = graph_rect.size();
            self.request_graph_layout_fit(ui, viewport);
            self.apply_pending_graph_layout_viewport_fit(ui, &custom_id, viewport);
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
