use serde::{Deserialize, Serialize};

use ploke_tree::graph::artifact_tree as tree;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentBreakdown {
    pub index: usize,
    pub artifacts: Vec<String>,
    pub roots: Vec<String>,
    #[serde(rename = "P_H")]
    pub p_h: Vec<Edge<HistorySource>>,
    #[serde(rename = "P_C")]
    pub p_c: Vec<Edge<ProducedChildSource>>,
    #[serde(rename = "P_O")]
    pub p_o: Vec<Edge<HistorySource>>,
    #[serde(rename = "P_B")]
    pub p_b: Vec<Edge<AppliedPatchSource>>,
}

impl From<(usize, &tree::Component<'_>)> for ComponentBreakdown {
    fn from((index, value): (usize, &tree::Component<'_>)) -> Self {
        Self {
            index,
            artifacts: value
                .artifacts
                .iter()
                .map(|key| key.as_str().to_owned())
                .collect(),
            roots: value
                .roots
                .iter()
                .map(|key| key.as_str().to_owned())
                .collect(),
            p_h: value
                .history_successors
                .iter()
                .map(|edge| Edge {
                    from: edge.from.as_str().to_owned(),
                    to: edge.to.as_str().to_owned(),
                    sources: edge.sources.iter().map(HistorySource::from).collect(),
                })
                .collect(),
            p_c: value
                .produced_child_edges
                .iter()
                .map(|edge| Edge {
                    from: edge.from.as_str().to_owned(),
                    to: edge.to.as_str().to_owned(),
                    sources: edge.sources.iter().map(ProducedChildSource::from).collect(),
                })
                .collect(),
            p_o: value
                .opened_from_edges
                .iter()
                .map(|edge| Edge {
                    from: edge.from.as_str().to_owned(),
                    to: edge.to.as_str().to_owned(),
                    sources: edge.sources.iter().map(HistorySource::from).collect(),
                })
                .collect(),
            p_b: value
                .applied_patch_edges
                .iter()
                .map(|edge| Edge {
                    from: edge.from.as_str().to_owned(),
                    to: edge.to.as_str().to_owned(),
                    sources: edge.sources.iter().map(AppliedPatchSource::from).collect(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge<S> {
    pub from: String,
    pub to: String,
    pub sources: Vec<S>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistorySource {
    pub block_hash: String,
    pub block_height: u64,
    pub lineage_id: String,
}

impl From<&&ploke_tree::graph::HistoryBlockNode> for HistorySource {
    fn from(value: &&ploke_tree::graph::HistoryBlockNode) -> Self {
        Self {
            block_hash: value.block_hash.0.clone(),
            block_height: value.block_height,
            lineage_id: value.lineage_id.0.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedPatchSource {
    pub branch_id: String,
    pub selection_entry_id: String,
    pub payload_index: usize,
    pub candidate_id: Option<String>,
    pub patch_id: Option<String>,
}

impl From<&&ploke_tree::graph::CandidateBranchNode> for AppliedPatchSource {
    fn from(value: &&ploke_tree::graph::CandidateBranchNode) -> Self {
        Self {
            branch_id: value.branch_id.clone(),
            selection_entry_id: value.selection_entry_id.0.clone(),
            payload_index: value.payload_index,
            candidate_id: value.candidate_id.as_ref().map(|id| id.0.clone()),
            patch_id: value.patch_id.as_ref().map(|id| id.0.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducedChildSource {
    pub node_id: String,
    pub parent_node_id: Option<String>,
    pub branch_id: String,
    pub candidate_id: String,
    pub patch_id: Option<String>,
}

impl From<&&ploke_records::child_plan::ChildPlanChildRecord> for ProducedChildSource {
    fn from(value: &&ploke_records::child_plan::ChildPlanChildRecord) -> Self {
        Self {
            node_id: value.node.node_id.as_str().to_owned(),
            parent_node_id: value
                .node
                .parent_node_id
                .as_ref()
                .map(|id| id.as_str().to_owned()),
            branch_id: value.resolved.branch.branch_id.clone(),
            candidate_id: value.resolved.branch.candidate_id.clone(),
            patch_id: value
                .surface
                .as_ref()
                .map(|surface| surface.patch_id.0.clone())
                .or_else(|| value.node.patch_id.as_ref().map(|id| id.0.clone()))
                .or_else(|| value.request.patch_id.as_ref().map(|id| id.0.clone())),
        }
    }
}
