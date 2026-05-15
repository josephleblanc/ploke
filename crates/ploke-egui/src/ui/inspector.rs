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
    pub patch: Option<PatchInspection<'g>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactInspection<'g> {
    pub key: &'g str,
    pub sources: Vec<&'g ploke_tree::graph::ArtifactNode>,
    pub incoming: Vec<ArtifactRelation<'g>>,
    pub outgoing: Vec<ArtifactRelation<'g>>,
    pub patches: Vec<PatchInspection<'g>>,
    pub selected_ruler: bool,
    pub primary_lineage: bool,
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
    pub artifact_incoming: Vec<InspectorEdge>,
    pub artifact_outgoing: Vec<InspectorEdge>,
    pub patches: Vec<PatchSnapshot>,
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
                artifact_incoming: Vec::new(),
                artifact_outgoing: Vec::new(),
                patches: Vec::new(),
                source_refs: Vec::new(),
                unavailable: vec![reason.row()],
            },
        }
    }

    pub(crate) fn has_record_refs(&self) -> bool {
        !self.source_refs.is_empty()
    }

    pub(crate) fn has_edges(&self) -> bool {
        !self.incoming.is_empty()
            || !self.outgoing.is_empty()
            || !self.artifact_incoming.is_empty()
            || !self.artifact_outgoing.is_empty()
    }

    pub fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("selection: {} {}\n", self.kind, self.label));
        render_rows(&mut out, "identity", &self.identity);
        render_rows(&mut out, "roles", &self.roles);
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchSnapshot {
    pub patch_id: String,
    pub summary: Vec<InspectorRow>,
    pub touches: Vec<InspectorRow>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unified_diff: String,
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
        patch: graph
            .child_plans
            .child_for_node_id(node.key.as_str())
            .map(|child| PatchInspection { child }),
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

    let sources = graph
        .artifacts
        .artifacts
        .values()
        .filter(|artifact| {
            artifact_node_key(artifact)
                .is_some_and(|artifact_key| artifact_key == node.key.as_str())
        })
        .collect();
    let patches = incoming
        .iter()
        .chain(outgoing.iter())
        .flat_map(|relation| relation.patch_ids.iter().copied())
        .filter_map(|patch_id| {
            graph
                .child_plans
                .child_for_patch_id(patch_id)
                .map(|child| PatchInspection { child })
        })
        .collect();

    SelectionInspector::Artifact(ArtifactInspection {
        key: node.key.as_str(),
        sources,
        incoming,
        outgoing,
        patches,
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
        InspectorRow::new("run forest node", node.key.as_str()),
        InspectorRow::new("candidate", node.candidate_id.as_str()),
        InspectorRow::new("source artifact", node.source_state_id.as_str()),
    ];
    maybe_row(
        &mut identity,
        "parent run forest node",
        node.parent.as_ref().map(|parent| parent.as_str()),
    );
    identity.push(InspectorRow::new(
        "child run forest nodes",
        child_node_list(inspector.children),
    ));
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
    let artifact_outgoing = run_forest_artifact_edges(node);
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
        artifact_incoming: Vec::new(),
        artifact_outgoing,
        patches: inspector
            .patch
            .iter()
            .map(|patch| patch_snapshot(patch))
            .collect(),
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
        artifact_incoming: artifact_edges(&inspector.incoming),
        artifact_outgoing: artifact_edges(&inspector.outgoing),
        patches: inspector
            .patches
            .iter()
            .map(|patch| patch_snapshot(patch))
            .collect(),
        source_refs,
        unavailable: Vec::new(),
    }
}

fn patch_snapshot(patch: &PatchInspection<'_>) -> PatchSnapshot {
    let child = patch.child;
    let patch_id = child
        .surface
        .as_ref()
        .map(|surface| surface.patch_id.0.clone())
        .or_else(|| {
            child
                .node
                .patch_id
                .as_ref()
                .map(|patch_id| patch_id.0.clone())
        })
        .unwrap_or_else(|| "not_recorded".to_owned());
    let mut summary = vec![
        InspectorRow::new(
            "target",
            child.resolved.target_relpath.display().to_string(),
        ),
        InspectorRow::new("branch", child.resolved.branch.branch_id.as_str()),
        InspectorRow::new("candidate", child.resolved.branch.candidate_id.as_str()),
        InspectorRow::new("source hash", child.resolved.source_content_hash.as_str()),
        InspectorRow::new(
            "proposed hash",
            child.resolved.branch.proposed_content_hash.as_str(),
        ),
    ];
    if let Some(surface) = child.surface.as_ref() {
        summary.push(InspectorRow::new(
            "base artifact",
            surface.base.artifact_id.0.as_str(),
        ));
        summary.push(InspectorRow::new(
            "derived artifact",
            surface.after.artifact_id.0.as_str(),
        ));
        summary.push(InspectorRow::new(
            "check",
            format!("{:?}", surface.check_status),
        ));
        summary.push(InspectorRow::new(
            "apply",
            format!("{:?}", surface.apply_status),
        ));
    }

    let touches = child
        .surface
        .as_ref()
        .map(|surface| {
            surface
                .touches
                .iter()
                .enumerate()
                .map(|(index, touch)| {
                    InspectorRow::new(
                        format!(
                            "{} {}:{}-{}",
                            index + 1,
                            touch.span_relpath.display(),
                            touch.start,
                            touch.end
                        ),
                        touch.replacement.as_str(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    PatchSnapshot {
        patch_id,
        summary,
        touches,
        unified_diff: crate::ui::diff::unified_rust_diff(
            child.resolved.target_relpath.to_string_lossy().as_ref(),
            child.resolved.source_content.as_str(),
            child.resolved.branch.proposed_content.as_str(),
        ),
    }
}

fn run_forest_artifact_edges(node: &ploke_tree::TreeNode) -> Vec<InspectorEdge> {
    let Some(from) = node.base_artifact_id.as_deref() else {
        return Vec::new();
    };
    let Some(to) = node.derived_artifact_id.as_deref() else {
        return Vec::new();
    };
    vec![InspectorEdge::new(
        ArtifactRelationKind::AppliedPatch.as_str(),
        from,
        to,
        1,
    )]
}

fn artifact_node_key(artifact: &ploke_tree::graph::ArtifactNode) -> Option<&str> {
    match &artifact.identity {
        ploke_tree::graph::ArtifactIdentity::HistoryRef(record) => Some(record.value.as_str()),
        ploke_tree::graph::ArtifactIdentity::PassiveId(artifact) => Some(artifact.0.as_str()),
    }
}

fn child_node_list(children: &[ploke_tree::NodeKey]) -> String {
    if children.is_empty() {
        return "none".to_owned();
    }
    children
        .iter()
        .map(|child| child.as_str())
        .collect::<Vec<_>>()
        .join(", ")
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

fn render_patches(out: &mut String, patches: &[PatchSnapshot]) {
    if patches.is_empty() {
        out.push_str("patches: none\n");
        return;
    }
    out.push_str("patches:\n");
    for patch in patches {
        out.push_str(&format!("- patch_id: {}\n", patch.patch_id));
        for row in &patch.summary {
            out.push_str(&format!("  - {}: {}\n", row.label, row.value));
        }
        if patch.touches.is_empty() {
            out.push_str("  - touches: none\n");
        } else {
            for touch in &patch.touches {
                out.push_str(&format!("  - touch {}: {}\n", touch.label, touch.value));
            }
        }
        render_diff_preview(out, &patch.unified_diff);
    }
}

fn render_diff_preview(out: &mut String, diff: &str) {
    if diff.is_empty() {
        out.push_str("  - diff: not_available\n");
        return;
    }

    let line_count = diff.lines().count();
    out.push_str(&format!("  - diff: available lines={line_count}\n"));
    for line in diff.lines().take(40) {
        out.push_str("    ");
        out.push_str(truncate_chars(line, 240));
        out.push('\n');
    }
    if line_count > 40 {
        out.push_str(&format!("    ... truncated {} lines\n", line_count - 40));
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> &str {
    if text.chars().count() <= max_chars {
        return text;
    }

    text.char_indices()
        .nth(max_chars)
        .map(|(index, _)| &text[..index])
        .unwrap_or(text)
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
        assert_eq!(snapshot.artifact_outgoing.len(), 1);
        assert_eq!(snapshot.artifact_outgoing[0].relation, "P_B");
        assert_eq!(snapshot.artifact_outgoing[0].from, "artifact:child:base");
        assert_eq!(snapshot.artifact_outgoing[0].to, "artifact:child:after");
        assert!(
            snapshot
                .identity
                .iter()
                .any(|row| row.label == "run forest node" && row.value == "child")
        );
        assert!(
            snapshot
                .identity
                .iter()
                .any(|row| row.label == "parent run forest node" && row.value == "parent")
        );
        assert!(
            snapshot
                .identity
                .iter()
                .any(|row| row.label == "child run forest nodes" && row.value == "none")
        );
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
}
