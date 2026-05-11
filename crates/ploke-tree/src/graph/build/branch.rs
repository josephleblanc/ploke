use crate::BranchRegistryEvidence;
use crate::graph::{EvidenceKind, EvidenceLocator, EvidenceSubject};

use super::Builder;

impl Builder {
    pub(super) fn ingest_branch_registry_summary(&mut self, evidence: &BranchRegistryEvidence) {
        self.attach_located_evidence(
            EvidenceSubject::BranchRegistrySummary {
                source_node_count: evidence.source_node_count,
                branch_count: evidence.branch_count,
                active_target_count: evidence.active_target_count,
                record_count: evidence.record_count,
                registry_snapshot_count: evidence.registry_snapshot_count,
                parent_comparison_count: evidence.parent_comparison_count,
                latest_campaign_id: evidence.latest_campaign_id.clone(),
                latest_recorded_at: evidence.latest_recorded_at.clone(),
            },
            EvidenceKind::BranchRegistrySummary,
            vec![EvidenceLocator::LoadedSummary {
                name: "branch_registry",
            }],
        );
    }
}
