//! Graph-resolved selection inspector projections.

use serde::{Deserialize, Serialize};

use crate::ui::view::{GraphSelectionDetail, GraphSelectionRef};

#[derive(Debug, Clone, PartialEq, Eq)]
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
        match &selection.reference {
            GraphSelectionRef::RunForestNode { key } => run_forest_node_inspection(graph, key),
            GraphSelectionRef::Artifact { key } => artifact_inspection(graph, key),
        }
    }

    pub fn snapshot(&self, selection: &GraphSelectionDetail) -> SelectionInspectorSnapshot {
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
            .map(|selection| {
                let inspector = Self::from_graph(graph, &selection);
                (selection, inspector)
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunForestNodeInspection<'g> {
    pub node: &'g ploke_tree::TreeNode,
    pub parent: Option<&'g ploke_tree::TreeNode>,
    pub children: &'g [ploke_tree::NodeKey],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactInspection<'g> {
    pub key: &'g str,
    pub sources: Vec<&'g ploke_tree::graph::ArtifactNode>,
    pub incoming: Vec<ArtifactRelation<'g>>,
    pub outgoing: Vec<ArtifactRelation<'g>>,
    pub selected_ruler: bool,
    pub primary_lineage: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactRelation<'g> {
    pub kind: ArtifactRelationKind,
    pub from: &'g str,
    pub to: &'g str,
    pub source_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactRelationKind {
    HistoryPatch,
    AppliedPatch,
}

impl ArtifactRelationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::HistoryPatch => "P_H",
            Self::AppliedPatch => "P_B",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnavailableReason {
    RunForestNotPresent,
    RunForestNodeNotFound,
    ArtifactNotFound,
}

impl UnavailableReason {
    fn row(self) -> InspectorRow {
        match self {
            Self::RunForestNotPresent => InspectorRow::new("run forest", "not_present"),
            Self::RunForestNodeNotFound => InspectorRow::new("run forest node", "not_found"),
            Self::ArtifactNotFound => InspectorRow::new("artifact", "not_found"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionInspectorSnapshot {
    pub kind: String,
    pub label: String,
    pub identity: Vec<InspectorRow>,
    pub roles: Vec<InspectorRow>,
    pub incoming: Vec<InspectorEdge>,
    pub outgoing: Vec<InspectorEdge>,
    pub source_refs: Vec<String>,
    pub unavailable: Vec<InspectorRow>,
}

impl SelectionInspectorSnapshot {
    fn from_inspection(
        selection: &GraphSelectionDetail,
        inspector: &SelectionInspector<'_>,
    ) -> Self {
        match inspector {
            SelectionInspector::RunForestNode(run) => snapshot_run_forest(selection, run),
            SelectionInspector::Artifact(artifact) => snapshot_artifact(selection, artifact),
            SelectionInspector::Unresolved(reason) => Self {
                kind: selection.kind.clone(),
                label: selection.label.clone(),
                identity: Vec::new(),
                roles: Vec::new(),
                incoming: Vec::new(),
                outgoing: Vec::new(),
                source_refs: Vec::new(),
                unavailable: vec![reason.row()],
            },
        }
    }

    pub(crate) fn has_record_refs(&self) -> bool {
        !self.source_refs.is_empty()
    }

    pub(crate) fn has_edges(&self) -> bool {
        !self.incoming.is_empty() || !self.outgoing.is_empty()
    }

    pub fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("selection: {} {}\n", self.kind, self.label));
        render_rows(&mut out, "identity", &self.identity);
        render_rows(&mut out, "roles", &self.roles);
        render_edges(&mut out, "incoming", &self.incoming);
        render_edges(&mut out, "outgoing", &self.outgoing);
        if self.source_refs.is_empty() {
            out.push_str("source_refs: none\n");
        } else {
            out.push_str("source_refs:\n");
            for source_ref in &self.source_refs {
                out.push_str(&format!("- {source_ref}\n"));
            }
        }
        render_rows(&mut out, "unavailable", &self.unavailable);
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectorRow {
    pub label: String,
    pub value: String,
}

impl InspectorRow {
    fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectorEdge {
    pub relation: String,
    pub from: String,
    pub to: String,
    pub source_count: usize,
}

impl InspectorEdge {
    fn new(
        relation: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
        source_count: usize,
    ) -> Self {
        Self {
            relation: relation.into(),
            from: from.into(),
            to: to.into(),
            source_count,
        }
    }
}

pub fn default_selections(graph: &ploke_tree::Graph) -> Vec<GraphSelectionDetail> {
    if let Some(forest) = graph
        .forest
        .as_ref()
        .filter(|forest| !forest.nodes.is_empty())
    {
        let mut nodes = forest.nodes.iter().collect::<Vec<_>>();
        nodes.sort_by(|left, right| {
            (left.generation, left.key.as_str()).cmp(&(right.generation, right.key.as_str()))
        });
        return nodes
            .into_iter()
            .enumerate()
            .map(|(index, node)| GraphSelectionDetail {
                kind: "artifact".to_owned(),
                label: artifact_handle_label(index + 1),
                detail: String::new(),
                reference: GraphSelectionRef::RunForestNode {
                    key: node.key.as_str().to_owned(),
                },
            })
            .collect();
    }

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
    })
}

fn artifact_inspection<'g>(graph: &'g ploke_tree::Graph, key: &str) -> SelectionInspector<'g> {
    let tree = graph.artifact_tree();
    let Some(node) = tree.nodes.values().find(|node| node.key.as_str() == key) else {
        return SelectionInspector::Unresolved(UnavailableReason::ArtifactNotFound);
    };

    let incoming = tree
        .history_successors
        .iter()
        .filter(|edge| edge.to == node.key)
        .map(|edge| ArtifactRelation {
            kind: ArtifactRelationKind::HistoryPatch,
            from: edge.from.as_str(),
            to: edge.to.as_str(),
            source_count: edge.sources.len(),
        })
        .chain(
            tree.applied_patch_edges
                .iter()
                .filter(|edge| edge.to == node.key)
                .map(|edge| ArtifactRelation {
                    kind: ArtifactRelationKind::AppliedPatch,
                    from: edge.from.as_str(),
                    to: edge.to.as_str(),
                    source_count: edge.sources.len(),
                }),
        )
        .collect();
    let outgoing = tree
        .history_successors
        .iter()
        .filter(|edge| edge.from == node.key)
        .map(|edge| ArtifactRelation {
            kind: ArtifactRelationKind::HistoryPatch,
            from: edge.from.as_str(),
            to: edge.to.as_str(),
            source_count: edge.sources.len(),
        })
        .chain(
            tree.applied_patch_edges
                .iter()
                .filter(|edge| edge.from == node.key)
                .map(|edge| ArtifactRelation {
                    kind: ArtifactRelationKind::AppliedPatch,
                    from: edge.from.as_str(),
                    to: edge.to.as_str(),
                    source_count: edge.sources.len(),
                }),
        )
        .collect();

    SelectionInspector::Artifact(ArtifactInspection {
        key: node.key.as_str(),
        sources: node.sources.clone(),
        incoming,
        outgoing,
        selected_ruler: tree.marks.selected_ruler == Some(node.key),
        primary_lineage: tree.marks.lineage_artifacts.contains(&node.key),
    })
}

fn snapshot_run_forest(
    selection: &GraphSelectionDetail,
    inspector: &RunForestNodeInspection<'_>,
) -> SelectionInspectorSnapshot {
    let node = inspector.node;
    let mut identity = vec![
        InspectorRow::new("node", node.key.as_str()),
        InspectorRow::new("candidate", node.candidate_id.as_str()),
        InspectorRow::new("source artifact", node.source_state_id.as_str()),
    ];
    maybe_row(
        &mut identity,
        "base artifact",
        node.base_artifact_id.as_deref(),
    );
    maybe_row(
        &mut identity,
        "derived artifact",
        node.derived_artifact_id.as_deref(),
    );
    maybe_row(&mut identity, "patch", node.patch_id.as_deref());

    let roles = vec![
        InspectorRow::new("generation", node.generation.to_string()),
        InspectorRow::new("branch", node.branch_id.as_str()),
        InspectorRow::new("target", node.target_relpath.as_str()),
        InspectorRow::new("phase", format!("{:?}", node.progress.phase)),
        InspectorRow::new("result", format!("{:?}", node.progress.result_class)),
    ];
    let incoming = inspector
        .parent
        .map(|parent| InspectorEdge::new("E_F", parent.key.as_str(), node.key.as_str(), 1))
        .into_iter()
        .collect();
    let outgoing = inspector
        .children
        .iter()
        .map(|child| InspectorEdge::new("E_F", node.key.as_str(), child.as_str(), 1))
        .collect();
    let source_refs =
        node.evidence
            .iter()
            .map(|evidence| {
                format!(
                    "{:?}:{:?}{}",
                    evidence.kind,
                    evidence.authority,
                    evidence
                        .recorded_at
                        .as_ref()
                        .map(|recorded_at| format!(" @{recorded_at}"))
                        .unwrap_or_default()
                )
            })
            .chain(node.diagnostics.iter().map(|diagnostic| {
                format!("diagnostic:{:?}:{}", diagnostic.severity, diagnostic.code)
            }))
            .collect();

    SelectionInspectorSnapshot {
        kind: selection.kind.clone(),
        label: selection.label.clone(),
        identity,
        roles,
        incoming,
        outgoing,
        source_refs,
        unavailable: Vec::new(),
    }
}

fn snapshot_artifact(
    selection: &GraphSelectionDetail,
    inspector: &ArtifactInspection<'_>,
) -> SelectionInspectorSnapshot {
    let mut roles = vec![InspectorRow::new(
        "source records",
        inspector.sources.len().to_string(),
    )];
    if inspector.selected_ruler {
        roles.push(InspectorRow::new("selected ruler", "true"));
    }
    if inspector.primary_lineage {
        roles.push(InspectorRow::new("lineage", "primary"));
    }

    let source_refs = inspector
        .sources
        .iter()
        .flat_map(|source| {
            let mut refs = Vec::new();
            match &source.identity {
                ploke_tree::graph::ArtifactIdentity::HistoryRef(artifact) => {
                    refs.push(format!("artifact_history_ref:{}", artifact.value));
                }
                ploke_tree::graph::ArtifactIdentity::PassiveId(artifact) => {
                    refs.push(format!("artifact_id:{}", artifact.0));
                }
            }
            if !source.evidence.is_empty() {
                refs.push(format!("artifact_evidence_count:{}", source.evidence.len()));
            }
            refs
        })
        .collect();

    SelectionInspectorSnapshot {
        kind: selection.kind.clone(),
        label: selection.label.clone(),
        identity: vec![InspectorRow::new("artifact", inspector.key)],
        roles,
        incoming: artifact_edges(&inspector.incoming),
        outgoing: artifact_edges(&inspector.outgoing),
        source_refs,
        unavailable: Vec::new(),
    }
}

fn artifact_edges(edges: &[ArtifactRelation<'_>]) -> Vec<InspectorEdge> {
    edges
        .iter()
        .map(|edge| InspectorEdge::new(edge.kind.as_str(), edge.from, edge.to, edge.source_count))
        .collect()
}

fn maybe_row(rows: &mut Vec<InspectorRow>, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        rows.push(InspectorRow::new(label, value));
    }
}

fn artifact_handle_label(index: usize) -> String {
    format!("A{index}")
}

fn render_rows(out: &mut String, heading: &str, rows: &[InspectorRow]) {
    if rows.is_empty() {
        out.push_str(&format!("{heading}: none\n"));
        return;
    }
    out.push_str(&format!("{heading}:\n"));
    for row in rows {
        out.push_str(&format!("- {}: {}\n", row.label, row.value));
    }
}

fn render_edges(out: &mut String, heading: &str, edges: &[InspectorEdge]) {
    if edges.is_empty() {
        out.push_str(&format!("{heading}: none\n"));
        return;
    }
    out.push_str(&format!("{heading}:\n"));
    for edge in edges {
        out.push_str(&format!(
            "- {}: {} -> {} ({})\n",
            edge.relation, edge.from, edge.to, edge.source_count
        ));
    }
}

#[cfg(test)]
mod tests {
    use ploke_tree::{
        AuthorityLabel, CampaignRef, EvidenceKind, EvidenceRef, Lanes, NodeKey, NodeKind, Phase,
        Progress, ResultClass, RunForest, Terminality, TreeNode,
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
                passive_evidence: Default::default(),
                diagnostics: Vec::new(),
            }),
            ..Default::default()
        };
        let (selection, inspector) = SelectionInspector::from_default_selector(&graph, "A2")
            .expect("A2 resolves from default visible selection order");
        let snapshot = inspector.snapshot(&selection);

        match inspector {
            SelectionInspector::RunForestNode(run) => {
                assert_eq!(run.node.key.as_str(), "child");
                assert_eq!(run.parent.unwrap().key.as_str(), "parent");
            }
            _ => panic!("expected run-forest node inspection"),
        }
        assert_eq!(snapshot.label, "A2");
        assert!(snapshot.has_record_refs());
        assert_eq!(snapshot.incoming.len(), 1);
        assert_eq!(snapshot.incoming[0].relation, "E_F");
        assert_eq!(snapshot.incoming[0].from, "parent");
        assert_eq!(snapshot.incoming[0].to, "child");
        assert!(snapshot.outgoing.is_empty());
        assert!(
            snapshot
                .identity
                .iter()
                .any(|row| row.label == "source artifact" && row.value == "artifact:child")
        );
    }

    fn key(value: &str) -> NodeKey {
        NodeKey::from(value)
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
            base_artifact_id: None,
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
}
