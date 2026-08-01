use ploke_records::history::{
    CandidateSetMembershipRecord, EvaluationPayloadRecord, SelectionDecisionEntryRecord,
};

pub(super) fn candidate_membership<'a>(
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

pub(super) fn membership_candidate_set_root<'a>(
    selection: &'a SelectionDecisionEntryRecord,
    membership: Option<&CandidateSetMembershipRecord>,
) -> Option<&'a str> {
    membership?;
    selection
        .candidate_set
        .as_ref()
        .map(|candidate_set| candidate_set.root.0.0.as_str())
}

pub(super) fn selected_candidate_set_root(
    selection: &SelectionDecisionEntryRecord,
) -> Option<&str> {
    let selected_membership_id = selection.selected_membership_id.as_ref()?;
    let candidate_set = selection.candidate_set.as_ref()?;
    candidate_set
        .memberships
        .iter()
        .any(|membership| membership.membership_id.as_ref() == Some(selected_membership_id))
        .then_some(candidate_set.root.0.0.as_str())
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
