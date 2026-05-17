use ploke_records::history::{
    ActorRefRecord, AdmittedEntryRecord, AdmittedEntryStateRecord, ArtifactRefRecord,
    BlockCommonRecord, CandidateSetMembershipRecord, CandidateSetProofRecord, CandidateSetRecord,
    CandidateSetRootRecord, ClaimsRecord, EntryCoreRecord, EntryKindRecord, EntryPayloadRecord,
    EvaluationPayloadRecord, EvidenceRefRecord, GenesisAuthorityRecord, LevelRecord,
    OpeningAuthorityRecord, OperationalEnvironmentRecord, ParentIdentityRefRecord, PhaseRecord,
    ProcedureRefRecord, RegimeRecord, RiskRecord, SealedBlockHeaderRecord, SealedBlockRecord,
    SealedBlockStateRecord, SelectionDecisionEntryRecord, SelectionScopeRecord, StepRecord,
    SubjectRefRecord, SuccessorRefRecord, SurfaceCommitmentRecord, SurfaceDeltaRecord,
    SurfaceRecord, SurfaceRootRecord, TreeKeyHashRecord,
};
use ploke_records::ids::{
    ArtifactId, BlockHash, BlockId, CandidateMembershipId, CandidateOccurrenceId, EntryId,
    HistoryHash, HistoryStateRoot, LineageId, RecordedAt, RuntimeId,
};
use ploke_records::playback::FineStepKind;
use ploke_records::selection::{Decision, MetricCandidate, MetricPolicy, MetricSet, Outcome};

use super::build::{
    fine_run_playback_from_sealed_history, fine_run_playback_ref_steps_from_sealed_history,
};
use super::ids::fine_candidate_step_id;
use super::membership::candidate_membership;

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
    let considered_order_hash = HistoryHash("order-hash".to_owned());
    let metrics = selection_metrics(
        "candidate-set-root",
        &considered_order_hash,
        &considered,
        &memberships,
    );
    SelectionDecisionEntryRecord {
        schema_version: 4,
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
        considered_order_hash,
        candidate_set: Some(CandidateSetRecord {
            root: CandidateSetRootRecord(HistoryHash("candidate-set-root".to_owned())),
            memberships,
        }),
        projection_failures: Vec::new(),
        traversal: None,
        metrics,
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

fn selection_metrics(
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
    let artifact = ArtifactRefRecord::from_artifact_id(ArtifactId("artifact:test".to_owned()));
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
                    opening_authority: OpeningAuthorityRecord::Genesis(GenesisAuthorityRecord {
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
                    }),
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
