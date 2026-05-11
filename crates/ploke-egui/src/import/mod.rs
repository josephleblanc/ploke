//! Read-only imports from typed run projections into the operator graph.
//!
//! This module adapts records loaded by `ploke-tree`; it does not parse
//! Prototype 1 files directly and it does not add runtime authority.

mod history;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::path::Path;

use ploke_records::branch::TreatmentBranchStatus;
use ploke_records::history::SealedBlockRecord;
use ploke_records::ids::{BranchId, SchedulerNodeId};
use ploke_records::scheduler::NodeRecord;
use ploke_tree::{CoarseHistorySpine, FsRunStore, FsRunStoreError, RunForestInput, RunRecordSet};

use crate::graph::{
    Artifact, Candidate, Edge, EdgeId, Evidence, EvidenceId, Graph, GraphError, Subject,
};

use history::{history_edge_id, history_evidence_id};

pub fn graph_from_run_root(run_root: impl AsRef<Path>) -> Result<Graph, ImportError> {
    let store = FsRunStore::new(run_root.as_ref());
    let records = store.load_record_set()?;

    graph_from_run_records(&records).map_err(ImportError::Graph)
}

pub fn graph_from_run_records(records: &RunRecordSet) -> Result<Graph, GraphError> {
    graph_from_run_parts(&records.forest_input, &records.history_blocks)
}

fn graph_from_run_parts(
    input: &RunForestInput,
    history_blocks: &[SealedBlockRecord],
) -> Result<Graph, GraphError> {
    let history_spine = ploke_tree::build_coarse_history_spine(history_blocks);
    let selected_branches = selected_branches(input, &history_spine);
    let selected_nodes = history::selected_nodes(&history_spine);
    let ruling_epochs = history::ruling_epochs(&history_spine);
    let nodes = merged_node_records(input);

    let mut graph = Graph::new();
    for node in nodes.values() {
        let status = if selected_nodes.contains(&node.node_id)
            || selected_branches.contains(&node.branch_id)
        {
            TreatmentBranchStatus::Selected
        } else {
            TreatmentBranchStatus::Synthesized
        };

        let artifact_edge_id = if let (Some(base), Some(derived), Some(patch)) = (
            node.base_artifact_id.clone(),
            node.derived_artifact_id.clone(),
            node.patch_id.clone(),
        ) {
            graph.insert_artifact(Artifact::new(base.clone()));
            graph.insert_artifact(Artifact::new(derived.clone()));

            let edge_id = artifact_edge_id(&node.node_id);
            let evidence_id = evidence_id(&node.node_id);
            let edge = Edge::artifact_patch(edge_id.clone(), base, derived)
                .with_status(status)
                .with_patch(crate::graph::PatchEvidence::new(patch, evidence_id.clone()))
                .with_evidence(evidence_id.clone());
            graph.insert_edge(edge)?;
            graph.insert_evidence(Evidence::new(evidence_id, Subject::Edge(edge_id.clone())));
            Some(edge_id)
        } else {
            None
        };

        let mut candidate = Candidate::new(
            node.node_id.clone(),
            node.branch_id.clone(),
            node.candidate_id.clone(),
            node.generation,
        )
        .with_parent(node.parent_node_id.clone())
        .with_status(status);
        if let Some(epoch) = ruling_epochs.get(&node.node_id).copied() {
            candidate = candidate.with_ruling_epoch(epoch);
        }
        if let Some(edge_id) = artifact_edge_id {
            candidate = candidate.with_artifact_edge(edge_id);
        }
        graph.insert_candidate(candidate);
        graph.insert_evidence(Evidence::new(
            evidence_id(&node.node_id),
            Subject::Candidate(node.node_id.clone()),
        ));
    }

    for node in nodes.values() {
        let Some(parent) = node.parent_node_id.clone() else {
            continue;
        };
        let status = if selected_nodes.contains(&node.node_id)
            || selected_branches.contains(&node.branch_id)
        {
            TreatmentBranchStatus::Selected
        } else {
            TreatmentBranchStatus::Synthesized
        };
        let edge_id = candidate_edge_id(&parent, &node.node_id);
        let evidence_id = evidence_id(&node.node_id);
        let edge = Edge::candidate_transition(edge_id.clone(), parent, node.node_id.clone())
            .with_status(status)
            .with_evidence(evidence_id.clone());
        graph.insert_edge(edge)?;
        graph.insert_evidence(Evidence::new(evidence_id, Subject::Edge(edge_id)));
    }

    for step in &history_spine.steps {
        let (Some(ruler), Some(selected)) = (&step.ruling_parent, &step.selected_node) else {
            continue;
        };
        graph.insert_history_succession_edge(
            history_edge_id(step.block_height),
            ruler.clone(),
            selected.clone(),
            step.block_height,
            history_evidence_id(step.block_height),
        )?;
    }

    Ok(graph)
}

fn merged_node_records(input: &RunForestInput) -> BTreeMap<SchedulerNodeId, &NodeRecord> {
    let mut nodes = BTreeMap::new();
    for node in &input.scheduler.nodes {
        nodes.insert(node.node_id.clone(), node);
    }
    for node in &input.node_records {
        nodes.insert(node.node_id.clone(), node);
    }
    nodes
}

fn selected_branches(
    input: &RunForestInput,
    history_spine: &CoarseHistorySpine,
) -> BTreeSet<BranchId> {
    let mut selected = history::selected_branches(history_spine);

    if let Some(decision) = &input.scheduler.last_continuation_decision {
        if let Some(branch_id) = &decision.selected_next_branch_id {
            selected.insert(branch_id.clone());
        }
    }

    selected
}

fn artifact_edge_id(node_id: &SchedulerNodeId) -> EdgeId {
    EdgeId::new(format!("artifact-edge:{}", node_id.as_str()))
}

fn candidate_edge_id(parent: &SchedulerNodeId, node_id: &SchedulerNodeId) -> EdgeId {
    EdgeId::new(format!(
        "candidate-edge:{}->{}",
        parent.as_str(),
        node_id.as_str()
    ))
}

fn evidence_id(node_id: &SchedulerNodeId) -> EvidenceId {
    EvidenceId::new(format!("scheduler-node:{}", node_id.as_str()))
}

#[derive(Debug)]
pub enum ImportError {
    Store(FsRunStoreError),
    Graph(GraphError),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(f, "{error}"),
            Self::Graph(error) => write!(f, "{error}"),
        }
    }
}

impl Error for ImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Graph(error) => Some(error),
        }
    }
}

impl From<FsRunStoreError> for ImportError {
    fn from(error: FsRunStoreError) -> Self {
        Self::Store(error)
    }
}
