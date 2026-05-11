use std::collections::HashMap;
use std::sync::Arc;

use petgraph::{Directed, stable_graph::StableGraph};
use ploke_records::branch::TreatmentBranchStatus;
use ploke_records::ids::{ArtifactId, SchedulerNodeId};

use crate::graph::{Candidate, Edge, EdgeEndpoint, EdgeKind, Graph as DomainGraph};

use super::edge::GraphEdgeShape;
use super::style::{EdgeStyle, LabelStyle, ViewStyle};

type WidgetGraph = egui_graphs::Graph<
    GraphNode,
    GraphEdgePayload,
    Directed,
    petgraph::stable_graph::DefaultIx,
    egui_graphs::DefaultNodeShape,
    GraphEdgeShape,
>;
type RawGraph = StableGraph<GraphNode, GraphEdgePayload, Directed>;

#[derive(Debug)]
pub(super) struct ArtifactViewCache {
    revision: Option<u64>,
    style: ViewStyle,
    graph: WidgetGraph,
}

impl Default for ArtifactViewCache {
    fn default() -> Self {
        Self {
            revision: None,
            style: ViewStyle::default(),
            graph: to_widget_graph(&RawGraph::default(), LabelStyle::default()),
        }
    }
}

impl ArtifactViewCache {
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
}

#[derive(Debug, Clone)]
pub(super) enum GraphNode {
    Candidate { label: Arc<str> },
    Artifact { id: ArtifactId },
}

#[derive(Debug, Clone)]
pub(super) struct GraphEdgePayload {
    pub(super) id: Arc<str>,
    pub(super) label: Arc<str>,
    pub(super) status: TreatmentBranchStatus,
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

    let mut candidates: Vec<_> = graph.candidates().collect();
    candidates.sort_by(|left, right| {
        left.generation()
            .cmp(&right.generation())
            .then_with(|| left.id().as_str().cmp(right.id().as_str()))
    });

    let mut index_by_candidate = HashMap::<SchedulerNodeId, _>::new();
    for (index, candidate) in candidates.into_iter().enumerate() {
        let raw_index = raw.add_node(GraphNode::Candidate {
            label: Arc::from(candidate_label(candidate, index)),
        });
        index_by_candidate.insert(candidate.id().clone(), raw_index);
    }

    let mut edges: Vec<_> = graph
        .edges()
        .filter(|edge| edge.kind() == &EdgeKind::CandidateTransition)
        .collect();
    edges.sort_by(|left, right| left.id().as_str().cmp(right.id().as_str()));
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
                id: Arc::from(edge.id().as_str()),
                label: candidate_edge_label(edge, index),
                status: edge.status(),
                style: style.edge,
            },
        );
    }

    to_widget_graph(&raw, style.labels)
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
                id: Arc::from(edge.id().as_str()),
                label: edge_label(edge, index),
                status: edge.status(),
                style: style.edge,
            },
        );
    }

    to_widget_graph(&raw, style.labels)
}

fn to_widget_graph(raw: &RawGraph, labels: LabelStyle) -> WidgetGraph {
    egui_graphs::to_graph_custom(
        raw,
        |node| match node.payload() {
            GraphNode::Candidate { label, .. } => node.set_label(label.to_string()),
            GraphNode::Artifact { id } => node.set_label(labels.artifact(id.0.as_str())),
        },
        |_edge| {},
    )
}

fn candidate_label(candidate: &Candidate, index: usize) -> String {
    if candidate.generation() == 0 {
        return "parent".to_owned();
    }

    format!("C{} g{}", index + 1, candidate.generation())
}

fn candidate_edge_label(edge: &Edge, index: usize) -> Arc<str> {
    if edge.status() == TreatmentBranchStatus::Selected {
        Arc::from("selected")
    } else {
        Arc::from(format!("C{}", index + 1))
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
