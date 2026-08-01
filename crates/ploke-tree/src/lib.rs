//! Read-only tree projections over passive Ploke records.
//!
//! This crate assembles UI-facing DTOs from passive records. Its filesystem
//! loader only deserializes machine-readable run files; it does not mutate loop
//! state, parse human output, or infer History authority from passive evidence.

use std::collections::BTreeMap;

pub mod browser;
pub mod graph;
mod playback;
pub mod store;
#[cfg(test)]
mod tests;

pub use graph::Graph;
pub use playback::{
    CoarseHistorySpine, CoarseHistoryStep, CoarseHistoryWarning, PlaybackCursor, PlaybackScope,
    ResponseTapeRef, RuntimeCoarse, RuntimePlaybackDeltaRef, RuntimePlaybackFrameRef,
    RuntimePlaybackGranularity, RuntimePlaybackIndex, RuntimePlaybackRef, RuntimePlaybackStepRef,
    RuntimePlaybackWarning, TurnArtifactKind, TurnCursor, TurnEventKind, TurnEventPlaybackRefSteps,
    TurnEventStepRef, build_coarse_history_spine, coarse_run_playback_from_sealed_history,
    coarse_run_playback_ref_steps_from_sealed_history, fine_run_playback_from_sealed_history,
    fine_run_playback_ref_steps_from_sealed_history, project_coarse_history_spine,
    turn_event_step_at, turn_event_steps_from_agent_turn_records, turn_event_steps_from_artifact,
};
pub use store::*;

use ploke_records::identity::ParentIdentityRecord;
use ploke_records::ids::CampaignId;
use ploke_records::invocation::{
    SuccessorCompletionRecord, SuccessorCompletionStatus, SuccessorReadyRecord,
};
use ploke_records::scheduler::{NodeRecord, NodeStatusRecord};
use serde::{Deserialize, Serialize};

/// UI-facing forest assembled from passive records.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunForest {
    pub campaign: CampaignRef,
    pub roots: Vec<NodeKey>,
    pub nodes: Vec<TreeNode>,
    pub lanes: Lanes,
    #[serde(default)]
    pub passive_evidence: PassiveEvidence,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

impl RunForest {
    /// Assemble a read-only projection from already-loaded passive records.
    pub fn from_records(input: RunForestInput) -> Self {
        assemble_run_forest(input)
    }
}

/// Assemble a read-only projection from already-loaded passive records.
pub fn assemble_run_forest(input: RunForestInput) -> RunForest {
    let RunForestInput {
        scheduler,
        node_records,
        parent_identity,
        successor_ready,
        successor_completion,
        passive_evidence,
    } = input;

    let campaign_id = scheduler.campaign_id.clone();
    let mut diagnostics = Vec::new();
    let merged_node_records = merge_node_records(&scheduler.nodes, node_records, &mut diagnostics);
    let mut nodes = merged_node_records
        .iter()
        .map(TreeNode::from_scheduler_node)
        .collect::<Vec<_>>();

    let mut index_by_key = BTreeMap::new();
    for (idx, node) in nodes.iter().enumerate() {
        if index_by_key.insert(node.key.clone(), idx).is_some() {
            diagnostics.push(Diagnostic::forest(
                DiagnosticSeverity::Error,
                "duplicate_scheduler_node",
                format!("scheduler contains duplicate node id {}", node.key.as_str()),
            ));
        }
    }

    let mut roots = Vec::new();
    for source in &merged_node_records {
        let key = NodeKey::from(source.node_id.as_str());
        let Some(idx) = index_by_key.get(&key).copied() else {
            continue;
        };

        match source.parent_node_id.as_ref() {
            Some(parent_id) => {
                let parent_key = NodeKey::from(parent_id.as_str());
                if let Some(parent_idx) = index_by_key.get(&parent_key).copied() {
                    nodes[idx].parent = Some(parent_key.clone());
                    nodes[parent_idx].children.push(key);
                } else {
                    roots.push(key.clone());
                    nodes[idx].diagnostics.push(Diagnostic::node(
                        DiagnosticSeverity::Warning,
                        "missing_scheduler_parent",
                        format!(
                            "node {} names missing scheduler parent {}",
                            key.as_str(),
                            parent_key.as_str()
                        ),
                        key,
                    ));
                }
            }
            None => roots.push(key),
        }
    }

    if let Some(parent) = parent_identity {
        attach_parent_identity(
            &mut nodes,
            &index_by_key,
            &mut diagnostics,
            &campaign_id,
            parent,
        );
    }

    for ready in successor_ready {
        attach_successor_ready(
            &mut nodes,
            &index_by_key,
            &mut diagnostics,
            &campaign_id,
            ready,
        );
    }

    for completion in successor_completion {
        attach_successor_completion(
            &mut nodes,
            &index_by_key,
            &mut diagnostics,
            &campaign_id,
            completion,
        );
    }

    RunForest {
        campaign: CampaignRef {
            campaign_id,
            updated_at: scheduler.updated_at,
        },
        roots,
        nodes,
        lanes: Lanes {
            frontier: scheduler
                .frontier_node_ids
                .iter()
                .map(|id| NodeKey::from(id.as_str()))
                .collect(),
            completed: scheduler
                .completed_node_ids
                .iter()
                .map(|id| NodeKey::from(id.as_str()))
                .collect(),
            failed: scheduler
                .failed_node_ids
                .iter()
                .map(|id| NodeKey::from(id.as_str()))
                .collect(),
        },
        passive_evidence,
        diagnostics,
    }
}

fn merge_node_records(
    scheduler_nodes: &[NodeRecord],
    node_records: Vec<NodeRecord>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<NodeRecord> {
    let mut merged = scheduler_nodes.to_vec();
    let mut index_by_node_id = BTreeMap::new();

    for (idx, node) in merged.iter().enumerate() {
        if index_by_node_id
            .insert(node.node_id.as_str().to_owned(), idx)
            .is_some()
        {
            diagnostics.push(Diagnostic::forest(
                DiagnosticSeverity::Error,
                "duplicate_scheduler_node",
                format!("scheduler contains duplicate node id {}", node.node_id),
            ));
        }
    }

    for node in node_records {
        let node_id = node.node_id.as_str().to_owned();
        if let Some(idx) = index_by_node_id.get(&node_id).copied() {
            merged[idx] = node;
        } else {
            index_by_node_id.insert(node_id, merged.len());
            merged.push(node);
        }
    }

    merged
}

fn attach_parent_identity(
    nodes: &mut [TreeNode],
    index_by_key: &BTreeMap<NodeKey, usize>,
    diagnostics: &mut Vec<Diagnostic>,
    campaign_id: &CampaignId,
    parent: ParentIdentityRecord,
) {
    let evidence = EvidenceRef {
        kind: EvidenceKind::ParentIdentity,
        authority: AuthorityLabel::TypedRecordEvidence,
        node_key: Some(NodeKey::from(parent.node_id.as_str())),
        runtime_id: None,
        recorded_at: Some(parent.created_at),
        detail: Some(format!("parent_id={}", parent.parent_id)),
    };

    if parent.campaign_id != *campaign_id {
        diagnostics.push(Diagnostic::forest_with_evidence(
            DiagnosticSeverity::Warning,
            "parent_identity_campaign_mismatch",
            format!(
                "parent identity campaign {} does not match scheduler campaign {}",
                parent.campaign_id, campaign_id
            ),
            evidence,
        ));
        return;
    }

    let key = evidence
        .node_key
        .clone()
        .expect("parent identity has node key");
    if let Some(idx) = index_by_key.get(&key).copied() {
        nodes[idx].evidence.push(evidence);
    } else {
        diagnostics.push(Diagnostic::forest_with_evidence(
            DiagnosticSeverity::Warning,
            "parent_identity_without_scheduler_node",
            format!(
                "parent identity references node {} absent from scheduler projection",
                key.as_str()
            ),
            evidence,
        ));
    }
}

fn attach_successor_ready(
    nodes: &mut [TreeNode],
    index_by_key: &BTreeMap<NodeKey, usize>,
    diagnostics: &mut Vec<Diagnostic>,
    campaign_id: &CampaignId,
    ready: SuccessorReadyRecord,
) {
    let key = NodeKey::from(ready.node_id.as_str());
    let evidence = EvidenceRef {
        kind: EvidenceKind::SuccessorReady,
        authority: AuthorityLabel::TypedRecordEvidence,
        node_key: Some(key.clone()),
        runtime_id: Some(ready.runtime_id.to_string()),
        recorded_at: Some(ready.recorded_at),
        detail: Some(format!("pid={}", ready.pid)),
    };

    if ready.campaign_id != *campaign_id {
        diagnostics.push(Diagnostic::forest_with_evidence(
            DiagnosticSeverity::Warning,
            "successor_ready_campaign_mismatch",
            format!(
                "successor ready campaign {} does not match scheduler campaign {}",
                ready.campaign_id, campaign_id
            ),
            evidence,
        ));
        return;
    }

    if let Some(idx) = index_by_key.get(&key).copied() {
        nodes[idx].evidence.push(evidence);
    } else {
        diagnostics.push(Diagnostic::forest_with_evidence(
            DiagnosticSeverity::Warning,
            "successor_ready_without_scheduler_node",
            format!(
                "successor ready record references node {} absent from scheduler projection",
                key.as_str()
            ),
            evidence,
        ));
    }
}

fn attach_successor_completion(
    nodes: &mut [TreeNode],
    index_by_key: &BTreeMap<NodeKey, usize>,
    diagnostics: &mut Vec<Diagnostic>,
    campaign_id: &CampaignId,
    completion: SuccessorCompletionRecord,
) {
    let key = NodeKey::from(completion.node_id.as_str());
    let status = completion.status;
    let evidence = EvidenceRef {
        kind: EvidenceKind::SuccessorCompletion,
        authority: AuthorityLabel::TypedRecordEvidence,
        node_key: Some(key.clone()),
        runtime_id: Some(completion.runtime_id.to_string()),
        recorded_at: Some(completion.recorded_at),
        detail: completion.detail,
    };

    if completion.campaign_id != *campaign_id {
        diagnostics.push(Diagnostic::forest_with_evidence(
            DiagnosticSeverity::Warning,
            "successor_completion_campaign_mismatch",
            format!(
                "successor completion campaign {} does not match scheduler campaign {}",
                completion.campaign_id, campaign_id
            ),
            evidence,
        ));
        return;
    }

    if let Some(idx) = index_by_key.get(&key).copied() {
        nodes[idx].evidence.push(evidence.clone());
        if status == SuccessorCompletionStatus::Failed {
            nodes[idx].diagnostics.push(Diagnostic::node_with_evidence(
                DiagnosticSeverity::Warning,
                "successor_completion_failed",
                format!("successor completion failed for node {}", key.as_str()),
                key,
                evidence,
            ));
        }
    } else {
        diagnostics.push(Diagnostic::forest_with_evidence(
            DiagnosticSeverity::Warning,
            "successor_completion_without_scheduler_node",
            format!(
                "successor completion references node {} absent from scheduler projection",
                key.as_str()
            ),
            evidence,
        ));
    }
}

/// Stable node key used by tree projections.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeKey(String);

impl NodeKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for NodeKey {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for NodeKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Coarse kind of node visible to UI renderers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    SchedulerSearchNode,
}

/// Authority level for a projected node or attached evidence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityLabel {
    MutableProjection,
    TypedRecordEvidence,
    LiveTransport,
    SealedVerifiedHistory,
    DegradedObservation,
}

/// Progress folded from scheduler node status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Progress {
    pub phase: Phase,
    pub terminality: Terminality,
    pub result_class: ResultClass,
}

/// UI phase for a scheduler node.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Planned,
    WorkspaceStaged,
    BinaryBuilt,
    Running,
    Completed,
    Failed,
    Unknown,
}

/// Whether a node appears terminal from the projection source.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Terminality {
    NonTerminal,
    Terminal,
    Unknown,
}

/// Result class visible from passive records.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultClass {
    Success,
    Failure,
    Unknown,
}

/// Reference to a source record that contributed to the projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceRef {
    pub kind: EvidenceKind,
    pub authority: AuthorityLabel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_key: Option<NodeKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Source record kind for an evidence reference.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    SchedulerNode,
    ParentIdentity,
    SuccessorReady,
    SuccessorCompletion,
}

/// Projection diagnostic for conservative assembly gaps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_key: Option<NodeKey>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
}

impl Diagnostic {
    fn forest(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            node_key: None,
            evidence: Vec::new(),
        }
    }

    fn forest_with_evidence(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        evidence: EvidenceRef,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            node_key: None,
            evidence: vec![evidence],
        }
    }

    fn node(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        node_key: NodeKey,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            node_key: Some(node_key),
            evidence: Vec::new(),
        }
    }

    fn node_with_evidence(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        node_key: NodeKey,
        evidence: EvidenceRef,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            node_key: Some(node_key),
            evidence: vec![evidence],
        }
    }
}

/// Severity for projection diagnostics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// Lightweight campaign identity for renderers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CampaignRef {
    pub campaign_id: CampaignId,
    pub updated_at: String,
}

/// Scheduler lane membership as a projection aid.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lanes {
    pub frontier: Vec<NodeKey>,
    pub completed: Vec<NodeKey>,
    pub failed: Vec<NodeKey>,
}

/// One tree node in a run forest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreeNode {
    pub key: NodeKey,
    pub kind: NodeKind,
    pub authority: AuthorityLabel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<NodeKey>,
    #[serde(default)]
    pub children: Vec<NodeKey>,
    pub generation: u32,
    pub branch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_branch_id: Option<String>,
    pub candidate_id: String,
    pub instance_id: String,
    pub source_state_id: String,
    pub target_relpath: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_artifact_id: Option<String>,
    pub progress: Progress,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

impl TreeNode {
    fn from_scheduler_node(record: &NodeRecord) -> Self {
        let key = NodeKey::from(record.node_id.as_str());
        Self {
            key: key.clone(),
            kind: NodeKind::SchedulerSearchNode,
            authority: AuthorityLabel::MutableProjection,
            parent: None,
            children: Vec::new(),
            generation: record.generation,
            branch_id: record.branch_id.as_str().to_owned(),
            parent_branch_id: record
                .parent_branch_id
                .as_ref()
                .map(|branch_id| branch_id.as_str().to_owned()),
            candidate_id: record.candidate_id.as_str().to_owned(),
            instance_id: record.instance_id.as_str().to_owned(),
            source_state_id: record.source_state_id.as_str().to_owned(),
            target_relpath: record.target_relpath.to_string_lossy().into_owned(),
            base_artifact_id: record.base_artifact_id.as_ref().map(|id| id.0.clone()),
            patch_id: record.patch_id.as_ref().map(|id| id.0.clone()),
            derived_artifact_id: record.derived_artifact_id.as_ref().map(|id| id.0.clone()),
            progress: progress_from_status(record.status),
            created_at: record.created_at.clone(),
            updated_at: record.updated_at.clone(),
            evidence: vec![EvidenceRef {
                kind: EvidenceKind::SchedulerNode,
                authority: AuthorityLabel::MutableProjection,
                node_key: Some(key),
                runtime_id: None,
                recorded_at: Some(record.updated_at.clone()),
                detail: None,
            }],
            diagnostics: Vec::new(),
        }
    }
}

fn progress_from_status(status: NodeStatusRecord) -> Progress {
    match status {
        NodeStatusRecord::Planned => Progress {
            phase: Phase::Planned,
            terminality: Terminality::NonTerminal,
            result_class: ResultClass::Unknown,
        },
        NodeStatusRecord::WorkspaceStaged => Progress {
            phase: Phase::WorkspaceStaged,
            terminality: Terminality::NonTerminal,
            result_class: ResultClass::Unknown,
        },
        NodeStatusRecord::BinaryBuilt => Progress {
            phase: Phase::BinaryBuilt,
            terminality: Terminality::NonTerminal,
            result_class: ResultClass::Unknown,
        },
        NodeStatusRecord::Running => Progress {
            phase: Phase::Running,
            terminality: Terminality::NonTerminal,
            result_class: ResultClass::Unknown,
        },
        NodeStatusRecord::Succeeded => Progress {
            phase: Phase::Completed,
            terminality: Terminality::Terminal,
            result_class: ResultClass::Success,
        },
        NodeStatusRecord::Failed => Progress {
            phase: Phase::Failed,
            terminality: Terminality::Terminal,
            result_class: ResultClass::Failure,
        },
    }
}
