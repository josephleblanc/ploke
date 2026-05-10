use ploke_records::history::{EntryPayloadRecord, SealedBlockRecord, SuccessorRefRecord};
use ploke_records::playback::{Coarse, CoarseStep, CoarseStepRef, EvidenceStrength, RunPlayback};
use serde::{Deserialize, Serialize};

/// Coarse sealed-History step projected for passive run playback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoarseHistoryStep {
    pub block_height: u64,
    pub block_hash: String,
    pub parent_block_hashes: Vec<String>,
    pub selected_successor: SuccessorRefRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_candidate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_occurrence_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_membership_id: Option<String>,
    pub considered_candidate_count: usize,
}

/// Coarse sealed-History spine with semantic warnings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoarseHistorySpine {
    pub steps: Vec<CoarseHistoryStep>,
    #[serde(default)]
    pub warnings: Vec<CoarseHistoryWarning>,
}

/// Warning emitted while projecting ordered sealed-History blocks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoarseHistoryWarning {
    ParentHashLinkMismatch {
        previous_block_height: u64,
        previous_block_hash: String,
        block_height: u64,
        block_hash: String,
        parent_block_hashes: Vec<String>,
    },
    MissingSelectionDecisionPayload {
        block_height: u64,
        block_hash: String,
    },
}

/// Build a coarse sealed-History spine from already loaded typed records.
pub fn build_coarse_history_spine(blocks: &[SealedBlockRecord]) -> CoarseHistorySpine {
    let mut ordered = blocks.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.state
            .header
            .common
            .block_height
            .cmp(&right.state.header.common.block_height)
            .then_with(|| {
                left.state
                    .header
                    .block_hash
                    .0
                    .cmp(&right.state.header.block_hash.0)
            })
    });

    let mut warnings = Vec::new();
    let mut steps = Vec::with_capacity(ordered.len());

    for (index, block) in ordered.iter().enumerate() {
        let selection = block
            .entries
            .iter()
            .rev()
            .find_map(|entry| match &entry.core.payload {
                EntryPayloadRecord::SelectionDecision(selection) => Some(selection),
                _ => None,
            });

        let block_hash = block.state.header.block_hash.0.clone();
        let parent_block_hashes = block
            .state
            .header
            .common
            .parent_block_hashes
            .iter()
            .map(|hash| hash.0.clone())
            .collect::<Vec<_>>();
        let block_height = block.state.header.common.block_height;

        if index > 0 {
            let previous = ordered[index - 1];
            let previous_block_hash = previous.state.header.block_hash.0.clone();
            if !parent_block_hashes
                .iter()
                .any(|parent_hash| parent_hash == &previous_block_hash)
            {
                warnings.push(CoarseHistoryWarning::ParentHashLinkMismatch {
                    previous_block_height: previous.state.header.common.block_height,
                    previous_block_hash,
                    block_height,
                    block_hash: block_hash.clone(),
                    parent_block_hashes: parent_block_hashes.clone(),
                });
            }
        }

        if selection.is_none() {
            warnings.push(CoarseHistoryWarning::MissingSelectionDecisionPayload {
                block_height,
                block_hash: block_hash.clone(),
            });
        }

        steps.push(CoarseHistoryStep {
            block_height,
            block_hash,
            parent_block_hashes,
            selected_successor: block.state.header.selected_successor.clone(),
            selected_candidate: selection
                .and_then(|selection| selection.selected_candidate.as_ref())
                .map(|subject| subject.value.clone()),
            selected_occurrence_id: selection
                .and_then(|selection| selection.selected_occurrence_id.as_ref())
                .map(|id| id.0.clone()),
            selected_membership_id: selection
                .and_then(|selection| selection.selected_membership_id.as_ref())
                .map(|id| id.0.clone()),
            considered_candidate_count: selection.map_or(0, |selection| selection.considered.len()),
        });
    }

    CoarseHistorySpine { steps, warnings }
}

/// Build a coarse sealed-History spine from already loaded typed records.
pub fn project_coarse_history_spine(blocks: &[SealedBlockRecord]) -> Vec<CoarseHistoryStep> {
    build_coarse_history_spine(blocks).steps
}

/// Build coarse playback ids from the sealed-History spine.
pub fn coarse_run_playback_from_sealed_history(
    blocks: &[SealedBlockRecord],
) -> RunPlayback<Coarse> {
    let steps = project_coarse_history_spine(blocks)
        .into_iter()
        .map(|step| CoarseStep {
            id: step.block_hash,
            evidence: EvidenceStrength::SealedHistory,
        })
        .collect();
    RunPlayback::new(steps)
}

/// Build borrowed coarse playback steps from sealed History blocks.
pub fn coarse_run_playback_ref_steps_from_sealed_history<'a>(
    blocks: &'a [SealedBlockRecord],
) -> Vec<CoarseStepRef<'a>> {
    let mut ordered = blocks.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.state
            .header
            .common
            .block_height
            .cmp(&right.state.header.common.block_height)
            .then_with(|| {
                left.state
                    .header
                    .block_hash
                    .0
                    .cmp(&right.state.header.block_hash.0)
            })
    });

    ordered
        .into_iter()
        .map(|block| CoarseStepRef {
            id: block.state.header.block_hash.0.as_str(),
            evidence: EvidenceStrength::SealedHistory,
        })
        .collect()
}
