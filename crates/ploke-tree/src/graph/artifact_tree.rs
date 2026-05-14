use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet},
};

use super::{
    ArtifactIdentity, ArtifactNode, CandidateBranchNode, Graph, HistoryBlockNode, LineageNode,
};

/// Borrowed artifact-first projection over a [`Graph`].
///
/// The projection owns only traversal indexes and diagnostics. Artifact facts
/// and relation sources remain borrowed from the source graph.
#[derive(Debug, PartialEq)]
pub struct Tree<'g> {
    pub nodes: BTreeMap<Key<'g>, Node<'g>>,
    pub history_successors: Vec<HistoryEdge<'g>>,
    pub candidate_derivations: Vec<CandidateEdge<'g>>,
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

#[derive(Debug, PartialEq)]
pub struct HistoryEdge<'g> {
    pub from: Key<'g>,
    pub to: Key<'g>,
    pub sources: Vec<&'g HistoryBlockNode>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CandidateEdge<'g> {
    pub from: Key<'g>,
    pub to: Key<'g>,
    pub sources: Vec<&'g CandidateBranchNode>,
}

#[derive(Debug, PartialEq)]
pub struct Marks<'g> {
    pub primary_lineage: Option<&'g LineageNode>,
    pub lineage_artifacts: Vec<Key<'g>>,
    pub lineage_edges: Vec<(Key<'g>, Key<'g>)>,
    pub selected_ruler: Option<Key<'g>>,
}

#[derive(Debug, PartialEq)]
pub struct Diagnostics<'g> {
    pub weak_component_count: usize,
    pub weakly_connected: bool,
    pub roots: Vec<Key<'g>>,
    pub orphan_artifacts: Vec<Key<'g>>,
    pub missing_endpoints: Vec<MissingEndpoint<'g>>,
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
    CandidateDerivation(&'g CandidateBranchNode),
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
    pub fn from_graph(graph: &'g Graph) -> Self {
        let mut nodes = BTreeMap::new();
        for artifact in graph.artifacts.artifacts.values() {
            let Some(key) = node_key(artifact) else {
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
        for block in graph.history.blocks.values() {
            let from = history_ref_key(&block.active_artifact.value);
            let to = history_ref_key(&block.selected_successor.artifact.value);
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

        let mut candidate_derivations =
            BTreeMap::<(Key<'g>, Key<'g>), Vec<&'g CandidateBranchNode>>::new();
        for branch in &graph.candidates.branches {
            let (Some(base), Some(derived)) = (
                branch.base_artifact_id.as_ref(),
                branch.derived_artifact_id.as_ref(),
            ) else {
                continue;
            };
            let from = passive_id_key(&base.0);
            let to = passive_id_key(&derived.0);
            if endpoints_present(
                &nodes,
                Relation::CandidateDerivation(branch),
                from,
                to,
                &mut missing_endpoints,
            ) {
                candidate_derivations
                    .entry((from, to))
                    .or_default()
                    .push(branch);
            }
        }

        let history_successors = history_successors
            .into_iter()
            .map(|((from, to), sources)| HistoryEdge { from, to, sources })
            .collect::<Vec<_>>();
        let candidate_derivations = candidate_derivations
            .into_iter()
            .map(|((from, to), sources)| CandidateEdge { from, to, sources })
            .collect::<Vec<_>>();

        let marks = Marks::from_graph(graph, &history_successors);
        let diagnostics = Diagnostics::from_edges(
            &nodes,
            &history_successors,
            &candidate_derivations,
            missing_endpoints,
        );

        Self {
            nodes,
            history_successors,
            candidate_derivations,
            marks,
            diagnostics,
        }
    }
}

impl<'g> Diagnostics<'g> {
    fn from_edges(
        nodes: &BTreeMap<Key<'g>, Node<'g>>,
        history_successors: &[HistoryEdge<'g>],
        candidate_derivations: &[CandidateEdge<'g>],
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
        for edge in candidate_derivations {
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

        Self {
            weak_component_count,
            weakly_connected: nodes.len() <= 1 || weak_component_count == 1,
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
    fn from_graph(graph: &'g Graph, history_successors: &[HistoryEdge<'g>]) -> Self {
        let primary_lineage = primary_lineage(graph);
        let mut lineage_artifacts = BTreeSet::new();
        let mut lineage_edges = BTreeSet::new();
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

        Self {
            primary_lineage,
            lineage_artifacts: lineage_artifacts.into_iter().collect(),
            lineage_edges: lineage_edges.into_iter().collect(),
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

fn node_key(artifact: &ArtifactNode) -> Option<Key<'_>> {
    match &artifact.identity {
        ArtifactIdentity::HistoryRef(artifact) => Some(history_ref_key(&artifact.value)),
        ArtifactIdentity::PassiveId(artifact) => Some(passive_id_key(&artifact.0)),
    }
}

fn history_ref_key(value: &str) -> Key<'_> {
    Key(value.strip_prefix("artifact:").unwrap_or(value))
}

fn passive_id_key(value: &str) -> Key<'_> {
    Key(value.strip_prefix("artifact:").unwrap_or(value))
}

fn join_edge<'g>(
    from: Key<'g>,
    to: Key<'g>,
    index_by_key: &BTreeMap<Key<'g>, usize>,
    parent: &mut [usize],
    incoming: &mut BTreeMap<Key<'g>, usize>,
    incident: &mut BTreeMap<Key<'g>, usize>,
) {
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

    use ploke_records::history::{
        ActorRefRecord, ArtifactRefRecord, ProcedureRefRecord, SurfaceCommitmentRecord,
        SurfaceDeltaRecord, SurfaceRecord, SurfaceRootRecord,
    };
    use ploke_records::ids::{
        ArtifactId, BlockHash, BlockId, EntryId, HistoryHash, LineageId, RecordedAt, RuntimeId,
    };

    use crate::graph::{
        ArtifactIdentity, ArtifactIndex, ArtifactKey, ArtifactNode, CandidateBranchNode,
        CandidateIndex, Graph, HistoryBlockNode, HistoryIndex, LineageNode, OpeningAuthorityNode,
        SuccessorNode,
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
        assert_eq!(tree.candidate_derivations.len(), 1);
        assert_eq!(tree.candidate_derivations[0].from.as_str(), "successor");
        assert_eq!(tree.candidate_derivations[0].to.as_str(), "derived");
        assert_eq!(tree.marks.selected_ruler.map(|key| key.as_str()), None);
        assert!(tree.diagnostics.weakly_connected);
        assert_eq!(tree.diagnostics.weak_component_count, 1);
        assert_eq!(key_strings(&tree.diagnostics.roots), vec!["base"]);
        assert!(tree.diagnostics.orphan_artifacts.is_empty());
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
                evidence: Vec::new(),
            },
        );
        let tree = graph.artifact_tree();
        assert!(!tree.diagnostics.weakly_connected);
        assert_eq!(tree.diagnostics.weak_component_count, 2);
        assert_eq!(
            key_strings(&tree.diagnostics.orphan_artifacts),
            vec!["orphan"]
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
        assert_eq!(tree.diagnostics.missing_endpoints.len(), 2);
        assert_eq!(
            tree.diagnostics
                .missing_endpoints
                .iter()
                .map(|endpoint| endpoint.artifact_key.as_str())
                .collect::<Vec<_>>(),
            vec!["base", "successor"]
        );
    }

    fn key_strings<'a>(keys: &[super::Key<'a>]) -> Vec<&'a str> {
        keys.iter().map(|key| key.as_str()).collect()
    }

    fn artifact_history(value: &str) -> (ArtifactKey, ArtifactNode) {
        let artifact = ArtifactRefRecord {
            value: value.to_owned(),
        };
        let key = ArtifactKey::HistoryRef {
            value: artifact.value.clone(),
        };
        (
            key.clone(),
            ArtifactNode {
                key,
                identity: ArtifactIdentity::HistoryRef(artifact),
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

    fn artifact_ref(value: &str) -> ArtifactRefRecord {
        ArtifactRefRecord {
            value: value.to_owned(),
        }
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
