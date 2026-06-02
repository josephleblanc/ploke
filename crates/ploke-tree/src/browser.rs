//! Renderer-neutral models for browsing typed Ploke run playback.
//!
//! This crate is the front-facing boundary for future UI and WebAssembly work.
//! It does not parse CLI output, read run directories, or decide History
//! authority. Callers provide typed records or `ploke-tree` projections; this
//! crate shapes them for visual browsing.

#[cfg(feature = "projection")]
use crate::{
    CoarseHistorySpine, CoarseHistoryWarning, build_coarse_history_spine,
    playback::fine_history_steps_from_sealed_history,
};
use ploke_records::branch::Disposition;
use ploke_records::ids::{ArtifactId, Coordinate, OperationTarget, PatchId, RuntimeId};
use ploke_records::playback::{EvidenceStrength, FineOrder, FineStepKind};
#[cfg(feature = "projection")]
use ploke_records::{evaluation::Artifact as EvaluationArtifact, history::SealedBlockRecord};
use serde::{Deserialize, Serialize};
#[cfg(feature = "projection")]
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackBrowserModel {
    pub schema_version: String,
    pub granularity: BrowserGranularity,
    pub step_count: usize,
    pub warning_count: usize,
    pub steps: Vec<PlaybackBrowserStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_graph: Option<RunExecutionGraph>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_summary: Option<RunSummary>,
}

/// Renderer-neutral projection of the Prototype 1 generative execution graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunExecutionGraph {
    pub schema_version: String,
    pub nodes: Vec<ExecutionNode>,
    pub edges: Vec<ExecutionEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionNode {
    pub id: String,
    pub kind: ExecutionNodeKind,
    pub label: String,
    pub evidence: EvidenceStrength,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_id: Option<RuntimeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_id: Option<PatchId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinate: Option<Coordinate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_target: Option<OperationTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionNodeKind {
    Runtime,
    Artifact,
    Operation,
    PatchAttempt,
    Evaluation,
    Selection,
    Handoff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionEdge {
    pub from: String,
    pub to: String,
    pub kind: ExecutionEdgeKind,
    pub evidence: EvidenceStrength,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEdgeKind {
    RuntimeExecutesOperation,
    RuntimeOperatesOnArtifact,
    ArtifactInputToOperation,
    OperationProducesPatch,
    PatchDerivesArtifact,
    ArtifactHydratesRuntime,
    BranchEvaluatedBy,
    EvaluationSelectsArtifact,
    HandoffLaunchesRuntime,
}

/// Top-level summary of a completed (or in-progress) run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSummary {
    pub campaign_id: String,
    pub node_count: usize,
    pub generation_count: u64,
    pub sealed_block_count: usize,
    pub evaluation_count: usize,
    pub evaluations_kept: usize,
    pub evaluations_rejected: usize,
    pub journal_entry_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_status: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserGranularity {
    CoarseHistory,
    FineHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackBrowserStep {
    pub index: usize,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fine_kind: Option<FineStepKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<FineOrder>,
    pub block_height: u64,
    pub block_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_candidate: Option<String>,
    pub considered_candidate_count: usize,
    pub warning_count: usize,
    pub evidence: EvidenceStrength,
    // ── join keys (extracted from label or joined from journal) ──
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub membership_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_set_root: Option<String>,
    // ── detail snapshots (populated by enriched projections) ──
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<EvaluationSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<SurfaceSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<ProtocolSnapshot>,
}

/// Evaluation metrics snapshot for a candidate step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationSnapshot {
    pub disposition: Disposition,
    pub tool_calls_total: u64,
    pub tool_calls_failed: u64,
    pub patch_attempted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_apply_state: Option<String>,
    pub nonempty_valid_patch: bool,
    pub convergence: bool,
    pub oracle_eligible: bool,
    pub aborted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasons: Option<Vec<String>>,
}

/// Surface/edit evidence snapshot for a candidate step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSnapshot {
    pub target_relpath: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_state_id: Option<String>,
}

/// Protocol artifact summary for a candidate step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolSnapshot {
    pub intent_segmentation_count: usize,
    pub tool_call_review_count: usize,
    pub segment_review_count: usize,
    pub issue_detection_count: usize,
    pub synthesis_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
}

/// Information about a node's branch, extracted from the transition journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeBranchInfo {
    pub branch_id: String,
    pub candidate_id: String,
    pub target_relpath: String,
    pub source_state_id: String,
    pub source_artifact_id: Option<ArtifactId>,
    pub operation_target: Option<OperationTarget>,
    pub generation_coordinate: Option<Coordinate>,
    pub source_content_hash: Option<String>,
    pub proposed_content_hash: Option<String>,
    pub patch_id: Option<PatchId>,
    pub derived_artifact_id: Option<ArtifactId>,
    pub child_runtime_id: Option<RuntimeId>,
    pub successor_runtime_id: Option<RuntimeId>,
}

#[cfg(feature = "projection")]
pub fn coarse_history_browser_model_from_blocks(
    blocks: &[SealedBlockRecord],
) -> PlaybackBrowserModel {
    coarse_history_browser_model(&build_coarse_history_spine(blocks))
}

#[cfg(feature = "projection")]
pub fn coarse_history_browser_model(spine: &CoarseHistorySpine) -> PlaybackBrowserModel {
    let steps = spine
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| PlaybackBrowserStep {
            index,
            id: step.block_hash.clone(),
            fine_kind: None,
            order: None,
            block_height: step.block_height,
            block_hash: step.block_hash.clone(),
            label: None,
            selected_candidate: step.selected_candidate.clone(),
            considered_candidate_count: step.considered_candidate_count,
            warning_count: warnings_for_block(&spine.warnings, step.block_height),
            evidence: EvidenceStrength::SealedHistory,
            node_id: None,
            branch_id: None,
            candidate_id: None,
            occurrence_id: step.selected_occurrence_id.clone(),
            membership_id: step.selected_membership_id.clone(),
            candidate_set_root: None,
            evaluation: None,
            surface: None,
            protocol: None,
        })
        .collect::<Vec<_>>();

    PlaybackBrowserModel {
        schema_version: "ploke-tree-browser.playback.v1".to_owned(),
        granularity: BrowserGranularity::CoarseHistory,
        step_count: steps.len(),
        warning_count: spine.warnings.len(),
        steps,
        execution_graph: None,
        run_summary: None,
    }
}

#[cfg(feature = "projection")]
pub fn fine_history_browser_model_from_blocks(
    blocks: &[SealedBlockRecord],
) -> PlaybackBrowserModel {
    let playback = fine_history_steps_from_sealed_history(blocks);
    let steps = playback
        .into_iter()
        .enumerate()
        .map(|(index, step)| {
            let block_hash = block_hash_from_fine_step(&step.id).unwrap_or_default();
            PlaybackBrowserStep {
                index,
                id: step.id,
                fine_kind: Some(step.kind),
                order: Some(step.order),
                block_height: step.order.block_height,
                block_hash,
                label: step.label,
                selected_candidate: None,
                considered_candidate_count: 0,
                warning_count: 0,
                evidence: step.evidence,
                node_id: None,
                branch_id: None,
                candidate_id: None,
                occurrence_id: step.occurrence_id,
                membership_id: step.membership_id,
                candidate_set_root: step.candidate_set_root,
                evaluation: None,
                surface: None,
                protocol: None,
            }
        })
        .collect::<Vec<_>>();

    PlaybackBrowserModel {
        schema_version: "ploke-tree-browser.playback.v1".to_owned(),
        granularity: BrowserGranularity::FineHistory,
        step_count: steps.len(),
        warning_count: 0,
        steps,
        execution_graph: None,
        run_summary: None,
    }
}

/// Enrich a fine-history browser model with evaluation, surface, and protocol
/// detail by joining against loaded records.
///
/// `evaluations` is keyed by `branch_id`.
/// `node_branches` maps `node-NODEID` → branch/surface info (built from the
/// transition journal's `MaterializeBranch` entries).
/// `protocol_snapshots` is keyed by `branch_id`; export code builds it from
/// the treatment run path in each branch evaluation artifact.
#[cfg(feature = "projection")]
pub fn enrich_fine_browser_model(
    model: &mut PlaybackBrowserModel,
    evaluations: &BTreeMap<String, EvaluationArtifact>,
    node_branches: &BTreeMap<String, NodeBranchInfo>,
    protocol_snapshots: Option<&BTreeMap<String, ProtocolSnapshot>>,
) {
    for step in &mut model.steps {
        // Extract node_id from candidate label: "candidate:node-NODEID:plan_index=N"
        let short_id = extract_node_id_from_label(step.label.as_deref());
        step.node_id = short_id.clone();

        // Build the full node_id key: "node-NODEID"
        let full_node_id = short_id.as_ref().map(|id| format!("node-{id}"));

        // Join through journal-derived node_branches map.
        if let Some(ref full_id) = full_node_id {
            if let Some(info) = node_branches.get(full_id) {
                step.branch_id = Some(info.branch_id.clone());
                step.candidate_id = Some(info.candidate_id.clone());
                step.surface = Some(SurfaceSnapshot {
                    target_relpath: info.target_relpath.clone(),
                    patch_id: info.patch_id.as_ref().map(|patch_id| patch_id.0.clone()),
                    source_content_hash: info.source_content_hash.clone(),
                    proposed_content_hash: info.proposed_content_hash.clone(),
                    source_state_id: Some(info.source_state_id.clone()),
                });
            }
        }

        // Attach evaluation snapshot by branch_id.
        if let Some(branch_id) = &step.branch_id {
            if let Some(protocol) =
                protocol_snapshots.and_then(|snapshots| snapshots.get(branch_id))
            {
                step.protocol = Some(protocol.clone());
            }

            if let Some(eval) = evaluations.get(branch_id.as_str()) {
                let metrics: Option<&ploke_records::evaluation::RunMetrics> = eval
                    .compared_instances
                    .first()
                    .and_then(|cmp| cmp.treatment_metrics.as_ref());
                step.evaluation = Some(EvaluationSnapshot {
                    disposition: eval.overall_disposition,
                    tool_calls_total: metrics.map_or(0, |m| m.tool_calls_total),
                    tool_calls_failed: metrics.map_or(0, |m| m.tool_calls_failed),
                    patch_attempted: metrics.map_or(false, |m| m.patch_attempted),
                    patch_apply_state: metrics.and_then(|m| {
                        if m.patch_apply_state.is_empty() {
                            None
                        } else {
                            Some(m.patch_apply_state.clone())
                        }
                    }),
                    nonempty_valid_patch: metrics.map_or(false, |m| m.nonempty_valid_patch),
                    convergence: metrics.map_or(false, |m| m.convergence),
                    oracle_eligible: metrics.map_or(false, |m| m.oracle_eligible),
                    aborted: metrics.map_or(false, |m| m.aborted),
                    reasons: Some(eval.reasons.clone()).filter(|r| !r.is_empty()),
                });
            }
        }
    }

    model.execution_graph = Some(build_execution_graph(model, node_branches));
}

#[cfg(feature = "projection")]
fn build_execution_graph(
    model: &PlaybackBrowserModel,
    node_branches: &BTreeMap<String, NodeBranchInfo>,
) -> RunExecutionGraph {
    let mut graph = GraphBuilder::default();

    for step in &model.steps {
        let Some(node_id) = step.node_id.as_ref().map(|id| format!("node-{id}")) else {
            continue;
        };
        let Some(info) = node_branches.get(&node_id) else {
            continue;
        };

        let evidence = step.evidence;
        let branch_context = BranchContext {
            node_id: Some(node_id.clone()),
            branch_id: Some(info.branch_id.clone()),
            candidate_id: Some(info.candidate_id.clone()),
            step_index: Some(step.index),
        };

        let base_artifact = info
            .source_artifact_id
            .clone()
            .or_else(|| artifact_from_target(info.operation_target.as_ref()))
            .or_else(|| {
                info.generation_coordinate
                    .as_ref()
                    .and_then(|coordinate| artifact_from_target(Some(&coordinate.target)))
            });

        if let Some(runtime_id) = info.child_runtime_id.clone() {
            let runtime_node = runtime_node_id(&runtime_id);
            graph.node(ExecutionNode {
                id: runtime_node.clone(),
                kind: ExecutionNodeKind::Runtime,
                label: format!("Runtime {}", runtime_id),
                evidence,
                runtime_id: Some(runtime_id),
                artifact_id: None,
                patch_id: None,
                coordinate: None,
                operation_target: None,
                node_id: branch_context.node_id.clone(),
                branch_id: branch_context.branch_id.clone(),
                candidate_id: branch_context.candidate_id.clone(),
                step_index: branch_context.step_index,
            });

            if let Some(artifact_id) = base_artifact.clone() {
                let artifact_node = artifact_node_id(&artifact_id);
                graph.artifact_node(artifact_id.clone(), evidence, &branch_context);
                graph.edge(
                    runtime_node,
                    artifact_node,
                    ExecutionEdgeKind::RuntimeOperatesOnArtifact,
                    evidence,
                    Some("operates on".to_owned()),
                );
            }
        }

        let operation_id = operation_node_id(&info.branch_id);
        graph.node(ExecutionNode {
            id: operation_id.clone(),
            kind: ExecutionNodeKind::Operation,
            label: format!("Operation {}", info.branch_id),
            evidence,
            runtime_id: info.child_runtime_id.clone(),
            artifact_id: base_artifact.clone(),
            patch_id: None,
            coordinate: info.generation_coordinate.clone(),
            operation_target: info
                .generation_coordinate
                .as_ref()
                .map(|coordinate| coordinate.target.clone())
                .or_else(|| info.operation_target.clone()),
            node_id: branch_context.node_id.clone(),
            branch_id: branch_context.branch_id.clone(),
            candidate_id: branch_context.candidate_id.clone(),
            step_index: branch_context.step_index,
        });

        if let Some(runtime_id) = info.child_runtime_id.as_ref() {
            graph.edge(
                runtime_node_id(runtime_id),
                operation_id.clone(),
                ExecutionEdgeKind::RuntimeExecutesOperation,
                evidence,
                Some("executes".to_owned()),
            );
        }

        if let Some(artifact_id) = base_artifact.clone() {
            let artifact_node = artifact_node_id(&artifact_id);
            graph.artifact_node(artifact_id, evidence, &branch_context);
            graph.edge(
                artifact_node,
                operation_id.clone(),
                ExecutionEdgeKind::ArtifactInputToOperation,
                evidence,
                Some("input to".to_owned()),
            );
        }

        let patch_id = info.patch_id.clone();
        if let Some(patch_id) = patch_id.clone() {
            let patch_node = patch_node_id(&patch_id);
            graph.node(ExecutionNode {
                id: patch_node.clone(),
                kind: ExecutionNodeKind::PatchAttempt,
                label: format!("Patch {}", patch_id.0.as_str()),
                evidence,
                runtime_id: info.child_runtime_id.clone(),
                artifact_id: base_artifact.clone(),
                patch_id: Some(patch_id.clone()),
                coordinate: info.generation_coordinate.clone(),
                operation_target: info.operation_target.clone(),
                node_id: branch_context.node_id.clone(),
                branch_id: branch_context.branch_id.clone(),
                candidate_id: branch_context.candidate_id.clone(),
                step_index: branch_context.step_index,
            });
            graph.edge(
                operation_id.clone(),
                patch_node.clone(),
                ExecutionEdgeKind::OperationProducesPatch,
                evidence,
                Some("produces".to_owned()),
            );

            if let Some(derived_artifact_id) = info.derived_artifact_id.clone() {
                let derived_node = artifact_node_id(&derived_artifact_id);
                graph.artifact_node(derived_artifact_id.clone(), evidence, &branch_context);
                graph.edge(
                    patch_node,
                    derived_node.clone(),
                    ExecutionEdgeKind::PatchDerivesArtifact,
                    evidence,
                    Some("derives".to_owned()),
                );

                if let Some(eval) = step.evaluation.as_ref() {
                    let evaluation_node = evaluation_node_id(&info.branch_id);
                    graph.node(ExecutionNode {
                        id: evaluation_node.clone(),
                        kind: ExecutionNodeKind::Evaluation,
                        label: format!(
                            "Evaluation {}",
                            disposition_label_for_graph(eval.disposition)
                        ),
                        evidence,
                        runtime_id: None,
                        artifact_id: Some(derived_artifact_id.clone()),
                        patch_id: Some(patch_id),
                        coordinate: None,
                        operation_target: None,
                        node_id: branch_context.node_id.clone(),
                        branch_id: branch_context.branch_id.clone(),
                        candidate_id: branch_context.candidate_id.clone(),
                        step_index: branch_context.step_index,
                    });
                    graph.edge(
                        info_branch_node_id(&info.branch_id),
                        evaluation_node.clone(),
                        ExecutionEdgeKind::BranchEvaluatedBy,
                        evidence,
                        Some("evaluated by".to_owned()),
                    );
                    graph.edge(
                        evaluation_node.clone(),
                        derived_node.clone(),
                        ExecutionEdgeKind::EvaluationSelectsArtifact,
                        evidence,
                        Some(disposition_label_for_graph(eval.disposition)),
                    );

                    if matches!(eval.disposition, Disposition::Keep) {
                        let selection_node = selection_node_id(&info.branch_id);
                        graph.node(ExecutionNode {
                            id: selection_node.clone(),
                            kind: ExecutionNodeKind::Selection,
                            label: format!("Selection {}", info.branch_id),
                            evidence,
                            runtime_id: None,
                            artifact_id: Some(derived_artifact_id.clone()),
                            patch_id: None,
                            coordinate: None,
                            operation_target: None,
                            node_id: branch_context.node_id.clone(),
                            branch_id: branch_context.branch_id.clone(),
                            candidate_id: branch_context.candidate_id.clone(),
                            step_index: branch_context.step_index,
                        });
                        graph.edge(
                            evaluation_node,
                            selection_node.clone(),
                            ExecutionEdgeKind::EvaluationSelectsArtifact,
                            evidence,
                            Some("selects".to_owned()),
                        );
                        graph.edge(
                            selection_node,
                            derived_node.clone(),
                            ExecutionEdgeKind::EvaluationSelectsArtifact,
                            evidence,
                            Some("selected artifact".to_owned()),
                        );
                    }
                }

                if let Some(successor_runtime_id) = info.successor_runtime_id.clone() {
                    let successor_node = runtime_node_id(&successor_runtime_id);
                    graph.node(ExecutionNode {
                        id: successor_node.clone(),
                        kind: ExecutionNodeKind::Runtime,
                        label: format!("Runtime {}", successor_runtime_id),
                        evidence,
                        runtime_id: Some(successor_runtime_id.clone()),
                        artifact_id: Some(derived_artifact_id.clone()),
                        patch_id: None,
                        coordinate: None,
                        operation_target: None,
                        node_id: branch_context.node_id.clone(),
                        branch_id: branch_context.branch_id.clone(),
                        candidate_id: branch_context.candidate_id.clone(),
                        step_index: branch_context.step_index,
                    });
                    let handoff_node = handoff_node_id(&info.branch_id);
                    graph.node(ExecutionNode {
                        id: handoff_node.clone(),
                        kind: ExecutionNodeKind::Handoff,
                        label: format!("Handoff {}", successor_runtime_id),
                        evidence,
                        runtime_id: Some(successor_runtime_id),
                        artifact_id: Some(derived_artifact_id.clone()),
                        patch_id: None,
                        coordinate: None,
                        operation_target: None,
                        node_id: branch_context.node_id.clone(),
                        branch_id: branch_context.branch_id.clone(),
                        candidate_id: branch_context.candidate_id.clone(),
                        step_index: branch_context.step_index,
                    });
                    graph.edge(
                        derived_node.clone(),
                        handoff_node.clone(),
                        ExecutionEdgeKind::ArtifactHydratesRuntime,
                        evidence,
                        Some("hydrates".to_owned()),
                    );
                    graph.edge(
                        handoff_node,
                        successor_node,
                        ExecutionEdgeKind::HandoffLaunchesRuntime,
                        evidence,
                        Some("launches".to_owned()),
                    );
                }
            }
        }
    }

    graph.finish()
}

#[cfg(feature = "projection")]
#[derive(Default)]
struct GraphBuilder {
    nodes: BTreeMap<String, ExecutionNode>,
    edges: BTreeMap<(String, String, ExecutionEdgeKind), ExecutionEdge>,
}

#[cfg(feature = "projection")]
#[derive(Clone)]
struct BranchContext {
    node_id: Option<String>,
    branch_id: Option<String>,
    candidate_id: Option<String>,
    step_index: Option<usize>,
}

#[cfg(feature = "projection")]
impl GraphBuilder {
    fn node(&mut self, node: ExecutionNode) {
        self.nodes.entry(node.id.clone()).or_insert(node);
    }

    fn artifact_node(
        &mut self,
        artifact_id: ArtifactId,
        evidence: EvidenceStrength,
        context: &BranchContext,
    ) {
        let node_id = artifact_node_id(&artifact_id);
        self.node(ExecutionNode {
            id: node_id,
            kind: ExecutionNodeKind::Artifact,
            label: format!("Artifact {}", artifact_id.0.as_str()),
            evidence,
            runtime_id: None,
            artifact_id: Some(artifact_id),
            patch_id: None,
            coordinate: None,
            operation_target: None,
            node_id: context.node_id.clone(),
            branch_id: context.branch_id.clone(),
            candidate_id: context.candidate_id.clone(),
            step_index: context.step_index,
        });
    }

    fn edge(
        &mut self,
        from: String,
        to: String,
        kind: ExecutionEdgeKind,
        evidence: EvidenceStrength,
        label: Option<String>,
    ) {
        self.edges
            .entry((from.clone(), to.clone(), kind))
            .or_insert(ExecutionEdge {
                from,
                to,
                kind,
                evidence,
                label,
            });
    }

    fn finish(self) -> RunExecutionGraph {
        RunExecutionGraph {
            schema_version: "ploke-tree-browser.execution-graph.v1".to_owned(),
            nodes: self.nodes.into_values().collect(),
            edges: self.edges.into_values().collect(),
        }
    }
}

#[cfg(feature = "projection")]
fn artifact_from_target(target: Option<&OperationTarget>) -> Option<ArtifactId> {
    match target {
        Some(OperationTarget::Artifact { artifact_id }) => Some(artifact_id.clone()),
        Some(OperationTarget::PatchSet {
            base_artifact_id, ..
        }) => Some(base_artifact_id.clone()),
        Some(OperationTarget::ArtifactSet {
            base_artifact_id, ..
        }) => base_artifact_id.clone(),
        None => None,
    }
}

#[cfg(feature = "projection")]
fn runtime_node_id(runtime_id: &RuntimeId) -> String {
    format!("runtime:{runtime_id}")
}

#[cfg(feature = "projection")]
fn artifact_node_id(artifact_id: &ArtifactId) -> String {
    artifact_id.0.clone()
}

#[cfg(feature = "projection")]
fn patch_node_id(patch_id: &PatchId) -> String {
    patch_id.0.clone()
}

#[cfg(feature = "projection")]
fn operation_node_id(branch_id: &str) -> String {
    format!("operation:{branch_id}")
}

#[cfg(feature = "projection")]
fn evaluation_node_id(branch_id: &str) -> String {
    format!("evaluation:{branch_id}")
}

#[cfg(feature = "projection")]
fn selection_node_id(branch_id: &str) -> String {
    format!("selection:{branch_id}")
}

#[cfg(feature = "projection")]
fn handoff_node_id(branch_id: &str) -> String {
    format!("handoff:{branch_id}")
}

#[cfg(feature = "projection")]
fn info_branch_node_id(branch_id: &str) -> String {
    operation_node_id(branch_id)
}

#[cfg(feature = "projection")]
fn disposition_label_for_graph(disposition: Disposition) -> String {
    match disposition {
        Disposition::Keep => "keep".to_owned(),
        Disposition::Reject => "reject".to_owned(),
    }
}

#[cfg(feature = "projection")]
fn extract_node_id_from_label(label: Option<&str>) -> Option<String> {
    let label = label?;
    // Format: "candidate:node-NODEID:plan_index=N"
    if !label.starts_with("candidate:node-") {
        return None;
    }
    let rest = label.strip_prefix("candidate:node-")?;
    rest.split(':').next().map(ToOwned::to_owned)
}

/// Build a `RunSummary` from loaded evidence counts.
#[cfg(feature = "projection")]
pub fn build_run_summary(
    campaign_id: String,
    node_count: usize,
    max_generation: u64,
    sealed_block_count: usize,
    evaluation_summary: Option<&crate::EvaluationArtifactSummary>,
    journal_entry_count: usize,
) -> RunSummary {
    RunSummary {
        campaign_id,
        node_count,
        generation_count: max_generation,
        sealed_block_count,
        evaluation_count: evaluation_summary.map_or(0, |s| s.parsed_count),
        evaluations_kept: evaluation_summary.map_or(0, |s| s.keep_count),
        evaluations_rejected: evaluation_summary.map_or(0, |s| s.reject_count),
        journal_entry_count,
        terminal_status: None,
    }
}

#[cfg(feature = "projection")]
fn block_hash_from_fine_step(id: &str) -> Option<String> {
    if id.contains(":entry:") || id.contains(":successor-selected") {
        return id.split(':').nth(2).map(ToOwned::to_owned);
    }
    Some(id.to_owned())
}

#[cfg(feature = "projection")]
fn warnings_for_block(warnings: &[CoarseHistoryWarning], block_height: u64) -> usize {
    warnings
        .iter()
        .filter(|warning| match warning {
            CoarseHistoryWarning::ParentHashLinkMismatch {
                block_height: current,
                ..
            }
            | CoarseHistoryWarning::MissingSelectionDecisionPayload {
                block_height: current,
                ..
            } => *current == block_height,
        })
        .count()
}

#[cfg(all(test, feature = "projection"))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::CoarseHistoryStep;
    use ploke_records::history::{ActorRefRecord, ArtifactRefRecord, SuccessorRefRecord};

    #[test]
    fn browser_model_projects_coarse_history_steps() {
        let spine = CoarseHistorySpine {
            steps: vec![CoarseHistoryStep {
                block_height: 3,
                block_hash: "hash-3".to_owned(),
                parent_block_hashes: vec!["hash-2".to_owned()],
                ruling_parent: None,
                selected_successor: SuccessorRefRecord {
                    runtime: ActorRefRecord::Process("successor".to_owned()),
                    artifact: ArtifactRefRecord::from_artifact_id(ArtifactId(
                        "artifact:successor".to_owned(),
                    )),
                },
                selected_node: None,
                selected_branch: None,
                selected_candidate: Some("candidate:a".to_owned()),
                selected_occurrence_id: Some("occurrence-a".to_owned()),
                selected_membership_id: Some("membership-a".to_owned()),
                considered_candidate_count: 5,
            }],
            warnings: vec![CoarseHistoryWarning::MissingSelectionDecisionPayload {
                block_height: 3,
                block_hash: "hash-3".to_owned(),
            }],
        };

        let model = coarse_history_browser_model(&spine);

        assert_eq!(model.granularity, BrowserGranularity::CoarseHistory);
        assert_eq!(model.step_count, 1);
        assert_eq!(model.warning_count, 1);
        assert_eq!(model.steps[0].id, "hash-3");
        assert_eq!(model.steps[0].warning_count, 1);
        assert_eq!(model.steps[0].evidence, EvidenceStrength::SealedHistory);
        assert_eq!(
            model.steps[0].occurrence_id.as_deref(),
            Some("occurrence-a")
        );
        assert_eq!(
            model.steps[0].membership_id.as_deref(),
            Some("membership-a")
        );
    }

    #[test]
    fn browser_model_leaves_fine_fields_empty_for_coarse_steps() {
        let spine = CoarseHistorySpine {
            steps: vec![CoarseHistoryStep {
                block_height: 3,
                block_hash: "hash-3".to_owned(),
                parent_block_hashes: vec!["hash-2".to_owned()],
                ruling_parent: None,
                selected_successor: SuccessorRefRecord {
                    runtime: ActorRefRecord::Process("successor".to_owned()),
                    artifact: ArtifactRefRecord::from_artifact_id(ArtifactId(
                        "artifact:successor".to_owned(),
                    )),
                },
                selected_node: None,
                selected_branch: None,
                selected_candidate: None,
                selected_occurrence_id: None,
                selected_membership_id: None,
                considered_candidate_count: 0,
            }],
            warnings: Vec::new(),
        };

        let model = coarse_history_browser_model(&spine);

        assert_eq!(model.steps[0].fine_kind, None);
        assert_eq!(model.steps[0].order, None);
    }

    #[test]
    fn browser_model_projects_empty_fine_history() {
        let model = fine_history_browser_model_from_blocks(&[]);

        assert_eq!(model.granularity, BrowserGranularity::FineHistory);
        assert_eq!(model.step_count, 0);
        assert!(model.steps.is_empty());
    }

    #[test]
    fn browser_model_deserializes_serialized_nested_snapshots() {
        let model = PlaybackBrowserModel {
            schema_version: "ploke-tree-browser.playback.v1".to_owned(),
            granularity: BrowserGranularity::FineHistory,
            step_count: 1,
            warning_count: 0,
            steps: vec![PlaybackBrowserStep {
                index: 0,
                id: "block-hash:entry:0:candidate:0".to_owned(),
                fine_kind: Some(FineStepKind::CandidateConsidered),
                order: Some(FineOrder {
                    block_height: 7,
                    phase_rank: 20,
                    entry_index: Some(0),
                    candidate_index: Some(0),
                }),
                block_height: 7,
                block_hash: "block-hash".to_owned(),
                label: Some("candidate:node-abc123:plan_index=0".to_owned()),
                selected_candidate: Some("candidate:abc123".to_owned()),
                considered_candidate_count: 3,
                warning_count: 0,
                evidence: EvidenceStrength::SealedHistory,
                node_id: Some("abc123".to_owned()),
                branch_id: Some("branch-abc123".to_owned()),
                candidate_id: Some("candidate-abc123".to_owned()),
                occurrence_id: Some("occurrence-abc123".to_owned()),
                membership_id: Some("membership-abc123".to_owned()),
                candidate_set_root: Some("root-abc123".to_owned()),
                evaluation: Some(EvaluationSnapshot {
                    disposition: Disposition::Keep,
                    tool_calls_total: 4,
                    tool_calls_failed: 1,
                    patch_attempted: true,
                    patch_apply_state: Some("applied".to_owned()),
                    nonempty_valid_patch: true,
                    convergence: false,
                    oracle_eligible: true,
                    aborted: false,
                    reasons: Some(vec!["improved oracle score".to_owned()]),
                }),
                surface: Some(SurfaceSnapshot {
                    target_relpath: "src/lib.rs".to_owned(),
                    patch_id: Some("patch-1".to_owned()),
                    source_content_hash: Some("source-hash".to_owned()),
                    proposed_content_hash: Some("proposed-hash".to_owned()),
                    source_state_id: Some("source-state".to_owned()),
                }),
                protocol: Some(ProtocolSnapshot {
                    intent_segmentation_count: 1,
                    tool_call_review_count: 2,
                    segment_review_count: 3,
                    issue_detection_count: 4,
                    synthesis_count: 5,
                    model_id: Some("model-a".to_owned()),
                    provider_slug: Some("provider-a".to_owned()),
                }),
            }],
            execution_graph: None,
            run_summary: Some(RunSummary {
                campaign_id: "campaign-a".to_owned(),
                node_count: 12,
                generation_count: 4,
                sealed_block_count: 7,
                evaluation_count: 9,
                evaluations_kept: 6,
                evaluations_rejected: 3,
                journal_entry_count: 32,
                terminal_status: Some("complete".to_owned()),
            }),
        };

        let json = serde_json::to_string(&model).expect("serialize browser model");
        let decoded: PlaybackBrowserModel =
            serde_json::from_str(&json).expect("deserialize browser model");

        assert_eq!(decoded, model);
    }

    #[test]
    fn browser_model_deserializes_omitted_optional_projection_fields() {
        let json = r#"{
            "schema_version":"ploke-tree-browser.playback.v1",
            "granularity":"coarse_history",
            "step_count":1,
            "warning_count":0,
            "steps":[{
                "index":0,
                "id":"block-hash",
                "block_height":3,
                "block_hash":"block-hash",
                "considered_candidate_count":0,
                "warning_count":0,
                "evidence":"sealed_history"
            }]
        }"#;

        let decoded: PlaybackBrowserModel =
            serde_json::from_str(json).expect("deserialize browser model");

        assert_eq!(decoded.run_summary, None);
        assert_eq!(decoded.execution_graph, None);
        assert_eq!(decoded.steps[0].fine_kind, None);
        assert_eq!(decoded.steps[0].order, None);
        assert_eq!(decoded.steps[0].evaluation, None);
        assert_eq!(decoded.steps[0].surface, None);
        assert_eq!(decoded.steps[0].protocol, None);
    }

    #[test]
    fn browser_model_builds_execution_graph_from_branch_info() {
        let mut model = PlaybackBrowserModel {
            schema_version: "ploke-tree-browser.playback.v1".to_owned(),
            granularity: BrowserGranularity::FineHistory,
            step_count: 1,
            warning_count: 0,
            steps: vec![PlaybackBrowserStep {
                index: 0,
                id: "block-hash:entry:0:candidate:0".to_owned(),
                fine_kind: Some(FineStepKind::CandidateConsidered),
                order: Some(FineOrder {
                    block_height: 7,
                    phase_rank: 20,
                    entry_index: Some(0),
                    candidate_index: Some(0),
                }),
                block_height: 7,
                block_hash: "block-hash".to_owned(),
                label: Some("candidate:node-abc123:plan_index=0".to_owned()),
                selected_candidate: None,
                considered_candidate_count: 1,
                warning_count: 0,
                evidence: EvidenceStrength::SealedHistory,
                node_id: None,
                branch_id: None,
                candidate_id: None,
                occurrence_id: None,
                membership_id: None,
                candidate_set_root: None,
                evaluation: None,
                surface: None,
                protocol: None,
            }],
            execution_graph: None,
            run_summary: None,
        };
        let base = ArtifactId("artifact:base".to_owned());
        let after = ArtifactId("artifact:after".to_owned());
        let patch = PatchId("patch:attempt".to_owned());
        let child_runtime = RuntimeId("11111111-1111-1111-1111-111111111111".to_owned());
        let successor_runtime = RuntimeId("22222222-2222-2222-2222-222222222222".to_owned());
        let mut branches = BTreeMap::new();
        branches.insert(
            "node-abc123".to_owned(),
            NodeBranchInfo {
                branch_id: "branch-abc123".to_owned(),
                candidate_id: "candidate-abc123".to_owned(),
                target_relpath: "src/lib.rs".to_owned(),
                source_state_id: "source-state".to_owned(),
                source_artifact_id: Some(base.clone()),
                operation_target: Some(OperationTarget::Artifact {
                    artifact_id: base.clone(),
                }),
                generation_coordinate: None,
                source_content_hash: Some("hash-old".to_owned()),
                proposed_content_hash: Some("hash-new".to_owned()),
                patch_id: Some(patch),
                derived_artifact_id: Some(after.clone()),
                child_runtime_id: Some(child_runtime),
                successor_runtime_id: Some(successor_runtime),
            },
        );

        enrich_fine_browser_model(&mut model, &BTreeMap::new(), &branches, None);

        let graph = model.execution_graph.expect("execution graph");
        assert!(graph.nodes.iter().any(|node| node.id == "artifact:base"));
        assert!(graph.nodes.iter().any(|node| node.id == "artifact:after"));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == ExecutionEdgeKind::RuntimeExecutesOperation
                && edge.to == "operation:branch-abc123"
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == ExecutionEdgeKind::RuntimeOperatesOnArtifact && edge.to == "artifact:base"
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == ExecutionEdgeKind::PatchDerivesArtifact && edge.to == "artifact:after"
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == ExecutionEdgeKind::ArtifactHydratesRuntime && edge.from == "artifact:after"
        }));
    }

    #[test]
    #[ignore]
    fn serialize_real_campaign_fine_browser_model_to_json() {
        let run_root = std::env::var("PLOKE_TREE_RUN_ROOT")
            .expect("set PLOKE_TREE_RUN_ROOT to a prototype1 run root");
        let store = crate::FsRunStore::new(run_root);
        let blocks = store.load_history_blocks().expect("load history blocks");
        let model = fine_history_browser_model_from_blocks(&blocks);
        let json = serde_json::to_string_pretty(&model).expect("serialize");
        let lines: Vec<&str> = json.lines().collect();
        let total_lines = lines.len();
        let preview_lines = lines.iter().take(200).copied().collect::<Vec<_>>();
        println!(
            "=== FineHistory BrowserModel JSON (first 200 of {total_lines} lines) ===\n{}\n=== END PREVIEW ===",
            preview_lines.join("\n")
        );
        // Write full JSON to a temp file for inspection
        let out_path = std::path::PathBuf::from("/tmp/ploke-browser-model-fine.json");
        std::fs::write(&out_path, &json).expect("write JSON");
        println!("Full JSON written to {}", out_path.display());
        assert!(total_lines > 0);
    }
}
