use ploke_records::history::{
    CandidateSetMembershipRecord, EvaluationPayloadRecord, SelectionDecisionEntryRecord,
    SubjectRefRecord,
};
use ploke_records::ids::{CandidateMembershipId, CandidateOccurrenceId};

pub(super) enum Resolution<'a> {
    Matched(&'a CandidateSetMembershipRecord),
    Missing,
    Ambiguous { matching_memberships: usize },
}

pub(super) struct Selected<'a> {
    candidate_node_id: &'a str,
    selected_candidate: Option<&'a SubjectRefRecord>,
    occurrence_id: Option<&'a CandidateOccurrenceId>,
    membership_id: Option<&'a CandidateMembershipId>,
}

impl<'a> Selected<'a> {
    pub(super) fn from_selection(selection: &'a SelectionDecisionEntryRecord) -> Self {
        Self {
            candidate_node_id: &selection.decision.candidate_node_id,
            selected_candidate: selection.selected_candidate.as_ref(),
            occurrence_id: selection.selected_occurrence_id.as_ref(),
            membership_id: selection.selected_membership_id.as_ref(),
        }
    }

    fn applies_to(&self, payload: &EvaluationPayloadRecord) -> bool {
        let has_coordinate = payload.sealed_evidence.is_some();
        if payload
            .sealed_evidence
            .as_ref()
            .is_some_and(|evidence| evidence.coordinate.node_id == self.candidate_node_id)
        {
            return true;
        }

        let has_selection_input = payload.selection_input.is_some();
        if payload
            .selection_input
            .as_ref()
            .is_some_and(|input| input.candidate.node_id == self.candidate_node_id)
        {
            return true;
        }

        if has_coordinate || has_selection_input {
            return false;
        }

        self.selected_candidate
            .is_some_and(|candidate| candidate == &payload.candidate)
    }
}

pub(super) fn for_payload<'a>(
    payload: &EvaluationPayloadRecord,
    selected: Option<&Selected<'_>>,
    memberships: &'a [CandidateSetMembershipRecord],
) -> Resolution<'a> {
    if let Some(selected) = selected
        && selected.applies_to(payload)
    {
        if let Some(membership_id) = selected.membership_id {
            match unique_membership(memberships.iter().filter(|member| {
                member.membership_id.as_ref() == Some(membership_id)
                    && member.candidate == payload.candidate
            })) {
                Resolution::Missing => {}
                resolved => return resolved,
            }
        }

        if let Some(occurrence_id) = selected.occurrence_id {
            match unique_membership(memberships.iter().filter(|member| {
                member.occurrence_id.as_ref() == Some(occurrence_id)
                    && member.candidate == payload.candidate
            })) {
                Resolution::Missing => {}
                resolved => return resolved,
            }
        }
    }

    unique_membership(
        memberships
            .iter()
            .filter(|member| member.candidate == payload.candidate),
    )
}

fn unique_membership<'a>(
    mut matching: impl Iterator<Item = &'a CandidateSetMembershipRecord>,
) -> Resolution<'a> {
    let Some(first) = matching.next() else {
        return Resolution::Missing;
    };
    let Some(_) = matching.next() else {
        return Resolution::Matched(first);
    };
    Resolution::Ambiguous {
        matching_memberships: 2 + matching.count(),
    }
}

#[cfg(test)]
mod tests {
    use ploke_records::history::{
        CandidateCoordinateRecord, CandidateEvidenceRecord, CandidateLifecycleRecord,
        CandidateSetMembershipRecord, CandidateSetProofRecord, EvaluationPayloadRecord,
        ProcedureRefRecord, SubjectRefRecord,
    };
    use ploke_records::ids::{CandidateMembershipId, CandidateOccurrenceId, HistoryHash};

    use super::{Resolution, Selected, for_payload};

    #[test]
    fn membership_matches_unique_recorded_candidate_identity() {
        let payload = payload("candidate:a");
        let membership_id = CandidateMembershipId("membership-a".to_owned());
        let memberships = vec![
            membership("candidate:b", None, "hash-b"),
            membership("candidate:a", Some(membership_id.clone()), "hash-a"),
        ];

        let Resolution::Matched(member) = for_payload(&payload, None, &memberships) else {
            panic!("expected unique candidate identity match");
        };

        assert_eq!(member.membership_id.as_ref(), Some(&membership_id));
    }

    #[test]
    fn membership_does_not_fall_back_to_vector_index() {
        let payload = payload("candidate:a");
        let memberships = vec![membership("candidate:b", None, "hash-b")];

        assert!(matches!(
            for_payload(&payload, None, &memberships),
            Resolution::Missing
        ));
    }

    #[test]
    fn membership_reports_ambiguous_recorded_candidate_identity() {
        let payload = payload("candidate:a");
        let memberships = vec![
            membership("candidate:a", None, "hash-a1"),
            membership("candidate:a", None, "hash-a2"),
        ];

        let Resolution::Ambiguous {
            matching_memberships,
        } = for_payload(&payload, None, &memberships)
        else {
            panic!("expected ambiguous candidate identity");
        };

        assert_eq!(matching_memberships, 2);
    }

    #[test]
    fn selected_membership_uses_recorded_selection_identity() {
        let payload = payload_with_coordinate("candidate:same", "node:first");
        let selected_membership = CandidateMembershipId("membership:first".to_owned());
        let selected_occurrence = CandidateOccurrenceId("occurrence:first".to_owned());
        let selected = Selected {
            candidate_node_id: "node:first",
            selected_candidate: Some(&payload.candidate),
            occurrence_id: Some(&selected_occurrence),
            membership_id: Some(&selected_membership),
        };
        let memberships = vec![
            membership(
                "candidate:same",
                Some(selected_membership.clone()),
                "hash-first",
            ),
            membership(
                "candidate:same",
                Some(CandidateMembershipId("membership:second".into())),
                "hash-second",
            ),
        ];

        let Resolution::Matched(member) = for_payload(&payload, Some(&selected), &memberships)
        else {
            panic!("expected selected membership identity match");
        };

        assert_eq!(member.membership_id.as_ref(), Some(&selected_membership));
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

    fn payload_with_coordinate(candidate: &str, node_id: &str) -> EvaluationPayloadRecord {
        let mut payload = payload(candidate);
        payload.sealed_evidence = Some(CandidateEvidenceRecord {
            schema_version: 1,
            coordinate: CandidateCoordinateRecord {
                node_id: node_id.to_owned(),
                parent_node_id: None,
                branch_id: None,
                generation: Some(1),
                plan_index: None,
                primary_runtime_id: Some("runtime:first".to_owned()),
            },
            lifecycle: CandidateLifecycleRecord {
                planner_outcome: "eligible".to_owned(),
                node_status: "completed".to_owned(),
            },
            evaluations: Vec::new(),
            runtimes: Vec::new(),
            branches: Vec::new(),
            extra_document_citations: Vec::new(),
            extra_journal_citations: Vec::new(),
            child_diagnostics: Vec::new(),
        });
        payload
    }

    fn membership(
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
}
