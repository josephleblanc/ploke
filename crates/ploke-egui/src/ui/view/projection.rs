use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{Color32, Vec2};
use petgraph::{Directed, stable_graph::StableGraph};
use ploke_records::branch::TreatmentBranchStatus;
use ploke_tree::Graph as DomainGraph;
use ploke_tree::graph::{
    ArtifactIdentity, ArtifactKey, ArtifactNode, CandidateBranchNode, CandidateNode,
    HistoryBlockNode, SelectionNode,
};

use super::diagnostics::graph_diagnostics;
use super::edge::GraphEdgeShape;
use super::style::{EdgeStyle, ViewStyle};
use super::{EdgeLabelDiagnostics, GraphViewDiagnostics};

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
    graph: WidgetGraph,
}

impl Default for GraphViewCache {
    fn default() -> Self {
        Self {
            signature: None,
            style: ViewStyle::default(),
            graph: to_widget_graph(&RawGraph::default(), ViewStyle::default()),
        }
    }
}

impl GraphViewCache {
    pub(super) fn refresh(&mut self, graph: &DomainGraph, style: ViewStyle) -> bool {
        let signature = GraphSignature::from(graph);
        if self.signature == Some(signature) && self.style == style {
            return false;
        }

        self.signature = Some(signature);
        self.style = style;
        self.graph = build_widget_graph(graph, style);
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
        graph_diagnostics(&self.graph, viewport_size, style, edge_labels)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GraphSignature {
    blocks: usize,
    artifacts: usize,
    candidates: usize,
    selections: usize,
    runtimes: usize,
    operations: usize,
    evidence: usize,
}

impl From<&DomainGraph> for GraphSignature {
    fn from(graph: &DomainGraph) -> Self {
        Self {
            blocks: graph.history.blocks.len(),
            artifacts: graph.artifacts.artifacts.len(),
            candidates: graph.candidates.candidates.len(),
            selections: graph.selections.selections.len(),
            runtimes: graph.runtimes.runtimes.len(),
            operations: graph.operations.operations.len(),
            evidence: graph.evidence.attachments.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum GraphNode {
    Artifact {
        label: Arc<str>,
        status: TreatmentBranchStatus,
    },
    Candidate {
        label: Arc<str>,
        status: TreatmentBranchStatus,
    },
}

#[derive(Debug, Clone)]
pub(super) struct GraphEdgePayload {
    pub(super) label: Arc<str>,
    pub(super) color: Color32,
    pub(super) style: EdgeStyle,
    pub(super) kind: ViewEdgeKind,
    pub(super) status: TreatmentBranchStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ViewEdgeKind {
    ArtifactPatch,
    HistoryArtifact,
}

fn build_widget_graph(graph: &DomainGraph, style: ViewStyle) -> WidgetGraph {
    match project_graph(graph) {
        GraphProjection::ArtifactTree(view) => build_artifact_graph(view, style),
        GraphProjection::CandidateInventory(view) => build_candidate_inventory_graph(view, style),
    }
}

enum GraphProjection<'g> {
    ArtifactTree(ArtifactTreeView<'g>),
    CandidateInventory(CandidateInventoryView<'g>),
}

struct ArtifactTreeView<'g> {
    artifacts: Vec<ArtifactViewNode<'g>>,
    edges: Vec<ArtifactEdge<'g>>,
}

struct ArtifactViewNode<'g> {
    artifact: &'g ArtifactNode,
    status: TreatmentBranchStatus,
}

struct ArtifactEdge<'g> {
    parent: &'g ArtifactNode,
    child: &'g ArtifactNode,
    label: Arc<str>,
    kind: ViewEdgeKind,
    status: TreatmentBranchStatus,
}

struct CandidateInventoryView<'g> {
    candidates: Vec<CandidateViewNode<'g>>,
}

struct CandidateViewNode<'g> {
    candidate: &'g CandidateNode,
    status: TreatmentBranchStatus,
}

fn project_graph(graph: &DomainGraph) -> GraphProjection<'_> {
    let artifact_tree = project_artifact_branch_tree(graph);
    if !artifact_tree.artifacts.is_empty() {
        return GraphProjection::ArtifactTree(artifact_tree);
    }

    let artifact_tree = project_history_artifact_tree(graph);
    if !artifact_tree.artifacts.is_empty() {
        return GraphProjection::ArtifactTree(artifact_tree);
    }

    GraphProjection::CandidateInventory(project_candidate_inventory(graph))
}

fn project_artifact_branch_tree(graph: &DomainGraph) -> ArtifactTreeView<'_> {
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

    let mut artifacts = Vec::new();
    let mut edges = Vec::new();

    for (index, branch) in branches.into_iter().enumerate() {
        let (Some(base_artifact_id), Some(derived_artifact_id)) = (
            branch.base_artifact_id.as_ref(),
            branch.derived_artifact_id.as_ref(),
        ) else {
            continue;
        };

        let parent_key = ArtifactKey::PassiveId {
            value: base_artifact_id.0.clone(),
        };
        let child_key = ArtifactKey::PassiveId {
            value: derived_artifact_id.0.clone(),
        };
        let Some(parent) = graph.artifacts.artifacts.get(&parent_key) else {
            continue;
        };
        let Some(child) = graph.artifacts.artifacts.get(&child_key) else {
            continue;
        };

        let status = branch_status(graph, branch);
        upsert_artifact(
            &mut artifacts,
            ArtifactViewNode {
                artifact: parent,
                status: TreatmentBranchStatus::Synthesized,
            },
        );
        upsert_artifact(
            &mut artifacts,
            ArtifactViewNode {
                artifact: child,
                status,
            },
        );
        edges.push(ArtifactEdge {
            parent,
            child,
            label: Arc::from(format!("P{}", index + 1)),
            kind: ViewEdgeKind::ArtifactPatch,
            status,
        });
    }

    ArtifactTreeView { artifacts, edges }
}

fn project_history_artifact_tree(graph: &DomainGraph) -> ArtifactTreeView<'_> {
    let mut artifacts_by_history_ref = HashMap::new();
    for artifact in graph.artifacts.artifacts.values() {
        if let ArtifactIdentity::HistoryRef(history_ref) = &artifact.identity {
            artifacts_by_history_ref.insert(history_ref.value.as_str(), artifact);
        }
    }

    let mut blocks = graph.history.blocks.values().collect::<Vec<_>>();
    blocks.sort_by(|left, right| {
        (&left.lineage_id.0, left.block_height, &left.block_hash.0).cmp(&(
            &right.lineage_id.0,
            right.block_height,
            &right.block_hash.0,
        ))
    });

    let mut artifacts = Vec::new();
    let mut edges = Vec::new();

    for block in blocks {
        let Some(parent) = artifacts_by_history_ref.get(block.active_artifact.value.as_str())
        else {
            continue;
        };
        let Some(child) =
            artifacts_by_history_ref.get(block.selected_successor.artifact.value.as_str())
        else {
            continue;
        };

        upsert_artifact(
            &mut artifacts,
            ArtifactViewNode {
                artifact: parent,
                status: artifact_status(graph, parent),
            },
        );
        upsert_artifact(
            &mut artifacts,
            ArtifactViewNode {
                artifact: child,
                status: artifact_status(graph, child),
            },
        );
        edges.push(ArtifactEdge {
            parent,
            child,
            label: Arc::from(format!("B{}", edge_block_height(block))),
            kind: ViewEdgeKind::HistoryArtifact,
            status: TreatmentBranchStatus::Selected,
        });
    }

    if artifacts.is_empty() {
        let mut inventory = graph.artifacts.artifacts.values().collect::<Vec<_>>();
        inventory.sort_by(|left, right| left.key.cmp(&right.key));
        for artifact in inventory {
            artifacts.push(ArtifactViewNode {
                artifact,
                status: artifact_status(graph, artifact),
            });
        }
    }

    ArtifactTreeView { artifacts, edges }
}

fn artifact_status(graph: &DomainGraph, artifact: &ArtifactNode) -> TreatmentBranchStatus {
    if graph.history.blocks.values().any(|block| {
        matches!(
            &artifact.identity,
            ArtifactIdentity::HistoryRef(history_ref)
                if history_ref.value == block.selected_successor.artifact.value
        )
    }) {
        TreatmentBranchStatus::Selected
    } else {
        TreatmentBranchStatus::Synthesized
    }
}

fn edge_block_height(block: &HistoryBlockNode) -> u64 {
    block.block_height
}

fn upsert_artifact<'g>(artifacts: &mut Vec<ArtifactViewNode<'g>>, candidate: ArtifactViewNode<'g>) {
    if let Some(existing) = artifacts
        .iter_mut()
        .find(|artifact| artifact.artifact.key == candidate.artifact.key)
    {
        if existing.status != TreatmentBranchStatus::Selected
            && candidate.status == TreatmentBranchStatus::Selected
        {
            existing.status = candidate.status;
        }
        return;
    }
    artifacts.push(candidate);
}

fn branch_status(graph: &DomainGraph, branch: &CandidateBranchNode) -> TreatmentBranchStatus {
    if graph.candidates.candidates.iter().any(|candidate| {
        candidate.branch_id.as_deref() == Some(branch.branch_id.as_str())
            && candidate_status(graph, candidate) == TreatmentBranchStatus::Selected
    }) {
        TreatmentBranchStatus::Selected
    } else {
        TreatmentBranchStatus::Synthesized
    }
}

fn project_candidate_inventory(graph: &DomainGraph) -> CandidateInventoryView<'_> {
    let mut candidates = graph.candidates.candidates.iter().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        (&left.selection_entry_id.0, left.payload_index)
            .cmp(&(&right.selection_entry_id.0, right.payload_index))
    });

    CandidateInventoryView {
        candidates: candidates
            .into_iter()
            .map(|candidate| CandidateViewNode {
                status: candidate_status(graph, candidate),
                candidate,
            })
            .collect(),
    }
}

fn candidate_status(graph: &DomainGraph, candidate: &CandidateNode) -> TreatmentBranchStatus {
    let Some(selection) = graph
        .selections
        .selections
        .get(&candidate.selection_entry_id)
    else {
        return TreatmentBranchStatus::Synthesized;
    };

    if candidate_selected(selection, candidate) {
        TreatmentBranchStatus::Selected
    } else {
        TreatmentBranchStatus::Synthesized
    }
}

fn candidate_selected(selection: &SelectionNode, candidate: &CandidateNode) -> bool {
    if let (Some(root), Some(selected_membership_id), Some(candidate_key)) = (
        selection.candidate_set_root.as_ref(),
        selection.selected_membership_id.as_ref(),
        candidate.membership_key.as_ref(),
    ) {
        if &candidate_key.candidate_set_root == root
            && &candidate_key.membership_id == selected_membership_id
        {
            return true;
        }
    }

    if let (Some(selected_occurrence_id), Some(occurrence_id)) = (
        selection.selected_occurrence_id.as_ref(),
        candidate.occurrence_id.as_ref(),
    ) {
        if occurrence_id == selected_occurrence_id {
            return true;
        }
    }

    selection
        .selected_candidate
        .as_ref()
        .is_some_and(|selected| selected == &candidate.subject)
}

fn build_artifact_graph(view: ArtifactTreeView<'_>, style: ViewStyle) -> WidgetGraph {
    let mut raw = RawGraph::default();
    let mut index_by_artifact = HashMap::<*const ArtifactNode, _>::new();

    for artifact in view.artifacts {
        let index = raw.add_node(GraphNode::Artifact {
            label: Arc::from(artifact_label(artifact.artifact, style)),
            status: artifact.status,
        });
        index_by_artifact.insert(artifact.artifact as *const ArtifactNode, index);
    }

    for edge in view.edges {
        let Some(parent) = index_by_artifact
            .get(&(edge.parent as *const ArtifactNode))
            .copied()
        else {
            continue;
        };
        let Some(child) = index_by_artifact
            .get(&(edge.child as *const ArtifactNode))
            .copied()
        else {
            continue;
        };
        raw.add_edge(
            parent,
            child,
            GraphEdgePayload {
                label: edge.label,
                color: style.edge.colors.color(edge.status),
                style: style.edge,
                kind: edge.kind,
                status: edge.status,
            },
        );
    }

    to_widget_graph(&raw, style)
}

fn build_candidate_inventory_graph(
    view: CandidateInventoryView<'_>,
    style: ViewStyle,
) -> WidgetGraph {
    let mut raw = RawGraph::default();

    for (index, candidate) in view.candidates.into_iter().enumerate() {
        raw.add_node(GraphNode::Candidate {
            label: Arc::from(candidate_label(candidate.candidate, index)),
            status: candidate.status,
        });
    }

    to_widget_graph(&raw, style)
}

fn artifact_label(artifact: &ArtifactNode, style: ViewStyle) -> String {
    match &artifact.identity {
        ArtifactIdentity::HistoryRef(history_ref) => {
            style.labels.artifact(history_ref.value.as_str())
        }
        ArtifactIdentity::PassiveId(artifact_id) => style.labels.artifact(artifact_id.0.as_str()),
    }
}

fn candidate_label(candidate: &CandidateNode, index: usize) -> String {
    if let Some(node_id) = candidate.node_id.as_ref() {
        if let Some(generation) = candidate.generation {
            return format!("{node_id} g{generation}");
        }
        return node_id.clone();
    }

    if let Some(generation) = candidate.generation {
        return format!("C{} g{}", index + 1, generation);
    }

    candidate.subject.value.clone()
}

fn to_widget_graph(raw: &RawGraph, style: ViewStyle) -> WidgetGraph {
    egui_graphs::to_graph_custom(
        raw,
        |node: &mut WidgetNode| {
            let visual = NodeVisual::from_node(node.payload(), style);
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
    fn from_node(node: &GraphNode, style: ViewStyle) -> Self {
        match node {
            GraphNode::Artifact { label, status } | GraphNode::Candidate { label, status } => {
                Self {
                    label: label.to_string(),
                    color: Some(style.edge.colors.color(*status)),
                }
            }
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

    use super::{GraphProjection, project_graph};

    #[test]
    fn projection_prefers_artifact_branch_tree_when_branch_derivation_exists() {
        let graph = graph_with_artifact_branch();

        match project_graph(&graph) {
            GraphProjection::ArtifactTree(view) => {
                assert_eq!(view.artifacts.len(), 2);
                assert_eq!(view.edges.len(), 1);
                assert_eq!(view.edges[0].label.as_ref(), "P1");
            }
            GraphProjection::CandidateInventory(_) => {
                panic!("artifact branch view should be the first viewer projection")
            }
        }
    }

    #[test]
    fn projection_falls_back_to_history_artifact_tree_when_branch_derivation_is_absent() {
        let graph = graph_with_selected_successor("artifact:parent", "artifact:child", 3);

        match project_graph(&graph) {
            GraphProjection::ArtifactTree(view) => {
                assert_eq!(view.artifacts.len(), 2);
                assert_eq!(view.edges.len(), 1);
                assert_eq!(view.edges[0].label.as_ref(), "B3");
                assert_eq!(view.edges[0].kind, super::ViewEdgeKind::HistoryArtifact);
            }
            GraphProjection::CandidateInventory(_) => {
                panic!("history artifact tree should be the fallback artifact projection")
            }
        }
    }

    #[test]
    fn projection_falls_back_to_candidate_inventory_without_history_artifacts() {
        let mut graph = Graph::default();
        graph
            .candidates
            .candidates
            .push(ploke_tree::graph::CandidateNode {
                selection_entry_id: ploke_records::ids::EntryId("entry-1".to_owned()),
                payload_index: 0,
                subject: ploke_records::history::SubjectRefRecord {
                    value: "candidate:1".to_owned(),
                },
                source: None,
                occurrence_id: None,
                membership_id: None,
                membership_key: None,
                node_id: Some("node-1".to_owned()),
                branch_id: None,
                generation: Some(1),
                primary_runtime_id: None,
                artifact_after: None,
                patch_id: None,
                evidence: Vec::new(),
            });

        match project_graph(&graph) {
            GraphProjection::CandidateInventory(view) => {
                assert_eq!(view.candidates.len(), 1);
            }
            GraphProjection::ArtifactTree(_) => {
                panic!("candidate inventory should be the fallback projection")
            }
        }
    }

    fn graph_with_artifact_branch() -> Graph {
        let mut graph = Graph::default();

        let parent_id = ArtifactId("artifact:parent".to_owned());
        let child_id = ArtifactId("artifact:child".to_owned());

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
            selection_entry_id: EntryId("entry-1".to_owned()),
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
                selection_entry_id: EntryId("entry-1".to_owned()),
                payload_index: 0,
                subject: SubjectRefRecord {
                    value: "candidate:1".to_owned(),
                },
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
