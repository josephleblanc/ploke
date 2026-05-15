use std::path::PathBuf;

use crate::graph::{
    AgentTurnArtifactKind, AgentTurnArtifactMetadata, EvidenceKind, EvidenceLocator,
    EvidenceSubject,
};
use crate::{
    AgentTurnArtifactEvidence, AgentTurnEvidence, ChildPlanEvidence, EvaluationEvidence,
    PassiveEvidence, ProtocolArtifactsEvidence, RunProfileEvidence,
};

use super::Builder;

impl Builder {
    pub(super) fn ingest_passive_evidence(&mut self, evidence: &PassiveEvidence) {
        if let Some(branch_registry) = evidence.branch_registry.as_ref() {
            self.ingest_branch_registry_summary(branch_registry);
        }
        if let Some(transition_journal) = evidence.transition_journal.as_ref() {
            self.attach_located_evidence(
                EvidenceSubject::TransitionJournalSummary {
                    line_count: transition_journal.line_count,
                    parsed_count: transition_journal.parsed_count,
                    parse_error_count: transition_journal.parse_error_count,
                },
                EvidenceKind::TransitionJournalSummary,
                vec![EvidenceLocator::LoadedSummary {
                    name: "transition_journal_summary",
                }],
            );
        }
        if let Some(history) = evidence.history.as_ref() {
            self.attach_located_evidence(
                EvidenceSubject::HistoryStorageSummary {
                    line_count: history.line_count,
                    sealed_block_count: history.sealed_block_count,
                    admitted_entry_count: history.admitted_entry_count,
                    record_parse_error_count: history.record_parse_error_count,
                    json_parse_error_count: history.json_parse_error_count,
                },
                EvidenceKind::HistoryStorageSummary,
                vec![EvidenceLocator::LoadedSummary {
                    name: "history_storage",
                }],
            );
        }
        if let Some(channel_envelopes) = evidence.channel_envelopes.as_ref() {
            self.ingest_channel_summary(channel_envelopes);
        }
        if let Some(child_plans) = evidence.child_plans.as_ref() {
            self.ingest_child_plan_summary(child_plans);
        }
        if let Some(evaluations) = evidence.evaluations.as_ref() {
            self.ingest_evaluation_evidence(evaluations);
        }
        if let Some(protocol_artifacts) = evidence.protocol_artifacts.as_ref() {
            self.ingest_protocol_artifacts(protocol_artifacts);
        }
        if let Some(run_profile) = evidence.run_profile.as_ref() {
            self.ingest_run_profile(run_profile);
        }
        if let Some(run_attempts) = evidence.run_attempts.as_ref() {
            self.ingest_run_attempts(run_attempts);
        }
        if let Some(agent_turns) = evidence.agent_turns.as_ref() {
            self.ingest_agent_turns(agent_turns);
        }
        self.ingest_attempt_runner_results(&evidence.attempt_runner_results);
    }

    fn ingest_child_plan_summary(&mut self, evidence: &ChildPlanEvidence) {
        self.graph.child_plans.plans = evidence
            .index
            .values()
            .map(|plan| (plan.parent_node_id.clone(), plan.clone()))
            .collect();
        self.attach_located_evidence(
            EvidenceSubject::ChildPlanSummary {
                file_count: evidence.summary.file_count,
                parsed_count: evidence.summary.parsed_count,
                child_count: evidence.summary.child_count,
                children_with_surface_count: evidence.summary.children_with_surface_count,
                rejected_surface_attempt_count: evidence.summary.rejected_surface_attempt_count,
            },
            EvidenceKind::ChildPlanSummary,
            vec![EvidenceLocator::LoadedSummary {
                name: "child_plans",
            }],
        );
    }

    fn ingest_evaluation_evidence(&mut self, evidence: &EvaluationEvidence) {
        self.attach_located_evidence(
            EvidenceSubject::EvaluationSummary {
                file_count: evidence.summary.file_count,
                parsed_count: evidence.summary.parsed_count,
                keep_count: evidence.summary.keep_count,
                reject_count: evidence.summary.reject_count,
            },
            EvidenceKind::EvaluationSummary,
            vec![EvidenceLocator::LoadedSummary {
                name: "evaluations",
            }],
        );

        for artifact in evidence.index.values() {
            let evidence_id = self.attach_located_evidence(
                EvidenceSubject::Branch(artifact.branch_id.clone()),
                EvidenceKind::CandidateEvaluation,
                vec![EvidenceLocator::EvaluationArtifact {
                    path: artifact.evaluation_artifact_path.clone(),
                }],
            );

            self.attach_to_branch(&artifact.branch_id, evidence_id);
        }
    }

    fn ingest_protocol_artifacts(&mut self, evidence: &ProtocolArtifactsEvidence) {
        self.attach_located_evidence(
            EvidenceSubject::ProtocolArtifactSummary {
                file_count: evidence.summary.file_count,
                parsed_count: evidence.summary.parsed_count,
                intent_segmentation_count: evidence.summary.intent_segmentation_count,
                review_count: evidence.summary.review_count,
                segment_review_count: evidence.summary.segment_review_count,
                typed_payload_count: evidence.summary.typed_payload_count,
            },
            EvidenceKind::ProtocolArtifactSummary,
            vec![EvidenceLocator::LoadedSummary {
                name: "protocol_artifacts",
            }],
        );

        for (path, artifact) in &evidence.index {
            self.attach_located_evidence(
                EvidenceSubject::ProtocolArtifact {
                    procedure_name: artifact.procedure_name.clone(),
                    subject_id: artifact.subject_id.clone(),
                    run_id: artifact.run_id.clone(),
                },
                EvidenceKind::ProtocolArtifact,
                vec![EvidenceLocator::ProtocolArtifact {
                    path: path.into(),
                    procedure_name: artifact.procedure_name.clone(),
                    subject_id: artifact.subject_id.clone(),
                    run_id: artifact.run_id.clone(),
                }],
            );
        }
    }

    fn ingest_run_profile(&mut self, evidence: &RunProfileEvidence) {
        if let Some(profile) = evidence.profile.as_ref() {
            self.attach_located_evidence(
                EvidenceSubject::RunProfileSummary(profile.into()),
                EvidenceKind::RunProfileSummary,
                vec![EvidenceLocator::LoadedSummary {
                    name: "run_profile",
                }],
            );
        }

        if let Some(commitment) = evidence.commitment.as_ref() {
            self.attach_located_evidence(
                EvidenceSubject::RunProfileCommitment(commitment.clone()),
                EvidenceKind::RunProfileCommitment,
                vec![EvidenceLocator::LoadedSummary {
                    name: "run_profile_commitment",
                }],
            );
        }
    }

    fn ingest_agent_turns(&mut self, evidence: &AgentTurnEvidence) {
        self.attach_located_evidence(
            EvidenceSubject::AgentTurnEvidenceSummary {
                trace_file_count: evidence.summary.trace_file_count,
                trace_parsed_count: evidence.summary.trace_parsed_count,
                summary_file_count: evidence.summary.summary_file_count,
                summary_parsed_count: evidence.summary.summary_parsed_count,
                artifact_with_terminal_record_count: evidence
                    .summary
                    .artifact_with_terminal_record_count,
                artifact_with_final_message_count: evidence
                    .summary
                    .artifact_with_final_message_count,
                artifact_with_applied_patch_count: evidence
                    .summary
                    .artifact_with_applied_patch_count,
                tool_request_event_count: evidence.summary.tool_request_event_count,
                tool_completed_event_count: evidence.summary.tool_completed_event_count,
                tool_failed_event_count: evidence.summary.tool_failed_event_count,
            },
            EvidenceKind::AgentTurnEvidenceSummary,
            vec![EvidenceLocator::LoadedSummary {
                name: "agent_turns",
            }],
        );

        for (path, artifact) in &evidence.traces {
            self.ingest_agent_turn_artifact(path, artifact, AgentTurnArtifactKind::Trace);
        }
        for (path, artifact) in &evidence.summaries {
            self.ingest_agent_turn_artifact(path, artifact, AgentTurnArtifactKind::Summary);
        }
    }

    fn ingest_agent_turn_artifact(
        &mut self,
        path: &str,
        artifact: &AgentTurnArtifactEvidence,
        kind: AgentTurnArtifactKind,
    ) {
        let node_id = agent_turn_path_node_id(path);
        let mut locators = vec![
            EvidenceLocator::LoadedSummary {
                name: match kind {
                    AgentTurnArtifactKind::Trace => "agent_turn_trace",
                    AgentTurnArtifactKind::Summary => "agent_turn_summary",
                },
            },
            EvidenceLocator::AgentTurnArtifact {
                path: PathBuf::from(path),
                kind,
                task_id: artifact.task_id.clone(),
            },
        ];
        if let Some(node_id) = node_id.as_ref() {
            locators.push(EvidenceLocator::SchedulerNode {
                node_id: node_id.clone(),
            });
        }

        let evidence_id = self.attach_located_evidence(
            EvidenceSubject::AgentTurnArtifact(agent_turn_metadata(artifact)),
            match kind {
                AgentTurnArtifactKind::Trace => EvidenceKind::AgentTurnTraceArtifact,
                AgentTurnArtifactKind::Summary => EvidenceKind::AgentTurnSummaryArtifact,
            },
            locators,
        );

        if let Some(node_id) = node_id.as_ref() {
            self.attach_to_node_branch(node_id, evidence_id);
        }
    }
}

fn agent_turn_metadata(artifact: &AgentTurnArtifactEvidence) -> AgentTurnArtifactMetadata {
    AgentTurnArtifactMetadata {
        task_id: artifact.task_id.clone(),
        selected_model: artifact.selected_model.clone(),
        user_message_id: artifact.user_message_id.clone(),
        event_count: artifact.event_count,
        terminal_outcome: artifact.terminal_outcome.clone(),
        terminal_attempts: artifact.terminal_attempts,
        final_assistant_message_id: artifact.final_assistant_message_id.clone(),
        patch_applied: artifact.patch_applied,
        all_proposals_applied: artifact.all_proposals_applied,
        edit_proposal_count: artifact.edit_proposal_count,
        create_proposal_count: artifact.create_proposal_count,
        expected_file_change_count: artifact.expected_file_change_count,
        llm_prompt_message_count: artifact.llm_prompt_message_count,
        has_llm_response: artifact.has_llm_response,
        tool_request_event_count: artifact.tool_request_event_count,
        tool_completed_event_count: artifact.tool_completed_event_count,
        tool_failed_event_count: artifact.tool_failed_event_count,
    }
}

fn agent_turn_path_node_id(path: &str) -> Option<String> {
    let mut components = path.split('/');
    while let Some(component) = components.next() {
        if component == "nodes" {
            return components
                .next()
                .filter(|node_id| !node_id.is_empty())
                .map(str::to_owned);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use ploke_records::ids::{ArtifactId, Coordinate, EntryId, OperationTarget, RuntimeId};
    use ploke_records::run_profile::{
        Execution, ExecutionStopAfter, Generation, GenerationSource, GenerationSurface,
        RUN_PROFILE_COMMITMENT_SCHEMA_VERSION, RUN_PROFILE_SCHEMA_VERSION,
        RunProfileCommitmentRecord, RunProfileRecord, Search, Selection, SelectionEvidence,
        SelectionStrategy, Storage, Target, TraceJsonl,
    };
    use ploke_records::scheduler::{ChildBudgetRecord, ChildScheduleModeRecord};

    use crate::graph::{
        AgentTurnArtifactKind, CandidateBranchNode, EvidenceKind, EvidenceLocator, EvidenceSubject,
        OperationKey, OperationTargetKey, RunProfileMetadata,
    };
    use crate::{
        AgentTurnArtifactEvidence, AgentTurnEvidence, AgentTurnEvidenceSummary, ChildPlanEvidence,
        ChildPlanSummary, PassiveEvidence, RunProfileEvidence,
    };

    use super::{Builder, agent_turn_metadata};

    #[test]
    fn passive_child_plans_attach_summary_evidence_and_source_records() {
        let mut builder = Builder::default();
        let mut passive = PassiveEvidence::default();
        passive.child_plans = Some(ChildPlanEvidence {
            summary: ChildPlanSummary {
                file_count: 2,
                parsed_count: 2,
                child_count: 3,
                children_with_surface_count: 1,
                rejected_surface_attempt_count: 4,
            },
            index: BTreeMap::new(),
        });

        builder.ingest_passive_evidence(&passive);
        let graph = builder.finish();

        assert_eq!(graph.runtimes.runtimes.len(), 0);
        assert!(graph.candidates.branches.is_empty());
        assert!(graph.child_plans.plans.is_empty());
        assert!(graph.evidence.attachments.values().any(|evidence| {
            evidence.kind == EvidenceKind::ChildPlanSummary
                && evidence.subject
                    == EvidenceSubject::ChildPlanSummary {
                        file_count: 2,
                        parsed_count: 2,
                        child_count: 3,
                        children_with_surface_count: 1,
                        rejected_surface_attempt_count: 4,
                    }
        }));
    }

    #[test]
    fn passive_run_profile_attaches_metadata_without_graph_authority() {
        let mut builder = Builder::default();
        let profile = run_profile_record();
        let commitment = RunProfileCommitmentRecord {
            schema_version: RUN_PROFILE_COMMITMENT_SCHEMA_VERSION.to_owned(),
            profile_path: PathBuf::from("run-profile.toml"),
            sha256: "sha256:profile".to_owned(),
            source_path: Some(PathBuf::from("profiles/overnight.toml")),
            admitted_at: "2026-05-11T00:00:00Z".to_owned(),
        };
        let mut passive = PassiveEvidence::default();
        passive.run_profile = Some(RunProfileEvidence {
            profile: Some(profile.clone()),
            commitment: Some(commitment.clone()),
        });

        builder.ingest_passive_evidence(&passive);
        let graph = builder.finish();

        assert!(graph.runtimes.runtimes.is_empty());
        assert!(graph.artifacts.artifacts.is_empty());
        assert!(graph.candidates.branches.is_empty());
        assert!(graph.authority.epochs_by_lineage.is_empty());
        assert!(graph.evidence.attachments.values().any(|evidence| {
            evidence.kind == EvidenceKind::RunProfileSummary
                && evidence.subject
                    == EvidenceSubject::RunProfileSummary(RunProfileMetadata::from(&profile))
        }));
        assert!(graph.evidence.attachments.values().any(|evidence| {
            evidence.kind == EvidenceKind::RunProfileCommitment
                && evidence.subject == EvidenceSubject::RunProfileCommitment(commitment.clone())
        }));
    }

    #[test]
    fn passive_agent_turn_node_artifact_remains_node_scoped_not_runtime_or_operation_evidence() {
        let runtime_id = RuntimeId("runtime:child".to_owned());
        let coordinate = Coordinate {
            runtime_id: runtime_id.clone(),
            target: OperationTarget::Artifact {
                artifact_id: ArtifactId("artifact-before".to_owned()),
            },
        };
        let mut builder = Builder::default();
        builder.graph.candidates.branches.push(CandidateBranchNode {
            selection_entry_id: EntryId("selection-1".to_owned()),
            payload_index: 0,
            branch_id: "branch-1".to_owned(),
            candidate_id: None,
            source_state_id: None,
            parent_branch_id: None,
            base_artifact_id: None,
            derived_artifact_id: None,
            patch_id: None,
            evidence: Vec::new(),
        });
        builder.observe_node_branch("node-1", "branch-1");
        builder.observe_runtime(&runtime_id);
        builder.observe_operation_coordinate(&coordinate);

        let artifact = agent_turn_artifact();
        let mut passive = PassiveEvidence::default();
        passive.agent_turns = Some(AgentTurnEvidence {
            summary: AgentTurnEvidenceSummary {
                summary_file_count: 1,
                summary_parsed_count: 1,
                artifact_with_terminal_record_count: 1,
                artifact_with_final_message_count: 1,
                artifact_with_applied_patch_count: 1,
                tool_request_event_count: 2,
                tool_completed_event_count: 1,
                tool_failed_event_count: 1,
                ..AgentTurnEvidenceSummary::default()
            },
            traces: BTreeMap::new(),
            summaries: BTreeMap::from([(
                "nodes/node-1/output/agent-turn-summary.json".to_owned(),
                artifact.clone(),
            )]),
        });

        builder.ingest_passive_evidence(&passive);
        let graph = builder.finish();

        assert!(graph.history.blocks.is_empty());
        assert!(graph.authority.epochs_by_lineage.is_empty());
        assert!(graph.evidence.attachments.values().any(|evidence| {
            evidence.kind == EvidenceKind::AgentTurnEvidenceSummary
                && matches!(
                    evidence.subject,
                    EvidenceSubject::AgentTurnEvidenceSummary {
                        summary_file_count: 1,
                        summary_parsed_count: 1,
                        tool_request_event_count: 2,
                        tool_completed_event_count: 1,
                        tool_failed_event_count: 1,
                        ..
                    }
                )
        }));

        let runtime = graph
            .runtimes
            .runtimes
            .get(&runtime_id)
            .expect("runtime exists independently of agent-turn evidence");
        assert!(runtime.evidence.iter().all(|id| {
            graph.evidence.attachments[id].kind != EvidenceKind::AgentTurnSummaryArtifact
        }));

        let operation_key = OperationKey::RuntimeTarget {
            runtime_id,
            target: OperationTargetKey::Artifact {
                artifact_id: ArtifactId("artifact-before".to_owned()),
            },
        };
        let operation = graph
            .operations
            .operations
            .get(&operation_key)
            .expect("operation exists independently of agent-turn evidence");
        assert!(operation.evidence.iter().all(|id| {
            graph.evidence.attachments[id].kind != EvidenceKind::AgentTurnSummaryArtifact
        }));

        let branch = graph
            .candidates
            .branches
            .iter()
            .find(|branch| branch.branch_id == "branch-1")
            .expect("branch joined by node id");
        assert!(branch.evidence.iter().any(|id| {
            let evidence = &graph.evidence.attachments[id];
            evidence.kind == EvidenceKind::AgentTurnSummaryArtifact
                && evidence.subject
                    == EvidenceSubject::AgentTurnArtifact(agent_turn_metadata(&artifact))
                && evidence.locators.iter().any(|locator| {
                    locator
                        == &EvidenceLocator::AgentTurnArtifact {
                            path: PathBuf::from("nodes/node-1/output/agent-turn-summary.json"),
                            kind: AgentTurnArtifactKind::Summary,
                            task_id: "task-1".to_owned(),
                        }
                })
                && evidence.locators.iter().any(|locator| {
                    locator
                        == &EvidenceLocator::SchedulerNode {
                            node_id: "node-1".to_owned(),
                        }
                })
        }));
    }

    #[test]
    fn passive_agent_turn_root_artifact_remains_evidence_only_when_node_join_is_absent() {
        let mut builder = Builder::default();
        let mut passive = PassiveEvidence::default();
        passive.agent_turns = Some(AgentTurnEvidence {
            summary: AgentTurnEvidenceSummary {
                trace_file_count: 1,
                trace_parsed_count: 1,
                ..AgentTurnEvidenceSummary::default()
            },
            traces: BTreeMap::from([("agent-turn-trace.json".to_owned(), agent_turn_artifact())]),
            summaries: BTreeMap::new(),
        });

        builder.ingest_passive_evidence(&passive);
        let graph = builder.finish();

        assert!(graph.history.blocks.is_empty());
        assert!(graph.authority.epochs_by_lineage.is_empty());
        assert!(graph.runtimes.runtimes.is_empty());
        assert!(graph.operations.operations.is_empty());
        assert!(graph.evidence.attachments.values().any(|evidence| {
            evidence.kind == EvidenceKind::AgentTurnTraceArtifact
                && evidence.locators.iter().any(|locator| {
                    locator
                        == &EvidenceLocator::AgentTurnArtifact {
                            path: PathBuf::from("agent-turn-trace.json"),
                            kind: AgentTurnArtifactKind::Trace,
                            task_id: "task-1".to_owned(),
                        }
                })
        }));
    }

    fn agent_turn_artifact() -> AgentTurnArtifactEvidence {
        AgentTurnArtifactEvidence {
            task_id: "task-1".to_owned(),
            selected_model: "openai/gpt-5".to_owned(),
            user_message_id: "user-1".to_owned(),
            event_count: 5,
            terminal_outcome: Some("completed".to_owned()),
            terminal_attempts: Some(1),
            final_assistant_message_id: Some("assistant-1".to_owned()),
            patch_applied: true,
            all_proposals_applied: true,
            edit_proposal_count: 1,
            create_proposal_count: 0,
            expected_file_change_count: 1,
            llm_prompt_message_count: 3,
            has_llm_response: true,
            tool_request_event_count: 2,
            tool_completed_event_count: 1,
            tool_failed_event_count: 1,
        }
    }

    fn run_profile_record() -> RunProfileRecord {
        RunProfileRecord {
            schema_version: RUN_PROFILE_SCHEMA_VERSION.to_owned(),
            name: "overnight-edit-surface".to_owned(),
            storage: Storage {
                worktree_root: PathBuf::from("worktrees"),
            },
            target: Target {
                dataset_key: Some("ripgrep".to_owned()),
                instance: Some("BurntSushi__ripgrep-2209".to_owned()),
            },
            search: Search {
                max_generations: 15,
                max_total_nodes: 96,
                children: ChildBudgetRecord { min: 6, max: 6 },
                schedule: ChildScheduleModeRecord::FullBatch,
                stop_on_first_keep: false,
                require_keep_for_continuation: false,
                explore_from_rejected: true,
            },
            generation: Generation {
                source: GenerationSource::EditSurface,
                surface: Some(GenerationSurface::WorkspaceExceptPlokeEval),
            },
            selection: Selection {
                strategy: SelectionStrategy::HistoryScoreChildProp,
                evidence: SelectionEvidence::OperationalAndProtocol,
                seed: 42,
            },
            execution: Execution {
                stop_after: ExecutionStopAfter::Complete,
                trace_jsonl: TraceJsonl::Auto,
                debug_tools: true,
            },
        }
    }
}
