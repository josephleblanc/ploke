use ploke_records::history::{EntryPayloadRecord, SealedBlockRecord};
use ploke_records::playback::{
    EvidenceStrength, Fine, FineOrder, FineStep, FineStepKind, FineStepRef, RunPlayback,
};

const PHASE_CANDIDATE_CONSIDERED: u16 = 40;
const PHASE_SUCCESSOR_SELECTED: u16 = 50;
const PHASE_HISTORY_ENTRY_ADMITTED: u16 = 60;
const PHASE_HISTORY_BLOCK_SEALED: u16 = 70;

/// Build the first fine-grained playback stream from sealed History records.
///
/// This is intentionally History-anchored. Runtime, journal, and invocation
/// phases will join into this stream later, but scheduler records must not
/// define fine playback order.
pub fn fine_run_playback_from_sealed_history(blocks: &[SealedBlockRecord]) -> RunPlayback<Fine> {
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
                    steps.push(FineStep {
                        id: format!(
                            "history:{block_height}:{block_hash}:entry:{entry_index}:candidate:{candidate_index}"
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
                    });
                }

                if let Some(selected) = selection.selected_candidate.as_ref() {
                    steps.push(FineStep {
                        id: format!("history:{block_height}:{block_hash}:successor-selected"),
                        kind: FineStepKind::SuccessorSelected,
                        evidence: EvidenceStrength::AdmittedHistory,
                        order: FineOrder {
                            block_height,
                            phase_rank: PHASE_SUCCESSOR_SELECTED,
                            entry_index: Some(entry_index),
                            candidate_index: None,
                        },
                        label: Some(selected.value.clone()),
                    });
                }
            }

            steps.push(FineStep {
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
            });
        }

        steps.push(FineStep {
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
        });
    }

    RunPlayback::new(steps)
}

/// Build borrowed fine playback steps from sealed History records.
///
/// Step identifiers are intentionally formatted the same way as the owned
/// projection. Labels borrow from the passive records.
pub fn fine_run_playback_ref_steps_from_sealed_history<'a>(
    blocks: &'a [SealedBlockRecord],
) -> Vec<FineStepRef<'a>> {
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
                    steps.push(FineStepRef {
                        id: format!(
                            "history:{block_height}:{block_hash}:entry:{entry_index}:candidate:{candidate_index}"
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
                    });
                }

                if let Some(selected) = selection.selected_candidate.as_ref() {
                    steps.push(FineStepRef {
                        id: format!("history:{block_height}:{block_hash}:successor-selected"),
                        kind: FineStepKind::SuccessorSelected,
                        evidence: EvidenceStrength::AdmittedHistory,
                        order: FineOrder {
                            block_height,
                            phase_rank: PHASE_SUCCESSOR_SELECTED,
                            entry_index: Some(entry_index),
                            candidate_index: None,
                        },
                        label: Some(selected.value.as_str()),
                    });
                }
            }

            steps.push(FineStepRef {
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
            });
        }

        steps.push(FineStepRef {
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
        });
    }

    steps
}
