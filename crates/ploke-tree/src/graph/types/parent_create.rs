use ploke_records::child_plan::ChildPlanChildRecord;
use ploke_records::history::{SurfaceEvidenceRecord, SurfaceProposalProducerRecord};
use ploke_records::ids::PatchId;

use super::{
    AgentTurnArtifactMetadata, CandidateBranchNode, EvidenceAttachment, EvidenceKind,
    EvidenceSubject, Graph,
};

#[derive(Debug, Clone, Copy)]
pub enum ParentCreateLookup<'g, 'q> {
    Attempt(ParentCreateAttempt<'g>),
    Unavailable(ParentCreateUnavailable<'q>),
    Ambiguous {
        count: usize,
        reason: ParentCreateUnavailable<'q>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ParentCreateAttempt<'g> {
    graph: &'g Graph,
    child: &'g ChildPlanChildRecord,
    branch: Option<&'g CandidateBranchNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentCreateUnavailable<'a> {
    MissingJoin {
        record: &'static str,
        key: &'static str,
        value: &'a str,
    },
    AmbiguousJoin {
        record: &'static str,
        key: &'static str,
        value: &'a str,
    },
}

impl Graph {
    pub fn parent_create_for_node_id<'g, 'q>(
        &'g self,
        node_id: &'q str,
    ) -> ParentCreateLookup<'g, 'q> {
        let Some(child) = self.child_plans.child_for_node_id(node_id) else {
            return ParentCreateLookup::Unavailable(ParentCreateUnavailable::MissingJoin {
                record: "child_plan",
                key: "node_id",
                value: node_id,
            });
        };
        self.parent_create_from_child(child)
    }

    pub fn parent_create_for_patch_id<'g, 'q>(
        &'g self,
        patch_id: &'q PatchId,
    ) -> ParentCreateLookup<'g, 'q> {
        let Some(child) = self.child_plans.child_for_patch_id(patch_id) else {
            return ParentCreateLookup::Unavailable(ParentCreateUnavailable::MissingJoin {
                record: "child_plan",
                key: "patch_id",
                value: patch_id.0.as_str(),
            });
        };
        self.parent_create_from_child(child)
    }

    pub fn parent_create_for_artifact_key<'g, 'q>(
        &'g self,
        artifact_key: &'q str,
    ) -> ParentCreateLookup<'g, 'q> {
        let mut first = None;
        let mut count = 0;
        for child in self
            .child_plans
            .plans
            .values()
            .flat_map(|plan| plan.children.iter())
            .filter(|child| child_derived_artifact(child) == Some(artifact_key))
        {
            count += 1;
            first.get_or_insert(child);
        }

        match (first, count) {
            (Some(child), 1) => self.parent_create_from_child(child),
            (Some(_), count) => ParentCreateLookup::Ambiguous {
                count,
                reason: ParentCreateUnavailable::AmbiguousJoin {
                    record: "child_plan",
                    key: "derived_artifact",
                    value: artifact_key,
                },
            },
            (None, _) => ParentCreateLookup::Unavailable(ParentCreateUnavailable::MissingJoin {
                record: "child_plan",
                key: "derived_artifact",
                value: artifact_key,
            }),
        }
    }

    fn parent_create_from_child<'g, 'q>(
        &'g self,
        child: &'g ChildPlanChildRecord,
    ) -> ParentCreateLookup<'g, 'q> {
        ParentCreateLookup::Attempt(ParentCreateAttempt {
            graph: self,
            child,
            branch: self
                .candidates
                .branches
                .iter()
                .find(|branch| branch.branch_id == child.resolved.branch.branch_id),
        })
    }
}

impl<'g> ParentCreateAttempt<'g> {
    pub fn child(&self) -> &'g ChildPlanChildRecord {
        self.child
    }

    pub fn branch(&self) -> Option<&'g CandidateBranchNode> {
        self.branch
    }

    pub fn surface(&self) -> Option<&'g SurfaceEvidenceRecord> {
        self.child.surface.as_ref()
    }

    pub fn surface_producer(&self) -> Option<&'g SurfaceProposalProducerRecord> {
        self.surface()
            .map(|surface| surface.effective_proposal_producer())
    }

    pub fn agent_turns(&self) -> impl Iterator<Item = &'g AgentTurnArtifactMetadata> + '_ {
        self.branch_evidence()
            .filter_map(|evidence| match &evidence.subject {
                EvidenceSubject::AgentTurnArtifact(metadata) => Some(metadata),
                _ => None,
            })
    }

    pub fn branch_evidence(&self) -> impl Iterator<Item = &'g EvidenceAttachment> + '_ {
        self.branch
            .into_iter()
            .flat_map(|branch| branch.evidence.iter())
            .filter_map(|id| self.graph.evidence.attachments.get(id))
    }

    pub fn candidate_evaluation_count(&self) -> usize {
        self.branch_evidence()
            .filter(|evidence| evidence.kind == EvidenceKind::CandidateEvaluation)
            .count()
    }

    pub fn source_ref_count(&self) -> usize {
        self.branch.map_or(0, |branch| branch.evidence.len())
    }
}

fn child_derived_artifact(child: &ChildPlanChildRecord) -> Option<&str> {
    child
        .surface
        .as_ref()
        .map(|surface| surface.after.artifact_id.0.as_str())
        .or_else(|| {
            child
                .node
                .derived_artifact_id
                .as_ref()
                .map(|id| id.0.as_str())
        })
}
