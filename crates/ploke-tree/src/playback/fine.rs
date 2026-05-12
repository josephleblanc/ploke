use ploke_records::history::{
    CandidateSetMembershipRecord, EntryPayloadRecord, EvaluationPayloadRecord, SealedBlockRecord,
    SelectionDecisionEntryRecord,
};
use ploke_records::playback::{EvidenceStrength, FineOrder, FineStepKind, FineStepRef};

const PHASE_CANDIDATE_CONSIDERED: u16 = 40;
const PHASE_SUCCESSOR_SELECTED: u16 = 50;
const PHASE_HISTORY_ENTRY_ADMITTED: u16 = 60;
const PHASE_HISTORY_BLOCK_SEALED: u16 = 70;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FineHistoryStep {
    pub id: String,
    pub kind: FineStepKind,
    pub evidence: EvidenceStrength,
    pub order: FineOrder,
    pub label: Option<String>,
    pub occurrence_id: Option<String>,
    pub membership_id: Option<String>,
    pub candidate_set_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FineRunPlayback {
    steps: Vec<FineHistoryStep>,
}

impl FineRunPlayback {
    pub fn new(steps: Vec<FineHistoryStep>) -> Self {
        Self { steps }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, FineHistoryStep> {
        self.steps.iter()
    }
}

impl IntoIterator for FineRunPlayback {
    type Item = FineHistoryStep;
    type IntoIter = std::vec::IntoIter<FineHistoryStep>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.into_iter()
    }
}

impl<'a> IntoIterator for &'a FineRunPlayback {
    type Item = &'a FineHistoryStep;
    type IntoIter = std::slice::Iter<'a, FineHistoryStep>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FineHistoryStepRef<'a> {
    pub id: String,
    pub kind: FineStepKind,
    pub evidence: EvidenceStrength,
    pub order: FineOrder,
    pub label: Option<&'a str>,
    pub occurrence_id: Option<&'a str>,
    pub membership_id: Option<&'a str>,
    pub candidate_set_root: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FineRunPlaybackRefSteps<'a> {
    steps: Vec<FineHistoryStepRef<'a>>,
    legacy_steps: Vec<FineStepRef<'a>>,
}

impl<'a> FineRunPlaybackRefSteps<'a> {
    pub fn new(steps: Vec<FineHistoryStepRef<'a>>) -> Self {
        let legacy_steps = steps
            .iter()
            .map(|step| FineStepRef {
                id: step.id.clone(),
                kind: step.kind,
                evidence: step.evidence,
                order: step.order,
                label: step.label,
                occurrence_id: step.occurrence_id,
                membership_id: step.membership_id,
            })
            .collect();
        Self {
            steps,
            legacy_steps,
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, FineHistoryStepRef<'a>> {
        self.steps.iter()
    }

    pub fn as_slice(&self) -> &[FineStepRef<'a>] {
        self.legacy_steps.as_slice()
    }
}

impl<'a> IntoIterator for FineRunPlaybackRefSteps<'a> {
    type Item = FineHistoryStepRef<'a>;
    type IntoIter = std::vec::IntoIter<FineHistoryStepRef<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.into_iter()
    }
}

impl<'a, 'b> IntoIterator for &'b FineRunPlaybackRefSteps<'a> {
    type Item = &'b FineHistoryStepRef<'a>;
    type IntoIter = std::slice::Iter<'b, FineHistoryStepRef<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.iter()
    }
}

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

fn candidate_membership<'a>(
    selection: &'a SelectionDecisionEntryRecord,
    candidate: &EvaluationPayloadRecord,
) -> Option<&'a CandidateSetMembershipRecord> {
    let memberships = selection
        .candidate_set
        .as_ref()
        .map(|set| set.memberships.as_slice())?;

    if selected_applies_to(selection, candidate) {
        if let Some(selected_membership_id) = selection.selected_membership_id.as_ref()
            && let Some(membership) = unique_membership(memberships.iter().filter(|membership| {
                membership.membership_id.as_ref() == Some(selected_membership_id)
                    && membership.candidate == candidate.candidate
            }))
        {
            return Some(membership);
        }

        if let Some(selected_occurrence_id) = selection.selected_occurrence_id.as_ref()
            && let Some(membership) = unique_membership(memberships.iter().filter(|membership| {
                membership.occurrence_id.as_ref() == Some(selected_occurrence_id)
                    && membership.candidate == candidate.candidate
            }))
        {
            return Some(membership);
        }
    }

    if !candidate_label_is_unique(selection, candidate) {
        return None;
    }

    unique_membership(
        memberships
            .iter()
            .filter(|membership| membership.candidate == candidate.candidate),
    )
}

fn selected_applies_to(
    selection: &SelectionDecisionEntryRecord,
    candidate: &EvaluationPayloadRecord,
) -> bool {
    if candidate
        .sealed_evidence
        .as_ref()
        .is_some_and(|evidence| evidence.coordinate.node_id == selection.decision.candidate_node_id)
    {
        return true;
    }

    candidate
        .selection_input
        .as_ref()
        .is_some_and(|input| input.candidate.node_id == selection.decision.candidate_node_id)
}

fn candidate_label_is_unique(
    selection: &SelectionDecisionEntryRecord,
    candidate: &EvaluationPayloadRecord,
) -> bool {
    selection
        .considered
        .iter()
        .filter(|payload| payload.candidate == candidate.candidate)
        .take(2)
        .count()
        == 1
}

fn unique_membership<'a>(
    mut memberships: impl Iterator<Item = &'a CandidateSetMembershipRecord>,
) -> Option<&'a CandidateSetMembershipRecord> {
    let first = memberships.next()?;
    if memberships.next().is_some() {
        return None;
    }
    Some(first)
}

fn membership_candidate_set_root<'a>(
    selection: &'a SelectionDecisionEntryRecord,
    membership: Option<&CandidateSetMembershipRecord>,
) -> Option<&'a str> {
    membership?;
    selection
        .candidate_set
        .as_ref()
        .map(|candidate_set| candidate_set.root.0.0.as_str())
}

fn selected_candidate_set_root(selection: &SelectionDecisionEntryRecord) -> Option<&str> {
    let selected_membership_id = selection.selected_membership_id.as_ref()?;
    let candidate_set = selection.candidate_set.as_ref()?;
    candidate_set
        .memberships
        .iter()
        .any(|membership| membership.membership_id.as_ref() == Some(selected_membership_id))
        .then_some(candidate_set.root.0.0.as_str())
}

fn fine_candidate_step_id(
    block_height: u64,
    block_hash: &str,
    entry_index: usize,
    candidate_index: usize,
    candidate_set_root: Option<&str>,
    occurrence_id: Option<&str>,
    membership_id: Option<&str>,
) -> String {
    if let Some(membership_id) = membership_id {
        if let Some(candidate_set_root) = candidate_set_root {
            return format!(
                "history:{block_height}:{block_hash}:entry:{entry_index}:candidate-membership:{}:{}",
                step_id_component(candidate_set_root),
                step_id_component(membership_id),
            );
        }
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
    candidate_set_root: Option<&str>,
    occurrence_id: Option<&str>,
    membership_id: Option<&str>,
) -> String {
    if let Some(membership_id) = membership_id {
        if let Some(candidate_set_root) = candidate_set_root {
            return format!(
                "history:{block_height}:{block_hash}:successor-selected:{}:{}",
                step_id_component(candidate_set_root),
                step_id_component(membership_id),
            );
        }
        return format!("history:{block_height}:{block_hash}:successor-selected:{membership_id}");
    }
    if let Some(occurrence_id) = occurrence_id {
        return format!("history:{block_height}:{block_hash}:successor-selected:{occurrence_id}");
    }
    format!("history:{block_height}:{block_hash}:successor-selected")
}

fn step_id_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '%' => encoded.push_str("%25"),
            ':' => encoded.push_str("%3A"),
            _ => encoded.push(ch),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use ploke_records::history::{
        ActorRefRecord, AdmittedEntryRecord, AdmittedEntryStateRecord, ArtifactRefRecord,
        BlockCommonRecord, CandidateSetMembershipRecord, CandidateSetProofRecord,
        CandidateSetRecord, CandidateSetRootRecord, ClaimsRecord, EntryCoreRecord, EntryKindRecord,
        EntryPayloadRecord, EvaluationPayloadRecord, EvidenceRefRecord, GenesisAuthorityRecord,
        LevelRecord, OpeningAuthorityRecord, OperationalEnvironmentRecord, ParentIdentityRefRecord,
        PhaseRecord, ProcedureRefRecord, RegimeRecord, RiskRecord, SealedBlockHeaderRecord,
        SealedBlockRecord, SealedBlockStateRecord, SelectionDecisionEntryRecord,
        SelectionScopeRecord, StepRecord, SubjectRefRecord, SuccessorRefRecord,
        SurfaceCommitmentRecord, SurfaceDeltaRecord, SurfaceRecord, SurfaceRootRecord,
        TreeKeyHashRecord,
    };
    use ploke_records::ids::{
        BlockHash, BlockId, CandidateMembershipId, CandidateOccurrenceId, EntryId, HistoryHash,
        HistoryStateRoot, LineageId, RecordedAt, RuntimeId,
    };
    use ploke_records::playback::FineStepKind;
    use ploke_records::selection::{Decision, Outcome};

    use super::{
        candidate_membership, fine_candidate_step_id, fine_run_playback_from_sealed_history,
        fine_run_playback_ref_steps_from_sealed_history,
    };

    #[test]
    fn candidate_membership_matches_recorded_candidate_not_vector_index() {
        let selected_membership = CandidateMembershipId("membership-a".to_owned());
        let selection = selection(
            vec![payload("candidate:a")],
            vec![
                membership(
                    "candidate:b",
                    Some(CandidateMembershipId("membership-b".to_owned())),
                ),
                membership("candidate:a", Some(selected_membership.clone())),
            ],
        );

        let resolved = candidate_membership(&selection, &selection.considered[0])
            .expect("candidate membership should resolve by candidate identity");

        assert_eq!(resolved.candidate.value, "candidate:a");
        assert_eq!(resolved.membership_id.as_ref(), Some(&selected_membership));
    }

    #[test]
    fn candidate_membership_is_absent_for_ambiguous_candidate_identity() {
        let selection = selection(
            vec![payload("candidate:a")],
            vec![
                membership(
                    "candidate:a",
                    Some(CandidateMembershipId("membership-a".to_owned())),
                ),
                membership(
                    "candidate:a",
                    Some(CandidateMembershipId("membership-b".to_owned())),
                ),
            ],
        );

        assert!(candidate_membership(&selection, &selection.considered[0]).is_none());
    }

    #[test]
    fn selected_membership_is_absent_for_duplicate_bare_candidate_labels() {
        let selected_membership = CandidateMembershipId("membership-selected".to_owned());
        let mut selection = selection(
            vec![payload("candidate:dup"), payload("candidate:dup")],
            vec![
                membership_with_payload_hash(
                    "candidate:dup",
                    Some(selected_membership.clone()),
                    "payload:first",
                ),
                membership_with_payload_hash(
                    "candidate:dup",
                    Some(CandidateMembershipId("membership-other".to_owned())),
                    "payload:second",
                ),
            ],
        );
        selection.selected_candidate = Some(SubjectRefRecord {
            value: "candidate:dup".to_owned(),
        });
        selection.selected_membership_id = Some(selected_membership);

        assert!(candidate_membership(&selection, &selection.considered[0]).is_none());
        assert!(candidate_membership(&selection, &selection.considered[1]).is_none());
    }

    #[test]
    fn selected_membership_is_absent_for_label_only_candidate_without_coordinate() {
        let selected_membership = CandidateMembershipId("membership-selected".to_owned());
        let mut selection = selection(
            vec![payload("candidate:a")],
            vec![
                membership_with_payload_hash(
                    "candidate:a",
                    Some(selected_membership.clone()),
                    "payload:selected",
                ),
                membership_with_payload_hash(
                    "candidate:a",
                    Some(CandidateMembershipId("membership-other".to_owned())),
                    "payload:other",
                ),
            ],
        );
        selection.selected_candidate = Some(SubjectRefRecord {
            value: "candidate:a".to_owned(),
        });
        selection.selected_membership_id = Some(selected_membership);

        assert!(candidate_membership(&selection, &selection.considered[0]).is_none());
    }

    #[test]
    fn fine_candidate_membership_step_id_is_set_scoped() {
        let first = fine_candidate_step_id(
            1,
            "block",
            0,
            0,
            Some("root:first"),
            None,
            Some("membership:shared"),
        );
        let second = fine_candidate_step_id(
            1,
            "block",
            0,
            0,
            Some("root:second"),
            None,
            Some("membership:shared"),
        );

        assert_ne!(first, second);
        assert!(first.contains("candidate-membership:root%3Afirst:membership%3Ashared"));
        assert!(second.contains("candidate-membership:root%3Asecond:membership%3Ashared"));
    }

    #[test]
    fn fine_run_playback_preserves_candidate_set_root() {
        let membership_id = CandidateMembershipId("membership-a".to_owned());
        let mut selection = selection(
            vec![payload("candidate:a")],
            vec![membership("candidate:a", Some(membership_id.clone()))],
        );
        selection.selected_candidate = Some(SubjectRefRecord {
            value: "candidate:a".to_owned(),
        });
        selection.selected_membership_id = Some(membership_id);
        let block = sealed_block(selection);

        let playback = fine_run_playback_from_sealed_history(std::slice::from_ref(&block));
        let candidate = playback
            .iter()
            .find(|step| step.kind == FineStepKind::CandidateConsidered)
            .expect("candidate step");
        assert_eq!(
            candidate.candidate_set_root.as_deref(),
            Some("candidate-set-root")
        );

        let selected = playback
            .iter()
            .find(|step| step.kind == FineStepKind::SuccessorSelected)
            .expect("selected step");
        assert_eq!(
            selected.candidate_set_root.as_deref(),
            Some("candidate-set-root")
        );
    }

    #[test]
    fn fine_ref_playback_preserves_candidate_set_root() {
        let membership_id = CandidateMembershipId("membership-a".to_owned());
        let mut selection = selection(
            vec![payload("candidate:a")],
            vec![membership("candidate:a", Some(membership_id.clone()))],
        );
        selection.selected_candidate = Some(SubjectRefRecord {
            value: "candidate:a".to_owned(),
        });
        selection.selected_membership_id = Some(membership_id);
        let blocks = [sealed_block(selection)];

        let playback = fine_run_playback_ref_steps_from_sealed_history(&blocks);
        let candidate = playback
            .iter()
            .find(|step| step.kind == FineStepKind::CandidateConsidered)
            .expect("candidate step");
        assert_eq!(candidate.candidate_set_root, Some("candidate-set-root"));

        let selected = playback
            .iter()
            .find(|step| step.kind == FineStepKind::SuccessorSelected)
            .expect("selected step");
        assert_eq!(selected.candidate_set_root, Some("candidate-set-root"));
        assert_eq!(playback.as_slice().len(), playback.iter().count());
    }

    fn selection(
        considered: Vec<EvaluationPayloadRecord>,
        memberships: Vec<CandidateSetMembershipRecord>,
    ) -> SelectionDecisionEntryRecord {
        SelectionDecisionEntryRecord {
            schema_version: 1,
            procedure_or_policy: ProcedureRefRecord {
                value: "prototype1.successor_selection.v1".to_owned(),
            },
            scope: SelectionScopeRecord {
                value: "test".to_owned(),
            },
            selected_candidate: None,
            selected_occurrence_id: None,
            selected_membership_id: None,
            considered,
            considered_sources: Vec::new(),
            considered_order_hash: HistoryHash("order-hash".to_owned()),
            candidate_set: Some(CandidateSetRecord {
                root: CandidateSetRootRecord(HistoryHash("candidate-set-root".to_owned())),
                memberships,
            }),
            projection_failures: Vec::new(),
            traversal: None,
            decision: Decision {
                procedure_id: "prototype1.successor_selection.v1".to_owned(),
                candidate_node_id: "node-a".to_owned(),
                selected_branch_id: None,
                branch_disposition: "keep".to_owned(),
                outcome: Outcome::Accepted,
                findings: Vec::new(),
                rationale: Vec::new(),
            },
        }
    }

    fn payload(candidate: &str) -> EvaluationPayloadRecord {
        EvaluationPayloadRecord {
            schema_version: 1,
            candidate: SubjectRefRecord {
                value: candidate.to_owned(),
            },
            procedure: ProcedureRefRecord {
                value: "prototype1.successor_selection.v1".to_owned(),
            },
            selection_input: None,
            selection_input_hash: None,
            projection_failures: Vec::new(),
            source_refs: Vec::new(),
            source_hashes: Vec::new(),
            sealed_evidence: None,
            artifact: None,
            surface_attempt: None,
        }
    }

    fn membership(
        candidate: &str,
        membership_id: Option<CandidateMembershipId>,
    ) -> CandidateSetMembershipRecord {
        membership_with_payload_hash(candidate, membership_id, &format!("{candidate}:payload"))
    }

    fn membership_with_payload_hash(
        candidate: &str,
        membership_id: Option<CandidateMembershipId>,
        payload_hash: &str,
    ) -> CandidateSetMembershipRecord {
        CandidateSetMembershipRecord {
            candidate: SubjectRefRecord {
                value: candidate.to_owned(),
            },
            payload_hash: HistoryHash(payload_hash.to_owned()),
            occurrence_id: Some(CandidateOccurrenceId(format!("{payload_hash}:occurrence"))),
            membership_id,
            proof: CandidateSetProofRecord {
                key: [0; 32],
                value: [1; 32],
                program: Vec::new(),
            },
        }
    }

    fn sealed_block(selection: SelectionDecisionEntryRecord) -> SealedBlockRecord {
        let runtime = ActorRefRecord::Runtime(RuntimeId("runtime:test".to_owned()));
        let artifact = ArtifactRefRecord {
            value: "artifact:test".to_owned(),
        };
        let entry = AdmittedEntryRecord {
            core: EntryCoreRecord {
                entry_id: EntryId("entry:test".to_owned()),
                entry_kind: EntryKindRecord::Decision,
                subject: SubjectRefRecord {
                    value: "subject:test".to_owned(),
                },
                executor: runtime.clone(),
                input_refs: Vec::new(),
                output_refs: Vec::new(),
                occurred_at: RecordedAt(100),
                payload: EntryPayloadRecord::SelectionDecision(selection),
            },
            state: AdmittedEntryStateRecord {
                observed: ploke_records::history::ObservedEntryRecord {
                    observer: runtime.clone(),
                    recorder: runtime.clone(),
                    operational_environment: OperationalEnvironmentRecord {
                        runtime: None,
                        artifact: None,
                        binary: None,
                        tool_surface: None,
                        procedure_version: None,
                        model: None,
                        code_graph: None,
                        oracle_task: None,
                        recorder: None,
                    },
                    payload_ref: EvidenceRefRecord {
                        value: "payload:test".to_owned(),
                    },
                    payload_hash: HistoryHash("payload:test".to_owned()),
                    observed_at: RecordedAt(100),
                    recorded_at: RecordedAt(100),
                },
                proposer: runtime.clone(),
                procedure_or_policy: ProcedureRefRecord {
                    value: "policy:selection".to_owned(),
                },
                admitting_authority: runtime.clone(),
                ruling_authority: runtime.clone(),
                lineage_id: LineageId("lineage:test".to_owned()),
                block_id: BlockId("block:test".to_owned()),
                block_height: 1,
            },
        };

        SealedBlockRecord {
            state: SealedBlockStateRecord {
                header: SealedBlockHeaderRecord {
                    common: BlockCommonRecord {
                        schema_version: 1,
                        block_id: BlockId("block:test".to_owned()),
                        lineage_id: LineageId("lineage:test".to_owned()),
                        block_height: 1,
                        parent_block_hashes: Vec::new(),
                        opened_from_state: HistoryStateRoot("state:test".to_owned()),
                        regime: RegimeRecord {
                            step: StepRecord(1),
                            phase: PhaseRecord::Consolidation,
                            risk: RiskRecord {
                                exploration: LevelRecord::Medium,
                                mutation: LevelRecord::Medium,
                                finality: LevelRecord::Medium,
                            },
                        },
                        opening_authority: OpeningAuthorityRecord::Genesis(
                            GenesisAuthorityRecord {
                                bootstrap_policy: ProcedureRefRecord {
                                    value: "policy:bootstrap".to_owned(),
                                },
                                tree_key: TreeKeyHashRecord {
                                    hash: HistoryHash("tree:test".to_owned()),
                                },
                                parent_identity: ParentIdentityRefRecord {
                                    evidence: EvidenceRefRecord {
                                        value: "parent:test".to_owned(),
                                    },
                                },
                            },
                        ),
                        opened_by: runtime.clone(),
                        opened_from_artifact: artifact.clone(),
                        ruling_authority: runtime.clone(),
                        policy_ref: ProcedureRefRecord {
                            value: "policy:selection".to_owned(),
                        },
                        surface: SurfaceCommitmentRecord {
                            immutable: SurfaceRecord {
                                root: SurfaceRootRecord {
                                    hash: HistoryHash("immutable:test".to_owned()),
                                },
                            },
                            mutated: SurfaceDeltaRecord {
                                before: SurfaceRecord {
                                    root: SurfaceRootRecord {
                                        hash: HistoryHash("mutated-before:test".to_owned()),
                                    },
                                },
                                after: SurfaceRecord {
                                    root: SurfaceRootRecord {
                                        hash: HistoryHash("mutated-after:test".to_owned()),
                                    },
                                },
                            },
                            ambient: SurfaceDeltaRecord {
                                before: SurfaceRecord {
                                    root: SurfaceRootRecord {
                                        hash: HistoryHash("ambient-before:test".to_owned()),
                                    },
                                },
                                after: SurfaceRecord {
                                    root: SurfaceRootRecord {
                                        hash: HistoryHash("ambient-after:test".to_owned()),
                                    },
                                },
                            },
                        },
                        opened_at: RecordedAt(90),
                    },
                    crown_lock_transition: EvidenceRefRecord {
                        value: "evidence:crown-lock".to_owned(),
                    },
                    selected_successor: SuccessorRefRecord {
                        runtime,
                        artifact: artifact.clone(),
                    },
                    active_artifact: artifact,
                    claims: ClaimsRecord {
                        policy: None,
                        surface: None,
                        manifest: None,
                        artifact: None,
                    },
                    sealed_at: RecordedAt(110),
                    entry_count: 1,
                    entries_root: HistoryHash("entries:test".to_owned()),
                    block_hash: BlockHash("block-hash:test".to_owned()),
                },
                private: (),
            },
            entries: vec![entry],
        }
    }
}
