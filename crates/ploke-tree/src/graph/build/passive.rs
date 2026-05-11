use crate::graph::{EvidenceKind, EvidenceLocator, EvidenceSubject};
use crate::{ChildPlanEvidence, EvaluationEvidence, PassiveEvidence, ProtocolArtifactsEvidence};

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
    }

    fn ingest_child_plan_summary(&mut self, evidence: &ChildPlanEvidence) {
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
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::graph::{EvidenceKind, EvidenceSubject};
    use crate::{ChildPlanEvidence, ChildPlanSummary, PassiveEvidence};

    use super::Builder;

    #[test]
    fn passive_child_plans_attach_summary_only_evidence() {
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
}
