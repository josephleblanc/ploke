use ploke_records::branch::{ResolvedTreatmentBranch, TreatmentBranchNode, TreatmentBranchStatus};
use ploke_records::evaluation::RunMetrics;
use ploke_records::history::{
    ActorRefRecord, AdmittedEntryRecord, AdmittedEntryStateRecord, CandidateArtifactRecord,
    CandidateCoordinateRecord, CandidateEvidenceRecord, CandidateLifecycleRecord,
    CandidateSetMembershipRecord, CandidateSetProofRecord, CandidateSetRecord,
    CandidateSetRootRecord, ComparedRunEvidenceRecord, EntryCoreRecord, EntryKindRecord,
    EntryPayloadRecord, EvaluationEvidenceRecord, EvaluationPayloadRecord, EvidenceCitationRecord,
    EvidenceRefRecord, ObservedEntryRecord, OperationalEnvironmentRecord, ProcedureRefRecord,
    ProtocolMetricsRecord, SelectionDecisionEntryRecord, SelectionScopeRecord, SubjectRefRecord,
    TraversalCandidateSourceRecord, TraversalEvidenceRecord, TraversalMetricInputsRecord,
    TraversalOracleModeRecord, TraversalStrategyRecord,
};
use ploke_records::ids::{
    ArtifactId, BlockHash, BlockId, BranchId, CandidateId, CandidateMembershipId,
    CandidateOccurrenceId, EntryId, HistoryHash, InstanceId, LineageId, OperationTarget,
    RecordedAt, RuntimeId, SchedulerNodeId, SourceStateId,
};
use ploke_records::scheduler::{NodeRecord, NodeStatusRecord};
use ploke_records::selection::{
    Decision, FormulaRecord, MetricCandidate, MetricPolicy, MetricSet, Outcome,
    ScoreChildPropRecord, ScoreChildPropRowRecord, SelectionFormulaRecord,
};

use crate::graph::{
    CandidateMembershipKey, CandidateSource, EvidenceKind, GraphWarningKind, HistoryEntryNode,
    HistoryPayloadKind, MetricCandidateKey, OperationKey, OperationTargetKey, SelectionFormulaKey,
    SelectionFormulaKind, SelectionMetricWitnessKey, score_child_prop_row_total_points,
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
        formula: None,
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
    let mut selection = selection_with_root(
        "root-metric",
        vec![payload_with_metrics("candidate:metric")],
        vec![membership(
            "candidate:metric",
            Some(membership_id.clone()),
            "payload-metric",
        )],
    );
    selection.formula = Some(score_child_prop_formula(selection.metrics.id.clone()));
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
    assert_eq!(candidate.compared_runs.len(), 1);
    let compared = candidate
        .compared_runs
        .first()
        .expect("compared-run metrics preserved");
    assert_eq!(
        compared
            .treatment_metrics
            .as_ref()
            .map(|metrics| metrics.tool_calls_failed),
        Some(1)
    );
    assert_eq!(
        compared
            .treatment_protocol
            .as_ref()
            .map(|metrics| metrics.reviewed_call_count),
        Some(3)
    );
    let formula = builder
        .graph
        .metrics
        .formulas
        .get(&SelectionFormulaKey {
            selection_entry_id: admitted_entry.core.entry_id.clone(),
            metric_set_id: selection.metrics.id.clone(),
        })
        .expect("selection-entry-scoped formula indexed");
    assert_eq!(formula.selection_entry_id, admitted_entry.core.entry_id);
    assert_eq!(formula.metric_set_id, selection.metrics.id);
    let SelectionFormulaKind::ScoreChildProp(score) = &formula.formula;
    assert_eq!(score.record.rows[0].payload_index, 0);
}

#[test]
fn selection_traversal_summary_populated_from_payload() {
    let mut selection = selection_with_root(
        "root-traversal",
        vec![payload_with_branch(
            "candidate:selected",
            "node-selected",
            "branch-selected",
            1,
        )],
        vec![membership("candidate:selected", None, "payload-selected")],
    );
    selection.selected_candidate = Some(SubjectRefRecord {
        value: "candidate:selected".to_owned(),
    });
    selection.decision.selected_branch_id = Some("branch-selected".to_owned());
    selection.traversal = Some(TraversalEvidenceRecord {
        seed: 7,
        strategy: TraversalStrategyRecord::ScoreChildProp {
            top_m: 3,
            lambda_millis: 10_000,
            metrics: TraversalMetricInputsRecord::OperationalAndProtocol,
            oracle: TraversalOracleModeRecord::RecordOnly,
            require_evidence: true,
        },
        selected_source: Some(TraversalCandidateSourceRecord::CurrentGeneration),
    });
    let admitted_entry = entry(selection.clone());
    let mut builder = Builder::default();

    builder.ingest_selection(&admitted_entry, &selection);

    let node = builder
        .graph
        .selections
        .selections
        .get(&admitted_entry.core.entry_id)
        .expect("selection node indexed");
    let traversal = node.traversal.as_ref().expect("traversal preserved");
    assert_eq!(traversal.seed, 7);
    assert_eq!(
        traversal.selected_source,
        Some(CandidateSource::CurrentGeneration)
    );
    assert!(matches!(
        traversal.strategy,
        TraversalStrategyRecord::ScoreChildProp { .. }
    ));
    assert_eq!(node.generation_label.as_deref(), Some("1"));
}

#[test]
fn selection_metric_witness_bundles_formula_row_and_metric_candidate() {
    let mut selection = selection_with_root(
        "root-witness",
        vec![payload_with_branch(
            "candidate:metric",
            "node:metric",
            "branch:metric",
            1,
        )],
        vec![membership("candidate:metric", None, "payload-metric")],
    );
    selection.formula = Some(score_child_prop_formula(selection.metrics.id.clone()));
    let admitted_entry = entry(selection.clone());
    let mut builder = Builder::default();

    builder.ingest_selection(&admitted_entry, &selection);

    let key = SelectionMetricWitnessKey {
        entry_id: admitted_entry.core.entry_id.clone(),
        payload_index: 0,
        branch_id: "branch:metric".to_owned(),
    };
    let witness = builder
        .graph
        .selection_metric_witness(&key)
        .expect("selection metric witness indexed");
    assert_eq!(witness.metric_candidate.payload_index, 0);
    assert_eq!(witness.metric_candidate.compared_runs.len(), 0);
    let row = witness
        .score_child_prop_row
        .expect("score_child_prop row preserved");
    assert_eq!(score_child_prop_row_total_points(row), Some(10_850));
    assert!(witness.imp_at_k.is_none());
}

#[test]
fn trajectory_generations_returns_selected_row_score() {
    let mut selection = selection_with_root(
        "root-trajectory",
        vec![payload_with_branch(
            "candidate:metric",
            "node:metric",
            "branch:metric",
            1,
        )],
        vec![membership("candidate:metric", None, "payload-metric")],
    );
    selection.selected_candidate = Some(SubjectRefRecord {
        value: "candidate:metric".to_owned(),
    });
    selection.decision.selected_branch_id = Some("branch:metric".to_owned());
    selection.formula = Some(score_child_prop_formula(selection.metrics.id.clone()));
    let admitted_entry = entry(selection.clone());
    let mut builder = Builder::default();
    builder.ingest_selection(&admitted_entry, &selection);
    index_selection_history(&mut builder, &admitted_entry);
    let graph = builder.graph;

    let rows = graph.trajectory_generations();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.generation, 1);
    assert_eq!(row.branch_id.as_deref(), Some("branch:metric"));
    assert_eq!(row.score_child_prop_total, Some(10_850));
    assert_eq!(row.decision_outcome, Outcome::Accepted);
}

#[test]
fn protocol_graph_fixture_selection_witness_and_trajectory() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ploke-egui/benchmark-fixtures/protocol-graph.json");
    if !path.exists() {
        return;
    }

    let snapshot = crate::GraphSnapshot::read_json(&path).expect("read protocol graph fixture");
    let graph = snapshot.graph();
    assert!(
        !graph.selections.selections.is_empty(),
        "protocol-graph fixture should contain selection decisions"
    );
    assert!(
        !graph.selections.metric_witnesses.is_empty(),
        "protocol-graph fixture should index selection metric witnesses"
    );

    let rows = graph.trajectory_generations();
    assert!(
        !rows.is_empty(),
        "protocol-graph should expose trajectory rows"
    );
    assert!(
        rows.iter().any(|row| row.score_child_prop_total.is_some()),
        "protocol-graph trajectory rows should carry score_child_prop totals"
    );
    assert!(
        graph
            .selections
            .selections
            .values()
            .any(|selection| selection.traversal.is_some()),
        "protocol-graph selections should preserve traversal evidence"
    );
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
    let mut payload = payload_with_branch(candidate, node_id, "branch:coordinate", 1);
    if let Some(sealed) = payload.sealed_evidence.as_mut() {
        sealed.coordinate.primary_runtime_id = Some(runtime_id.to_owned());
    }
    payload
}

fn payload_with_branch(
    candidate: &str,
    node_id: &str,
    branch_id: &str,
    generation: u32,
) -> EvaluationPayloadRecord {
    let mut payload = payload(candidate);
    payload.sealed_evidence = Some(CandidateEvidenceRecord {
        schema_version: 1,
        coordinate: CandidateCoordinateRecord {
            node_id: node_id.to_owned(),
            parent_node_id: None,
            branch_id: Some(branch_id.to_owned()),
            generation: Some(generation),
            plan_index: None,
            primary_runtime_id: Some("runtime:fixture".to_owned()),
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

fn index_selection_history(builder: &mut Builder, admitted: &AdmittedEntryRecord) {
    builder.graph.history.entries.insert(
        admitted.core.entry_id.clone(),
        HistoryEntryNode {
            entry_id: admitted.core.entry_id.clone(),
            block_hash: BlockHash("block-hash".to_owned()),
            lineage_id: admitted.state.lineage_id.clone(),
            block_id: admitted.state.block_id.clone(),
            block_height: admitted.state.block_height,
            kind: admitted.core.entry_kind.clone(),
            subject: admitted.core.subject.clone(),
            executor: admitted.core.executor.clone(),
            observer: admitted.state.observed.observer.clone(),
            recorder: admitted.state.observed.recorder.clone(),
            proposer: admitted.state.proposer.clone(),
            admitting_authority: admitted.state.admitting_authority.clone(),
            ruling_authority: admitted.state.ruling_authority.clone(),
            procedure_or_policy: admitted.state.procedure_or_policy.clone(),
            payload: HistoryPayloadKind::SelectionDecision,
            occurred_at: admitted.core.occurred_at,
            observed_at: admitted.state.observed.observed_at,
            recorded_at: admitted.state.observed.recorded_at,
            input_refs: admitted.core.input_refs.clone(),
            output_refs: admitted.core.output_refs.clone(),
            payload_ref: admitted.state.observed.payload_ref.clone(),
        },
    );
}

fn payload_with_metrics(candidate: &str) -> EvaluationPayloadRecord {
    let mut payload = payload_with_coordinate(candidate, "node:metric", "runtime:metric");
    let sealed = payload
        .sealed_evidence
        .as_mut()
        .expect("coordinate payload has sealed evidence");
    sealed.evaluations.push(EvaluationEvidenceRecord {
        branch_id: "branch:metric".to_owned(),
        evaluation_procedure_id: Some("eval:metric".to_owned()),
        evaluator_identity: None,
        eval_set_identity: None,
        evaluation_artifact_citation: None,
        overall_disposition: Some("keep".to_owned()),
        primary_report_citation: citation("evaluation:metric"),
        compared_runs: vec![ComparedRunEvidenceRecord {
            instance_id: Some("instance:metric".to_owned()),
            status: Some("completed".to_owned()),
            baseline_citation: None,
            treatment_citation: None,
            baseline_metrics: Some(run_metrics(0, 2)),
            treatment_metrics: Some(run_metrics(1, 1)),
            oracle_evaluation: None,
            baseline_protocol: Some(protocol_metrics(1, 4)),
            treatment_protocol: Some(protocol_metrics(3, 1)),
            diagnostics: Vec::new(),
            baseline_run: None,
            treatment_run: None,
        }],
    });
    payload
}

fn citation(ref_id: &str) -> EvidenceCitationRecord {
    EvidenceCitationRecord {
        ref_id: ref_id.to_owned(),
        content_hash: None,
        record_name: None,
    }
}

fn run_metrics(tool_calls_failed: u64, partial_patch_failures: u64) -> RunMetrics {
    RunMetrics {
        tool_calls_total: 4,
        tool_calls_failed,
        patch_attempted: true,
        patch_apply_state: "applied".to_owned(),
        submission_artifact_state: "present".to_owned(),
        patch_projection_check_state: Default::default(),
        partial_patch_failures,
        same_file_patch_retry_count: 0,
        same_file_patch_max_streak: 0,
        aborted: false,
        aborted_repair_loop: false,
        nonempty_valid_patch: true,
        convergence: true,
        oracle_eligible: true,
    }
}

fn protocol_metrics(
    reviewed_call_count: usize,
    missing_call_count: usize,
) -> ProtocolMetricsRecord {
    ProtocolMetricsRecord {
        scanned_artifact_count: 1,
        artifact_counts: Default::default(),
        total_calls_in_run: reviewed_call_count + missing_call_count,
        total_segments_in_anchor: 0,
        reviewed_call_count,
        reviewed_segment_count: 0,
        missing_call_count,
        missing_segment_count: 0,
        skipped_segment_review_count: 0,
        segment_anchor_mismatch_count: 0,
        call_review_overall_counts: Default::default(),
        segment_review_overall_counts: Default::default(),
        call_review_confidence_counts: Default::default(),
        segment_review_confidence_counts: Default::default(),
        calls_with_segment_crosswalk: 0,
        calls_without_segment_crosswalk: 0,
        average_calls_per_anchor_segment_x1000: 0,
        review_signal_totals: Default::default(),
    }
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

fn score_child_prop_formula(metric_set_id: HistoryHash) -> SelectionFormulaRecord {
    SelectionFormulaRecord {
        metric_set_id,
        formula: FormulaRecord::ScoreChildProp(ScoreChildPropRecord {
            schema_version: 1,
            seed: 7,
            top_m: 1,
            lambda_millis: 1000,
            lambda: 1.0,
            metric_inputs: "operational".to_owned(),
            oracle_mode: "record_only".to_owned(),
            oracle_require_evidence: true,
            total_weight: 0.5,
            sample: Some(0.25),
            sample_threshold: Some(0.125),
            uniform_fallback_slot: None,
            selected_index: Some(0),
            selected_candidate: Some("candidate:metric".to_owned()),
            alpha_mid: 0.5,
            oracle_used_for_alpha: false,
            rows: vec![ScoreChildPropRowRecord {
                payload_index: 0,
                candidate: "candidate:metric".to_owned(),
                node_id: Some("candidate:metric".to_owned()),
                branch_id: Some("branch:metric".to_owned()),
                branch_disposition: Some("keep".to_owned()),
                base_outcome: Outcome::Accepted,
                outcome_points: 10_000,
                operational_points: 850,
                protocol_points: 0,
                imp_at_k_delta: Some(0),
                imp_at_k_score_excluded: false,
                performance: Some(10_850),
                oracle_resolved: None,
                oracle_configured: None,
                oracle_rate: None,
                oracle_used_for_alpha: false,
                child_count: Some(0),
                alpha: Some(0.5),
                alpha_mid: Some(0.5),
                exploitation: Some(0.5),
                exploration: Some(1.0),
                weight: Some(0.5),
                cumulative_lower: Some(0.0),
                cumulative_upper: Some(0.5),
                sample_hit: true,
                selectable: true,
                exclusion_reason: None,
                performance_present: true,
                selection_input_present: true,
                decision_present: true,
                selected: true,
            }],
        }),
    }
}
