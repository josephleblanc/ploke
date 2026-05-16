use serde::{Deserialize, Serialize};

use ploke_tree::graph::artifact_tree as tree;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentBreakdown {
    pub index: usize,
    pub artifacts: Vec<String>,
    pub roots: Vec<String>,
    #[serde(rename = "P_H")]
    pub p_h: Vec<Edge<HistorySource>>,
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
