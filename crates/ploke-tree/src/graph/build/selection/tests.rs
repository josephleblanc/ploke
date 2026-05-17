use ploke_records::branch::{ResolvedTreatmentBranch, TreatmentBranchNode, TreatmentBranchStatus};
use ploke_records::history::{
    ActorRefRecord, AdmittedEntryRecord, AdmittedEntryStateRecord, CandidateArtifactRecord,
    CandidateCoordinateRecord, CandidateEvidenceRecord, CandidateLifecycleRecord,
    CandidateSetMembershipRecord, CandidateSetProofRecord, CandidateSetRecord,
    CandidateSetRootRecord, EntryCoreRecord, EntryKindRecord, EntryPayloadRecord,
    EvaluationPayloadRecord, EvidenceRefRecord, ObservedEntryRecord, OperationalEnvironmentRecord,
    ProcedureRefRecord, SelectionDecisionEntryRecord, SelectionScopeRecord, SubjectRefRecord,
};
use ploke_records::ids::{
    ArtifactId, BlockId, BranchId, CandidateId, CandidateMembershipId, CandidateOccurrenceId,
    EntryId, HistoryHash, InstanceId, LineageId, OperationTarget, RecordedAt, RuntimeId,
    SchedulerNodeId, SourceStateId,
};
use ploke_records::scheduler::{NodeRecord, NodeStatusRecord};
use ploke_records::selection::{Decision, MetricCandidate, MetricPolicy, MetricSet, Outcome};

use crate::graph::{
    CandidateMembershipKey, EvidenceKind, GraphWarningKind, MetricCandidateKey, OperationKey,
    OperationTargetKey,
};

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
fn selected_coordinate_payload_does_not_use_label_only_membership_fallback() {
    let other_membership = CandidateMembershipId("membership:other".to_owned());
    let missing_selected_membership = CandidateMembershipId("membership:missing".to_owned());
    let mut selection = selection(
        vec![payload_with_coordinate(
            "candidate:same",
            "node:first",
            "runtime:first",
        )],
        vec![membership(
            "candidate:same",
            Some(other_membership),
            "hash-other",
        )],
    );
    selection.selected_candidate = Some(SubjectRefRecord {
        value: "candidate:same".to_owned(),
    });
    selection.selected_membership_id = Some(missing_selected_membership);
    selection.decision.candidate_node_id = "node:first".to_owned();
    let admitted_entry = entry(selection.clone());
    let mut builder = Builder::default();

    builder.ingest_selection(&admitted_entry, &selection);

    assert_eq!(builder.graph.candidates.candidates[0].membership_id, None);
    assert_eq!(builder.graph.candidates.candidates[0].membership_key, None);
    assert!(
        builder
            .graph
            .warnings
            .iter()
            .any(|warning| { warning.kind == GraphWarningKind::SelectedMembershipMissing })
    );
    assert!(builder.graph.warnings.iter().any(|warning| {
        warning.kind == GraphWarningKind::CandidateSetMembershipMissingForPayload
    }));
}

#[test]
fn selection_payload_populates_runtime_target_operation_identity() {
    let selection = selection(
        vec![payload_with_operation("candidate:a", "node-1", "runtime-1")],
        vec![membership(
            "candidate:a",
            Some(CandidateMembershipId("m1".to_owned())),
            "hash-a",
        )],
    );
    let admitted_entry = entry(selection.clone());
    let mut builder = Builder::default();

    builder.ingest_selection(&admitted_entry, &selection);

    let key = OperationKey::RuntimeTarget {
        runtime_id: RuntimeId("runtime-1".to_owned()),
        target: OperationTargetKey::Artifact {
            artifact_id: ArtifactId("artifact-before".to_owned()),
        },
    };
    let operation = builder
        .graph
        .operations
        .operations
        .get(&key)
        .expect("selection payload operation is indexed by runtime and target");
    assert!(operation.evidence.iter().any(|id| {
        builder.graph.evidence.attachments[id].kind == EvidenceKind::CandidatePayload
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
    let considered_order_hash = HistoryHash("order-hash".to_owned());
    let metrics = metrics(root, &considered_order_hash, &considered, &memberships);
    SelectionDecisionEntryRecord {
        schema_version: 4,
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
        considered_order_hash,
        candidate_set: Some(CandidateSetRecord {
            root: CandidateSetRootRecord(HistoryHash(root.to_owned())),
            memberships,
        }),
        projection_failures: Vec::new(),
        traversal: None,
        metrics,
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

#[test]
fn selection_metrics_ingestion_preserves_metric_bindings() {
    let membership_id = CandidateMembershipId("membership:metric".to_owned());
    let selection = selection_with_root(
        "root-metric",
        vec![payload("candidate:metric")],
        vec![membership(
            "candidate:metric",
            Some(membership_id.clone()),
            "payload-metric",
        )],
    );
    let admitted_entry = entry(selection.clone());
    let mut builder = Builder::default();

    builder.ingest_selection(&admitted_entry, &selection);

    let set = builder
        .graph
        .metrics
        .sets
        .get(&selection.metrics.id)
        .expect("metric set indexed");
    assert_eq!(set.selection_entry_id, admitted_entry.core.entry_id);
    assert_eq!(
        set.candidate_set_root.as_ref(),
        selection.metrics.candidate_set_root.as_ref()
    );
    let candidate = builder
        .graph
        .metrics
        .candidates
        .get(&MetricCandidateKey {
            metric_set_id: selection.metrics.id.clone(),
            payload_index: 0,
        })
        .expect("metric candidate indexed");
    assert_eq!(candidate.membership_id.as_ref(), Some(&membership_id));
    assert_eq!(candidate.payload_hash.0, "payload-metric");
}

#[test]
fn selection_metrics_candidate_set_hash_mismatch_warns() {
    let mut selection = selection_with_root(
        "root-metric",
        vec![payload("candidate:metric")],
        vec![membership("candidate:metric", None, "payload-metric")],
    );
    selection.metrics.candidate_set_root = Some(HistoryHash("other-root".to_owned()));
    let admitted_entry = entry(selection.clone());
    let mut builder = Builder::default();

    builder.ingest_selection(&admitted_entry, &selection);

    assert!(builder.graph.warnings.iter().any(|warning| {
        warning.kind == GraphWarningKind::SelectionMetricBindingMismatch
            && warning.detail.contains("candidate_set_root")
    }));
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

fn payload_with_operation(
    candidate: &str,
    node_id: &str,
    runtime_id: &str,
) -> EvaluationPayloadRecord {
    let mut payload = payload_with_coordinate(candidate, node_id, runtime_id);
    payload.artifact = Some(CandidateArtifactRecord {
        schema_version: 1,
        node: operation_node_record(),
        resolved: resolved_branch(),
        surface: None,
    });
    payload
}

fn operation_node_record() -> NodeRecord {
    NodeRecord {
        schema_version: "prototype1-treatment-node.v1".to_owned(),
        node_id: SchedulerNodeId("node-1".to_owned()),
        parent_node_id: None,
        generation: 1,
        instance_id: InstanceId("instance-1".to_owned()),
        source_state_id: SourceStateId("source-1".to_owned()),
        operation_target: Some(operation_target()),
        base_artifact_id: Some(ArtifactId("artifact-before".to_owned())),
        patch_id: None,
        derived_artifact_id: Some(ArtifactId("artifact-after".to_owned())),
        parent_branch_id: None,
        branch_id: BranchId("branch-1".to_owned()),
        candidate_id: CandidateId("candidate-1".to_owned()),
        target_relpath: "src/lib.rs".into(),
        node_dir: "nodes/node-1".into(),
        workspace_root: "worktree".into(),
        binary_path: "target/debug/ploke".into(),
        runner_request_path: "runner-request.json".into(),
        runner_result_path: "runner-result.json".into(),
        status: NodeStatusRecord::Succeeded,
        created_at: "2026-05-11T00:00:00Z".to_owned(),
        updated_at: "2026-05-11T00:01:00Z".to_owned(),
    }
}

fn resolved_branch() -> ResolvedTreatmentBranch {
    ResolvedTreatmentBranch {
        instance_id: "instance-1".to_owned(),
        source_state_id: "source-1".to_owned(),
        parent_branch_id: None,
        target_relpath: "src/lib.rs".into(),
        source_content: "before".to_owned(),
        source_content_hash: "hash-before".to_owned(),
        selected_branch_id: Some("branch-1".to_owned()),
        branch: TreatmentBranchNode {
            branch_id: "branch-1".to_owned(),
            candidate_id: "candidate-1".to_owned(),
            patch_id: None,
            branch_label: "branch 1".to_owned(),
            synthesized_spec_id: "spec-1".to_owned(),
            proposed_content: "after".to_owned(),
            proposed_content_hash: "hash-after".to_owned(),
            generation_target: Some(operation_target()),
            generation_coordinate: None,
            status: TreatmentBranchStatus::Synthesized,
            apply_id: None,
            applied_content_hash: None,
            derived_artifact_id: Some(ArtifactId("artifact-after".to_owned())),
            latest_evaluation: None,
        },
    }
}

fn operation_target() -> OperationTarget {
    OperationTarget::Artifact {
        artifact_id: ArtifactId("artifact-before".to_owned()),
    }
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

fn metrics(
    root: &str,
    considered_order_hash: &HistoryHash,
    considered: &[EvaluationPayloadRecord],
    memberships: &[CandidateSetMembershipRecord],
) -> MetricSet {
    MetricSet {
        schema_version: 1,
        id: HistoryHash(format!("metric-set:{root}")),
        considered_order_hash: considered_order_hash.clone(),
        candidate_set_root: Some(HistoryHash(root.to_owned())),
        policy: MetricPolicy::default(),
        candidates: considered
            .iter()
            .enumerate()
            .map(|(index, payload)| {
                let membership = memberships.get(index);
                MetricCandidate {
                    payload_index: index,
                    payload_hash: membership
                        .map(|member| member.payload_hash.clone())
                        .unwrap_or_else(|| HistoryHash(format!("payload:{index}"))),
                    candidate: payload.candidate.value.clone(),
                    occurrence_id: membership.and_then(|member| member.occurrence_id.clone()),
                    membership_id: membership.and_then(|member| member.membership_id.clone()),
                    imp_at_k: None,
                }
            })
            .collect(),
    }
}
