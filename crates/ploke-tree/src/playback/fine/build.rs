use ploke_records::history::{EntryPayloadRecord, SealedBlockRecord};
use ploke_records::playback::{EvidenceStrength, FineOrder, FineStepKind};

use super::ids::{fine_candidate_step_id, fine_selected_step_id};
use super::membership::{
    candidate_membership, membership_candidate_set_root, selected_candidate_set_root,
};
use super::types::{FineHistoryStep, FineHistoryStepRef, FineRunPlayback, FineRunPlaybackRefSteps};

const PHASE_CANDIDATE_CONSIDERED: u16 = 40;
const PHASE_SUCCESSOR_SELECTED: u16 = 50;
const PHASE_HISTORY_ENTRY_ADMITTED: u16 = 60;
const PHASE_HISTORY_BLOCK_SEALED: u16 = 70;

/// Build the first fine-grained playback stream from sealed History records.
///
/// This is intentionally History-anchored. Runtime, journal, and invocation
/// phases will join into this stream later, but scheduler records must not
/// define fine playback order.
pub fn fine_run_playback_from_sealed_history(blocks: &[SealedBlockRecord]) -> FineRunPlayback {
    FineRunPlayback::new(fine_history_steps_from_sealed_history(blocks))
}

pub fn fine_history_steps_from_sealed_history(
    blocks: &[SealedBlockRecord],
) -> Vec<FineHistoryStep> {
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

    let mut steps = Vec::new();
    for block in ordered {
        let block_height = block.state.header.common.block_height;
        let block_hash = block.state.header.block_hash.0.as_str();

        for (entry_index, entry) in block.entries.iter().enumerate() {
            if let EntryPayloadRecord::SelectionDecision(selection) = &entry.core.payload {
                for (candidate_index, candidate) in selection.considered.iter().enumerate() {
                    let membership = candidate_membership(selection, candidate);
                    let occurrence_id = membership
                        .and_then(|membership| membership.occurrence_id.as_ref())
                        .map(|id| id.0.clone());
                    let membership_id = membership
                        .and_then(|membership| membership.membership_id.as_ref())
                        .map(|id| id.0.clone());
                    let candidate_set_root =
                        membership_candidate_set_root(selection, membership).map(ToOwned::to_owned);
                    steps.push(FineHistoryStep {
                        id: fine_candidate_step_id(
                            block_height,
                            block_hash,
                            entry_index,
                            candidate_index,
                            candidate_set_root.as_deref(),
                            occurrence_id.as_deref(),
                            membership_id.as_deref(),
                        ),
                        kind: FineStepKind::CandidateConsidered,
                        evidence: EvidenceStrength::AdmittedHistory,
                        order: FineOrder {
                            block_height,
                            phase_rank: PHASE_CANDIDATE_CONSIDERED,
                            entry_index: Some(entry_index),
                            candidate_index: Some(candidate_index),
                        },
                        label: Some(candidate.candidate.value.clone()),
                        occurrence_id,
                        membership_id,
                        candidate_set_root,
                    });
                }

                if let Some(selected) = selection.selected_candidate.as_ref() {
                    let occurrence_id = selection
                        .selected_occurrence_id
                        .as_ref()
                        .map(|id| id.0.clone());
                    let membership_id = selection
                        .selected_membership_id
                        .as_ref()
                        .map(|id| id.0.clone());
                    let candidate_set_root =
                        selected_candidate_set_root(selection).map(ToOwned::to_owned);
                    steps.push(FineHistoryStep {
                        id: fine_selected_step_id(
                            block_height,
                            block_hash,
                            candidate_set_root.as_deref(),
                            occurrence_id.as_deref(),
                            membership_id.as_deref(),
                        ),
                        kind: FineStepKind::SuccessorSelected,
                        evidence: EvidenceStrength::AdmittedHistory,
                        order: FineOrder {
                            block_height,
                            phase_rank: PHASE_SUCCESSOR_SELECTED,
                            entry_index: Some(entry_index),
                            candidate_index: None,
                        },
                        label: Some(selected.value.clone()),
                        occurrence_id,
                        membership_id,
                        candidate_set_root,
                    });
                }
            }

            steps.push(FineHistoryStep {
                id: format!(
                    "history:{block_height}:{block_hash}:entry:{entry_index}:admitted:{}",
                    entry.core.entry_id.0
                ),
                kind: FineStepKind::HistoryEntryAdmitted,
                evidence: EvidenceStrength::AdmittedHistory,
                order: FineOrder {
                    block_height,
                    phase_rank: PHASE_HISTORY_ENTRY_ADMITTED,
                    entry_index: Some(entry_index),
                    candidate_index: None,
                },
                label: Some(entry.core.subject.value.clone()),
                occurrence_id: None,
                membership_id: None,
                candidate_set_root: None,
            });
        }

        steps.push(FineHistoryStep {
            id: block.state.header.block_hash.0.clone(),
            kind: FineStepKind::HistoryBlockSealed,
            evidence: EvidenceStrength::SealedHistory,
            order: FineOrder {
                block_height,
                phase_rank: PHASE_HISTORY_BLOCK_SEALED,
                entry_index: None,
                candidate_index: None,
            },
            label: Some(block.state.header.selected_successor.artifact.value.clone()),
            occurrence_id: None,
            membership_id: None,
            candidate_set_root: None,
        });
    }

    steps
}

/// Build borrowed fine playback steps from sealed History records.
///
/// Step identifiers are intentionally formatted the same way as the owned
/// projection. Labels borrow from the passive records.
pub fn fine_run_playback_ref_steps_from_sealed_history<'a>(
    blocks: &'a [SealedBlockRecord],
) -> FineRunPlaybackRefSteps<'a> {
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

    let mut steps = Vec::new();
    for block in ordered {
        let block_height = block.state.header.common.block_height;
        let block_hash = block.state.header.block_hash.0.as_str();

        for (entry_index, entry) in block.entries.iter().enumerate() {
            if let EntryPayloadRecord::SelectionDecision(selection) = &entry.core.payload {
                for (candidate_index, candidate) in selection.considered.iter().enumerate() {
                    let membership = candidate_membership(selection, candidate);
                    let occurrence_id = membership
                        .and_then(|membership| membership.occurrence_id.as_ref())
                        .map(|id| id.0.as_str());
                    let membership_id = membership
                        .and_then(|membership| membership.membership_id.as_ref())
                        .map(|id| id.0.as_str());
                    let candidate_set_root = membership_candidate_set_root(selection, membership);
                    steps.push(FineHistoryStepRef {
                        id: fine_candidate_step_id(
                            block_height,
                            block_hash,
                            entry_index,
                            candidate_index,
                            candidate_set_root,
                            occurrence_id,
                            membership_id,
                        ),
                        kind: FineStepKind::CandidateConsidered,
                        evidence: EvidenceStrength::AdmittedHistory,
                        order: FineOrder {
                            block_height,
                            phase_rank: PHASE_CANDIDATE_CONSIDERED,
                            entry_index: Some(entry_index),
                            candidate_index: Some(candidate_index),
                        },
                        label: Some(candidate.candidate.value.as_str()),
                        occurrence_id,
                        membership_id,
                        candidate_set_root,
                    });
                }

                if let Some(selected) = selection.selected_candidate.as_ref() {
                    let occurrence_id = selection
                        .selected_occurrence_id
                        .as_ref()
                        .map(|id| id.0.as_str());
                    let membership_id = selection
                        .selected_membership_id
                        .as_ref()
                        .map(|id| id.0.as_str());
                    let candidate_set_root = selected_candidate_set_root(selection);
                    steps.push(FineHistoryStepRef {
                        id: fine_selected_step_id(
                            block_height,
                            block_hash,
                            candidate_set_root,
                            occurrence_id,
                            membership_id,
                        ),
                        kind: FineStepKind::SuccessorSelected,
                        evidence: EvidenceStrength::AdmittedHistory,
                        order: FineOrder {
                            block_height,
                            phase_rank: PHASE_SUCCESSOR_SELECTED,
                            entry_index: Some(entry_index),
                            candidate_index: None,
                        },
                        label: Some(selected.value.as_str()),
                        occurrence_id,
                        membership_id,
                        candidate_set_root,
                    });
                }
            }

            steps.push(FineHistoryStepRef {
                id: format!(
                    "history:{block_height}:{block_hash}:entry:{entry_index}:admitted:{}",
                    entry.core.entry_id.0
                ),
                kind: FineStepKind::HistoryEntryAdmitted,
                evidence: EvidenceStrength::AdmittedHistory,
                order: FineOrder {
                    block_height,
                    phase_rank: PHASE_HISTORY_ENTRY_ADMITTED,
                    entry_index: Some(entry_index),
                    candidate_index: None,
                },
                label: Some(entry.core.subject.value.as_str()),
                occurrence_id: None,
                membership_id: None,
                candidate_set_root: None,
            });
        }

        steps.push(FineHistoryStepRef {
            id: block.state.header.block_hash.0.clone(),
            kind: FineStepKind::HistoryBlockSealed,
            evidence: EvidenceStrength::SealedHistory,
            order: FineOrder {
                block_height,
                phase_rank: PHASE_HISTORY_BLOCK_SEALED,
                entry_index: None,
                candidate_index: None,
            },
            label: Some(
                block
                    .state
                    .header
                    .selected_successor
                    .artifact
                    .value
                    .as_str(),
            ),
            occurrence_id: None,
            membership_id: None,
            candidate_set_root: None,
        });
    }

    FineRunPlaybackRefSteps::new(steps)
}
