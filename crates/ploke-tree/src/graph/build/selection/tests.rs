use ploke_records::history::{
    ActorRefRecord, AdmittedEntryRecord, AdmittedEntryStateRecord, CandidateCoordinateRecord,
    CandidateEvidenceRecord, CandidateLifecycleRecord, CandidateSetMembershipRecord,
    CandidateSetProofRecord, CandidateSetRecord, CandidateSetRootRecord, EntryCoreRecord,
    EntryKindRecord, EntryPayloadRecord, EvaluationPayloadRecord, EvidenceRefRecord,
    ObservedEntryRecord, OperationalEnvironmentRecord, ProcedureRefRecord,
    SelectionDecisionEntryRecord, SelectionScopeRecord, SubjectRefRecord,
};
use ploke_records::ids::{
    BlockId, CandidateMembershipId, CandidateOccurrenceId, EntryId, HistoryHash, LineageId,
    RecordedAt,
};
use ploke_records::selection::{Decision, Outcome};

use crate::graph::{CandidateMembershipKey, GraphWarningKind};

use super::super::Builder;

#[test]
fn ingest_selection_attaches_membership_by_identity_not_index() {
    let membership_a = CandidateMembershipId("membership-a".to_owned());
    let membership_b = CandidateMembershipId("membership-b".to_owned());
    let selection = selection(
        vec![payload("candidate:a"), payload("candidate:b")],
        vec![
            membership("candidate:b", Some(membership_b.clone()), "hash-b"),
            membership("candidate:a", Some(membership_a.clone()), "hash-a"),
        ],
    );
    let admitted_entry = entry(selection.clone());
    let mut builder = Builder::default();

    builder.ingest_selection(&admitted_entry, &selection);

    assert_eq!(
        builder.graph.candidates.candidates[0]
            .membership_id
            .as_ref(),
        Some(&membership_a)
    );
    assert_eq!(
        builder.graph.candidates.candidates[1]
            .membership_id
            .as_ref(),
        Some(&membership_b)
    );
    assert!(
        builder.graph.warnings.iter().all(|warning| {
            warning.kind != GraphWarningKind::CandidateSetMembershipMissingForPayload
                && warning.kind != GraphWarningKind::CandidateSetMembershipAmbiguousForPayload
        }),
        "reordered memberships should not be treated as missing or ambiguous"
    );
}

#[test]
fn selected_coordinate_payload_uses_recorded_selected_membership() {
    let first = payload_with_coordinate("candidate:same", "node:first", "runtime:first");
    let second = payload_with_coordinate("candidate:same", "node:second", "runtime:second");
    let membership_first = CandidateMembershipId("membership:first".to_owned());
    let membership_second = CandidateMembershipId("membership:second".to_owned());
    let mut selection = selection(
        vec![first, second],
        vec![
            membership(
                "candidate:same",
                Some(membership_second.clone()),
                "hash-second",
            ),
            membership(
                "candidate:same",
                Some(membership_first.clone()),
                "hash-first",
            ),
        ],
    );
    selection.selected_candidate = Some(SubjectRefRecord {
        value: "candidate:same".to_owned(),
    });
    selection.selected_membership_id = Some(membership_first.clone());
    selection.decision.candidate_node_id = "node:first".to_owned();
    let admitted_entry = entry(selection.clone());
    let mut builder = Builder::default();

    builder.ingest_selection(&admitted_entry, &selection);

    assert_eq!(
        builder.graph.candidates.candidates[0]
            .membership_id
            .as_ref(),
        Some(&membership_first)
    );
    assert_eq!(builder.graph.candidates.candidates[1].membership_id, None);
    assert!(builder.graph.warnings.iter().any(|warning| {
        warning.kind == GraphWarningKind::CandidateSetMembershipAmbiguousForPayload
    }));
}

#[test]
fn present_empty_candidate_set_warns_but_absent_set_does_not() {
    let present_empty = selection(vec![payload("candidate:a")], Vec::new());
    let admitted_entry = entry(present_empty.clone());
    let mut builder = Builder::default();

    builder.ingest_selection(&admitted_entry, &present_empty);

    assert!(
        builder.graph.warnings.iter().any(|warning| {
            warning.kind == GraphWarningKind::CandidateSetMembershipCountMismatch
        })
    );
    assert!(builder.graph.warnings.iter().any(|warning| {
        warning.kind == GraphWarningKind::CandidateSetMembershipMissingForPayload
    }));

    let mut absent = selection(vec![payload("candidate:a")], Vec::new());
    absent.candidate_set = None;
    let admitted_entry = entry(absent.clone());
    let mut builder = Builder::default();

    builder.ingest_selection(&admitted_entry, &absent);

    assert!(builder.graph.warnings.iter().all(|warning| {
        warning.kind != GraphWarningKind::CandidateSetMembershipCountMismatch
            && warning.kind != GraphWarningKind::CandidateSetMembershipMissingForPayload
    }));
}

#[test]
fn duplicate_membership_id_preserves_first_node() {
    let duplicate_id = CandidateMembershipId("membership:duplicate".to_owned());
    let selection = selection(
        vec![payload("candidate:first"), payload("candidate:second")],
        vec![
            membership("candidate:first", Some(duplicate_id.clone()), "hash-first"),
            membership(
                "candidate:second",
                Some(duplicate_id.clone()),
                "hash-second",
            ),
        ],
    );
    let admitted_entry = entry(selection.clone());
    let mut builder = Builder::default();

    builder.ingest_selection(&admitted_entry, &selection);

    let node = builder
        .graph
        .candidates
        .memberships
        .get(&membership_key("root-hash", &duplicate_id))
        .expect("first membership node retained");
    assert_eq!(node.candidate_subject.value, "candidate:first");
    assert_eq!(node.payload_hash, "hash-first");
    assert!(
        builder
            .graph
            .warnings
            .iter()
            .any(|warning| { warning.kind == GraphWarningKind::DuplicateCandidateMembershipId })
    );
}

#[test]
fn same_membership_id_under_different_candidate_set_roots_keeps_both_nodes() {
    let membership_id = CandidateMembershipId("membership:scoped".to_owned());
    let first = selection_with_root(
        "root:first",
        vec![payload("candidate:first")],
        vec![membership(
            "candidate:first",
            Some(membership_id.clone()),
            "hash-first",
        )],
    );
    let second = selection_with_root(
        "root:second",
        vec![payload("candidate:second")],
        vec![membership(
            "candidate:second",
            Some(membership_id.clone()),
            "hash-second",
        )],
    );
    let mut builder = Builder::default();

    builder.ingest_selection(&entry_with_id("entry:first", first.clone()), &first);
    builder.ingest_selection(&entry_with_id("entry:second", second.clone()), &second);

    let first_key = membership_key("root:first", &membership_id);
    let second_key = membership_key("root:second", &membership_id);
    let first_node = builder
        .graph
        .candidates
        .memberships
        .get(&first_key)
        .expect("first candidate set membership retained");
    let second_node = builder
        .graph
        .candidates
        .memberships
        .get(&second_key)
        .expect("second candidate set membership retained");

    assert_eq!(first_node.candidate_subject.value, "candidate:first");
    assert_eq!(second_node.candidate_subject.value, "candidate:second");
    assert_eq!(
        builder.graph.candidates.candidates[0]
            .membership_key
            .clone(),
        Some(first_key)
    );
    assert_eq!(
        builder.graph.candidates.candidates[1]
            .membership_key
            .clone(),
        Some(second_key)
    );
    assert!(
        builder
            .graph
            .warnings
            .iter()
            .all(|warning| warning.kind != GraphWarningKind::DuplicateCandidateMembershipId)
    );
}

fn selection(
    considered: Vec<EvaluationPayloadRecord>,
    memberships: Vec<CandidateSetMembershipRecord>,
) -> SelectionDecisionEntryRecord {
    selection_with_root("root-hash", considered, memberships)
}

fn selection_with_root(
    root: &str,
    considered: Vec<EvaluationPayloadRecord>,
    memberships: Vec<CandidateSetMembershipRecord>,
) -> SelectionDecisionEntryRecord {
    SelectionDecisionEntryRecord {
        schema_version: 3,
        procedure_or_policy: ProcedureRefRecord {
            value: "prototype1.successor_selection.history_traversal.v1".to_owned(),
        },
        scope: SelectionScopeRecord {
            value: "history".to_owned(),
        },
        selected_candidate: None,
        selected_occurrence_id: None,
        selected_membership_id: None,
        considered,
        considered_sources: Vec::new(),
        considered_order_hash: HistoryHash("order-hash".to_owned()),
        candidate_set: Some(CandidateSetRecord {
            root: CandidateSetRootRecord(HistoryHash(root.to_owned())),
            memberships,
        }),
        projection_failures: Vec::new(),
        traversal: None,
        decision: Decision {
            procedure_id: "prototype1.successor_selection.history_traversal.v1".to_owned(),
            candidate_node_id: "candidate:a".to_owned(),
            selected_branch_id: None,
            branch_disposition: "keep".to_owned(),
            outcome: Outcome::Accepted,
            findings: Vec::new(),
            rationale: Vec::new(),
        },
    }
}

fn entry(selection: SelectionDecisionEntryRecord) -> AdmittedEntryRecord {
    entry_with_id("entry-1", selection)
}

fn entry_with_id(entry_id: &str, selection: SelectionDecisionEntryRecord) -> AdmittedEntryRecord {
    AdmittedEntryRecord {
        core: EntryCoreRecord {
            entry_id: EntryId(entry_id.to_owned()),
            entry_kind: EntryKindRecord::Decision,
            subject: SubjectRefRecord {
                value: "selection".to_owned(),
            },
            executor: ActorRefRecord::Process("test".to_owned()),
            input_refs: Vec::new(),
            output_refs: Vec::new(),
            occurred_at: RecordedAt(1),
            payload: EntryPayloadRecord::SelectionDecision(selection),
        },
        state: AdmittedEntryStateRecord {
            observed: ObservedEntryRecord {
                observer: ActorRefRecord::Process("test".to_owned()),
                recorder: ActorRefRecord::Process("test".to_owned()),
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
                    value: "payload-ref".to_owned(),
                },
                payload_hash: HistoryHash("payload-hash".to_owned()),
                observed_at: RecordedAt(2),
                recorded_at: RecordedAt(3),
            },
            proposer: ActorRefRecord::Process("test".to_owned()),
            procedure_or_policy: ProcedureRefRecord {
                value: "prototype1.successor_selection.history_traversal.v1".to_owned(),
            },
            admitting_authority: ActorRefRecord::Process("test".to_owned()),
            ruling_authority: ActorRefRecord::Process("test".to_owned()),
            lineage_id: LineageId("lineage-1".to_owned()),
            block_id: BlockId("block-1".to_owned()),
            block_height: 1,
        },
    }
}

fn membership_key(root: &str, membership_id: &CandidateMembershipId) -> CandidateMembershipKey {
    CandidateMembershipKey {
        candidate_set_root: CandidateSetRootRecord(HistoryHash(root.to_owned())),
        membership_id: membership_id.clone(),
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

fn payload_with_coordinate(
    candidate: &str,
    node_id: &str,
    runtime_id: &str,
) -> EvaluationPayloadRecord {
    let mut payload = payload(candidate);
    payload.sealed_evidence = Some(CandidateEvidenceRecord {
        schema_version: 1,
        coordinate: CandidateCoordinateRecord {
            node_id: node_id.to_owned(),
            parent_node_id: None,
            branch_id: None,
            generation: Some(1),
            plan_index: None,
            primary_runtime_id: Some(runtime_id.to_owned()),
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
