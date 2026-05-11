//! Read-only imports from typed run projections into the operator graph.
//!
//! This module adapts records loaded by `ploke-tree`; it does not parse
//! Prototype 1 files directly and it does not add runtime authority.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::path::Path;

use ploke_records::branch::TreatmentBranchStatus;
use ploke_records::history::{EntryPayloadRecord, SealedBlockRecord};
use ploke_records::ids::{BranchId, SchedulerNodeId};
use ploke_records::scheduler::NodeRecord;
use ploke_tree::{FsRunStore, FsRunStoreError, RunForestInput};

use crate::graph::{
    Artifact, Candidate, Edge, EdgeId, Evidence, EvidenceId, Graph, GraphError, Subject,
};

pub fn graph_from_run_root(run_root: impl AsRef<Path>) -> Result<Graph, ImportError> {
    let store = FsRunStore::new(run_root.as_ref());
    let input = store.load()?;
    let history_blocks = store.load_history_blocks()?;

    graph_from_run_records(&input, &history_blocks).map_err(ImportError::Graph)
}

pub fn graph_from_run_records(
    input: &RunForestInput,
    history_blocks: &[SealedBlockRecord],
) -> Result<Graph, GraphError> {
    let selected_branches = selected_branches(input, history_blocks);
    let selected_nodes = selected_nodes(history_blocks);
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
    history_blocks: &[SealedBlockRecord],
) -> BTreeSet<BranchId> {
    let mut selected = BTreeSet::new();

    if let Some(decision) = &input.scheduler.last_continuation_decision {
        if let Some(branch_id) = &decision.selected_next_branch_id {
            selected.insert(branch_id.clone());
        }
    }

    for block in history_blocks {
        for entry in &block.entries {
            let EntryPayloadRecord::SelectionDecision(selection) = &entry.core.payload else {
                continue;
            };
            if let Some(branch_id) = &selection.decision.selected_branch_id {
                selected.insert(BranchId(branch_id.clone()));
            }
        }
    }

    selected
}

fn selected_nodes(history_blocks: &[SealedBlockRecord]) -> BTreeSet<SchedulerNodeId> {
    let mut selected = BTreeSet::new();
    for block in history_blocks {
        for entry in &block.entries {
            let EntryPayloadRecord::SelectionDecision(selection) = &entry.core.payload else {
                continue;
            };
            selected.insert(SchedulerNodeId(
                selection.decision.candidate_node_id.clone(),
            ));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn real_run_imports_execution_spine() {
        let run_root =
            std::env::var("PLOKE_EGUI_RUN_ROOT").expect("PLOKE_EGUI_RUN_ROOT must be set");
        let graph = graph_from_run_root(run_root).expect("import graph from run root");

        println!(
            "graph candidates={} candidate_edges={} artifacts={} artifact_edges={} selected_candidates={} evidence={}",
            graph.candidate_count(),
            graph.candidate_edge_count(),
            graph.artifact_count(),
            graph.artifact_edge_count(),
            graph
                .candidates()
                .filter(|candidate| candidate.status() == TreatmentBranchStatus::Selected)
                .count(),
            graph.evidence_count()
        );

        assert!(
            graph.candidate_count() > 0,
            "real run should import candidate nodes"
        );
        assert!(
            graph.candidate_edge_count() > 0,
            "real run should import candidate edges"
        );
        assert!(
            graph.artifact_count() > 0,
            "real run should import artifact nodes"
        );
        assert!(
            graph.artifact_edge_count() > 0,
            "real run should import artifact edges"
        );
        assert!(
            graph
                .candidates()
                .any(|candidate| candidate.status() == TreatmentBranchStatus::Selected),
            "real run should mark the History-selected candidate"
        );
    }
}
