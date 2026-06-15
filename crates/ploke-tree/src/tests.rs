use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ploke_records::history::{
    ActorRefRecord, AdmittedEntryRecord, AdmittedEntryStateRecord, ArtifactRefRecord,
    BlockCommonRecord, CandidateSetMembershipRecord, CandidateSetProofRecord, CandidateSetRecord,
    CandidateSetRootRecord, ClaimsRecord, EntryCoreRecord, EntryKindRecord, EntryPayloadRecord,
    EvidenceRefRecord, GenesisAuthorityRecord, LevelRecord, OpeningAuthorityRecord,
    OperationalEnvironmentRecord, ParentIdentityRefRecord, PhaseRecord, ProcedureRefRecord,
    RegimeRecord, RiskRecord, SealedBlockHeaderRecord, SealedBlockRecord, SealedBlockStateRecord,
    SelectionDecisionEntryRecord, StepRecord, SubjectRefRecord, SuccessorRefRecord,
    SurfaceCommitmentRecord, SurfaceDeltaRecord, SurfaceRecord, SurfaceRootRecord,
    TreeKeyHashRecord,
};
use ploke_records::identity::ParentIdentityRecord;
use ploke_records::ids::{
    ArtifactId, BlockHash, BlockId, BranchId, CampaignId, CandidateId, CandidateMembershipId,
    CandidateOccurrenceId, EntryId, HistoryHash, HistoryStateRoot, InstanceId, LineageId,
    RecordedAt, RuntimeId, SchedulerNodeId, SourceStateId,
};
use ploke_records::invocation::{InvocationRecord, Role};
use ploke_records::journal::JournalEntry;
use ploke_records::playback::{EvidenceStrength, FineStepKind, RunPlaybackRef};
use ploke_records::scheduler::{RunnerRequestRecord, RunnerResultRecord, SchedulerStateRecord};
use ploke_records::selection::{Decision, MetricCandidate, MetricPolicy, MetricSet, Outcome};

use super::*;

#[test]
#[ignore = "real-run diagnostic; reads ~/.ploke-eval campaign records"]
fn real_run_run_forest_node_ids_are_not_artifact_tree_ids() {
    let run_root =
        Path::new("/home/brasides/.ploke-eval/campaigns/p1-broad-smoke-3x4-20260515-1/prototype1");
    let parent_root =
        Path::new("/home/brasides/.ploke-eval/worktrees/p1-broad-smoke-3x4-20260515-1");
    assert!(
        run_root.join("scheduler.json").is_file(),
        "missing real run scheduler at {}",
        run_root.display()
    );

    let store = FsRunStore::new(run_root).with_parent_root(parent_root);
    let records = store
        .load_record_set()
        .expect("load typed real-run records");
    let graph = Graph::from_records(&records);
    let forest = graph.forest.as_ref().expect("graph carries RunForest");
    let artifact_tree = graph.artifact_tree();

    let artifact_keys = artifact_tree
        .nodes
        .keys()
        .copied()
        .map(|key| key.as_str())
        .collect::<BTreeSet<_>>();

    let direct_matches = forest
        .nodes
        .iter()
        .map(|node| normalize_artifact_key(node.key.as_str()))
        .filter(|node_key| artifact_keys.contains(node_key))
        .collect::<Vec<_>>();

    let mut carried_artifact_refs = Vec::new();
    for node in &forest.nodes {
        if let Some(id) = node.base_artifact_id.as_deref() {
            carried_artifact_refs.push((
                node.key.as_str(),
                "base_artifact_id",
                normalize_artifact_key(id),
            ));
        }
        if let Some(id) = node.derived_artifact_id.as_deref() {
            carried_artifact_refs.push((
                node.key.as_str(),
                "derived_artifact_id",
                normalize_artifact_key(id),
            ));
        }
    }

    let carried_matches = carried_artifact_refs
        .iter()
        .copied()
        .filter(|(_, _, artifact_id)| artifact_keys.contains(artifact_id))
        .collect::<Vec<_>>();

    eprintln!("run={}", run_root.display());
    eprintln!(
        "run_forest_nodes={} artifact_tree_nodes={} direct_node_id_matches={}",
        forest.nodes.len(),
        artifact_tree.nodes.len(),
        direct_matches.len()
    );
    eprintln!(
        "run_forest_carried_artifact_refs={} carried_refs_present_in_artifact_tree={}",
        carried_artifact_refs.len(),
        carried_matches.len()
    );
    eprintln!(
        "sample_run_forest_node_ids={:?}",
        forest
            .nodes
            .iter()
            .map(|node| node.key.as_str())
            .take(8)
            .collect::<Vec<_>>()
    );
    eprintln!(
        "sample_artifact_tree_ids={:?}",
        artifact_keys.iter().copied().take(8).collect::<Vec<_>>()
    );
    eprintln!(
        "sample_carried_artifact_refs={:?}",
        carried_matches.iter().copied().take(12).collect::<Vec<_>>()
    );

    assert!(
        !forest.nodes.is_empty(),
        "real run should contain run forest nodes"
    );
    assert!(
        !artifact_tree.nodes.is_empty(),
        "real run should contain artifact tree nodes"
    );
    assert!(
        direct_matches.is_empty(),
        "RunForest node ids unexpectedly matched ArtifactTree artifact ids: {direct_matches:?}"
    );
    assert!(
        !carried_matches.is_empty(),
        "RunForest nodes should carry artifact ids that resolve into the ArtifactTree"
    );
}

fn normalize_artifact_key(value: &str) -> &str {
    value.strip_prefix("artifact:").unwrap_or(value)
}

#[test]
fn scheduler_nodes_become_mutable_projection_tree_nodes() {
    let forest = assemble_run_forest(RunForestInput {
        scheduler: scheduler(vec![node("node-0", None, NodeStatusRecord::Running)]),
        node_records: Vec::new(),
        parent_identity: None,
        successor_ready: Vec::new(),
        successor_completion: Vec::new(),
        passive_evidence: PassiveEvidence::default(),
    });

    assert_eq!(forest.roots, vec![NodeKey::from("node-0")]);
    assert_eq!(forest.nodes.len(), 1);
    assert_eq!(forest.nodes[0].authority, AuthorityLabel::MutableProjection);
    assert_eq!(forest.nodes[0].progress.phase, Phase::Running);
    assert_eq!(
        forest.nodes[0].evidence[0].authority,
        AuthorityLabel::MutableProjection
    );
}

#[test]
fn parent_child_relationships_use_only_explicit_scheduler_parent_fields() {
    let mut branch_only = node("branch-only", None, NodeStatusRecord::Planned);
    branch_only.parent_branch_id = Some(BranchId("branch-root".to_owned()));

    let forest = assemble_run_forest(RunForestInput {
        scheduler: scheduler(vec![
            node("root", None, NodeStatusRecord::Succeeded),
            node("child", Some("root"), NodeStatusRecord::Planned),
            branch_only,
        ]),
        node_records: Vec::new(),
        parent_identity: None,
        successor_ready: Vec::new(),
        successor_completion: Vec::new(),
        passive_evidence: PassiveEvidence::default(),
    });

    let root = forest.node("root");
    let child = forest.node("child");
    let branch_only = forest.node("branch-only");

    assert_eq!(
        forest.roots,
        vec![NodeKey::from("root"), NodeKey::from("branch-only")]
    );
    assert_eq!(root.children, vec![NodeKey::from("child")]);
    assert_eq!(child.parent, Some(NodeKey::from("root")));
    assert_eq!(branch_only.parent_branch_id, Some("branch-root".to_owned()));
    assert_eq!(branch_only.parent, None);
    assert!(branch_only.children.is_empty());
}

#[test]
fn missing_scheduler_parent_stays_root_with_diagnostic() {
    let forest = assemble_run_forest(RunForestInput {
        scheduler: scheduler(vec![node(
            "child",
            Some("missing"),
            NodeStatusRecord::Planned,
        )]),
        node_records: Vec::new(),
        parent_identity: None,
        successor_ready: Vec::new(),
        successor_completion: Vec::new(),
        passive_evidence: PassiveEvidence::default(),
    });

    let child = forest.node("child");
    assert_eq!(forest.roots, vec![NodeKey::from("child")]);
    assert_eq!(child.parent, None);
    assert_eq!(child.diagnostics[0].code, "missing_scheduler_parent");
}

#[test]
fn successor_records_attach_evidence_without_history_authority() {
    let forest = assemble_run_forest(RunForestInput {
        scheduler: scheduler(vec![node("node-1", None, NodeStatusRecord::Succeeded)]),
        node_records: Vec::new(),
        parent_identity: None,
        successor_ready: vec![SuccessorReadyRecord {
            schema_version: "prototype1-successor-ready.v1".to_owned(),
            campaign_id: CampaignId::from("campaign-1"),
            node_id: "node-1".to_owned(),
            runtime_id: RuntimeId("runtime-1".to_owned()),
            pid: 42,
            recorded_at: "2026-05-08T12:02:00Z".to_owned(),
        }],
        successor_completion: vec![SuccessorCompletionRecord {
            schema_version: "prototype1-successor-completion.v1".to_owned(),
            campaign_id: CampaignId::from("campaign-1"),
            node_id: "node-1".to_owned(),
            runtime_id: RuntimeId("runtime-1".to_owned()),
            status: SuccessorCompletionStatus::Succeeded,
            trace_path: None,
            detail: None,
            recorded_at: "2026-05-08T12:03:00Z".to_owned(),
        }],
        passive_evidence: PassiveEvidence::default(),
    });

    let node = forest.node("node-1");
    assert_eq!(node.authority, AuthorityLabel::MutableProjection);
    assert!(
        node.evidence
            .iter()
            .any(|evidence| evidence.kind == EvidenceKind::SuccessorReady
                && evidence.authority == AuthorityLabel::TypedRecordEvidence)
    );
    assert!(node.evidence.iter().any(|evidence| evidence.kind
        == EvidenceKind::SuccessorCompletion
        && evidence.authority == AuthorityLabel::TypedRecordEvidence));
    assert!(
        !node
            .evidence
            .iter()
            .any(|evidence| evidence.authority == AuthorityLabel::SealedVerifiedHistory)
    );
}

#[test]
fn successor_completion_failure_adds_diagnostic_not_history_authority() {
    let forest = assemble_run_forest(RunForestInput {
        scheduler: scheduler(vec![node("node-1", None, NodeStatusRecord::Succeeded)]),
        node_records: Vec::new(),
        parent_identity: None,
        successor_ready: Vec::new(),
        successor_completion: vec![SuccessorCompletionRecord {
            schema_version: "prototype1-successor-completion.v1".to_owned(),
            campaign_id: CampaignId::from("campaign-1"),
            node_id: "node-1".to_owned(),
            runtime_id: RuntimeId("runtime-1".to_owned()),
            status: SuccessorCompletionStatus::Failed,
            trace_path: None,
            detail: Some("rehydration failed".to_owned()),
            recorded_at: "2026-05-08T12:03:00Z".to_owned(),
        }],
        passive_evidence: PassiveEvidence::default(),
    });

    let node = forest.node("node-1");
    assert_eq!(node.authority, AuthorityLabel::MutableProjection);
    assert_eq!(node.diagnostics[0].code, "successor_completion_failed");
    assert_eq!(
        node.diagnostics[0].evidence[0].authority,
        AuthorityLabel::TypedRecordEvidence
    );
}

#[test]
fn run_forest_dto_serializes_to_json() {
    let forest = assemble_run_forest(RunForestInput {
        scheduler: scheduler(vec![node("node-0", None, NodeStatusRecord::Running)]),
        node_records: Vec::new(),
        parent_identity: None,
        successor_ready: Vec::new(),
        successor_completion: Vec::new(),
        passive_evidence: PassiveEvidence::default(),
    });

    let json = serde_json::to_value(&forest).expect("serialize forest");
    assert_eq!(json["nodes"][0]["authority"], "mutable_projection");
}

#[test]
fn separate_node_records_supplement_and_override_scheduler_nodes() {
    let mut root_override = node("root", None, NodeStatusRecord::Succeeded);
    root_override.generation = 0;

    let forest = assemble_run_forest(RunForestInput {
        scheduler: scheduler(vec![node("root", None, NodeStatusRecord::Planned)]),
        node_records: vec![
            root_override,
            node("child", Some("root"), NodeStatusRecord::Running),
        ],
        parent_identity: None,
        successor_ready: Vec::new(),
        successor_completion: Vec::new(),
        passive_evidence: PassiveEvidence::default(),
    });

    assert_eq!(forest.nodes.len(), 2);
    assert_eq!(forest.node("root").progress.phase, Phase::Completed);
    assert_eq!(forest.node("root").children, vec![NodeKey::from("child")]);
    assert_eq!(forest.node("child").parent, Some(NodeKey::from("root")));
    assert!(forest.lanes.frontier.is_empty());
}

#[test]
fn coarse_history_spine_orders_by_height_preserves_selection_and_emits_warnings() {
    let block_zero =
        synthetic_sealed_block(0, vec![], "hash-0", Some(("candidate-0", 2)), "runtime-0");
    let block_one = synthetic_sealed_block(
        1,
        vec!["hash-0"],
        "hash-1",
        Some(("candidate-1", 3)),
        "runtime-1",
    );
    let block_two =
        synthetic_sealed_block(2, vec!["unexpected-parent"], "hash-2", None, "runtime-2");

    let spine =
        build_coarse_history_spine(&[block_two.clone(), block_zero.clone(), block_one.clone()]);
    let CoarseHistorySpine { steps, warnings } = spine;
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].block_height, 0);
    assert_eq!(steps[1].block_height, 1);
    assert_eq!(steps[2].block_height, 2);
    assert_eq!(steps[1].parent_block_hashes, vec!["hash-0".to_owned()]);
    assert_eq!(
        steps[2].parent_block_hashes,
        vec!["unexpected-parent".to_owned()]
    );
    assert_eq!(steps[1].selected_candidate.as_deref(), Some("candidate-1"));
    assert_eq!(steps[1].considered_candidate_count, 3);
    assert_eq!(steps[2].selected_candidate, None);
    assert_eq!(steps[2].considered_candidate_count, 0);
    assert_eq!(
        warnings,
        vec![
            CoarseHistoryWarning::ParentHashLinkMismatch {
                previous_block_height: 1,
                previous_block_hash: "hash-1".to_owned(),
                block_height: 2,
                block_hash: "hash-2".to_owned(),
                parent_block_hashes: vec!["unexpected-parent".to_owned()],
            },
            CoarseHistoryWarning::MissingSelectionDecisionPayload {
                block_height: 2,
                block_hash: "hash-2".to_owned(),
            },
        ]
    );

    let projected_steps =
        project_coarse_history_spine(&[block_two.clone(), block_zero.clone(), block_one]);
    assert_eq!(projected_steps, steps);

    match &steps[0].selected_successor.runtime {
        ActorRefRecord::Runtime(runtime) => assert_eq!(runtime.0, "runtime-0"),
        other => panic!("expected runtime successor, got {other:?}"),
    }

    let playback =
        coarse_run_playback_from_sealed_history(&[block_two.clone(), block_zero.clone()]);
    let ids = playback
        .iter()
        .map(|step| step.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["hash-0", "hash-2"]);
    assert!(
        playback
            .iter()
            .all(|step| step.evidence == EvidenceStrength::SealedHistory)
    );

    let ref_blocks = [block_two.clone(), block_zero.clone()];
    let playback_ref_steps = coarse_run_playback_ref_steps_from_sealed_history(&ref_blocks);
    let playback_ref =
        RunPlaybackRef::<ploke_records::playback::Coarse>::new(playback_ref_steps.as_slice());
    let ref_ids = playback_ref.iter().map(|step| step.id).collect::<Vec<_>>();
    assert_eq!(ref_ids, vec!["hash-0", "hash-2"]);
    assert!(
        playback_ref
            .into_iter()
            .all(|step| step.evidence == EvidenceStrength::SealedHistory)
    );

    let fine = fine_run_playback_from_sealed_history(&[block_zero.clone(), block_two.clone()]);
    let kinds = fine.iter().map(|step| step.kind).collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            FineStepKind::CandidateConsidered,
            FineStepKind::CandidateConsidered,
            FineStepKind::SuccessorSelected,
            FineStepKind::HistoryEntryAdmitted,
            FineStepKind::HistoryBlockSealed,
            FineStepKind::HistoryBlockSealed,
        ]
    );
    assert!(
        fine.iter()
            .all(|step| step.evidence >= EvidenceStrength::AdmittedHistory)
    );
    assert!(
        fine.iter()
            .map(|step| step.order)
            .collect::<Vec<_>>()
            .windows(2)
            .all(|pair| pair[0] <= pair[1]),
        "fine playback must preserve causal order keys"
    );

    let fine_ref_steps = fine_run_playback_ref_steps_from_sealed_history(&ref_blocks);
    let fine_ref = RunPlaybackRef::<ploke_records::playback::Fine>::new(fine_ref_steps.as_slice());
    assert_eq!(
        fine_ref.iter().map(|step| step.kind).collect::<Vec<_>>(),
        kinds
    );
    assert_eq!(
        fine_ref
            .iter()
            .map(|step| step.id.as_str())
            .collect::<Vec<_>>(),
        fine.iter().map(|step| step.id.as_str()).collect::<Vec<_>>()
    );
}

#[test]
fn history_playback_projects_occurrence_membership_identity() {
    let occurrence_id = CandidateOccurrenceId("a".repeat(64));
    let membership_id = CandidateMembershipId("b".repeat(64));
    let mut block = synthetic_sealed_block(
        0,
        vec![],
        "hash-0",
        Some(("candidate:node-a:plan_index=0", 1)),
        "runtime-0",
    );

    let selection = block
        .entries
        .iter_mut()
        .find_map(|entry| match &mut entry.core.payload {
            EntryPayloadRecord::SelectionDecision(selection) => Some(selection),
            _ => None,
        })
        .expect("selection entry");
    selection.selected_occurrence_id = Some(occurrence_id.clone());
    selection.selected_membership_id = Some(membership_id.clone());
    selection.candidate_set = Some(CandidateSetRecord {
        root: CandidateSetRootRecord(HistoryHash("c".repeat(64))),
        memberships: vec![CandidateSetMembershipRecord {
            candidate: SubjectRefRecord {
                value: "candidate:node-a:plan_index=0-0".to_owned(),
            },
            payload_hash: HistoryHash("d".repeat(64)),
            occurrence_id: Some(occurrence_id.clone()),
            membership_id: Some(membership_id.clone()),
            proof: CandidateSetProofRecord {
                key: [0; 32],
                value: [1; 32],
                program: Vec::new(),
            },
        }],
    });

    let spine = build_coarse_history_spine(&[block.clone()]);
    assert_eq!(
        spine.steps[0].selected_occurrence_id.as_deref(),
        Some(occurrence_id.0.as_str())
    );
    assert_eq!(
        spine.steps[0].selected_membership_id.as_deref(),
        Some(membership_id.0.as_str())
    );

    let fine = fine_run_playback_from_sealed_history(&[block]);
    let candidate = fine
        .iter()
        .find(|step| step.kind == FineStepKind::CandidateConsidered)
        .expect("candidate step");
    assert_eq!(
        candidate.occurrence_id.as_deref(),
        Some(occurrence_id.0.as_str())
    );
    assert_eq!(
        candidate.membership_id.as_deref(),
        Some(membership_id.0.as_str())
    );
    assert!(candidate.id.contains("candidate-membership:"));

    let selected = fine
        .iter()
        .find(|step| step.kind == FineStepKind::SuccessorSelected)
        .expect("selected step");
    assert_eq!(
        selected.occurrence_id.as_deref(),
        Some(occurrence_id.0.as_str())
    );
    assert_eq!(
        selected.membership_id.as_deref(),
        Some(membership_id.0.as_str())
    );
}

#[test]
fn fs_run_store_loads_synthetic_run_read_only_counts() {
    let root = temp_run_root("synthetic");
    fs::create_dir_all(root.join("nodes").join("root")).expect("create root node dir");
    fs::create_dir_all(root.join("nodes").join("child")).expect("create child node dir");
    fs::create_dir_all(root.join("history").join("blocks")).expect("create history dir");
    fs::create_dir_all(
        root.join("nodes")
            .join("child")
            .join("channels")
            .join("runtime-1"),
    )
    .expect("create channel dir");

    write_json(
        &root.join("scheduler.json"),
        &scheduler(vec![node("root", None, NodeStatusRecord::Planned)]),
    );
    write_json(
        &root.join("nodes").join("root").join("node.json"),
        &node("root", None, NodeStatusRecord::Succeeded),
    );
    write_json(
        &root.join("nodes").join("child").join("node.json"),
        &node("child", Some("root"), NodeStatusRecord::Running),
    );
    write_json(&root.join("branches.json"), &minimal_branch_registry());
    fs::write(
        root.join("transition-journal.jsonl"),
        format!("{}\nnot-json\n", minimal_journal_line()),
    )
    .expect("write journal");
    fs::write(
        root.join("history")
            .join("blocks")
            .join("segment-000000.jsonl"),
        r#"{"state":{},"entries":[{},{}]}"#,
    )
    .expect("write history");
    fs::write(
        root.join("nodes")
            .join("child")
            .join("channels")
            .join("runtime-1")
            .join("child-to-parent.jsonl"),
        format!("{}\n", minimal_channel_line()),
    )
    .expect("write channel");
    fs::create_dir_all(root.join("evaluations")).expect("create evaluations dir");
    fs::write(
        root.join("evaluations").join("branch-synthetic-1.json"),
        r#"{
  "baseline_campaign_id": "campaign-0",
  "branch_id": "branch-synthetic-1",
  "treatment_campaign_id": "campaign-1",
  "branch_registry_path": "branches.json",
  "evaluation_artifact_path": "evaluations/branch-synthetic-1.json",
  "treatment_campaign_manifest": "nodes/child/manifest.json",
  "treatment_closure_state_path": "nodes/child/closure-state.json",
  "overall_disposition": "keep",
  "reasons": [],
  "compared_instances": [
    {
      "instance_id": "instance-1",
      "baseline_metrics": {
        "tool_calls_total": 3,
        "tool_calls_failed": 0,
        "patch_attempted": false,
        "patch_apply_state": "no",
        "submission_artifact_state": "nonempty",
        "partial_patch_failures": 0,
        "same_file_patch_retry_count": 0,
        "same_file_patch_max_streak": 0,
        "aborted": false,
        "aborted_repair_loop": false,
        "nonempty_valid_patch": true,
        "convergence": true,
        "oracle_eligible": true
      },
      "treatment_metrics": {
        "tool_calls_total": 5,
        "tool_calls_failed": 1,
        "patch_attempted": true,
        "patch_apply_state": "yes",
        "submission_artifact_state": "empty",
        "partial_patch_failures": 0,
        "same_file_patch_retry_count": 1,
        "same_file_patch_max_streak": 1,
        "aborted": true,
        "aborted_repair_loop": false,
        "nonempty_valid_patch": false,
        "convergence": false,
        "oracle_eligible": false
      },
      "evaluation": {
        "disposition": "keep",
        "reasons": []
      },
      "status": "compared"
    }
  ]
}"#,
    )
    .expect("write synthetic evaluation");

    let parent_identity_path = root.join("parent_identity.json");
    write_json(&parent_identity_path, &parent_identity("root", 0));

    let forest = FsRunStore::new(&root)
        .with_parent_identity_path(&parent_identity_path)
        .load_forest()
        .expect("load forest");

    assert_eq!(forest.nodes.len(), 2);
    assert_eq!(forest.node("root").progress.phase, Phase::Completed);
    assert!(forest.node("root").evidence.iter().any(|evidence| {
        evidence.kind == EvidenceKind::ParentIdentity
            && evidence.authority == AuthorityLabel::TypedRecordEvidence
    }));
    assert_eq!(
        forest
            .passive_evidence
            .branch_registry
            .as_ref()
            .expect("branch evidence")
            .source_node_count,
        0
    );
    assert_eq!(
        forest
            .passive_evidence
            .transition_journal
            .as_ref()
            .expect("journal evidence")
            .parsed_count,
        1
    );
    assert_eq!(
        forest
            .passive_evidence
            .transition_journal
            .as_ref()
            .expect("journal evidence")
            .parse_error_count,
        1
    );
    assert_eq!(
        forest
            .passive_evidence
            .history
            .as_ref()
            .expect("history evidence")
            .admitted_entry_count,
        0
    );
    assert_eq!(
        forest
            .passive_evidence
            .history
            .as_ref()
            .expect("history evidence")
            .record_parse_error_count,
        1
    );
    assert_eq!(
        forest
            .passive_evidence
            .channel_envelopes
            .as_ref()
            .expect("channel evidence")
            .parsed_count,
        1
    );
    let evaluations = forest
        .passive_evidence
        .evaluations
        .as_ref()
        .expect("evaluation evidence");
    assert_eq!(evaluations.summary.file_count, 1);
    assert_eq!(evaluations.summary.parsed_count, 1);
    assert_eq!(evaluations.summary.keep_count, 1);
    assert_eq!(evaluations.summary.reject_count, 0);
    let artifact = evaluations
        .index
        .get("branch-synthetic-1")
        .expect("branch-synthetic-1");
    let compared = artifact
        .compared_instances
        .first()
        .expect("one synthetic compared instance");
    assert_eq!(
        compared
            .baseline_metrics
            .as_ref()
            .expect("baseline metrics")
            .tool_calls_total,
        3
    );
    assert!(
        compared
            .treatment_metrics
            .as_ref()
            .expect("treatment metrics")
            .aborted
    );
    assert_eq!(
        compared
            .evaluation
            .as_ref()
            .expect("evaluation")
            .disposition,
        ploke_records::branch::Disposition::Keep
    );
    assert!(forest.passive_evidence.protocol_artifacts.is_none());
    assert_no_sealed_history_authority(&forest);

    fs::remove_dir_all(root).expect("remove temp run");
}

#[test]
fn fs_run_store_loads_protocol_artifacts_from_evaluation_record_paths() {
    let root = temp_run_root("protocol-from-evaluations");
    let baseline_run = root
        .join("bench")
        .join("baseline")
        .join("runs")
        .join("run-a");
    let treatment_run = root
        .join("bench")
        .join("treatments")
        .join("branch-1")
        .join("runs")
        .join("run-b");
    let baseline_protocol_dir = baseline_run.join("protocol-artifacts");
    let treatment_protocol_dir = treatment_run.join("protocol-artifacts");

    fs::create_dir_all(root.join("evaluations")).expect("create evaluations dir");
    fs::create_dir_all(&baseline_protocol_dir).expect("create baseline protocol dir");
    fs::create_dir_all(&treatment_protocol_dir).expect("create treatment protocol dir");

    write_json(
        &root.join("scheduler.json"),
        &scheduler(vec![node("root", None, NodeStatusRecord::Succeeded)]),
    );
    write_json(
        &root.join("evaluations").join("branch-1.json"),
        &serde_json::json!({
            "baseline_campaign_id": "campaign-0",
            "branch_id": "branch-1",
            "treatment_campaign_id": "campaign-1",
            "branch_registry_path": "branches.json",
            "evaluation_artifact_path": "evaluations/branch-1.json",
            "treatment_campaign_manifest": "campaign.json",
            "treatment_closure_state_path": "closure-state.json",
            "overall_disposition": "keep",
            "compared_instances": [{
                "instance_id": "instance-1",
                "baseline_record_path": baseline_run.join("record.json.gz"),
                "treatment_record_path": treatment_run.join("record.json.gz"),
                "status": "compared"
            }]
        }),
    );
    write_json(
        &baseline_protocol_dir.join("1000_intervention_issue_detection_instance-1.json"),
        &protocol_issue_detection_artifact("run-a", "instance-1", 1000),
    );
    write_json(
        &treatment_protocol_dir.join("2000_intervention_issue_detection_instance-1.json"),
        &protocol_issue_detection_artifact("run-b", "instance-1", 2000),
    );

    let records = FsRunStore::new(&root)
        .load_record_set()
        .expect("load record set with protocol artifacts");
    let protocol_artifacts = records
        .forest_input
        .passive_evidence
        .protocol_artifacts
        .as_ref()
        .expect("protocol artifact evidence");

    assert_eq!(protocol_artifacts.summary.file_count, 2);
    assert_eq!(protocol_artifacts.summary.parsed_count, 2);
    assert_eq!(protocol_artifacts.summary.typed_payload_count, 2);
    assert!(
        protocol_artifacts.index.contains_key(
            &baseline_protocol_dir
                .join("1000_intervention_issue_detection_instance-1.json")
                .to_string_lossy()
                .to_string()
        )
    );
    assert!(
        protocol_artifacts.index.contains_key(
            &treatment_protocol_dir
                .join("2000_intervention_issue_detection_instance-1.json")
                .to_string_lossy()
                .to_string()
        )
    );

    let graph = Graph::from_records(&records);
    let dirs = graph.protocol_artifact_dirs();
    assert!(dirs.contains(&baseline_protocol_dir));
    assert!(dirs.contains(&treatment_protocol_dir));
    assert_eq!(
        graph
            .protocol_artifacts()
            .expect("graph carries protocol artifacts")
            .summary
            .parsed_count,
        2
    );

    fs::remove_dir_all(root).expect("remove temp run");
}

#[test]
fn fs_run_store_loads_compressed_run_records_from_evaluation_record_paths() {
    let root = temp_run_root("run-records-from-evaluations");
    let baseline_run = root
        .join("bench")
        .join("baseline")
        .join("runs")
        .join("run-a");
    let treatment_run = root
        .join("bench")
        .join("treatments")
        .join("branch-1")
        .join("runs")
        .join("run-b");
    let baseline_record_path = baseline_run.join("record.json.gz");
    let treatment_record_path = treatment_run.join("record.json.gz");

    fs::create_dir_all(root.join("evaluations")).expect("create evaluations dir");
    fs::create_dir_all(&baseline_run).expect("create baseline run dir");
    fs::create_dir_all(&treatment_run).expect("create treatment run dir");

    write_json(
        &root.join("scheduler.json"),
        &scheduler(vec![node("root", None, NodeStatusRecord::Succeeded)]),
    );
    write_json(
        &root.join("evaluations").join("branch-1.json"),
        &serde_json::json!({
            "baseline_campaign_id": "campaign-0",
            "branch_id": "branch-1",
            "treatment_campaign_id": "campaign-1",
            "branch_registry_path": "branches.json",
            "evaluation_artifact_path": "evaluations/branch-1.json",
            "treatment_campaign_manifest": "campaign.json",
            "treatment_closure_state_path": "closure-state.json",
            "overall_disposition": "reject",
            "compared_instances": [{
                "instance_id": "instance-1",
                "baseline_record_path": baseline_record_path,
                "treatment_record_path": treatment_record_path,
                "status": "compared"
            }]
        }),
    );
    write_gzip_json(
        &baseline_record_path,
        &minimal_run_record_json("instance-1", "baseline-run", "nonempty", 2, 1),
    );
    write_gzip_json(
        &treatment_record_path,
        &minimal_run_record_json("instance-1", "treatment-run", "empty", 1, 0),
    );

    let records = FsRunStore::new(&root)
        .load_record_set()
        .expect("load record set with compressed run records");
    let run_records = records
        .forest_input
        .passive_evidence
        .run_records
        .as_ref()
        .expect("run record evidence");

    assert_eq!(run_records.summary.file_count, 2);
    assert_eq!(run_records.summary.parsed_count, 2);
    assert_eq!(run_records.summary.branch_ref_count, 2);
    assert_eq!(run_records.summary.baseline_ref_count, 1);
    assert_eq!(run_records.summary.treatment_ref_count, 1);
    assert_eq!(run_records.summary.records_with_setup_count, 2);
    assert_eq!(run_records.summary.records_with_packaging_count, 2);
    assert_eq!(run_records.summary.total_turn_count, 2);
    assert_eq!(run_records.summary.total_tool_call_count, 3);
    assert_eq!(run_records.summary.failed_tool_call_count, 1);
    assert_eq!(run_records.stats.len(), 2);

    let branch_refs = run_records
        .refs_by_branch
        .get("branch-1")
        .expect("branch run record refs");
    assert_eq!(branch_refs.len(), 2);
    assert!(branch_refs.iter().any(|record_ref| {
        record_ref.arm == ComparedRunArm::Baseline
            && run_records
                .stats
                .get(&record_ref.record_key)
                .is_some_and(|stats| {
                    stats.turn_count == 1
                        && stats.tool_call_count == 2
                        && stats.failed_tool_call_count == 1
                })
            && run_records
                .index
                .get(&record_ref.record_key)
                .is_some_and(|record| record.manifest_id == "baseline-run")
    }));
    assert!(branch_refs.iter().any(|record_ref| {
        record_ref.arm == ComparedRunArm::Treatment
            && run_records
                .stats
                .get(&record_ref.record_key)
                .is_some_and(|stats| {
                    stats.turn_count == 1
                        && stats.tool_call_count == 1
                        && stats.failed_tool_call_count == 0
                })
            && run_records
                .index
                .get(&record_ref.record_key)
                .is_some_and(|record| {
                    record.manifest_id == "treatment-run"
                        && record.phases.packaging.as_ref().is_some_and(|packaging| {
                            packaging.submission_artifact_state
                                == ploke_records::run_record::SubmissionArtifactState::Empty
                        })
                })
    }));

    let graph = Graph::from_records(&records);
    let graph_records = graph.run_records().expect("graph carries run records");
    assert_eq!(graph_records.summary.parsed_count, 2);
    assert_eq!(
        graph
            .run_record_refs_for_branch("branch-1")
            .map(|record_ref| record_ref.arm)
            .collect::<Vec<_>>(),
        vec![ComparedRunArm::Baseline, ComparedRunArm::Treatment]
    );

    fs::remove_dir_all(root).expect("remove temp run");
}

#[test]
#[ignore = "real-run diagnostic; reads ~/.ploke-eval campaign records"]
fn real_run_p1_five_gen_1x3_protocol_artifact_dirs() {
    let run_root =
        Path::new("/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1");
    assert!(
        run_root.join("scheduler.json").is_file(),
        "missing real run scheduler at {}",
        run_root.display()
    );

    let records = FsRunStore::new(run_root)
        .load_record_set()
        .expect("load typed real-run records");
    let graph = Graph::from_records(&records);
    let dirs = graph.protocol_artifact_dirs();
    let evaluations = records
        .forest_input
        .passive_evidence
        .evaluations
        .as_ref()
        .expect("real run carries evaluation evidence");

    let compared_instances = evaluations
        .index
        .values()
        .map(|evaluation| evaluation.compared_instances.len())
        .sum::<usize>();
    let existing_dirs = dirs.iter().filter(|dir| dir.is_dir()).count();

    eprintln!("run_root={}", run_root.display());
    eprintln!(
        "evaluations={} compared_instances={} protocol_artifact_dirs={} existing_dirs={}",
        evaluations.index.len(),
        compared_instances,
        dirs.len(),
        existing_dirs
    );

    match records
        .forest_input
        .passive_evidence
        .protocol_artifacts
        .as_ref()
    {
        Some(protocol_artifacts) => eprintln!(
            "loaded_protocol_artifacts file_count={} parsed_count={} typed_payload_count={}",
            protocol_artifacts.summary.file_count,
            protocol_artifacts.summary.parsed_count,
            protocol_artifacts.summary.typed_payload_count
        ),
        None => eprintln!("loaded_protocol_artifacts=none"),
    }

    for (branch_id, evaluation) in &evaluations.index {
        eprintln!(
            "evaluation branch_id={} evaluation_artifact_path={}",
            branch_id,
            evaluation.evaluation_artifact_path.display()
        );
        for compared in &evaluation.compared_instances {
            eprintln!("  instance_id={}", compared.instance_id);
            match compared.baseline_record_path.as_ref() {
                Some(record_path) => {
                    eprintln!("    baseline_record_path={}", record_path.display());
                    if let Some(run_dir) = record_path.parent() {
                        eprintln!(
                            "    baseline_protocol_artifact_dir={}",
                            run_dir.join("protocol-artifacts").display()
                        );
                    }
                }
                None => eprintln!("    baseline_record_path=<none>"),
            }
            match compared.treatment_record_path.as_ref() {
                Some(record_path) => {
                    eprintln!("    treatment_record_path={}", record_path.display());
                    if let Some(run_dir) = record_path.parent() {
                        eprintln!(
                            "    treatment_protocol_artifact_dir={}",
                            run_dir.join("protocol-artifacts").display()
                        );
                    }
                }
                None => eprintln!("    treatment_record_path=<none>"),
            }
        }
    }

    for dir in &dirs {
        eprintln!(
            "protocol_artifact_dir exists={} {}",
            dir.is_dir(),
            dir.display()
        );
    }

    assert!(
        !dirs.is_empty(),
        "expected protocol artifact dirs from evaluations"
    );
    assert!(
        dirs.iter()
            .all(|dir| dir.file_name().and_then(|name| name.to_str()) == Some("protocol-artifacts")),
        "all derived dirs should be protocol-artifacts directories"
    );
}

#[test]
#[ignore = "real-run diagnostic; reads ~/.ploke-eval campaign records"]
fn real_run_p1_five_gen_1x3_loads_compressed_run_records() {
    let run_root =
        Path::new("/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1");
    assert!(
        run_root.join("scheduler.json").is_file(),
        "missing real run scheduler at {}",
        run_root.display()
    );

    let records = FsRunStore::new(run_root)
        .load_record_set()
        .expect("load typed real-run records");
    let graph = Graph::from_records(&records);
    let run_records = graph.run_records().expect("real run carries run records");

    eprintln!("run_root={}", run_root.display());
    eprintln!(
        "loaded_run_records file_count={} parsed_count={} branch_refs={} baseline_refs={} treatment_refs={} setup={} packaging={} turns={} tool_calls={} failed_tool_calls={}",
        run_records.summary.file_count,
        run_records.summary.parsed_count,
        run_records.summary.branch_ref_count,
        run_records.summary.baseline_ref_count,
        run_records.summary.treatment_ref_count,
        run_records.summary.records_with_setup_count,
        run_records.summary.records_with_packaging_count,
        run_records.summary.total_turn_count,
        run_records.summary.total_tool_call_count,
        run_records.summary.failed_tool_call_count
    );

    let branch_id = "branch-b53a075a894cedf9";
    let branch_refs = graph
        .run_record_refs_for_branch(branch_id)
        .collect::<Vec<_>>();
    eprintln!(
        "branch_id={} run_record_refs={}",
        branch_id,
        branch_refs.len()
    );
    assert_eq!(
        branch_refs.len(),
        2,
        "expected baseline and treatment records for {branch_id}"
    );

    for record_ref in branch_refs {
        let record = run_records
            .index
            .get(&record_ref.record_key)
            .expect("loaded branch record");
        let stats = run_records
            .stats
            .get(&record_ref.record_key)
            .expect("loaded branch record stats");
        eprintln!(
            "  arm={:?} instance_id={} record_path={}",
            record_ref.arm,
            record_ref.instance_id,
            record_ref.record_path.display()
        );
        eprintln!(
            "    manifest_id={} model={:?} provider={:?} repo_root={} turns={} tool_calls={} failed_tool_calls={}",
            record.manifest_id,
            record.metadata.agent.model_id,
            record.metadata.agent.provider,
            record.metadata.benchmark.repo_root.display(),
            stats.turn_count,
            stats.tool_call_count,
            stats.failed_tool_call_count
        );
        if let Some(packaging) = record.phases.packaging.as_ref() {
            eprintln!(
                "    packaging submission={:?} patch_projection_check={:?}",
                packaging.submission_artifact_state, packaging.patch_projection_check_state
            );
        }
        assert_eq!(
            record.schema_version,
            ploke_records::run_record::RUN_RECORD_SCHEMA_VERSION
        );
        assert_eq!(
            record.metadata.benchmark.instance_id,
            "BurntSushi__ripgrep-2209"
        );
        assert_eq!(stats.turn_count, record.turn_count());
    }

    assert!(run_records.summary.parsed_count >= 2);
    assert_eq!(run_records.stats.len(), run_records.index.len());
    assert!(run_records.summary.total_tool_call_count > 0);
}

#[test]
fn fs_run_store_loads_branch_log_evidence() {
    let root = temp_run_root("branch-log");
    fs::create_dir_all(&root).expect("create run root");
    write_json(
        &root.join("scheduler.json"),
        &scheduler(vec![node("root", None, NodeStatusRecord::Succeeded)]),
    );
    fs::write(
            root.join("branches.json"),
            r#"{"schema_version":"prototype1-branch-record.v1","recorded_at":"2026-05-11T10:34:56Z","body":{"kind":"parent_comparison","campaign_id":"campaign-1","instance_id":"instance-1","source_state_id":"source-1","parent_branch_id":"branch-parent","target_relpath":"crates/ploke-llm/src/lib.rs","branch_id":"branch-1","candidate_id":"candidate-1","summary":{"baseline_campaign_id":"campaign-1","treatment_campaign_id":"campaign-1-treatment-branch-1","compared_instances":1,"rejected_instances":0,"overall_disposition":"keep","evaluated_at":"2026-05-11T10:34:55Z"}}}
"#,
        )
        .expect("write branch log");

    let forest = FsRunStore::new(&root).load_forest().expect("load forest");
    let branches = forest
        .passive_evidence
        .branch_registry
        .as_ref()
        .expect("branch evidence");

    assert_eq!(branches.record_count, 1);
    assert_eq!(branches.registry_snapshot_count, 0);
    assert_eq!(branches.parent_comparison_count, 1);
    assert_eq!(
        branches.latest_campaign_id.as_ref(),
        Some(&CampaignId::from("campaign-1"))
    );
    assert_eq!(branches.source_node_count, 0);
    assert_eq!(branches.branch_count, 0);
    assert_eq!(branches.active_target_count, 0);

    fs::remove_dir_all(root).expect("remove temp run");
}

#[test]
fn fs_run_store_loads_typed_transition_journal_in_append_order() {
    let root = temp_run_root("typed-journal");
    fs::create_dir_all(&root).expect("create run root");
    fs::write(
        root.join("transition-journal.jsonl"),
        format!("{}\n\n{}\n", minimal_journal_line(), minimal_journal_line()),
    )
    .expect("write journal");

    let journal = FsRunStore::new(&root)
        .load_transition_journal()
        .expect("load typed transition journal");

    assert_eq!(journal.len(), 2);
    assert_eq!(
        journal
            .iter()
            .map(|entry| entry.line_number)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert!(matches!(
        journal.entries[0].record,
        JournalEntry::Resource(_)
    ));
    assert!(matches!(
        journal.entries[1].record,
        JournalEntry::Resource(_)
    ));

    fs::remove_dir_all(root).expect("remove temp run");
}

#[test]
fn fs_run_store_loads_record_set() {
    let root = temp_run_root("record-set");
    fs::create_dir_all(root.join("history").join("blocks")).expect("create history dir");

    write_json(
        &root.join("scheduler.json"),
        &scheduler(vec![node("root", None, NodeStatusRecord::Succeeded)]),
    );
    fs::write(
        root.join("transition-journal.jsonl"),
        format!("{}\n", minimal_journal_line()),
    )
    .expect("write journal");
    fs::write(
        root.join("history")
            .join("blocks")
            .join("segment-000000.jsonl"),
        format!(
            "{}\n",
            serde_json::to_string(&synthetic_sealed_block(
                0,
                Vec::new(),
                &"4".repeat(64),
                None,
                "runtime-1",
            ))
            .expect("serialize sealed block")
        ),
    )
    .expect("write history");

    let records = FsRunStore::new(&root)
        .load_record_set()
        .expect("load record set");

    assert_eq!(records.forest_input.scheduler.nodes.len(), 1);
    assert_eq!(records.history_blocks.len(), 1);
    assert_eq!(records.transition_journal.len(), 1);

    fs::remove_dir_all(root).expect("remove temp run");
}

#[test]
fn fs_run_store_loads_run_attempt_evidence() {
    let root = temp_run_root("run-attempts");
    let root_node = node("root", None, NodeStatusRecord::Succeeded);
    let mut child_node = node("child", Some("root"), NodeStatusRecord::Succeeded);
    child_node.derived_artifact_id = Some(ArtifactId("artifact-child".to_owned()));
    let child_dir = root.join("nodes").join("child");
    fs::create_dir_all(child_dir.join("invocations")).expect("create invocation dir");
    fs::create_dir_all(child_dir.join("results")).expect("create results dir");
    fs::create_dir_all(root.join("nodes").join("root")).expect("create root node dir");

    write_json(
        &root.join("scheduler.json"),
        &scheduler(vec![root_node, child_node.clone()]),
    );
    let request = runner_request(&child_node);
    write_json(&child_dir.join("runner-request.json"), &request);
    write_json(
        &child_dir.join("runner-result.json"),
        &runner_result(&child_node),
    );
    let mut attempt_result = runner_result(&child_node);
    attempt_result.recorded_at = "2026-05-08T12:01:30Z".to_owned();
    write_json(
        &child_dir.join("results").join("runtime-1.json"),
        &attempt_result,
    );
    write_json(
        &child_dir.join("invocations").join("runtime-1.json"),
        &InvocationRecord {
            schema_version: "prototype1-invocation.v1".to_owned(),
            role: Role::Child,
            campaign_id: CampaignId::from("campaign-1"),
            node_id: child_node.node_id.as_str().to_owned(),
            runtime_id: RuntimeId("runtime-1".to_owned()),
            journal_path: root.join("transition-journal.jsonl"),
            channel_root: Some(child_dir.join("channels").join("runtime-1")),
            node: Some(child_node),
            request: Some(request),
            resolved: None,
            active_parent_root: None,
            created_at: "2026-05-08T12:01:00Z".to_owned(),
        },
    );

    let records = FsRunStore::new(&root)
        .load_record_set()
        .expect("load record set");
    let attempts = records
        .forest_input
        .passive_evidence
        .run_attempts
        .as_ref()
        .expect("run attempt evidence");

    assert_eq!(attempts.summary.runner_request_file_count, 1);
    assert_eq!(attempts.summary.runner_request_parsed_count, 1);
    assert_eq!(attempts.summary.runner_result_file_count, 1);
    assert_eq!(attempts.summary.runner_result_parsed_count, 1);
    assert_eq!(attempts.summary.invocation_file_count, 1);
    assert_eq!(attempts.summary.invocation_parsed_count, 1);
    assert_eq!(attempts.summary.child_invocation_count, 1);
    assert_eq!(attempts.summary.successor_invocation_count, 0);
    assert_eq!(
        attempts
            .runner_requests
            .get("nodes/child/runner-request.json")
            .expect("runner request")
            .node_id
            .as_str(),
        "child"
    );
    assert_eq!(
        attempts
            .runner_results
            .get("nodes/child/runner-result.json")
            .expect("runner result")
            .branch_id
            .as_str(),
        "branch-child"
    );
    assert_eq!(
        records
            .forest_input
            .passive_evidence
            .attempt_runner_results
            .get("nodes/child/results/runtime-1.json")
            .expect("attempt runner result")
            .recorded_at
            .as_str(),
        "2026-05-08T12:01:30Z"
    );
    assert_eq!(
        attempts
            .invocations
            .get("nodes/child/invocations/runtime-1.json")
            .expect("invocation")
            .runtime_id
            .0
            .as_str(),
        "runtime-1"
    );
    let graph = Graph::from_records(&records);
    let child_invocations = graph.child_invocations().collect::<Vec<_>>();
    assert_eq!(child_invocations.len(), 1);
    assert_eq!(child_invocations[0].1.node_id.as_str(), "child");
    let artifact_id = ArtifactId("artifact-child".to_owned());
    let artifact_child_invocations = graph
        .child_invocations_for_artifact(&artifact_id)
        .collect::<Vec<_>>();
    assert_eq!(artifact_child_invocations.len(), 1);
    assert_eq!(
        artifact_child_invocations[0].0,
        "nodes/child/invocations/runtime-1.json"
    );

    fs::remove_dir_all(root).expect("remove temp run");
}

#[test]
fn fs_run_store_loads_run_profile_evidence() {
    let root = temp_run_root("run-profile");
    fs::create_dir_all(&root).expect("create run root");

    write_json(
        &root.join("scheduler.json"),
        &scheduler(vec![node("root", None, NodeStatusRecord::Succeeded)]),
    );
    fs::write(
        root.join("run-profile.toml"),
        r#"
schema_version = "prototype1-run-profile.v1"
name = "synthetic-run-profile"

[storage]
worktree_root = "~/.ploke-eval/worktrees"

[target]
dataset_key = "ripgrep"
instance = "BurntSushi__ripgrep-2209"

[search]
max_generations = 3
max_total_nodes = 12
children = { min = 2, max = 4 }
schedule = "full-batch"
stop_on_first_keep = false
require_keep_for_continuation = true
explore_from_rejected = true

[generation]
source = "edit-surface"
surface = "workspace-except-ploke-eval"

[selection]
strategy = "history-score-child-prop"
evidence = "operational-and-protocol"
seed = 42

[execution]
stop_after = "complete"
trace_jsonl = "auto"
debug_tools = true
"#,
    )
    .expect("write profile");
    write_json(
        &root.join("run-profile.commitment.json"),
        &serde_json::json!({
            "schema_version": "prototype1-run-profile-commitment.v1",
            "profile_path": root.join("run-profile.toml"),
            "sha256": "0123456789abcdef",
            "source_path": "/tmp/profiles/synthetic-run-profile.toml",
            "admitted_at": "2026-05-11T12:00:00Z"
        }),
    );

    let records = FsRunStore::new(&root)
        .load_record_set()
        .expect("load record set");
    let run_profile = records
        .forest_input
        .passive_evidence
        .run_profile
        .as_ref()
        .expect("run profile evidence");
    let profile = run_profile.profile.as_ref().expect("profile record");
    let commitment = run_profile.commitment.as_ref().expect("commitment record");

    assert_eq!(profile.name, "synthetic-run-profile");
    assert_eq!(profile.search.max_generations, 3);
    assert_eq!(profile.selection.seed, 42);
    assert_eq!(commitment.sha256, "0123456789abcdef");
    assert_eq!(commitment.profile_path, root.join("run-profile.toml"));

    fs::remove_dir_all(root).expect("remove temp run");
}

#[test]
fn fs_run_store_loads_child_plan_evidence() {
    let root = temp_run_root("child-plan");
    let child_plan_dir = root.join("messages").join("child-plan");
    fs::create_dir_all(&child_plan_dir).expect("create child-plan dir");

    write_json(
        &root.join("scheduler.json"),
        &scheduler(vec![node("root", None, NodeStatusRecord::Succeeded)]),
    );
    let child_plan_path = child_plan_dir.join("root.json");
    fs::write(
        &child_plan_path,
        minimal_child_plan_json(&child_plan_path).to_string(),
    )
    .expect("write child-plan");

    let records = FsRunStore::new(&root)
        .load_record_set()
        .expect("load record set");
    let child_plans = records
        .forest_input
        .passive_evidence
        .child_plans
        .as_ref()
        .expect("child-plan evidence");

    assert_eq!(child_plans.summary.file_count, 1);
    assert_eq!(child_plans.summary.parsed_count, 1);
    assert_eq!(child_plans.summary.child_count, 1);
    assert_eq!(child_plans.summary.children_with_surface_count, 0);
    assert_eq!(child_plans.summary.rejected_surface_attempt_count, 0);
    let plan = child_plans.index.get("root").expect("root child plan");
    assert_eq!(plan.parent_node_id.as_str(), "root");
    assert_eq!(plan.child_generation, 1);
    assert_eq!(plan.children[0].node.node_id.as_str(), "child");

    fs::remove_dir_all(root).expect("remove temp run");
}

#[test]
fn typed_transition_journal_reports_bad_source_line() {
    let root = temp_run_root("typed-journal-error");
    fs::create_dir_all(&root).expect("create run root");
    fs::write(
        root.join("transition-journal.jsonl"),
        format!("{}\nnot-json\n", minimal_journal_line()),
    )
    .expect("write journal");

    let err = FsRunStore::new(&root)
        .load_transition_journal()
        .expect_err("bad journal line should fail typed loading");

    match err {
        FsRunStoreError::JsonLine {
            path, line_number, ..
        } => {
            assert_eq!(path, root.join("transition-journal.jsonl"));
            assert_eq!(line_number, 2);
        }
        other => panic!("expected JsonLine error, got {other:?}"),
    }

    fs::remove_dir_all(root).expect("remove temp run");
}

#[test]
#[ignore]
fn fs_run_store_loads_real_campaign() {
    let run_root = std::env::var("PLOKE_TREE_RUN_ROOT")
        .expect("set PLOKE_TREE_RUN_ROOT to a prototype1 run root");
    let mut store = FsRunStore::new(run_root);

    if let Ok(path) = std::env::var("PLOKE_TREE_PARENT_IDENTITY_PATH") {
        store = store.with_parent_identity_path(path);
    } else if let Ok(parent_root) = std::env::var("PLOKE_TREE_PARENT_ROOT") {
        store = store.with_parent_root(parent_root);
    }
    if let Ok(path) = std::env::var("PLOKE_TREE_PROTOCOL_ARTIFACTS_DIR") {
        store = store.with_protocol_artifacts_dir(path);
    }

    let forest = store.load_forest().expect("load real campaign forest");
    let max_generation = forest
        .nodes
        .iter()
        .map(|node| node.generation)
        .max()
        .unwrap_or(0);
    let journal = forest
        .passive_evidence
        .transition_journal
        .as_ref()
        .expect("journal evidence present");
    let history = forest.passive_evidence.history.as_ref();

    println!(
        "nodes={} max_generation={} roots={} journal_lines={} journal_parsed={} journal_errors={} history_blocks={} history_entries={} history_record_errors={} history_json_errors={} branches={:?} channels={:?} evaluations={:?}",
        forest.nodes.len(),
        max_generation,
        forest.roots.len(),
        journal.line_count,
        journal.parsed_count,
        journal.parse_error_count,
        history.map_or(0, |history| history.sealed_block_count),
        history.map_or(0, |history| history.admitted_entry_count),
        history.map_or(0, |history| history.record_parse_error_count),
        history.map_or(0, |history| history.json_parse_error_count),
        forest.passive_evidence.branch_registry,
        forest.passive_evidence.channel_envelopes,
        forest
            .passive_evidence
            .evaluations
            .as_ref()
            .map(|v| &v.summary),
    );

    assert!(
        !forest.nodes.is_empty(),
        "expected node records to be loaded"
    );
    assert!(!forest.roots.is_empty(), "expected at least one root node");
    assert!(journal.line_count > 0);
    let Some(history) = history else {
        return;
    };
    assert!(history.sealed_block_count > 0);
    assert!(history.admitted_entry_count > 0);
    let branches = forest
        .passive_evidence
        .branch_registry
        .as_ref()
        .expect("branch registry evidence present");
    assert!(
        branches.record_count > 0
            || branches.registry_snapshot_count > 0
            || branches.source_node_count > 0,
        "expected typed branch registry or branch log evidence"
    );
    assert!(
        forest
            .passive_evidence
            .channel_envelopes
            .as_ref()
            .expect("channel evidence present")
            .parsed_count
            > 0
    );
    let evaluations = forest
        .passive_evidence
        .evaluations
        .as_ref()
        .expect("evaluation evidence present");
    assert_eq!(
        evaluations.summary.file_count, evaluations.summary.parsed_count,
        "real-run evaluation artifacts should parse through typed records"
    );
    assert!(
        evaluations.summary.keep_count + evaluations.summary.reject_count
            == evaluations.summary.parsed_count,
        "evaluation disposition counts should cover parsed artifacts"
    );

    if let Some(protocol_artifacts) = forest.passive_evidence.protocol_artifacts.as_ref() {
        assert_eq!(
            protocol_artifacts.summary.file_count, protocol_artifacts.summary.parsed_count,
            "real-run protocol artifacts should parse through typed records"
        );
        assert_eq!(
            protocol_artifacts.summary.parsed_count, protocol_artifacts.summary.typed_payload_count,
            "loaded protocol artifacts should expose typed payload bodies"
        );
    }
    assert_no_sealed_history_authority(&forest);
}

#[test]
#[ignore]
fn coarse_playback_real_run() {
    let run_root = std::env::var("PLOKE_TREE_RUN_ROOT")
        .expect("set PLOKE_TREE_RUN_ROOT to a prototype1 run root");
    let store = FsRunStore::new(run_root);

    let blocks = store
        .load_history_blocks()
        .expect("load sealed history blocks");
    let spine = build_coarse_history_spine(&blocks);
    let steps = project_coarse_history_spine(&blocks);

    println!(
        "coarse_history blocks={} steps={} warnings={}",
        blocks.len(),
        steps.len(),
        spine.warnings.len()
    );
    for step in &steps {
        let warning_count = spine
            .warnings
            .iter()
            .filter(|warning| match warning {
                CoarseHistoryWarning::ParentHashLinkMismatch { block_height, .. }
                | CoarseHistoryWarning::MissingSelectionDecisionPayload { block_height, .. } => {
                    *block_height == step.block_height
                }
            })
            .count();
        println!(
            "block {} selected {} candidates {} warnings {}",
            step.block_height,
            step.selected_candidate.as_deref().unwrap_or("none"),
            step.considered_candidate_count,
            warning_count
        );
    }

    assert!(
        !blocks.is_empty(),
        "real run should have sealed history blocks"
    );
    assert_eq!(
        steps.len(),
        blocks.len(),
        "coarse history should project one step per sealed block"
    );
    assert!(
        steps
            .windows(2)
            .all(|pair| pair[0].block_height <= pair[1].block_height),
        "coarse history projection must preserve monotonic block heights"
    );
    assert!(
        steps.iter().all(|step| step.selected_candidate.is_some()),
        "real-run coarse history steps should expose selected candidates"
    );
    assert!(
        steps.iter().all(|step| step.considered_candidate_count > 0),
        "real-run coarse history steps should expose considered candidate counts"
    );

    let fine = fine_run_playback_from_sealed_history(&blocks);
    let fine_steps = fine.iter().collect::<Vec<_>>();
    let candidate_steps = fine_steps
        .iter()
        .filter(|step| step.kind == FineStepKind::CandidateConsidered)
        .count();
    let selected_steps = fine_steps
        .iter()
        .filter(|step| step.kind == FineStepKind::SuccessorSelected)
        .count();
    let admitted_steps = fine_steps
        .iter()
        .filter(|step| step.kind == FineStepKind::HistoryEntryAdmitted)
        .count();
    let sealed_steps = fine_steps
        .iter()
        .filter(|step| step.kind == FineStepKind::HistoryBlockSealed)
        .count();

    println!(
        "fine_history steps={} candidates={} selected={} admitted={} sealed={}",
        fine_steps.len(),
        candidate_steps,
        selected_steps,
        admitted_steps,
        sealed_steps
    );
    assert!(candidate_steps > 0);
    assert_eq!(selected_steps, blocks.len());
    assert_eq!(sealed_steps, blocks.len());
    assert!(admitted_steps >= blocks.len());
    assert!(
        fine_steps
            .windows(2)
            .all(|pair| pair[0].order <= pair[1].order),
        "fine playback must preserve causal order keys"
    );
}

trait ForestTestExt {
    fn node(&self, key: &str) -> &TreeNode;
}

impl ForestTestExt for RunForest {
    fn node(&self, key: &str) -> &TreeNode {
        self.nodes
            .iter()
            .find(|node| node.key == NodeKey::from(key))
            .expect("node exists")
    }
}

fn assert_no_sealed_history_authority(forest: &RunForest) {
    assert!(
        forest
            .nodes
            .iter()
            .all(|node| node.authority != AuthorityLabel::SealedVerifiedHistory)
    );
    assert!(forest.nodes.iter().all(|node| {
        node.evidence
            .iter()
            .all(|evidence| evidence.authority != AuthorityLabel::SealedVerifiedHistory)
    }));
}

fn scheduler(nodes: Vec<NodeRecord>) -> SchedulerStateRecord {
    SchedulerStateRecord {
        schema_version: "prototype1-scheduler.v1".to_owned(),
        campaign_id: CampaignId("campaign-1".to_owned()),
        updated_at: "2026-05-08T12:01:00Z".to_owned(),
        policy: Default::default(),
        frontier_node_ids: nodes
            .iter()
            .filter(|node| node.status == NodeStatusRecord::Running)
            .map(|node| node.node_id.clone())
            .collect(),
        completed_node_ids: nodes
            .iter()
            .filter(|node| node.status == NodeStatusRecord::Succeeded)
            .map(|node| node.node_id.clone())
            .collect(),
        failed_node_ids: nodes
            .iter()
            .filter(|node| node.status == NodeStatusRecord::Failed)
            .map(|node| node.node_id.clone())
            .collect(),
        last_continuation_decision: None,
        nodes,
    }
}

fn node(id: &str, parent: Option<&str>, status: NodeStatusRecord) -> NodeRecord {
    NodeRecord {
        schema_version: "prototype1-treatment-node.v1".to_owned(),
        node_id: SchedulerNodeId(id.to_owned()),
        parent_node_id: parent.map(|id| SchedulerNodeId(id.to_owned())),
        generation: if parent.is_some() { 1 } else { 0 },
        instance_id: InstanceId(format!("instance-{id}")),
        source_state_id: SourceStateId(format!("source-{id}")),
        operation_target: None,
        base_artifact_id: None,
        patch_id: None,
        derived_artifact_id: None,
        parent_branch_id: parent.map(|id| BranchId(format!("branch-{id}"))),
        branch_id: BranchId(format!("branch-{id}")),
        candidate_id: CandidateId(format!("candidate-{id}")),
        target_relpath: PathBuf::from("crates/ploke-core/tool_text/request_code_context.md"),
        node_dir: PathBuf::from(format!("/tmp/prototype1/nodes/{id}")),
        workspace_root: PathBuf::from(format!("/tmp/worktrees/{id}")),
        binary_path: PathBuf::from(format!("/tmp/worktrees/{id}/target/debug/ploke")),
        runner_request_path: PathBuf::from(format!(
            "/tmp/prototype1/nodes/{id}/runner-request.json"
        )),
        runner_result_path: PathBuf::from(format!("/tmp/prototype1/nodes/{id}/runner-result.json")),
        status,
        created_at: "2026-05-08T12:00:00Z".to_owned(),
        updated_at: "2026-05-08T12:01:00Z".to_owned(),
    }
}

fn parent_identity(node_id: &str, generation: u32) -> ParentIdentityRecord {
    ParentIdentityRecord {
        schema_version: "prototype1-parent-identity.v1".to_owned(),
        campaign_id: CampaignId::from("campaign-1"),
        parent_id: node_id.to_owned(),
        node_id: node_id.to_owned(),
        generation,
        instance_id: Some(format!("instance-{node_id}")),
        previous_parent_id: None,
        parent_node_id: None,
        branch_id: format!("branch-{node_id}"),
        artifact_branch: Some(format!("artifact-{node_id}")),
        created_at: "2026-05-08T12:00:00Z".to_owned(),
    }
}

fn runner_request(node: &NodeRecord) -> RunnerRequestRecord {
    RunnerRequestRecord {
        schema_version: "prototype1-runner-request.v1".to_owned(),
        campaign_id: CampaignId("campaign-1".to_owned()),
        node_id: node.node_id.clone(),
        generation: node.generation,
        instance_id: node.instance_id.clone(),
        source_state_id: node.source_state_id.clone(),
        operation_target: node.operation_target.clone(),
        base_artifact_id: node.base_artifact_id.clone(),
        patch_id: node.patch_id.clone(),
        derived_artifact_id: node.derived_artifact_id.clone(),
        branch_id: node.branch_id.clone(),
        target_relpath: node.target_relpath.clone(),
        workspace_root: node.workspace_root.clone(),
        binary_path: node.binary_path.clone(),
        stop_on_error: true,
        runner_args: vec!["prototype1".to_owned(), "runner".to_owned()],
    }
}

fn runner_result(node: &NodeRecord) -> RunnerResultRecord {
    RunnerResultRecord {
        schema_version: "prototype1-runner-result.v1".to_owned(),
        campaign_id: CampaignId("campaign-1".to_owned()),
        node_id: node.node_id.clone(),
        generation: node.generation,
        branch_id: node.branch_id.clone(),
        status: node.status,
        disposition: ploke_records::scheduler::RunnerDispositionRecord::Succeeded,
        treatment_campaign_id: Some(CampaignId::from("campaign-1-treatment")),
        evaluation_artifact_path: Some(PathBuf::from("evaluations/branch-child.json")),
        detail: None,
        exit_code: Some(0),
        stdout_excerpt: Some(String::new()),
        stderr_excerpt: Some(String::new()),
        recorded_at: "2026-05-08T12:02:00Z".to_owned(),
    }
}

fn minimal_branch_registry() -> serde_json::Value {
    serde_json::json!({
        "schema_version": "prototype1-branch-registry.v1",
        "campaign_id": "campaign-1",
        "updated_at": "2026-05-08T12:00:00Z",
        "source_nodes": [],
        "active_targets": []
    })
}

fn minimal_journal_line() -> String {
    serde_json::json!({
        "kind": "resource",
        "recorded_at": 1770000000000i64,
        "campaign_id": "campaign-1",
        "parent_id": "root",
        "node_id": "root",
        "generation": 0,
        "subject": "cargo_target",
        "phase": "parent_start",
        "path": "/tmp/target",
        "status": "measured",
        "bytes": 1
    })
    .to_string()
}

fn minimal_channel_line() -> String {
    serde_json::json!({
        "schema_version": "prototype1-runtime-channel.v1",
        "direction": "child_to_parent",
        "campaign_id": "campaign-1",
        "node_id": "child",
        "runtime_id": "11111111-1111-1111-1111-111111111111",
        "message_id": "22222222-2222-2222-2222-222222222222",
        "recorded_at": 1770000000000i64,
        "body_hash": "abc123",
        "body": "ready"
    })
    .to_string()
}

fn minimal_child_plan_json(message_path: &Path) -> serde_json::Value {
    serde_json::json!({
        "message": message_path.to_string_lossy(),
        "parent_node_id": "root",
        "child_generation": 1,
        "children": [{
            "node": {
                "schema_version": "prototype1-treatment-node.v1",
                "node_id": "child",
                "parent_node_id": "root",
                "generation": 1,
                "instance_id": "instance-child",
                "source_state_id": "source-child",
                "branch_id": "branch-child",
                "candidate_id": "candidate-child",
                "target_relpath": "src/lib.rs",
                "node_dir": "/tmp/prototype1/nodes/child",
                "workspace_root": "/tmp/worktrees/child",
                "binary_path": "/tmp/worktrees/child/target/debug/ploke",
                "runner_request_path": "/tmp/prototype1/nodes/child/runner-request.json",
                "runner_result_path": "/tmp/prototype1/nodes/child/runner-result.json",
                "status": "planned",
                "created_at": "2026-05-11T12:00:00Z",
                "updated_at": "2026-05-11T12:01:00Z"
            },
            "request": {
                "schema_version": "prototype1-runner-request.v1",
                "campaign_id": "campaign-1",
                "node_id": "child",
                "generation": 1,
                "instance_id": "instance-child",
                "source_state_id": "source-child",
                "branch_id": "branch-child",
                "target_relpath": "src/lib.rs",
                "workspace_root": "/tmp/worktrees/child",
                "binary_path": "/tmp/worktrees/child/target/debug/ploke",
                "stop_on_error": false,
                "runner_args": ["prototype1", "runner"]
            },
            "resolved": {
                "instance_id": "instance-child",
                "source_state_id": "source-child",
                "parent_branch_id": "branch-root",
                "target_relpath": "src/lib.rs",
                "source_content": "fn main() {}",
                "source_content_hash": "sha256:source",
                "selected_branch_id": "branch-child",
                "branch": {
                    "branch_id": "branch-child",
                    "candidate_id": "candidate-child",
                    "branch_label": "candidate-child",
                    "synthesized_spec_id": "spec-child",
                    "proposed_content": "fn main() { println!(\"child\"); }",
                    "proposed_content_hash": "sha256:proposed",
                    "status": "synthesized"
                }
            }
        }]
    })
}

fn write_json(path: &Path, value: &impl Serialize) {
    let json = serde_json::to_vec_pretty(value).expect("serialize fixture");
    fs::write(path, json).expect("write fixture");
}

fn write_gzip_json(path: &Path, value: &impl Serialize) {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    let file = fs::File::create(path).expect("create gzip fixture");
    let mut encoder = GzEncoder::new(file, Compression::default());
    let json = serde_json::to_vec_pretty(value).expect("serialize gzip fixture");
    encoder.write_all(&json).expect("write gzip fixture");
    encoder.finish().expect("finish gzip fixture");
}

fn minimal_run_record_json(
    instance_id: &str,
    manifest_id: &str,
    submission_artifact_state: &str,
    tool_call_count: usize,
    failed_tool_call_count: usize,
) -> serde_json::Value {
    let mut tool_calls = Vec::new();
    for index in 0..tool_call_count {
        let failed = index < failed_tool_call_count;
        let status = if failed { "Failed" } else { "Completed" };
        let result = if failed {
            serde_json::json!({
                "status": status,
                "request_id": format!("request-{index}"),
                "parent_id": "parent-1",
                "call_id": format!("call-{index}"),
                "tool": "read_file",
                "error": "fixture failure",
                "ui_payload": null,
                "latency_ms": 7
            })
        } else {
            serde_json::json!({
                "status": status,
                "request_id": format!("request-{index}"),
                "parent_id": "parent-1",
                "call_id": format!("call-{index}"),
                "tool": "read_file",
                "content": "fixture content",
                "ui_payload": null,
                "latency_ms": 7
            })
        };
        tool_calls.push(serde_json::json!({
            "request": {
                "request_id": format!("request-{index}"),
                "parent_id": "parent-1",
                "call_id": format!("call-{index}"),
                "tool": "read_file",
                "arguments": "{\"path\":\"src/lib.rs\"}"
            },
            "result": result,
            "latency_ms": 7
        }));
    }

    serde_json::json!({
        "schema_version": "run-record.v1",
        "manifest_id": manifest_id,
        "metadata": {
            "run_arm": {
                "id": "structured-current-policy",
                "role": "treatment",
                "command": "run single agent",
                "execution": "agent-single-turn"
            },
            "benchmark": {
                "instance_id": instance_id,
                "repo_root": "/tmp/repo",
                "base_sha": "abc123",
                "issue": {
                    "title": "fixture",
                    "body": "fixture body",
                    "body_path": null
                }
            },
            "agent": {
                "selected_model": "fixture/model",
                "selected_provider": "fixture-provider"
            },
            "runtime": {},
            "budget": {
                "max_turns": 40,
                "max_tool_calls": 200,
                "wall_clock_secs": 1800
            }
        },
        "phases": {
            "setup": {
                "started_at": "2026-05-16T00:00:00Z",
                "ended_at": "2026-05-16T00:00:01Z",
                "repo_state": {
                    "repo_root": "/tmp/repo",
                    "requested_base_sha": "abc123",
                    "checked_out_head_sha": "abc123",
                    "git_status_porcelain": ""
                },
                "indexing_status": {
                    "status": "success",
                    "detail": "fixture"
                },
                "indexed_crates": [],
                "parse_failures": [],
                "db_timestamp_micros": 1
            },
            "agent_turns": [{
                "turn_number": 1,
                "started_at": "2026-05-16T00:00:01Z",
                "ended_at": "2026-05-16T00:00:02Z",
                "db_timestamp_micros": 2,
                "issue_prompt": "fixture prompt",
                "llm_request": {
                    "model": "fixture/model",
                    "messages": [{
                        "role": "user",
                        "content": "fixture prompt"
                    }]
                },
                "tool_calls": tool_calls,
                "outcome": {
                    "type": "ToolCalls",
                    "count": tool_call_count
                }
            }],
            "packaging": {
                "started_at": "2026-05-16T00:00:03Z",
                "ended_at": "2026-05-16T00:00:04Z",
                "submission_artifact_state": submission_artifact_state,
                "msb_submission_path": "/tmp/submission.jsonl",
                "patch_projection_path": "/tmp/benchmark-patch-projection.json",
                "patch_projection_check_state": "passed"
            }
        },
        "db_time_travel_index": [{
            "turn": 1,
            "timestamp_micros": 2,
            "event": "turn_complete"
        }],
        "conversation": [],
        "timing": {
            "started_at": "2026-05-16T00:00:00Z",
            "ended_at": "2026-05-16T00:00:04Z",
            "total_wall_clock_secs": 4.0,
            "setup_wall_clock_secs": 1.0,
            "agent_wall_clock_secs": 1.0
        }
    })
}

fn protocol_issue_detection_artifact(
    run_id: &str,
    subject_id: &str,
    created_at_ms: u64,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "protocol-artifact.v1",
        "procedure_name": "intervention_issue_detection",
        "subject_id": subject_id,
        "run_id": run_id,
        "created_at_ms": created_at_ms,
        "input": {
            "run_id": run_id,
            "subject_id": subject_id,
            "total_calls_in_run": 3,
            "anchor_segment_count": 1,
            "protocol_reviewed_call_count": 1,
            "protocol_reviewed_segment_count": 0,
            "protocol_artifact_count": 2
        },
        "output": {
            "cases": []
        },
        "artifact": {
            "case_count": 0
        }
    })
}

fn temp_run_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("ploke-tree-{name}-{}-{nanos}", std::process::id()))
}

fn synthetic_sealed_block(
    block_height: u64,
    parent_block_hashes: Vec<&str>,
    block_hash: &str,
    selection: Option<(&str, usize)>,
    runtime_id: &str,
) -> SealedBlockRecord {
    let runtime = ActorRefRecord::Runtime(RuntimeId(runtime_id.to_owned()));
    let artifact =
        ArtifactRefRecord::from_artifact_id(ArtifactId(format!("artifact:{runtime_id}")));

    let entries = selection
        .map(|(candidate, considered_count)| {
            vec![AdmittedEntryRecord {
                core: EntryCoreRecord {
                    entry_id: EntryId(format!("entry-{block_height}")),
                    entry_kind: EntryKindRecord::Decision,
                    subject: SubjectRefRecord {
                        value: format!("subject-{block_height}"),
                    },
                    executor: runtime.clone(),
                    input_refs: Vec::new(),
                    output_refs: Vec::new(),
                    occurred_at: RecordedAt(100 + block_height as i64),
                    payload: EntryPayloadRecord::SelectionDecision(SelectionDecisionEntryRecord {
                        schema_version: 1,
                        procedure_or_policy: ProcedureRefRecord {
                            value: "policy:selection".to_owned(),
                        },
                        scope: ploke_records::history::SelectionScopeRecord {
                            value: "history".to_owned(),
                        },
                        selected_candidate: Some(SubjectRefRecord {
                            value: candidate.to_owned(),
                        }),
                        selected_occurrence_id: None,
                        selected_membership_id: None,
                        considered: (0..considered_count)
                            .map(|idx| ploke_records::history::EvaluationPayloadRecord {
                                schema_version: 1,
                                candidate: SubjectRefRecord {
                                    value: format!("{candidate}-{idx}"),
                                },
                                procedure: ProcedureRefRecord {
                                    value: "selection:eval".to_owned(),
                                },
                                selection_input: None,
                                selection_input_hash: None,
                                projection_failures: Vec::new(),
                                source_refs: Vec::new(),
                                source_hashes: Vec::new(),
                                sealed_evidence: None,
                                artifact: None,
                                surface_attempt: None,
                            })
                            .collect(),
                        considered_sources: Vec::new(),
                        considered_order_hash: HistoryHash("a".repeat(64)),
                        candidate_set: None,
                        projection_failures: Vec::new(),
                        traversal: None,
                        metrics: synthetic_selection_metrics(
                            candidate,
                            considered_count,
                            &HistoryHash("a".repeat(64)),
                        ),
                        formula: None,
                        decision: Decision {
                            procedure_id: "selector-v1".to_owned(),
                            candidate_node_id: "node-0".to_owned(),
                            selected_branch_id: Some("branch-0".to_owned()),
                            branch_disposition: "keep".to_owned(),
                            outcome: Outcome::Accepted,
                            findings: Vec::new(),
                            rationale: Vec::new(),
                        },
                    }),
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
                            value: "payload".to_owned(),
                        },
                        payload_hash: HistoryHash("b".repeat(64)),
                        observed_at: RecordedAt(100 + block_height as i64),
                        recorded_at: RecordedAt(100 + block_height as i64),
                    },
                    proposer: runtime.clone(),
                    procedure_or_policy: ProcedureRefRecord {
                        value: "policy:selection".to_owned(),
                    },
                    admitting_authority: runtime.clone(),
                    ruling_authority: runtime.clone(),
                    lineage_id: LineageId("lineage:synthetic".to_owned()),
                    block_id: BlockId(format!("block-{block_height}")),
                    block_height,
                },
            }]
        })
        .unwrap_or_default();

    SealedBlockRecord {
        state: SealedBlockStateRecord {
            header: SealedBlockHeaderRecord {
                common: BlockCommonRecord {
                    schema_version: 1,
                    block_id: BlockId(format!("block-{block_height}")),
                    lineage_id: LineageId("lineage:synthetic".to_owned()),
                    block_height,
                    parent_block_hashes: parent_block_hashes
                        .into_iter()
                        .map(|hash| BlockHash(hash.to_owned()))
                        .collect(),
                    opened_from_state: HistoryStateRoot("0".repeat(64)),
                    regime: RegimeRecord {
                        step: StepRecord(block_height),
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
                            hash: HistoryHash("c".repeat(64)),
                        },
                        parent_identity: ParentIdentityRefRecord {
                            evidence: EvidenceRefRecord {
                                value: "parent".to_owned(),
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
                                hash: HistoryHash("d".repeat(64)),
                            },
                        },
                        mutated: SurfaceDeltaRecord {
                            before: SurfaceRecord {
                                root: SurfaceRootRecord {
                                    hash: HistoryHash("e".repeat(64)),
                                },
                            },
                            after: SurfaceRecord {
                                root: SurfaceRootRecord {
                                    hash: HistoryHash("f".repeat(64)),
                                },
                            },
                        },
                        ambient: SurfaceDeltaRecord {
                            before: SurfaceRecord {
                                root: SurfaceRootRecord {
                                    hash: HistoryHash("1".repeat(64)),
                                },
                            },
                            after: SurfaceRecord {
                                root: SurfaceRootRecord {
                                    hash: HistoryHash("2".repeat(64)),
                                },
                            },
                        },
                    },
                    opened_at: RecordedAt(90 + block_height as i64),
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
                sealed_at: RecordedAt(110 + block_height as i64),
                entry_count: entries.len(),
                entries_root: HistoryHash("3".repeat(64)),
                block_hash: BlockHash(block_hash.to_owned()),
            },
            private: (),
        },
        entries,
    }
}

fn synthetic_selection_metrics(
    candidate: &str,
    considered_count: usize,
    considered_order_hash: &HistoryHash,
) -> MetricSet {
    MetricSet {
        schema_version: 1,
        id: HistoryHash(format!("metric-set:{candidate}:{considered_count}")),
        considered_order_hash: considered_order_hash.clone(),
        candidate_set_root: None,
        policy: MetricPolicy::default(),
        candidates: (0..considered_count)
            .map(|idx| MetricCandidate {
                payload_index: idx,
                payload_hash: HistoryHash(format!("payload:{candidate}:{idx}")),
                candidate: format!("{candidate}-{idx}"),
                occurrence_id: None,
                membership_id: None,
                imp_at_k: None,
            })
            .collect(),
    }
}
