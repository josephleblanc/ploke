use ploke_records::identity::ParentIdentityRecord;
use ploke_records::invocation::{SuccessorCompletionRecord, SuccessorReadyRecord};

use crate::graph::{EvidenceId, EvidenceKind, EvidenceLocator, EvidenceSubject};

use super::Builder;

impl Builder {
    pub(super) fn ingest_parent_identity(&mut self, parent: &ParentIdentityRecord) -> EvidenceId {
        let evidence_id = self.attach_located_evidence(
            EvidenceSubject::SchedulerNode(parent.node_id.clone()),
            EvidenceKind::ParentIdentity,
            vec![EvidenceLocator::ParentIdentity {
                node_id: parent.node_id.clone(),
                parent_id: parent.parent_id.clone(),
            }],
        );
        self.attach_to_branch(&parent.branch_id, evidence_id);
        evidence_id
    }

    pub(super) fn ingest_successor_ready(&mut self, ready: &SuccessorReadyRecord) -> EvidenceId {
        let evidence_id = self.attach_located_evidence(
            EvidenceSubject::Runtime(ready.runtime_id.clone()),
            EvidenceKind::SuccessorReady,
            vec![EvidenceLocator::SuccessorReady {
                node_id: ready.node_id.clone(),
                runtime_id: ready.runtime_id.clone(),
            }],
        );
        self.attach_to_runtime(&ready.runtime_id, evidence_id);
        evidence_id
    }

    pub(super) fn ingest_successor_completion(
        &mut self,
        completion: &SuccessorCompletionRecord,
    ) -> EvidenceId {
        let evidence_id = self.attach_located_evidence(
            EvidenceSubject::Runtime(completion.runtime_id.clone()),
            EvidenceKind::SuccessorCompletion,
            vec![EvidenceLocator::SuccessorCompletion {
                node_id: completion.node_id.clone(),
                runtime_id: completion.runtime_id.clone(),
            }],
        );
        self.attach_to_runtime(&completion.runtime_id, evidence_id);
        evidence_id
    }
}
