use serde::{Deserialize, Serialize};

use crate::ui::view::GraphViewDiagnostics;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x00000100000001b3;
const KEY_PREVIEW_LIMIT: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphIdentity {
    pub forest_nodes: usize,
    pub forest_roots: usize,
    pub default_visible_nodes: usize,
    pub default_visible_edges: usize,
    pub artifact_tree_nodes: usize,
    #[serde(rename = "artifact_tree_P_H")]
    pub artifact_tree_p_h: usize,
    #[serde(rename = "artifact_tree_P_C")]
    pub artifact_tree_p_c: usize,
    #[serde(rename = "artifact_tree_P_O")]
    pub artifact_tree_p_o: usize,
    #[serde(rename = "artifact_tree_P_B")]
    pub artifact_tree_p_b: usize,
    pub visible_node_fingerprint: String,
    pub visible_node_keys_preview: Vec<String>,
    pub visible_node_keys_truncated: usize,
}

impl GraphIdentity {
    pub fn from_graph(graph: &ploke_tree::Graph, diagnostics: &GraphViewDiagnostics) -> Self {
        let visible_node_keys = visible_node_keys(graph);
        let visible_node_keys_preview = visible_node_keys
            .iter()
            .take(KEY_PREVIEW_LIMIT)
            .map(|key| key.value.to_owned())
            .collect::<Vec<_>>();
        let visible_node_keys_truncated = visible_node_keys
            .len()
            .saturating_sub(visible_node_keys_preview.len());
        let artifact_tree = graph.artifact_tree();

        Self {
            forest_nodes: graph
                .forest
                .as_ref()
                .map(|forest| forest.nodes.len())
                .unwrap_or_default(),
            forest_roots: graph
                .forest
                .as_ref()
                .map(|forest| forest.roots.len())
                .unwrap_or_default(),
            default_visible_nodes: diagnostics.node_count,
            default_visible_edges: diagnostics.edge_count,
            artifact_tree_nodes: artifact_tree.nodes.len(),
            artifact_tree_p_h: artifact_tree.history_successors.len(),
            artifact_tree_p_c: artifact_tree.produced_child_edges.len(),
            artifact_tree_p_o: artifact_tree.opened_from_edges.len(),
            artifact_tree_p_b: artifact_tree.applied_patch_edges.len(),
            visible_node_fingerprint: fingerprint(&visible_node_keys),
            visible_node_keys_preview,
            visible_node_keys_truncated,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisibleNodeKey<'g> {
    set: &'static str,
    value: &'g str,
}

fn visible_node_keys(graph: &ploke_tree::Graph) -> Vec<VisibleNodeKey<'_>> {
    let mut keys = graph
        .artifact_tree()
        .nodes
        .values()
        .map(|node| VisibleNodeKey {
            set: "A",
            value: node.key.as_str(),
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.value.cmp(right.value));
    keys
}

fn fingerprint(keys: &[VisibleNodeKey<'_>]) -> String {
    let mut hash = FNV_OFFSET;
    for key in keys {
        hash = update_hash(hash, key.set.as_bytes());
        hash = update_hash(hash, &[0]);
        hash = update_hash(hash, key.value.as_bytes());
        hash = update_hash(hash, &[0xff]);
    }
    format!("{hash:016x}")
}

fn update_hash(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::view::{
        GraphConnectivityDiagnostics, GraphReadabilityDiagnostics, GraphViewMode,
        artifact_tree::Shape,
    };
    use eframe::egui::Vec2;
    use ploke_tree::{CampaignRef, Lanes, NodeKey, RunForest, TreeNode};

    #[test]
    fn graph_identity_fingerprints_default_visible_artifact_nodes() {
        let graph = ploke_tree::Graph {
            forest: Some(RunForest {
                campaign: CampaignRef {
                    campaign_id: "campaign".to_owned(),
                    updated_at: "now".to_owned(),
                },
                roots: vec![NodeKey::from("root")],
                nodes: vec![tree_node("child"), tree_node("root")],
                lanes: Lanes {
                    frontier: Vec::new(),
                    completed: Vec::new(),
                    failed: Vec::new(),
                },
                passive_evidence: Default::default(),
                diagnostics: Vec::new(),
            }),
            ..Default::default()
        };
        let identity = GraphIdentity::from_graph(&graph, &diagnostics(2, 1));

        assert_eq!(identity.forest_nodes, 2);
        assert_eq!(identity.forest_roots, 1);
        assert_eq!(identity.default_visible_nodes, 2);
        assert_eq!(identity.default_visible_edges, 1);
        assert!(identity.visible_node_keys_preview.is_empty());
        assert_eq!(identity.visible_node_keys_truncated, 0);
        assert_eq!(identity.visible_node_fingerprint, fingerprint(&[]));
    }

    fn diagnostics(node_count: usize, edge_count: usize) -> GraphViewDiagnostics {
        GraphViewDiagnostics {
            mode: GraphViewMode::ArtifactTree,
            node_count,
            edge_count,
            connectivity: GraphConnectivityDiagnostics::default(),
            artifact_tree: Shape::default(),
            graph_size: Vec2::new(100.0, 100.0),
            viewport_size: Vec2::new(100.0, 100.0),
            aspect_ratio: 1.0,
            viewport_aspect_ratio: 1.0,
            fitted_size: Vec2::new(100.0, 100.0),
            fitted_fill: Vec2::new(1.0, 1.0),
            center_offset: Vec2::ZERO,
            edge_labels: Default::default(),
            readability: GraphReadabilityDiagnostics::default(),
        }
    }

    fn tree_node(key: &str) -> TreeNode {
        TreeNode {
            key: NodeKey::from(key),
            kind: ploke_tree::NodeKind::SchedulerSearchNode,
            authority: ploke_tree::AuthorityLabel::MutableProjection,
            parent: None,
            children: Vec::new(),
            generation: 0,
            branch_id: format!("branch:{key}"),
            parent_branch_id: None,
            candidate_id: format!("candidate:{key}"),
            instance_id: format!("instance:{key}"),
            source_state_id: format!("artifact:{key}"),
            target_relpath: "target.rs".to_owned(),
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            progress: ploke_tree::Progress {
                phase: ploke_tree::Phase::Planned,
                terminality: ploke_tree::Terminality::NonTerminal,
                result_class: ploke_tree::ResultClass::Unknown,
            },
            created_at: "created".to_owned(),
            updated_at: "updated".to_owned(),
            evidence: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
