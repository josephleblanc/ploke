use ploke_records::journal::{
    ActiveCheckoutAdvancedRecord, BuildRecord, ChildArtifactCommittedRecord, ChildRecord,
    CompletionRecord, JournalEntry, ParentStartedRecord, ReadyRecord, SpawnRecord, SuccessorRecord,
    SuccessorStateRecord, TransitionRecord,
};

use crate::TransitionJournal;
use crate::graph::{EvidenceId, EvidenceKind, EvidenceLocator, EvidenceSubject};

use super::Builder;

impl Builder {
    pub(super) fn ingest_transition_journal(&mut self, journal: &TransitionJournal) {
        self.attach_located_evidence(
            EvidenceSubject::TransitionJournalLoadedEntries {
                loaded_entry_count: journal.len(),
            },
            EvidenceKind::TransitionJournalLoadedEntries,
            vec![EvidenceLocator::LoadedSummary {
                name: "transition_journal_loaded_records",
            }],
        );

        for record in journal.iter() {
            self.ingest_transition_journal_entry(record.line_number, &record.record);
        }
    }

    fn ingest_transition_journal_entry(&mut self, line_number: usize, entry: &JournalEntry) {
        match entry {
            JournalEntry::ParentStarted(record) => self.ingest_parent_started(line_number, record),
            JournalEntry::Resource(record) => {
                let evidence_id = self.attach_journal_line(
                    line_number,
                    EvidenceSubject::SchedulerNode(record.node_id.clone()),
                );
                if let Some(runtime_id) = record.runtime_id.as_ref() {
                    self.attach_to_runtime(runtime_id, evidence_id);
                }
            }
            JournalEntry::ChildArtifactCommitted(record) => {
                self.ingest_child_artifact_committed(line_number, record);
            }
            JournalEntry::ActiveCheckoutAdvanced(record) => {
                self.ingest_active_checkout_advanced(line_number, record);
            }
            JournalEntry::SuccessorHandoff(record) => {
                let evidence_id = self.attach_journal_line(
                    line_number,
                    EvidenceSubject::Runtime(record.runtime_id.clone()),
                );
                self.attach_to_runtime(&record.runtime_id, evidence_id);
            }
            JournalEntry::Successor(record) => self.ingest_successor_journal(line_number, record),
            JournalEntry::MaterializeBranch(record) => {
                self.ingest_transition_record(line_number, record)
            }
            JournalEntry::BuildChild(record) => self.ingest_build_record(line_number, record),
            JournalEntry::SpawnChild(record) => self.ingest_spawn_record(line_number, record),
            JournalEntry::Child(record) => self.ingest_child_record(line_number, record),
            JournalEntry::ChildReady(record) => self.ingest_ready_record(line_number, record),
            JournalEntry::ObserveChild(record) => {
                self.ingest_completion_record(line_number, record)
            }
        }
    }

    fn ingest_parent_started(&mut self, line_number: usize, record: &ParentStartedRecord) {
        let evidence_id = self.attach_journal_line(
            line_number,
            EvidenceSubject::SchedulerNode(record.parent_identity.node_id.clone()),
        );
        self.attach_to_branch(&record.parent_identity.branch_id, evidence_id);
        if let Some(runtime_id) = record.handoff_runtime_id.as_ref() {
            self.attach_to_runtime(runtime_id, evidence_id);
        }
    }

    fn ingest_child_artifact_committed(
        &mut self,
        line_number: usize,
        record: &ChildArtifactCommittedRecord,
    ) {
        let evidence_id = self.attach_journal_line(
            line_number,
            EvidenceSubject::SchedulerNode(record.node_id.clone()),
        );
        self.attach_to_branch(&record.child_branch, evidence_id);
    }

    fn ingest_active_checkout_advanced(
        &mut self,
        line_number: usize,
        record: &ActiveCheckoutAdvancedRecord,
    ) {
        let evidence_id = self.attach_journal_line(
            line_number,
            EvidenceSubject::Branch(record.selected_branch.clone()),
        );
        self.attach_to_branch(&record.selected_branch, evidence_id);
    }

    fn ingest_successor_journal(&mut self, line_number: usize, record: &SuccessorRecord) {
        let subject = record
            .runtime_id
            .as_ref()
            .map(|runtime_id| EvidenceSubject::Runtime(runtime_id.clone()))
            .unwrap_or_else(|| EvidenceSubject::SchedulerNode(record.node_id.clone()));
        let evidence_id = self.attach_journal_line(line_number, subject);

        if let Some(runtime_id) = record.runtime_id.as_ref() {
            self.attach_to_runtime(runtime_id, evidence_id);
        }

        match &record.state {
            SuccessorStateRecord::Selected {
                decision,
                selection_decision,
            } => {
                if let Some(branch_id) = decision.selected_next_branch_id.as_ref() {
                    self.attach_to_branch(branch_id.as_str(), evidence_id);
                }
                if let Some(branch_id) = selection_decision
                    .as_ref()
                    .and_then(|decision| decision.selected_branch_id.as_ref())
                {
                    self.attach_to_branch(branch_id, evidence_id);
                }
            }
            SuccessorStateRecord::Checkout {
                selected_branch, ..
            } => {
                self.attach_to_branch(selected_branch, evidence_id);
            }
            SuccessorStateRecord::Spawned { .. }
            | SuccessorStateRecord::Ready { .. }
            | SuccessorStateRecord::TimedOut { .. }
            | SuccessorStateRecord::ExitedBeforeReady { .. }
            | SuccessorStateRecord::Completed { .. } => {}
        }
    }

    fn ingest_transition_record(&mut self, line_number: usize, record: &TransitionRecord) {
        let evidence_id = self.attach_journal_line(
            line_number,
            EvidenceSubject::Branch(record.refs.branch_id.clone()),
        );
        self.attach_to_branch(&record.refs.branch_id, evidence_id);
    }

    fn ingest_build_record(&mut self, line_number: usize, record: &BuildRecord) {
        let evidence_id = self.attach_journal_line(
            line_number,
            EvidenceSubject::Branch(record.refs.branch_id.clone()),
        );
        self.attach_to_branch(&record.refs.branch_id, evidence_id);
    }

    fn ingest_spawn_record(&mut self, line_number: usize, record: &SpawnRecord) {
        let evidence_id = self.attach_journal_line(
            line_number,
            EvidenceSubject::Runtime(record.runtime_id.clone()),
        );
        self.attach_to_runtime(&record.runtime_id, evidence_id);
        self.attach_to_branch(&record.refs.branch_id, evidence_id);
    }

    fn ingest_child_record(&mut self, line_number: usize, record: &ChildRecord) {
        let evidence_id = self.attach_journal_line(
            line_number,
            EvidenceSubject::Runtime(record.runtime_id.clone()),
        );
        self.attach_to_runtime(&record.runtime_id, evidence_id);
        self.attach_to_branch(&record.refs.branch_id, evidence_id);
    }

    fn ingest_ready_record(&mut self, line_number: usize, record: &ReadyRecord) {
        let evidence_id = self.attach_journal_line(
            line_number,
            EvidenceSubject::Runtime(record.runtime_id.clone()),
        );
        self.attach_to_runtime(&record.runtime_id, evidence_id);
        self.attach_to_branch(&record.refs.branch_id, evidence_id);
    }

    fn ingest_completion_record(&mut self, line_number: usize, record: &CompletionRecord) {
        let evidence_id = self.attach_journal_line(
            line_number,
            EvidenceSubject::Runtime(record.runtime_id.clone()),
        );
        self.attach_to_runtime(&record.runtime_id, evidence_id);
        self.attach_to_branch(&record.refs.branch_id, evidence_id);
    }

    fn attach_journal_line(&mut self, line_number: usize, subject: EvidenceSubject) -> EvidenceId {
        self.attach_located_evidence(
            subject,
            EvidenceKind::TransitionJournalEntry,
            vec![EvidenceLocator::TransitionJournalLine { line_number }],
        )
    }
}

#[cfg(test)]
mod tests {
    use ploke_records::journal::{JournalEntry, SuccessorRecord, SuccessorStateRecord};
    use ploke_records::scheduler::{ContinuationDecisionRecord, ContinuationDispositionRecord};

    use crate::graph::{CandidateBranchNode, EvidenceKind, EvidenceSubject};
    use crate::{JsonlRecord, TransitionJournal};

    use super::Builder;

    #[test]
    fn transition_journal_selected_successor_attaches_to_runtime_and_branch() {
        let mut builder = Builder::default();
        builder.graph.candidates.branches.push(CandidateBranchNode {
            selection_entry_id: ploke_records::ids::EntryId("entry-1".to_owned()),
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

        let runtime_id = ploke_records::ids::RuntimeId("runtime-1".to_owned());
        let journal = TransitionJournal {
            entries: vec![JsonlRecord {
                line_number: 7,
                record: JournalEntry::Successor(SuccessorRecord {
                    runtime_id: Some(runtime_id.clone()),
                    recorded_at: ploke_records::ids::RecordedAt(0),
                    campaign_id: "campaign-1".to_owned(),
                    node_id: "node-1".to_owned(),
                    state: SuccessorStateRecord::Selected {
                        decision: ContinuationDecisionRecord {
                            disposition: ContinuationDispositionRecord::ContinueReady,
                            selected_next_branch_id: Some(ploke_records::ids::BranchId(
                                "branch-1".to_owned(),
                            )),
                            selected_branch_disposition: None,
                            next_generation: 2,
                            total_nodes_after_continue: 3,
                        },
                        selection_decision: None,
                    },
                }),
            }],
        };

        builder.ingest_transition_journal(&journal);
        let graph = builder.finish();

        let branch = graph
            .candidates
            .branches
            .iter()
            .find(|branch| branch.branch_id == "branch-1")
            .expect("branch is still present");
        assert!(branch.evidence.iter().any(|id| {
            graph.evidence.attachments[id].kind == EvidenceKind::TransitionJournalEntry
        }));

        let runtime = graph
            .runtimes
            .runtimes
            .get(&runtime_id)
            .expect("journal runtime is graph-reachable");
        assert!(runtime.evidence.iter().any(|id| {
            graph.evidence.attachments[id].kind == EvidenceKind::TransitionJournalEntry
        }));
    }

    #[test]
    fn transition_journal_loaded_entries_do_not_claim_parse_completeness() {
        let mut builder = Builder::default();
        let journal = TransitionJournal {
            entries: vec![JsonlRecord {
                line_number: 11,
                record: JournalEntry::Successor(SuccessorRecord {
                    runtime_id: None,
                    recorded_at: ploke_records::ids::RecordedAt(0),
                    campaign_id: "campaign-1".to_owned(),
                    node_id: "node-1".to_owned(),
                    state: SuccessorStateRecord::ExitedBeforeReady { exit_code: None },
                }),
            }],
        };

        builder.ingest_transition_journal(&journal);
        let graph = builder.finish();

        assert!(graph.evidence.attachments.values().any(|evidence| {
            evidence.kind == EvidenceKind::TransitionJournalLoadedEntries
                && evidence.subject
                    == EvidenceSubject::TransitionJournalLoadedEntries {
                        loaded_entry_count: 1,
                    }
        }));
        assert!(
            !graph
                .evidence
                .attachments
                .values()
                .any(|evidence| evidence.kind == EvidenceKind::TransitionJournalSummary)
        );
    }
}
