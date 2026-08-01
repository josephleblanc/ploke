use ploke_records::evaluation::PatchProjectionCheckState;
use ploke_records::protocol::ArtifactBody;
use ploke_records::run_record::SubmissionArtifactState;
use ploke_tree::{
    ClosureEvidence, ProtocolArtifactSummary, ProtocolArtifactsEvidence, RunRecordEvidence,
    RunRecordSummary, graph::EvalProtocolEvidence,
};

pub(crate) struct EvalProtocolDashboard<'g> {
    evidence: EvalProtocolEvidence<'g>,
    availability: EvalProtocolAvailability,
}

impl<'g> EvalProtocolDashboard<'g> {
    pub(crate) fn from_graph(graph: &'g ploke_tree::Graph) -> Self {
        let evidence = graph.eval_protocol_evidence();
        let availability = EvalProtocolAvailability::from_evidence(evidence);

        Self {
            evidence,
            availability,
        }
    }

    #[cfg(test)]
    pub(crate) fn evidence(&self) -> EvalProtocolEvidence<'g> {
        self.evidence
    }

    pub(crate) fn availability(&self) -> EvalProtocolAvailability {
        self.availability
    }

    pub(crate) fn is_available(&self) -> bool {
        self.availability.closure == EvidenceState::Available
            || self.availability.run_records == EvidenceState::Available
            || self.availability.protocol_artifacts == EvidenceState::Available
    }

    pub(crate) fn closure(&self) -> Option<&'g ClosureEvidence> {
        self.evidence.closure
    }

    pub(crate) fn run_records(&self) -> Option<&'g RunRecordEvidence> {
        self.evidence.run_records
    }

    pub(crate) fn protocol_artifacts(&self) -> Option<&'g ProtocolArtifactsEvidence> {
        self.evidence.protocol_artifacts
    }

    pub(crate) fn run_records_summary(&self) -> Option<&'g RunRecordSummary> {
        self.run_records().map(|run_records| &run_records.summary)
    }

    pub(crate) fn run_records_file_count(&self) -> Option<usize> {
        self.run_records_summary().map(|summary| summary.file_count)
    }

    pub(crate) fn run_records_parsed_count(&self) -> Option<usize> {
        self.run_records_summary()
            .map(|summary| summary.parsed_count)
    }

    pub(crate) fn run_records_branch_ref_count(&self) -> Option<usize> {
        self.run_records_summary()
            .map(|summary| summary.branch_ref_count)
    }

    pub(crate) fn run_records_total_turn_count(&self) -> Option<usize> {
        self.run_records_summary()
            .map(|summary| summary.total_turn_count)
    }

    pub(crate) fn run_records_total_tool_call_count(&self) -> Option<usize> {
        self.run_records_summary()
            .map(|summary| summary.total_tool_call_count)
    }

    pub(crate) fn run_records_failed_tool_call_count(&self) -> Option<usize> {
        self.run_records_summary()
            .map(|summary| summary.failed_tool_call_count)
    }

    pub(crate) fn protocol_summary(&self) -> Option<&'g ProtocolArtifactSummary> {
        self.protocol_artifacts()
            .map(|protocol_artifacts| &protocol_artifacts.summary)
    }

    pub(crate) fn protocol_artifacts_file_count(&self) -> Option<usize> {
        self.protocol_summary().map(|summary| summary.file_count)
    }

    pub(crate) fn protocol_artifacts_parsed_count(&self) -> Option<usize> {
        self.protocol_summary().map(|summary| summary.parsed_count)
    }

    pub(crate) fn protocol_artifacts_review_count(&self) -> Option<usize> {
        self.protocol_summary().map(|summary| summary.review_count)
    }

    pub(crate) fn protocol_artifacts_intent_segmentation_count(&self) -> Option<usize> {
        self.protocol_summary()
            .map(|summary| summary.intent_segmentation_count)
    }

    pub(crate) fn protocol_artifacts_segment_review_count(&self) -> Option<usize> {
        self.protocol_summary()
            .map(|summary| summary.segment_review_count)
    }

    pub(crate) fn eval_patch_counts(&self) -> EvalPatchCounts {
        let mut counts = EvalPatchCounts::default();
        let Some(run_records) = self.run_records() else {
            return counts;
        };

        for record in run_records.index.values() {
            if record.phases.patch.is_some() {
                counts.patch_phase_count += 1;
            }

            if let Some(packaging) = record.phases.packaging.as_ref() {
                match packaging.submission_artifact_state {
                    SubmissionArtifactState::Empty => counts.empty_submission_count += 1,
                    SubmissionArtifactState::Nonempty => counts.nonempty_submission_count += 1,
                    SubmissionArtifactState::NotRecorded
                    | SubmissionArtifactState::NotApplicable
                    | SubmissionArtifactState::Missing => {}
                }
                counts
                    .patch_projection
                    .observe(packaging.patch_projection_check_state);
            }

            for turn in &record.phases.agent_turns {
                let Some(artifact) = turn.agent_turn_artifact.as_ref() else {
                    continue;
                };
                counts.edit_proposal_count += artifact.patch_artifact.edit_proposals.len();
                counts.create_proposal_count += artifact.patch_artifact.create_proposals.len();
                counts.expected_file_change_count +=
                    artifact.patch_artifact.expected_file_changes.len();
                if artifact.patch_artifact.applied {
                    counts.applied_patch_artifact_count += 1;
                }
            }
        }

        counts
    }

    pub(crate) fn protocol_aggregate_counts(&self) -> ProtocolAggregateCounts {
        let mut counts = ProtocolAggregateCounts::default();
        let Some(protocol_artifacts) = self.protocol_artifacts() else {
            return counts;
        };

        for artifact in protocol_artifacts.index.values() {
            match &artifact.body {
                ArtifactBody::ToolCallReview(_) => counts.call_review_count += 1,
                ArtifactBody::ToolCallSegmentReview(_) => counts.segment_review_count += 1,
                ArtifactBody::InterventionIssueDetection(payload) => {
                    counts.issue_detection_count += 1;
                    counts.issue_detection_case_count += payload.output.cases.len();
                }
                ArtifactBody::InterventionSynthesis(payload) => {
                    counts.intervention_synthesis_count += 1;
                    counts.intervention_candidate_count +=
                        payload.output.candidate_set.candidates.len();
                }
                ArtifactBody::InterventionApply(_) => counts.intervention_apply_count += 1,
                ArtifactBody::ToolCallIntentSegmentation(_) => {}
            }
        }

        counts
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct EvalPatchCounts {
    pub patch_phase_count: usize,
    pub empty_submission_count: usize,
    pub nonempty_submission_count: usize,
    pub edit_proposal_count: usize,
    pub create_proposal_count: usize,
    pub expected_file_change_count: usize,
    pub applied_patch_artifact_count: usize,
    pub patch_projection: PatchProjectionCounts,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PatchProjectionCounts {
    pub not_recorded: usize,
    pub not_applicable: usize,
    pub passed: usize,
    pub failed: usize,
    pub not_run: usize,
}

impl PatchProjectionCounts {
    fn observe(&mut self, state: PatchProjectionCheckState) {
        match state {
            PatchProjectionCheckState::NotRecorded => self.not_recorded += 1,
            PatchProjectionCheckState::NotApplicable => self.not_applicable += 1,
            PatchProjectionCheckState::Passed => self.passed += 1,
            PatchProjectionCheckState::Failed => self.failed += 1,
            PatchProjectionCheckState::NotRun => self.not_run += 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ProtocolAggregateCounts {
    pub call_review_count: usize,
    pub segment_review_count: usize,
    pub issue_detection_count: usize,
    pub issue_detection_case_count: usize,
    pub intervention_synthesis_count: usize,
    pub intervention_candidate_count: usize,
    pub intervention_apply_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceState {
    Available,
    Missing,
    #[allow(dead_code)]
    NotApplicable,
}

impl EvidenceState {
    fn from_option<T>(value: Option<&T>) -> Self {
        if value.is_some() {
            Self::Available
        } else {
            Self::Missing
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EvalProtocolAvailability {
    pub closure: EvidenceState,
    pub run_records: EvidenceState,
    pub protocol_artifacts: EvidenceState,
}

impl EvalProtocolAvailability {
    fn from_evidence(evidence: EvalProtocolEvidence<'_>) -> Self {
        Self {
            closure: EvidenceState::from_option(evidence.closure),
            run_records: EvidenceState::from_option(evidence.run_records),
            protocol_artifacts: EvidenceState::from_option(evidence.protocol_artifacts),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use ploke_records::agent_turn::{
        AgentTurnArtifactRecord, ExpectedFileChangeRecord, PatchArtifactRecord,
        ProposalSnapshotRecord,
    };
    use ploke_records::evaluation::PatchProjectionCheckState;
    use ploke_records::ids::CampaignId;
    use ploke_records::run_record::{
        AgentMetadata, BenchmarkMetadata, EvalBudget, PackagingPhase, PatchPhase, RunArm,
        RunArmRole, RunMetadata, RunPhases, RunRecord, RuntimeMetadata, SubmissionArtifactState,
        TurnOutcome, TurnRecord,
    };
    use ploke_tree::{
        CampaignRef, Lanes, PassiveEvidence, ProtocolArtifactSummary, ProtocolArtifactsEvidence,
        RunForest, RunRecordEvidence, RunRecordSummary,
    };

    #[test]
    fn eval_protocol_dashboard_reports_all_sources_missing_for_empty_graph() {
        let graph = ploke_tree::Graph::default();
        let dashboard = EvalProtocolDashboard::from_graph(&graph);

        assert_eq!(dashboard.availability().closure, EvidenceState::Missing);
        assert_eq!(dashboard.availability().run_records, EvidenceState::Missing);
        assert_eq!(
            dashboard.availability().protocol_artifacts,
            EvidenceState::Missing
        );
        assert!(!dashboard.is_available());
    }

    #[test]
    fn eval_protocol_dashboard_exposes_typed_scalar_missing_values_for_empty_graph() {
        let graph = ploke_tree::Graph::default();
        let dashboard = EvalProtocolDashboard::from_graph(&graph);
        let evidence = dashboard.evidence();

        assert!(evidence.closure.is_none());
        assert!(evidence.run_records.is_none());
        assert!(evidence.protocol_artifacts.is_none());

        assert!(dashboard.closure().is_none());
        assert!(dashboard.run_records().is_none());
        assert!(dashboard.run_records_summary().is_none());
        assert_eq!(dashboard.run_records_file_count(), None);
        assert_eq!(dashboard.run_records_parsed_count(), None);
        assert_eq!(dashboard.run_records_branch_ref_count(), None);
        assert_eq!(dashboard.run_records_total_turn_count(), None);
        assert_eq!(dashboard.run_records_total_tool_call_count(), None);
        assert_eq!(dashboard.run_records_failed_tool_call_count(), None);
        assert!(dashboard.protocol_artifacts().is_none());
        assert!(dashboard.protocol_summary().is_none());
        assert_eq!(dashboard.protocol_artifacts_file_count(), None);
        assert_eq!(dashboard.protocol_artifacts_parsed_count(), None);
        assert_eq!(dashboard.protocol_artifacts_review_count(), None);
        assert_eq!(
            dashboard.protocol_artifacts_intent_segmentation_count(),
            None
        );
        assert_eq!(dashboard.protocol_artifacts_segment_review_count(), None);
    }

    #[test]
    fn eval_protocol_dashboard_exposes_run_record_and_protocol_summary_counts() {
        let graph = graph_with_passive_evidence(PassiveEvidence {
            run_records: Some(RunRecordEvidence {
                summary: RunRecordSummary {
                    file_count: 5,
                    parsed_count: 4,
                    branch_ref_count: 3,
                    baseline_ref_count: 2,
                    treatment_ref_count: 1,
                    records_with_setup_count: 4,
                    records_with_packaging_count: 3,
                    total_turn_count: 12,
                    total_tool_call_count: 34,
                    failed_tool_call_count: 5,
                },
                ..RunRecordEvidence::default()
            }),
            protocol_artifacts: Some(ProtocolArtifactsEvidence {
                summary: ProtocolArtifactSummary {
                    file_count: 9,
                    parsed_count: 8,
                    intent_segmentation_count: 7,
                    review_count: 6,
                    segment_review_count: 5,
                    typed_payload_count: 8,
                },
                ..ProtocolArtifactsEvidence::default()
            }),
            ..PassiveEvidence::default()
        });
        let dashboard = EvalProtocolDashboard::from_graph(&graph);

        assert!(dashboard.run_records().is_some());
        assert_eq!(dashboard.run_records_parsed_count(), Some(4));
        assert_eq!(dashboard.run_records_total_turn_count(), Some(12));
        assert_eq!(dashboard.run_records_total_tool_call_count(), Some(34));
        assert_eq!(dashboard.run_records_failed_tool_call_count(), Some(5));

        assert!(dashboard.protocol_artifacts().is_some());
        assert_eq!(dashboard.protocol_artifacts_parsed_count(), Some(8));
        assert_eq!(
            dashboard.protocol_artifacts_intent_segmentation_count(),
            Some(7)
        );
        assert_eq!(dashboard.protocol_artifacts_review_count(), Some(6));
        assert_eq!(dashboard.protocol_artifacts_segment_review_count(), Some(5));
    }

    #[test]
    fn eval_protocol_dashboard_computes_typed_patch_counts_without_label_maps() {
        let mut run_records = RunRecordEvidence::default();
        run_records.index = BTreeMap::from([
            (
                "record:passed".to_owned(),
                run_record_fixture(
                    "manifest:passed",
                    true,
                    SubmissionArtifactState::Nonempty,
                    PatchProjectionCheckState::Passed,
                    patch_artifact_fixture(2, 1, 3, true),
                ),
            ),
            (
                "record:failed".to_owned(),
                run_record_fixture(
                    "manifest:failed",
                    false,
                    SubmissionArtifactState::Empty,
                    PatchProjectionCheckState::Failed,
                    patch_artifact_fixture(0, 0, 0, false),
                ),
            ),
            (
                "record:not-run".to_owned(),
                run_record_fixture(
                    "manifest:not-run",
                    false,
                    SubmissionArtifactState::NotApplicable,
                    PatchProjectionCheckState::NotRun,
                    patch_artifact_fixture(0, 0, 0, false),
                ),
            ),
        ]);
        let graph = graph_with_passive_evidence(PassiveEvidence {
            run_records: Some(run_records),
            ..PassiveEvidence::default()
        });
        let dashboard = EvalProtocolDashboard::from_graph(&graph);

        assert_eq!(
            dashboard.eval_patch_counts(),
            EvalPatchCounts {
                patch_phase_count: 1,
                empty_submission_count: 1,
                nonempty_submission_count: 1,
                edit_proposal_count: 2,
                create_proposal_count: 1,
                expected_file_change_count: 3,
                applied_patch_artifact_count: 1,
                patch_projection: PatchProjectionCounts {
                    not_recorded: 0,
                    not_applicable: 0,
                    passed: 1,
                    failed: 1,
                    not_run: 1,
                },
            }
        );
    }

    fn graph_with_passive_evidence(passive_evidence: PassiveEvidence) -> ploke_tree::Graph {
        ploke_tree::Graph {
            forest: Some(RunForest {
                campaign: CampaignRef {
                    campaign_id: CampaignId::from("campaign"),
                    updated_at: "now".to_owned(),
                },
                roots: Vec::new(),
                nodes: Vec::new(),
                lanes: Lanes {
                    frontier: Vec::new(),
                    completed: Vec::new(),
                    failed: Vec::new(),
                },
                passive_evidence,
                diagnostics: Vec::new(),
            }),
            ..ploke_tree::Graph::default()
        }
    }

    fn run_record_fixture(
        manifest_id: &str,
        has_patch_phase: bool,
        submission_artifact_state: SubmissionArtifactState,
        patch_projection_check_state: PatchProjectionCheckState,
        patch_artifact: PatchArtifactRecord,
    ) -> RunRecord {
        RunRecord {
            schema_version: "run-record.v1".to_owned(),
            manifest_id: manifest_id.to_owned(),
            metadata: RunMetadata {
                run_arm: RunArm {
                    id: "arm".to_owned(),
                    role: RunArmRole::Treatment,
                    command: "run".to_owned(),
                    execution: "test".to_owned(),
                },
                benchmark: BenchmarkMetadata {
                    instance_id: "instance".to_owned(),
                    repo_root: PathBuf::from("/tmp/repo"),
                    base_sha: None,
                    issue: None,
                },
                agent: AgentMetadata::default(),
                runtime: RuntimeMetadata::default(),
                budget: EvalBudget {
                    max_turns: 40,
                    max_tool_calls: 200,
                    wall_clock_secs: 1800,
                },
            },
            phases: RunPhases {
                patch: has_patch_phase.then(|| PatchPhase {
                    started_at: "start".to_owned(),
                    ended_at: "end".to_owned(),
                    patch_artifact: patch_artifact.clone(),
                    diff: None,
                }),
                packaging: Some(PackagingPhase {
                    started_at: "start".to_owned(),
                    ended_at: "end".to_owned(),
                    submission_artifact_state,
                    msb_submission_path: None,
                    patch_projection_path: None,
                    patch_projection_check_state,
                }),
                agent_turns: vec![TurnRecord {
                    turn_number: 1,
                    started_at: "start".to_owned(),
                    ended_at: "end".to_owned(),
                    db_timestamp_micros: 1,
                    issue_prompt: "issue".to_owned(),
                    llm_request: None,
                    llm_response: None,
                    tool_calls: Vec::new(),
                    outcome: TurnOutcome::Content,
                    agent_turn_artifact: Some(AgentTurnArtifactRecord {
                        task_id: "task".to_owned(),
                        selected_model: "model".to_owned(),
                        issue_prompt: "issue".to_owned(),
                        user_message_id: "user".to_owned(),
                        events: Vec::new(),
                        prompt_debug: None,
                        terminal_record: None,
                        final_assistant_message: None,
                        patch_artifact,
                        llm_prompt: Vec::new(),
                        llm_response: None,
                        model_route: None,
                    }),
                }],
                ..RunPhases::default()
            },
            db_time_travel_index: Vec::new(),
            conversation: Vec::new(),
            timing: None,
        }
    }

    fn patch_artifact_fixture(
        edit_proposals: usize,
        create_proposals: usize,
        expected_file_changes: usize,
        applied: bool,
    ) -> PatchArtifactRecord {
        PatchArtifactRecord {
            edit_proposals: (0..edit_proposals)
                .map(|index| proposal_fixture("edit", index))
                .collect(),
            create_proposals: (0..create_proposals)
                .map(|index| proposal_fixture("create", index))
                .collect(),
            applied,
            all_proposals_applied: applied,
            expected_file_changes: (0..expected_file_changes)
                .map(|index| ExpectedFileChangeRecord {
                    path: format!("src/{index}.rs"),
                    existed_before: true,
                    exists_after: true,
                    before_sha256: None,
                    after_sha256: None,
                    changed: true,
                })
                .collect(),
            any_expected_file_changed: expected_file_changes > 0,
            all_expected_files_changed: expected_file_changes > 0,
        }
    }

    fn proposal_fixture(kind: &str, index: usize) -> ProposalSnapshotRecord {
        ProposalSnapshotRecord {
            request_id: format!("request:{kind}:{index}"),
            call_id: format!("call:{kind}:{index}"),
            status: "applied".to_owned(),
            files: vec![format!("src/{kind}_{index}.rs")],
            preview_mode: "direct".to_owned(),
        }
    }
}
