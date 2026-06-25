use super::*;

pub(in crate::unit) fn assert_call_candidate(
    candidates: &[CallContextCandidate],
    node_id: Uuid,
    relation: CallContextRelation,
    call_site_id: Uuid,
    target_id: Uuid,
    message: &str,
) {
    assert!(
        candidates.iter().any(|candidate| {
            candidate.node_id == node_id
                && candidate.relation == relation
                && candidate.call_site_id == call_site_id
                && candidate.target_id == target_id
                && candidate.distance == 1
        }),
        "{message}; candidates: {candidates:#?}"
    );
}

pub(in crate::unit) fn assert_incoming_candidate(
    candidates: &[CallContextCandidate],
    owner_id: Uuid,
    call_site_id: Uuid,
    target_id: Uuid,
    message: &str,
) {
    assert_call_candidate(
        candidates,
        owner_id,
        CallContextRelation::IncomingCaller,
        call_site_id,
        target_id,
        message,
    );
}

pub(in crate::unit) fn assert_outgoing_candidate(
    candidates: &[CallContextCandidate],
    target_id: Uuid,
    call_site_id: Uuid,
    message: &str,
) {
    assert_call_candidate(
        candidates,
        target_id,
        CallContextRelation::OutgoingTarget,
        call_site_id,
        target_id,
        message,
    );
}

pub(in crate::unit) fn assert_incoming_candidates_for_target(
    candidates: &[CallContextCandidate],
    target_id: Uuid,
    message: &str,
) {
    assert!(
        candidates.iter().all(|candidate| {
            candidate.target_id == target_id
                && candidate.relation == CallContextRelation::IncomingCaller
                && candidate.distance == 1
        }),
        "{message}: {candidates:#?}"
    );
}
