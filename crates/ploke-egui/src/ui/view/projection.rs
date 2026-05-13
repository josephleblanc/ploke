use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use eframe::egui::{Color32, Vec2};
use petgraph::{
    Directed,
    Direction::Incoming,
    stable_graph::{NodeIndex, StableGraph},
};
use ploke_records::ids::ArtifactId;
use ploke_tree::Graph as DomainGraph;
use ploke_tree::graph::{
    ArtifactIdentity, ArtifactNode, CandidateBranchNode, CandidateNode, EvidenceSubject,
    OperationKey, OperationTargetKey, SelectionNode,
};

use super::diagnostics::graph_diagnostics;
use super::edge::GraphEdgeShape;
use super::style::{EdgeStyle, ViewStyle};
use super::{
    ComponentRootDiagnostic, EdgeLabelDiagnostics, GraphConnectivityDiagnostics,
    GraphSelectionDetail, GraphViewDiagnostics, GraphViewMode,
};

pub(super) type WidgetGraph = egui_graphs::Graph<
    GraphNode,
    GraphEdgePayload,
    Directed,
    petgraph::stable_graph::DefaultIx,
    egui_graphs::DefaultNodeShape,
    GraphEdgeShape,
>;
type RawGraph = StableGraph<GraphNode, GraphEdgePayload, Directed>;
type WidgetNode = egui_graphs::Node<
    GraphNode,
    GraphEdgePayload,
    Directed,
    petgraph::stable_graph::DefaultIx,
    egui_graphs::DefaultNodeShape,
>;

#[derive(Debug)]
pub(super) struct GraphViewCache {
    signature: Option<GraphSignature>,
    style: ViewStyle,
    mode: GraphViewMode,
    graph: WidgetGraph,
    connectivity: GraphConnectivityDiagnostics,
}

impl Default for GraphViewCache {
    fn default() -> Self {
        Self {
            signature: None,
            style: ViewStyle::default(),
            mode: GraphViewMode::default(),
            graph: to_widget_graph(&RawGraph::default(), ViewStyle::default()),
            connectivity: GraphConnectivityDiagnostics::default(),
        }
    }
}

impl GraphViewCache {
    pub(super) fn refresh(
        &mut self,
        graph: &DomainGraph,
        style: ViewStyle,
        mode: GraphViewMode,
    ) -> bool {
        let signature = GraphSignature::from(graph);
        if self.signature == Some(signature) && self.style == style && self.mode == mode {
            return false;
        }

        self.signature = Some(signature);
        self.style = style;
        self.mode = mode;
        let built = build_widget_graph(graph, style, mode);
        self.graph = built.graph;
        self.connectivity = built.connectivity;
        true
    }

    pub(super) fn graph_mut(&mut self) -> &mut WidgetGraph {
        &mut self.graph
    }

    pub(super) fn diagnostics(
        &self,
        viewport_size: Vec2,
        style: ViewStyle,
        edge_labels: EdgeLabelDiagnostics,
    ) -> Option<GraphViewDiagnostics> {
        graph_diagnostics(
            &self.graph,
            viewport_size,
            style,
            edge_labels,
            self.connectivity.clone(),
            self.mode,
        )
    }

    pub(super) fn selected_node_detail(&self) -> Option<GraphSelectionDetail> {
        let selected = self.graph.selected_nodes().first().copied()?;
        let node = self.graph.g().node_weight(selected)?;
        let payload = node.payload();
        Some(GraphSelectionDetail {
            kind: payload.kind_name().to_owned(),
            label: payload.label().to_owned(),
            detail: payload.detail().to_owned(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GraphSignature {
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
    evidence: usize,
    warnings: usize,
}

impl From<&DomainGraph> for GraphSignature {
    fn from(graph: &DomainGraph) -> Self {
        Self {
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
            evidence: graph.evidence.attachments.len(),
            warnings: graph.warnings.len(),
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
        color: Color32,
    },
    Candidate {
        label: Arc<str>,
        detail: Arc<str>,
        color: Color32,
    },
    Record {
        kind: GraphNodeKind,
        label: Arc<str>,
        detail: Arc<str>,
        color: Color32,
    },
    Synthetic {
        kind: GraphNodeKind,
        label: Arc<str>,
        detail: Arc<str>,
        color: Color32,
    },
}

impl GraphNode {
    fn kind_name(&self) -> &'static str {
        match self {
            Self::Artifact { .. } => "artifact",
            Self::Candidate { .. } => "candidate",
            Self::Record { kind, .. } | Self::Synthetic { kind, .. } => kind.as_str(),
        }
    }

    fn label(&self) -> &str {
        match self {
            Self::Artifact { label, .. }
            | Self::Candidate { label, .. }
            | Self::Record { label, .. }
            | Self::Synthetic { label, .. } => label,
        }
    }

    fn detail(&self) -> &str {
        match self {
            Self::Artifact { detail, .. }
            | Self::Candidate { detail, .. }
            | Self::Record { detail, .. }
            | Self::Synthetic { detail, .. } => detail,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GraphNodeKind {
    Run,
    Unattached,
    Lineage,
    HistoryBlock,
    HistoryEntry,
    Selection,
    Membership,
    Branch,
    Runtime,
    Operation,
    Evidence,
    Warning,
}

impl GraphNodeKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Unattached => "unattached",
            Self::Lineage => "lineage",
            Self::HistoryBlock => "history-block",
            Self::HistoryEntry => "history-entry",
            Self::Selection => "selection",
            Self::Membership => "membership",
            Self::Branch => "branch",
            Self::Runtime => "runtime",
            Self::Operation => "operation",
            Self::Evidence => "evidence",
            Self::Warning => "warning",
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ViewEdgeKind {
    ArtifactPatch,
    HistoryArtifact,
    Candidate,
    Operation,
    Evidence,
    Synthetic,
}

struct BuiltWidgetGraph {
    graph: WidgetGraph,
    connectivity: GraphConnectivityDiagnostics,
}

#[derive(Debug)]
pub(super) struct ProjectedGraph {
    raw: RawGraph,
    connectivity: GraphConnectivityDiagnostics,
}

pub(super) fn project_graph(graph: &DomainGraph, style: ViewStyle) -> ProjectedGraph {
    let mut raw = RawGraph::default();
    let run = raw.add_node(GraphNode::Synthetic {
        kind: GraphNodeKind::Run,
        label: Arc::from("run"),
        detail: Arc::from("loaded Prototype 1 run graph"),
        color: style.edge.colors.applied,
    });

    let mut lineage_nodes = HashMap::new();
    let mut block_nodes = HashMap::new();
    let mut entry_nodes = HashMap::new();
    let mut selection_nodes = HashMap::new();
    let mut membership_nodes = BTreeMap::new();
    let mut artifact_nodes = HashMap::new();
    let mut artifact_history_refs = HashMap::new();
    let mut artifact_passive_ids = HashMap::new();
    let mut candidate_nodes = HashMap::new();
    let mut branch_nodes = HashMap::new();
    let mut runtime_nodes = HashMap::new();

    for lineage in graph.history.lineages.values() {
        let node = raw.add_node(record_node(
            GraphNodeKind::Lineage,
            format!("L:{}", compact_id(&lineage.lineage_id.0)),
            format!("lineage_id: {}", lineage.lineage_id.0),
            style.edge.colors.applied,
        ));
        lineage_nodes.insert(&lineage.lineage_id, node);
        add_edge(
            &mut raw,
            run,
            node,
            "lineage",
            false,
            ViewEdgeKind::Synthetic,
            style,
        );
    }

    for block in graph.history.blocks.values() {
        let node = raw.add_node(record_node(
            GraphNodeKind::HistoryBlock,
            format!("B{}", block.block_height),
            format!(
                "block_hash: {}\nblock_id: {}\nlineage_id: {}",
                block.block_hash.0, block.block_id.0, block.lineage_id.0
            ),
            style.edge.colors.selected,
        ));
        block_nodes.insert(&block.block_hash, node);
        if let Some(lineage) = lineage_nodes.get(&block.lineage_id).copied() {
            add_edge(
                &mut raw,
                lineage,
                node,
                "block",
                false,
                ViewEdgeKind::HistoryArtifact,
                style,
            );
        }
    }

    for block in graph.history.blocks.values() {
        let Some(child) = block_nodes.get(&block.block_hash).copied() else {
            continue;
        };
        for parent_hash in &block.parent_block_hashes {
            if let Some(parent) = block_nodes.get(parent_hash).copied() {
                add_edge(
                    &mut raw,
                    parent,
                    child,
                    "next",
                    false,
                    ViewEdgeKind::HistoryArtifact,
                    style,
                );
            }
        }
    }

    for entry in graph.history.entries.values() {
        let node = raw.add_node(record_node(
            GraphNodeKind::HistoryEntry,
            format!("E:{}", compact_id(&entry.entry_id.0)),
            format!(
                "entry_id: {}\nblock_hash: {}\nkind: {:?}\nsubject: {}",
                entry.entry_id.0, entry.block_hash.0, entry.kind, entry.subject.value
            ),
            style.edge.colors.restored,
        ));
        entry_nodes.insert(&entry.entry_id, node);
        if let Some(block) = block_nodes.get(&entry.block_hash).copied() {
            add_edge(
                &mut raw,
                block,
                node,
                "entry",
                false,
                ViewEdgeKind::HistoryArtifact,
                style,
            );
        }
    }

    for artifact in graph.artifacts.artifacts.values() {
        let node = raw.add_node(GraphNode::Artifact {
            label: Arc::from(artifact_label(artifact, style)),
            detail: Arc::from(artifact_detail(artifact)),
            color: artifact_color(graph, artifact, style),
        });
        artifact_nodes.insert(&artifact.key, node);
        match &artifact.identity {
            ArtifactIdentity::HistoryRef(history_ref) => {
                artifact_history_refs.insert(history_ref.value.as_str(), node);
            }
            ArtifactIdentity::PassiveId(artifact_id) => {
                artifact_passive_ids.insert(artifact_id, node);
            }
        }
    }

    for runtime in graph.runtimes.runtimes.values() {
        let node = raw.add_node(record_node(
            GraphNodeKind::Runtime,
            format!("R:{}", compact_id(&runtime.runtime_id.0)),
            format!("runtime_id: {}", runtime.runtime_id.0),
            style.edge.colors.applied,
        ));
        runtime_nodes.insert(&runtime.runtime_id, node);
    }

    for block in graph.history.blocks.values() {
        let Some(block_node) = block_nodes.get(&block.block_hash).copied() else {
            continue;
        };
        if let Some(active) = artifact_history_refs.get(block.active_artifact.value.as_str()) {
            add_edge(
                &mut raw,
                block_node,
                *active,
                "active",
                false,
                ViewEdgeKind::HistoryArtifact,
                style,
            );
        }
        if let Some(successor) =
            artifact_history_refs.get(block.selected_successor.artifact.value.as_str())
        {
            add_edge(
                &mut raw,
                block_node,
                *successor,
                "successor",
                false,
                ViewEdgeKind::HistoryArtifact,
                style,
            );
        }
        if let ploke_records::history::ActorRefRecord::Runtime(runtime_id) =
            &block.selected_successor.runtime
        {
            if let Some(runtime) = runtime_nodes.get(runtime_id).copied() {
                add_edge(
                    &mut raw,
                    block_node,
                    runtime,
                    "runtime",
                    false,
                    ViewEdgeKind::Operation,
                    style,
                );
            }
        }
    }

    for selection in graph.selections.selections.values() {
        let node = raw.add_node(record_node(
            GraphNodeKind::Selection,
            format!("S:{}", compact_id(&selection.entry_id.0)),
            format!(
                "entry_id: {}\nselected_candidate: {:?}\nselected_occurrence_id: {:?}\nselected_membership_id: {:?}",
                selection.entry_id.0,
                selection.selected_candidate,
                selection.selected_occurrence_id,
                selection.selected_membership_id
            ),
            style.edge.colors.selected,
        ));
        selection_nodes.insert(&selection.entry_id, node);
        if let Some(entry) = entry_nodes.get(&selection.entry_id).copied() {
            add_edge(
                &mut raw,
                entry,
                node,
                "selection",
                false,
                ViewEdgeKind::Candidate,
                style,
            );
        }
    }

    let mut candidates = graph.candidates.candidates.iter().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        (&left.selection_entry_id.0, left.payload_index)
            .cmp(&(&right.selection_entry_id.0, right.payload_index))
    });
    for candidate in candidates {
        let node = raw.add_node(GraphNode::Candidate {
            label: Arc::from(candidate_label(candidate)),
            detail: Arc::from(candidate_detail(candidate)),
            color: candidate_color(graph, candidate, style),
        });
        candidate_nodes.insert(
            (&candidate.selection_entry_id, candidate.payload_index),
            node,
        );
        if let Some(selection) = selection_nodes.get(&candidate.selection_entry_id).copied() {
            add_edge(
                &mut raw,
                selection,
                node,
                "candidate",
                false,
                ViewEdgeKind::Candidate,
                style,
            );
        }
        if let Some(artifact_after) = candidate.artifact_after.as_ref() {
            if let Some(artifact) = artifact_passive_ids.get(artifact_after).copied() {
                add_edge(
                    &mut raw,
                    node,
                    artifact,
                    "artifact",
                    false,
                    ViewEdgeKind::ArtifactPatch,
                    style,
                );
            }
        }
    }

    for (membership_key, membership) in &graph.candidates.memberships {
        let node = raw.add_node(record_node(
            GraphNodeKind::Membership,
            format!("M:{}", compact_id(&membership.membership_id.0)),
            format!(
                "membership_id: {}\ncandidate_set_root: {}\nselection_entry_id: {}\nsubject: {}\npayload_hash: {}",
                membership.membership_id.0,
                membership.candidate_set_root.0.0,
                membership.selection_entry_id.0,
                membership.candidate_subject.value,
                membership.payload_hash
            ),
            style.edge.colors.restored,
        ));
        membership_nodes.insert(membership_key, node);
        if let Some(selection) = selection_nodes.get(&membership.selection_entry_id).copied() {
            add_edge(
                &mut raw,
                selection,
                node,
                "member",
                false,
                ViewEdgeKind::Candidate,
                style,
            );
        }
    }

    for candidate in &graph.candidates.candidates {
        let (Some(membership_key), Some(candidate_node)) = (
            candidate.membership_key.as_ref(),
            candidate_nodes.get(&(&candidate.selection_entry_id, candidate.payload_index)),
        ) else {
            continue;
        };
        if let Some(membership) = membership_nodes.get(membership_key).copied() {
            add_edge(
                &mut raw,
                membership,
                *candidate_node,
                "candidate",
                false,
                ViewEdgeKind::Candidate,
                style,
            );
        }
    }

    let mut branches = graph.candidates.branches.iter().collect::<Vec<_>>();
    branches.sort_by(|left, right| {
        (
            &left.branch_id,
            &left.candidate_id,
            &left.derived_artifact_id,
        )
            .cmp(&(
                &right.branch_id,
                &right.candidate_id,
                &right.derived_artifact_id,
            ))
    });
    for (index, branch) in branches.into_iter().enumerate() {
        let branch_selected = branch_selected_by_graph(graph, branch);
        let node = raw.add_node(record_node(
            GraphNodeKind::Branch,
            format!("P{}", index + 1),
            branch_detail(branch),
            if branch_selected {
                style.edge.colors.selected
            } else {
                style.edge.colors.synthesized
            },
        ));
        branch_nodes.insert(branch.branch_id.as_str(), node);
        if let Some(selection) = selection_nodes.get(&branch.selection_entry_id).copied() {
            add_edge(
                &mut raw,
                selection,
                node,
                "branch",
                false,
                ViewEdgeKind::Candidate,
                style,
            );
        }
        if let Some(base_artifact_id) = branch.base_artifact_id.as_ref() {
            if let Some(base) = artifact_passive_ids.get(base_artifact_id).copied() {
                add_edge(
                    &mut raw,
                    base,
                    node,
                    "base",
                    false,
                    ViewEdgeKind::ArtifactPatch,
                    style,
                );
            }
        }
        if let Some(derived_artifact_id) = branch.derived_artifact_id.as_ref() {
            if let Some(derived) = artifact_passive_ids.get(derived_artifact_id).copied() {
                add_edge(
                    &mut raw,
                    node,
                    derived,
                    "derived",
                    false,
                    ViewEdgeKind::ArtifactPatch,
                    style,
                );
            }
        }
    }

    for candidate in &graph.candidates.candidates {
        let Some(candidate_node) =
            candidate_nodes.get(&(&candidate.selection_entry_id, candidate.payload_index))
        else {
            continue;
        };
        if let Some(branch_id) = candidate.branch_id.as_deref() {
            if let Some(branch) = branch_nodes.get(branch_id).copied() {
                add_edge(
                    &mut raw,
                    *candidate_node,
                    branch,
                    "branch",
                    false,
                    ViewEdgeKind::Candidate,
                    style,
                );
            }
        }
    }

    for operation in graph.operations.operations.values() {
        let node = raw.add_node(record_node(
            GraphNodeKind::Operation,
            operation_label(&operation.key),
            format!("operation: {:?}", operation.key),
            style.edge.colors.restored,
        ));
        match &operation.key {
            OperationKey::HistoryEntry { entry_id } => {
                if let Some(entry) = entry_nodes.get(entry_id).copied() {
                    add_edge(
                        &mut raw,
                        entry,
                        node,
                        "op",
                        false,
                        ViewEdgeKind::Operation,
                        style,
                    );
                }
            }
            OperationKey::RuntimeTarget { runtime_id, target } => {
                if let Some(runtime) = runtime_nodes.get(runtime_id).copied() {
                    add_edge(
                        &mut raw,
                        runtime,
                        node,
                        "op",
                        false,
                        ViewEdgeKind::Operation,
                        style,
                    );
                }
                connect_operation_target(&mut raw, node, target, &artifact_passive_ids, style);
            }
        }
    }

    for (index, evidence) in graph.evidence.attachments.values().enumerate() {
        let node = raw.add_node(record_node(
            GraphNodeKind::Evidence,
            format!("V{}", index + 1),
            format!(
                "evidence_id: {:?}\nsubject: {:?}\nkind: {:?}",
                evidence.id, evidence.subject, evidence.kind
            ),
            style.edge.colors.restored,
        ));
        if let Some(subject) = evidence_subject_node(
            &evidence.subject,
            &block_nodes,
            &entry_nodes,
            &selection_nodes,
            &candidate_nodes,
            &branch_nodes,
            &runtime_nodes,
            &artifact_passive_ids,
        ) {
            add_edge(
                &mut raw,
                subject,
                node,
                "evidence",
                false,
                ViewEdgeKind::Evidence,
                style,
            );
        }
    }

    for (index, warning) in graph.warnings.iter().enumerate() {
        let node = raw.add_node(record_node(
            GraphNodeKind::Warning,
            format!("W{}", index + 1),
            format!("warning: {:?}\n{}", warning.kind, warning.detail),
            style.edge.colors.dropped,
        ));
        add_edge(
            &mut raw,
            run,
            node,
            "warning",
            false,
            ViewEdgeKind::Synthetic,
            style,
        );
    }

    let components = component_roots(&raw);
    let connectivity = GraphConnectivityDiagnostics {
        component_count_before_anchoring: components.len(),
        component_roots_before_anchoring: components
            .iter()
            .map(|component| ComponentRootDiagnostic {
                kind: component.kind.to_owned(),
                label: component.label.to_owned(),
            })
            .collect(),
        hidden_record_count: 0,
        hidden_edge_count: 0,
        hidden_evidence_count: 0,
        hidden_operation_count: 0,
        hidden_unattached_component_count: 0,
        synthetic_anchors_visible: components.len() > 1,
    };
    anchor_unattached_components(&mut raw, run, &components, style);

    ProjectedGraph { raw, connectivity }
}

fn project_artifact_tree(graph: &DomainGraph, style: ViewStyle) -> ProjectedGraph {
    let full = project_graph(graph, style);
    let mut raw = RawGraph::default();
    let mut artifact_nodes = BTreeMap::new();
    let mut history_refs = HashMap::new();
    let mut passive_ids = HashMap::new();
    let current_parent = current_parent_artifact_ref(graph);

    for artifact in graph.artifacts.artifacts.values() {
        let node = add_artifact_tree_node(
            &mut raw,
            &mut artifact_nodes,
            artifact,
            current_parent,
            style,
        );
        match &artifact.identity {
            ArtifactIdentity::HistoryRef(history_ref) => {
                history_refs.insert(history_ref.value.as_str(), node);
            }
            ArtifactIdentity::PassiveId(artifact_id) => {
                passive_ids.insert(artifact_id, node);
            }
        }
    }

    let mut patch_index = 1;
    let mut blocks = graph.history.blocks.values().collect::<Vec<_>>();
    blocks.sort_by_key(|block| block.block_height);
    for block in blocks {
        let Some(parent) = history_refs
            .get(block.active_artifact.value.as_str())
            .or_else(|| history_refs.get(block.opened_from_artifact.value.as_str()))
            .copied()
        else {
            continue;
        };
        let Some(child) = history_refs
            .get(block.selected_successor.artifact.value.as_str())
            .copied()
        else {
            continue;
        };
        if add_unique_edge(
            &mut raw,
            parent,
            child,
            format!("P{patch_index}"),
            true,
            ViewEdgeKind::ArtifactPatch,
            style,
        ) {
            patch_index += 1;
        }
    }

    let mut branches = graph.candidates.branches.iter().collect::<Vec<_>>();
    branches.sort_by(|left, right| {
        (
            &left.selection_entry_id.0,
            left.payload_index,
            &left.branch_id,
            &left.derived_artifact_id,
        )
            .cmp(&(
                &right.selection_entry_id.0,
                right.payload_index,
                &right.branch_id,
                &right.derived_artifact_id,
            ))
    });
    for branch in branches {
        let (Some(base_id), Some(derived_id)) = (
            branch.base_artifact_id.as_ref(),
            branch.derived_artifact_id.as_ref(),
        ) else {
            continue;
        };
        let (Some(base), Some(derived)) = (
            passive_ids.get(base_id).copied(),
            passive_ids.get(derived_id).copied(),
        ) else {
            continue;
        };
        if add_unique_edge(
            &mut raw,
            base,
            derived,
            format!("P{patch_index}"),
            true,
            ViewEdgeKind::ArtifactPatch,
            style,
        ) {
            patch_index += 1;
        }
    }

    let components = component_roots(&raw);
    let hidden_record_count = full.raw.node_count().saturating_sub(raw.node_count());
    let hidden_edge_count = full.raw.edge_count().saturating_sub(raw.edge_count());
    let connectivity = GraphConnectivityDiagnostics {
        component_count_before_anchoring: components.len(),
        component_roots_before_anchoring: components
            .iter()
            .map(|component| ComponentRootDiagnostic {
                kind: component.kind.to_owned(),
                label: component.label.to_owned(),
            })
            .collect(),
        hidden_record_count,
        hidden_edge_count,
        hidden_evidence_count: graph.evidence.attachments.len(),
        hidden_operation_count: graph.operations.operations.len(),
        hidden_unattached_component_count: components.len().saturating_sub(1),
        synthetic_anchors_visible: false,
    };

    ProjectedGraph { raw, connectivity }
}

fn primary_lineage_blocks<'a>(
    graph: &'a DomainGraph,
) -> Option<(
    &'a ploke_records::ids::LineageId,
    Vec<&'a ploke_tree::graph::HistoryBlockNode>,
)> {
    graph
        .history
        .lineages
        .values()
        .filter_map(|lineage| {
            let mut blocks = lineage
                .blocks
                .iter()
                .filter_map(|block_hash| graph.history.blocks.get(block_hash))
                .collect::<Vec<_>>();
            blocks.sort_by_key(|block| block.block_height);
            if blocks.is_empty() {
                None
            } else {
                Some((&lineage.lineage_id, blocks))
            }
        })
        .max_by_key(|(_, blocks)| {
            (
                blocks.len(),
                blocks
                    .iter()
                    .map(|block| block.block_height)
                    .max()
                    .unwrap_or_default(),
            )
        })
}

fn current_parent_artifact_ref(graph: &DomainGraph) -> Option<&str> {
    let (_, blocks) = primary_lineage_blocks(graph)?;
    blocks
        .into_iter()
        .max_by_key(|block| block.block_height)
        .map(|block| block.active_artifact.value.as_str())
}

fn add_artifact_tree_node(
    raw: &mut RawGraph,
    artifact_nodes: &mut BTreeMap<String, NodeIndex>,
    artifact: &ArtifactNode,
    current_parent: Option<&str>,
    style: ViewStyle,
) -> NodeIndex {
    let key = artifact_key(artifact);
    if let Some(node) = artifact_nodes.get(&key).copied() {
        return node;
    }

    let node = raw.add_node(GraphNode::Artifact {
        label: Arc::from(artifact_label(artifact, style)),
        detail: Arc::from(artifact_detail(artifact)),
        color: artifact_tree_color(artifact, current_parent, style),
    });
    artifact_nodes.insert(key, node);
    node
}

fn artifact_key(artifact: &ArtifactNode) -> String {
    match &artifact.identity {
        ArtifactIdentity::HistoryRef(history_ref) => format!("history:{}", history_ref.value),
        ArtifactIdentity::PassiveId(artifact_id) => format!("passive:{}", artifact_id.0),
    }
}

fn add_unique_edge(
    raw: &mut RawGraph,
    source: NodeIndex,
    target: NodeIndex,
    label: impl Into<Arc<str>>,
    label_visible: bool,
    kind: ViewEdgeKind,
    style: ViewStyle,
) -> bool {
    if raw.edges_connecting(source, target).next().is_some() {
        return false;
    }
    add_edge(raw, source, target, label, label_visible, kind, style);
    true
}

fn build_widget_graph(
    graph: &DomainGraph,
    style: ViewStyle,
    mode: GraphViewMode,
) -> BuiltWidgetGraph {
    let projected: ProjectedGraph = match mode {
        GraphViewMode::ArtifactTree => project_artifact_tree(graph, style),
        GraphViewMode::FullDebug => project_graph(graph, style),
    };
    BuiltWidgetGraph {
        graph: to_widget_graph(&projected.raw, style),
        connectivity: projected.connectivity,
    }
}

fn artifact_color(graph: &DomainGraph, artifact: &ArtifactNode, style: ViewStyle) -> Color32 {
    if graph.history.blocks.values().any(|block| {
        matches!(
            &artifact.identity,
            ArtifactIdentity::HistoryRef(history_ref)
                if history_ref.value == block.selected_successor.artifact.value
        )
    }) {
        style.edge.colors.selected
    } else {
        style.edge.colors.synthesized
    }
}

fn artifact_tree_color(
    artifact: &ArtifactNode,
    current_parent: Option<&str>,
    style: ViewStyle,
) -> Color32 {
    if matches!(
        (&artifact.identity, current_parent),
        (ArtifactIdentity::HistoryRef(history_ref), Some(current))
            if history_ref.value == current
    ) {
        style.edge.colors.selected
    } else {
        style.edge.colors.synthesized
    }
}

fn branch_selected_by_graph(graph: &DomainGraph, branch: &CandidateBranchNode) -> bool {
    graph.candidates.candidates.iter().any(|candidate| {
        candidate.branch_id.as_deref() == Some(branch.branch_id.as_str())
            && candidate_selected_by_graph(graph, candidate)
    })
}

fn candidate_color(graph: &DomainGraph, candidate: &CandidateNode, style: ViewStyle) -> Color32 {
    if candidate_selected_by_graph(graph, candidate) {
        style.edge.colors.selected
    } else {
        style.edge.colors.synthesized
    }
}

fn candidate_selected_by_graph(graph: &DomainGraph, candidate: &CandidateNode) -> bool {
    let Some(selection) = graph
        .selections
        .selections
        .get(&candidate.selection_entry_id)
    else {
        return false;
    };

    candidate_selected(selection, candidate)
}

fn candidate_selected(selection: &SelectionNode, candidate: &CandidateNode) -> bool {
    if let (Some(root), Some(selected_membership_id)) = (
        selection.candidate_set_root.as_ref(),
        selection.selected_membership_id.as_ref(),
    ) {
        return candidate
            .membership_key
            .as_ref()
            .is_some_and(|candidate_key| {
                &candidate_key.candidate_set_root == root
                    && &candidate_key.membership_id == selected_membership_id
            });
    }

    if let Some(selected_occurrence_id) = selection.selected_occurrence_id.as_ref() {
        return candidate
            .occurrence_id
            .as_ref()
            .is_some_and(|occurrence_id| occurrence_id == selected_occurrence_id);
    }

    selection
        .selected_candidate
        .as_ref()
        .is_some_and(|selected| selected == &candidate.subject)
}

fn artifact_label(artifact: &ArtifactNode, style: ViewStyle) -> String {
    truncate_label(match &artifact.identity {
        ArtifactIdentity::HistoryRef(history_ref) => {
            format!(
                "A:{}",
                compact_id(&style.labels.artifact(history_ref.value.as_str()))
            )
        }
        ArtifactIdentity::PassiveId(artifact_id) => {
            format!(
                "A:{}",
                compact_id(&style.labels.artifact(artifact_id.0.as_str()))
            )
        }
    })
}

fn candidate_label(candidate: &CandidateNode) -> String {
    let base = format!(
        "C{}:{}",
        compact_id(&candidate.selection_entry_id.0),
        candidate.payload_index + 1
    );
    truncate_label(
        candidate
            .generation
            .map(|generation| format!("{base} g{generation}"))
            .unwrap_or(base),
    )
}

fn record_node(
    kind: GraphNodeKind,
    label: impl Into<String>,
    detail: impl Into<String>,
    color: Color32,
) -> GraphNode {
    GraphNode::Record {
        kind,
        label: Arc::from(truncate_label(label.into())),
        detail: Arc::from(detail.into()),
        color,
    }
}

fn add_edge(
    raw: &mut RawGraph,
    source: NodeIndex,
    target: NodeIndex,
    label: impl Into<Arc<str>>,
    label_visible: bool,
    kind: ViewEdgeKind,
    style: ViewStyle,
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
        },
    );
}

fn edge_color(kind: ViewEdgeKind, style: ViewStyle) -> Color32 {
    match kind {
        ViewEdgeKind::HistoryArtifact => style.edge.colors.selected,
        ViewEdgeKind::ArtifactPatch => style.edge.colors.synthesized,
        ViewEdgeKind::Candidate => style.edge.colors.selected,
        ViewEdgeKind::Operation => style.edge.colors.applied,
        ViewEdgeKind::Evidence => style.edge.colors.restored,
        ViewEdgeKind::Synthetic => style.edge.colors.dropped,
    }
}

fn connect_operation_target(
    raw: &mut RawGraph,
    operation: NodeIndex,
    target: &OperationTargetKey,
    artifacts: &HashMap<&ArtifactId, NodeIndex>,
    style: ViewStyle,
) {
    match target {
        OperationTargetKey::Artifact { artifact_id } => {
            if let Some(artifact) = artifacts.get(artifact_id).copied() {
                add_edge(
                    raw,
                    operation,
                    artifact,
                    "target",
                    false,
                    ViewEdgeKind::Operation,
                    style,
                );
            }
        }
        OperationTargetKey::PatchSet {
            base_artifact_id,
            patch_ids: _,
        } => {
            if let Some(artifact) = artifacts.get(base_artifact_id).copied() {
                add_edge(
                    raw,
                    operation,
                    artifact,
                    "base",
                    false,
                    ViewEdgeKind::Operation,
                    style,
                );
            }
        }
        OperationTargetKey::ArtifactSet {
            base_artifact_id,
            artifact_ids,
        } => {
            if let Some(base_artifact_id) = base_artifact_id {
                if let Some(artifact) = artifacts.get(base_artifact_id).copied() {
                    add_edge(
                        raw,
                        operation,
                        artifact,
                        "base",
                        false,
                        ViewEdgeKind::Operation,
                        style,
                    );
                }
            }
            for artifact_id in artifact_ids {
                if let Some(artifact) = artifacts.get(artifact_id).copied() {
                    add_edge(
                        raw,
                        operation,
                        artifact,
                        "target",
                        false,
                        ViewEdgeKind::Operation,
                        style,
                    );
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn evidence_subject_node(
    subject: &EvidenceSubject,
    blocks: &HashMap<&ploke_records::ids::BlockHash, NodeIndex>,
    entries: &HashMap<&ploke_records::ids::EntryId, NodeIndex>,
    selections: &HashMap<&ploke_records::ids::EntryId, NodeIndex>,
    candidates: &HashMap<(&ploke_records::ids::EntryId, usize), NodeIndex>,
    branches: &HashMap<&str, NodeIndex>,
    runtimes: &HashMap<&ploke_records::ids::RuntimeId, NodeIndex>,
    artifacts: &HashMap<&ArtifactId, NodeIndex>,
) -> Option<NodeIndex> {
    match subject {
        EvidenceSubject::HistoryBlock(block_hash) => blocks.get(block_hash).copied(),
        EvidenceSubject::HistoryEntry(entry_id) => entries.get(entry_id).copied(),
        EvidenceSubject::Runtime(runtime_id) => runtimes.get(runtime_id).copied(),
        EvidenceSubject::Artifact(artifact_id) => artifacts.get(artifact_id).copied(),
        EvidenceSubject::Candidate {
            selection_entry_id,
            payload_index,
        } => candidates
            .get(&(selection_entry_id, *payload_index))
            .copied(),
        EvidenceSubject::Branch(branch_id) => branches.get(branch_id.as_str()).copied(),
        EvidenceSubject::Selection(entry_id) => selections.get(entry_id).copied(),
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
        | EvidenceSubject::RunProfileSummary(_)
        | EvidenceSubject::RunProfileCommitment(_)
        | EvidenceSubject::AgentTurnEvidenceSummary { .. }
        | EvidenceSubject::AgentTurnArtifact(_) => None,
    }
}

fn artifact_detail(artifact: &ArtifactNode) -> String {
    match &artifact.identity {
        ArtifactIdentity::HistoryRef(history_ref) => {
            format!("artifact history ref: {}", history_ref.value)
        }
        ArtifactIdentity::PassiveId(artifact_id) => format!("artifact_id: {}", artifact_id.0),
    }
}

fn candidate_detail(candidate: &CandidateNode) -> String {
    format!(
        "selection_entry_id: {}\npayload_index: {}\nsubject: {}\nnode_id: {:?}\nbranch_id: {:?}\noccurrence_id: {:?}\nmembership_id: {:?}",
        candidate.selection_entry_id.0,
        candidate.payload_index,
        candidate.subject.value,
        candidate.node_id,
        candidate.branch_id,
        candidate.occurrence_id,
        candidate.membership_id
    )
}

fn branch_detail(branch: &CandidateBranchNode) -> String {
    format!(
        "branch_id: {}\nselection_entry_id: {}\npayload_index: {}\ncandidate_id: {:?}\nbase_artifact_id: {:?}\nderived_artifact_id: {:?}\npatch_id: {:?}",
        branch.branch_id,
        branch.selection_entry_id.0,
        branch.payload_index,
        branch.candidate_id,
        branch.base_artifact_id,
        branch.derived_artifact_id,
        branch.patch_id
    )
}

fn operation_label(key: &OperationKey) -> String {
    match key {
        OperationKey::HistoryEntry { entry_id } => format!("O:{}", compact_id(&entry_id.0)),
        OperationKey::RuntimeTarget { runtime_id, .. } => {
            format!("O:{}", compact_id(&runtime_id.0))
        }
    }
}

const MAX_PRIMARY_LABEL_CHARS: usize = 12;

fn compact_id(value: &str) -> String {
    let trimmed = value
        .strip_prefix("artifact:")
        .or_else(|| value.strip_prefix("runtime:"))
        .or_else(|| value.strip_prefix("candidate:"))
        .or_else(|| value.strip_prefix("lineage:"))
        .or_else(|| value.strip_prefix("entry:"))
        .or_else(|| value.strip_prefix("block:"))
        .or_else(|| value.strip_prefix("membership:"))
        .or_else(|| value.strip_prefix("patch:"))
        .unwrap_or(value);
    truncate_label(trimmed)
}

fn truncate_label(value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    if value.chars().count() <= MAX_PRIMARY_LABEL_CHARS {
        return value.to_owned();
    }
    let visible_chars = MAX_PRIMARY_LABEL_CHARS.saturating_sub(3);
    let mut chars = value.chars();
    let prefix = chars.by_ref().take(visible_chars).collect::<String>();
    format!("{prefix}...")
}

#[derive(Debug)]
struct ComponentRoot {
    index: NodeIndex,
    kind: &'static str,
    label: String,
}

fn component_roots(raw: &RawGraph) -> Vec<ComponentRoot> {
    let mut visited = std::collections::HashSet::new();
    let mut roots = Vec::new();
    let mut nodes = raw.node_indices().collect::<Vec<_>>();
    nodes.sort_by_key(|node| node.index());

    for node in nodes {
        if visited.contains(&node) {
            continue;
        }
        let mut stack = vec![node];
        let mut component = Vec::new();
        visited.insert(node);
        while let Some(current) = stack.pop() {
            component.push(current);
            for neighbor in raw.neighbors_undirected(current) {
                if visited.insert(neighbor) {
                    stack.push(neighbor);
                }
            }
        }
        component.sort_by_key(|node| node.index());
        let root = component
            .iter()
            .copied()
            .find(|candidate| {
                raw.neighbors_directed(*candidate, Incoming)
                    .all(|incoming| !component.contains(&incoming))
            })
            .unwrap_or(component[0]);
        let payload = &raw[root];
        roots.push(ComponentRoot {
            index: root,
            kind: payload.kind_name(),
            label: payload.label().to_owned(),
        });
    }

    roots
}

fn anchor_unattached_components(
    raw: &mut RawGraph,
    run: NodeIndex,
    components: &[ComponentRoot],
    style: ViewStyle,
) {
    let Some(run_component) = components.iter().find(|component| component.index == run) else {
        return;
    };
    let unattached = components
        .iter()
        .filter(|component| component.index != run_component.index)
        .collect::<Vec<_>>();
    if unattached.is_empty() {
        return;
    }

    let anchor = raw.add_node(GraphNode::Synthetic {
        kind: GraphNodeKind::Unattached,
        label: Arc::from("unattached"),
        detail: Arc::from("synthetic anchor for records not connected by loaded graph facts"),
        color: style.edge.colors.dropped,
    });
    add_edge(
        raw,
        run,
        anchor,
        "unattached",
        true,
        ViewEdgeKind::Synthetic,
        style,
    );
    for component in unattached {
        add_edge(
            raw,
            anchor,
            component.index,
            "unresolved",
            true,
            ViewEdgeKind::Synthetic,
            style,
        );
    }
}

fn to_widget_graph(raw: &RawGraph, style: ViewStyle) -> WidgetGraph {
    egui_graphs::to_graph_custom(
        raw,
        |node: &mut WidgetNode| {
            let visual = NodeVisual::from_node(node.payload());
            node.set_label(visual.label);
            if let Some(color) = visual.color {
                node.set_color(color);
            }
            node.display_mut().radius = style.layout.node_radius;
        },
        |_edge| {},
    )
}

struct NodeVisual {
    label: String,
    color: Option<eframe::egui::Color32>,
}

impl NodeVisual {
    fn from_node(node: &GraphNode) -> Self {
        match node {
            GraphNode::Artifact { label, color, .. }
            | GraphNode::Candidate { label, color, .. }
            | GraphNode::Record { label, color, .. }
            | GraphNode::Synthetic { label, color, .. } => Self {
                label: label.to_string(),
                color: Some(*color),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ploke_records::history::{
        ActorRefRecord, ArtifactRefRecord, ProcedureRefRecord, SurfaceCommitmentRecord,
        SurfaceDeltaRecord, SurfaceRecord, SurfaceRootRecord,
    };
    use ploke_records::ids::{BlockHash, BlockId, HistoryHash, LineageId, RecordedAt, RuntimeId};
    use ploke_tree::Graph;
    use ploke_tree::graph::{
        ArtifactIdentity, ArtifactIndex, ArtifactKey, ArtifactNode, AuthorityIndex, CandidateIndex,
        HistoryBlockNode, HistoryIndex, OpeningAuthorityNode, SelectionIndex, SuccessorNode,
    };

    use ploke_records::history::SubjectRefRecord;
    use ploke_records::ids::{ArtifactId, CandidateId, EntryId};
    use ploke_tree::graph::{CandidateBranchNode, CandidateSource};

    use super::{GraphNode, MAX_PRIMARY_LABEL_CHARS, project_artifact_tree, project_graph};
    use crate::ui::view::ViewStyle;

    #[test]
    fn projection_composes_branch_artifacts_candidates_and_selection() {
        let graph = graph_with_artifact_branch_selection();
        let projected = project_graph(&graph, ViewStyle::default());

        assert_eq!(count_nodes(&projected, "artifact"), 2);
        assert_eq!(count_nodes(&projected, "candidate"), 1);
        assert_eq!(count_nodes(&projected, "selection"), 1);
        assert_eq!(count_nodes(&projected, "branch"), 1);
    }

    #[test]
    fn projection_keeps_history_and_candidates_when_both_are_loaded() {
        let mut graph = graph_with_selected_successor("artifact:parent", "artifact:child", 3);
        let candidate_graph = graph_with_candidate_inventory_selection();
        graph.candidates = candidate_graph.candidates;
        graph.selections = candidate_graph.selections;

        let projected = project_graph(&graph, ViewStyle::default());

        assert_eq!(count_nodes(&projected, "history-block"), 1);
        assert_eq!(count_nodes(&projected, "artifact"), 2);
        assert_eq!(count_nodes(&projected, "candidate"), 2);
        assert_eq!(count_nodes(&projected, "selection"), 1);
    }

    #[test]
    fn projection_adds_unattached_anchor_for_components_without_loaded_relations() {
        let graph = graph_with_candidate_inventory_selection();
        let projected = project_graph(&graph, ViewStyle::default());

        assert!(projected.connectivity.component_count_before_anchoring > 1);
        assert_eq!(count_nodes(&projected, "unattached"), 1);
    }

    #[test]
    fn artifact_tree_does_not_render_unattached_anchor_by_default() {
        let graph = graph_with_candidate_inventory_selection();
        let projected = project_artifact_tree(&graph, ViewStyle::default());

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

        let projected = project_artifact_tree(&graph, ViewStyle::default());

        assert_eq!(projected.raw.node_count(), 0);
        assert_eq!(projected.raw.edge_count(), 0);
        assert_eq!(count_nodes(&projected, "candidate"), 0);
        assert_eq!(count_nodes(&projected, "selection"), 0);
    }

    #[test]
    fn artifact_tree_keeps_debug_record_classes_hidden() {
        let graph = graph_with_artifact_branch_selection();
        let projected = project_artifact_tree(&graph, ViewStyle::default());

        assert_eq!(count_nodes(&projected, "artifact"), 2);
        assert_eq!(projected.raw.edge_count(), 1);
        assert_eq!(count_nodes(&projected, "candidate"), 0);
        assert_eq!(count_nodes(&projected, "selection"), 0);
        assert_eq!(count_nodes(&projected, "branch"), 0);
        assert_eq!(count_nodes(&projected, "membership"), 0);
        assert_eq!(count_nodes(&projected, "operation"), 0);
        assert_eq!(count_nodes(&projected, "evidence"), 0);
        assert_eq!(count_nodes(&projected, "warning"), 0);
    }

    #[test]
    fn artifact_tree_connects_history_artifacts_with_patch_edge() {
        let graph = graph_with_selected_successor("artifact:parent", "artifact:child", 3);
        let projected = project_artifact_tree(&graph, ViewStyle::default());

        assert_eq!(count_nodes(&projected, "artifact"), 2);
        assert_eq!(projected.raw.edge_count(), 1);
        let edge = projected.raw.edge_weights().next().expect("patch edge");
        assert_eq!(edge.label.as_ref(), "P1");
        assert!(edge.label_visible);
        assert_eq!(edge.kind, super::ViewEdgeKind::ArtifactPatch);
        assert_eq!(count_nodes(&projected, "history-block"), 0);
        assert_eq!(count_nodes(&projected, "lineage"), 0);
    }

    #[test]
    fn primary_labels_are_short_handles() {
        let graph = graph_with_candidate_inventory_selection();
        let projected = project_graph(&graph, ViewStyle::default());

        for node in projected.raw.node_weights() {
            assert!(
                node.label().chars().count() <= MAX_PRIMARY_LABEL_CHARS,
                "label should be compact: {}",
                node.label()
            );
        }
    }

    #[test]
    fn full_raw_ids_remain_in_node_detail_text() {
        let graph = graph_with_candidate_inventory_selection();
        let projected = project_graph(&graph, ViewStyle::default());

        assert!(projected.raw.node_weights().any(|node| {
            node.kind_name() == "candidate" && node.detail().contains("candidate:duplicate")
        }));
    }

    #[test]
    fn selected_candidate_color_uses_membership_key() {
        let graph = graph_with_candidate_inventory_selection();
        let projected = project_graph(&graph, ViewStyle::default());
        let selected = projected
            .raw
            .node_weights()
            .find(|node| {
                node.kind_name() == "candidate" && node.detail().contains("membership:selected")
            })
            .expect("selected candidate node");

        assert_eq!(
            node_color(selected),
            Some(ViewStyle::default().edge.colors.selected)
        );
    }

    fn count_nodes(projected: &super::ProjectedGraph, kind: &str) -> usize {
        projected
            .raw
            .node_weights()
            .filter(|node| node.kind_name() == kind)
            .count()
    }

    fn node_color(node: &GraphNode) -> Option<eframe::egui::Color32> {
        match node {
            GraphNode::Artifact { color, .. }
            | GraphNode::Candidate { color, .. }
            | GraphNode::Record { color, .. }
            | GraphNode::Synthetic { color, .. } => Some(*color),
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
                artifact_after: Some(child_id),
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
        let parent_ref = ArtifactRefRecord {
            value: parent.to_owned(),
        };
        let child_ref = ArtifactRefRecord {
            value: child.to_owned(),
        };

        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            ArtifactKey::HistoryRef {
                value: parent.to_owned(),
            },
            ArtifactNode {
                key: ArtifactKey::HistoryRef {
                    value: parent.to_owned(),
                },
                identity: ArtifactIdentity::HistoryRef(parent_ref.clone()),
                evidence: Vec::new(),
            },
        );
        artifacts.insert(
            ArtifactKey::HistoryRef {
                value: child.to_owned(),
            },
            ArtifactNode {
                key: ArtifactKey::HistoryRef {
                    value: child.to_owned(),
                },
                identity: ArtifactIdentity::HistoryRef(child_ref.clone()),
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
