use std::{
    collections::{BTreeMap, HashMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::Arc,
};

use eframe::egui::{Color32, Vec2};
use petgraph::{
    Directed,
    stable_graph::{NodeIndex, StableGraph},
    visit::{EdgeRef, IntoEdgeReferences},
};
use ploke_records::ids::ArtifactId;
use ploke_tree::Graph as DomainGraph;
use ploke_tree::graph::{
    ArtifactIdentity, ArtifactNode, EvidenceSubject, OperationKey, OperationTargetKey,
};

use super::artifact_tree;
use super::diagnostics::{graph_diagnostics, readability_diagnostics};
use super::edge::GraphEdgeShape;
use super::node::GraphNodeShape;
use super::style::{EdgeStyle, ViewStyle};
use super::{
    ArtifactTreeFilters, EdgeLabelDiagnostics, GraphConnectivityDiagnostics,
    GraphReadabilityDiagnostics, GraphSelectionDetail, GraphSelectionRef, GraphViewDiagnostics,
    GraphViewMode,
};

pub(super) type WidgetGraph = egui_graphs::Graph<
    GraphNode,
    GraphEdgePayload,
    Directed,
    petgraph::stable_graph::DefaultIx,
    GraphNodeShape,
    GraphEdgeShape,
>;
type RawGraph = StableGraph<GraphNode, GraphEdgePayload, Directed>;
type WidgetNode = egui_graphs::Node<
    GraphNode,
    GraphEdgePayload,
    Directed,
    petgraph::stable_graph::DefaultIx,
    GraphNodeShape,
>;

#[derive(Debug)]
pub(super) struct GraphViewCache {
    signature: Option<GraphSignature>,
    style: ViewStyle,
    mode: GraphViewMode,
    filters: ArtifactTreeFilters,
    graph: WidgetGraph,
    connectivity: GraphConnectivityDiagnostics,
    artifact_tree: artifact_tree::Shape,
    readability: ReadabilityCache,
}

impl Default for GraphViewCache {
    fn default() -> Self {
        Self {
            signature: None,
            style: ViewStyle::default(),
            mode: GraphViewMode::default(),
            filters: ArtifactTreeFilters::default(),
            graph: to_widget_graph(&RawGraph::default(), ViewStyle::default()),
            connectivity: GraphConnectivityDiagnostics::default(),
            artifact_tree: artifact_tree::Shape::default(),
            readability: ReadabilityCache::default(),
        }
    }
}

impl GraphViewCache {
    pub(super) fn refresh(
        &mut self,
        graph: &DomainGraph,
        style: ViewStyle,
        mode: GraphViewMode,
        filters: ArtifactTreeFilters,
    ) -> bool {
        profiling::scope!("ploke-egui.graph-view-cache.refresh");
        let signature = GraphSignature::from(graph);
        let projection_changed =
            self.signature != Some(signature) || self.style != style || self.filters != filters;
        let mode_changed = self.mode != mode;

        if !projection_changed && !mode_changed {
            return false;
        }

        if projection_changed {
            self.signature = Some(signature);
            self.style = style;
            self.filters = filters;
            let built = build_widget_graph(graph, style, mode, filters);
            self.graph = built.graph;
            self.connectivity = built.connectivity;
            self.artifact_tree = built.artifact_tree;
            self.readability.clear();
        }
        self.mode = mode;
        self.apply_visibility(mode);
        self.readability.clear();
        true
    }

    pub(super) fn graph_mut(&mut self) -> &mut WidgetGraph {
        &mut self.graph
    }

    pub(super) fn layout_state(&self, style: ViewStyle) -> super::layout::State {
        super::layout::State {
            triggered: false,
            row_dist: style.layout.row_distance,
            col_dist: style.layout.column_distance,
            lane_dist: style.layout.lane_distance,
            max_columns: style.layout.max_columns,
            visibility_filter_active: true,
            visible_nodes: self.visible_node_indices(),
            visible_edges: self.visible_edge_indices(),
        }
    }

    pub(super) fn diagnostics(
        &mut self,
        viewport_size: Vec2,
        style: ViewStyle,
        edge_labels: EdgeLabelDiagnostics,
    ) -> Option<GraphViewDiagnostics> {
        let readability = self.readability.get_or_compute(
            &self.graph,
            self.signature,
            style,
            self.mode,
            self.filters,
        );
        graph_diagnostics(
            &self.graph,
            viewport_size,
            style,
            edge_labels,
            self.connectivity,
            self.artifact_tree,
            self.mode,
            readability,
        )
    }

    pub(super) fn selected_reference(&self) -> Option<&GraphSelectionRef> {
        self.selected_payload().map(GraphNode::reference)
    }

    pub(super) fn selected_label(&self) -> Option<&str> {
        self.selected_payload().map(GraphNode::label)
    }

    pub(super) fn selected_kind(&self) -> Option<&'static str> {
        self.selected_payload().map(GraphNode::kind_name)
    }

    pub(super) fn selected_node(&self) -> Option<(&GraphSelectionRef, &str, &'static str)> {
        let payload = self.selected_payload()?;
        Some((payload.reference(), payload.label(), payload.kind_name()))
    }

    pub(super) fn selected_node_detail(&self) -> Option<GraphSelectionDetail> {
        let payload = self.selected_payload()?;
        Some(GraphSelectionDetail {
            kind: payload.kind_name().to_owned(),
            label: payload.label().to_owned(),
            detail: payload.detail().to_owned(),
            reference: payload.reference().clone(),
        })
    }

    pub(super) fn select_reference(&mut self, reference: &GraphSelectionRef) -> bool {
        let target = self.graph.g().node_indices().find(|node| {
            self.graph.g().node_weight(*node).is_some_and(|weight| {
                let payload = weight.payload();
                payload.visible() && payload.reference() == reference
            })
        });

        let Some(target) = target else {
            return false;
        };

        let node_indices = self.graph.g().node_indices().collect::<Vec<_>>();
        for node in node_indices {
            if let Some(weight) = self.graph.g_mut().node_weight_mut(node) {
                weight.set_selected(node == target);
            }
        }
        let edge_indices = self.graph.g().edge_indices().collect::<Vec<_>>();
        for edge in edge_indices {
            if let Some(weight) = self.graph.g_mut().edge_weight_mut(edge) {
                weight.set_selected(false);
            }
        }
        self.graph.set_selected_nodes(vec![target]);
        self.graph.set_selected_edges(Vec::new());
        true
    }

    fn selected_payload(&self) -> Option<&GraphNode> {
        let selected = self.graph.selected_nodes().first().copied()?;
        let node = self.graph.g().node_weight(selected)?;
        let payload = node.payload();
        payload.visible().then_some(payload)
    }

    fn apply_visibility(&mut self, mode: GraphViewMode) {
        let mask = mode.layer_mask();
        for node in self.graph.g_mut().node_weights_mut() {
            let visible =
                node.payload().layers().contains_any(mask) && node.payload().filter_visible();
            node.payload_mut().set_visible(visible);
            if !visible {
                node.set_selected(false);
                node.set_hovered(false);
                node.set_dragged(false);
            }
        }

        let edge_visibility = self
            .graph
            .g()
            .edge_indices()
            .map(|edge| {
                let Some((source, target)) = self.graph.g().edge_endpoints(edge) else {
                    return (edge, false);
                };
                let Some(payload) = self.graph.g().edge_weight(edge).map(|edge| edge.payload())
                else {
                    return (edge, false);
                };
                let source_visible = self
                    .graph
                    .g()
                    .node_weight(source)
                    .is_some_and(|node| node.payload().visible());
                let target_visible = self
                    .graph
                    .g()
                    .node_weight(target)
                    .is_some_and(|node| node.payload().visible());
                (
                    edge,
                    payload.layers.contains_any(mask)
                        && payload.filter_visible
                        && source_visible
                        && target_visible,
                )
            })
            .collect::<Vec<_>>();
        for (edge, visible) in edge_visibility {
            if let Some(edge) = self.graph.g_mut().edge_weight_mut(edge) {
                edge.payload_mut().set_visible(visible);
                if !visible {
                    edge.set_selected(false);
                }
            }
        }

        self.graph.set_selected_nodes(
            self.graph
                .selected_nodes()
                .iter()
                .copied()
                .filter(|node| {
                    self.graph
                        .g()
                        .node_weight(*node)
                        .is_some_and(|node| node.payload().visible())
                })
                .collect(),
        );
        self.graph.set_selected_edges(
            self.graph
                .selected_edges()
                .iter()
                .copied()
                .filter(|edge| {
                    self.graph
                        .g()
                        .edge_weight(*edge)
                        .is_some_and(|edge| edge.payload().visible())
                })
                .collect(),
        );
    }

    fn visible_node_indices(&self) -> Vec<usize> {
        let mut nodes = self
            .graph
            .g()
            .node_indices()
            .filter(|node| {
                self.graph
                    .g()
                    .node_weight(*node)
                    .is_some_and(|node| node.payload().visible())
            })
            .map(|node| node.index())
            .collect::<Vec<_>>();
        nodes.sort_unstable();
        nodes
    }

    fn visible_edge_indices(&self) -> Vec<usize> {
        let mut edges = self
            .graph
            .g()
            .edge_indices()
            .filter(|edge| {
                self.graph
                    .g()
                    .edge_weight(*edge)
                    .is_some_and(|edge| edge.payload().visible())
            })
            .map(|edge| edge.index())
            .collect::<Vec<_>>();
        edges.sort_unstable();
        edges
    }

    #[cfg(test)]
    fn readability_rebuilds(&self) -> usize {
        self.readability.rebuilds
    }
}

#[derive(Debug, Default)]
struct ReadabilityCache {
    key: Option<ReadabilityKey>,
    value: GraphReadabilityDiagnostics,
    rebuilds: usize,
}

impl ReadabilityCache {
    fn clear(&mut self) {
        self.key = None;
    }

    fn get_or_compute(
        &mut self,
        graph: &WidgetGraph,
        signature: Option<GraphSignature>,
        style: ViewStyle,
        mode: GraphViewMode,
        filters: ArtifactTreeFilters,
    ) -> GraphReadabilityDiagnostics {
        let key = ReadabilityKey {
            signature,
            style,
            mode,
            filters,
            layout: readability_layout_fingerprint(graph),
        };
        if self.key.as_ref() != Some(&key) {
            self.value = readability_diagnostics(graph, style);
            self.key = Some(key);
            self.rebuilds += 1;
        }
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ReadabilityKey {
    signature: Option<GraphSignature>,
    style: ViewStyle,
    mode: GraphViewMode,
    filters: ArtifactTreeFilters,
    layout: u64,
}

fn readability_layout_fingerprint(graph: &WidgetGraph) -> u64 {
    let mut state = DefaultHasher::new();
    for node in graph.g().node_indices() {
        node.index().hash(&mut state);
        if let Some(weight) = graph.g().node_weight(node) {
            let payload = weight.payload();
            payload.visible().hash(&mut state);
            payload.filter_visible().hash(&mut state);
            let location = weight.location();
            location.x.to_bits().hash(&mut state);
            location.y.to_bits().hash(&mut state);
        }
    }
    for edge in graph.g().edge_references() {
        edge.source().index().hash(&mut state);
        edge.target().index().hash(&mut state);
        let payload = edge.weight().payload();
        payload.visible().hash(&mut state);
        payload.filter_visible.hash(&mut state);
        edge_kind_id(payload.kind).hash(&mut state);
        edge_pattern_id(payload.pattern).hash(&mut state);
        payload.color.to_array().hash(&mut state);
    }
    state.finish()
}

fn edge_kind_id(kind: ViewEdgeKind) -> u8 {
    match kind {
        ViewEdgeKind::ArtifactPatch => 0,
        #[cfg(test)]
        ViewEdgeKind::HistoryArtifact => 1,
    }
}

fn edge_pattern_id(pattern: EdgePattern) -> u8 {
    match pattern {
        EdgePattern::Solid => 0,
        EdgePattern::Dotted => 1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GraphSignature {
    fingerprint: u64,
    forest_nodes: usize,
    forest_roots: usize,
    lineages: usize,
    blocks: usize,
    entries: usize,
    artifacts: usize,
    candidates: usize,
    branches: usize,
    memberships: usize,
    selections: usize,
    runtimes: usize,
    operations: usize,
    child_plans: usize,
    evidence: usize,
    warnings: usize,
}

impl From<&DomainGraph> for GraphSignature {
    fn from(graph: &DomainGraph) -> Self {
        Self {
            fingerprint: graph_projection_fingerprint(graph),
            forest_nodes: graph.forest.as_ref().map_or(0, |forest| forest.nodes.len()),
            forest_roots: graph.forest.as_ref().map_or(0, |forest| forest.roots.len()),
            lineages: graph.history.lineages.len(),
            blocks: graph.history.blocks.len(),
            entries: graph.history.entries.len(),
            artifacts: graph.artifacts.artifacts.len(),
            candidates: graph.candidates.candidates.len(),
            branches: graph.candidates.branches.len(),
            memberships: graph.candidates.memberships.len(),
            selections: graph.selections.selections.len(),
            runtimes: graph.runtimes.runtimes.len(),
            operations: graph.operations.operations.len(),
            child_plans: graph.child_plans.plans.len(),
            evidence: graph.evidence.attachments.len(),
            warnings: graph.warnings.len(),
        }
    }
}

fn graph_projection_fingerprint(graph: &DomainGraph) -> u64 {
    let mut state = DefaultHasher::new();

    if let Some(forest) = graph
        .forest
        .as_ref()
        .filter(|forest| !forest.nodes.is_empty())
    {
        "run-forest".hash(&mut state);
        let mut nodes = forest.nodes.iter().collect::<Vec<_>>();
        nodes.sort_by(|left, right| {
            (left.generation, left.key.as_str()).cmp(&(right.generation, right.key.as_str()))
        });
        for node in nodes {
            hash_run_forest_node(node, &mut state);
        }
        let mut roots = forest
            .roots
            .iter()
            .map(|root| root.as_str())
            .collect::<Vec<_>>();
        roots.sort_unstable();
        roots.hash(&mut state);
    } else {
        "artifact-tree".hash(&mut state);
        for artifact in graph.artifacts.artifacts.values() {
            hash_artifact_node(artifact, &mut state);
        }
        for lineage in graph.history.lineages.values() {
            lineage.lineage_id.hash(&mut state);
            lineage.blocks.hash(&mut state);
        }
        for block in graph.history.blocks.values() {
            block.block_hash.hash(&mut state);
            block.lineage_id.hash(&mut state);
            block.block_height.hash(&mut state);
            block.active_artifact.id().0.hash(&mut state);
            block.selected_successor.artifact.id().0.hash(&mut state);
        }
        for branch in &graph.candidates.branches {
            branch.selection_entry_id.hash(&mut state);
            branch.payload_index.hash(&mut state);
            branch.branch_id.hash(&mut state);
            branch.base_artifact_id.hash(&mut state);
            branch.derived_artifact_id.hash(&mut state);
            branch.patch_id.hash(&mut state);
        }
    }

    for warning in &graph.warnings {
        graph_warning_kind_id(warning.kind).hash(&mut state);
        warning.detail.hash(&mut state);
    }

    state.finish()
}

fn hash_run_forest_node(node: &ploke_tree::TreeNode, state: &mut DefaultHasher) {
    node.key.as_str().hash(state);
    node.parent
        .as_ref()
        .map(|parent| parent.as_str())
        .hash(state);
    node.children.len().hash(state);
    for child in &node.children {
        child.as_str().hash(state);
    }
    node.generation.hash(state);
    node.branch_id.hash(state);
    node.parent_branch_id.hash(state);
    node.candidate_id.hash(state);
    node.instance_id.hash(state);
    node.source_state_id.hash(state);
    node.target_relpath.hash(state);
    node.base_artifact_id.hash(state);
    node.patch_id.hash(state);
    node.derived_artifact_id.hash(state);
    progress_id(node.progress.phase).hash(state);
    terminality_id(node.progress.terminality).hash(state);
    result_class_id(node.progress.result_class).hash(state);
    node.created_at.hash(state);
    node.updated_at.hash(state);
    for evidence in &node.evidence {
        evidence_kind_id(evidence.kind).hash(state);
        authority_id(evidence.authority).hash(state);
        evidence
            .node_key
            .as_ref()
            .map(|key| key.as_str())
            .hash(state);
        evidence.runtime_id.hash(state);
        evidence.recorded_at.hash(state);
        evidence.detail.hash(state);
    }
    for diagnostic in &node.diagnostics {
        diagnostic_severity_id(diagnostic.severity).hash(state);
        diagnostic.code.hash(state);
        diagnostic.message.hash(state);
        diagnostic
            .node_key
            .as_ref()
            .map(|key| key.as_str())
            .hash(state);
    }
}

fn hash_artifact_node(node: &ArtifactNode, state: &mut DefaultHasher) {
    node.key.hash(state);
    match &node.identity {
        ArtifactIdentity::HistoryRef(history_ref) => {
            "history-ref".hash(state);
            history_ref.id().0.hash(state);
        }
        ArtifactIdentity::PassiveId(artifact_id) => {
            "passive-id".hash(state);
            artifact_id.hash(state);
        }
    }
    node.evidence.hash(state);
}

fn progress_id(phase: ploke_tree::Phase) -> u8 {
    match phase {
        ploke_tree::Phase::Planned => 0,
        ploke_tree::Phase::WorkspaceStaged => 1,
        ploke_tree::Phase::BinaryBuilt => 2,
        ploke_tree::Phase::Running => 3,
        ploke_tree::Phase::Completed => 4,
        ploke_tree::Phase::Failed => 5,
        ploke_tree::Phase::Unknown => 6,
    }
}

fn terminality_id(terminality: ploke_tree::Terminality) -> u8 {
    match terminality {
        ploke_tree::Terminality::NonTerminal => 0,
        ploke_tree::Terminality::Terminal => 1,
        ploke_tree::Terminality::Unknown => 2,
    }
}

fn result_class_id(result_class: ploke_tree::ResultClass) -> u8 {
    match result_class {
        ploke_tree::ResultClass::Success => 0,
        ploke_tree::ResultClass::Failure => 1,
        ploke_tree::ResultClass::Unknown => 2,
    }
}

fn evidence_kind_id(kind: ploke_tree::EvidenceKind) -> u8 {
    match kind {
        ploke_tree::EvidenceKind::SchedulerNode => 0,
        ploke_tree::EvidenceKind::ParentIdentity => 1,
        ploke_tree::EvidenceKind::SuccessorReady => 2,
        ploke_tree::EvidenceKind::SuccessorCompletion => 3,
    }
}

fn authority_id(authority: ploke_tree::AuthorityLabel) -> u8 {
    match authority {
        ploke_tree::AuthorityLabel::MutableProjection => 0,
        ploke_tree::AuthorityLabel::TypedRecordEvidence => 1,
        ploke_tree::AuthorityLabel::LiveTransport => 2,
        ploke_tree::AuthorityLabel::SealedVerifiedHistory => 3,
        ploke_tree::AuthorityLabel::DegradedObservation => 4,
    }
}

fn diagnostic_severity_id(severity: ploke_tree::DiagnosticSeverity) -> u8 {
    match severity {
        ploke_tree::DiagnosticSeverity::Info => 0,
        ploke_tree::DiagnosticSeverity::Warning => 1,
        ploke_tree::DiagnosticSeverity::Error => 2,
    }
}

fn graph_warning_kind_id(kind: ploke_tree::graph::GraphWarningKind) -> u8 {
    match kind {
        ploke_tree::graph::GraphWarningKind::DuplicateBlockHash => 0,
        ploke_tree::graph::GraphWarningKind::DuplicateEntryId => 1,
        ploke_tree::graph::GraphWarningKind::DuplicateLineageBlockHeight => 2,
        ploke_tree::graph::GraphWarningKind::BlockEntryCountMismatch => 3,
        ploke_tree::graph::GraphWarningKind::DuplicateCandidateMembershipId => 4,
        ploke_tree::graph::GraphWarningKind::CandidateSetMembershipCountMismatch => 5,
        ploke_tree::graph::GraphWarningKind::CandidateSetMembershipMissingForPayload => 6,
        ploke_tree::graph::GraphWarningKind::CandidateSetMembershipAmbiguousForPayload => 7,
        ploke_tree::graph::GraphWarningKind::SelectedMembershipMissing => 8,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GraphLayerMask(u8);

impl GraphLayerMask {
    pub(super) const EMPTY: Self = Self(0);
    pub(super) const ARTIFACT: Self = Self(1 << 0);
    pub(super) const LINEAGE: Self = Self(1 << 1);

    fn contains_any(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

impl std::ops::BitOr for GraphLayerMask {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl GraphViewMode {
    fn layer_mask(self) -> GraphLayerMask {
        match self {
            Self::ArtifactTree => GraphLayerMask::ARTIFACT,
            Self::Lineage => GraphLayerMask::LINEAGE,
            Self::ArtifactAndLineage => GraphLayerMask::ARTIFACT | GraphLayerMask::LINEAGE,
            Self::Empty => GraphLayerMask::EMPTY,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum GraphNode {
    // egui_graphs owns widget payloads. Labels are compact render-only text;
    // full raw identifiers stay in detail text for selection/debug views.
    Artifact {
        label: Arc<str>,
        detail: Arc<str>,
        reference: GraphSelectionRef,
        color: Color32,
        layers: GraphLayerMask,
        filter_visible: bool,
        visible: bool,
    },
}

impl GraphNode {
    fn kind_name(&self) -> &'static str {
        match self {
            Self::Artifact { .. } => "artifact",
        }
    }

    fn label(&self) -> &str {
        match self {
            Self::Artifact { label, .. } => label,
        }
    }

    fn detail(&self) -> &str {
        match self {
            Self::Artifact { detail, .. } => detail,
        }
    }

    fn reference(&self) -> &GraphSelectionRef {
        match self {
            Self::Artifact { reference, .. } => reference,
        }
    }

    fn color(&self) -> Color32 {
        match self {
            Self::Artifact { color, .. } => *color,
        }
    }

    pub(super) fn visible(&self) -> bool {
        match self {
            Self::Artifact { visible, .. } => *visible,
        }
    }

    fn filter_visible(&self) -> bool {
        match self {
            Self::Artifact { filter_visible, .. } => *filter_visible,
        }
    }

    fn set_visible(&mut self, visible: bool) {
        match self {
            Self::Artifact { visible: slot, .. } => *slot = visible,
        }
    }

    fn layers(&self) -> GraphLayerMask {
        match self {
            Self::Artifact { layers, .. } => *layers,
        }
    }

    fn add_layer(&mut self, layer: GraphLayerMask) {
        match self {
            Self::Artifact { layers, .. } => layers.insert(layer),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct GraphEdgePayload {
    // Edge payloads are owned by egui_graphs; labels/colors/kinds here are
    // render-only and must not be used as semantic graph authority.
    pub(super) label: Arc<str>,
    pub(super) label_visible: bool,
    pub(super) color: Color32,
    pub(super) style: EdgeStyle,
    pub(super) kind: ViewEdgeKind,
    pub(super) pattern: EdgePattern,
    pub(super) layers: GraphLayerMask,
    pub(super) filter_visible: bool,
    pub(super) visible: bool,
}

impl GraphEdgePayload {
    pub(super) fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EdgePattern {
    Solid,
    Dotted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ViewEdgeKind {
    ArtifactPatch,
    #[cfg(test)]
    HistoryArtifact,
}

struct BuiltWidgetGraph {
    graph: WidgetGraph,
    connectivity: GraphConnectivityDiagnostics,
    artifact_tree: artifact_tree::Shape,
}

#[derive(Debug)]
pub(super) struct ProjectedGraph {
    raw: RawGraph,
    connectivity: GraphConnectivityDiagnostics,
    artifact_tree: artifact_tree::Shape,
}

/// archaeology:artifact-relations
/// proof:docs/active/archaeology/ploke-tree-graph/artifact-relations.md
/// archaeology:artifact-child-consideration
/// proof:docs/active/archaeology/ploke-tree-graph/artifact-child-consideration.md
fn project_artifact_tree(
    graph: &DomainGraph,
    style: ViewStyle,
    filters: ArtifactTreeFilters,
) -> ProjectedGraph {
    profiling::scope!("ploke-egui.project-artifact-tree");
    let mut raw = RawGraph::default();
    let mut artifact_nodes = BTreeMap::new();
    let mut artifact_lookup = HashMap::new();
    let tree = graph.artifact_tree();
    let selected_ruler = tree.marks.selected_ruler;
    let mut ruler_highlights = HashSet::new();
    let lineage_refs = tree
        .marks
        .lineage_artifacts
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let unconsidered_children = tree
        .marks
        .unconsidered_children
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let unconsidered_child_edges = tree
        .marks
        .unconsidered_child_edges
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let lineage_edges = tree
        .marks
        .lineage_edges
        .iter()
        .copied()
        .collect::<HashSet<_>>();

    for tree_node in tree.nodes.values() {
        let Some(artifact) = tree_node.sources.first().copied() else {
            continue;
        };
        let dimmed = unconsidered_children.contains(&tree_node.key);
        let node = add_artifact_tree_node(
            &mut raw,
            &mut artifact_nodes,
            tree_node.key,
            artifact,
            style,
            dimmed,
            !filters.hide_unconsidered_children || !dimmed,
        );
        if selected_ruler == Some(tree_node.key) {
            set_artifact_tree_node_color(&mut raw, node, style.edge.colors.selected);
            ruler_highlights.insert(node);
        }
        if lineage_refs.contains(&tree_node.key) {
            raw[node].add_layer(GraphLayerMask::LINEAGE);
        }
        artifact_lookup.entry(tree_node.key).or_insert(node);
    }

    let mut patch_index = 1;
    let mut history_edges = tree.history_successors.iter().collect::<Vec<_>>();
    history_edges.sort_by_key(|edge| {
        edge.sources
            .iter()
            .map(|source| source.block_height)
            .min()
            .unwrap_or_default()
    });
    for edge in history_edges {
        let (Some(parent), Some(child)) = (
            artifact_lookup.get(&edge.from).copied(),
            artifact_lookup.get(&edge.to).copied(),
        ) else {
            continue;
        };
        if parent == child {
            continue;
        }
        let layers = if lineage_edges.contains(&(edge.from, edge.to)) {
            GraphLayerMask::ARTIFACT | GraphLayerMask::LINEAGE
        } else {
            GraphLayerMask::ARTIFACT
        };
        if add_unique_edge_with_layers(
            &mut raw,
            parent,
            child,
            format!("P{patch_index}"),
            true,
            ViewEdgeKind::ArtifactPatch,
            EdgePattern::Solid,
            style,
            layers,
            true,
        ) {
            patch_index += 1;
        }
    }

    let mut produced_children = tree.produced_child_edges.iter().collect::<Vec<_>>();
    produced_children.sort_by(|left, right| {
        let left_source = left.sources.first();
        let right_source = right.sources.first();
        (
            left_source.map(|source| source.node.node_id.as_str()),
            left_source
                .and_then(|source| source.node.parent_node_id.as_ref().map(|id| id.as_str())),
            left_source.map(|source| source.resolved.branch.branch_id.as_str()),
            left_source.and_then(|source| source.node.derived_artifact_id.as_ref()),
        )
            .cmp(&(
                right_source.map(|source| source.node.node_id.as_str()),
                right_source
                    .and_then(|source| source.node.parent_node_id.as_ref().map(|id| id.as_str())),
                right_source.map(|source| source.resolved.branch.branch_id.as_str()),
                right_source.and_then(|source| source.node.derived_artifact_id.as_ref()),
            ))
    });
    for edge in produced_children {
        let (Some(base), Some(derived)) = (
            artifact_lookup.get(&edge.from).copied(),
            artifact_lookup.get(&edge.to).copied(),
        ) else {
            continue;
        };
        if base == derived {
            continue;
        }
        let dotted = unconsidered_child_edges.contains(&(edge.from, edge.to));
        if add_unique_edge_with_layers(
            &mut raw,
            base,
            derived,
            format!("P{patch_index}"),
            true,
            ViewEdgeKind::ArtifactPatch,
            if dotted {
                EdgePattern::Dotted
            } else {
                EdgePattern::Solid
            },
            style,
            GraphLayerMask::ARTIFACT,
            !filters.hide_unconsidered_children || !dotted,
        ) {
            patch_index += 1;
        }
    }

    let hidden_record_count = full_debug_record_count(graph).saturating_sub(raw.node_count());
    let hidden_edge_count = full_debug_edge_count(graph).saturating_sub(raw.edge_count());
    let connectivity = GraphConnectivityDiagnostics {
        component_count_before_anchoring: tree.diagnostics.weak_component_count,
        hidden_record_count,
        hidden_edge_count,
        hidden_evidence_count: graph.evidence.attachments.len(),
        hidden_operation_count: graph.operations.operations.len(),
        hidden_unattached_component_count: tree.diagnostics.weak_component_count.saturating_sub(1),
        synthetic_anchors_visible: false,
    };
    let artifact_tree = artifact_tree::Shape::new(
        artifact_tree::Nodes::new(tree.nodes.len()),
        artifact_tree::Edges::new(
            tree.history_successors.len(),
            tree.produced_child_edges.len(),
            tree.opened_from_edges.len(),
            tree.applied_patch_edges.len(),
        ),
        artifact_tree::Components::new(
            tree.diagnostics.weak_component_count,
            tree.diagnostics.roots.len(),
            tree.diagnostics.orphan_artifacts.len(),
        ),
        artifact_tree::Marks::new(
            ruler_highlights.len(),
            tree.marks.unconsidered_children.len(),
            tree.marks.unconsidered_child_edges.len(),
        ),
    );

    ProjectedGraph {
        raw,
        connectivity,
        artifact_tree,
    }
}

fn full_debug_record_count(graph: &DomainGraph) -> usize {
    1 + graph.history.lineages.len()
        + graph.history.blocks.len()
        + graph.history.entries.len()
        + graph.artifacts.artifacts.len()
        + graph.runtimes.runtimes.len()
        + graph.selections.selections.len()
        + graph.candidates.candidates.len()
        + graph.candidates.memberships.len()
        + graph.candidates.branches.len()
        + graph.operations.operations.len()
        + graph.evidence.attachments.len()
        + graph.warnings.len()
}

fn full_debug_edge_count(graph: &DomainGraph) -> usize {
    let mut count = graph.history.lineages.len();

    for block in graph.history.blocks.values() {
        if graph.history.lineages.contains_key(&block.lineage_id) {
            count += 1;
        }
        count += block
            .parent_block_hashes
            .iter()
            .filter(|hash| graph.history.blocks.contains_key(*hash))
            .count();
        if has_history_artifact(graph, block.active_artifact.id().0.as_str()) {
            count += 1;
        }
        if has_history_artifact(graph, block.selected_successor.artifact.id().0.as_str()) {
            count += 1;
        }
        if let ploke_records::history::ActorRefRecord::Runtime(runtime_id) =
            &block.selected_successor.runtime
        {
            if graph.runtimes.runtimes.contains_key(runtime_id) {
                count += 1;
            }
        }
    }

    count += graph
        .history
        .entries
        .values()
        .filter(|entry| graph.history.blocks.contains_key(&entry.block_hash))
        .count();

    count += graph
        .selections
        .selections
        .values()
        .filter(|selection| graph.history.entries.contains_key(&selection.entry_id))
        .count();

    for candidate in &graph.candidates.candidates {
        if graph
            .selections
            .selections
            .contains_key(&candidate.selection_entry_id)
        {
            count += 1;
        }
        if candidate
            .artifact_after
            .as_ref()
            .is_some_and(|artifact| has_passive_artifact(graph, artifact))
        {
            count += 1;
        }
        if candidate
            .membership_key
            .as_ref()
            .is_some_and(|key| graph.candidates.memberships.contains_key(key))
        {
            count += 1;
        }
        if candidate
            .branch_id
            .as_deref()
            .is_some_and(|branch_id| has_branch(graph, branch_id))
        {
            count += 1;
        }
    }

    count += graph
        .candidates
        .memberships
        .values()
        .filter(|membership| {
            graph
                .selections
                .selections
                .contains_key(&membership.selection_entry_id)
        })
        .count();

    for branch in &graph.candidates.branches {
        if graph
            .selections
            .selections
            .contains_key(&branch.selection_entry_id)
        {
            count += 1;
        }
        if branch
            .base_artifact_id
            .as_ref()
            .is_some_and(|artifact| has_passive_artifact(graph, artifact))
        {
            count += 1;
        }
        if branch
            .derived_artifact_id
            .as_ref()
            .is_some_and(|artifact| has_passive_artifact(graph, artifact))
        {
            count += 1;
        }
    }

    for operation in graph.operations.operations.values() {
        match &operation.key {
            OperationKey::HistoryEntry { entry_id } => {
                if graph.history.entries.contains_key(entry_id) {
                    count += 1;
                }
            }
            OperationKey::RuntimeTarget { runtime_id, target } => {
                if graph.runtimes.runtimes.contains_key(runtime_id) {
                    count += 1;
                }
                count += operation_target_edge_count(graph, target);
            }
        }
    }

    count += graph
        .evidence
        .attachments
        .values()
        .filter(|evidence| evidence_subject_has_loaded_node(graph, &evidence.subject))
        .count();
    count + graph.warnings.len()
}

fn operation_target_edge_count(graph: &DomainGraph, target: &OperationTargetKey) -> usize {
    match target {
        OperationTargetKey::Artifact { artifact_id } => {
            usize::from(has_passive_artifact(graph, artifact_id))
        }
        OperationTargetKey::PatchSet {
            base_artifact_id, ..
        } => usize::from(has_passive_artifact(graph, base_artifact_id)),
        OperationTargetKey::ArtifactSet {
            base_artifact_id,
            artifact_ids,
        } => {
            base_artifact_id.as_ref().map_or(0, |artifact| {
                usize::from(has_passive_artifact(graph, artifact))
            }) + artifact_ids
                .iter()
                .filter(|artifact| has_passive_artifact(graph, artifact))
                .count()
        }
    }
}

fn evidence_subject_has_loaded_node(graph: &DomainGraph, subject: &EvidenceSubject) -> bool {
    match subject {
        EvidenceSubject::HistoryBlock(block_hash) => graph.history.blocks.contains_key(block_hash),
        EvidenceSubject::HistoryEntry(entry_id) => graph.history.entries.contains_key(entry_id),
        EvidenceSubject::Runtime(runtime_id) => graph.runtimes.runtimes.contains_key(runtime_id),
        EvidenceSubject::Artifact(artifact_id) => has_passive_artifact(graph, artifact_id),
        EvidenceSubject::Candidate {
            selection_entry_id,
            payload_index,
        } => graph.candidates.candidates.iter().any(|candidate| {
            &candidate.selection_entry_id == selection_entry_id
                && candidate.payload_index == *payload_index
        }),
        EvidenceSubject::Branch(branch_id) => has_branch(graph, branch_id),
        EvidenceSubject::Selection(entry_id) => graph.selections.selections.contains_key(entry_id),
        EvidenceSubject::SchedulerCampaign(_)
        | EvidenceSubject::SchedulerNode(_)
        | EvidenceSubject::TransitionJournalSummary { .. }
        | EvidenceSubject::TransitionJournalLoadedEntries { .. }
        | EvidenceSubject::BranchRegistrySummary { .. }
        | EvidenceSubject::HistoryStorageSummary { .. }
        | EvidenceSubject::ChannelSummary { .. }
        | EvidenceSubject::EvaluationSummary { .. }
        | EvidenceSubject::ChildPlanSummary { .. }
        | EvidenceSubject::ProtocolArtifactSummary { .. }
        | EvidenceSubject::ProtocolArtifact { .. }
        | EvidenceSubject::RunRecordSummary { .. }
        | EvidenceSubject::RunRecord { .. }
        | EvidenceSubject::RunProfileSummary(_)
        | EvidenceSubject::RunProfileCommitment(_)
        | EvidenceSubject::AgentTurnEvidenceSummary { .. }
        | EvidenceSubject::AgentTurnArtifact(_) => false,
    }
}

fn has_history_artifact(graph: &DomainGraph, value: &str) -> bool {
    graph.artifacts.artifacts.values().any(|artifact| {
        matches!(
            &artifact.identity,
            ArtifactIdentity::HistoryRef(history_ref) if history_ref.id().0 == value
        )
    })
}

fn has_passive_artifact(graph: &DomainGraph, artifact_id: &ArtifactId) -> bool {
    graph.artifacts.artifacts.values().any(|artifact| {
        matches!(
            &artifact.identity,
            ArtifactIdentity::PassiveId(passive_id) if passive_id == artifact_id
        )
    })
}

fn has_branch(graph: &DomainGraph, branch_id: &str) -> bool {
    graph
        .candidates
        .branches
        .iter()
        .any(|branch| branch.branch_id == branch_id)
}

fn add_artifact_tree_node<'a>(
    raw: &mut RawGraph,
    artifact_nodes: &mut BTreeMap<ploke_tree::graph::artifact_tree::Key<'a>, NodeIndex>,
    key: ploke_tree::graph::artifact_tree::Key<'a>,
    artifact: &'a ArtifactNode,
    style: ViewStyle,
    dimmed: bool,
    filter_visible: bool,
) -> NodeIndex {
    if let Some(node) = artifact_nodes.get(&key).copied() {
        return node;
    }

    let color = if dimmed {
        style.edge.colors.synthesized.gamma_multiply(0.45)
    } else {
        style.edge.colors.synthesized
    };
    let node = raw.add_node(GraphNode::Artifact {
        label: Arc::from(artifact_handle_label(artifact_nodes.len() + 1)),
        detail: Arc::from(artifact.detail_text()),
        reference: GraphSelectionRef::Artifact {
            key: key.as_str().to_owned(),
        },
        color,
        layers: GraphLayerMask::ARTIFACT,
        filter_visible,
        visible: true,
    });
    artifact_nodes.insert(key, node);
    node
}

#[allow(irrefutable_let_patterns)]
fn set_artifact_tree_node_color(raw: &mut RawGraph, node: NodeIndex, color: Color32) {
    if let GraphNode::Artifact {
        color: node_color, ..
    } = &mut raw[node]
    {
        *node_color = color;
    }
}

fn add_unique_edge_with_layers(
    raw: &mut RawGraph,
    source: NodeIndex,
    target: NodeIndex,
    label: impl Into<Arc<str>>,
    label_visible: bool,
    kind: ViewEdgeKind,
    pattern: EdgePattern,
    style: ViewStyle,
    layers: GraphLayerMask,
    filter_visible: bool,
) -> bool {
    if let Some(edge) = raw
        .edges_connecting(source, target)
        .find(|edge| edge.weight().kind == kind)
        .map(|edge| edge.id())
    {
        if let Some(payload) = raw.edge_weight_mut(edge) {
            payload.layers.insert(layers);
            payload.filter_visible &= filter_visible;
            if payload.pattern != EdgePattern::Dotted {
                payload.pattern = pattern;
            }
        }
        return false;
    }
    add_edge_with_layers(
        raw,
        source,
        target,
        label,
        label_visible,
        kind,
        pattern,
        style,
        layers,
        filter_visible,
    );
    true
}

fn build_widget_graph(
    graph: &DomainGraph,
    style: ViewStyle,
    _mode: GraphViewMode,
    filters: ArtifactTreeFilters,
) -> BuiltWidgetGraph {
    profiling::scope!("ploke-egui.build-widget-graph");
    let projected = project_artifact_tree(graph, style, filters);
    BuiltWidgetGraph {
        graph: to_widget_graph(&projected.raw, style),
        connectivity: projected.connectivity,
        artifact_tree: projected.artifact_tree,
    }
}

fn add_edge_with_layers(
    raw: &mut RawGraph,
    source: NodeIndex,
    target: NodeIndex,
    label: impl Into<Arc<str>>,
    label_visible: bool,
    kind: ViewEdgeKind,
    pattern: EdgePattern,
    style: ViewStyle,
    layers: GraphLayerMask,
    filter_visible: bool,
) {
    raw.add_edge(
        source,
        target,
        GraphEdgePayload {
            label: label.into(),
            label_visible,
            color: edge_color(kind, style),
            style: style.edge,
            kind,
            pattern,
            layers,
            filter_visible,
            visible: true,
        },
    );
}

fn edge_color(kind: ViewEdgeKind, style: ViewStyle) -> Color32 {
    match kind {
        ViewEdgeKind::ArtifactPatch => style.edge.colors.synthesized,
        #[cfg(test)]
        ViewEdgeKind::HistoryArtifact => style.edge.colors.selected,
    }
}

trait DetailText {
    fn detail_text(&self) -> String;
}

impl DetailText for ArtifactNode {
    fn detail_text(&self) -> String {
        if let Some(artifact_id) = self.artifact_ids().first() {
            return format!("artifact_id: {}", artifact_id.0);
        }
        if let Some(history_ref) = self.artifact_refs().first() {
            return format!("artifact history ref: {}", history_ref.as_str());
        }
        match &self.identity {
            ArtifactIdentity::HistoryRef(history_ref) => {
                format!("artifact history ref: {}", history_ref.as_str())
            }
            ArtifactIdentity::PassiveId(artifact_id) => format!("artifact_id: {}", artifact_id.0),
        }
    }
}

fn artifact_handle_label(index: usize) -> String {
    format!("A{index}")
}

fn to_widget_graph(raw: &RawGraph, style: ViewStyle) -> WidgetGraph {
    profiling::scope!("ploke-egui.to-widget-graph");
    egui_graphs::to_graph_custom(
        raw,
        |node: &mut WidgetNode| {
            let (label, color) = {
                let payload = node.payload();
                (payload.label().to_owned(), payload.color())
            };
            node.set_label(label);
            node.set_color(color);
            node.display_mut().set_radius(style.layout.node_radius);
        },
        |_edge| {},
    )
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use petgraph::stable_graph::NodeIndex;
    use ploke_records::branch::{
        ResolvedTreatmentBranch, TreatmentBranchNode, TreatmentBranchStatus,
    };
    use ploke_records::child_plan::{ChildPlanChildRecord, ChildPlanRecord};
    use ploke_records::history::{
        ActorRefRecord, ArtifactRefRecord, ProcedureRefRecord, SurfaceCommitmentRecord,
        SurfaceDeltaRecord, SurfaceRecord, SurfaceRootRecord,
    };
    use ploke_records::ids::{BlockHash, BlockId, HistoryHash, LineageId, RecordedAt, RuntimeId};
    use ploke_records::scheduler::{
        NodeRecord, NodeStatusRecord, RunnerRequestRecord, SCHEDULER_STATE_SCHEMA_V1,
        SchedulerStateRecord, SearchPolicyRecord, TREATMENT_NODE_SCHEMA_V1,
    };
    use ploke_tree::Graph;
    use ploke_tree::graph::{
        ArtifactIdentity, ArtifactIndex, ArtifactKey, ArtifactNode, AuthorityIndex, CandidateIndex,
        HistoryBlockNode, HistoryIndex, OpeningAuthorityNode, SelectionIndex, SuccessorNode,
    };

    use ploke_records::history::SubjectRefRecord;
    use ploke_records::ids::{
        ArtifactId, BranchId, CampaignId, CandidateId, EntryId, InstanceId, PatchId,
        SchedulerNodeId, SourceStateId,
    };
    use ploke_tree::graph::{CandidateBranchNode, CandidateSource};
    use ploke_tree::{PassiveEvidence, RunForestInput, RunRecordSet, TransitionJournal};

    use super::{EdgePattern, GraphNode, GraphViewCache, project_artifact_tree};
    use crate::ui::view::{ArtifactTreeFilters, GraphViewMode, ViewStyle};

    #[test]
    fn artifact_tree_does_not_render_unattached_anchor_by_default() {
        let graph = graph_with_candidate_inventory_selection();
        let projected = project_artifact_tree(&graph, ViewStyle::default(), filters());

        assert_eq!(projected.connectivity.component_count_before_anchoring, 0);
        assert_eq!(count_nodes(&projected, "unattached"), 0);
        assert!(!projected.connectivity.synthetic_anchors_visible);
        assert!(projected.connectivity.hidden_record_count > 0);
    }

    #[test]
    fn artifact_tree_hides_candidate_inventory_by_default() {
        let mut graph = graph_with_candidate_inventory_selection();
        let entry_id = EntryId("entry-1".to_owned());
        for index in 2..64 {
            graph
                .candidates
                .candidates
                .push(ploke_tree::graph::CandidateNode {
                    selection_entry_id: entry_id.clone(),
                    payload_index: index,
                    subject: SubjectRefRecord {
                        value: format!("candidate:{index}"),
                    },
                    source: None,
                    occurrence_id: None,
                    membership_id: None,
                    membership_key: None,
                    node_id: Some(format!("node-{index}")),
                    branch_id: None,
                    generation: Some(1),
                    primary_runtime_id: None,
                    artifact_after: None,
                    patch_id: None,
                    evidence: Vec::new(),
                });
        }

        let projected = project_artifact_tree(&graph, ViewStyle::default(), filters());

        assert_eq!(projected.raw.node_count(), 0);
        assert_eq!(projected.raw.edge_count(), 0);
        assert_eq!(count_nodes(&projected, "candidate"), 0);
        assert_eq!(count_nodes(&projected, "selection"), 0);
    }

    #[test]
    fn artifact_tree_stays_artifact_first_when_scheduler_records_are_present() {
        let mut graph = graph_with_selected_successor("artifact:parent", "artifact:child", 3);
        graph.forest = Graph::from_records(&run_records_with_parent_and_children()).forest;

        let projected = project_artifact_tree(&graph, ViewStyle::default(), filters());

        assert_eq!(projected.artifact_tree.nodes().run_forest, 0);
        assert!(projected.artifact_tree.nodes().artifacts > 0);
        assert_eq!(
            count_nodes(&projected, "artifact"),
            projected.raw.node_count()
        );
        assert_eq!(projected.artifact_tree.edges().run_forest, 0);
    }

    #[test]
    fn artifact_tree_keeps_debug_record_classes_hidden() {
        let graph = graph_with_artifact_branch_selection();
        let projected = project_artifact_tree(&graph, ViewStyle::default(), filters());

        assert_eq!(count_nodes(&projected, "artifact"), 2);
        assert_eq!(projected.raw.edge_count(), 1);
        assert_eq!(projected.artifact_tree.nodes().artifacts, 2);
        assert_eq!(projected.artifact_tree.edges().history_patches, 0);
        assert_eq!(projected.artifact_tree.edges().produced_child_edges, 1);
        assert_eq!(projected.artifact_tree.edges().applied_patch_edges, 1);
        assert_eq!(projected.artifact_tree.edges().total(), 2);
        assert_eq!(count_nodes(&projected, "candidate"), 0);
        assert_eq!(count_nodes(&projected, "selection"), 0);
        assert_eq!(count_nodes(&projected, "branch"), 0);
        assert_eq!(count_nodes(&projected, "membership"), 0);
        assert_eq!(count_nodes(&projected, "operation"), 0);
        assert_eq!(count_nodes(&projected, "evidence"), 0);
        assert_eq!(count_nodes(&projected, "warning"), 0);
    }

    #[test]
    fn artifact_tree_reports_component_edge_sources() {
        let graph = graph_with_artifact_branch_selection();
        let tree = graph.artifact_tree();
        let components = &tree.diagnostics.components;

        assert_eq!(components.len(), 1);
        assert_eq!(
            components[0]
                .roots
                .iter()
                .map(|key| key.as_str())
                .collect::<Vec<_>>(),
            vec!["parent"]
        );
        assert_eq!(
            components[0]
                .artifacts
                .iter()
                .map(|key| key.as_str())
                .collect::<Vec<_>>(),
            vec!["child", "parent"]
        );
        assert!(components[0].history_successors.is_empty());
        assert_eq!(components[0].produced_child_edges.len(), 1);
        assert_eq!(components[0].applied_patch_edges.len(), 1);

        let edge = &components[0].produced_child_edges[0];
        assert_eq!(edge.from.as_str(), "parent");
        assert_eq!(edge.to.as_str(), "child");
        assert_eq!(edge.sources[0].resolved.branch.branch_id, "branch-1");
        assert_eq!(edge.sources[0].node.node_id.as_str(), "node-1");
        assert_eq!(edge.sources[0].resolved.branch.candidate_id, "candidate-1");
    }

    #[test]
    fn artifact_tree_connects_history_artifacts_with_patch_edge() {
        let graph = graph_with_selected_successor("artifact:parent", "artifact:child", 3);
        let projected = project_artifact_tree(&graph, ViewStyle::default(), filters());

        assert_eq!(count_nodes(&projected, "artifact"), 2);
        assert_eq!(projected.raw.edge_count(), 1);
        assert_eq!(projected.artifact_tree.nodes().artifacts, 2);
        assert_eq!(projected.artifact_tree.edges().history_patches, 1);
        assert_eq!(projected.artifact_tree.edges().opened_from_edges, 1);
        assert_eq!(projected.artifact_tree.edges().applied_patch_edges, 0);
        assert_eq!(projected.artifact_tree.components().weak, 1);
        assert_eq!(projected.artifact_tree.components().roots, 1);
        assert_eq!(projected.artifact_tree.components().orphan_artifacts, 0);
        assert!(projected.artifact_tree.components().weakly_connected(2));
        assert_eq!(projected.artifact_tree.marks().ruler_highlights, 1);
        let kinds = projected
            .raw
            .edge_weights()
            .map(|edge| edge.kind)
            .collect::<Vec<_>>();
        assert!(kinds.contains(&super::ViewEdgeKind::ArtifactPatch));
        assert_eq!(count_nodes(&projected, "history-block"), 0);
        assert_eq!(count_nodes(&projected, "lineage"), 0);
    }

    #[test]
    fn artifact_tree_resolves_history_refs_and_passive_ids_to_same_artifact_node() {
        let mut graph = graph_with_selected_successor("artifact:base", "artifact:base", 3);
        let base_id = ArtifactId("base".to_owned());
        let child_id = ArtifactId("child".to_owned());
        let entry_id = EntryId("entry-branch".to_owned());

        graph.artifacts.artifacts.insert(
            ArtifactKey::PassiveId {
                value: base_id.0.clone(),
            },
            ArtifactNode {
                key: ArtifactKey::PassiveId {
                    value: base_id.0.clone(),
                },
                identity: ArtifactIdentity::PassiveId(base_id.clone()),
                ids: ploke_tree::graph::ArtifactIds {
                    artifact_ids: vec![base_id.clone()],
                    ..Default::default()
                },
                evidence: Vec::new(),
            },
        );
        graph.artifacts.artifacts.insert(
            ArtifactKey::PassiveId {
                value: child_id.0.clone(),
            },
            ArtifactNode {
                key: ArtifactKey::PassiveId {
                    value: child_id.0.clone(),
                },
                identity: ArtifactIdentity::PassiveId(child_id.clone()),
                ids: ploke_tree::graph::ArtifactIds {
                    artifact_ids: vec![child_id.clone()],
                    ..Default::default()
                },
                evidence: Vec::new(),
            },
        );
        graph.candidates.branches.push(CandidateBranchNode {
            selection_entry_id: entry_id,
            payload_index: 0,
            branch_id: "branch-1".to_owned(),
            candidate_id: None,
            source_state_id: None,
            parent_branch_id: None,
            base_artifact_id: Some(base_id),
            derived_artifact_id: Some(child_id),
            patch_id: None,
            evidence: Vec::new(),
        });
        graph
            .candidates
            .candidates
            .push(ploke_tree::graph::CandidateNode {
                selection_entry_id: EntryId("entry-base".to_owned()),
                payload_index: 0,
                subject: SubjectRefRecord {
                    value: "candidate:base".to_owned(),
                },
                source: Some(CandidateSource::CurrentGeneration),
                occurrence_id: None,
                membership_id: None,
                membership_key: None,
                node_id: Some("node-base".to_owned()),
                branch_id: Some("branch-base".to_owned()),
                generation: Some(0),
                primary_runtime_id: None,
                artifact_after: Some(ArtifactId("base".to_owned())),
                patch_id: None,
                evidence: Vec::new(),
            });
        graph.child_plans.plans.insert(
            SchedulerNodeId("node-base".to_owned()),
            child_plan_record(
                "node-base",
                "node-child",
                "base",
                "child",
                "branch-1",
                "candidate-1",
                "patch-1",
            ),
        );

        let projected = project_artifact_tree(&graph, ViewStyle::default(), filters());

        assert_eq!(count_nodes(&projected, "artifact"), 2);
        assert_eq!(projected.raw.edge_count(), 1);
        assert_eq!(projected.artifact_tree.nodes().artifacts, 2);
        assert_eq!(projected.artifact_tree.edges().history_patches, 1);
        assert_eq!(projected.artifact_tree.edges().produced_child_edges, 1);
        assert_eq!(projected.artifact_tree.edges().applied_patch_edges, 1);
        assert_eq!(projected.artifact_tree.components().weak, 1);
        assert!(projected.artifact_tree.components().weakly_connected(2));
    }

    #[test]
    fn artifact_tree_edges_flow_from_parent_to_child() {
        let graph = graph_with_selected_successor("artifact:parent", "artifact:child", 3);
        let projected = project_artifact_tree(&graph, ViewStyle::default(), filters());

        let parent = artifact_node(&projected, "artifact:parent");
        let child = artifact_node(&projected, "artifact:child");

        assert!(
            projected
                .raw
                .edges_connecting(parent, child)
                .next()
                .is_some(),
            "artifact tree edges must flow parent -> child"
        );
        assert!(
            projected
                .raw
                .edges_connecting(child, parent)
                .next()
                .is_none(),
            "artifact tree must not reverse the git-tree direction"
        );
    }

    #[test]
    fn artifact_tree_highlights_selected_successor_as_next_ruler() {
        let style = ViewStyle::default();
        let graph = graph_with_selected_successor("artifact:parent", "artifact:child", 3);
        let projected = project_artifact_tree(&graph, style, filters());

        let parent = artifact_node(&projected, "artifact:parent");
        let child = artifact_node(&projected, "artifact:child");

        assert_eq!(
            node_color(&projected.raw[parent]),
            Some(style.edge.colors.synthesized)
        );
        assert_eq!(
            node_color(&projected.raw[child]),
            Some(style.edge.colors.selected)
        );
    }

    #[test]
    fn artifact_tree_dims_unconsidered_children_and_dots_their_edges() {
        let style = ViewStyle::default();
        let graph = graph_with_unconsidered_sibling();
        let projected = project_artifact_tree(&graph, style, filters());

        let parent = artifact_node(&projected, "artifact:parent");
        let selected = artifact_node(&projected, "artifact:selected");
        let sibling = artifact_node(&projected, "artifact:sibling");
        let selected_edge = projected
            .raw
            .edges_connecting(parent, selected)
            .next()
            .expect("selected child edge");
        let sibling_edge = projected
            .raw
            .edges_connecting(parent, sibling)
            .next()
            .expect("sibling child edge");

        assert_eq!(projected.artifact_tree.marks().dimmed_children, 1);
        assert_eq!(projected.artifact_tree.marks().dotted_child_edges, 1);
        assert_eq!(
            node_color(&projected.raw[sibling]),
            Some(style.edge.colors.synthesized.gamma_multiply(0.45))
        );
        assert_eq!(selected_edge.weight().pattern, EdgePattern::Solid);
        assert_eq!(sibling_edge.weight().pattern, EdgePattern::Dotted);
    }

    #[test]
    fn artifact_tree_filter_hides_unconsidered_children_from_visible_projection() {
        let style = ViewStyle::default();
        let graph = graph_with_unconsidered_sibling();
        let mut cache = GraphViewCache::default();

        assert!(cache.refresh(
            &graph,
            style,
            GraphViewMode::ArtifactTree,
            ArtifactTreeFilters {
                hide_unconsidered_children: true,
            },
        ));

        let sibling = cache
            .graph
            .g()
            .node_indices()
            .find(|node| {
                cache.graph.g()[*node]
                    .payload()
                    .detail()
                    .contains("artifact:sibling")
            })
            .expect("sibling node");
        let visible_nodes = cache
            .graph
            .g()
            .node_weights()
            .filter(|node| node.payload().visible())
            .count();
        let visible_edges = cache
            .graph
            .g()
            .edge_weights()
            .filter(|edge| edge.payload().visible())
            .count();

        assert!(!cache.graph.g()[sibling].payload().visible());
        assert_eq!(visible_nodes, 2);
        assert_eq!(visible_edges, 1);
    }

    #[test]
    fn primary_labels_are_short_handles() {
        let graph = materialized_artifact_graph_with_four_nodes();
        let projected = project_artifact_tree(&graph, ViewStyle::default(), filters());

        let labels = projected
            .raw
            .node_weights()
            .map(|node| node.label().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(labels, ["A1", "A2", "A3", "A4"]);
    }

    #[test]
    fn full_raw_ids_remain_in_node_detail_text() {
        let graph = materialized_artifact_graph_with_four_nodes();
        let projected = project_artifact_tree(&graph, ViewStyle::default(), filters());

        assert!(
            projected
                .raw
                .node_weights()
                .any(|node| { node.detail().contains("artifact_id: artifact:child-a") })
        );
    }

    #[test]
    fn cache_rebuilds_when_projection_identity_changes_without_count_change() {
        let style = ViewStyle::default();
        let mut cache = GraphViewCache::default();
        let first = graph_with_selected_successor("artifact:old-parent", "artifact:old-child", 3);
        let second = graph_with_selected_successor("artifact:new-parent", "artifact:new-child", 3);

        assert!(cache.refresh(
            &first,
            style,
            GraphViewMode::ArtifactTree,
            ArtifactTreeFilters::default(),
        ));
        assert!(
            cache
                .graph
                .g()
                .node_weights()
                .any(|node| { node.payload().detail().contains("artifact:old-child") })
        );

        assert!(cache.refresh(
            &second,
            style,
            GraphViewMode::ArtifactTree,
            ArtifactTreeFilters::default(),
        ));
        assert!(
            cache
                .graph
                .g()
                .node_weights()
                .any(|node| { node.payload().detail().contains("artifact:new-child") })
        );
        assert!(
            !cache
                .graph
                .g()
                .node_weights()
                .any(|node| { node.payload().detail().contains("artifact:old-child") })
        );
    }

    #[test]
    fn readability_diagnostics_reuse_cache_on_stable_frames() {
        let style = ViewStyle::default();
        let graph = graph_with_selected_successor("artifact:parent", "artifact:child", 3);
        let mut cache = GraphViewCache::default();

        assert!(cache.refresh(
            &graph,
            style,
            GraphViewMode::ArtifactTree,
            ArtifactTreeFilters::default(),
        ));
        assert_eq!(cache.readability_rebuilds(), 0);

        let viewport = eframe::egui::Vec2::new(800.0, 600.0);
        let edge_labels = crate::ui::view::EdgeLabelDiagnostics::default();
        assert!(cache.diagnostics(viewport, style, edge_labels).is_some());
        assert_eq!(cache.readability_rebuilds(), 1);
        assert!(cache.diagnostics(viewport, style, edge_labels).is_some());
        assert_eq!(cache.readability_rebuilds(), 1);

        let first = cache
            .graph
            .g()
            .node_indices()
            .next()
            .expect("projected node");
        cache.graph.g_mut()[first].set_location(eframe::egui::Pos2::new(20.0, 20.0));
        assert!(cache.diagnostics(viewport, style, edge_labels).is_some());
        assert_eq!(cache.readability_rebuilds(), 2);
    }

    fn count_nodes(projected: &super::ProjectedGraph, kind: &str) -> usize {
        projected
            .raw
            .node_weights()
            .filter(|node| node.kind_name() == kind)
            .count()
    }

    fn artifact_node(projected: &super::ProjectedGraph, artifact_ref: &str) -> NodeIndex {
        projected
            .raw
            .node_indices()
            .find(|node| {
                projected.raw[*node].kind_name() == "artifact"
                    && projected.raw[*node].detail().contains(artifact_ref)
            })
            .expect("artifact node should be projected")
    }

    fn node_color(node: &GraphNode) -> Option<eframe::egui::Color32> {
        match node {
            GraphNode::Artifact { color, .. } => Some(*color),
        }
    }

    fn filters() -> ArtifactTreeFilters {
        ArtifactTreeFilters::default()
    }

    fn run_records_with_parent_and_children() -> RunRecordSet {
        let root = scheduler_node("node-root", None, 0, "branch-root", "artifact:root");
        let child_a = scheduler_node(
            "node-child-a",
            Some("node-root"),
            1,
            "branch-a",
            "artifact:child-a",
        );
        let child_b = scheduler_node(
            "node-child-b",
            Some("node-root"),
            1,
            "branch-b",
            "artifact:child-b",
        );
        let child_c = scheduler_node(
            "node-child-c",
            Some("node-root"),
            1,
            "branch-c",
            "artifact:child-c",
        );
        let nodes = vec![root, child_a, child_b, child_c];

        RunRecordSet {
            forest_input: RunForestInput {
                scheduler: SchedulerStateRecord {
                    schema_version: SCHEDULER_STATE_SCHEMA_V1.to_owned(),
                    campaign_id: CampaignId("campaign:test".to_owned()),
                    updated_at: "2026-05-14T00:00:00Z".to_owned(),
                    policy: SearchPolicyRecord::default(),
                    frontier_node_ids: Vec::new(),
                    completed_node_ids: nodes.iter().map(|node| node.node_id.clone()).collect(),
                    failed_node_ids: Vec::new(),
                    last_continuation_decision: None,
                    nodes,
                },
                node_records: Vec::new(),
                parent_identity: None,
                successor_ready: Vec::new(),
                successor_completion: Vec::new(),
                passive_evidence: PassiveEvidence::default(),
            },
            history_blocks: Vec::new(),
            transition_journal: TransitionJournal::default(),
        }
    }

    fn scheduler_node(
        node_id: &str,
        parent_node_id: Option<&str>,
        generation: u32,
        branch_id: &str,
        derived_artifact_id: &str,
    ) -> NodeRecord {
        NodeRecord {
            schema_version: TREATMENT_NODE_SCHEMA_V1.to_owned(),
            node_id: SchedulerNodeId(node_id.to_owned()),
            parent_node_id: parent_node_id.map(|id| SchedulerNodeId(id.to_owned())),
            generation,
            instance_id: InstanceId(format!("instance:{node_id}")),
            source_state_id: SourceStateId(format!("source:{node_id}")),
            operation_target: None,
            base_artifact_id: Some(ArtifactId("artifact:root".to_owned())),
            patch_id: Some(PatchId(format!("patch:{node_id}"))),
            derived_artifact_id: Some(ArtifactId(derived_artifact_id.to_owned())),
            parent_branch_id: parent_node_id.map(|_| BranchId("branch-root".to_owned())),
            branch_id: BranchId(branch_id.to_owned()),
            candidate_id: CandidateId(format!("candidate:{node_id}")),
            target_relpath: PathBuf::from("crates/ploke-tui/src/demo.rs"),
            node_dir: PathBuf::from(format!("nodes/{node_id}")),
            workspace_root: PathBuf::from(format!("workspaces/{node_id}")),
            binary_path: PathBuf::from(format!("workspaces/{node_id}/target/debug/ploke-eval")),
            runner_request_path: PathBuf::from(format!("nodes/{node_id}/runner-request.json")),
            runner_result_path: PathBuf::from(format!("nodes/{node_id}/runner-result.json")),
            status: NodeStatusRecord::Succeeded,
            created_at: "2026-05-14T00:00:00Z".to_owned(),
            updated_at: "2026-05-14T00:01:00Z".to_owned(),
        }
    }

    fn graph_with_artifact_branch_selection() -> Graph {
        let mut graph = Graph::default();

        let parent_id = ArtifactId("artifact:parent".to_owned());
        let child_id = ArtifactId("artifact:child".to_owned());
        let entry_id = EntryId("entry-1".to_owned());
        let subject = SubjectRefRecord {
            value: "candidate:1".to_owned(),
        };

        graph.artifacts.artifacts.insert(
            ArtifactKey::PassiveId {
                value: parent_id.0.clone(),
            },
            ArtifactNode {
                key: ArtifactKey::PassiveId {
                    value: parent_id.0.clone(),
                },
                identity: ArtifactIdentity::PassiveId(parent_id.clone()),
                ids: ploke_tree::graph::ArtifactIds {
                    artifact_ids: vec![parent_id.clone()],
                    ..Default::default()
                },
                evidence: Vec::new(),
            },
        );
        graph.artifacts.artifacts.insert(
            ArtifactKey::PassiveId {
                value: child_id.0.clone(),
            },
            ArtifactNode {
                key: ArtifactKey::PassiveId {
                    value: child_id.0.clone(),
                },
                identity: ArtifactIdentity::PassiveId(child_id.clone()),
                ids: ploke_tree::graph::ArtifactIds {
                    artifact_ids: vec![child_id.clone()],
                    ..Default::default()
                },
                evidence: Vec::new(),
            },
        );

        graph.candidates.branches.push(CandidateBranchNode {
            selection_entry_id: entry_id.clone(),
            payload_index: 0,
            branch_id: "branch-1".to_owned(),
            candidate_id: Some(CandidateId("candidate-1".to_owned())),
            source_state_id: Some("source-1".to_owned()),
            parent_branch_id: Some("branch-0".to_owned()),
            base_artifact_id: Some(parent_id.clone()),
            derived_artifact_id: Some(child_id.clone()),
            patch_id: None,
            evidence: Vec::new(),
        });
        graph
            .candidates
            .candidates
            .push(ploke_tree::graph::CandidateNode {
                selection_entry_id: EntryId("entry-parent".to_owned()),
                payload_index: 0,
                subject: SubjectRefRecord {
                    value: "candidate:parent".to_owned(),
                },
                source: Some(CandidateSource::CurrentGeneration),
                occurrence_id: None,
                membership_id: None,
                membership_key: None,
                node_id: Some("node-parent".to_owned()),
                branch_id: Some("branch-0".to_owned()),
                generation: Some(0),
                primary_runtime_id: None,
                artifact_after: Some(parent_id.clone()),
                patch_id: None,
                evidence: Vec::new(),
            });
        graph
            .candidates
            .candidates
            .push(ploke_tree::graph::CandidateNode {
                selection_entry_id: entry_id.clone(),
                payload_index: 0,
                subject: subject.clone(),
                source: Some(CandidateSource::CurrentGeneration),
                occurrence_id: None,
                membership_id: None,
                membership_key: None,
                node_id: Some("node-1".to_owned()),
                branch_id: Some("branch-1".to_owned()),
                generation: Some(1),
                primary_runtime_id: None,
                artifact_after: Some(child_id.clone()),
                patch_id: None,
                evidence: Vec::new(),
            });
        graph
            .candidates
            .candidates
            .push(ploke_tree::graph::CandidateNode {
                selection_entry_id: EntryId("entry-selected".to_owned()),
                payload_index: 0,
                subject: SubjectRefRecord {
                    value: "candidate:selected".to_owned(),
                },
                source: Some(CandidateSource::CurrentGeneration),
                occurrence_id: None,
                membership_id: None,
                membership_key: None,
                node_id: Some("node-selected".to_owned()),
                branch_id: Some("branch-selected".to_owned()),
                generation: Some(1),
                primary_runtime_id: None,
                artifact_after: Some(ArtifactId("artifact:selected".to_owned())),
                patch_id: Some(PatchId("patch-selected".to_owned())),
                evidence: Vec::new(),
            });
        graph.child_plans.plans.insert(
            SchedulerNodeId("node-parent".to_owned()),
            child_plan_record(
                "node-parent",
                "node-1",
                parent_id.0.as_str(),
                child_id.0.as_str(),
                "branch-1",
                "candidate-1",
                "patch-1",
            ),
        );
        graph.selections.selections.insert(
            entry_id.clone(),
            ploke_tree::graph::SelectionNode {
                entry_id,
                procedure_or_policy: ProcedureRefRecord {
                    value: "policy:test".to_owned(),
                },
                scope: ploke_records::history::SelectionScopeRecord {
                    value: "scope:test".to_owned(),
                },
                selected_candidate: Some(subject),
                selected_occurrence_id: None,
                selected_membership_id: None,
                candidate_set_root: None,
                considered_count: 1,
                projection_failure_count: 0,
                decision_outcome: ploke_records::selection::Outcome::Accepted,
            },
        );

        graph
    }

    fn graph_with_candidate_inventory_selection() -> Graph {
        let mut graph = Graph::default();
        let entry_id = EntryId("entry-1".to_owned());
        let subject = SubjectRefRecord {
            value: "candidate:duplicate".to_owned(),
        };
        let root = ploke_records::history::CandidateSetRootRecord(ploke_records::ids::HistoryHash(
            "root:selected".to_owned(),
        ));
        let selected_membership_id =
            ploke_records::ids::CandidateMembershipId("membership:selected".to_owned());
        let other_membership_id =
            ploke_records::ids::CandidateMembershipId("membership:other".to_owned());

        graph
            .candidates
            .candidates
            .push(ploke_tree::graph::CandidateNode {
                selection_entry_id: entry_id.clone(),
                payload_index: 0,
                subject: subject.clone(),
                source: None,
                occurrence_id: None,
                membership_id: Some(other_membership_id.clone()),
                membership_key: Some(ploke_tree::graph::CandidateMembershipKey {
                    candidate_set_root: root.clone(),
                    membership_id: other_membership_id,
                }),
                node_id: Some("node-other".to_owned()),
                branch_id: None,
                generation: Some(1),
                primary_runtime_id: None,
                artifact_after: None,
                patch_id: None,
                evidence: Vec::new(),
            });
        graph
            .candidates
            .candidates
            .push(ploke_tree::graph::CandidateNode {
                selection_entry_id: entry_id.clone(),
                payload_index: 1,
                subject: subject.clone(),
                source: None,
                occurrence_id: None,
                membership_id: Some(selected_membership_id.clone()),
                membership_key: Some(ploke_tree::graph::CandidateMembershipKey {
                    candidate_set_root: root.clone(),
                    membership_id: selected_membership_id.clone(),
                }),
                node_id: Some("node-selected".to_owned()),
                branch_id: None,
                generation: Some(1),
                primary_runtime_id: None,
                artifact_after: None,
                patch_id: None,
                evidence: Vec::new(),
            });
        graph.selections.selections.insert(
            entry_id.clone(),
            ploke_tree::graph::SelectionNode {
                entry_id,
                procedure_or_policy: ProcedureRefRecord {
                    value: "policy:test".to_owned(),
                },
                scope: ploke_records::history::SelectionScopeRecord {
                    value: "scope:test".to_owned(),
                },
                selected_candidate: Some(subject),
                selected_occurrence_id: None,
                selected_membership_id: Some(selected_membership_id),
                candidate_set_root: Some(root),
                considered_count: 2,
                projection_failure_count: 0,
                decision_outcome: ploke_records::selection::Outcome::Accepted,
            },
        );

        graph
    }

    fn graph_with_selected_successor(parent: &str, child: &str, block_height: u64) -> Graph {
        let lineage_id = LineageId("lineage:test".to_owned());
        let block_hash = BlockHash(format!("{block_height:064x}"));
        let parent_ref = ArtifactRefRecord::from_artifact_id(ArtifactId(parent.to_owned()));
        let child_ref = ArtifactRefRecord::from_artifact_id(ArtifactId(child.to_owned()));

        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            ArtifactKey::HistoryRef {
                id: parent_ref.id().0.clone(),
            },
            ArtifactNode {
                key: ArtifactKey::HistoryRef {
                    id: parent_ref.id().0.clone(),
                },
                identity: ArtifactIdentity::HistoryRef(parent_ref.clone()),
                ids: ploke_tree::graph::ArtifactIds {
                    artifact_refs: vec![parent_ref.clone()],
                    ..Default::default()
                },
                evidence: Vec::new(),
            },
        );
        artifacts.insert(
            ArtifactKey::HistoryRef {
                id: child_ref.id().0.clone(),
            },
            ArtifactNode {
                key: ArtifactKey::HistoryRef {
                    id: child_ref.id().0.clone(),
                },
                identity: ArtifactIdentity::HistoryRef(child_ref.clone()),
                ids: ploke_tree::graph::ArtifactIds {
                    artifact_refs: vec![child_ref.clone()],
                    ..Default::default()
                },
                evidence: Vec::new(),
            },
        );

        let mut blocks = BTreeMap::new();
        blocks.insert(
            block_hash.clone(),
            HistoryBlockNode {
                block_hash: block_hash.clone(),
                block_id: BlockId(format!("block-{block_height}")),
                lineage_id: lineage_id.clone(),
                block_height,
                parent_block_hashes: Vec::new(),
                opened_from_artifact: parent_ref.clone(),
                active_artifact: parent_ref,
                selected_successor: SuccessorNode {
                    runtime: ActorRefRecord::Runtime(RuntimeId("runtime:child".to_owned())),
                    artifact: child_ref,
                },
                opening_authority: OpeningAuthorityNode::Predecessor {
                    predecessor_block_hash: block_hash.clone(),
                },
                ruling_authority: ActorRefRecord::Process("parent:ruler".to_owned()),
                policy_ref: ProcedureRefRecord {
                    value: "policy:test".to_owned(),
                },
                surface: surface_commitment(),
                opened_at: RecordedAt(block_height as i64),
                sealed_at: RecordedAt(block_height as i64 + 1),
                entry_count: 0,
                entries: Vec::new(),
            },
        );

        let mut lineages = BTreeMap::new();
        lineages.insert(
            lineage_id.clone(),
            ploke_tree::graph::LineageNode {
                lineage_id: lineage_id.clone(),
                blocks: vec![block_hash.clone()],
            },
        );

        let mut epochs = BTreeMap::new();
        epochs.insert(lineage_id, vec![block_hash]);

        Graph {
            history: HistoryIndex {
                lineages,
                blocks,
                entries: BTreeMap::new(),
            },
            authority: AuthorityIndex {
                epochs_by_lineage: epochs,
            },
            artifacts: ArtifactIndex { artifacts },
            runtimes: Default::default(),
            operations: Default::default(),
            candidates: CandidateIndex::default(),
            selections: SelectionIndex::default(),
            evidence: Default::default(),
            warnings: Vec::new(),
            ..Graph::default()
        }
    }

    fn graph_with_unconsidered_sibling() -> Graph {
        let mut graph = graph_with_selected_successor("artifact:selected", "artifact:selected", 3);
        let parent_id = ArtifactId("artifact:parent".to_owned());
        let sibling_id = ArtifactId("artifact:sibling".to_owned());

        graph.artifacts.artifacts.insert(
            ArtifactKey::PassiveId {
                value: parent_id.0.clone(),
            },
            ArtifactNode {
                key: ArtifactKey::PassiveId {
                    value: parent_id.0.clone(),
                },
                identity: ArtifactIdentity::PassiveId(parent_id.clone()),
                ids: ploke_tree::graph::ArtifactIds {
                    artifact_ids: vec![parent_id.clone()],
                    ..Default::default()
                },
                evidence: Vec::new(),
            },
        );
        graph.artifacts.artifacts.insert(
            ArtifactKey::PassiveId {
                value: sibling_id.0.clone(),
            },
            ArtifactNode {
                key: ArtifactKey::PassiveId {
                    value: sibling_id.0.clone(),
                },
                identity: ArtifactIdentity::PassiveId(sibling_id.clone()),
                ids: ploke_tree::graph::ArtifactIds {
                    artifact_ids: vec![sibling_id.clone()],
                    ..Default::default()
                },
                evidence: Vec::new(),
            },
        );
        graph
            .candidates
            .candidates
            .push(ploke_tree::graph::CandidateNode {
                selection_entry_id: EntryId("entry-parent".to_owned()),
                payload_index: 0,
                subject: SubjectRefRecord {
                    value: "candidate:parent".to_owned(),
                },
                source: Some(CandidateSource::CurrentGeneration),
                occurrence_id: None,
                membership_id: None,
                membership_key: None,
                node_id: Some("node-parent".to_owned()),
                branch_id: Some("branch-parent".to_owned()),
                generation: Some(0),
                primary_runtime_id: None,
                artifact_after: Some(parent_id.clone()),
                patch_id: None,
                evidence: Vec::new(),
            });
        graph
            .candidates
            .candidates
            .push(ploke_tree::graph::CandidateNode {
                selection_entry_id: EntryId("entry-selected".to_owned()),
                payload_index: 0,
                subject: SubjectRefRecord {
                    value: "candidate:selected".to_owned(),
                },
                source: Some(CandidateSource::CurrentGeneration),
                occurrence_id: None,
                membership_id: None,
                membership_key: None,
                node_id: Some("node-selected".to_owned()),
                branch_id: Some("branch-selected".to_owned()),
                generation: Some(1),
                primary_runtime_id: None,
                artifact_after: Some(ArtifactId("artifact:selected".to_owned())),
                patch_id: Some(PatchId("patch-selected".to_owned())),
                evidence: Vec::new(),
            });
        graph.child_plans.plans.insert(
            SchedulerNodeId("node-parent".to_owned()),
            ChildPlanRecord {
                children: vec![
                    child_plan_record(
                        "node-parent",
                        "node-selected",
                        parent_id.0.as_str(),
                        "artifact:selected",
                        "branch-selected",
                        "candidate-selected",
                        "patch-selected",
                    )
                    .children
                    .into_iter()
                    .next()
                    .expect("selected child"),
                    child_plan_record(
                        "node-parent",
                        "node-sibling",
                        parent_id.0.as_str(),
                        sibling_id.0.as_str(),
                        "branch-sibling",
                        "candidate-sibling",
                        "patch-sibling",
                    )
                    .children
                    .into_iter()
                    .next()
                    .expect("sibling child"),
                ],
                ..child_plan_record(
                    "node-parent",
                    "node-selected",
                    parent_id.0.as_str(),
                    "artifact:selected",
                    "branch-selected",
                    "candidate-selected",
                    "patch-selected",
                )
            },
        );

        graph
    }

    fn child_plan_record(
        parent_node_id: &str,
        child_node_id: &str,
        base_artifact_id: &str,
        derived_artifact_id: &str,
        branch_id: &str,
        candidate_id: &str,
        patch_id: &str,
    ) -> ChildPlanRecord {
        ChildPlanRecord {
            message: PathBuf::from(format!("/tmp/{parent_node_id}.json")),
            parent_node_id: SchedulerNodeId(parent_node_id.to_owned()),
            child_generation: 1,
            children: vec![ChildPlanChildRecord {
                node: child_plan_node_record(
                    child_node_id,
                    parent_node_id,
                    base_artifact_id,
                    derived_artifact_id,
                    branch_id,
                    candidate_id,
                    patch_id,
                ),
                request: runner_request_record(
                    child_node_id,
                    base_artifact_id,
                    derived_artifact_id,
                    branch_id,
                    patch_id,
                ),
                resolved: resolved_branch(branch_id, candidate_id, patch_id, base_artifact_id),
                surface: None,
            }],
            rejected_surface_attempts: Vec::new(),
        }
    }

    fn child_plan_node_record(
        child_node_id: &str,
        parent_node_id: &str,
        base_artifact_id: &str,
        derived_artifact_id: &str,
        branch_id: &str,
        candidate_id: &str,
        patch_id: &str,
    ) -> NodeRecord {
        NodeRecord {
            schema_version: TREATMENT_NODE_SCHEMA_V1.to_owned(),
            node_id: SchedulerNodeId(child_node_id.to_owned()),
            parent_node_id: Some(SchedulerNodeId(parent_node_id.to_owned())),
            generation: 1,
            instance_id: InstanceId("instance".to_owned()),
            source_state_id: SourceStateId("source".to_owned()),
            operation_target: None,
            base_artifact_id: Some(ArtifactId(base_artifact_id.to_owned())),
            patch_id: Some(PatchId(patch_id.to_owned())),
            derived_artifact_id: Some(ArtifactId(derived_artifact_id.to_owned())),
            parent_branch_id: None,
            branch_id: BranchId(branch_id.to_owned()),
            candidate_id: CandidateId(candidate_id.to_owned()),
            target_relpath: PathBuf::from("src/lib.rs"),
            node_dir: PathBuf::from(format!("/tmp/nodes/{child_node_id}")),
            workspace_root: PathBuf::from(format!("/tmp/workspaces/{child_node_id}")),
            binary_path: PathBuf::from("/tmp/bin/ploke"),
            runner_request_path: PathBuf::from(format!("/tmp/nodes/{child_node_id}/request.json")),
            runner_result_path: PathBuf::from(format!("/tmp/nodes/{child_node_id}/result.json")),
            status: NodeStatusRecord::Planned,
            created_at: "created".to_owned(),
            updated_at: "updated".to_owned(),
        }
    }

    fn runner_request_record(
        child_node_id: &str,
        base_artifact_id: &str,
        derived_artifact_id: &str,
        branch_id: &str,
        patch_id: &str,
    ) -> RunnerRequestRecord {
        RunnerRequestRecord {
            schema_version: "prototype1-runner-request.v1".to_owned(),
            campaign_id: CampaignId("campaign".to_owned()),
            node_id: SchedulerNodeId(child_node_id.to_owned()),
            generation: 1,
            instance_id: InstanceId("instance".to_owned()),
            source_state_id: SourceStateId("source".to_owned()),
            operation_target: None,
            base_artifact_id: Some(ArtifactId(base_artifact_id.to_owned())),
            patch_id: Some(PatchId(patch_id.to_owned())),
            derived_artifact_id: Some(ArtifactId(derived_artifact_id.to_owned())),
            branch_id: BranchId(branch_id.to_owned()),
            target_relpath: PathBuf::from("src/lib.rs"),
            workspace_root: PathBuf::from(format!("/tmp/workspaces/{child_node_id}")),
            binary_path: PathBuf::from("/tmp/bin/ploke"),
            stop_on_error: false,
            runner_args: vec!["prototype1".to_owned(), "runner".to_owned()],
        }
    }

    fn resolved_branch(
        branch_id: &str,
        candidate_id: &str,
        patch_id: &str,
        base_artifact_id: &str,
    ) -> ResolvedTreatmentBranch {
        ResolvedTreatmentBranch {
            instance_id: "instance".to_owned(),
            source_state_id: "source".to_owned(),
            parent_branch_id: None,
            target_relpath: PathBuf::from("src/lib.rs"),
            source_content: "fn main() {}".to_owned(),
            source_content_hash: "sha256:source".to_owned(),
            selected_branch_id: Some(branch_id.to_owned()),
            branch: TreatmentBranchNode {
                branch_id: branch_id.to_owned(),
                candidate_id: candidate_id.to_owned(),
                patch_id: Some(PatchId(patch_id.to_owned())),
                branch_label: candidate_id.to_owned(),
                synthesized_spec_id: "spec".to_owned(),
                proposed_content: "fn main() { println!(\"hi\"); }".to_owned(),
                proposed_content_hash: "sha256:proposal".to_owned(),
                generation_target: Some(ploke_records::ids::OperationTarget::Artifact {
                    artifact_id: ArtifactId(base_artifact_id.to_owned()),
                }),
                generation_coordinate: Some(ploke_records::ids::Coordinate {
                    runtime_id: RuntimeId("runtime".to_owned()),
                    target: ploke_records::ids::OperationTarget::Artifact {
                        artifact_id: ArtifactId(base_artifact_id.to_owned()),
                    },
                }),
                status: TreatmentBranchStatus::Applied,
                apply_id: Some("apply".to_owned()),
                applied_content_hash: Some("sha256:applied".to_owned()),
                derived_artifact_id: None,
                latest_evaluation: None,
            },
        }
    }

    fn materialized_artifact_graph_with_four_nodes() -> Graph {
        let ids = [
            "artifact:root",
            "artifact:child-a",
            "artifact:child-b",
            "artifact:child-c",
        ];
        let artifacts = ids
            .iter()
            .copied()
            .map(|id| {
                (
                    ArtifactKey::PassiveId {
                        value: id.to_owned(),
                    },
                    ArtifactNode {
                        key: ArtifactKey::PassiveId {
                            value: id.to_owned(),
                        },
                        identity: ArtifactIdentity::PassiveId(ArtifactId(id.to_owned())),
                        ids: ploke_tree::graph::ArtifactIds {
                            artifact_ids: vec![ArtifactId(id.to_owned())],
                            ..Default::default()
                        },
                        evidence: Vec::new(),
                    },
                )
            })
            .collect();

        let candidates = ids
            .iter()
            .copied()
            .enumerate()
            .map(|(index, id)| ploke_tree::graph::CandidateNode {
                selection_entry_id: EntryId(format!("entry:{index}")),
                payload_index: 0,
                subject: SubjectRefRecord {
                    value: format!("candidate:{index}"),
                },
                source: Some(CandidateSource::CurrentGeneration),
                occurrence_id: None,
                membership_id: None,
                membership_key: None,
                node_id: Some(format!("node:{index}")),
                branch_id: Some(format!("branch:{index}")),
                generation: Some(index as u32),
                primary_runtime_id: None,
                artifact_after: Some(ArtifactId(id.to_owned())),
                patch_id: None,
                evidence: Vec::new(),
            })
            .collect();

        Graph {
            artifacts: ArtifactIndex { artifacts },
            candidates: CandidateIndex {
                candidates,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn surface_commitment() -> SurfaceCommitmentRecord {
        SurfaceCommitmentRecord {
            immutable: surface("1"),
            mutated: SurfaceDeltaRecord {
                before: surface("2"),
                after: surface("3"),
            },
            ambient: SurfaceDeltaRecord {
                before: surface("4"),
                after: surface("5"),
            },
        }
    }

    fn surface(seed: &str) -> SurfaceRecord {
        SurfaceRecord {
            root: SurfaceRootRecord {
                hash: HistoryHash(seed.repeat(64)),
            },
        }
    }
}
