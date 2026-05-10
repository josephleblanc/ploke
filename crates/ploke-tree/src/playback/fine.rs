use ploke_records::history::{
    CandidateSetMembershipRecord, EntryPayloadRecord, SealedBlockRecord,
    SelectionDecisionEntryRecord,
};
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
                    let membership = candidate_membership(selection, candidate_index);
                    let occurrence_id = membership
                        .and_then(|membership| membership.occurrence_id.as_ref())
                        .map(|id| id.0.clone());
                    let membership_id = membership
                        .and_then(|membership| membership.membership_id.as_ref())
                        .map(|id| id.0.clone());
                    steps.push(FineStep {
                        id: fine_candidate_step_id(
                            block_height,
                            block_hash,
                            entry_index,
                            candidate_index,
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
                    steps.push(FineStep {
                        id: fine_selected_step_id(
                            block_height,
                            block_hash,
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
                occurrence_id: None,
                membership_id: None,
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
            occurrence_id: None,
            membership_id: None,
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
                    let membership = candidate_membership(selection, candidate_index);
                    let occurrence_id = membership
                        .and_then(|membership| membership.occurrence_id.as_ref())
                        .map(|id| id.0.as_str());
                    let membership_id = membership
                        .and_then(|membership| membership.membership_id.as_ref())
                        .map(|id| id.0.as_str());
                    steps.push(FineStepRef {
                        id: fine_candidate_step_id(
                            block_height,
                            block_hash,
                            entry_index,
                            candidate_index,
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
                    steps.push(FineStepRef {
                        id: fine_selected_step_id(
                            block_height,
                            block_hash,
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
                occurrence_id: None,
                membership_id: None,
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
            occurrence_id: None,
            membership_id: None,
        });
    }

    steps
}

fn candidate_membership(
    selection: &SelectionDecisionEntryRecord,
    candidate_index: usize,
) -> Option<&CandidateSetMembershipRecord> {
    selection
        .candidate_set
        .as_ref()
        .and_then(|set| set.memberships.get(candidate_index))
}

fn fine_candidate_step_id(
    block_height: u64,
    block_hash: &str,
    entry_index: usize,
    candidate_index: usize,
    occurrence_id: Option<&str>,
    membership_id: Option<&str>,
) -> String {
    if let Some(membership_id) = membership_id {
        return format!(
            "history:{block_height}:{block_hash}:entry:{entry_index}:candidate-membership:{membership_id}"
        );
    }
    if let Some(occurrence_id) = occurrence_id {
        return format!(
            "history:{block_height}:{block_hash}:entry:{entry_index}:candidate-occurrence:{occurrence_id}"
        );
    }
    format!("history:{block_height}:{block_hash}:entry:{entry_index}:candidate:{candidate_index}")
}

fn fine_selected_step_id(
    block_height: u64,
    block_hash: &str,
    occurrence_id: Option<&str>,
    membership_id: Option<&str>,
) -> String {
    if let Some(membership_id) = membership_id {
        return format!("history:{block_height}:{block_hash}:successor-selected:{membership_id}");
    }
    if let Some(occurrence_id) = occurrence_id {
        return format!("history:{block_height}:{block_hash}:successor-selected:{occurrence_id}");
    }
    format!("history:{block_height}:{block_hash}:successor-selected")
}
