use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use eframe::egui::{Color32, Vec2};
use petgraph::{Directed, stable_graph::StableGraph};
use ploke_records::branch::TreatmentBranchStatus;
use ploke_records::ids::{ArtifactId, SchedulerNodeId};

use crate::graph::{Candidate, Edge, EdgeEndpoint, EdgeId, EdgeKind, Graph as DomainGraph};

use super::diagnostics::graph_diagnostics;
use super::edge::GraphEdgeShape;
use super::order::visual_candidate_order;
use super::style::{EdgeStyle, ViewStyle};
use super::{EdgeLabelDiagnostics, GraphViewDiagnostics};

pub(super) type WidgetGraph = egui_graphs::Graph<
    GraphNode,
    GraphEdgePayload,
    Directed,
    petgraph::stable_graph::DefaultIx,
    egui_graphs::DefaultNodeShape,
    GraphEdgeShape,
>;
type RawGraph = StableGraph<GraphNode, GraphEdgePayload, Directed>;
type WidgetNode = egui_graphs::Node<
    GraphNode,
    GraphEdgePayload,
    Directed,
    petgraph::stable_graph::DefaultIx,
    egui_graphs::DefaultNodeShape,
>;

#[derive(Debug)]
pub(super) struct GraphViewCache {
    revision: Option<u64>,
    style: ViewStyle,
    graph: WidgetGraph,
}

impl Default for GraphViewCache {
    fn default() -> Self {
        Self {
            revision: None,
            style: ViewStyle::default(),
            graph: to_widget_graph(&RawGraph::default(), ViewStyle::default()),
        }
    }
}

impl GraphViewCache {
    pub(super) fn refresh(&mut self, graph: &DomainGraph, style: ViewStyle) -> bool {
        if self.revision == Some(graph.revision()) && self.style == style {
            return false;
        }

        self.revision = Some(graph.revision());
        self.style = style;
        self.graph = build_widget_graph(graph, style);
        true
    }

    pub(super) fn graph_mut(&mut self) -> &mut WidgetGraph {
        &mut self.graph
    }

    pub(super) fn diagnostics(
        &self,
        graph: &DomainGraph,
        viewport_size: Vec2,
        style: ViewStyle,
        edge_labels: EdgeLabelDiagnostics,
    ) -> Option<GraphViewDiagnostics> {
        graph_diagnostics(graph, &self.graph, viewport_size, style, edge_labels)
    }
}

#[derive(Debug, Clone)]
pub(super) enum GraphNode {
    Candidate {
        label: Arc<str>,
        status: TreatmentBranchStatus,
        ruling_epoch: Option<u64>,
    },
    Artifact {
        id: ArtifactId,
    },
}

#[derive(Debug, Clone)]
pub(super) struct GraphEdgePayload {
    pub(super) id: EdgeId,
    pub(super) label: Arc<str>,
    pub(super) color: Color32,
    pub(super) style: EdgeStyle,
}

fn build_widget_graph(graph: &DomainGraph, style: ViewStyle) -> WidgetGraph {
    if graph.candidate_count() > 0 {
        return build_candidate_graph(graph, style);
    }
    build_artifact_graph(graph, style)
}

fn build_candidate_graph(graph: &DomainGraph, style: ViewStyle) -> WidgetGraph {
    let mut raw = RawGraph::default();

    let candidates = visual_candidate_order(graph.candidates().collect());

    let mut index_by_candidate = HashMap::<SchedulerNodeId, _>::new();
    for (index, candidate) in candidates.into_iter().enumerate() {
        let raw_index = raw.add_node(GraphNode::Candidate {
            label: Arc::from(candidate_label(candidate, index)),
            status: candidate.status(),
            ruling_epoch: candidate.ruling_epoch(),
        });
        index_by_candidate.insert(candidate.id().clone(), raw_index);
    }

    let history_endpoints = history_succession_endpoints(graph);
    let mut edges: Vec<_> = graph
        .edges()
        .filter(|edge| candidate_edge_visible(edge, &history_endpoints))
        .collect();
    edges.sort_by(|left, right| {
        edge_order_key(left)
            .cmp(&edge_order_key(right))
            .then_with(|| left.id().as_str().cmp(right.id().as_str()))
    });
    for (index, edge) in edges.into_iter().enumerate() {
        let EdgeEndpoint::Candidate(parent_id) = edge.parent() else {
            continue;
        };
        let EdgeEndpoint::Candidate(candidate_id) = edge.candidate() else {
            continue;
        };
        let parent = index_by_candidate
            .get(parent_id)
            .copied()
            .expect("Graph validates candidate edge parent endpoints");
        let candidate = index_by_candidate
            .get(candidate_id)
            .copied()
            .expect("Graph validates candidate edge candidate endpoints");
        raw.add_edge(
            parent,
            candidate,
            GraphEdgePayload {
                id: edge.id().clone(),
                label: candidate_edge_label(edge, index),
                color: style.edge.colors.color(edge.status()),
                style: style.edge,
            },
        );
    }

    to_widget_graph(&raw, style)
}

fn build_artifact_graph(graph: &DomainGraph, style: ViewStyle) -> WidgetGraph {
    let mut raw = RawGraph::default();

    let mut artifacts: Vec<_> = graph.artifacts().collect();
    artifacts.sort_by(|left, right| left.id().0.cmp(&right.id().0));

    let mut index_by_artifact = HashMap::<ArtifactId, _>::new();
    for artifact in artifacts {
        let index = raw.add_node(GraphNode::Artifact {
            id: artifact.id().clone(),
        });
        index_by_artifact.insert(artifact.id().clone(), index);
    }

    let mut edges: Vec<_> = graph
        .edges()
        .filter(|edge| matches!(edge.kind(), EdgeKind::ArtifactPatch { .. }))
        .collect();
    edges.sort_by(|left, right| left.id().as_str().cmp(right.id().as_str()));
    for (index, edge) in edges.into_iter().enumerate() {
        let EdgeEndpoint::Artifact(parent_id) = edge.parent() else {
            continue;
        };
        let EdgeEndpoint::Artifact(candidate_id) = edge.candidate() else {
            continue;
        };
        let parent = index_by_artifact
            .get(parent_id)
            .copied()
            .expect("Graph validates artifact edge parent endpoints");
        let candidate = index_by_artifact
            .get(candidate_id)
            .copied()
            .expect("Graph validates artifact edge candidate endpoints");
        raw.add_edge(
            parent,
            candidate,
            GraphEdgePayload {
                id: edge.id().clone(),
                label: edge_label(edge, index),
                color: style.edge.colors.color(edge.status()),
                style: style.edge,
            },
        );
    }

    to_widget_graph(&raw, style)
}

fn to_widget_graph(raw: &RawGraph, style: ViewStyle) -> WidgetGraph {
    egui_graphs::to_graph_custom(
        raw,
        |node: &mut WidgetNode| {
            let visual = NodeVisual::from_node(node.payload(), style);
            node.set_label(visual.label);
            if let Some(color) = visual.color {
                node.set_color(color);
            }
            node.display_mut().radius = style.layout.node_radius;
        },
        |_edge| {},
    )
}

struct NodeVisual {
    label: String,
    color: Option<eframe::egui::Color32>,
}

impl NodeVisual {
    fn from_node(node: &GraphNode, style: ViewStyle) -> Self {
        match node {
            GraphNode::Candidate {
                label,
                status,
                ruling_epoch,
            } => Self {
                label: candidate_node_label(label, *ruling_epoch),
                color: Some(style.edge.colors.color(*status)),
            },
            GraphNode::Artifact { id } => Self {
                label: style.labels.artifact(id.0.as_str()),
                color: None,
            },
        }
    }
}

fn candidate_label(candidate: &Candidate, index: usize) -> String {
    if candidate.generation() == 0 {
        return "parent".to_owned();
    }

    format!("C{} g{}", index + 1, candidate.generation())
}

fn candidate_node_label(label: &str, ruling_epoch: Option<u64>) -> String {
    match ruling_epoch {
        Some(epoch) => format!("B{epoch} {label}"),
        None => label.to_owned(),
    }
}

fn candidate_edge_label(edge: &Edge, index: usize) -> Arc<str> {
    if matches!(edge.kind(), EdgeKind::HistorySuccession { .. }) {
        return Arc::from("S");
    }

    if edge.status() == TreatmentBranchStatus::Selected {
        Arc::from("S")
    } else {
        Arc::from(format!("C{}", index + 1))
    }
}

fn candidate_edge_visible(
    edge: &Edge,
    history_endpoints: &HashSet<(SchedulerNodeId, SchedulerNodeId)>,
) -> bool {
    match edge.kind() {
        EdgeKind::HistorySuccession { .. } => true,
        EdgeKind::CandidateTransition => {
            let (EdgeEndpoint::Candidate(parent), EdgeEndpoint::Candidate(candidate)) =
                (edge.parent(), edge.candidate())
            else {
                return false;
            };
            !history_endpoints.contains(&(parent.clone(), candidate.clone()))
        }
        EdgeKind::ArtifactPatch { .. } => false,
    }
}

fn history_succession_endpoints(
    graph: &DomainGraph,
) -> HashSet<(SchedulerNodeId, SchedulerNodeId)> {
    graph
        .edges()
        .filter(|edge| matches!(edge.kind(), EdgeKind::HistorySuccession { .. }))
        .filter_map(|edge| {
            let (EdgeEndpoint::Candidate(parent), EdgeEndpoint::Candidate(candidate)) =
                (edge.parent(), edge.candidate())
            else {
                return None;
            };
            Some((parent.clone(), candidate.clone()))
        })
        .collect()
}

fn edge_order_key(edge: &Edge) -> (u8, u64) {
    match edge.kind() {
        EdgeKind::HistorySuccession { block_height } => (0, *block_height),
        EdgeKind::CandidateTransition => (1, 0),
        EdgeKind::ArtifactPatch { .. } => (2, 0),
    }
}

fn edge_label(edge: &Edge, index: usize) -> Arc<str> {
    if edge.patch().is_some() {
        Arc::from(format!("P{}", index + 1))
    } else {
        Arc::from(status_label(edge.status()))
    }
}

fn status_label(status: TreatmentBranchStatus) -> &'static str {
    match status {
        TreatmentBranchStatus::Synthesized => "synthesized",
        TreatmentBranchStatus::Selected => "selected",
        TreatmentBranchStatus::Applied => "applied",
        TreatmentBranchStatus::Restored => "restored",
        TreatmentBranchStatus::Dropped => "dropped",
    }
}
