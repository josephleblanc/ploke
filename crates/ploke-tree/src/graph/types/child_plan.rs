use std::collections::BTreeMap;

use ploke_records::child_plan::{ChildPlanChildRecord, ChildPlanRecord};
use ploke_records::ids::{PatchId, SchedulerNodeId};

/// Typed child-plan records attached to the read-side graph.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChildPlanIndex {
    pub plans: BTreeMap<SchedulerNodeId, ChildPlanRecord>,
}

impl ChildPlanIndex {
    pub fn plan_for_parent_node_id(&self, node_id: &str) -> Option<&ChildPlanRecord> {
        self.plans
            .iter()
            .find(|(parent_node_id, _)| parent_node_id.as_str() == node_id)
            .map(|(_, plan)| plan)
    }

    pub fn child_for_node_id(&self, node_id: &str) -> Option<&ChildPlanChildRecord> {
        self.plans
            .values()
            .flat_map(|plan| &plan.children)
            .find(|child| child.node.node_id.as_str() == node_id)
    }

    pub fn child_for_patch_id(&self, patch_id: &PatchId) -> Option<&ChildPlanChildRecord> {
        self.plans
            .values()
            .flat_map(|plan| &plan.children)
            .find(|child| child.node.patch_id.as_ref() == Some(patch_id))
    }
}
