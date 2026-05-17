#[path = "selection/membership.rs"]
mod membership;

#[cfg(test)]
#[path = "selection/tests.rs"]
mod tests;

use std::collections::btree_map;

use ploke_records::history::{
    AdmittedEntryRecord, EvaluationPayloadRecord, SelectionDecisionEntryRecord,
    TraversalCandidateSourceRecord,
};
use ploke_records::ids::{CandidateId, Coordinate, RuntimeId};

use crate::graph::{
    CandidateBranchNode, CandidateMembershipKey, CandidateMembershipNode, CandidateNode,
    CandidateSource, EvidenceKind, EvidenceSubject, GraphWarningKind, MetricCandidateKey,
    MetricCandidateNode, MetricSetNode, SelectionNode,
};

use super::Builder;

impl Builder {
    pub(super) fn ingest_selection(
        &mut self,
        entry: &AdmittedEntryRecord,
        selection: &SelectionDecisionEntryRecord,
    ) {
        self.attach_evidence(
            EvidenceSubject::Selection(entry.core.entry_id.clone()),
            EvidenceKind::SelectionDecision,
            Vec::new(),
        );

        let candidate_set_root = selection
            .candidate_set
            .as_ref()
            .map(|candidate_set| candidate_set.root.clone());
        let selection_node = SelectionNode {
            entry_id: entry.core.entry_id.clone(),
            procedure_or_policy: selection.procedure_or_policy.clone(),
            scope: selection.scope.clone(),
            selected_candidate: selection.selected_candidate.clone(),
            selected_occurrence_id: selection.selected_occurrence_id.clone(),
            selected_membership_id: selection.selected_membership_id.clone(),
            candidate_set_root: candidate_set_root.clone(),
            considered_count: selection.considered.len(),
            projection_failure_count: selection.projection_failures.len(),
            metric_set_id: selection.metrics.id.clone(),
            decision_outcome: selection.decision.outcome,
        };
        self.graph
            .selections
            .selections
            .insert(entry.core.entry_id.clone(), selection_node);
        self.ingest_selection_metrics(entry, selection, candidate_set_root.as_ref());

        let candidate_set = selection.candidate_set.as_ref();
        let memberships = candidate_set.map(|candidate_set| candidate_set.memberships.as_slice());
        if let Some(memberships) = memberships
            && memberships.len() != selection.considered.len()
        {
            self.warn(
                GraphWarningKind::CandidateSetMembershipCountMismatch,
                format!(
                    "selection entry {} considered={} memberships={}",
                    entry.core.entry_id.0,
                    selection.considered.len(),
                    memberships.len()
                ),
            );
        }

        if let Some(candidate_set) = candidate_set {
            for member in &candidate_set.memberships {
                if let Some(membership_id) = member.membership_id.clone() {
                    let key = CandidateMembershipKey {
                        candidate_set_root: candidate_set.root.clone(),
                        membership_id: membership_id.clone(),
                    };
                    let node = CandidateMembershipNode {
                        membership_id: membership_id.clone(),
                        candidate_set_root: candidate_set.root.clone(),
                        occurrence_id: member.occurrence_id.clone(),
                        candidate_subject: member.candidate.clone(),
                        selection_entry_id: entry.core.entry_id.clone(),
                        payload_hash: member.payload_hash.0.clone(),
                    };
                    if let btree_map::Entry::Vacant(e) =
                        self.graph.candidates.memberships.entry(key)
                    {
                        e.insert(node);
                    } else {
                        self.warn(
                            GraphWarningKind::DuplicateCandidateMembershipId,
                            format!(
                                "selection entry {} candidate_set repeats membership_id {}",
                                entry.core.entry_id.0, membership_id.0
                            ),
                        );
                    }
                }
            }
        }

        let selected_membership_seen =
            selection
                .selected_membership_id
                .as_ref()
                .is_none_or(|selected| {
                    memberships
                        .unwrap_or(&[])
                        .iter()
                        .any(|member| member.membership_id.as_ref() == Some(selected))
                });
        let selected = membership::Selected::from_selection(selection);
        for (index, payload) in selection.considered.iter().enumerate() {
            let source = selection
                .considered_sources
                .get(index)
                .map(|source| match source {
                    TraversalCandidateSourceRecord::History => CandidateSource::History,
                    TraversalCandidateSourceRecord::CurrentGeneration => {
                        CandidateSource::CurrentGeneration
                    }
                });
            let membership =
                memberships.and_then(|memberships| {
                    match membership::for_payload(payload, Some(&selected), memberships) {
                        membership::Resolution::Matched(member) => Some(member),
                        membership::Resolution::Missing => {
                            self.warn(
                                GraphWarningKind::CandidateSetMembershipMissingForPayload,
                                format!(
                                    "selection entry {} payload_index={} candidate {} has no matching candidate_set membership",
                                    entry.core.entry_id.0, index, payload.candidate.value
                                ),
                            );
                            None
                        }
                        membership::Resolution::Ambiguous {
                            matching_memberships,
                        } => {
                            self.warn(
                                GraphWarningKind::CandidateSetMembershipAmbiguousForPayload,
                                format!(
                                    "selection entry {} payload_index={} candidate {} matches {} candidate_set memberships; membership not attached",
                                    entry.core.entry_id.0, index, payload.candidate.value, matching_memberships
                                ),
                            );
                            None
                        }
                    }
                });
            let coordinate = payload
                .sealed_evidence
                .as_ref()
                .map(|evidence| &evidence.coordinate);
            let artifact_after = payload.artifact.as_ref().and_then(|artifact| {
                artifact
                    .resolved
                    .branch
                    .derived_artifact_id
                    .clone()
                    .or_else(|| {
                        artifact
                            .surface
                            .as_ref()
                            .map(|surface| surface.after.artifact_id.clone())
                    })
            });
            if let Some(artifact_id) = artifact_after.as_ref() {
                self.observe_artifact_id(artifact_id);
            }
            let patch_id = payload.artifact.as_ref().and_then(|artifact| {
                artifact.resolved.branch.patch_id.clone().or_else(|| {
                    artifact
                        .surface
                        .as_ref()
                        .map(|surface| surface.patch_id.clone())
                })
            });

            let evidence_id = self.attach_evidence(
                EvidenceSubject::Candidate {
                    selection_entry_id: entry.core.entry_id.clone(),
                    payload_index: index,
                },
                EvidenceKind::CandidatePayload,
                payload.source_refs.clone(),
            );
            if let Some(coordinate) = operation_coordinate(payload) {
                self.attach_to_operation_coordinate(&coordinate, evidence_id);
            }
            if let Some(sealed_evidence) = payload.sealed_evidence.as_ref() {
                for branch in &sealed_evidence.branches {
                    self.graph.candidates.branches.push(CandidateBranchNode {
                        selection_entry_id: entry.core.entry_id.clone(),
                        payload_index: index,
                        branch_id: branch.branch_id.clone(),
                        candidate_id: branch.candidate_id.clone().map(CandidateId),
                        source_state_id: branch.source_state_id.clone(),
                        parent_branch_id: payload
                            .artifact
                            .as_ref()
                            .and_then(|artifact| artifact.resolved.parent_branch_id.clone()),
                        base_artifact_id: payload.artifact.as_ref().and_then(|artifact| {
                            artifact
                                .surface
                                .as_ref()
                                .map(|surface| surface.base.artifact_id.clone())
                        }),
                        derived_artifact_id: artifact_after.clone(),
                        patch_id: patch_id.clone(),
                        evidence: vec![evidence_id],
                    });
                }
            }

            self.graph.candidates.candidates.push(CandidateNode {
                selection_entry_id: entry.core.entry_id.clone(),
                payload_index: index,
                subject: payload.candidate.clone(),
                source,
                occurrence_id: membership.and_then(|member| member.occurrence_id.clone()),
                membership_id: membership.and_then(|member| member.membership_id.clone()),
                membership_key: membership.and_then(|member| {
                    member
                        .membership_id
                        .clone()
                        .zip(candidate_set_root.clone())
                        .map(
                            |(membership_id, candidate_set_root)| CandidateMembershipKey {
                                candidate_set_root,
                                membership_id,
                            },
                        )
                }),
                node_id: coordinate
                    .map(|coordinate| coordinate.node_id.clone())
                    .or_else(|| {
                        payload
                            .selection_input
                            .as_ref()
                            .map(|input| input.candidate.node_id.clone())
                    }),
                branch_id: coordinate
                    .and_then(|coordinate| coordinate.branch_id.clone())
                    .or_else(|| {
                        payload
                            .selection_input
                            .as_ref()
                            .map(|input| input.candidate.branch_id.clone())
                    }),
                generation: coordinate
                    .and_then(|coordinate| coordinate.generation)
                    .or_else(|| {
                        payload
                            .selection_input
                            .as_ref()
                            .map(|input| input.candidate.generation)
                    }),
                primary_runtime_id: coordinate
                    .and_then(|coordinate| coordinate.primary_runtime_id.clone()),
                artifact_after,
                patch_id,
                evidence: vec![evidence_id],
            });
        }

        if !selected_membership_seen {
            self.warn(
                GraphWarningKind::SelectedMembershipMissing,
                format!(
                    "selection entry {} selected_membership_id is absent from candidate_set",
                    entry.core.entry_id.0
                ),
            );
        }
    }

    fn ingest_selection_metrics(
        &mut self,
        entry: &AdmittedEntryRecord,
        selection: &SelectionDecisionEntryRecord,
        candidate_set_root: Option<&ploke_records::history::CandidateSetRootRecord>,
    ) {
        let metric_set = &selection.metrics;
        if metric_set.considered_order_hash != selection.considered_order_hash {
            self.warn(
                GraphWarningKind::SelectionMetricBindingMismatch,
                format!(
                    "selection entry {} metrics considered_order_hash does not match decision",
                    entry.core.entry_id.0
                ),
            );
        }
        let expected_root = candidate_set_root.map(|root| &root.0);
        if metric_set.candidate_set_root.as_ref() != expected_root {
            self.warn(
                GraphWarningKind::SelectionMetricBindingMismatch,
                format!(
                    "selection entry {} metrics candidate_set_root does not match decision",
                    entry.core.entry_id.0
                ),
            );
        }

        self.graph.metrics.sets.insert(
            metric_set.id.clone(),
            MetricSetNode {
                metric_set_id: metric_set.id.clone(),
                selection_entry_id: entry.core.entry_id.clone(),
                considered_order_hash: metric_set.considered_order_hash.clone(),
                candidate_set_root: metric_set.candidate_set_root.clone(),
                candidate_count: metric_set.candidates.len(),
            },
        );

        for candidate in &metric_set.candidates {
            self.graph.metrics.candidates.insert(
                MetricCandidateKey {
                    metric_set_id: metric_set.id.clone(),
                    payload_index: candidate.payload_index,
                },
                MetricCandidateNode {
                    metric_set_id: metric_set.id.clone(),
                    selection_entry_id: entry.core.entry_id.clone(),
                    payload_index: candidate.payload_index,
                    payload_hash: candidate.payload_hash.clone(),
                    candidate: candidate.candidate.clone(),
                    occurrence_id: candidate.occurrence_id.clone(),
                    membership_id: candidate.membership_id.clone(),
                    imp_at_k: candidate.imp_at_k.clone(),
                },
            );
        }
    }
}

fn operation_coordinate(payload: &EvaluationPayloadRecord) -> Option<Coordinate> {
    let runtime_id = payload
        .sealed_evidence
        .as_ref()?
        .coordinate
        .primary_runtime_id
        .as_ref()?;
    let target = payload.artifact.as_ref()?.node.operation_target.clone()?;
    Some(Coordinate {
        runtime_id: RuntimeId(runtime_id.clone()),
        target,
    })
}
