//! Renderer-neutral models for browsing typed Ploke run playback.
//!
//! This crate is the front-facing boundary for future UI and WebAssembly work.
//! It does not parse CLI output, read run directories, or decide History
//! authority. Callers provide typed records or `ploke-tree` projections; this
//! crate shapes them for visual browsing.

use ploke_records::branch::Disposition;
use ploke_records::playback::{EvidenceStrength, FineOrder, FineStepKind};
#[cfg(feature = "projection")]
use ploke_records::{evaluation::Artifact as EvaluationArtifact, history::SealedBlockRecord};
#[cfg(feature = "projection")]
use ploke_tree::{
    CoarseHistorySpine, CoarseHistoryWarning, build_coarse_history_spine,
    fine_run_playback_from_sealed_history,
};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub membership_id: Option<String>,
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
    pub source_content_hash: Option<String>,
    pub proposed_content_hash: Option<String>,
    pub patch_id: Option<String>,
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
        run_summary: None,
    }
}

#[cfg(feature = "projection")]
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
            occurrence_id: step.occurrence_id.clone(),
            membership_id: step.membership_id.clone(),
            evaluation: None,
            surface: None,
            protocol: None,
        })
        .collect::<Vec<_>>();

    PlaybackBrowserModel {
        schema_version: "ploke-tree-browser.playback.v1".to_owned(),
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
                    patch_id: info.patch_id.clone(),
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
                selected_successor: SuccessorRefRecord {
                    runtime: ActorRefRecord::Process("successor".to_owned()),
                    artifact: ArtifactRefRecord {
                        value: "artifact:successor".to_owned(),
                    },
                },
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
