//! Graph-resolved selection inspector projections.

use serde::Serialize;

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
    fn row(self) -> InspectorRow<'static> {
        match self {
            Self::RunForestNotPresent => InspectorRow::new("run forest", "not_present"),
            Self::RunForestNodeNotFound => InspectorRow::new("run forest node", "not_found"),
            Self::ArtifactNotFound => InspectorRow::new("artifact", "not_found"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SelectionInspectorSnapshot<'a> {
    pub kind: &'a str,
    pub label: &'a str,
    pub identity: Vec<InspectorRow<'a>>,
    pub roles: Vec<InspectorRow<'a>>,
    pub metrics: Vec<InspectorMetric>,
    pub incoming: Vec<InspectorEdge<'a>>,
    pub outgoing: Vec<InspectorEdge<'a>>,
    pub artifact_incoming: Vec<InspectorEdge<'a>>,
    pub artifact_outgoing: Vec<InspectorEdge<'a>>,
    pub patches: Vec<PatchSnapshot<'a>>,
    pub source_refs: Vec<SourceRef<'a>>,
    pub unavailable: Vec<InspectorRow<'a>>,
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
                identity: Vec::new(),
                roles: Vec::new(),
                metrics: Vec::new(),
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
        render_metrics(&mut out, "metrics", &self.metrics);
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
        render_rows(&mut out, "unavailable", &self.unavailable);
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct InspectorRow<'a> {
    pub label: &'static str,
    pub value: &'a str,
}

impl<'a> InspectorRow<'a> {
    fn new(label: &'static str, value: &'a str) -> Self {
        Self { label, value }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct InspectorMetric {
    pub label: &'static str,
    pub value: usize,
}

impl InspectorMetric {
    fn new(label: &'static str, value: usize) -> Self {
        Self { label, value }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct InspectorEdge<'a> {
    pub relation: &'static str,
    pub from: &'a str,
    pub to: &'a str,
    pub source_count: usize,
}

impl<'a> InspectorEdge<'a> {
    fn new(relation: &'static str, from: &'a str, to: &'a str, source_count: usize) -> Self {
        Self {
            relation,
            from,
            to,
            source_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PatchSnapshot<'a> {
    pub patch_id: &'a str,
    pub summary: Vec<InspectorRow<'a>>,
    pub touches: Vec<PatchTouch<'a>>,
    pub source_content: &'a str,
    pub proposed_content: &'a str,
    pub target_relpath: &'a str,
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
    ArtifactEvidenceCount {
        count: usize,
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
            Self::ArtifactEvidenceCount { count } => write!(f, "artifact_evidence_count:{count}"),
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

fn snapshot_run_forest<'a, 'g>(
    selection: &'a GraphSelectionDetail,
    inspector: RunForestNodeInspection<'g>,
) -> SelectionInspectorSnapshot<'a>
where
    'g: 'a,
{
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
        InspectorRow::new("branch", node.branch_id.as_str()),
        InspectorRow::new("target", node.target_relpath.as_str()),
        InspectorRow::new("phase", phase_label(node.progress.phase)),
        InspectorRow::new("result", result_class_label(node.progress.result_class)),
    ];
    let metrics = vec![
        InspectorMetric::new("generation", node.generation as usize),
        InspectorMetric::new("child run forest nodes", inspector.children.len()),
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
        identity,
        roles,
        metrics,
        incoming,
        outgoing,
        artifact_incoming: Vec::new(),
        artifact_outgoing,
        patches: inspector.patch.into_iter().map(patch_snapshot).collect(),
        source_refs,
        unavailable: Vec::new(),
    }
}

fn snapshot_artifact<'a, 'g>(
    selection: &'a GraphSelectionDetail,
    inspector: ArtifactInspection<'g>,
) -> SelectionInspectorSnapshot<'a>
where
    'g: 'a,
{
    let mut roles = Vec::new();
    if inspector.selected_ruler {
        roles.push(InspectorRow::new("selected ruler", "true"));
    }
    if inspector.primary_lineage {
        roles.push(InspectorRow::new("lineage", "primary"));
    }

    let source_refs = inspector
        .sources
        .iter()
        .flat_map(|source| match &source.identity {
            ploke_tree::graph::ArtifactIdentity::HistoryRef(artifact) => vec![
                SourceRef::ArtifactHistoryRef {
                    artifact: artifact.value.as_str(),
                },
                SourceRef::ArtifactEvidenceCount {
                    count: source.evidence.len(),
                },
            ],
            ploke_tree::graph::ArtifactIdentity::PassiveId(artifact) => vec![
                SourceRef::ArtifactId {
                    artifact: artifact.0.as_str(),
                },
                SourceRef::ArtifactEvidenceCount {
                    count: source.evidence.len(),
                },
            ],
        })
        .collect();

    SelectionInspectorSnapshot {
        kind: selection.kind.as_str(),
        label: selection.label.as_str(),
        identity: vec![InspectorRow::new("artifact", inspector.key)],
        roles,
        metrics: vec![InspectorMetric::new(
            "source records",
            inspector.sources.len(),
        )],
        incoming: artifact_edges(&inspector.incoming),
        outgoing: artifact_edges(&inspector.outgoing),
        artifact_incoming: artifact_edges(&inspector.incoming),
        artifact_outgoing: artifact_edges(&inspector.outgoing),
        patches: inspector.patches.into_iter().map(patch_snapshot).collect(),
        source_refs,
        unavailable: Vec::new(),
    }
}

fn patch_snapshot<'a>(patch: PatchInspection<'a>) -> PatchSnapshot<'a> {
    let child = patch.child;
    let patch_id = child
        .surface
        .as_ref()
        .map(|surface| surface.patch_id.0.as_str())
        .or_else(|| {
            child
                .node
                .patch_id
                .as_ref()
                .map(|patch_id| patch_id.0.as_str())
        })
        .unwrap_or("not_recorded");
    let mut summary = vec![
        InspectorRow::new(
            "target",
            child
                .resolved
                .target_relpath
                .to_str()
                .unwrap_or("non_utf8_path"),
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
            surface_check_status_label(surface.check_status),
        ));
        summary.push(InspectorRow::new(
            "apply",
            surface_apply_status_label(surface.apply_status),
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
                .map(|(index, touch)| PatchTouch {
                    index: index + 1,
                    relpath: touch.span_relpath.to_str().unwrap_or("non_utf8_path"),
                    start: touch.start,
                    end: touch.end,
                    replacement: touch.replacement.as_str(),
                })
                .collect()
        })
        .unwrap_or_default();

    PatchSnapshot {
        patch_id,
        summary,
        touches,
        source_content: child.resolved.source_content.as_str(),
        proposed_content: child.resolved.branch.proposed_content.as_str(),
        target_relpath: child
            .resolved
            .target_relpath
            .to_str()
            .unwrap_or("non_utf8_path"),
    }
}

fn run_forest_artifact_edges(node: &ploke_tree::TreeNode) -> Vec<InspectorEdge<'_>> {
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

fn artifact_edges<'a>(edges: &[ArtifactRelation<'a>]) -> Vec<InspectorEdge<'a>> {
    edges
        .iter()
        .map(|edge| InspectorEdge::new(edge.kind.as_str(), edge.from, edge.to, edge.source_count))
        .collect()
}

fn maybe_row<'a>(rows: &mut Vec<InspectorRow<'a>>, label: &'static str, value: Option<&'a str>) {
    if let Some(value) = value {
        rows.push(InspectorRow::new(label, value));
    }
}

fn artifact_handle_label(index: usize) -> String {
    format!("A{index}")
}

fn phase_label(phase: ploke_tree::Phase) -> &'static str {
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

fn result_class_label(result_class: ploke_tree::ResultClass) -> &'static str {
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

fn surface_check_status_label(
    status: ploke_records::history::SurfaceCheckStatusRecord,
) -> &'static str {
    match status {
        ploke_records::history::SurfaceCheckStatusRecord::Checked => "checked",
    }
}

fn surface_apply_status_label(
    status: ploke_records::history::SurfaceApplyStatusRecord,
) -> &'static str {
    match status {
        ploke_records::history::SurfaceApplyStatusRecord::Applied => "applied",
    }
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

fn render_metrics(out: &mut String, heading: &str, metrics: &[InspectorMetric]) {
    if metrics.is_empty() {
        out.push_str(&format!("{heading}: none\n"));
        return;
    }
    out.push_str(&format!("{heading}:\n"));
    for metric in metrics {
        out.push_str(&format!("- {}: {}\n", metric.label, metric.value));
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
        SourceRef::ArtifactEvidenceCount { count } => {
            out.push_str(&format!("- artifact_evidence_count:{count}\n"));
        }
    }
}

fn render_edges(out: &mut String, heading: &str, edges: &[InspectorEdge<'_>]) {
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
                out.push_str(&format!(
                    "  - touch {} {}:{}-{}: {}\n",
                    touch.index, touch.relpath, touch.start, touch.end, touch.replacement
                ));
            }
        }
        render_diff_preview(out, &patch.unified_diff());
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
                .metrics
                .iter()
                .any(|metric| metric.label == "child run forest nodes" && metric.value == 0)
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
