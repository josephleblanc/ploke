mod checks;
mod components;
mod layout;
mod text;

use serde::{Deserialize, Serialize};

use crate::diagnostics::{GraphIdentity, SelectionSnapshot};
use crate::ui::inspector::SelectionInspectorSnapshot;
use crate::ui::view::{GraphViewDiagnostics, artifact_tree as tree};

pub use components::ComponentBreakdown;
pub use layout::{Layout, WidthBudget};

const VERSION: &str = "ploke-egui.default-view-contract.v1";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report<'a> {
    pub schema_version: String,
    pub layout: Layout,
    pub graph_identity: Option<GraphIdentity>,
    pub controls: Controls,
    pub center: ArtifactTree,
    pub inspector: Inspector<'a>,
    pub timeline: Timeline,
    pub checks: Vec<Check>,
}

impl<'a> Report<'a> {
    pub(crate) fn from_parts(
        diagnostics: &GraphViewDiagnostics,
        graph_has_content: bool,
        run_error: Option<String>,
        graph_identity: Option<GraphIdentity>,
        selected: Option<SelectionSnapshot<'a>>,
        selected_inspector: Option<SelectionInspectorSnapshot<'a>>,
        component_breakdown: Vec<ComponentBreakdown>,
    ) -> Self {
        let layout = Layout::current();
        let controls = Controls {
            run_selector_present: true,
            mode_selector_present: true,
            load_state: LoadState::from_app_state(graph_has_content, run_error.as_deref()),
            run_error,
            quick_filters_present: false,
        };
        let center = ArtifactTree::from_diagnostics(diagnostics, component_breakdown);
        let inspector = Inspector {
            right_inspector_present: true,
            selected_detail: selected,
            record_refs_present: selected_inspector
                .as_ref()
                .is_some_and(SelectionInspectorSnapshot::has_record_refs),
            drilldown_candidates_present: selected_inspector
                .as_ref()
                .is_some_and(SelectionInspectorSnapshot::has_drilldown_candidates),
            unavailable_reason_classified: true,
            selected_inspector,
        };
        let timeline = Timeline {
            bottom_timeline_present: true,
            compact: true,
            synced_selection: inspector.selected_detail.is_some(),
            spans_reported: true,
            order_strength_reported: true,
        };
        let checks = checks::build(&layout, &controls, &center, &inspector, &timeline);

        Self {
            schema_version: VERSION.to_owned(),
            layout,
            graph_identity,
            controls,
            center,
            inspector,
            timeline,
            checks,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Controls {
    pub run_selector_present: bool,
    pub mode_selector_present: bool,
    pub load_state: LoadState,
    pub run_error: Option<String>,
    pub quick_filters_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoadState {
    Loaded,
    Empty,
    Failed,
}

impl LoadState {
    fn from_app_state(graph_has_content: bool, run_error: Option<&str>) -> Self {
        if run_error.is_some() {
            Self::Failed
        } else if graph_has_content {
            Self::Loaded
        } else {
            Self::Empty
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTree {
    pub mode: String,
    pub canvas_present: bool,
    pub synthetic_anchors_visible: bool,
    pub visible_node_count: usize,
    pub visible_edge_count: usize,
    pub hidden_edge_count: usize,
    pub nodes: Nodes,
    pub edges: Edges,
    pub components: Components,
    pub component_breakdown: Vec<ComponentBreakdown>,
    pub marks: Marks,
}

impl ArtifactTree {
    fn from_diagnostics(
        diagnostics: &GraphViewDiagnostics,
        component_breakdown: Vec<ComponentBreakdown>,
    ) -> Self {
        let shape = &diagnostics.artifact_tree;
        Self {
            mode: diagnostics.mode.as_str().to_owned(),
            canvas_present: true,
            synthetic_anchors_visible: diagnostics.connectivity.synthetic_anchors_visible,
            visible_node_count: diagnostics.node_count,
            visible_edge_count: diagnostics.edge_count,
            hidden_edge_count: diagnostics.connectivity.hidden_edge_count,
            nodes: shape.nodes().into(),
            edges: shape.edges().into(),
            components: (shape.components(), shape.nodes()).into(),
            component_breakdown,
            marks: shape.marks().into(),
        }
    }
}

pub(crate) fn component_breakdown(graph: &ploke_tree::Graph) -> Vec<ComponentBreakdown> {
    graph
        .artifact_tree()
        .diagnostics
        .components
        .iter()
        .enumerate()
        .map(|(index, component)| ComponentBreakdown::from((index + 1, component)))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nodes {
    #[serde(rename = "F")]
    pub f: usize,
    #[serde(rename = "A")]
    pub a: usize,
}

impl From<tree::Nodes> for Nodes {
    fn from(value: tree::Nodes) -> Self {
        Self {
            f: value.run_forest,
            a: value.artifacts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edges {
    #[serde(rename = "E_F")]
    pub e_f: usize,
    #[serde(rename = "P_H")]
    pub p_h: usize,
    #[serde(rename = "P_C")]
    pub p_c: usize,
    #[serde(rename = "P_O")]
    pub p_o: usize,
    #[serde(rename = "P_B")]
    pub p_b: usize,
}

impl Edges {
    pub fn total(&self) -> usize {
        self.e_f + self.p_h + self.p_c + self.p_o + self.p_b
    }

    pub fn visible_primary_total(&self) -> usize {
        self.e_f + self.p_h + self.p_c
    }
}

impl From<tree::Edges> for Edges {
    fn from(value: tree::Edges) -> Self {
        Self {
            e_f: value.run_forest,
            p_h: value.history_patches,
            p_c: value.produced_child_edges,
            p_o: value.opened_from_edges,
            p_b: value.applied_patch_edges,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Components {
    pub weak: usize,
    pub roots: usize,
    pub orphan_artifacts: usize,
    pub weakly_connected: bool,
}

impl From<(tree::Components, tree::Nodes)> for Components {
    fn from((components, nodes): (tree::Components, tree::Nodes)) -> Self {
        Self {
            weak: components.weak,
            roots: components.roots,
            orphan_artifacts: components.orphan_artifacts,
            weakly_connected: components.weakly_connected(nodes.total()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marks {
    pub ruler_highlights: usize,
}

impl From<tree::Marks> for Marks {
    fn from(value: tree::Marks) -> Self {
        Self {
            ruler_highlights: value.ruler_highlights,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inspector<'a> {
    pub right_inspector_present: bool,
    pub selected_detail: Option<SelectionSnapshot<'a>>,
    pub selected_inspector: Option<SelectionInspectorSnapshot<'a>>,
    pub record_refs_present: bool,
    pub drilldown_candidates_present: bool,
    pub unavailable_reason_classified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timeline {
    pub bottom_timeline_present: bool,
    pub compact: bool,
    pub synced_selection: bool,
    pub spans_reported: bool,
    pub order_strength_reported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub id: String,
    pub status: CheckStatus,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    Passed,
    Failed,
    Missing,
    NotApplicable,
}
