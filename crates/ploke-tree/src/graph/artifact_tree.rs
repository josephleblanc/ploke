use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet},
};

use ploke_records::child_plan::ChildPlanChildRecord;
use ploke_records::history::ArtifactRefRecord;
use ploke_records::ids::ArtifactId;

use super::{ArtifactNode, CandidateBranchNode, Graph, HistoryBlockNode, LineageNode};

/// Borrowed artifact-first projection over a [`Graph`].
///
/// The projection owns only traversal indexes and diagnostics. Artifact facts
/// and relation sources remain borrowed from the source graph.
#[derive(Debug, PartialEq)]
pub struct Tree<'g> {
    pub nodes: BTreeMap<Key<'g>, Node<'g>>,
    pub history_successors: Vec<HistoryEdge<'g>>,
    pub opened_from_edges: Vec<HistoryEdge<'g>>,
    pub produced_child_edges: Vec<ProducedChildEdge<'g>>,
    pub applied_patch_edges: Vec<AppliedPatchEdge<'g>>,
    pub marks: Marks<'g>,
    pub diagnostics: Diagnostics<'g>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key<'g>(&'g str);

impl<'g> Key<'g> {
    pub fn as_str(self) -> &'g str {
        self.0
    }
}

impl Borrow<str> for Key<'_> {
    fn borrow(&self) -> &str {
        self.0
    }
}

#[derive(Debug, PartialEq)]
pub struct Node<'g> {
    pub key: Key<'g>,
    pub sources: Vec<&'g ArtifactNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEdge<'g> {
    pub from: Key<'g>,
    pub to: Key<'g>,
    pub sources: Vec<&'g HistoryBlockNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedPatchEdge<'g> {
    pub from: Key<'g>,
    pub to: Key<'g>,
    pub sources: Vec<&'g CandidateBranchNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducedChildEdge<'g> {
    pub from: Key<'g>,
    pub to: Key<'g>,
    pub sources: Vec<&'g ChildPlanChildRecord>,
}

#[derive(Debug, PartialEq)]
pub struct Marks<'g> {
    pub primary_lineage: Option<&'g LineageNode>,
    pub lineage_artifacts: Vec<Key<'g>>,
    pub lineage_edges: Vec<(Key<'g>, Key<'g>)>,
    pub non_lineage_children: Vec<Key<'g>>,
    pub non_lineage_child_edges: Vec<(Key<'g>, Key<'g>)>,
    pub selected_ruler: Option<Key<'g>>,
}

#[derive(Debug, PartialEq)]
pub struct Diagnostics<'g> {
    pub weak_component_count: usize,
    pub weakly_connected: bool,
    pub components: Vec<Component<'g>>,
    pub roots: Vec<Key<'g>>,
    pub orphan_artifacts: Vec<Key<'g>>,
    pub missing_endpoints: Vec<MissingEndpoint<'g>>,
}

#[derive(Debug, PartialEq)]
pub struct Component<'g> {
    pub artifacts: Vec<Key<'g>>,
    pub roots: Vec<Key<'g>>,
    pub history_successors: Vec<HistoryEdge<'g>>,
    pub opened_from_edges: Vec<HistoryEdge<'g>>,
    pub produced_child_edges: Vec<ProducedChildEdge<'g>>,
    pub applied_patch_edges: Vec<AppliedPatchEdge<'g>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MissingEndpoint<'g> {
    pub relation: Relation<'g>,
    pub endpoint: Endpoint,
    pub artifact_key: Key<'g>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Relation<'g> {
    HistorySuccessor(&'g HistoryBlockNode),
    HistoryOpenedFrom(&'g HistoryBlockNode),
    ProducedChild(&'g ChildPlanChildRecord),
    AppliedPatch(&'g CandidateBranchNode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    From,
    To,
}

impl Graph {
    pub fn artifact_tree(&self) -> Tree<'_> {
        Tree::from_graph(self)
    }
}

impl<'g> Tree<'g> {
    /// archaeology:artifact-promotion-continuity
    /// proof:docs/active/archaeology/ploke-tree-graph/artifact-promotion-continuity.md
    pub fn from_graph(graph: &'g Graph) -> Self {
        let promotion_aliases = promotion_aliases(graph);
        let material_keys = material_keys(graph, &promotion_aliases);
        let mut nodes = BTreeMap::new();
        for artifact in graph.artifacts.artifacts.values() {
            let Some(key) = node_key(artifact, &promotion_aliases, &material_keys) else {
                continue;
            };
            nodes
                .entry(key)
                .or_insert_with(|| Node {
                    key,
                    sources: Vec::new(),
                })
                .sources
                .push(artifact);
        }

        let mut missing_endpoints = Vec::new();
        let mut history_successors =
            BTreeMap::<(Key<'g>, Key<'g>), Vec<&'g HistoryBlockNode>>::new();
        let mut opened_from_edges =
            BTreeMap::<(Key<'g>, Key<'g>), Vec<&'g HistoryBlockNode>>::new();
        let mut produced_child_edges =
            BTreeMap::<(Key<'g>, Key<'g>), Vec<&'g ChildPlanChildRecord>>::new();
        for block in graph.history.blocks.values() {
            let opened_from = canonical_key(
                history_ref_key(&block.opened_from_artifact),
                &promotion_aliases,
            );
            let active = canonical_key(history_ref_key(&block.active_artifact), &promotion_aliases);
            if endpoints_present(
                &nodes,
                Relation::HistoryOpenedFrom(block),
                opened_from,
                active,
                &mut missing_endpoints,
            ) {
                opened_from_edges
                    .entry((opened_from, active))
                    .or_default()
                    .push(block);
            }

            let from = canonical_key(history_ref_key(&block.active_artifact), &promotion_aliases);
            let to = canonical_key(
                history_ref_key(&block.selected_successor.artifact),
                &promotion_aliases,
            );
            if endpoints_present(
                &nodes,
                Relation::HistorySuccessor(block),
                from,
                to,
                &mut missing_endpoints,
            ) {
                history_successors
                    .entry((from, to))
                    .or_default()
                    .push(block);
            }
        }

        for plan in graph.child_plans.plans.values() {
            let Some(parent) = parent_artifact_key(
                graph,
                plan.parent_node_id.as_str(),
                &promotion_aliases,
                &material_keys,
            ) else {
                continue;
            };
            for child in &plan.children {
                let Some(derived) = child_derived_artifact(child) else {
                    continue;
                };
                let to = canonical_key(passive_id_key(derived.0.as_str()), &promotion_aliases);
                if endpoints_present(
                    &nodes,
                    Relation::ProducedChild(child),
                    parent,
                    to,
                    &mut missing_endpoints,
                ) {
                    produced_child_edges
                        .entry((parent, to))
                        .or_default()
                        .push(child);
                }
            }
        }

        let mut applied_patch_edges =
            BTreeMap::<(Key<'g>, Key<'g>), Vec<&'g CandidateBranchNode>>::new();
        for branch in &graph.candidates.branches {
            let (Some(base), Some(derived)) = (
                branch.base_artifact_id.as_ref(),
                branch.derived_artifact_id.as_ref(),
            ) else {
                continue;
            };
            let from = canonical_key(passive_id_key(&base.0), &promotion_aliases);
            let to = canonical_key(passive_id_key(&derived.0), &promotion_aliases);
            if endpoints_present(
                &nodes,
                Relation::AppliedPatch(branch),
                from,
                to,
                &mut missing_endpoints,
            ) {
                applied_patch_edges
                    .entry((from, to))
                    .or_default()
                    .push(branch);
            }
        }

        let history_successors = history_successors
            .into_iter()
            .map(|((from, to), sources)| HistoryEdge { from, to, sources })
            .collect::<Vec<_>>();
        let opened_from_edges = opened_from_edges
            .into_iter()
            .map(|((from, to), sources)| HistoryEdge { from, to, sources })
            .collect::<Vec<_>>();
        let produced_child_edges = produced_child_edges
            .into_iter()
            .map(|((from, to), sources)| ProducedChildEdge { from, to, sources })
            .collect::<Vec<_>>();
        let applied_patch_edges = applied_patch_edges
            .into_iter()
            .map(|((from, to), sources)| AppliedPatchEdge { from, to, sources })
            .collect::<Vec<_>>();

        let marks = Marks::from_graph(graph, &history_successors, &produced_child_edges);
        let diagnostics = Diagnostics::from_edges(
            &nodes,
            &history_successors,
            &opened_from_edges,
            &produced_child_edges,
            &applied_patch_edges,
            missing_endpoints,
        );

        Self {
            nodes,
            history_successors,
            opened_from_edges,
            produced_child_edges,
            applied_patch_edges,
            marks,
            diagnostics,
        }
    }
}

impl<'g> Diagnostics<'g> {
    fn from_edges(
        nodes: &BTreeMap<Key<'g>, Node<'g>>,
        history_successors: &[HistoryEdge<'g>],
        opened_from_edges: &[HistoryEdge<'g>],
        produced_child_edges: &[ProducedChildEdge<'g>],
        applied_patch_edges: &[AppliedPatchEdge<'g>],
        missing_endpoints: Vec<MissingEndpoint<'g>>,
    ) -> Self {
        let keys = nodes.keys().copied().collect::<Vec<_>>();
        let mut index_by_key = BTreeMap::new();
        for (index, key) in keys.iter().copied().enumerate() {
            index_by_key.insert(key, index);
        }

        let mut parent = (0..keys.len()).collect::<Vec<_>>();
        let mut incoming = keys.iter().copied().map(|key| (key, 0usize)).collect();
        let mut incident = keys.iter().copied().map(|key| (key, 0usize)).collect();

        for edge in history_successors {
            join_edge(
                edge.from,
                edge.to,
                &index_by_key,
                &mut parent,
                &mut incoming,
                &mut incident,
            );
        }
        for edge in produced_child_edges {
            join_edge(
                edge.from,
                edge.to,
                &index_by_key,
                &mut parent,
                &mut incoming,
                &mut incident,
            );
        }

        let mut components = BTreeMap::<usize, Vec<Key<'g>>>::new();
        for key in keys.iter().copied() {
            let Some(index) = index_by_key.get(&key).copied() else {
                continue;
            };
            let root = find(&mut parent, index);
            components.entry(root).or_default().push(key);
        }

        let roots = incoming
            .iter()
            .filter_map(|(key, count)| (*count == 0).then_some(*key))
            .collect::<Vec<_>>();
        let orphan_artifacts = incident
            .iter()
            .filter_map(|(key, count)| (*count == 0).then_some(*key))
            .collect::<Vec<_>>();
        let weak_component_count = components.len();
        let components = components
            .into_values()
            .map(|artifacts| {
                let roots = artifacts
                    .iter()
                    .copied()
                    .filter(|key| incoming.get(key).copied().unwrap_or_default() == 0)
                    .collect();
                let history_successors = history_successors
                    .iter()
                    .filter(|edge| artifacts.contains(&edge.from))
                    .cloned()
                    .collect();
                let opened_from_edges = opened_from_edges
                    .iter()
                    .filter(|edge| artifacts.contains(&edge.from))
                    .cloned()
                    .collect();
                let produced_child_edges = produced_child_edges
                    .iter()
                    .filter(|edge| artifacts.contains(&edge.from))
                    .cloned()
                    .collect();
                let applied_patch_edges = applied_patch_edges
                    .iter()
                    .filter(|edge| artifacts.contains(&edge.from))
                    .cloned()
                    .collect();
                Component {
                    artifacts,
                    roots,
                    history_successors,
                    opened_from_edges,
                    produced_child_edges,
                    applied_patch_edges,
                }
            })
            .collect();

        Self {
            weak_component_count,
            weakly_connected: nodes.len() <= 1 || weak_component_count == 1,
            components,
            roots,
            orphan_artifacts,
            missing_endpoints,
        }
    }
}

fn endpoints_present<'g>(
    nodes: &BTreeMap<Key<'g>, Node<'g>>,
    relation: Relation<'g>,
    from: Key<'g>,
    to: Key<'g>,
    missing_endpoints: &mut Vec<MissingEndpoint<'g>>,
) -> bool {
    let mut present = true;
    if !nodes.contains_key(&from) {
        missing_endpoints.push(MissingEndpoint {
            relation,
            endpoint: Endpoint::From,
            artifact_key: from,
        });
        present = false;
    }
    if !nodes.contains_key(&to) {
        missing_endpoints.push(MissingEndpoint {
            relation,
            endpoint: Endpoint::To,
            artifact_key: to,
        });
        present = false;
    }
    present
}

impl<'g> Marks<'g> {
    fn from_graph(
        graph: &'g Graph,
        history_successors: &[HistoryEdge<'g>],
        produced_child_edges: &[ProducedChildEdge<'g>],
    ) -> Self {
        let primary_lineage = primary_lineage(graph);
        let mut lineage_artifacts = BTreeSet::new();
        let mut lineage_edges = BTreeSet::new();
        let mut non_lineage_children = BTreeSet::new();
        let mut non_lineage_child_edges = BTreeSet::new();
        let mut selected_ruler = None;

        if let Some(lineage) = primary_lineage {
            for edge in history_successors.iter().filter(|edge| {
                edge.sources
                    .iter()
                    .any(|source| lineage.blocks.contains(&source.block_hash))
            }) {
                lineage_artifacts.insert(edge.from);
                lineage_artifacts.insert(edge.to);
                lineage_edges.insert((edge.from, edge.to));
            }

            if let Some(block) = lineage
                .blocks
                .iter()
                .filter_map(|block_hash| graph.history.blocks.get(block_hash))
                .max_by_key(|block| block.block_height)
            {
                selected_ruler = history_successors
                    .iter()
                    .find(|edge| {
                        edge.sources
                            .iter()
                            .any(|source| source.block_hash == block.block_hash)
                    })
                    .map(|edge| edge.to);
            }
        }

        for edge in produced_child_edges {
            if !lineage_artifacts.contains(&edge.to) {
                non_lineage_children.insert(edge.to);
                non_lineage_child_edges.insert((edge.from, edge.to));
            }
        }

        Self {
            primary_lineage,
            lineage_artifacts: lineage_artifacts.into_iter().collect(),
            lineage_edges: lineage_edges.into_iter().collect(),
            non_lineage_children: non_lineage_children.into_iter().collect(),
            non_lineage_child_edges: non_lineage_child_edges.into_iter().collect(),
            selected_ruler,
        }
    }
}

fn primary_lineage(graph: &Graph) -> Option<&LineageNode> {
    graph
        .history
        .lineages
        .values()
        .filter(|lineage| {
            lineage
                .blocks
                .iter()
                .any(|block_hash| graph.history.blocks.contains_key(block_hash))
        })
        .max_by_key(|lineage| {
            let mut block_count = 0;
            let mut max_height = 0;
            for block in lineage
                .blocks
                .iter()
                .filter_map(|block_hash| graph.history.blocks.get(block_hash))
            {
                block_count += 1;
                max_height = max_height.max(block.block_height);
            }
            (block_count, max_height)
        })
}

fn node_key<'g>(
    artifact: &'g ArtifactNode,
    promotion_aliases: &BTreeMap<Key<'g>, Key<'g>>,
    material_keys: &BTreeSet<Key<'g>>,
) -> Option<Key<'g>> {
    let key = canonical_key(Key(artifact.entity_key()), promotion_aliases);
    material_keys.contains(&key).then_some(key)
}

fn history_ref_key(artifact: &ArtifactRefRecord) -> Key<'_> {
    Key(artifact.graph_entity_key())
}

fn passive_id_key(value: &str) -> Key<'_> {
    Key(value.strip_prefix("artifact:").unwrap_or(value))
}

fn canonical_key<'g>(mut key: Key<'g>, aliases: &BTreeMap<Key<'g>, Key<'g>>) -> Key<'g> {
    let mut seen = BTreeSet::new();
    while let Some(next) = aliases.get(&key).copied() {
        if next == key || !seen.insert(key) {
            break;
        }
        key = next;
    }
    key
}

/// archaeology:artifact-promotion-continuity
/// proof:docs/active/archaeology/ploke-tree-graph/artifact-promotion-continuity.md
fn material_keys<'g>(graph: &'g Graph, aliases: &BTreeMap<Key<'g>, Key<'g>>) -> BTreeSet<Key<'g>> {
    let mut keys = BTreeSet::new();
    for block in graph.history.blocks.values() {
        keys.insert(canonical_key(
            history_ref_key(&block.opened_from_artifact),
            aliases,
        ));
        keys.insert(canonical_key(
            history_ref_key(&block.active_artifact),
            aliases,
        ));
        keys.insert(canonical_key(
            history_ref_key(&block.selected_successor.artifact),
            aliases,
        ));
    }
    for candidate in &graph.candidates.candidates {
        if let Some(artifact_id) = candidate.artifact_after.as_ref() {
            keys.insert(canonical_key(
                passive_id_key(artifact_id.0.as_str()),
                aliases,
            ));
        }
    }
    for child in graph
        .child_plans
        .plans
        .values()
        .flat_map(|plan| plan.children.iter())
    {
        if let Some(artifact_id) = child_derived_artifact(child) {
            keys.insert(canonical_key(
                passive_id_key(artifact_id.0.as_str()),
                aliases,
            ));
        }
    }
    for plan in graph.child_plans.plans.values() {
        if let Some(key) = only_key(parent_artifact_matches(
            graph,
            plan.parent_node_id.as_str(),
            aliases,
        )) {
            keys.insert(key);
        }
    }
    keys
}

/// archaeology:artifact-promotion-continuity
/// proof:docs/active/archaeology/ploke-tree-graph/artifact-promotion-continuity.md
fn parent_artifact_key<'g>(
    graph: &'g Graph,
    parent_node_id: &str,
    aliases: &BTreeMap<Key<'g>, Key<'g>>,
    material_keys: &BTreeSet<Key<'g>>,
) -> Option<Key<'g>> {
    let key = only_key(parent_artifact_matches(graph, parent_node_id, aliases))?;
    material_keys.contains(&key).then_some(key)
}

fn parent_artifact_matches<'g>(
    graph: &'g Graph,
    parent_node_id: &str,
    aliases: &BTreeMap<Key<'g>, Key<'g>>,
) -> BTreeSet<Key<'g>> {
    let mut matches = BTreeSet::new();
    for candidate in &graph.candidates.candidates {
        if candidate.node_id.as_deref() != Some(parent_node_id) {
            continue;
        }
        if let Some(artifact_id) = candidate.artifact_after.as_ref() {
            matches.insert(canonical_key(
                passive_id_key(artifact_id.0.as_str()),
                aliases,
            ));
        }
    }
    if let Some(child) = graph.child_plans.child_for_node_id(parent_node_id)
        && let Some(artifact_id) = child_derived_artifact(child)
    {
        matches.insert(canonical_key(
            passive_id_key(artifact_id.0.as_str()),
            aliases,
        ));
    }
    matches
}

fn child_derived_artifact(child: &ChildPlanChildRecord) -> Option<&ArtifactId> {
    child
        .surface
        .as_ref()
        .map(|surface| &surface.after.artifact_id)
        .or(child.node.derived_artifact_id.as_ref())
}

fn only_key<'g>(mut keys: BTreeSet<Key<'g>>) -> Option<Key<'g>> {
    if keys.len() == 1 {
        keys.pop_first()
    } else {
        None
    }
}

fn promotion_aliases<'g>(graph: &'g Graph) -> BTreeMap<Key<'g>, Key<'g>> {
    let mut aliases = BTreeMap::new();
    for plan in graph.child_plans.plans.values() {
        let parent_node_id = plan.parent_node_id.as_str();
        let super::ParentCreateLookup::Attempt(attempt) =
            graph.parent_create_for_node_id(parent_node_id)
        else {
            continue;
        };
        let (Some(selected_child), Some(next_parent_base)) = (
            attempt.derived_artifact_id(),
            attempt.next_parent_base_artifact_id(),
        ) else {
            continue;
        };
        let selected_child = passive_id_key(selected_child.0.as_str());
        let next_parent_base = passive_id_key(next_parent_base.0.as_str());
        if selected_child != next_parent_base {
            aliases.insert(next_parent_base, selected_child);
        }
    }
    aliases
}

fn join_edge<'g>(
    from: Key<'g>,
    to: Key<'g>,
    index_by_key: &BTreeMap<Key<'g>, usize>,
    parent: &mut [usize],
    incoming: &mut BTreeMap<Key<'g>, usize>,
    incident: &mut BTreeMap<Key<'g>, usize>,
) {
    if from == to {
        return;
    }

    if let Some(count) = incoming.get_mut(&to) {
        *count += 1;
    }
    if let Some(count) = incident.get_mut(&from) {
        *count += 1;
    }
    if let Some(count) = incident.get_mut(&to) {
        *count += 1;
    }

    let (Some(left), Some(right)) = (
        index_by_key.get(&from).copied(),
        index_by_key.get(&to).copied(),
    ) else {
        return;
    };
    union(parent, left, right);
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left_root = find(parent, left);
    let right_root = find(parent, right);
    if left_root != right_root {
        parent[right_root] = left_root;
    }
}

fn find(parent: &mut [usize], index: usize) -> usize {
    let mut root = index;
    while parent[root] != root {
        root = parent[root];
    }

    let mut current = index;
    while parent[current] != current {
        let next = parent[current];
        parent[current] = root;
        current = next;
    }

    root
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use ploke_records::branch::{
        ResolvedTreatmentBranch, TreatmentBranchNode, TreatmentBranchStatus,
    };
    use ploke_records::child_plan::{ChildPlanChildRecord, ChildPlanRecord};
    use ploke_records::history::{
        ActorRefRecord, ArtifactRefRecord, ProcedureRefRecord, SurfaceCommitmentRecord,
        SurfaceDeltaRecord, SurfaceRecord, SurfaceRootRecord,
    };
    use ploke_records::ids::{
        ArtifactId, BlockHash, BlockId, BranchId, CampaignId, CandidateId, EntryId, HistoryHash,
        InstanceId, LineageId, PatchId, RecordedAt, RuntimeId, SchedulerNodeId, SourceStateId,
    };
    use ploke_records::scheduler::{NodeRecord, NodeStatusRecord, RunnerRequestRecord};

    use crate::graph::{
        ArtifactIdentity, ArtifactIds, ArtifactIndex, ArtifactKey, ArtifactNode,
        CandidateBranchNode, CandidateIndex, ChildPlanIndex, Graph, HistoryBlockNode, HistoryIndex,
        LineageNode, OpeningAuthorityNode, SuccessorNode,
    };

    #[test]
    fn artifact_tree_coalesces_history_refs_and_passive_ids() {
        let mut graph = Graph {
            artifacts: ArtifactIndex {
                artifacts: BTreeMap::from([
                    artifact_history("artifact:base"),
                    artifact_passive("base"),
                    artifact_history("artifact:successor"),
                    artifact_passive("derived"),
                ]),
            },
            history: HistoryIndex {
                blocks: BTreeMap::from([(
                    BlockHash("block-hash".to_owned()),
                    history_block("artifact:base", "artifact:successor"),
                )]),
                ..HistoryIndex::default()
            },
            candidates: CandidateIndex {
                candidates: vec![crate::graph::CandidateNode {
                    selection_entry_id: EntryId("entry-derived".to_owned()),
                    payload_index: 0,
                    subject: ploke_records::history::SubjectRefRecord {
                        value: "candidate:derived".to_owned(),
                    },
                    source: Some(crate::graph::CandidateSource::CurrentGeneration),
                    occurrence_id: None,
                    membership_id: None,
                    membership_key: None,
                    node_id: Some("node-derived".to_owned()),
                    branch_id: Some("branch-derived".to_owned()),
                    generation: Some(1),
                    primary_runtime_id: None,
                    artifact_after: Some(ArtifactId("derived".to_owned())),
                    patch_id: None,
                    evidence: Vec::new(),
                }],
                branches: vec![candidate_branch("successor", "derived")],
                ..CandidateIndex::default()
            },
            ..Graph::default()
        };

        let tree = graph.artifact_tree();

        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.nodes["base"].sources.len(), 2);
        assert_eq!(tree.history_successors.len(), 1);
        assert_eq!(tree.history_successors[0].from.as_str(), "base");
        assert_eq!(tree.history_successors[0].to.as_str(), "successor");
        assert_eq!(tree.opened_from_edges.len(), 1);
        assert_eq!(tree.opened_from_edges[0].from.as_str(), "base");
        assert_eq!(tree.opened_from_edges[0].to.as_str(), "base");
        assert_eq!(tree.applied_patch_edges.len(), 1);
        assert_eq!(tree.applied_patch_edges[0].from.as_str(), "successor");
        assert_eq!(tree.applied_patch_edges[0].to.as_str(), "derived");
        assert_eq!(tree.marks.selected_ruler.map(|key| key.as_str()), None);
        assert!(!tree.diagnostics.weakly_connected);
        assert_eq!(tree.diagnostics.weak_component_count, 2);
        assert_eq!(
            key_strings(&tree.diagnostics.roots),
            vec!["base", "derived"]
        );
        assert_eq!(
            key_strings(&tree.diagnostics.orphan_artifacts),
            vec!["derived"]
        );
        assert!(tree.diagnostics.missing_endpoints.is_empty());

        graph.artifacts.artifacts.insert(
            ArtifactKey::PassiveId {
                value: "orphan".to_owned(),
            },
            ArtifactNode {
                key: ArtifactKey::PassiveId {
                    value: "orphan".to_owned(),
                },
                identity: ArtifactIdentity::PassiveId(ArtifactId("orphan".to_owned())),
                ids: ArtifactIds {
                    artifact_ids: vec![ArtifactId("orphan".to_owned())],
                    ..ArtifactIds::default()
                },
                evidence: Vec::new(),
            },
        );
        let tree = graph.artifact_tree();
        assert!(!tree.diagnostics.weakly_connected);
        assert_eq!(tree.diagnostics.weak_component_count, 2);
        assert_eq!(
            key_strings(&tree.diagnostics.orphan_artifacts),
            vec!["derived"]
        );
    }

    #[test]
    fn artifact_tree_marks_primary_lineage_and_selected_ruler() {
        let lineage = LineageId("lineage:primary".to_owned());
        let side_lineage = LineageId("lineage:side".to_owned());
        let block_one = BlockHash("block:one".to_owned());
        let block_two = BlockHash("block:two".to_owned());
        let side_block = BlockHash("block:side".to_owned());
        let graph = Graph {
            artifacts: ArtifactIndex {
                artifacts: BTreeMap::from([
                    artifact_history("artifact:base"),
                    artifact_history("artifact:middle"),
                    artifact_history("artifact:leaf"),
                    artifact_history("artifact:side"),
                ]),
            },
            history: HistoryIndex {
                lineages: BTreeMap::from([
                    (
                        lineage.clone(),
                        LineageNode {
                            lineage_id: lineage.clone(),
                            blocks: vec![block_one.clone(), block_two.clone()],
                        },
                    ),
                    (
                        side_lineage.clone(),
                        LineageNode {
                            lineage_id: side_lineage.clone(),
                            blocks: vec![side_block.clone()],
                        },
                    ),
                ]),
                blocks: BTreeMap::from([
                    (
                        block_one.clone(),
                        history_block_in(
                            block_one,
                            lineage.clone(),
                            0,
                            "artifact:base",
                            "artifact:middle",
                        ),
                    ),
                    (
                        block_two.clone(),
                        history_block_in(block_two, lineage, 1, "artifact:middle", "artifact:leaf"),
                    ),
                    (
                        side_block.clone(),
                        history_block_in(
                            side_block,
                            side_lineage,
                            9,
                            "artifact:side",
                            "artifact:side",
                        ),
                    ),
                ]),
                ..HistoryIndex::default()
            },
            ..Graph::default()
        };

        let tree = graph.artifact_tree();

        assert_eq!(
            tree.marks
                .primary_lineage
                .map(|lineage| lineage.lineage_id.0.as_str()),
            Some("lineage:primary")
        );
        assert_eq!(
            key_strings(&tree.marks.lineage_artifacts),
            vec!["base", "leaf", "middle"]
        );
        assert_eq!(
            tree.marks
                .lineage_edges
                .iter()
                .map(|(from, to)| (from.as_str(), to.as_str()))
                .collect::<Vec<_>>(),
            vec![("base", "middle"), ("middle", "leaf")]
        );
        assert_eq!(
            tree.marks.selected_ruler.map(|key| key.as_str()),
            Some("leaf")
        );
    }

    #[test]
    fn artifact_tree_reports_missing_history_endpoints() {
        let graph = Graph {
            history: HistoryIndex {
                blocks: BTreeMap::from([(
                    BlockHash("block-hash".to_owned()),
                    history_block("artifact:base", "artifact:successor"),
                )]),
                ..HistoryIndex::default()
            },
            ..Graph::default()
        };

        let tree = graph.artifact_tree();

        assert!(tree.history_successors.is_empty());
        assert_eq!(tree.diagnostics.missing_endpoints.len(), 4);
        assert_eq!(
            tree.diagnostics
                .missing_endpoints
                .iter()
                .map(|endpoint| endpoint.artifact_key.as_str())
                .collect::<Vec<_>>(),
            vec!["base", "base", "base", "successor"]
        );
    }

    #[test]
    fn artifact_tree_collapses_selected_child_and_next_parent_base_artifact() {
        let selected = "selected";
        let next_parent_base = "next-parent-base";
        let grandchild = "grandchild";
        let child_node_id = "node-selected-child";

        let graph = Graph {
            artifacts: ArtifactIndex {
                artifacts: BTreeMap::from([
                    artifact_passive(selected),
                    artifact_passive(next_parent_base),
                    artifact_passive(grandchild),
                ]),
            },
            candidates: CandidateIndex {
                branches: vec![candidate_branch(next_parent_base, grandchild)],
                ..CandidateIndex::default()
            },
            child_plans: ChildPlanIndex {
                plans: BTreeMap::from([
                    (
                        SchedulerNodeId("node-parent-g1".to_owned()),
                        child_plan_record(
                            "node-parent-g1",
                            child_node_id,
                            "parent-base",
                            selected,
                            "branch-selected",
                            "candidate-selected",
                            "patch-selected",
                        ),
                    ),
                    (
                        SchedulerNodeId(child_node_id.to_owned()),
                        child_plan_record(
                            child_node_id,
                            "node-child-g2",
                            next_parent_base,
                            grandchild,
                            "branch-grandchild",
                            "candidate-grandchild",
                            "patch-grandchild",
                        ),
                    ),
                ]),
            },
            ..Graph::default()
        };

        let tree = graph.artifact_tree();

        assert_eq!(tree.nodes.len(), 2);
        assert_eq!(tree.nodes[selected].sources.len(), 2);
        assert_eq!(tree.applied_patch_edges.len(), 1);
        assert_eq!(tree.applied_patch_edges[0].from.as_str(), selected);
        assert_eq!(tree.applied_patch_edges[0].to.as_str(), grandchild);
        assert!(tree.diagnostics.weakly_connected);
    }

    #[test]
    fn artifact_tree_materializes_parent_for_non_lineage_siblings() {
        let graph = Graph {
            artifacts: ArtifactIndex {
                artifacts: BTreeMap::from([
                    artifact_passive("parent"),
                    artifact_passive("selected"),
                    artifact_passive("sibling"),
                ]),
            },
            history: HistoryIndex {
                lineages: BTreeMap::from([(
                    LineageId("lineage:primary".to_owned()),
                    LineageNode {
                        lineage_id: LineageId("lineage:primary".to_owned()),
                        blocks: vec![BlockHash("block:selected".to_owned())],
                    },
                )]),
                blocks: BTreeMap::from([(
                    BlockHash("block:selected".to_owned()),
                    history_block_in(
                        BlockHash("block:selected".to_owned()),
                        LineageId("lineage:primary".to_owned()),
                        1,
                        "artifact:selected",
                        "artifact:selected",
                    ),
                )]),
                ..HistoryIndex::default()
            },
            candidates: CandidateIndex {
                candidates: vec![crate::graph::CandidateNode {
                    selection_entry_id: EntryId("entry-parent".to_owned()),
                    payload_index: 0,
                    subject: ploke_records::history::SubjectRefRecord {
                        value: "candidate:parent".to_owned(),
                    },
                    source: Some(crate::graph::CandidateSource::CurrentGeneration),
                    occurrence_id: None,
                    membership_id: None,
                    membership_key: None,
                    node_id: Some("node-parent".to_owned()),
                    branch_id: Some("branch-parent".to_owned()),
                    generation: Some(0),
                    primary_runtime_id: None,
                    artifact_after: Some(ArtifactId("parent".to_owned())),
                    patch_id: None,
                    evidence: Vec::new(),
                }],
                ..CandidateIndex::default()
            },
            child_plans: ChildPlanIndex {
                plans: BTreeMap::from([(
                    SchedulerNodeId("node-parent".to_owned()),
                    ChildPlanRecord {
                        children: vec![
                            child_plan_record(
                                "node-parent",
                                "node-selected",
                                "parent",
                                "selected",
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
                                "parent",
                                "sibling",
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
                            "parent",
                            "selected",
                            "branch-selected",
                            "candidate-selected",
                            "patch-selected",
                        )
                    },
                )]),
            },
            ..Graph::default()
        };

        let tree = graph.artifact_tree();

        assert_eq!(
            key_strings(&tree.nodes.keys().copied().collect::<Vec<_>>()),
            vec!["parent", "selected", "sibling"]
        );
        assert_eq!(tree.produced_child_edges.len(), 2);
        assert_eq!(
            key_strings(&tree.marks.non_lineage_children),
            vec!["sibling"]
        );
        assert_eq!(
            tree.marks
                .non_lineage_child_edges
                .iter()
                .map(|(from, to)| (from.as_str(), to.as_str()))
                .collect::<Vec<_>>(),
            vec![("parent", "sibling")]
        );
    }

    fn key_strings<'a>(keys: &[super::Key<'a>]) -> Vec<&'a str> {
        keys.iter().map(|key| key.as_str()).collect()
    }

    fn artifact_history(value: &str) -> (ArtifactKey, ArtifactNode) {
        let artifact =
            ArtifactRefRecord::from_artifact_id(ploke_records::ids::ArtifactId(value.to_owned()));
        let key = ArtifactKey::HistoryRef {
            id: artifact.id().0.clone(),
        };
        (
            key.clone(),
            ArtifactNode {
                key,
                identity: ArtifactIdentity::HistoryRef(artifact),
                ids: ArtifactIds {
                    artifact_refs: vec![ArtifactRefRecord::from_artifact_id(
                        ploke_records::ids::ArtifactId(value.to_owned()),
                    )],
                    ..ArtifactIds::default()
                },
                evidence: Vec::new(),
            },
        )
    }

    fn artifact_passive(value: &str) -> (ArtifactKey, ArtifactNode) {
        let artifact = ArtifactId(value.to_owned());
        let key = ArtifactKey::PassiveId {
            value: artifact.0.clone(),
        };
        (
            key.clone(),
            ArtifactNode {
                key,
                identity: ArtifactIdentity::PassiveId(artifact),
                ids: ArtifactIds {
                    artifact_ids: vec![ArtifactId(value.to_owned())],
                    ..ArtifactIds::default()
                },
                evidence: Vec::new(),
            },
        )
    }

    fn history_block(active: &str, successor: &str) -> HistoryBlockNode {
        history_block_in(
            BlockHash("block-hash".to_owned()),
            LineageId("lineage".to_owned()),
            0,
            active,
            successor,
        )
    }

    fn history_block_in(
        block_hash: BlockHash,
        lineage_id: LineageId,
        block_height: u64,
        active: &str,
        successor: &str,
    ) -> HistoryBlockNode {
        HistoryBlockNode {
            block_hash,
            block_id: BlockId("block-id".to_owned()),
            lineage_id,
            block_height,
            parent_block_hashes: Vec::new(),
            opened_from_artifact: artifact_ref(active),
            active_artifact: artifact_ref(active),
            selected_successor: SuccessorNode {
                runtime: ActorRefRecord::Runtime(RuntimeId("runtime".to_owned())),
                artifact: artifact_ref(successor),
            },
            opening_authority: OpeningAuthorityNode::Genesis {
                bootstrap_policy: ProcedureRefRecord {
                    value: "policy".to_owned(),
                },
                tree_key_hash: "tree".to_owned(),
                parent_identity_ref: ploke_records::history::EvidenceRefRecord {
                    value: "parent-identity".to_owned(),
                },
            },
            ruling_authority: ActorRefRecord::Runtime(RuntimeId("runtime".to_owned())),
            policy_ref: ProcedureRefRecord {
                value: "policy".to_owned(),
            },
            surface: surface(),
            opened_at: RecordedAt(1),
            sealed_at: RecordedAt(2),
            entry_count: 0,
            entries: Vec::<EntryId>::new(),
        }
    }

    fn candidate_branch(base: &str, derived: &str) -> CandidateBranchNode {
        CandidateBranchNode {
            selection_entry_id: EntryId("entry".to_owned()),
            payload_index: 0,
            branch_id: "branch".to_owned(),
            candidate_id: None,
            source_state_id: None,
            parent_branch_id: None,
            base_artifact_id: Some(ArtifactId(base.to_owned())),
            derived_artifact_id: Some(ArtifactId(derived.to_owned())),
            patch_id: None,
            evidence: Vec::new(),
        }
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
                node: node_record(
                    child_node_id,
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

    fn node_record(
        child_node_id: &str,
        base_artifact_id: &str,
        derived_artifact_id: &str,
        branch_id: &str,
        candidate_id: &str,
        patch_id: &str,
    ) -> NodeRecord {
        NodeRecord {
            schema_version: "prototype1-treatment-node.v1".to_owned(),
            node_id: SchedulerNodeId(child_node_id.to_owned()),
            parent_node_id: None,
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
            runner_request_path: PathBuf::from(format!("/tmp/requests/{child_node_id}.json")),
            runner_result_path: PathBuf::from(format!("/tmp/results/{child_node_id}.json")),
            status: NodeStatusRecord::Planned,
            created_at: "now".to_owned(),
            updated_at: "now".to_owned(),
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

    fn artifact_ref(value: &str) -> ArtifactRefRecord {
        ArtifactRefRecord::from_artifact_id(ploke_records::ids::ArtifactId(value.to_owned()))
    }

    fn surface() -> SurfaceCommitmentRecord {
        SurfaceCommitmentRecord {
            immutable: surface_record("immutable"),
            mutated: SurfaceDeltaRecord {
                before: surface_record("mutated-before"),
                after: surface_record("mutated-after"),
            },
            ambient: SurfaceDeltaRecord {
                before: surface_record("ambient-before"),
                after: surface_record("ambient-after"),
            },
        }
    }

    fn surface_record(value: &str) -> SurfaceRecord {
        SurfaceRecord {
            root: SurfaceRootRecord {
                hash: HistoryHash(value.to_owned()),
            },
        }
    }
}
