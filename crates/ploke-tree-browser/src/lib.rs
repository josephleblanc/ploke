//! Renderer-neutral models for browsing typed Ploke run playback.
//!
//! This crate is the front-facing boundary for future UI and WebAssembly work.
//! It does not parse CLI output, read run directories, or decide History
//! authority. Callers provide typed records or `ploke-tree` projections; this
//! crate shapes them for visual browsing.

use ploke_records::history::SealedBlockRecord;
use ploke_records::playback::{EvidenceStrength, FineOrder, FineStepKind};
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
        })
        .collect::<Vec<_>>();

    PlaybackBrowserModel {
        schema_version: "ploke-tree-browser.playback.v1",
        granularity: BrowserGranularity::CoarseHistory,
        step_count: steps.len(),
        warning_count: spine.warnings.len(),
        steps,
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
        })
        .collect::<Vec<_>>();

    PlaybackBrowserModel {
        schema_version: "ploke-tree-browser.playback.v1",
        granularity: BrowserGranularity::FineHistory,
        step_count: steps.len(),
        warning_count: 0,
        steps,
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
