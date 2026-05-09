//! Renderer-neutral models for browsing typed Ploke run playback.
//!
//! This crate is the front-facing boundary for future UI and WebAssembly work.
//! It does not parse CLI output, read run directories, or decide History
//! authority. Callers provide typed records or `ploke-tree` projections; this
//! crate shapes them for visual browsing.

use std::collections::BTreeMap;

use ploke_records::branch::{Disposition, Prototype1BranchRegistry};
use ploke_records::evaluation::Artifact as EvaluationArtifact;
use ploke_records::history::SealedBlockRecord;
use ploke_records::playback::{EvidenceStrength, FineOrder, FineStepKind};
use ploke_records::protocol::Artifact as ProtocolArtifact;
use ploke_tree::{
    CoarseHistorySpine, CoarseHistoryWarning, build_coarse_history_spine,
    fine_run_playback_from_sealed_history,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackBrowserModel {
    pub schema_version: &'static str,
    pub granularity: BrowserGranularity,
    pub step_count: usize,
    pub warning_count: usize,
    pub steps: Vec<PlaybackBrowserStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_summary: Option<RunSummary>,
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

pub fn coarse_history_browser_model_from_blocks(
    blocks: &[SealedBlockRecord],
) -> PlaybackBrowserModel {
    coarse_history_browser_model(&build_coarse_history_spine(blocks))
}

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
            evaluation: None,
            surface: None,
            protocol: None,
        })
        .collect::<Vec<_>>();

    PlaybackBrowserModel {
        schema_version: "ploke-tree-browser.playback.v1",
        granularity: BrowserGranularity::CoarseHistory,
        step_count: steps.len(),
        warning_count: spine.warnings.len(),
        steps,
        run_summary: None,
    }
}

pub fn fine_history_browser_model_from_blocks(
    blocks: &[SealedBlockRecord],
) -> PlaybackBrowserModel {
    let playback = fine_run_playback_from_sealed_history(blocks);
    let steps = playback
        .iter()
        .enumerate()
        .map(|(index, step)| PlaybackBrowserStep {
            index,
            id: step.id.clone(),
            fine_kind: Some(step.kind),
            order: Some(step.order),
            block_height: step.order.block_height,
            block_hash: block_hash_from_fine_step(step.id.as_str()).unwrap_or_default(),
            label: step.label.clone(),
            selected_candidate: None,
            considered_candidate_count: 0,
            warning_count: 0,
            evidence: step.evidence,
            node_id: None,
            branch_id: None,
            candidate_id: None,
            evaluation: None,
            surface: None,
            protocol: None,
        })
        .collect::<Vec<_>>();

    PlaybackBrowserModel {
        schema_version: "ploke-tree-browser.playback.v1",
        granularity: BrowserGranularity::FineHistory,
        step_count: steps.len(),
        warning_count: 0,
        steps,
        run_summary: None,
    }
}

/// Enrich a fine-history browser model with evaluation, surface, and protocol
/// detail by joining against loaded records.
///
/// `evaluations` is keyed by `branch_id`.
/// `protocol_artifacts` is keyed by a run-scoped artifact path.
/// `branch_registry` provides the node→branch→candidate join chain.
pub fn enrich_fine_browser_model(
    model: &mut PlaybackBrowserModel,
    evaluations: &BTreeMap<String, EvaluationArtifact>,
    branch_registry: Option<&Prototype1BranchRegistry>,
    protocol_artifacts: Option<&BTreeMap<String, ProtocolArtifact>>,
) {
    // Build node_id → source_node lookup from branch registry.
    let source_by_instance: BTreeMap<&str, &ploke_records::branch::InterventionSourceNode> =
        if let Some(registry) = branch_registry {
            registry
                .source_nodes
                .iter()
                .map(|source| (source.instance_id.as_str(), source))
                .collect()
        } else {
            BTreeMap::new()
        };

    // Build branch_id → branch_node lookup.
    let branch_by_id: BTreeMap<&str, &ploke_records::branch::TreatmentBranchNode> =
        if let Some(registry) = branch_registry {
            registry
                .source_nodes
                .iter()
                .flat_map(|source| source.branches.iter())
                .map(|branch| (branch.branch_id.as_str(), branch))
                .collect()
        } else {
            BTreeMap::new()
        };

    // Build node_id → protocol artifact counts.
    let protocol_by_node: BTreeMap<&str, ProtocolCounts> =
        if let Some(artifacts) = protocol_artifacts {
            artifacts
                .values()
                .fold(BTreeMap::new(), |mut acc, artifact| {
                    let entry = acc.entry(artifact.subject_id.as_str()).or_default();
                    match artifact.procedure_name.as_str() {
                        "tool_call_intent_segmentation" => entry.intent_segmentation_count += 1,
                        "tool_call_review" => entry.tool_call_review_count += 1,
                        "tool_call_segment_review" => entry.segment_review_count += 1,
                        "intervention_issue_detection" => entry.issue_detection_count += 1,
                        "intervention_synthesis" => entry.synthesis_count += 1,
                        _ => {}
                    }
                    if entry.model_id.is_none() {
                        entry.model_id = artifact.model_id.clone();
                        entry.provider_slug = artifact.provider_slug.clone();
                    }
                    acc
                })
        } else {
            BTreeMap::new()
        };

    for step in &mut model.steps {
        // Extract node_id from candidate label: "candidate:node-NODEID:plan_index=N"
        let node_id = extract_node_id_from_label(step.label.as_deref());
        step.node_id = node_id.clone();

        // Join through branch registry to find branch_id and surface evidence.
        if let Some(node_id) = &node_id {
            // Look up source node by instance_id (which may match node_id pattern).
            // The branch registry keys by instance_id; we try node_id as instance_id.
            if let Some(source) = source_by_instance.get(node_id.as_str()) {
                if let Some(selected) = &source.selected_branch_id {
                    step.branch_id = Some(selected.clone());
                    if let Some(branch) = branch_by_id.get(selected.as_str()) {
                        step.candidate_id = Some(branch.candidate_id.clone());
                        step.surface = Some(SurfaceSnapshot {
                            target_relpath: source.target_relpath.to_string_lossy().into_owned(),
                            patch_id: branch.patch_id.as_ref().map(|p| p.0.clone()),
                            source_content_hash: Some(source.source_content_hash.clone()),
                            proposed_content_hash: Some(branch.proposed_content_hash.clone()),
                            source_state_id: Some(source.source_state_id.clone()),
                        });
                    }
                }
            }

            // Attach protocol snapshot.
            if let Some(counts) = protocol_by_node.get(node_id.as_str()) {
                step.protocol = Some(ProtocolSnapshot {
                    intent_segmentation_count: counts.intent_segmentation_count,
                    tool_call_review_count: counts.tool_call_review_count,
                    segment_review_count: counts.segment_review_count,
                    issue_detection_count: counts.issue_detection_count,
                    synthesis_count: counts.synthesis_count,
                    model_id: counts.model_id.clone(),
                    provider_slug: counts.provider_slug.clone(),
                });
            }
        }

        // Attach evaluation snapshot by branch_id.
        if let Some(branch_id) = &step.branch_id {
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
}

#[derive(Debug, Default)]
struct ProtocolCounts {
    intent_segmentation_count: usize,
    tool_call_review_count: usize,
    segment_review_count: usize,
    issue_detection_count: usize,
    synthesis_count: usize,
    model_id: Option<String>,
    provider_slug: Option<String>,
}

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
pub fn build_run_summary(
    campaign_id: String,
    node_count: usize,
    max_generation: u64,
    sealed_block_count: usize,
    evaluation_summary: Option<&ploke_tree::EvaluationArtifactSummary>,
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

fn block_hash_from_fine_step(id: &str) -> Option<String> {
    if id.contains(":entry:") || id.contains(":successor-selected") {
        return id.split(':').nth(2).map(ToOwned::to_owned);
    }
    Some(id.to_owned())
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_records::history::{ActorRefRecord, ArtifactRefRecord, SuccessorRefRecord};
    use ploke_tree::CoarseHistoryStep;

    #[test]
    fn browser_model_projects_coarse_history_steps() {
        let spine = CoarseHistorySpine {
            steps: vec![CoarseHistoryStep {
                block_height: 3,
                block_hash: "hash-3".to_owned(),
                parent_block_hashes: vec!["hash-2".to_owned()],
                selected_successor: SuccessorRefRecord {
                    runtime: ActorRefRecord::Process("successor".to_owned()),
                    artifact: ArtifactRefRecord {
                        value: "artifact:successor".to_owned(),
                    },
                },
                selected_candidate: Some("candidate:a".to_owned()),
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
    }

    #[test]
    fn browser_model_leaves_fine_fields_empty_for_coarse_steps() {
        let spine = CoarseHistorySpine {
            steps: vec![CoarseHistoryStep {
                block_height: 3,
                block_hash: "hash-3".to_owned(),
                parent_block_hashes: vec!["hash-2".to_owned()],
                selected_successor: SuccessorRefRecord {
                    runtime: ActorRefRecord::Process("successor".to_owned()),
                    artifact: ArtifactRefRecord {
                        value: "artifact:successor".to_owned(),
                    },
                },
                selected_candidate: None,
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
    #[ignore]
    fn serialize_real_campaign_fine_browser_model_to_json() {
        let run_root = std::env::var("PLOKE_TREE_RUN_ROOT")
            .expect("set PLOKE_TREE_RUN_ROOT to a prototype1 run root");
        let store = ploke_tree::FsRunStore::new(run_root);
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
