use std::collections::{BTreeMap, BTreeSet};

use ploke_records::ids::{BranchId, SchedulerNodeId};
use ploke_tree::CoarseHistorySpine;

use crate::graph::{EdgeId, EvidenceId};

pub(super) fn ruling_epochs(spine: &CoarseHistorySpine) -> BTreeMap<SchedulerNodeId, u64> {
    spine
        .steps
        .iter()
        .enumerate()
        .filter_map(|(sequence, step)| {
            step.ruling_parent
                .as_ref()
                .map(|ruler| (ruler.clone(), sequence as u64))
        })
        .collect()
}

pub(super) fn selected_nodes(spine: &CoarseHistorySpine) -> BTreeSet<SchedulerNodeId> {
    spine
        .steps
        .iter()
        .filter_map(|step| step.selected_node.clone())
        .collect()
}

pub(super) fn selected_branches(spine: &CoarseHistorySpine) -> BTreeSet<BranchId> {
    spine
        .steps
        .iter()
        .filter_map(|step| step.selected_branch.clone())
        .collect()
}

pub(super) fn history_edge_id(block_height: u64) -> EdgeId {
    EdgeId::new(format!("history-succession:{block_height}"))
}

pub(super) fn history_evidence_id(block_height: u64) -> EvidenceId {
    EvidenceId::new(format!("sealed-history:{block_height}"))
}
