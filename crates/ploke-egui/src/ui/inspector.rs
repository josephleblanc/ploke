//! Graph-resolved selection inspector projections.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::ui::text::decor::Badge;
use crate::ui::view::{GraphSelectionDetail, GraphSelectionRef};
use ploke_records::ids::ArtifactId;
use ploke_tree::graph::{
    ArtifactKey, ParentCreateAttempt, ParentCreateLookup, ParentCreateUnavailable,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct GraphRevision(u64);

impl GraphRevision {
    pub(crate) fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

#[derive(Debug, Default)]
pub(crate) struct InspectorCache {
    key: Option<InspectorCacheKey>,
    sections: InspectorSections,
    rebuilds: usize,
}

impl InspectorCache {
    pub(crate) fn sections(
        &mut self,
        graph: &ploke_tree::Graph,
        revision: GraphRevision,
        selection: Option<&GraphSelectionRef>,
    ) -> Option<&InspectorSections> {
        let Some(selection) = selection else {
            self.key = None;
            self.sections = InspectorSections::default();
            return None;
        };

        if !self
            .key
            .as_ref()
            .is_some_and(|key| key.revision == revision && key.selection == *selection)
        {
            let inspector = SelectionInspector::from_reference(graph, selection);
            self.sections = InspectorSections::from_inspector(graph, selection, &inspector);
            self.key = Some(InspectorCacheKey {
                revision,
                selection: selection.clone(),
            });
            self.rebuilds += 1;
        }

        Some(&self.sections)
    }

    #[cfg(test)]
    fn rebuilds(&self) -> usize {
        self.rebuilds
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectorCacheKey {
    revision: GraphRevision,
    selection: GraphSelectionRef,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct InspectorSections {
    identity: Option<IdentitySlot>,
    metrics: Option<MetricsSlot>,
    parent_create: Option<ParentCreateSlot>,
    roles: Vec<BadgeSlot>,
    run_records: Vec<RunRecordSlot>,
    incoming: Vec<ArtifactRelationSlot>,
    outgoing: Vec<ArtifactRelationSlot>,
    artifact_incoming: Vec<ArtifactRelationSlot>,
    artifact_outgoing: Vec<ArtifactRelationSlot>,
    patches: Vec<PatchSlot>,
    source_refs: Vec<SourceRefSlot>,
    unavailable: Option<UnavailableReason>,
}

impl InspectorSections {
    fn from_inspector(
        _graph: &ploke_tree::Graph,
        selection: &GraphSelectionRef,
        inspector: &SelectionInspector<'_>,
    ) -> Self {
        match inspector {
            SelectionInspector::RunForestNode(run) => Self::from_run_forest(run),
            SelectionInspector::Artifact(artifact) => Self::from_artifact(artifact),
            SelectionInspector::Unresolved(reason) => Self {
                identity: Some(IdentitySlot::from_selection(selection)),
                metrics: None,
                parent_create: ParentCreateSlot::from_selection(selection),
                roles: Vec::new(),
                run_records: Vec::new(),
                incoming: Vec::new(),
                outgoing: Vec::new(),
                artifact_incoming: Vec::new(),
                artifact_outgoing: Vec::new(),
                patches: Vec::new(),
                source_refs: Vec::new(),
                unavailable: Some(*reason),
            },
        }
    }

    fn from_run_forest(run: &RunForestNodeInspection<'_>) -> Self {
        let node_key = run.node.key.as_str().to_owned();
        let incoming = run
            .parent
            .map(|parent| {
                ArtifactRelationSlot::new(
                    EdgeRelation::RunForest,
                    parent.key.as_str(),
                    run.node.key.as_str(),
                    1,
                )
            })
            .into_iter()
            .collect();
        let outgoing = run
            .children
            .iter()
            .map(|child| {
                ArtifactRelationSlot::new(
                    EdgeRelation::RunForest,
                    run.node.key.as_str(),
                    child.as_str(),
                    1,
                )
            })
            .collect();
        let artifact_outgoing = run
            .node
            .base_artifact_id
            .as_deref()
            .zip(run.node.derived_artifact_id.as_deref())
            .into_iter()
            .map(|(from, to)| ArtifactRelationSlot::new(EdgeRelation::AppliedPatch, from, to, 1))
            .collect();

        Self {
            identity: Some(IdentitySlot::RunForestNode {
                node_key: node_key.clone(),
            }),
            metrics: Some(MetricsSlot::RunForestNode {
                node_key: node_key.clone(),
            }),
            parent_create: Some(ParentCreateSlot::RunForestNode {
                node_key: node_key.clone(),
            }),
            roles: run.role_badges.iter().map(BadgeSlot::from_badge).collect(),
            run_records: run
                .run_records
                .iter()
                .map(RunRecordSlot::from_inspection)
                .collect(),
            incoming,
            outgoing,
            artifact_incoming: Vec::new(),
            artifact_outgoing,
            patches: run.patch.iter().map(PatchSlot::from_inspection).collect(),
            source_refs: run_forest_source_slots(run.node),
            unavailable: None,
        }
    }

    fn from_artifact(artifact: &ArtifactInspection<'_>) -> Self {
        let artifact_sources = artifact
            .sources
            .iter()
            .map(|source| ArtifactSourceSlot {
                key: source.key.clone(),
            })
            .collect::<Vec<_>>();
        let incoming = artifact
            .incoming
            .iter()
            .map(ArtifactRelationSlot::from_relation)
            .collect::<Vec<_>>();
        let outgoing = artifact
            .outgoing
            .iter()
            .map(ArtifactRelationSlot::from_relation)
            .collect::<Vec<_>>();

        Self {
            identity: Some(IdentitySlot::Artifact {
                sources: artifact_sources.clone(),
            }),
            metrics: Some(MetricsSlot::Artifact {
                sources: artifact_sources.clone(),
            }),
            parent_create: Some(ParentCreateSlot::Artifact {
                key: artifact.identity().artifact().to_owned(),
            }),
            roles: artifact
                .role_badges
                .iter()
                .map(BadgeSlot::from_badge)
                .collect(),
            run_records: artifact
                .run_records
                .iter()
                .map(RunRecordSlot::from_inspection)
                .collect(),
            incoming: incoming.clone(),
            outgoing: outgoing.clone(),
            artifact_incoming: incoming,
            artifact_outgoing: outgoing,
            patches: artifact
                .patches
                .iter()
                .map(PatchSlot::from_inspection)
                .collect(),
            source_refs: artifact_source_slots(artifact.sources.as_slice()),
            unavailable: None,
        }
    }

    pub(crate) fn identity(&self) -> Option<&IdentitySlot> {
        self.identity.as_ref()
    }

    pub(crate) fn metrics(&self) -> Option<&MetricsSlot> {
        self.metrics.as_ref()
    }

    pub(crate) fn parent_create(&self) -> Option<&ParentCreateSlot> {
        self.parent_create.as_ref()
    }

    pub(crate) fn roles(&self) -> &[BadgeSlot] {
        self.roles.as_slice()
    }

    pub(crate) fn run_records(&self) -> &[RunRecordSlot] {
        self.run_records.as_slice()
    }

    pub(crate) fn graph_edges_in(&self) -> &[ArtifactRelationSlot] {
        self.incoming.as_slice()
    }

    pub(crate) fn graph_edges_out(&self) -> &[ArtifactRelationSlot] {
        self.outgoing.as_slice()
    }

    pub(crate) fn artifact_edges_in(&self) -> &[ArtifactRelationSlot] {
        self.artifact_incoming.as_slice()
    }

    pub(crate) fn artifact_edges_out(&self) -> &[ArtifactRelationSlot] {
        self.artifact_outgoing.as_slice()
    }

    pub(crate) fn patches(&self) -> &[PatchSlot] {
        self.patches.as_slice()
    }

    pub(crate) fn source_refs(&self) -> &[SourceRefSlot] {
        self.source_refs.as_slice()
    }

    pub(crate) fn unavailable(&self) -> Option<UnavailableReason> {
        self.unavailable
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IdentitySlot {
    RunForestNode { node_key: String },
    Artifact { sources: Vec<ArtifactSourceSlot> },
}

impl IdentitySlot {
    fn from_selection(selection: &GraphSelectionRef) -> Self {
        match selection {
            GraphSelectionRef::RunForestNode { key } => Self::RunForestNode {
                node_key: key.clone(),
            },
            GraphSelectionRef::Artifact { key } => Self::Artifact {
                sources: vec![ArtifactSourceSlot {
                    key: ArtifactKey::PassiveId { value: key.clone() },
                }],
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MetricsSlot {
    RunForestNode { node_key: String },
    Artifact { sources: Vec<ArtifactSourceSlot> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParentCreateSlot {
    RunForestNode { node_key: String },
    Artifact { key: String },
}

impl ParentCreateSlot {
    fn from_selection(selection: &GraphSelectionRef) -> Option<Self> {
        match selection {
            GraphSelectionRef::RunForestNode { key } => Some(Self::RunForestNode {
                node_key: key.clone(),
            }),
            GraphSelectionRef::Artifact { key } => Some(Self::Artifact { key: key.clone() }),
        }
    }

    pub(crate) fn resolve<'g>(&self, graph: &'g ploke_tree::Graph) -> ParentCreateLookup<'g, '_> {
        match self {
            Self::RunForestNode { node_key } => graph.parent_create_for_node_id(node_key.as_str()),
            Self::Artifact { key } => graph.parent_create_for_artifact_key(key.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactSourceSlot {
    /// archaeology:artifact-identity
    /// proof:docs/active/archaeology/ploke-tree-graph/artifact-identity.md
    pub(crate) key: ArtifactKey,
}

impl ArtifactSourceSlot {
    pub(crate) fn resolve<'g>(
        &self,
        graph: &'g ploke_tree::Graph,
    ) -> Option<&'g ploke_tree::graph::ArtifactNode> {
        graph.artifacts.artifacts.get(&self.key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BadgeRole {
    Parent,
    Child,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BadgeSlot {
    /// archaeology:artifact-identity
    /// proof:docs/active/archaeology/ploke-tree-graph/artifact-identity.md
    pub(crate) artifact_id: ArtifactId,
    pub(crate) role: BadgeRole,
}

impl BadgeSlot {
    fn from_badge(badge: &Badge<'_>) -> Self {
        match badge {
            Badge::Parent(artifact_id) => Self {
                artifact_id: (*artifact_id).clone(),
                role: BadgeRole::Parent,
            },
            Badge::Child(artifact_id) => Self {
                artifact_id: (*artifact_id).clone(),
                role: BadgeRole::Child,
            },
        }
    }

    pub(crate) fn badge(&self) -> Badge<'_> {
        match self.role {
            BadgeRole::Parent => Badge::Parent(&self.artifact_id),
            BadgeRole::Child => Badge::Child(&self.artifact_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactRelationSlot {
    /// archaeology:artifact-relations
    /// proof:docs/active/archaeology/ploke-tree-graph/artifact-relations.md
    pub(crate) relation: EdgeRelation,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) source_count: usize,
}

impl ArtifactRelationSlot {
    fn new(relation: EdgeRelation, from: &str, to: &str, source_count: usize) -> Self {
        Self {
            relation,
            from: from.to_owned(),
            to: to.to_owned(),
            source_count,
        }
    }

    fn from_relation(relation: &ArtifactRelation<'_>) -> Self {
        Self::new(
            relation.kind.edge_relation(),
            relation.from,
            relation.to,
            relation.source_count,
        )
    }

    pub(crate) fn edge(&self) -> SelectionEdge<'_> {
        SelectionEdge::new(
            self.relation,
            self.from.as_str(),
            self.to.as_str(),
            self.source_count,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatchSlot {
    /// archaeology:run-record-branch-output
    /// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
    pub(crate) child_node_id: String,
}

impl PatchSlot {
    fn from_inspection(patch: &PatchInspection<'_>) -> Self {
        Self {
            child_node_id: patch.child.node.node_id.as_str().to_owned(),
        }
    }

    pub(crate) fn resolve<'g>(&self, graph: &'g ploke_tree::Graph) -> Option<PatchInspection<'g>> {
        graph
            .child_plans
            .child_for_node_id(self.child_node_id.as_str())
            .map(|child| PatchInspection { child })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunRecordSlot {
    /// archaeology:run-record-branch-output
    /// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
    pub(crate) branch_id: String,
    pub(crate) record_key: String,
}

impl RunRecordSlot {
    fn from_inspection(record: RunRecordInspection<'_>) -> Self {
        Self {
            branch_id: record.record_ref.branch_id.clone(),
            record_key: record.record_ref.record_key.clone(),
        }
    }

    pub(crate) fn resolve<'g>(
        &self,
        graph: &'g ploke_tree::Graph,
    ) -> Option<RunRecordInspection<'g>> {
        let evidence = graph.run_records()?;
        let record_ref = evidence
            .refs_by_branch
            .get(self.branch_id.as_str())?
            .iter()
            .find(|record_ref| record_ref.record_key == self.record_key)?;
        evidence
            .index
            .get(&self.record_key)
            .zip(evidence.stats.get(&self.record_key))
            .map(|(record, stats)| RunRecordInspection {
                record_ref,
                record,
                stats,
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceRefSlot {
    Evidence { node_key: String, index: usize },
    Diagnostic { node_key: String, index: usize },
    Artifact { key: ArtifactKey },
}

impl SourceRefSlot {
    pub(crate) fn resolve<'g>(&self, graph: &'g ploke_tree::Graph) -> Option<SourceRef<'g>> {
        match self {
            Self::Evidence { node_key, index } => find_run_forest_node(graph, node_key)
                .and_then(|node| node.evidence.get(*index))
                .map(|evidence| SourceRef::Evidence {
                    kind: evidence_kind_label(evidence.kind),
                    authority: authority_label(evidence.authority),
                    recorded_at: evidence.recorded_at.as_deref(),
                }),
            Self::Diagnostic { node_key, index } => find_run_forest_node(graph, node_key)
                .and_then(|node| node.diagnostics.get(*index))
                .map(|diagnostic| SourceRef::Diagnostic {
                    severity: diagnostic_severity_label(diagnostic.severity),
                    code: diagnostic.code.as_str(),
                }),
            Self::Artifact { key } => {
                let source = graph.artifacts.artifacts.get(key)?;
                Some(match &source.identity {
                    ploke_tree::graph::ArtifactIdentity::HistoryRef(artifact) => {
                        SourceRef::ArtifactHistoryRef {
                            artifact: artifact.as_str(),
                        }
                    }
                    ploke_tree::graph::ArtifactIdentity::PassiveId(artifact) => {
                        SourceRef::ArtifactId {
                            artifact: artifact.0.as_str(),
                        }
                    }
                })
            }
        }
    }
}

fn run_forest_source_slots(node: &ploke_tree::TreeNode) -> Vec<SourceRefSlot> {
    let node_key = node.key.as_str().to_owned();
    node.evidence
        .iter()
        .enumerate()
        .map(|(index, _)| SourceRefSlot::Evidence {
            node_key: node_key.clone(),
            index,
        })
        .chain(
            node.diagnostics
                .iter()
                .enumerate()
                .map(|(index, _)| SourceRefSlot::Diagnostic {
                    node_key: node_key.clone(),
                    index,
                }),
        )
        .collect()
}

fn artifact_source_slots(sources: &[&ploke_tree::graph::ArtifactNode]) -> Vec<SourceRefSlot> {
    sources
        .iter()
        .map(|source| SourceRefSlot::Artifact {
            key: source.key.clone(),
        })
        .collect()
}

pub(crate) fn find_run_forest_node<'g>(
    graph: &'g ploke_tree::Graph,
    key: &str,
) -> Option<&'g ploke_tree::TreeNode> {
    graph
        .forest
        .as_ref()?
        .nodes
        .iter()
        .find(|node| node.key.as_str() == key)
}

#[derive(Debug, Clone)]
pub enum SelectionInspector<'g> {
    RunForestNode(RunForestNodeInspection<'g>),
    Artifact(ArtifactInspection<'g>),
    Unresolved(UnavailableReason),
}

impl<'g> SelectionInspector<'g> {
    pub(crate) fn from_graph(
        graph: &'g ploke_tree::Graph,
        selection: &GraphSelectionDetail,
    ) -> Self {
        Self::from_reference(graph, &selection.reference)
    }

    pub(crate) fn from_reference(
        graph: &'g ploke_tree::Graph,
        reference: &GraphSelectionRef,
    ) -> Self {
        match reference {
            GraphSelectionRef::RunForestNode { key } => run_forest_node_inspection(graph, key),
            GraphSelectionRef::Artifact { key } => artifact_inspection(graph, key),
        }
    }

    pub fn snapshot<'s>(self, selection: &'s GraphSelectionDetail) -> SelectionInspectorSnapshot<'s>
    where
        'g: 's,
    {
        SelectionInspectorSnapshot::from_inspection(selection, self)
    }

    pub fn from_default_selector(
        graph: &'g ploke_tree::Graph,
        selector: &str,
    ) -> Option<(GraphSelectionDetail, Self)> {
        default_selections(graph)
            .into_iter()
            .find(|selection| {
                selection.label == selector || selection.reference.matches_key(selector)
            })
            .or_else(|| graph_selection_by_key(graph, selector))
            .map(|selection| {
                let inspector = Self::from_graph(graph, &selection);
                (selection, inspector)
            })
    }

    /// archaeology:artifact-identity
    /// proof:docs/active/archaeology/ploke-tree-graph/artifact-identity.md
    pub fn artifact_ids_section(&self) -> ArtifactIdsSectionSnapshot<'g> {
        match self {
            SelectionInspector::RunForestNode(run) => ArtifactIdsSectionSnapshot {
                selection_kind: ArtifactIdsSelectionKind::RunForestNode,
                state: ArtifactIdsSectionState::NotApplicable {
                    selection_key: run.node.key.as_str(),
                },
            },
            SelectionInspector::Artifact(artifact) => {
                let identity = artifact.identity();
                ArtifactIdsSectionSnapshot {
                    selection_kind: ArtifactIdsSelectionKind::Artifact,
                    state: ArtifactIdsSectionState::Rendered {
                        selection_key: identity.artifact(),
                        artifact_ids: identity
                            .artifact_ids()
                            .map(|artifact_id| artifact_id.0.as_str())
                            .collect(),
                        artifact_refs: identity
                            .artifact_refs()
                            .map(|artifact_ref| artifact_ref.as_str())
                            .collect(),
                        tree_keys: identity
                            .tree_keys()
                            .map(|tree_key| tree_key.hash.0.as_str())
                            .collect(),
                    },
                }
            }
            SelectionInspector::Unresolved(reason) => ArtifactIdsSectionSnapshot {
                selection_kind: ArtifactIdsSelectionKind::Unresolved,
                state: ArtifactIdsSectionState::Unavailable(*reason),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunForestNodeInspection<'g> {
    pub node: &'g ploke_tree::TreeNode,
    pub parent: Option<&'g ploke_tree::TreeNode>,
    pub children: &'g [ploke_tree::NodeKey],
    pub patch: Option<PatchInspection<'g>>,
    pub parent_create: ParentCreateLookup<'g, 'g>,
    pub role_badges: Vec<Badge<'g>>,
    /// archaeology:run-record-branch-output
    /// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
    pub run_records: RunRecordBranchInspection<'g>,
}

#[derive(Debug, Clone)]
pub struct ArtifactInspection<'g> {
    pub sources: Vec<&'g ploke_tree::graph::ArtifactNode>,
    pub incoming: Vec<ArtifactRelation<'g>>,
    pub outgoing: Vec<ArtifactRelation<'g>>,
    pub patches: Vec<PatchInspection<'g>>,
    pub parent_create: ParentCreateLookup<'g, 'g>,
    pub role_badges: Vec<Badge<'g>>,
    /// archaeology:run-record-branch-output
    /// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
    pub run_records: RunRecordBranchInspection<'g>,
}

#[derive(Debug, Clone, Copy)]
pub struct RunRecordInspection<'g> {
    pub record_ref: &'g ploke_tree::BranchRunRecordRef,
    pub record: &'g ploke_records::run_record::RunRecord,
    pub stats: &'g ploke_tree::RunRecordStats,
}

/// archaeology:run-record-branch-output
/// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
#[derive(Debug, Clone, Copy)]
pub struct RunRecordBranchInspection<'g> {
    evidence: Option<&'g ploke_tree::RunRecordEvidence>,
    refs: &'g [ploke_tree::BranchRunRecordRef],
}

impl<'g> RunRecordBranchInspection<'g> {
    pub(crate) fn empty() -> Self {
        Self {
            evidence: None,
            refs: &[],
        }
    }

    pub(crate) fn from_graph(graph: &'g ploke_tree::Graph, branch_id: &str) -> Self {
        let Some(evidence) = graph.run_records() else {
            return Self::empty();
        };
        let refs = evidence
            .refs_by_branch
            .get(branch_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        Self {
            evidence: Some(evidence),
            refs,
        }
    }

    pub(crate) fn iter(self) -> impl Iterator<Item = RunRecordInspection<'g>> + 'g {
        let evidence = self.evidence;
        self.refs.iter().filter_map(move |record_ref| {
            evidence.and_then(|evidence| {
                evidence
                    .index
                    .get(&record_ref.record_key)
                    .zip(evidence.stats.get(&record_ref.record_key))
                    .map(|(record, stats)| RunRecordInspection {
                        record_ref,
                        record,
                        stats,
                    })
            })
        })
    }
}

impl<'g> ArtifactInspection<'g> {
    pub(crate) fn identity(&self) -> ArtifactIdentityWitness<'_, 'g> {
        ArtifactIdentityWitness {
            sources: self.sources.as_slice(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactIdentityWitness<'a, 'g> {
    pub sources: &'a [&'g ploke_tree::graph::ArtifactNode],
}

impl<'a, 'g> ArtifactIdentityWitness<'a, 'g> {
    pub(crate) fn artifact(self) -> &'g str {
        let Some(label) = self
            .sources
            .first()
            .map(|source| artifact_node_label_for_render(source))
        else {
            return "missing_artifact_identity";
        };
        debug_assert!(
            self.sources
                .iter()
                .all(|source| artifact_node_label_for_render(source) == label)
        );
        label
    }

    pub(crate) fn artifact_ids(
        self,
    ) -> impl Iterator<Item = &'g ploke_records::ids::ArtifactId> + 'a {
        self.bundle()
            .into_iter()
            .flat_map(|source| source.artifact_ids().iter())
    }

    pub(crate) fn artifact_refs(
        self,
    ) -> impl Iterator<Item = &'g ploke_records::history::ArtifactRefRecord> + 'a {
        self.bundle()
            .into_iter()
            .flat_map(|source| source.artifact_refs().iter())
    }

    pub(crate) fn tree_keys(
        self,
    ) -> impl Iterator<Item = &'g ploke_records::history::TreeKeyHashRecord> + 'a {
        self.bundle()
            .into_iter()
            .flat_map(|source| source.tree_keys().iter())
    }

    fn bundle(self) -> Option<&'g ploke_tree::graph::ArtifactNode> {
        self.sources.first().copied()
    }
}

/// archaeology:artifact-identity
/// proof:docs/active/archaeology/ploke-tree-graph/artifact-identity.md
pub(crate) fn artifact_node_label_for_render(source: &ploke_tree::graph::ArtifactNode) -> &str {
    source
        .artifact_ids()
        .first()
        .map(|artifact| artifact.0.as_str())
        .or_else(|| {
            source
                .artifact_refs()
                .first()
                .map(|artifact| artifact.as_str())
        })
        .unwrap_or_else(|| artifact_identity_label(&source.identity))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRelation<'g> {
    pub kind: ArtifactRelationKind,
    pub from: &'g str,
    pub to: &'g str,
    pub source_count: usize,
    pub patch_ids: Vec<&'g ploke_records::ids::PatchId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatchInspection<'g> {
    pub child: &'g ploke_records::child_plan::ChildPlanChildRecord,
}

impl<'g> PatchInspection<'g> {
    pub(crate) fn patch_id(self) -> &'g str {
        self.child
            .surface
            .as_ref()
            .map(|surface| surface.patch_id.0.as_str())
            .or_else(|| {
                self.child
                    .node
                    .patch_id
                    .as_ref()
                    .map(|patch_id| patch_id.0.as_str())
            })
            .unwrap_or("not_recorded")
    }

    pub(crate) fn target_relpath(self) -> &'g str {
        self.child
            .resolved
            .target_relpath
            .to_str()
            .unwrap_or("non_utf8_path")
    }

    pub(crate) fn branch_id(self) -> &'g str {
        self.child.resolved.branch.branch_id.as_str()
    }

    pub(crate) fn candidate_id(self) -> &'g str {
        self.child.resolved.branch.candidate_id.as_str()
    }

    pub(crate) fn source_content_hash(self) -> &'g str {
        self.child.resolved.source_content_hash.as_str()
    }

    pub(crate) fn proposed_content_hash(self) -> &'g str {
        self.child.resolved.branch.proposed_content_hash.as_str()
    }

    pub(crate) fn base_artifact(self) -> Option<&'g str> {
        self.child
            .surface
            .as_ref()
            .map(|surface| surface.base.artifact_id.0.as_str())
    }

    pub(crate) fn derived_artifact(self) -> Option<&'g str> {
        self.child
            .surface
            .as_ref()
            .map(|surface| surface.after.artifact_id.0.as_str())
    }

    pub(crate) fn check_status(self) -> Option<ploke_records::history::SurfaceCheckStatusRecord> {
        self.child
            .surface
            .as_ref()
            .map(|surface| surface.check_status)
    }

    pub(crate) fn apply_status(self) -> Option<ploke_records::history::SurfaceApplyStatusRecord> {
        self.child
            .surface
            .as_ref()
            .map(|surface| surface.apply_status)
    }

    pub(crate) fn source_content(self) -> &'g str {
        self.child.resolved.source_content.as_str()
    }

    pub(crate) fn proposed_content(self) -> &'g str {
        self.child.resolved.branch.proposed_content.as_str()
    }

    pub(crate) fn touches(self) -> impl Iterator<Item = PatchTouch<'g>> + 'g {
        self.child.surface.iter().flat_map(|surface| {
            surface
                .touches
                .iter()
                .enumerate()
                .map(|(index, touch)| PatchTouch {
                    index: index + 1,
                    relpath: touch.span_relpath.to_str().unwrap_or("non_utf8_path"),
                    start: touch.start,
                    end: touch.end,
                    replacement: touch.replacement.as_str(),
                })
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactRelationKind {
    HistoryPatch,
    ProducedChild,
    HistoryOpenedFrom,
    AppliedPatch,
}

impl ArtifactRelationKind {
    fn edge_relation(self) -> EdgeRelation {
        match self {
            Self::HistoryPatch => EdgeRelation::HistoryPatch,
            Self::ProducedChild => EdgeRelation::ProducedChild,
            Self::HistoryOpenedFrom => EdgeRelation::HistoryOpenedFrom,
            Self::AppliedPatch => EdgeRelation::AppliedPatch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    RunForestNotPresent,
    RunForestNodeNotFound,
    ArtifactNotFound,
}

impl UnavailableReason {
    pub(crate) fn subject(self) -> &'static str {
        match self {
            Self::RunForestNotPresent => "run forest",
            Self::RunForestNodeNotFound => "run forest node",
            Self::ArtifactNotFound => "artifact",
        }
    }

    pub(crate) fn state(self) -> &'static str {
        match self {
            Self::RunForestNotPresent => "not_present",
            Self::RunForestNodeNotFound | Self::ArtifactNotFound => "not_found",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SelectionInspectorSnapshot<'a> {
    pub kind: &'a str,
    pub label: &'a str,
    pub identity: Option<SelectionIdentity<'a>>,
    pub roles: Vec<Badge<'a>>,
    pub metrics: Option<SelectionMetrics>,
    pub parent_create: Option<ParentCreateSnapshot<'a>>,
    /// archaeology:run-record-branch-output
    /// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
    pub run_records: Vec<RunRecordSnapshot<'a>>,
    pub incoming: Vec<SelectionEdge<'a>>,
    pub outgoing: Vec<SelectionEdge<'a>>,
    pub artifact_incoming: Vec<SelectionEdge<'a>>,
    pub artifact_outgoing: Vec<SelectionEdge<'a>>,
    pub patches: Vec<PatchSnapshot<'a>>,
    pub source_refs: Vec<SourceRef<'a>>,
    pub unavailable: Option<UnavailableReason>,
}

impl<'a> SelectionInspectorSnapshot<'a> {
    fn from_inspection<'g>(
        selection: &'a GraphSelectionDetail,
        inspector: SelectionInspector<'g>,
    ) -> Self
    where
        'g: 'a,
    {
        match inspector {
            SelectionInspector::RunForestNode(run) => snapshot_run_forest(selection, run),
            SelectionInspector::Artifact(artifact) => snapshot_artifact(selection, artifact),
            SelectionInspector::Unresolved(reason) => Self {
                kind: selection.kind.as_str(),
                label: selection.label.as_str(),
                identity: None,
                roles: Vec::new(),
                metrics: None,
                parent_create: None,
                run_records: Vec::new(),
                incoming: Vec::new(),
                outgoing: Vec::new(),
                artifact_incoming: Vec::new(),
                artifact_outgoing: Vec::new(),
                patches: Vec::new(),
                source_refs: Vec::new(),
                unavailable: Some(reason),
            },
        }
    }

    pub(crate) fn has_record_refs(&self) -> bool {
        !self.source_refs.is_empty()
    }

    pub(crate) fn has_drilldown_candidates(&self) -> bool {
        self.parent_create
            .as_ref()
            .is_some_and(|parent_create| parent_create.state == ParentCreateState::Available)
    }

    pub fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("selection: {} {}\n", self.kind, self.label));
        render_identity(&mut out, self.identity.as_ref());
        render_badges(&mut out, "roles", &self.roles);
        render_metrics(&mut out, self.metrics.as_ref());
        render_parent_create(&mut out, self.parent_create.as_ref());
        render_run_records(&mut out, &self.run_records);
        render_edges(&mut out, "incoming", &self.incoming);
        render_edges(&mut out, "outgoing", &self.outgoing);
        render_edges(&mut out, "artifact_incoming", &self.artifact_incoming);
        render_edges(&mut out, "artifact_outgoing", &self.artifact_outgoing);
        render_patches(&mut out, &self.patches);
        if self.source_refs.is_empty() {
            out.push_str("source_refs: none\n");
        } else {
            out.push_str("source_refs:\n");
            for source_ref in &self.source_refs {
                render_source_ref(&mut out, source_ref);
            }
        }
        render_unavailable(&mut out, self.unavailable);
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionIdentity<'a> {
    RunForestNode(RunForestNodeIdentity<'a>),
    Artifact(ArtifactSelectionIdentity<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RunForestNodeIdentity<'a> {
    pub node_key: &'a str,
    pub candidate_id: &'a str,
    pub source_artifact: &'a str,
    pub parent_node: Option<&'a str>,
    pub base_artifact: Option<&'a str>,
    pub derived_artifact: Option<&'a str>,
    pub patch: Option<&'a str>,
    pub branch_id: &'a str,
    pub target_relpath: &'a str,
    pub phase: ploke_tree::Phase,
    pub result: ploke_tree::ResultClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ArtifactSelectionIdentity<'a> {
    pub artifact: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactIdsSelectionKind {
    RunForestNode,
    Artifact,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtifactIdsSectionSnapshot<'a> {
    pub selection_kind: ArtifactIdsSelectionKind,
    pub state: ArtifactIdsSectionState<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ArtifactIdsSectionState<'a> {
    NotApplicable {
        selection_key: &'a str,
    },
    Rendered {
        selection_key: &'a str,
        artifact_ids: Vec<&'a str>,
        artifact_refs: Vec<&'a str>,
        tree_keys: Vec<&'a str>,
    },
    Unavailable(UnavailableReason),
}

impl<'a> ArtifactIdsSectionSnapshot<'a> {
    pub fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "selection_kind: {}\n",
            artifact_ids_selection_kind_label(self.selection_kind)
        ));
        match &self.state {
            ArtifactIdsSectionState::NotApplicable { selection_key } => {
                out.push_str("artifact_ids: not_applicable\n");
                out.push_str(&format!("selection_key: {selection_key}\n"));
            }
            ArtifactIdsSectionState::Rendered {
                selection_key,
                artifact_ids,
                artifact_refs,
                tree_keys,
            } => {
                out.push_str("artifact_ids: rendered\n");
                out.push_str(&format!("selection_key: {selection_key}\n"));
                render_id_list(&mut out, "artifact_id", artifact_ids);
                render_id_list(&mut out, "artifact_ref", artifact_refs);
                render_id_list(&mut out, "tree_key", tree_keys);
            }
            ArtifactIdsSectionState::Unavailable(reason) => {
                out.push_str("artifact_ids: unavailable\n");
                out.push_str(&format!("subject: {}\n", reason.subject()));
                out.push_str(&format!("state: {}\n", reason.state()));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMetrics {
    RunForestNode(RunForestMetrics),
    Artifact(ArtifactMetrics),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RunForestMetrics {
    pub generation: u32,
    pub child_run_forest_nodes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ArtifactMetrics {
    pub source_records: usize,
    pub evidence_refs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParentCreateSnapshot<'a> {
    pub state: ParentCreateState,
    pub ambiguous_count: Option<usize>,
    pub unavailable: Option<ParentCreateUnavailableSnapshot<'a>>,
    pub child_node_id: Option<&'a str>,
    pub target_relpath: Option<&'a str>,
    pub branch_id: Option<&'a str>,
    pub candidate_id: Option<&'a str>,
    pub stop_on_error: Option<bool>,
    pub surface_target_relpath: Option<&'a str>,
    pub surface_producer: Option<&'static str>,
    pub surface_check: Option<&'static str>,
    pub surface_apply: Option<&'static str>,
    pub touched_files: Option<usize>,
    pub router: Option<&'a str>,
    pub router_model: Option<&'a str>,
    pub agent_turns: Vec<AgentTurnSnapshot<'a>>,
    pub tool_requested: usize,
    pub tool_completed: usize,
    pub tool_failed: usize,
    pub edit_proposals: usize,
    pub create_proposals: usize,
    pub expected_file_changes: usize,
    pub candidate_evaluation_count: usize,
    pub source_ref_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParentCreateState {
    Available,
    Missing,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ParentCreateUnavailableSnapshot<'a> {
    pub record: &'static str,
    pub key: &'static str,
    pub value: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AgentTurnSnapshot<'a> {
    pub task_id: &'a str,
    pub selected_model: &'a str,
    pub event_count: usize,
    pub terminal_outcome: Option<&'a str>,
    pub has_llm_response: bool,
    pub patch_applied: bool,
    pub all_proposals_applied: bool,
    pub llm_prompt_message_count: usize,
    pub tool_requested: usize,
    pub tool_completed: usize,
    pub tool_failed: usize,
    pub edit_proposals: usize,
    pub create_proposals: usize,
    pub expected_file_changes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RunRecordSnapshot<'a> {
    pub arm: ploke_tree::ComparedRunArm,
    pub instance_id: &'a str,
    pub record_path: &'a Path,
    pub manifest_id: &'a str,
    pub model: Option<&'a str>,
    pub provider: Option<&'a str>,
    pub repo_root: &'a Path,
    pub turn_count: usize,
    pub tool_call_count: usize,
    pub failed_tool_call_count: usize,
    pub packaging: Option<ploke_records::run_record::SubmissionArtifactState>,
    pub patch_projection_check: Option<ploke_records::evaluation::PatchProjectionCheckState>,
    pub total_wall_clock_millis: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SelectionEdge<'a> {
    pub relation: EdgeRelation,
    pub from: &'a str,
    pub to: &'a str,
    pub source_count: usize,
}

impl<'a> SelectionEdge<'a> {
    fn new(relation: EdgeRelation, from: &'a str, to: &'a str, source_count: usize) -> Self {
        Self {
            relation,
            from,
            to,
            source_count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EdgeRelation {
    #[serde(rename = "E_F")]
    RunForest,
    #[serde(rename = "P_H")]
    HistoryPatch,
    #[serde(rename = "P_C")]
    ProducedChild,
    #[serde(rename = "P_O")]
    HistoryOpenedFrom,
    #[serde(rename = "P_B")]
    AppliedPatch,
}

impl EdgeRelation {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::RunForest => "E_F",
            Self::HistoryPatch => "P_H",
            Self::ProducedChild => "P_C",
            Self::HistoryOpenedFrom => "P_O",
            Self::AppliedPatch => "P_B",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PatchSnapshot<'a> {
    pub patch_id: &'a str,
    pub target_relpath: &'a str,
    pub branch_id: &'a str,
    pub candidate_id: &'a str,
    pub source_content_hash: &'a str,
    pub proposed_content_hash: &'a str,
    pub base_artifact: Option<&'a str>,
    pub derived_artifact: Option<&'a str>,
    pub check_status: Option<ploke_records::history::SurfaceCheckStatusRecord>,
    pub apply_status: Option<ploke_records::history::SurfaceApplyStatusRecord>,
    pub touches: Vec<PatchTouch<'a>>,
    pub source_content: &'a str,
    pub proposed_content: &'a str,
}

impl<'a> PatchSnapshot<'a> {
    pub fn unified_diff(&self) -> String {
        crate::ui::diff::unified_rust_diff(
            self.target_relpath,
            self.source_content,
            self.proposed_content,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PatchTouch<'a> {
    pub index: usize,
    pub relpath: &'a str,
    pub start: usize,
    pub end: usize,
    pub replacement: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SourceRef<'a> {
    Evidence {
        kind: &'static str,
        authority: &'static str,
        recorded_at: Option<&'a str>,
    },
    Diagnostic {
        severity: &'static str,
        code: &'a str,
    },
    ArtifactHistoryRef {
        artifact: &'a str,
    },
    ArtifactId {
        artifact: &'a str,
    },
}

impl std::fmt::Display for SourceRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Evidence {
                kind,
                authority,
                recorded_at,
            } => {
                write!(f, "{kind}:{authority}")?;
                if let Some(recorded_at) = recorded_at {
                    write!(f, " @{recorded_at}")?;
                }
                Ok(())
            }
            Self::Diagnostic { severity, code } => write!(f, "diagnostic:{severity}:{code}"),
            Self::ArtifactHistoryRef { artifact } => write!(f, "artifact_history_ref:{artifact}"),
            Self::ArtifactId { artifact } => write!(f, "artifact_id:{artifact}"),
        }
    }
}

pub fn default_selections(graph: &ploke_tree::Graph) -> Vec<GraphSelectionDetail> {
    graph
        .artifact_tree()
        .nodes
        .values()
        .enumerate()
        .map(|(index, node)| GraphSelectionDetail {
            kind: "artifact".to_owned(),
            label: artifact_handle_label(index + 1),
            detail: String::new(),
            reference: GraphSelectionRef::Artifact {
                key: node.key.as_str().to_owned(),
            },
        })
        .collect()
}

impl GraphSelectionRef {
    fn matches_key(&self, selector: &str) -> bool {
        match self {
            Self::Artifact { key } | Self::RunForestNode { key } => key == selector,
        }
    }
}

fn graph_selection_by_key(
    graph: &ploke_tree::Graph,
    selector: &str,
) -> Option<GraphSelectionDetail> {
    if graph.forest.as_ref().is_some_and(|forest| {
        forest
            .nodes
            .iter()
            .any(|node| node.key.as_str() == selector)
    }) {
        return Some(GraphSelectionDetail {
            kind: "artifact".to_owned(),
            label: selector.to_owned(),
            detail: String::new(),
            reference: GraphSelectionRef::RunForestNode {
                key: selector.to_owned(),
            },
        });
    }

    graph
        .artifact_tree()
        .nodes
        .values()
        .find(|node| node.key.as_str() == selector)
        .map(|node| GraphSelectionDetail {
            kind: "artifact".to_owned(),
            label: selector.to_owned(),
            detail: String::new(),
            reference: GraphSelectionRef::Artifact {
                key: node.key.as_str().to_owned(),
            },
        })
}

fn run_forest_node_inspection<'g>(
    graph: &'g ploke_tree::Graph,
    key: &str,
) -> SelectionInspector<'g> {
    let Some(forest) = graph.forest.as_ref() else {
        return SelectionInspector::Unresolved(UnavailableReason::RunForestNotPresent);
    };
    let Some(node) = forest.nodes.iter().find(|node| node.key.as_str() == key) else {
        return SelectionInspector::Unresolved(UnavailableReason::RunForestNodeNotFound);
    };
    let parent = node.parent.as_ref().and_then(|parent| {
        forest
            .nodes
            .iter()
            .find(|candidate| candidate.key == *parent)
    });
    SelectionInspector::RunForestNode(RunForestNodeInspection {
        node,
        parent,
        children: &node.children,
        patch: graph
            .child_plans
            .child_for_node_id(node.key.as_str())
            .map(|child| PatchInspection { child }),
        parent_create: graph.parent_create_for_node_id(node.key.as_str()),
        role_badges: role_badges_for_run_forest_node(graph, node),
        run_records: RunRecordBranchInspection::from_graph(graph, node.branch_id.as_str()),
    })
}

fn artifact_inspection<'g>(graph: &'g ploke_tree::Graph, key: &str) -> SelectionInspector<'g> {
    let tree = graph.artifact_tree();
    let Some(node) = tree.nodes.values().find(|node| node.key.as_str() == key) else {
        return SelectionInspector::Unresolved(UnavailableReason::ArtifactNotFound);
    };

    let incoming: Vec<_> = tree
        .history_successors
        .iter()
        .filter(|edge| edge.to == node.key)
        .map(|edge| ArtifactRelation {
            kind: ArtifactRelationKind::HistoryPatch,
            from: edge.from.as_str(),
            to: edge.to.as_str(),
            source_count: edge.sources.len(),
            patch_ids: Vec::new(),
        })
        .chain(
            tree.produced_child_edges
                .iter()
                .filter(|edge| edge.to == node.key)
                .map(|edge| ArtifactRelation {
                    kind: ArtifactRelationKind::ProducedChild,
                    from: edge.from.as_str(),
                    to: edge.to.as_str(),
                    source_count: edge.sources.len(),
                    patch_ids: edge.sources.iter().filter_map(child_patch_id).collect(),
                }),
        )
        .chain(
            tree.opened_from_edges
                .iter()
                .filter(|edge| edge.to == node.key)
                .map(|edge| ArtifactRelation {
                    kind: ArtifactRelationKind::HistoryOpenedFrom,
                    from: edge.from.as_str(),
                    to: edge.to.as_str(),
                    source_count: edge.sources.len(),
                    patch_ids: Vec::new(),
                }),
        )
        .chain(
            tree.applied_patch_edges
                .iter()
                .filter(|edge| edge.to == node.key)
                .map(|edge| ArtifactRelation {
                    kind: ArtifactRelationKind::AppliedPatch,
                    from: edge.from.as_str(),
                    to: edge.to.as_str(),
                    source_count: edge.sources.len(),
                    patch_ids: edge
                        .sources
                        .iter()
                        .filter_map(|source| source.patch_id.as_ref())
                        .collect(),
                }),
        )
        .collect();
    let outgoing: Vec<_> = tree
        .history_successors
        .iter()
        .filter(|edge| edge.from == node.key)
        .map(|edge| ArtifactRelation {
            kind: ArtifactRelationKind::HistoryPatch,
            from: edge.from.as_str(),
            to: edge.to.as_str(),
            source_count: edge.sources.len(),
            patch_ids: Vec::new(),
        })
        .chain(
            tree.produced_child_edges
                .iter()
                .filter(|edge| edge.from == node.key)
                .map(|edge| ArtifactRelation {
                    kind: ArtifactRelationKind::ProducedChild,
                    from: edge.from.as_str(),
                    to: edge.to.as_str(),
                    source_count: edge.sources.len(),
                    patch_ids: edge.sources.iter().filter_map(child_patch_id).collect(),
                }),
        )
        .chain(
            tree.opened_from_edges
                .iter()
                .filter(|edge| edge.from == node.key)
                .map(|edge| ArtifactRelation {
                    kind: ArtifactRelationKind::HistoryOpenedFrom,
                    from: edge.from.as_str(),
                    to: edge.to.as_str(),
                    source_count: edge.sources.len(),
                    patch_ids: Vec::new(),
                }),
        )
        .chain(
            tree.applied_patch_edges
                .iter()
                .filter(|edge| edge.from == node.key)
                .map(|edge| ArtifactRelation {
                    kind: ArtifactRelationKind::AppliedPatch,
                    from: edge.from.as_str(),
                    to: edge.to.as_str(),
                    source_count: edge.sources.len(),
                    patch_ids: edge
                        .sources
                        .iter()
                        .filter_map(|source| source.patch_id.as_ref())
                        .collect(),
                }),
        )
        .collect();

    let parent_create = graph.parent_create_for_artifact_key(node.key.as_str());
    let sources = node.sources.clone();
    let mut patches = BTreeMap::new();
    for child in incoming
        .iter()
        .chain(outgoing.iter())
        .flat_map(|relation| relation.patch_ids.iter())
        .filter_map(|patch_id| graph.child_plans.child_for_patch_id(patch_id))
    {
        patches
            .entry(child.node.node_id.as_str())
            .or_insert(PatchInspection { child });
    }
    if let ParentCreateLookup::Attempt(attempt) = parent_create {
        let child = attempt.child();
        patches
            .entry(child.node.node_id.as_str())
            .or_insert(PatchInspection { child });
    }
    let patches: Vec<_> = patches.into_values().collect();
    let run_records = artifact_run_record_inspection(graph, parent_create, &patches);

    SelectionInspector::Artifact(ArtifactInspection {
        sources,
        incoming,
        outgoing,
        patches,
        parent_create,
        role_badges: role_badges_for_artifact_node(graph, node),
        run_records,
    })
}

fn artifact_run_record_inspection<'g>(
    graph: &'g ploke_tree::Graph,
    parent_create: ParentCreateLookup<'g, 'g>,
    patches: &[PatchInspection<'g>],
) -> RunRecordBranchInspection<'g> {
    if let ParentCreateLookup::Attempt(attempt) = parent_create {
        return RunRecordBranchInspection::from_graph(
            graph,
            attempt.child().resolved.branch.branch_id.as_str(),
        );
    }

    let mut branch_id = None;
    for patch in patches {
        let next = patch.branch_id();
        match branch_id {
            None => branch_id = Some(next),
            Some(current) if current == next => {}
            Some(_) => return RunRecordBranchInspection::empty(),
        }
    }

    branch_id
        .map(|branch_id| RunRecordBranchInspection::from_graph(graph, branch_id))
        .unwrap_or_else(RunRecordBranchInspection::empty)
}

fn snapshot_run_forest<'a, 'g>(
    selection: &'a GraphSelectionDetail,
    inspector: RunForestNodeInspection<'g>,
) -> SelectionInspectorSnapshot<'a>
where
    'g: 'a,
{
    let RunForestNodeInspection {
        node,
        parent,
        children,
        patch,
        parent_create,
        role_badges,
        run_records,
    } = inspector;
    let identity = SelectionIdentity::RunForestNode(run_forest_node_identity(node));
    let metrics = SelectionMetrics::RunForestNode(RunForestMetrics {
        generation: node.generation,
        child_run_forest_nodes: children.len(),
    });
    let incoming = parent
        .into_iter()
        .map(|parent| {
            SelectionEdge::new(
                EdgeRelation::RunForest,
                parent.key.as_str(),
                node.key.as_str(),
                1,
            )
        })
        .collect();
    let outgoing = children
        .iter()
        .map(|child| {
            SelectionEdge::new(
                EdgeRelation::RunForest,
                node.key.as_str(),
                child.as_str(),
                1,
            )
        })
        .collect();
    let artifact_outgoing = node
        .base_artifact_id
        .as_deref()
        .zip(node.derived_artifact_id.as_deref())
        .into_iter()
        .map(|(from, to)| SelectionEdge::new(EdgeRelation::AppliedPatch, from, to, 1))
        .collect();
    let source_refs = node
        .evidence
        .iter()
        .map(|evidence| SourceRef::Evidence {
            kind: evidence_kind_label(evidence.kind),
            authority: authority_label(evidence.authority),
            recorded_at: evidence.recorded_at.as_deref(),
        })
        .chain(
            node.diagnostics
                .iter()
                .map(|diagnostic| SourceRef::Diagnostic {
                    severity: diagnostic_severity_label(diagnostic.severity),
                    code: diagnostic.code.as_str(),
                }),
        )
        .collect();

    SelectionInspectorSnapshot {
        kind: selection.kind.as_str(),
        label: selection.label.as_str(),
        identity: Some(identity),
        roles: role_badges,
        metrics: Some(metrics),
        parent_create: Some(parent_create_snapshot(parent_create)),
        run_records: run_records.iter().map(run_record_snapshot).collect(),
        incoming,
        outgoing,
        artifact_incoming: Vec::new(),
        artifact_outgoing,
        patches: patch.into_iter().map(patch_snapshot).collect(),
        source_refs,
        unavailable: None,
    }
}

fn snapshot_artifact<'a, 'g>(
    selection: &'a GraphSelectionDetail,
    inspector: ArtifactInspection<'g>,
) -> SelectionInspectorSnapshot<'a>
where
    'g: 'a,
{
    let ArtifactInspection {
        sources,
        incoming,
        outgoing,
        patches,
        parent_create,
        role_badges,
        run_records,
    } = inspector;
    let source_refs = sources
        .iter()
        .map(|source| match &source.identity {
            ploke_tree::graph::ArtifactIdentity::HistoryRef(artifact) => {
                SourceRef::ArtifactHistoryRef {
                    artifact: artifact.as_str(),
                }
            }
            ploke_tree::graph::ArtifactIdentity::PassiveId(artifact) => SourceRef::ArtifactId {
                artifact: artifact.0.as_str(),
            },
        })
        .collect();
    let identity = ArtifactIdentityWitness {
        sources: sources.as_slice(),
    };

    SelectionInspectorSnapshot {
        kind: selection.kind.as_str(),
        label: selection.label.as_str(),
        identity: Some(SelectionIdentity::Artifact(ArtifactSelectionIdentity {
            artifact: identity.artifact(),
        })),
        roles: role_badges,
        metrics: Some(SelectionMetrics::Artifact(artifact_metrics(&sources))),
        parent_create: Some(parent_create_snapshot(parent_create)),
        run_records: run_records.iter().map(run_record_snapshot).collect(),
        incoming: incoming
            .iter()
            .map(|edge| {
                SelectionEdge::new(
                    edge.kind.edge_relation(),
                    edge.from,
                    edge.to,
                    edge.source_count,
                )
            })
            .collect(),
        outgoing: outgoing
            .iter()
            .map(|edge| {
                SelectionEdge::new(
                    edge.kind.edge_relation(),
                    edge.from,
                    edge.to,
                    edge.source_count,
                )
            })
            .collect(),
        artifact_incoming: incoming
            .into_iter()
            .map(|edge| {
                SelectionEdge::new(
                    edge.kind.edge_relation(),
                    edge.from,
                    edge.to,
                    edge.source_count,
                )
            })
            .collect(),
        artifact_outgoing: outgoing
            .into_iter()
            .map(|edge| {
                SelectionEdge::new(
                    edge.kind.edge_relation(),
                    edge.from,
                    edge.to,
                    edge.source_count,
                )
            })
            .collect(),
        patches: patches.into_iter().map(patch_snapshot).collect(),
        source_refs,
        unavailable: None,
    }
}

fn run_record_snapshot(record: RunRecordInspection<'_>) -> RunRecordSnapshot<'_> {
    let packaging = record.record.phases.packaging.as_ref();
    RunRecordSnapshot {
        arm: record.record_ref.arm,
        instance_id: record.record_ref.instance_id.as_str(),
        record_path: record.record_ref.record_path.as_path(),
        manifest_id: record.record.manifest_id.as_str(),
        model: record.record.metadata.agent.model_id.as_deref(),
        provider: record.record.metadata.agent.provider.as_deref(),
        repo_root: record.record.metadata.benchmark.repo_root.as_path(),
        turn_count: record.stats.turn_count,
        tool_call_count: record.stats.tool_call_count,
        failed_tool_call_count: record.stats.failed_tool_call_count,
        packaging: packaging.map(|packaging| packaging.submission_artifact_state),
        patch_projection_check: packaging.map(|packaging| packaging.patch_projection_check_state),
        total_wall_clock_millis: record.stats.total_wall_clock_millis,
    }
}

pub(crate) fn parent_create_snapshot<'a>(
    lookup: ParentCreateLookup<'a, 'a>,
) -> ParentCreateSnapshot<'a> {
    match lookup {
        ParentCreateLookup::Attempt(attempt) => parent_create_attempt_snapshot(attempt),
        ParentCreateLookup::Unavailable(reason) => ParentCreateSnapshot {
            state: ParentCreateState::Missing,
            ambiguous_count: None,
            unavailable: Some(parent_create_unavailable_snapshot(reason)),
            child_node_id: None,
            target_relpath: None,
            branch_id: None,
            candidate_id: None,
            stop_on_error: None,
            surface_target_relpath: None,
            surface_producer: None,
            surface_check: None,
            surface_apply: None,
            touched_files: None,
            router: None,
            router_model: None,
            agent_turns: Vec::new(),
            tool_requested: 0,
            tool_completed: 0,
            tool_failed: 0,
            edit_proposals: 0,
            create_proposals: 0,
            expected_file_changes: 0,
            candidate_evaluation_count: 0,
            source_ref_count: 0,
        },
        ParentCreateLookup::Ambiguous { count, reason } => ParentCreateSnapshot {
            state: ParentCreateState::Ambiguous,
            ambiguous_count: Some(count),
            unavailable: Some(parent_create_unavailable_snapshot(reason)),
            child_node_id: None,
            target_relpath: None,
            branch_id: None,
            candidate_id: None,
            stop_on_error: None,
            surface_target_relpath: None,
            surface_producer: None,
            surface_check: None,
            surface_apply: None,
            touched_files: None,
            router: None,
            router_model: None,
            agent_turns: Vec::new(),
            tool_requested: 0,
            tool_completed: 0,
            tool_failed: 0,
            edit_proposals: 0,
            create_proposals: 0,
            expected_file_changes: 0,
            candidate_evaluation_count: 0,
            source_ref_count: 0,
        },
    }
}

fn parent_create_attempt_snapshot<'a>(
    attempt: ParentCreateAttempt<'a>,
) -> ParentCreateSnapshot<'a> {
    let child = attempt.child();
    let surface = attempt.surface();
    let mut agent_turns = Vec::new();
    let mut tool_requested = 0;
    let mut tool_completed = 0;
    let mut tool_failed = 0;
    let mut edit_proposals = 0;
    let mut create_proposals = 0;
    let mut expected_file_changes = 0;

    for turn in attempt.agent_turns() {
        tool_requested += turn.tool_request_event_count;
        tool_completed += turn.tool_completed_event_count;
        tool_failed += turn.tool_failed_event_count;
        edit_proposals += turn.edit_proposal_count;
        create_proposals += turn.create_proposal_count;
        expected_file_changes += turn.expected_file_change_count;
        agent_turns.push(AgentTurnSnapshot {
            task_id: turn.task_id.as_str(),
            selected_model: turn.selected_model.as_str(),
            event_count: turn.event_count,
            terminal_outcome: turn.terminal_outcome.as_deref(),
            has_llm_response: turn.has_llm_response,
            patch_applied: turn.patch_applied,
            all_proposals_applied: turn.all_proposals_applied,
            llm_prompt_message_count: turn.llm_prompt_message_count,
            tool_requested: turn.tool_request_event_count,
            tool_completed: turn.tool_completed_event_count,
            tool_failed: turn.tool_failed_event_count,
            edit_proposals: turn.edit_proposal_count,
            create_proposals: turn.create_proposal_count,
            expected_file_changes: turn.expected_file_change_count,
        });
    }

    let (surface_producer, router, router_model) = match attempt.surface_producer() {
        Some(ploke_records::history::SurfaceProposalProducerRecord::NonRouter) => {
            (Some("non_router"), None, None)
        }
        Some(ploke_records::history::SurfaceProposalProducerRecord::Router { request_policy }) => (
            Some("router"),
            Some(request_policy.router.as_str()),
            Some(request_policy.model.value.as_str()),
        ),
        None => (None, None, None),
    };

    ParentCreateSnapshot {
        state: ParentCreateState::Available,
        ambiguous_count: None,
        unavailable: None,
        child_node_id: Some(child.node.node_id.as_str()),
        target_relpath: Some(
            child
                .request
                .target_relpath
                .to_str()
                .unwrap_or("non_utf8_path"),
        ),
        branch_id: Some(child.resolved.branch.branch_id.as_str()),
        candidate_id: Some(child.resolved.branch.candidate_id.as_str()),
        stop_on_error: Some(child.request.stop_on_error),
        surface_target_relpath: surface
            .map(|surface| surface.target_relpath.to_str().unwrap_or("non_utf8_path")),
        surface_producer,
        surface_check: surface.map(|surface| surface_check_status_label(surface.check_status)),
        surface_apply: surface.map(|surface| surface_apply_status_label(surface.apply_status)),
        touched_files: surface.map(|surface| surface.touches.len()),
        router,
        router_model,
        agent_turns,
        tool_requested,
        tool_completed,
        tool_failed,
        edit_proposals,
        create_proposals,
        expected_file_changes,
        candidate_evaluation_count: attempt.candidate_evaluation_count(),
        source_ref_count: attempt.source_ref_count(),
    }
}

fn parent_create_unavailable_snapshot<'a>(
    reason: ParentCreateUnavailable<'a>,
) -> ParentCreateUnavailableSnapshot<'a> {
    match reason {
        ParentCreateUnavailable::MissingJoin { record, key, value }
        | ParentCreateUnavailable::AmbiguousJoin { record, key, value } => {
            ParentCreateUnavailableSnapshot { record, key, value }
        }
    }
}

fn patch_snapshot<'a>(patch: PatchInspection<'a>) -> PatchSnapshot<'a> {
    PatchSnapshot {
        patch_id: patch.patch_id(),
        target_relpath: patch.target_relpath(),
        branch_id: patch.branch_id(),
        candidate_id: patch.candidate_id(),
        source_content_hash: patch.source_content_hash(),
        proposed_content_hash: patch.proposed_content_hash(),
        base_artifact: patch.base_artifact(),
        derived_artifact: patch.derived_artifact(),
        check_status: patch.check_status(),
        apply_status: patch.apply_status(),
        touches: patch.touches().collect(),
        source_content: patch.source_content(),
        proposed_content: patch.proposed_content(),
    }
}

pub(crate) fn run_forest_node_identity(node: &ploke_tree::TreeNode) -> RunForestNodeIdentity<'_> {
    RunForestNodeIdentity {
        node_key: node.key.as_str(),
        candidate_id: node.candidate_id.as_str(),
        source_artifact: node.source_state_id.as_str(),
        parent_node: node.parent.as_ref().map(|parent| parent.as_str()),
        base_artifact: node.base_artifact_id.as_deref(),
        derived_artifact: node.derived_artifact_id.as_deref(),
        patch: node.patch_id.as_deref(),
        branch_id: node.branch_id.as_str(),
        target_relpath: node.target_relpath.as_str(),
        phase: node.progress.phase,
        result: node.progress.result_class,
    }
}

pub(crate) fn artifact_metrics(sources: &[&ploke_tree::graph::ArtifactNode]) -> ArtifactMetrics {
    ArtifactMetrics {
        source_records: sources.len(),
        evidence_refs: sources.iter().map(|source| source.evidence.len()).sum(),
    }
}

#[cfg(test)]
pub(crate) fn run_forest_incoming_edges<'a>(
    inspector: &'a RunForestNodeInspection<'a>,
) -> impl Iterator<Item = SelectionEdge<'a>> + 'a {
    inspector
        .parent
        .map(|parent| {
            SelectionEdge::new(
                EdgeRelation::RunForest,
                parent.key.as_str(),
                inspector.node.key.as_str(),
                1,
            )
        })
        .into_iter()
}

fn artifact_identity_label(identity: &ploke_tree::graph::ArtifactIdentity) -> &str {
    match identity {
        ploke_tree::graph::ArtifactIdentity::HistoryRef(artifact) => artifact.as_str(),
        ploke_tree::graph::ArtifactIdentity::PassiveId(artifact) => {
            artifact.0.strip_prefix("artifact:").unwrap_or(&artifact.0)
        }
    }
}

fn child_patch_id<'a>(
    child: &&'a ploke_records::child_plan::ChildPlanChildRecord,
) -> Option<&'a ploke_records::ids::PatchId> {
    child
        .surface
        .as_ref()
        .map(|surface| &surface.patch_id)
        .or(child.node.patch_id.as_ref())
        .or(child.request.patch_id.as_ref())
}

#[cfg(test)]
pub(crate) fn artifact_edges<'a>(
    edges: &'a [ArtifactRelation<'a>],
) -> impl Iterator<Item = SelectionEdge<'a>> + 'a {
    edges.iter().map(|edge| {
        SelectionEdge::new(
            edge.kind.edge_relation(),
            edge.from,
            edge.to,
            edge.source_count,
        )
    })
}

fn role_badges_for_run_forest_node<'g>(
    graph: &'g ploke_tree::Graph,
    node: &ploke_tree::TreeNode,
) -> Vec<Badge<'g>> {
    let node_key = node.key.as_str();
    let derived_artifact_id = node.derived_artifact_id.as_deref();
    unique_role_badges(graph.invocations().filter_map(move |(_, invocation)| {
        let badge = Badge::from_invocation(invocation)?;
        let artifact_matches = derived_artifact_id
            .is_some_and(|artifact_id| badge.artifact_id().0.as_str() == artifact_id);
        (invocation.node_id.as_str() == node_key || artifact_matches).then_some(invocation)
    }))
}

fn role_badges_for_artifact_node<'g>(
    graph: &'g ploke_tree::Graph,
    node: &ploke_tree::graph::artifact_tree::Node<'g>,
) -> Vec<Badge<'g>> {
    let mut artifact_ids = Vec::new();
    for source in &node.sources {
        for artifact_id in source.artifact_ids() {
            let artifact_id = artifact_id.0.as_str();
            if !artifact_ids.contains(&artifact_id) {
                artifact_ids.push(artifact_id);
            }
        }
    }
    unique_role_badges(graph.invocations().filter_map(move |(_, invocation)| {
        let badge = Badge::from_invocation(invocation)?;
        artifact_ids
            .iter()
            .any(|artifact_id| badge.artifact_id().0.as_str() == *artifact_id)
            .then_some(invocation)
    }))
}

fn unique_role_badges<'g>(
    invocations: impl IntoIterator<Item = &'g ploke_records::invocation::InvocationRecord>,
) -> Vec<Badge<'g>> {
    let mut parent = None;
    let mut child = None;
    for invocation in invocations {
        match Badge::from_invocation(invocation) {
            Some(Badge::Parent(artifact_id)) if parent.is_none() => {
                parent = Some(Badge::Parent(artifact_id));
            }
            Some(Badge::Child(artifact_id)) if child.is_none() => {
                child = Some(Badge::Child(artifact_id));
            }
            _ => {}
        }
    }

    parent.into_iter().chain(child).collect()
}

fn artifact_handle_label(index: usize) -> String {
    format!("A{index}")
}

pub(crate) fn phase_label(phase: ploke_tree::Phase) -> &'static str {
    match phase {
        ploke_tree::Phase::Planned => "planned",
        ploke_tree::Phase::WorkspaceStaged => "workspace_staged",
        ploke_tree::Phase::BinaryBuilt => "binary_built",
        ploke_tree::Phase::Running => "running",
        ploke_tree::Phase::Completed => "completed",
        ploke_tree::Phase::Failed => "failed",
        ploke_tree::Phase::Unknown => "unknown",
    }
}

pub(crate) fn result_class_label(result_class: ploke_tree::ResultClass) -> &'static str {
    match result_class {
        ploke_tree::ResultClass::Success => "success",
        ploke_tree::ResultClass::Failure => "failure",
        ploke_tree::ResultClass::Unknown => "unknown",
    }
}

fn evidence_kind_label(kind: ploke_tree::EvidenceKind) -> &'static str {
    match kind {
        ploke_tree::EvidenceKind::SchedulerNode => "scheduler_node",
        ploke_tree::EvidenceKind::ParentIdentity => "parent_identity",
        ploke_tree::EvidenceKind::SuccessorReady => "successor_ready",
        ploke_tree::EvidenceKind::SuccessorCompletion => "successor_completion",
    }
}

fn authority_label(authority: ploke_tree::AuthorityLabel) -> &'static str {
    match authority {
        ploke_tree::AuthorityLabel::MutableProjection => "mutable_projection",
        ploke_tree::AuthorityLabel::TypedRecordEvidence => "typed_record_evidence",
        ploke_tree::AuthorityLabel::LiveTransport => "live_transport",
        ploke_tree::AuthorityLabel::SealedVerifiedHistory => "sealed_verified_history",
        ploke_tree::AuthorityLabel::DegradedObservation => "degraded_observation",
    }
}

fn diagnostic_severity_label(severity: ploke_tree::DiagnosticSeverity) -> &'static str {
    match severity {
        ploke_tree::DiagnosticSeverity::Info => "info",
        ploke_tree::DiagnosticSeverity::Warning => "warning",
        ploke_tree::DiagnosticSeverity::Error => "error",
    }
}

pub(crate) fn surface_check_status_label(
    status: ploke_records::history::SurfaceCheckStatusRecord,
) -> &'static str {
    match status {
        ploke_records::history::SurfaceCheckStatusRecord::Checked => "checked",
    }
}

pub(crate) fn surface_apply_status_label(
    status: ploke_records::history::SurfaceApplyStatusRecord,
) -> &'static str {
    match status {
        ploke_records::history::SurfaceApplyStatusRecord::Applied => "applied",
    }
}

fn render_badges(out: &mut String, heading: &str, badges: &[Badge]) {
    if badges.is_empty() {
        out.push_str(&format!("{heading}: none\n"));
        return;
    }
    out.push_str(&format!("{heading}:\n"));
    for badge in badges {
        let role: &'static str = badge.into();
        out.push_str(&format!("- {role}: {}\n", badge.artifact_id().0));
    }
}

fn render_identity(out: &mut String, identity: Option<&SelectionIdentity<'_>>) {
    let Some(identity) = identity else {
        out.push_str("identity: none\n");
        return;
    };

    out.push_str("identity:\n");
    match identity {
        SelectionIdentity::RunForestNode(identity) => {
            out.push_str(&format!("- run_forest_node: {}\n", identity.node_key));
            out.push_str(&format!("- candidate: {}\n", identity.candidate_id));
            out.push_str(&format!(
                "- source_artifact: {}\n",
                identity.source_artifact
            ));
            if let Some(parent) = identity.parent_node {
                out.push_str(&format!("- parent_run_forest_node: {parent}\n"));
            }
            if let Some(base) = identity.base_artifact {
                out.push_str(&format!("- base_artifact: {base}\n"));
            }
            if let Some(derived) = identity.derived_artifact {
                out.push_str(&format!("- derived_artifact: {derived}\n"));
            }
            if let Some(patch) = identity.patch {
                out.push_str(&format!("- patch: {patch}\n"));
            }
            out.push_str(&format!("- branch: {}\n", identity.branch_id));
            out.push_str(&format!("- target: {}\n", identity.target_relpath));
            out.push_str(&format!("- phase: {}\n", phase_label(identity.phase)));
            out.push_str(&format!(
                "- result: {}\n",
                result_class_label(identity.result)
            ));
        }
        SelectionIdentity::Artifact(identity) => {
            out.push_str(&format!("- artifact: {}\n", identity.artifact));
        }
    }
}

fn render_metrics(out: &mut String, metrics: Option<&SelectionMetrics>) {
    if metrics.is_none() {
        out.push_str("metrics: none\n");
        return;
    }
    out.push_str("metrics:\n");
    match metrics.expect("checked above") {
        SelectionMetrics::RunForestNode(metrics) => {
            out.push_str(&format!("- generation: {}\n", metrics.generation));
            out.push_str(&format!(
                "- child_run_forest_nodes: {}\n",
                metrics.child_run_forest_nodes
            ));
        }
        SelectionMetrics::Artifact(metrics) => {
            out.push_str(&format!("- source_records: {}\n", metrics.source_records));
            out.push_str(&format!("- evidence_refs: {}\n", metrics.evidence_refs));
        }
    }
}

fn render_parent_create(out: &mut String, parent_create: Option<&ParentCreateSnapshot<'_>>) {
    let Some(parent_create) = parent_create else {
        out.push_str("parent_create: none\n");
        return;
    };

    out.push_str("parent_create:\n");
    match parent_create.state {
        ParentCreateState::Available => {
            out.push_str("- state: available\n");
            if let Some(target) = parent_create.target_relpath {
                out.push_str(&format!("- target: {target}\n"));
            }
            if let Some(producer) = parent_create.surface_producer {
                out.push_str(&format!("- surface: {producer}\n"));
            }
            if let Some(touched_files) = parent_create.touched_files {
                out.push_str(&format!("- surface_touches: {touched_files}\n"));
            }
            if let Some(model) = parent_create.router_model {
                out.push_str(&format!("- model: {model}\n"));
            }
            out.push_str(&format!(
                "- tools: requested={} completed={} failed={}\n",
                parent_create.tool_requested,
                parent_create.tool_completed,
                parent_create.tool_failed
            ));
            out.push_str(&format!(
                "- llm_proposals: edits={} creates={} expected_files={}\n",
                parent_create.edit_proposals,
                parent_create.create_proposals,
                parent_create.expected_file_changes
            ));
            if let (Some(check), Some(apply)) =
                (parent_create.surface_check, parent_create.surface_apply)
            {
                out.push_str(&format!("- check_apply: {check}/{apply}\n"));
            }
            out.push_str(&format!(
                "- child_eval_evidence: {}\n",
                parent_create.candidate_evaluation_count
            ));
        }
        ParentCreateState::Missing => {
            out.push_str("- state: missing\n");
            if let Some(reason) = parent_create.unavailable {
                out.push_str(&format!(
                    "- missing_join: {}.{}={}\n",
                    reason.record, reason.key, reason.value
                ));
            }
        }
        ParentCreateState::Ambiguous => {
            out.push_str("- state: ambiguous\n");
            if let Some(count) = parent_create.ambiguous_count {
                out.push_str(&format!("- matches: {count}\n"));
            }
            if let Some(reason) = parent_create.unavailable {
                out.push_str(&format!(
                    "- ambiguous_join: {}.{}={}\n",
                    reason.record, reason.key, reason.value
                ));
            }
        }
    }
}

fn render_run_records(out: &mut String, records: &[RunRecordSnapshot<'_>]) {
    if records.is_empty() {
        out.push_str("run_records: none\n");
        return;
    }

    out.push_str(&format!("run_records: count={}\n", records.len()));
    for record in records {
        out.push_str(&format!("- arm: {:?}\n", record.arm));
        out.push_str(&format!("  instance: {}\n", record.instance_id));
        out.push_str(&format!("  manifest: {}\n", record.manifest_id));
        if let Some(model) = record.model {
            out.push_str(&format!("  model: {model}\n"));
        }
        if let Some(provider) = record.provider {
            out.push_str(&format!("  provider: {provider}\n"));
        }
        out.push_str(&format!("  repo_root: {}\n", record.repo_root.display()));
        out.push_str(&format!(
            "  tools: total={} failed={}\n",
            record.tool_call_count, record.failed_tool_call_count
        ));
        out.push_str(&format!("  turns: {}\n", record.turn_count));
        if let Some(packaging) = record.packaging {
            out.push_str(&format!("  packaging: {packaging:?}\n"));
        }
        if let Some(check) = record.patch_projection_check {
            out.push_str(&format!("  patch_projection_check: {check:?}\n"));
        }
    }
}

fn render_source_ref(out: &mut String, source_ref: &SourceRef<'_>) {
    match source_ref {
        SourceRef::Evidence {
            kind,
            authority,
            recorded_at,
        } => {
            out.push_str(&format!("- {kind}:{authority}"));
            if let Some(recorded_at) = recorded_at {
                out.push_str(&format!(" @{recorded_at}"));
            }
            out.push('\n');
        }
        SourceRef::Diagnostic { severity, code } => {
            out.push_str(&format!("- diagnostic:{severity}:{code}\n"));
        }
        SourceRef::ArtifactHistoryRef { artifact } => {
            out.push_str(&format!("- artifact_history_ref:{artifact}\n"));
        }
        SourceRef::ArtifactId { artifact } => {
            out.push_str(&format!("- artifact_id:{artifact}\n"));
        }
    }
}

fn render_edges(out: &mut String, heading: &str, edges: &[SelectionEdge<'_>]) {
    if edges.is_empty() {
        out.push_str(&format!("{heading}: none\n"));
        return;
    }
    out.push_str(&format!("{heading}:\n"));
    for edge in edges {
        out.push_str(&format!(
            "- {}: {} -> {} ({})\n",
            edge.relation.label(),
            edge.from,
            edge.to,
            edge.source_count
        ));
    }
}

fn render_patches(out: &mut String, patches: &[PatchSnapshot]) {
    if patches.is_empty() {
        out.push_str("patch_debug: none\n");
        return;
    }

    out.push_str(&format!("patch_debug: count={}\n", patches.len()));
    for patch in patches.iter().take(4) {
        out.push_str(&format!("- patch_id: {}\n", patch.patch_id));
        out.push_str(&format!("  target: {}\n", patch.target_relpath));
        out.push_str(&format!("  branch: {}\n", patch.branch_id));
        out.push_str(&format!("  candidate: {}\n", patch.candidate_id));
        out.push_str(&format!("  source_hash: {}\n", patch.source_content_hash));
        out.push_str(&format!(
            "  proposed_hash: {}\n",
            patch.proposed_content_hash
        ));
        if let Some(base) = patch.base_artifact {
            out.push_str(&format!("  base_artifact: {base}\n"));
        }
        if let Some(derived) = patch.derived_artifact {
            out.push_str(&format!("  derived_artifact: {derived}\n"));
        }
        if let Some(check) = patch.check_status {
            out.push_str(&format!("  check: {}\n", surface_check_status_label(check)));
        }
        if let Some(apply) = patch.apply_status {
            out.push_str(&format!("  apply: {}\n", surface_apply_status_label(apply)));
        }
    }
    if patches.len() > 4 {
        out.push_str(&format!("... {} more patch records\n", patches.len() - 4));
    }
}

fn render_unavailable(out: &mut String, unavailable: Option<UnavailableReason>) {
    let Some(reason) = unavailable else {
        out.push_str("unavailable: none\n");
        return;
    };
    out.push_str("unavailable:\n");
    out.push_str(&format!("- {}: {}\n", reason.subject(), reason.state()));
}

fn artifact_ids_selection_kind_label(kind: ArtifactIdsSelectionKind) -> &'static str {
    match kind {
        ArtifactIdsSelectionKind::RunForestNode => "run_forest_node",
        ArtifactIdsSelectionKind::Artifact => "artifact",
        ArtifactIdsSelectionKind::Unresolved => "unresolved",
    }
}

fn render_id_list(out: &mut String, label: &str, ids: &[&str]) {
    if ids.is_empty() {
        out.push_str(&format!("{label}: none\n"));
        return;
    }
    out.push_str(&format!("{label}:\n"));
    for id in ids {
        out.push_str(&format!("- {id}\n"));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use crate::ui::view::{GraphSelectionDetail, GraphSelectionRef};
    use ploke_records::branch::{
        ResolvedTreatmentBranch, TreatmentBranchNode, TreatmentBranchStatus,
    };
    use ploke_records::child_plan::ChildPlanChildRecord;
    use ploke_records::history::SubjectRefRecord;
    use ploke_records::history::{
        SurfaceApplyStatusRecord, SurfaceArtifactRefRecord, SurfaceCheckStatusRecord,
        SurfaceEvidenceRecord, SurfaceTouchRecord,
    };
    use ploke_records::ids::{
        ArtifactId, BranchId, CampaignId, CandidateId, EntryId, HistoryHash, InstanceId, PatchId,
        RuntimeId, SchedulerNodeId, SourceStateId,
    };
    use ploke_records::invocation::{InvocationRecord, Role};
    use ploke_records::scheduler::{NodeRecord, NodeStatusRecord, RunnerRequestRecord};
    use ploke_tree::{
        AuthorityLabel, BranchRunRecordRef, CampaignRef, ComparedRunArm, EvidenceKind, EvidenceRef,
        Lanes, NodeKey, NodeKind, PassiveEvidence, Phase, Progress, ResultClass,
        RunAttemptEvidence, RunAttemptSummary, RunForest, RunRecordEvidence, RunRecordStats,
        RunRecordSummary, Terminality, TreeNode,
    };

    use super::*;

    #[test]
    fn selected_run_forest_node_reports_identity_edges_and_sources() {
        let parent = key("parent");
        let child = key("child");
        let graph = ploke_tree::Graph {
            forest: Some(RunForest {
                campaign: CampaignRef {
                    campaign_id: "campaign".to_owned(),
                    updated_at: "now".to_owned(),
                },
                roots: vec![parent.clone()],
                nodes: vec![
                    node(parent.clone(), None, vec![child.clone()], 0),
                    node(child.clone(), Some(parent.clone()), Vec::new(), 1),
                ],
                lanes: Lanes {
                    frontier: Vec::new(),
                    completed: Vec::new(),
                    failed: Vec::new(),
                },
                passive_evidence: passive_invocations(vec![
                    invocation(
                        "child",
                        "runtime-child",
                        Role::Child,
                        "artifact:child:after",
                    ),
                    invocation(
                        "child",
                        "runtime-parent",
                        Role::Successor,
                        "artifact:child:after",
                    ),
                ]),
                diagnostics: Vec::new(),
            }),
            ..Default::default()
        };
        let selection = GraphSelectionDetail {
            kind: "artifact".to_owned(),
            label: "A2".to_owned(),
            detail: String::new(),
            reference: GraphSelectionRef::RunForestNode {
                key: "child".to_owned(),
            },
        };
        let inspector = SelectionInspector::from_graph(&graph, &selection);

        match &inspector {
            SelectionInspector::RunForestNode(run) => {
                assert_eq!(run.node.key.as_str(), "child");
                assert_eq!(run.parent.unwrap().key.as_str(), "parent");
            }
            _ => panic!("expected run-forest node inspection"),
        }
        let snapshot = inspector.snapshot(&selection);
        assert_eq!(snapshot.label, "A2");
        assert!(snapshot.has_record_refs());
        assert_eq!(snapshot.incoming.len(), 1);
        assert_eq!(snapshot.incoming[0].relation, EdgeRelation::RunForest);
        assert_eq!(snapshot.incoming[0].from, "parent");
        assert_eq!(snapshot.incoming[0].to, "child");
        assert!(snapshot.outgoing.is_empty());
        assert_eq!(snapshot.artifact_outgoing.len(), 1);
        assert_eq!(
            snapshot.artifact_outgoing[0].relation,
            EdgeRelation::AppliedPatch
        );
        assert_eq!(snapshot.artifact_outgoing[0].from, "artifact:child:base");
        assert_eq!(snapshot.artifact_outgoing[0].to, "artifact:child:after");
        let Some(SelectionIdentity::RunForestNode(identity)) = snapshot.identity else {
            panic!("expected run-forest identity");
        };
        assert_eq!(identity.node_key, "child");
        assert_eq!(identity.parent_node, Some("parent"));
        assert_eq!(identity.source_artifact, "artifact:child");
        let Some(SelectionMetrics::RunForestNode(metrics)) = snapshot.metrics else {
            panic!("expected run-forest metrics");
        };
        assert_eq!(metrics.child_run_forest_nodes, 0);
        assert!(
            snapshot
                .roles
                .iter()
                .any(|badge| matches!(badge, Badge::Child(artifact_id) if artifact_id.0 == "artifact:child:after"))
        );
        assert!(
            snapshot
                .roles
                .iter()
                .any(|badge| matches!(badge, Badge::Parent(artifact_id) if artifact_id.0 == "artifact:child:after"))
        );
    }

    #[test]
    fn selected_run_forest_node_exposes_branch_run_record_output_witnesses() {
        let parent = key("parent");
        let child = key("child");
        let mut passive_evidence = PassiveEvidence::default();
        let baseline_key = "/runs/baseline/record.json.gz".to_owned();
        let treatment_key = "/runs/treatment/record.json.gz".to_owned();
        let baseline_record = run_record_fixture("baseline-run", "nonempty", 18, 2);
        let treatment_record = run_record_fixture("treatment-run", "empty", 39, 8);
        passive_evidence.run_records = Some(RunRecordEvidence {
            summary: RunRecordSummary {
                file_count: 2,
                parsed_count: 2,
                branch_ref_count: 2,
                baseline_ref_count: 1,
                treatment_ref_count: 1,
                records_with_setup_count: 2,
                records_with_packaging_count: 2,
                total_turn_count: 2,
                total_tool_call_count: 57,
                failed_tool_call_count: 10,
            },
            index: BTreeMap::from([
                (baseline_key.clone(), baseline_record.clone()),
                (treatment_key.clone(), treatment_record.clone()),
            ]),
            stats: BTreeMap::from([
                (
                    baseline_key.clone(),
                    RunRecordStats::from_record(&baseline_record),
                ),
                (
                    treatment_key.clone(),
                    RunRecordStats::from_record(&treatment_record),
                ),
            ]),
            refs_by_branch: BTreeMap::from([(
                "branch:child".to_owned(),
                vec![
                    BranchRunRecordRef {
                        branch_id: "branch:child".to_owned(),
                        instance_id: "instance:child".to_owned(),
                        arm: ComparedRunArm::Baseline,
                        record_key: baseline_key,
                        record_path: PathBuf::from("/runs/baseline/record.json.gz"),
                    },
                    BranchRunRecordRef {
                        branch_id: "branch:child".to_owned(),
                        instance_id: "instance:child".to_owned(),
                        arm: ComparedRunArm::Treatment,
                        record_key: treatment_key,
                        record_path: PathBuf::from("/runs/treatment/record.json.gz"),
                    },
                ],
            )]),
        });

        let graph = ploke_tree::Graph {
            forest: Some(RunForest {
                campaign: CampaignRef {
                    campaign_id: "campaign".to_owned(),
                    updated_at: "now".to_owned(),
                },
                roots: vec![parent.clone()],
                nodes: vec![
                    node(parent.clone(), None, vec![child.clone()], 0),
                    node(child.clone(), Some(parent.clone()), Vec::new(), 1),
                ],
                lanes: Lanes {
                    frontier: Vec::new(),
                    completed: Vec::new(),
                    failed: Vec::new(),
                },
                passive_evidence,
                diagnostics: Vec::new(),
            }),
            ..Default::default()
        };
        let selection = GraphSelectionDetail {
            kind: "artifact".to_owned(),
            label: "child".to_owned(),
            detail: String::new(),
            reference: GraphSelectionRef::RunForestNode {
                key: "child".to_owned(),
            },
        };
        let inspector = SelectionInspector::from_graph(&graph, &selection);
        let SelectionInspector::RunForestNode(run) = &inspector else {
            panic!("expected run-forest node inspection");
        };

        assert_eq!(run.run_records.iter().count(), 2);
        assert!(run.run_records.iter().any(|record| {
            record.record_ref.arm == ComparedRunArm::Baseline
                && record.record.tool_call_count() == 18
                && record.record.failed_tool_call_count() == 2
        }));
        assert!(run.run_records.iter().any(|record| {
            record.record_ref.arm == ComparedRunArm::Treatment
                && record.record.tool_call_count() == 39
                && record.record.failed_tool_call_count() == 8
        }));

        let snapshot = inspector.snapshot(&selection);
        assert_eq!(snapshot.run_records.len(), 2);
        assert_eq!(snapshot.run_records[0].arm, ComparedRunArm::Baseline);
        assert_eq!(snapshot.run_records[0].tool_call_count, 18);
        assert_eq!(snapshot.run_records[1].arm, ComparedRunArm::Treatment);
        assert_eq!(snapshot.run_records[1].failed_tool_call_count, 8);
        assert!(snapshot.render_text().contains("run_records: count=2"));
    }

    #[test]
    fn selected_artifact_reports_typed_identity_metrics_and_patch_edge() {
        let graph = artifact_graph_with_applied_patch_edge();
        let (selection, inspector) = SelectionInspector::from_default_selector(&graph, "after")
            .expect("artifact key resolves from default selections");
        let snapshot = inspector.snapshot(&selection);

        let Some(SelectionIdentity::Artifact(identity)) = snapshot.identity else {
            panic!("expected artifact identity");
        };
        assert_eq!(identity.artifact, "artifact:after");
        let Some(SelectionMetrics::Artifact(metrics)) = snapshot.metrics else {
            panic!("expected artifact metrics");
        };
        assert_eq!(metrics.source_records, 1);
        assert_eq!(metrics.evidence_refs, 0);
        assert_eq!(snapshot.incoming.len(), 2);
        assert_eq!(snapshot.incoming[0].relation, EdgeRelation::ProducedChild);
        assert_eq!(snapshot.incoming[0].from, "base");
        assert_eq!(snapshot.incoming[0].to, "after");
        assert_eq!(snapshot.incoming[1].relation, EdgeRelation::AppliedPatch);
        assert_eq!(snapshot.patches.len(), 1);
        assert_eq!(snapshot.run_records.len(), 2);
        assert_eq!(snapshot.run_records[0].arm, ComparedRunArm::Baseline);
        assert_eq!(snapshot.run_records[0].tool_call_count, 2);
        assert_eq!(snapshot.run_records[1].arm, ComparedRunArm::Treatment);
        assert_eq!(snapshot.run_records[1].tool_call_count, 3);
        assert_eq!(
            snapshot
                .parent_create
                .as_ref()
                .expect("parent create snapshot")
                .state,
            ParentCreateState::Available
        );
        assert_eq!(
            snapshot.source_refs,
            vec![SourceRef::ArtifactId {
                artifact: "artifact:after"
            }]
        );
    }

    #[test]
    fn selected_artifact_exposes_borrowed_identity_witness_for_live_inspector() {
        let graph = artifact_graph_with_mixed_identity_sources();
        let (_, inspector) = SelectionInspector::from_default_selector(&graph, "after")
            .expect("artifact key resolves from default selections");
        let history_node = graph
            .artifacts
            .artifacts
            .get(&ploke_tree::graph::ArtifactKey::HistoryRef {
                id: ploke_records::history::ArtifactRefRecord::from_artifact_id(ArtifactId(
                    "artifact:after".to_owned(),
                ))
                .id()
                .0
                .clone(),
            })
            .expect("history artifact source exists");
        let passive_node = graph
            .artifacts
            .artifacts
            .get(&ploke_tree::graph::ArtifactKey::PassiveId {
                value: "artifact:after".to_owned(),
            })
            .expect("passive artifact source exists");

        let SelectionInspector::Artifact(artifact) = inspector else {
            panic!("expected artifact inspection");
        };
        let identity = artifact.identity();

        assert_eq!(identity.artifact(), "artifact:after");
        for source in identity.sources {
            assert_eq!(
                source
                    .artifact_ids()
                    .iter()
                    .map(|artifact| artifact.0.as_str())
                    .collect::<Vec<_>>(),
                vec!["artifact:after"]
            );
            assert_eq!(
                source
                    .artifact_refs()
                    .iter()
                    .map(|artifact| artifact.as_str())
                    .collect::<Vec<_>>(),
                vec!["artifact:after"]
            );
        }

        let mut saw_history_ref = false;
        let mut saw_passive_id = false;
        for source in identity.sources {
            match &source.identity {
                ploke_tree::graph::ArtifactIdentity::HistoryRef(record) => {
                    assert_eq!(record.as_str(), "artifact:after");
                    let ploke_tree::graph::ArtifactIdentity::HistoryRef(expected) =
                        &history_node.identity
                    else {
                        panic!("expected history artifact identity");
                    };
                    assert!(std::ptr::eq(record, expected));
                    saw_history_ref = true;
                }
                ploke_tree::graph::ArtifactIdentity::PassiveId(record) => {
                    assert_eq!(record.0, "artifact:after");
                    let ploke_tree::graph::ArtifactIdentity::PassiveId(expected) =
                        &passive_node.identity
                    else {
                        panic!("expected passive artifact identity");
                    };
                    assert!(std::ptr::eq(record, expected));
                    saw_passive_id = true;
                }
            }
        }

        assert!(saw_history_ref);
        assert!(saw_passive_id);
    }

    #[test]
    fn artifact_ids_section_reports_rendered_ids_for_artifact_selection() {
        let graph = artifact_graph_with_mixed_identity_sources();
        let (_, inspector) = SelectionInspector::from_default_selector(&graph, "after")
            .expect("artifact key resolves from default selections");

        let section = inspector.artifact_ids_section();
        let rendered = section.render_text();

        assert_eq!(section.selection_kind, ArtifactIdsSelectionKind::Artifact);
        let ArtifactIdsSectionState::Rendered {
            selection_key,
            artifact_ids,
            artifact_refs,
            tree_keys,
        } = section.state
        else {
            panic!("expected rendered artifact ids section");
        };
        assert_eq!(selection_key, "artifact:after");
        assert_eq!(artifact_ids, vec!["artifact:after"]);
        assert_eq!(artifact_refs, vec!["artifact:after"]);
        assert!(tree_keys.is_empty());
        assert!(rendered.contains("selection_kind: artifact"));
        assert!(rendered.contains("artifact_ids: rendered"));
        assert!(rendered.contains("artifact_id:\n- artifact:after"));
    }

    #[test]
    fn artifact_ids_section_reports_not_applicable_for_explicit_run_forest_selection() {
        let parent = key("parent");
        let child = key("child");
        let graph = ploke_tree::Graph {
            forest: Some(RunForest {
                campaign: CampaignRef {
                    campaign_id: "campaign".to_owned(),
                    updated_at: "now".to_owned(),
                },
                roots: vec![parent.clone()],
                nodes: vec![
                    node(parent.clone(), None, vec![child.clone()], 0),
                    node(child.clone(), Some(parent.clone()), Vec::new(), 1),
                ],
                lanes: Lanes {
                    frontier: Vec::new(),
                    completed: Vec::new(),
                    failed: Vec::new(),
                },
                passive_evidence: passive_invocations(Vec::new()),
                diagnostics: Vec::new(),
            }),
            ..Default::default()
        };
        let selection = GraphSelectionDetail {
            kind: "artifact".to_owned(),
            label: "child".to_owned(),
            detail: String::new(),
            reference: GraphSelectionRef::RunForestNode {
                key: "child".to_owned(),
            },
        };
        let inspector = SelectionInspector::from_graph(&graph, &selection);

        let section = inspector.artifact_ids_section();
        let rendered = section.render_text();

        assert_eq!(
            section.selection_kind,
            ArtifactIdsSelectionKind::RunForestNode
        );
        let ArtifactIdsSectionState::NotApplicable { selection_key } = section.state else {
            panic!("expected not_applicable artifact ids section");
        };
        assert_eq!(selection_key, "child");
        assert!(rendered.contains("selection_kind: run_forest_node"));
        assert!(rendered.contains("artifact_ids: not_applicable"));
    }

    #[test]
    fn default_selector_resolves_artifact_graph_key_even_when_forest_is_present() {
        let mut graph = artifact_graph_with_mixed_identity_sources();
        graph.forest = Some(RunForest {
            campaign: CampaignRef {
                campaign_id: "campaign".to_owned(),
                updated_at: "now".to_owned(),
            },
            roots: vec![key("parent")],
            nodes: vec![
                node(key("parent"), None, vec![key("child")], 0),
                node(key("child"), Some(key("parent")), Vec::new(), 1),
            ],
            lanes: Lanes {
                frontier: Vec::new(),
                completed: Vec::new(),
                failed: Vec::new(),
            },
            passive_evidence: passive_invocations(Vec::new()),
            diagnostics: Vec::new(),
        });

        let (_, inspector) = SelectionInspector::from_default_selector(&graph, "after")
            .expect("artifact graph key resolves even when forest is present");

        assert!(matches!(inspector, SelectionInspector::Artifact(_)));
    }

    #[test]
    fn default_visible_selections_prefer_artifact_tree_even_when_forest_is_present() {
        let mut graph = artifact_graph_with_mixed_identity_sources();
        graph.forest = Some(RunForest {
            campaign: CampaignRef {
                campaign_id: "campaign".to_owned(),
                updated_at: "now".to_owned(),
            },
            roots: vec![key("parent")],
            nodes: vec![
                node(key("parent"), None, vec![key("child")], 0),
                node(key("child"), Some(key("parent")), Vec::new(), 1),
            ],
            lanes: Lanes {
                frontier: Vec::new(),
                completed: Vec::new(),
                failed: Vec::new(),
            },
            passive_evidence: passive_invocations(Vec::new()),
            diagnostics: Vec::new(),
        });

        let (selection, inspector) = SelectionInspector::from_default_selector(&graph, "A1")
            .expect("A1 resolves from visible default artifact selections");

        assert!(matches!(
            selection.reference,
            GraphSelectionRef::Artifact { .. }
        ));
        assert!(matches!(inspector, SelectionInspector::Artifact(_)));
    }

    #[test]
    fn unresolved_selection_carries_typed_unavailable_reason() {
        let selection = GraphSelectionDetail {
            kind: "artifact".to_owned(),
            label: "missing".to_owned(),
            detail: String::new(),
            reference: GraphSelectionRef::Artifact {
                key: "artifact:missing".to_owned(),
            },
        };
        let graph = ploke_tree::Graph::default();
        let inspector = SelectionInspector::from_graph(&graph, &selection);
        let snapshot = inspector.snapshot(&selection);

        assert_eq!(snapshot.identity, None);
        assert_eq!(snapshot.metrics, None);
        assert_eq!(
            snapshot.unavailable,
            Some(UnavailableReason::ArtifactNotFound)
        );
    }

    #[test]
    fn patch_debug_snapshot_uses_named_child_plan_fields() {
        let child = child_plan_child_record();
        let snapshot = patch_snapshot(PatchInspection { child: &child });

        assert_eq!(snapshot.patch_id, "patch:attempt-1");
        assert_eq!(snapshot.target_relpath, "src/lib.rs");
        assert_eq!(snapshot.branch_id, "branch-child");
        assert_eq!(snapshot.candidate_id, "candidate-1");
        assert_eq!(snapshot.source_content_hash, "sha256:source");
        assert_eq!(snapshot.proposed_content_hash, "sha256:proposed");
        assert_eq!(snapshot.base_artifact, Some("artifact:base"));
        assert_eq!(snapshot.derived_artifact, Some("artifact:after"));
        assert_eq!(
            snapshot.check_status,
            Some(SurfaceCheckStatusRecord::Checked)
        );
        assert_eq!(
            snapshot.apply_status,
            Some(SurfaceApplyStatusRecord::Applied)
        );
        assert_eq!(snapshot.touches.len(), 1);
        assert_eq!(snapshot.touches[0].replacement, "println!(\"hi\");");

        let json = serde_json::to_value(&snapshot).expect("serialize patch snapshot");
        assert!(json.get("summary").is_none());
        assert_eq!(json["branch_id"], "branch-child");
        assert_eq!(json["check_status"], "checked");
    }

    #[test]
    fn typed_edge_relation_classification_covers_all_visible_relations() {
        let graph = graph_with_parent_child();
        let selection = GraphSelectionDetail {
            kind: "artifact".to_owned(),
            label: "A2".to_owned(),
            detail: String::new(),
            reference: GraphSelectionRef::RunForestNode {
                key: "child".to_owned(),
            },
        };
        let inspector = SelectionInspector::from_graph(&graph, &selection);
        let SelectionInspector::RunForestNode(run) = inspector else {
            panic!("expected run-forest node inspection");
        };
        let incoming = run_forest_incoming_edges(&run).collect::<Vec<_>>();
        assert_eq!(incoming[0].relation, EdgeRelation::RunForest);

        let relations = vec![
            ArtifactRelation {
                kind: ArtifactRelationKind::HistoryPatch,
                from: "artifact:old",
                to: "artifact:new",
                source_count: 1,
                patch_ids: Vec::new(),
            },
            ArtifactRelation {
                kind: ArtifactRelationKind::ProducedChild,
                from: "artifact:parent",
                to: "artifact:child",
                source_count: 1,
                patch_ids: Vec::new(),
            },
            ArtifactRelation {
                kind: ArtifactRelationKind::AppliedPatch,
                from: "artifact:base",
                to: "artifact:after",
                source_count: 1,
                patch_ids: Vec::new(),
            },
            ArtifactRelation {
                kind: ArtifactRelationKind::HistoryOpenedFrom,
                from: "artifact:opened",
                to: "artifact:active",
                source_count: 1,
                patch_ids: Vec::new(),
            },
        ];
        let edges = artifact_edges(&relations).collect::<Vec<_>>();
        assert_eq!(edges[0].relation, EdgeRelation::HistoryPatch);
        assert_eq!(edges[1].relation, EdgeRelation::ProducedChild);
        assert_eq!(edges[2].relation, EdgeRelation::AppliedPatch);
        assert_eq!(edges[3].relation, EdgeRelation::HistoryOpenedFrom);
    }

    #[test]
    fn inspector_cache_reuses_section_slices_for_stable_selection() {
        let graph = artifact_graph_with_applied_patch_edge();
        let (selection, inspector) = SelectionInspector::from_default_selector(&graph, "after")
            .expect("artifact key resolves from default selections");
        let snapshot = inspector.snapshot(&selection);
        let mut cache = InspectorCache::default();
        let revision = GraphRevision::default();

        {
            let sections = cache
                .sections(&graph, revision, Some(&selection.reference))
                .expect("cached sections");
            let _: &[ArtifactRelationSlot] = sections.artifact_edges_in();
            let _: &[PatchSlot] = sections.patches();
            let _: &[RunRecordSlot] = sections.run_records();
            assert_eq!(
                sections.artifact_edges_in().len(),
                snapshot.artifact_incoming.len()
            );
            assert_eq!(
                sections.artifact_edges_out().len(),
                snapshot.artifact_outgoing.len()
            );
            assert_eq!(sections.patches().len(), snapshot.patches.len());
            assert_eq!(sections.run_records().len(), snapshot.run_records.len());
        }
        assert_eq!(cache.rebuilds(), 1);

        cache.sections(&graph, revision, Some(&selection.reference));
        assert_eq!(cache.rebuilds(), 1);

        cache.sections(&graph, revision.next(), Some(&selection.reference));
        assert_eq!(cache.rebuilds(), 2);

        let other = GraphSelectionRef::Artifact {
            key: "base".to_owned(),
        };
        cache.sections(&graph, revision.next(), Some(&other));
        assert_eq!(cache.rebuilds(), 3);
    }

    #[test]
    fn inspector_cache_slots_resolve_against_current_graph() {
        let graph = artifact_graph_with_applied_patch_edge();
        let (selection, _) = SelectionInspector::from_default_selector(&graph, "after")
            .expect("artifact key resolves from default selections");
        let mut cache = InspectorCache::default();
        let sections = cache
            .sections(&graph, GraphRevision::default(), Some(&selection.reference))
            .expect("cached sections");

        let patch = sections.patches()[0]
            .resolve(&graph)
            .expect("patch slot resolves");
        assert_eq!(patch.patch_id(), "patch:attempt-1");
        let record = sections.run_records()[0]
            .resolve(&graph)
            .expect("run-record slot resolves");
        assert_eq!(record.record_ref.branch_id, "branch-child");
        let source_ref = sections.source_refs()[0]
            .resolve(&graph)
            .expect("source slot resolves");
        assert_eq!(
            source_ref,
            SourceRef::ArtifactId {
                artifact: "artifact:after"
            }
        );
    }

    #[test]
    fn selection_snapshot_serializes_typed_schema() {
        let graph = artifact_graph_with_applied_patch_edge();
        let (selection, inspector) = SelectionInspector::from_default_selector(&graph, "after")
            .expect("artifact key resolves from default selections");
        let snapshot = inspector.snapshot(&selection);
        let json = serde_json::to_value(&snapshot).expect("serialize selection snapshot");

        assert_eq!(json["identity"]["artifact"]["artifact"], "artifact:after");
        assert_eq!(json["metrics"]["artifact"]["source_records"], 1);
        assert_eq!(json["incoming"][0]["relation"], "P_C");
        assert_eq!(json["incoming"][1]["relation"], "P_B");
        assert_eq!(json["unavailable"], serde_json::Value::Null);
    }

    fn key(value: &str) -> NodeKey {
        NodeKey::from(value)
    }

    fn graph_with_parent_child() -> ploke_tree::Graph {
        let parent = key("parent");
        let child = key("child");
        ploke_tree::Graph {
            forest: Some(RunForest {
                campaign: CampaignRef {
                    campaign_id: "campaign".to_owned(),
                    updated_at: "now".to_owned(),
                },
                roots: vec![parent.clone()],
                nodes: vec![
                    node(parent.clone(), None, vec![child.clone()], 0),
                    node(child, Some(parent), Vec::new(), 1),
                ],
                lanes: Lanes {
                    frontier: Vec::new(),
                    completed: Vec::new(),
                    failed: Vec::new(),
                },
                passive_evidence: PassiveEvidence::default(),
                diagnostics: Vec::new(),
            }),
            ..Default::default()
        }
    }

    fn artifact_graph_with_applied_patch_edge() -> ploke_tree::Graph {
        let base = ArtifactId("artifact:base".to_owned());
        let after = ArtifactId("artifact:after".to_owned());
        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            ploke_tree::graph::ArtifactKey::PassiveId {
                value: base.0.clone(),
            },
            ploke_tree::graph::ArtifactNode {
                key: ploke_tree::graph::ArtifactKey::PassiveId {
                    value: base.0.clone(),
                },
                identity: ploke_tree::graph::ArtifactIdentity::PassiveId(base.clone()),
                ids: ploke_tree::graph::ArtifactIds {
                    artifact_ids: vec![base.clone()],
                    ..Default::default()
                },
                evidence: Vec::new(),
            },
        );
        artifacts.insert(
            ploke_tree::graph::ArtifactKey::PassiveId {
                value: after.0.clone(),
            },
            ploke_tree::graph::ArtifactNode {
                key: ploke_tree::graph::ArtifactKey::PassiveId {
                    value: after.0.clone(),
                },
                identity: ploke_tree::graph::ArtifactIdentity::PassiveId(after.clone()),
                ids: ploke_tree::graph::ArtifactIds {
                    artifact_ids: vec![after.clone()],
                    ..Default::default()
                },
                evidence: Vec::new(),
            },
        );

        let baseline_key = "/runs/artifact-baseline/record.json.gz".to_owned();
        let treatment_key = "/runs/artifact-treatment/record.json.gz".to_owned();
        let baseline_record = run_record_fixture("artifact-baseline-run", "nonempty", 2, 1);
        let treatment_record = run_record_fixture("artifact-treatment-run", "empty", 3, 0);
        let mut passive_evidence = PassiveEvidence::default();
        passive_evidence.run_records = Some(RunRecordEvidence {
            summary: RunRecordSummary {
                file_count: 2,
                parsed_count: 2,
                branch_ref_count: 2,
                baseline_ref_count: 1,
                treatment_ref_count: 1,
                records_with_setup_count: 0,
                records_with_packaging_count: 2,
                total_turn_count: 2,
                total_tool_call_count: 5,
                failed_tool_call_count: 1,
            },
            index: BTreeMap::from([
                (baseline_key.clone(), baseline_record.clone()),
                (treatment_key.clone(), treatment_record.clone()),
            ]),
            stats: BTreeMap::from([
                (
                    baseline_key.clone(),
                    RunRecordStats::from_record(&baseline_record),
                ),
                (
                    treatment_key.clone(),
                    RunRecordStats::from_record(&treatment_record),
                ),
            ]),
            refs_by_branch: BTreeMap::from([(
                "branch-child".to_owned(),
                vec![
                    BranchRunRecordRef {
                        branch_id: "branch-child".to_owned(),
                        instance_id: "instance-1".to_owned(),
                        arm: ComparedRunArm::Baseline,
                        record_key: baseline_key,
                        record_path: PathBuf::from("/runs/artifact-baseline/record.json.gz"),
                    },
                    BranchRunRecordRef {
                        branch_id: "branch-child".to_owned(),
                        instance_id: "instance-1".to_owned(),
                        arm: ComparedRunArm::Treatment,
                        record_key: treatment_key,
                        record_path: PathBuf::from("/runs/artifact-treatment/record.json.gz"),
                    },
                ],
            )]),
        });

        ploke_tree::Graph {
            forest: Some(RunForest {
                campaign: CampaignRef {
                    campaign_id: "campaign".to_owned(),
                    updated_at: "now".to_owned(),
                },
                roots: Vec::new(),
                nodes: Vec::new(),
                lanes: Lanes {
                    frontier: Vec::new(),
                    completed: Vec::new(),
                    failed: Vec::new(),
                },
                passive_evidence,
                diagnostics: Vec::new(),
            }),
            artifacts: ploke_tree::graph::ArtifactIndex { artifacts },
            candidates: ploke_tree::graph::CandidateIndex {
                candidates: vec![ploke_tree::graph::CandidateNode {
                    selection_entry_id: EntryId("entry:base".to_owned()),
                    payload_index: 0,
                    subject: SubjectRefRecord {
                        value: "candidate:base".to_owned(),
                    },
                    source: Some(ploke_tree::graph::CandidateSource::CurrentGeneration),
                    occurrence_id: None,
                    membership_id: None,
                    membership_key: None,
                    node_id: Some("node-base".to_owned()),
                    branch_id: Some("branch-base".to_owned()),
                    generation: Some(0),
                    primary_runtime_id: None,
                    artifact_after: Some(base.clone()),
                    patch_id: None,
                    evidence: Vec::new(),
                }],
                branches: vec![ploke_tree::graph::CandidateBranchNode {
                    selection_entry_id: EntryId("entry:1".to_owned()),
                    payload_index: 0,
                    branch_id: "branch-child".to_owned(),
                    candidate_id: Some(CandidateId("candidate-1".to_owned())),
                    source_state_id: Some("artifact:base".to_owned()),
                    parent_branch_id: None,
                    base_artifact_id: Some(base),
                    derived_artifact_id: Some(after),
                    patch_id: Some(PatchId("patch:attempt-1".to_owned())),
                    evidence: Vec::new(),
                }],
                ..Default::default()
            },
            child_plans: ploke_tree::graph::ChildPlanIndex {
                plans: BTreeMap::from([(
                    SchedulerNodeId("node-base".to_owned()),
                    child_plan_record(),
                )]),
            },
            ..Default::default()
        }
    }

    fn child_plan_record() -> ploke_records::child_plan::ChildPlanRecord {
        let child = child_plan_child_record();
        ploke_records::child_plan::ChildPlanRecord {
            message: PathBuf::from("/tmp/node-base.json"),
            parent_node_id: SchedulerNodeId("node-base".to_owned()),
            child_generation: 1,
            children: vec![child],
            rejected_surface_attempts: Vec::new(),
        }
    }

    fn artifact_graph_with_mixed_identity_sources() -> ploke_tree::Graph {
        let history_after = ploke_records::history::ArtifactRefRecord::from_artifact_id(
            ArtifactId("artifact:after".to_owned()),
        );
        let passive_after = ArtifactId("artifact:after".to_owned());
        let ids = ploke_tree::graph::ArtifactIds {
            artifact_ids: vec![passive_after.clone()],
            artifact_refs: vec![history_after.clone()],
            ..Default::default()
        };
        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            ploke_tree::graph::ArtifactKey::HistoryRef {
                id: history_after.id().0.clone(),
            },
            ploke_tree::graph::ArtifactNode {
                key: ploke_tree::graph::ArtifactKey::HistoryRef {
                    id: history_after.id().0.clone(),
                },
                identity: ploke_tree::graph::ArtifactIdentity::HistoryRef(history_after),
                ids: ids.clone(),
                evidence: Vec::new(),
            },
        );
        artifacts.insert(
            ploke_tree::graph::ArtifactKey::PassiveId {
                value: passive_after.0.clone(),
            },
            ploke_tree::graph::ArtifactNode {
                key: ploke_tree::graph::ArtifactKey::PassiveId {
                    value: passive_after.0.clone(),
                },
                identity: ploke_tree::graph::ArtifactIdentity::PassiveId(passive_after),
                ids,
                evidence: Vec::new(),
            },
        );

        ploke_tree::Graph {
            artifacts: ploke_tree::graph::ArtifactIndex { artifacts },
            candidates: ploke_tree::graph::CandidateIndex {
                candidates: vec![ploke_tree::graph::CandidateNode {
                    selection_entry_id: EntryId("entry:after".to_owned()),
                    payload_index: 0,
                    subject: SubjectRefRecord {
                        value: "candidate:after".to_owned(),
                    },
                    source: Some(ploke_tree::graph::CandidateSource::CurrentGeneration),
                    occurrence_id: None,
                    membership_id: None,
                    membership_key: None,
                    node_id: Some("node-after".to_owned()),
                    branch_id: Some("branch-after".to_owned()),
                    generation: Some(1),
                    primary_runtime_id: None,
                    artifact_after: Some(ArtifactId("artifact:after".to_owned())),
                    patch_id: None,
                    evidence: Vec::new(),
                }],
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn run_record_fixture(
        manifest_id: &str,
        submission: &str,
        tool_calls: usize,
        failed_tools: usize,
    ) -> ploke_records::run_record::RunRecord {
        let mut calls = Vec::new();
        for index in 0..tool_calls {
            let failed = index < failed_tools;
            let result = if failed {
                serde_json::json!({
                    "status": "Failed",
                    "request_id": format!("request-{index}"),
                    "parent_id": "parent",
                    "call_id": format!("call-{index}"),
                    "tool": "read_file",
                    "error": "fixture failure",
                    "ui_payload": null,
                    "latency_ms": 1
                })
            } else {
                serde_json::json!({
                    "status": "Completed",
                    "request_id": format!("request-{index}"),
                    "parent_id": "parent",
                    "call_id": format!("call-{index}"),
                    "tool": "read_file",
                    "content": "fixture",
                    "ui_payload": null,
                    "latency_ms": 1
                })
            };
            calls.push(serde_json::json!({
                "request": {
                    "request_id": format!("request-{index}"),
                    "parent_id": "parent",
                    "call_id": format!("call-{index}"),
                    "tool": "read_file",
                    "arguments": "{\"path\":\"src/lib.rs\"}"
                },
                "result": result,
                "latency_ms": 1
            }));
        }

        serde_json::from_value(serde_json::json!({
            "schema_version": "run-record.v1",
            "manifest_id": manifest_id,
            "metadata": {
                "run_arm": {
                    "id": "structured-current-policy",
                    "role": "treatment",
                    "command": "run single agent",
                    "execution": "agent-single-turn"
                },
                "benchmark": {
                    "instance_id": "instance:child",
                    "repo_root": "/tmp/repo",
                    "base_sha": "abc123",
                    "issue": null
                },
                "agent": {
                    "selected_model": "fixture/model",
                    "selected_provider": "fixture-provider"
                },
                "runtime": {},
                "budget": {
                    "max_turns": 40,
                    "max_tool_calls": 200,
                    "wall_clock_secs": 1800
                }
            },
            "phases": {
                "agent_turns": [{
                    "turn_number": 1,
                    "started_at": "2026-05-16T00:00:00Z",
                    "ended_at": "2026-05-16T00:00:01Z",
                    "db_timestamp_micros": 1,
                    "issue_prompt": "fixture",
                    "tool_calls": calls,
                    "outcome": {
                        "type": "ToolCalls",
                        "count": tool_calls
                    }
                }],
                "packaging": {
                    "started_at": "2026-05-16T00:00:01Z",
                    "ended_at": "2026-05-16T00:00:02Z",
                    "submission_artifact_state": submission,
                    "patch_projection_check_state": "passed"
                }
            },
            "db_time_travel_index": [],
            "conversation": []
        }))
        .expect("run record fixture")
    }

    fn node(
        key: NodeKey,
        parent: Option<NodeKey>,
        children: Vec<NodeKey>,
        generation: u32,
    ) -> TreeNode {
        TreeNode {
            key: key.clone(),
            kind: NodeKind::SchedulerSearchNode,
            authority: AuthorityLabel::MutableProjection,
            parent,
            children,
            generation,
            branch_id: format!("branch:{}", key.as_str()),
            parent_branch_id: None,
            candidate_id: format!("candidate:{}", key.as_str()),
            instance_id: format!("instance:{}", key.as_str()),
            source_state_id: format!("artifact:{}", key.as_str()),
            target_relpath: "target.rs".to_owned(),
            base_artifact_id: Some(format!("artifact:{}:base", key.as_str())),
            patch_id: None,
            derived_artifact_id: Some(format!("artifact:{}:after", key.as_str())),
            progress: Progress {
                phase: Phase::Completed,
                terminality: Terminality::Terminal,
                result_class: ResultClass::Success,
            },
            created_at: "created".to_owned(),
            updated_at: "updated".to_owned(),
            evidence: vec![EvidenceRef {
                kind: EvidenceKind::SchedulerNode,
                authority: AuthorityLabel::MutableProjection,
                node_key: Some(key),
                runtime_id: None,
                recorded_at: Some("updated".to_owned()),
                detail: None,
            }],
            diagnostics: Vec::new(),
        }
    }

    fn passive_invocations(invocations: Vec<InvocationRecord>) -> PassiveEvidence {
        let child_invocation_count = invocations
            .iter()
            .filter(|invocation| invocation.role == Role::Child)
            .count();
        let successor_invocation_count = invocations
            .iter()
            .filter(|invocation| invocation.role == Role::Successor)
            .count();
        let invocation_file_count = invocations.len();
        let invocations = invocations
            .into_iter()
            .map(|invocation| {
                (
                    format!(
                        "nodes/{}/invocations/{}.json",
                        invocation.node_id, invocation.runtime_id.0
                    ),
                    invocation,
                )
            })
            .collect::<BTreeMap<_, _>>();

        PassiveEvidence {
            run_attempts: Some(RunAttemptEvidence {
                summary: RunAttemptSummary {
                    invocation_file_count,
                    invocation_parsed_count: invocation_file_count,
                    child_invocation_count,
                    successor_invocation_count,
                    ..Default::default()
                },
                invocations,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn invocation(
        node_id: &str,
        runtime_id: &str,
        role: Role,
        derived_artifact_id: &str,
    ) -> InvocationRecord {
        InvocationRecord {
            schema_version: "prototype1-invocation.v1".to_owned(),
            role,
            campaign_id: "campaign".to_owned(),
            node_id: node_id.to_owned(),
            runtime_id: RuntimeId(runtime_id.to_owned()),
            journal_path: PathBuf::from("transition-journal.jsonl"),
            channel_root: None,
            node: None,
            request: Some(RunnerRequestRecord {
                schema_version: "prototype1-runner-request.v1".to_owned(),
                campaign_id: CampaignId("campaign".to_owned()),
                node_id: SchedulerNodeId(node_id.to_owned()),
                generation: 1,
                instance_id: InstanceId(format!("instance:{node_id}")),
                source_state_id: SourceStateId(format!("artifact:{node_id}")),
                operation_target: None,
                base_artifact_id: Some(ArtifactId(format!("artifact:{node_id}:base"))),
                patch_id: None,
                derived_artifact_id: Some(ArtifactId(derived_artifact_id.to_owned())),
                branch_id: BranchId(format!("branch:{node_id}")),
                target_relpath: PathBuf::from("target.rs"),
                workspace_root: PathBuf::from("."),
                binary_path: PathBuf::from("target/debug/ploke-eval"),
                stop_on_error: false,
                runner_args: Vec::new(),
            }),
            resolved: None,
            active_parent_root: None,
            created_at: "created".to_owned(),
        }
    }

    fn child_plan_child_record() -> ChildPlanChildRecord {
        let patch_id = PatchId("patch:attempt-1".to_owned());
        let base = ArtifactId("artifact:base".to_owned());
        let after = ArtifactId("artifact:after".to_owned());
        let branch = BranchId("branch-child".to_owned());
        let candidate = CandidateId("candidate-1".to_owned());
        let instance = InstanceId("instance-1".to_owned());
        let source = SourceStateId("source-1".to_owned());
        let node_id = SchedulerNodeId("child-1".to_owned());

        ChildPlanChildRecord {
            node: NodeRecord {
                schema_version: "prototype1-treatment-node.v1".to_owned(),
                node_id: node_id.clone(),
                parent_node_id: Some(SchedulerNodeId("parent-1".to_owned())),
                generation: 1,
                instance_id: instance.clone(),
                source_state_id: source.clone(),
                operation_target: None,
                base_artifact_id: Some(base.clone()),
                patch_id: Some(patch_id.clone()),
                derived_artifact_id: Some(after.clone()),
                parent_branch_id: None,
                branch_id: branch.clone(),
                candidate_id: candidate.clone(),
                target_relpath: PathBuf::from("src/lib.rs"),
                node_dir: PathBuf::from("nodes/child-1"),
                workspace_root: PathBuf::from("worktrees/child-1"),
                binary_path: PathBuf::from("target/debug/ploke-eval"),
                runner_request_path: PathBuf::from("nodes/child-1/runner-request.json"),
                runner_result_path: PathBuf::from("nodes/child-1/runner-result.json"),
                status: NodeStatusRecord::Planned,
                created_at: "created".to_owned(),
                updated_at: "updated".to_owned(),
            },
            request: RunnerRequestRecord {
                schema_version: "prototype1-runner-request.v1".to_owned(),
                campaign_id: CampaignId("campaign".to_owned()),
                node_id,
                generation: 1,
                instance_id: instance.clone(),
                source_state_id: source.clone(),
                operation_target: None,
                base_artifact_id: Some(base.clone()),
                patch_id: Some(patch_id.clone()),
                derived_artifact_id: Some(after.clone()),
                branch_id: branch.clone(),
                target_relpath: PathBuf::from("src/lib.rs"),
                workspace_root: PathBuf::from("worktrees/child-1"),
                binary_path: PathBuf::from("target/debug/ploke-eval"),
                stop_on_error: false,
                runner_args: Vec::new(),
            },
            resolved: ResolvedTreatmentBranch {
                instance_id: instance.0.clone(),
                source_state_id: source.0.clone(),
                parent_branch_id: None,
                target_relpath: PathBuf::from("src/lib.rs"),
                source_content: "fn main() {}\n".to_owned(),
                source_content_hash: "sha256:source".to_owned(),
                selected_branch_id: Some(branch.0.clone()),
                branch: TreatmentBranchNode {
                    branch_id: branch.0.clone(),
                    candidate_id: candidate.0.clone(),
                    patch_id: Some(patch_id.clone()),
                    branch_label: "candidate-1".to_owned(),
                    synthesized_spec_id: "spec-1".to_owned(),
                    proposed_content: "fn main() { println!(\"hi\"); }\n".to_owned(),
                    proposed_content_hash: "sha256:proposed".to_owned(),
                    generation_target: None,
                    generation_coordinate: None,
                    status: TreatmentBranchStatus::Synthesized,
                    apply_id: None,
                    applied_content_hash: None,
                    derived_artifact_id: Some(after.clone()),
                    latest_evaluation: None,
                },
            },
            surface: Some(SurfaceEvidenceRecord {
                schema_version: 2,
                producer_id: "prototype1:tui-edit-surface:deterministic-v1".to_owned(),
                proposal_id: "proposal-accepted".to_owned(),
                run_id: "run-accepted".to_owned(),
                policy: "workspace_except_ploke_eval".to_owned(),
                target_relpath: PathBuf::from("src/lib.rs"),
                base: SurfaceArtifactRefRecord {
                    artifact_id: base,
                    hash: "sha256:source".to_owned(),
                },
                after: SurfaceArtifactRefRecord {
                    artifact_id: after,
                    hash: "sha256:applied".to_owned(),
                },
                patch_id,
                source_content_hash: "sha256:source".to_owned(),
                proposed_content_hash: "sha256:proposed".to_owned(),
                proposal_producer: None,
                generator_surface: None,
                touches: vec![SurfaceTouchRecord {
                    target_relpath: PathBuf::from("src/lib.rs"),
                    target_name: "direct-splice:eof-comment".to_owned(),
                    span_relpath: PathBuf::from("src/lib.rs"),
                    start: 12,
                    end: 12,
                    base_hash: "sha256:source".to_owned(),
                    replacement: "println!(\"hi\");".to_owned(),
                    replacement_hash: "sha256:replacement".to_owned(),
                }],
                touches_digest: HistoryHash("sha256:touches".to_owned()),
                delta_id: "surface-delta:sha256:delta".to_owned(),
                delta_digest: HistoryHash("sha256:delta".to_owned()),
                check_status: SurfaceCheckStatusRecord::Checked,
                apply_status: SurfaceApplyStatusRecord::Applied,
            }),
        }
    }
}
