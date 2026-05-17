use std::path::PathBuf;

use ploke_records::history::{
    ActorRefRecord, AdmittedEntryRecord, AdmittedEntryStateRecord, ArtifactRefRecord,
    BlockCommonRecord, ClaimsRecord, EntryCoreRecord, EntryKindRecord, EntryPayloadRecord,
    EvidenceRefRecord, GenesisAuthorityRecord, LevelRecord, OpeningAuthorityRecord,
    OperationalEnvironmentRecord, ParentIdentityRefRecord, PhaseRecord, ProcedureRefRecord,
    RegimeRecord, RiskRecord, SealedBlockHeaderRecord, SealedBlockRecord, SealedBlockStateRecord,
    SelectionDecisionEntryRecord, SelectionScopeRecord, StepRecord, SuccessorRefRecord,
    SurfaceCommitmentRecord, SurfaceDeltaRecord, SurfaceRecord, SurfaceRootRecord,
    TreeKeyHashRecord,
};
use ploke_records::ids::{
    ArtifactId, BlockHash, BlockId, BranchId, CampaignId, CandidateId, EntryId, HistoryHash,
    HistoryStateRoot, InstanceId, LineageId, PatchId, RecordedAt, RuntimeId, SchedulerNodeId,
    SourceStateId,
};
use ploke_records::scheduler::{NodeRecord, NodeStatusRecord, SchedulerStateRecord};
use ploke_records::selection::{self, Outcome};
use ploke_tree::{Graph, PassiveEvidence, RunForestInput, RunRecordSet, TransitionJournal};

use super::*;

#[test]
fn import_delegates_to_ploke_tree_graph() {
    let records = RunRecordSet {
        forest_input: RunForestInput {
            scheduler: scheduler(vec![
                node("root", None, 0),
                node("selected", Some("root"), 1),
                node("other", Some("root"), 1),
            ]),
            node_records: Vec::new(),
            parent_identity: None,
            successor_ready: Vec::new(),
            successor_completion: Vec::new(),
            passive_evidence: PassiveEvidence::default(),
        },
        history_blocks: vec![sealed_block(0, "root", "selected")],
        transition_journal: TransitionJournal::default(),
    };

    let imported = graph_from_run_records(&records);
    let expected = Graph::from_records(&records);

    assert_eq!(imported, expected);
}

#[test]
#[ignore]
fn real_run_imports_execution_spine() {
    let run_root = std::env::var("PLOKE_EGUI_RUN_ROOT").expect("PLOKE_EGUI_RUN_ROOT must be set");
    let graph = graph_from_run_root(run_root).expect("import graph from run root");

    println!(
        "graph blocks={} candidates={} artifacts={} selections={} runtimes={} operations={} evidence={}",
        graph.history.blocks.len(),
        graph.candidates.candidates.len(),
        graph.artifacts.artifacts.len(),
        graph.selections.selections.len(),
        graph.runtimes.runtimes.len(),
        graph.operations.operations.len(),
        graph.evidence.attachments.len(),
    );

    assert!(!graph.history.blocks.is_empty());
    assert!(!graph.candidates.candidates.is_empty());
    assert!(!graph.artifacts.artifacts.is_empty());
    assert!(!graph.selections.selections.is_empty());
}

fn scheduler(nodes: Vec<NodeRecord>) -> SchedulerStateRecord {
    SchedulerStateRecord {
        schema_version: "prototype1-scheduler.v1".to_owned(),
        campaign_id: CampaignId("campaign-1".to_owned()),
        updated_at: "2026-05-11T12:01:00Z".to_owned(),
        policy: Default::default(),
        frontier_node_ids: Vec::new(),
        completed_node_ids: nodes.iter().map(|node| node.node_id.clone()).collect(),
        failed_node_ids: Vec::new(),
        last_continuation_decision: None,
        nodes,
    }
}

fn node(id: &str, parent: Option<&str>, generation: u32) -> NodeRecord {
    NodeRecord {
        schema_version: "prototype1-treatment-node.v1".to_owned(),
        node_id: SchedulerNodeId(id.to_owned()),
        parent_node_id: parent.map(|id| SchedulerNodeId(id.to_owned())),
        generation,
        instance_id: InstanceId(format!("instance-{id}")),
        source_state_id: SourceStateId(format!("source-{id}")),
        operation_target: None,
        base_artifact_id: Some(ArtifactId(format!("artifact-base-{id}"))),
        patch_id: Some(PatchId(format!("patch-{id}"))),
        derived_artifact_id: Some(ArtifactId(format!("artifact-derived-{id}"))),
        parent_branch_id: parent.map(|id| BranchId(format!("branch-{id}"))),
        branch_id: BranchId(format!("branch-{id}")),
        candidate_id: CandidateId(format!("candidate-{id}")),
        target_relpath: PathBuf::from("target.md"),
        node_dir: PathBuf::from(format!("/tmp/prototype1/nodes/{id}")),
        workspace_root: PathBuf::from(format!("/tmp/worktrees/{id}")),
        binary_path: PathBuf::from(format!("/tmp/worktrees/{id}/target/debug/ploke")),
        runner_request_path: PathBuf::from(format!(
            "/tmp/prototype1/nodes/{id}/runner-request.json"
        )),
        runner_result_path: PathBuf::from(format!("/tmp/prototype1/nodes/{id}/runner-result.json")),
        status: NodeStatusRecord::Succeeded,
        created_at: "2026-05-11T12:00:00Z".to_owned(),
        updated_at: "2026-05-11T12:01:00Z".to_owned(),
    }
}

fn sealed_block(block_height: u64, ruler: &str, selected: &str) -> SealedBlockRecord {
    let actor = ActorRefRecord::Process(format!("parent:{ruler}"));
    let artifact = ArtifactRefRecord::from_artifact_id(ArtifactId(format!("artifact:{ruler}")));
    let evidence = EvidenceRefRecord {
        value: format!("evidence:block:{block_height}"),
    };

    SealedBlockRecord {
        state: SealedBlockStateRecord {
            header: SealedBlockHeaderRecord {
                common: BlockCommonRecord {
                    schema_version: 1,
                    block_id: BlockId(format!("block-{block_height}")),
                    lineage_id: LineageId("lineage:test".to_owned()),
                    block_height,
                    parent_block_hashes: Vec::new(),
                    opened_from_state: HistoryStateRoot("0".repeat(64)),
                    regime: RegimeRecord {
                        step: StepRecord(block_height),
                        phase: PhaseRecord::Consolidation,
                        risk: RiskRecord {
                            exploration: LevelRecord::Low,
                            mutation: LevelRecord::Low,
                            finality: LevelRecord::Low,
                        },
                    },
                    opening_authority: OpeningAuthorityRecord::Genesis(GenesisAuthorityRecord {
                        bootstrap_policy: ProcedureRefRecord {
                            value: "policy:bootstrap".to_owned(),
                        },
                        tree_key: TreeKeyHashRecord {
                            hash: HistoryHash("a".repeat(64)),
                        },
                        parent_identity: ParentIdentityRefRecord {
                            evidence: evidence.clone(),
                        },
                    }),
                    opened_by: actor.clone(),
                    opened_from_artifact: artifact.clone(),
                    ruling_authority: actor.clone(),
                    policy_ref: ProcedureRefRecord {
                        value: "policy:prototype1".to_owned(),
                    },
                    surface: surface_commitment(),
                    opened_at: RecordedAt(block_height as i64),
                },
                crown_lock_transition: evidence,
                selected_successor: SuccessorRefRecord {
                    runtime: ActorRefRecord::Runtime(RuntimeId(format!("runtime:{selected}"))),
                    artifact: ArtifactRefRecord::from_artifact_id(ArtifactId(format!(
                        "artifact:{selected}"
                    ))),
                },
                active_artifact: artifact,
                claims: ClaimsRecord {
                    policy: None,
                    surface: None,
                    manifest: None,
                    artifact: None,
                },
                sealed_at: RecordedAt(block_height as i64 + 1),
                entry_count: 1,
                entries_root: HistoryHash("b".repeat(64)),
                block_hash: BlockHash(format!("{block_height:064x}")),
            },
            private: (),
        },
        entries: vec![selection_entry(block_height, selected)],
    }
}

fn selection_entry(block_height: u64, selected: &str) -> AdmittedEntryRecord {
    let actor = ActorRefRecord::Process(format!("parent:block:{block_height}"));
    let core = EntryCoreRecord {
        entry_id: EntryId(format!("entry-{block_height}")),
        entry_kind: EntryKindRecord::Decision,
        subject: ploke_records::history::SubjectRefRecord {
            value: format!("successor-selection:{block_height}"),
        },
        executor: actor.clone(),
        input_refs: Vec::new(),
        output_refs: Vec::new(),
        occurred_at: RecordedAt(block_height as i64),
        payload: EntryPayloadRecord::SelectionDecision(SelectionDecisionEntryRecord {
            schema_version: 1,
            procedure_or_policy: ProcedureRefRecord {
                value: "policy:select".to_owned(),
            },
            scope: SelectionScopeRecord {
                value: "test-scope".to_owned(),
            },
            selected_candidate: Some(ploke_records::history::SubjectRefRecord {
                value: format!("candidate:{selected}"),
            }),
            selected_occurrence_id: None,
            selected_membership_id: None,
            considered: Vec::new(),
            considered_sources: Vec::new(),
            considered_order_hash: HistoryHash("c".repeat(64)),
            candidate_set: None,
            projection_failures: Vec::new(),
            traversal: None,
            metrics: selection_metrics(selected, block_height),
            decision: selection::Decision {
                procedure_id: "policy:select".to_owned(),
                candidate_node_id: selected.to_owned(),
                selected_branch_id: Some(format!("branch-{selected}")),
                branch_disposition: "keep".to_owned(),
                outcome: Outcome::Accepted,
                findings: Vec::new(),
                rationale: Vec::new(),
            },
        }),
    };
    let state = AdmittedEntryStateRecord {
        observed: ploke_records::history::ObservedEntryRecord {
            observer: actor.clone(),
            recorder: actor.clone(),
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
                value: format!("payload:{block_height}"),
            },
            payload_hash: HistoryHash("d".repeat(64)),
            observed_at: RecordedAt(block_height as i64),
            recorded_at: RecordedAt(block_height as i64),
        },
        proposer: actor.clone(),
        procedure_or_policy: ProcedureRefRecord {
            value: "policy:select".to_owned(),
        },
        admitting_authority: actor.clone(),
        ruling_authority: actor,
        lineage_id: LineageId("lineage:test".to_owned()),
        block_id: BlockId(format!("block-{block_height}")),
        block_height,
    };
    AdmittedEntryRecord { core, state }
}

fn selection_metrics(candidate: &str, block_height: u64) -> selection::MetricSet {
    selection::MetricSet {
        schema_version: 1,
        id: HistoryHash(format!("metric-set-{block_height}")),
        considered_order_hash: HistoryHash("c".repeat(64)),
        candidate_set_root: None,
        policy: selection::MetricPolicy::default(),
        candidates: vec![selection::MetricCandidate {
            payload_index: 0,
            payload_hash: HistoryHash(format!("payload:{candidate}")),
            candidate: format!("candidate:{candidate}"),
            occurrence_id: None,
            membership_id: None,
            imp_at_k: None,
        }],
    }
}

fn surface_commitment() -> SurfaceCommitmentRecord {
    SurfaceCommitmentRecord {
        immutable: surface("1"),
        mutated: SurfaceDeltaRecord {
            before: surface("2"),
            after: surface("3"),
        },
        ambient: SurfaceDeltaRecord {
            before: surface("4"),
            after: surface("5"),
        },
    }
}

fn surface(seed: &str) -> SurfaceRecord {
    SurfaceRecord {
        root: SurfaceRootRecord {
            hash: HistoryHash(seed.repeat(64)),
        },
    }
}
