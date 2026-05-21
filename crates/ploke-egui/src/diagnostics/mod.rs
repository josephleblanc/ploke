//! Native diagnostic snapshots emitted by the running operator UI.
//!
//! This module persists render-only observations made by the live egui view.
//! It does not recompute layout or interpret run records.

mod default_view;
mod graph_identity;

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use eframe::egui::Vec2;
use serde::{Deserialize, Serialize};

#[cfg(any(test, all(not(target_arch = "wasm32"), feature = "dev")))]
use crate::ui::inspector::SelectionInspector;
use crate::ui::inspector::SelectionInspectorSnapshot;
use crate::ui::view::{GraphSelectionDetail, GraphViewDiagnostics};
pub use default_view::{
    CheckStatus as ContractCheckStatus, ComponentBreakdown, Layout as DefaultViewLayout,
    Report as DefaultViewContractReport, WidthBudget as DefaultViewWidthBudget,
};
pub use graph_identity::GraphIdentity;

const SNAPSHOT_VERSION: &str = "ploke-egui.graph-diagnostics.v1";
#[cfg(any(test, all(not(target_arch = "wasm32"), feature = "dev")))]
const SELECTED_GRAPH_ITEM_VERSION: &str = "ploke-egui.selected-graph-item.v1";
const DEFAULT_MAX_SNAPSHOTS: u64 = 10;

#[derive(Debug)]
pub struct SnapshotSink {
    root: PathBuf,
    max_snapshots: u64,
    sequence: u64,
    last_bytes: Option<Vec<u8>>,
}

impl SnapshotSink {
    pub fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("snapshots"))?;
        Ok(Self {
            root,
            max_snapshots: DEFAULT_MAX_SNAPSHOTS,
            sequence: 0,
            last_bytes: None,
        })
    }

    pub fn observe(&mut self, observation: SnapshotObservation<'_>) -> io::Result<bool> {
        let comparison = Snapshot::from_observation(self.sequence, observation.clone());
        let comparison_bytes = serde_json::to_vec_pretty(&comparison).map_err(io::Error::other)?;
        if self.last_bytes.as_ref() == Some(&comparison_bytes) {
            return Ok(false);
        }

        self.sequence = self.sequence.saturating_add(1);
        let snapshot = Snapshot::from_observation(self.sequence, observation);
        let bytes = serde_json::to_vec_pretty(&snapshot).map_err(io::Error::other)?;
        self.last_bytes = Some(bytes.clone());
        self.write_snapshot(&snapshot, bytes)?;
        Ok(true)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn write_snapshot(&self, snapshot: &Snapshot<'_>, bytes: Vec<u8>) -> io::Result<()> {
        fs::write(self.root.join("latest.json"), &bytes)?;
        fs::write(self.root.join("latest.txt"), snapshot.render_text())?;
        let slot = ((snapshot.sequence - 1) % self.max_snapshots) + 1;
        fs::write(
            self.root.join("snapshots").join(format!("{slot:02}.json")),
            bytes,
        )?;
        fs::write(
            self.root.join("snapshots").join(format!("{slot:02}.txt")),
            snapshot.render_text(),
        )
    }
}

pub fn write_manual_snapshot(observation: SnapshotObservation<'_>) -> io::Result<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/manual-diagnostics");
    fs::create_dir_all(&root)?;

    let snapshot = Snapshot::from_observation(1, observation);
    let bytes = serde_json::to_vec_pretty(&snapshot).map_err(io::Error::other)?;
    fs::write(root.join("latest.json"), bytes)?;
    fs::write(root.join("latest.txt"), snapshot.render_text())?;
    Ok(root.join("latest.txt"))
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotObservation<'a> {
    pub diagnostics: GraphViewDiagnostics,
    pub artifact_components: Vec<ComponentBreakdown>,
    pub graph_has_content: bool,
    pub hide_unconsidered_children: bool,
    pub run_error: Option<String>,
    pub run: Option<RunSnapshot>,
    pub graph_identity: Option<GraphIdentity>,
    pub selected: Option<SelectionSnapshot<'a>>,
    pub selected_inspector: Option<SelectionInspectorSnapshot<'a>>,
}

impl<'a> SnapshotObservation<'a> {
    pub fn new(diagnostics: GraphViewDiagnostics) -> Self {
        Self {
            graph_has_content: diagnostics.node_count > 0,
            hide_unconsidered_children: false,
            diagnostics,
            artifact_components: Vec::new(),
            run_error: None,
            run: None,
            graph_identity: None,
            selected: None,
            selected_inspector: None,
        }
    }

    pub fn with_graph_has_content(mut self, graph_has_content: bool) -> Self {
        self.graph_has_content = graph_has_content;
        self
    }

    pub fn with_run_error(mut self, run_error: Option<String>) -> Self {
        self.run_error = run_error;
        self
    }

    pub fn with_hide_unconsidered_children(mut self, hide: bool) -> Self {
        self.hide_unconsidered_children = hide;
        self
    }

    pub fn with_run(mut self, run: Option<RunSnapshot>) -> Self {
        self.run = run;
        self
    }

    pub fn with_graph_identity(mut self, graph_identity: GraphIdentity) -> Self {
        self.graph_identity = Some(graph_identity);
        self
    }

    pub fn with_selected(mut self, selected: Option<&'a GraphSelectionDetail>) -> Self {
        self.selected = selected.map(SelectionSnapshot::from);
        self
    }

    pub(crate) fn with_selected_inspector(
        mut self,
        inspector: Option<SelectionInspectorSnapshot<'a>>,
    ) -> Self {
        self.selected_inspector = inspector;
        self
    }

    pub fn with_artifact_components(mut self, components: Vec<ComponentBreakdown>) -> Self {
        self.artifact_components = components;
        self
    }
}

pub fn artifact_component_breakdown(graph: &ploke_tree::Graph) -> Vec<ComponentBreakdown> {
    default_view::component_breakdown(graph)
}

#[cfg(any(test, all(not(target_arch = "wasm32"), feature = "dev")))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct SelectedGraphItemSnapshot<'a> {
    pub schema_version: &'static str,
    pub selected: &'a GraphSelectionDetail,
    pub selected_inspector: SelectionInspectorSnapshot<'a>,
}

#[cfg(any(test, all(not(target_arch = "wasm32"), feature = "dev")))]
impl<'a> SelectedGraphItemSnapshot<'a> {
    pub(crate) fn from_graph(
        graph: &'a ploke_tree::Graph,
        selected: &'a GraphSelectionDetail,
    ) -> Self {
        Self {
            schema_version: SELECTED_GRAPH_ITEM_VERSION,
            selected,
            selected_inspector: SelectionInspector::from_graph(graph, selected).snapshot(selected),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSnapshot {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SelectionSnapshot<'a> {
    pub kind: &'a str,
    pub label: &'a str,
}

impl<'a> From<&'a GraphSelectionDetail> for SelectionSnapshot<'a> {
    fn from(value: &'a GraphSelectionDetail) -> Self {
        Self {
            kind: value.kind.as_str(),
            label: value.label.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot<'a> {
    pub schema_version: String,
    pub sequence: u64,
    pub run: Option<RunSnapshot>,
    pub view_mode: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub component_count_before_anchoring: usize,
    pub component_roots_before_anchoring: Vec<String>,
    pub hidden_record_count: usize,
    pub hidden_edge_count: usize,
    pub hidden_evidence_count: usize,
    pub hidden_operation_count: usize,
    pub hidden_unattached_component_count: usize,
    pub synthetic_anchors_visible: bool,
    pub graph_size: Pair,
    pub viewport_size: Pair,
    pub aspect_ratio: f32,
    pub viewport_aspect_ratio: f32,
    pub fitted_size: Pair,
    pub fitted_fill: Pair,
    pub center_offset: Pair,
    pub edge_label_count: usize,
    pub edge_label_collisions: usize,
    pub edge_label_edge_intersections: usize,
    pub edge_label_edge_collisions: usize,
    pub edge_edge_crossings: usize,
    pub edge_crossings_by_kind: CrossingKinds,
    pub candidate_clutter_crossings: usize,
    pub long_edge_count: usize,
    pub backtracking_edge_count: usize,
    pub selected_path_crossings: usize,
    pub default_view_contract: DefaultViewContractReport<'a>,
    pub findings: Vec<SnapshotFinding>,
}

impl<'a> Snapshot<'a> {
    pub fn from_observation(sequence: u64, observation: SnapshotObservation<'a>) -> Self {
        let SnapshotObservation {
            diagnostics,
            artifact_components,
            graph_has_content,
            hide_unconsidered_children,
            run_error,
            run,
            graph_identity,
            selected,
            selected_inspector,
        } = observation;
        let default_view_contract = DefaultViewContractReport::from_parts(
            &diagnostics,
            graph_has_content,
            hide_unconsidered_children,
            run_error,
            graph_identity,
            selected,
            selected_inspector,
            artifact_components,
        );
        Self::from_diagnostics_and_contract(sequence, run, diagnostics, default_view_contract)
    }

    #[cfg(test)]
    fn from_diagnostics(sequence: u64, diagnostics: GraphViewDiagnostics) -> Self {
        let observation = SnapshotObservation::new(diagnostics);
        Self::from_observation(sequence, observation)
    }

    fn from_diagnostics_and_contract(
        sequence: u64,
        run: Option<RunSnapshot>,
        diagnostics: GraphViewDiagnostics,
        default_view_contract: DefaultViewContractReport<'a>,
    ) -> Self {
        let findings = ranked_findings(&diagnostics);
        let component_roots_before_anchoring = default_view_contract
            .center
            .component_breakdown
            .iter()
            .flat_map(|component| component.roots.iter().cloned())
            .collect();

        Self {
            schema_version: SNAPSHOT_VERSION.to_owned(),
            sequence,
            run,
            view_mode: diagnostics.mode.as_str().to_owned(),
            node_count: diagnostics.node_count,
            edge_count: diagnostics.edge_count,
            component_count_before_anchoring: diagnostics
                .connectivity
                .component_count_before_anchoring,
            component_roots_before_anchoring,
            hidden_record_count: diagnostics.connectivity.hidden_record_count,
            hidden_edge_count: diagnostics.connectivity.hidden_edge_count,
            hidden_evidence_count: diagnostics.connectivity.hidden_evidence_count,
            hidden_operation_count: diagnostics.connectivity.hidden_operation_count,
            hidden_unattached_component_count: diagnostics
                .connectivity
                .hidden_unattached_component_count,
            synthetic_anchors_visible: diagnostics.connectivity.synthetic_anchors_visible,
            graph_size: Pair::from(diagnostics.graph_size),
            viewport_size: Pair::from(diagnostics.viewport_size),
            aspect_ratio: diagnostics.aspect_ratio,
            viewport_aspect_ratio: diagnostics.viewport_aspect_ratio,
            fitted_size: Pair::from(diagnostics.fitted_size),
            fitted_fill: Pair::from(diagnostics.fitted_fill),
            center_offset: Pair::from(diagnostics.center_offset),
            edge_label_count: diagnostics.edge_labels.label_count,
            edge_label_collisions: diagnostics.edge_labels.collision_count,
            edge_label_edge_intersections: diagnostics.edge_labels.edge_intersection_count,
            edge_label_edge_collisions: diagnostics.edge_labels.edge_collision_count,
            edge_edge_crossings: diagnostics.readability.edge_edge_crossings,
            edge_crossings_by_kind: CrossingKinds {
                artifact_artifact: diagnostics
                    .readability
                    .edge_crossings_by_kind
                    .artifact_artifact,
                candidate_candidate: diagnostics
                    .readability
                    .edge_crossings_by_kind
                    .candidate_candidate,
                mixed: diagnostics.readability.edge_crossings_by_kind.mixed,
            },
            candidate_clutter_crossings: diagnostics
                .readability
                .edge_crossings_by_kind
                .candidate_candidate,
            long_edge_count: diagnostics.readability.long_edge_count,
            backtracking_edge_count: diagnostics.readability.backtracking_edge_count,
            selected_path_crossings: diagnostics.readability.selected_path_crossings,
            default_view_contract,
            findings,
        }
    }

    pub fn render_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "ploke-egui snapshot #{}, mode {}",
            self.sequence, self.view_mode
        );
        if let Some(run) = &self.run {
            let _ = writeln!(out, "run: {}", run.name);
            let _ = writeln!(out, "run root: {}", run.path);
        } else {
            let _ = writeln!(out, "run: none");
        }
        let _ = writeln!(
            out,
            "nodes: {}, edges: {}",
            self.node_count, self.edge_count
        );
        let _ = writeln!(
            out,
            "components: {}, synthetic anchors visible: {}",
            self.component_count_before_anchoring, self.synthetic_anchors_visible
        );
        let _ = writeln!(
            out,
            "readability: crossings={}, selected_path_crossings={}, long_edges={}, backtracking={}",
            self.edge_edge_crossings,
            self.selected_path_crossings,
            self.long_edge_count,
            self.backtracking_edge_count
        );
        let _ = writeln!(out);
        self.render_sidebar_section_text_into(&mut out);
        let _ = writeln!(out);
        self.default_view_contract.render_text_into(&mut out);
        if self.findings.is_empty() {
            let _ = writeln!(out, "findings: none");
        } else {
            let _ = writeln!(out, "findings:");
            for finding in &self.findings {
                let _ = writeln!(out, "- {:?}: {}", finding.severity, finding.title);
                for evidence in &finding.evidence {
                    let _ = writeln!(out, "  - {evidence}");
                }
            }
        }
        out
    }

    fn render_sidebar_section_text_into(&self, out: &mut String) {
        let _ = writeln!(out, "sidebar diagnostics:");
        if let Some(run) = &self.run {
            let _ = writeln!(out, "Run: {}", run.name);
            let _ = writeln!(out, "Run root: {}", run.path);
        } else {
            let _ = writeln!(out, "Run: none");
        }
        let _ = writeln!(out, "Mode: {}", self.view_mode);
        let _ = writeln!(out, "View nodes: {}", self.node_count);
        let _ = writeln!(out, "View edges: {}", self.edge_count);
        let _ = writeln!(
            out,
            "Components before anchors: {}",
            self.component_count_before_anchoring
        );
        let _ = writeln!(
            out,
            "Hidden records: {}, hidden edges: {}, hidden evidence: {}, hidden operations: {}, unattached components: {}",
            self.hidden_record_count,
            self.hidden_edge_count,
            self.hidden_evidence_count,
            self.hidden_operation_count,
            self.hidden_unattached_component_count
        );
        let _ = writeln!(
            out,
            "Synthetic anchors visible: {}",
            self.synthetic_anchors_visible
        );
        let _ = writeln!(
            out,
            "Graph: {:.0} x {:.0}",
            self.graph_size.x, self.graph_size.y
        );
        let _ = writeln!(out, "Aspect: {:.2}", self.aspect_ratio);
        let _ = writeln!(
            out,
            "Fit fill: {:.0}% x {:.0}%",
            self.fitted_fill.x * 100.0,
            self.fitted_fill.y * 100.0
        );
        let _ = writeln!(
            out,
            "Edge labels: {}, label collisions: {}, edge intersections: {}, edge collisions: {}",
            self.edge_label_count,
            self.edge_label_collisions,
            self.edge_label_edge_intersections,
            self.edge_label_edge_collisions
        );
        let _ = writeln!(
            out,
            "Candidate clutter: {}",
            self.candidate_clutter_crossings
        );
        let _ = writeln!(
            out,
            "Crossings: {}, artifact/artifact: {}, mixed: {}, long edges: {}, backtracking: {}, selected crossings: {}",
            self.edge_edge_crossings,
            self.edge_crossings_by_kind.artifact_artifact,
            self.edge_crossings_by_kind.mixed,
            self.long_edge_count,
            self.backtracking_edge_count,
            self.selected_path_crossings
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFinding {
    pub severity: FindingSeverity,
    pub title: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FindingSeverity {
    Info,
    Low,
    Medium,
    High,
}

fn ranked_findings(diagnostics: &GraphViewDiagnostics) -> Vec<SnapshotFinding> {
    let mut findings = Vec::new();

    let aspect_mismatch = diagnostics.aspect_ratio / diagnostics.viewport_aspect_ratio.max(0.01);
    if diagnostics.fitted_fill.y < 0.15 || aspect_mismatch > 6.0 {
        findings.push(SnapshotFinding {
            severity: FindingSeverity::High,
            title: "Graph composition collapses into a thin horizontal strip.".to_owned(),
            evidence: vec![
                format!("graph aspect ratio: {:.2}", diagnostics.aspect_ratio),
                format!(
                    "viewport aspect ratio: {:.2}",
                    diagnostics.viewport_aspect_ratio
                ),
                format!(
                    "fitted vertical fill: {:.0}%",
                    diagnostics.fitted_fill.y * 100.0
                ),
            ],
        });
    }

    if diagnostics.readability.selected_path_crossings > 0 {
        findings.push(SnapshotFinding {
            severity: if diagnostics.readability.selected_path_crossings >= 4 {
                FindingSeverity::High
            } else {
                FindingSeverity::Medium
            },
            title: "Primary lineage is crossed by other edges.".to_owned(),
            evidence: vec![format!(
                "selected path crossings: {}",
                diagnostics.readability.selected_path_crossings
            )],
        });
    }

    if diagnostics.edge_labels.collision_count > 0
        || diagnostics.edge_labels.edge_collision_count > 0
        || diagnostics.edge_labels.edge_intersection_count > 0
    {
        findings.push(SnapshotFinding {
            severity: if diagnostics.edge_labels.collision_count
                + diagnostics.edge_labels.edge_collision_count
                + diagnostics.edge_labels.edge_intersection_count
                >= 6
            {
                FindingSeverity::High
            } else {
                FindingSeverity::Medium
            },
            title: "Edge labels compete with nearby graph geometry.".to_owned(),
            evidence: vec![
                format!(
                    "label collisions: {}",
                    diagnostics.edge_labels.collision_count
                ),
                format!(
                    "edge intersections: {}",
                    diagnostics.edge_labels.edge_intersection_count
                ),
                format!(
                    "edge collisions: {}",
                    diagnostics.edge_labels.edge_collision_count
                ),
            ],
        });
    }

    if diagnostics.readability.edge_edge_crossings > 0 {
        findings.push(SnapshotFinding {
            severity: if diagnostics.readability.edge_edge_crossings >= 8 {
                FindingSeverity::High
            } else {
                FindingSeverity::Medium
            },
            title: "Edge crossings reduce graph scanability.".to_owned(),
            evidence: vec![
                format!(
                    "edge crossings: {}",
                    diagnostics.readability.edge_edge_crossings
                ),
                format!(
                    "artifact/artifact crossings: {}",
                    diagnostics
                        .readability
                        .edge_crossings_by_kind
                        .artifact_artifact
                ),
                format!(
                    "mixed crossings: {}",
                    diagnostics.readability.edge_crossings_by_kind.mixed
                ),
            ],
        });
    }

    if diagnostics.readability.long_edge_count > 0 {
        findings.push(SnapshotFinding {
            severity: if diagnostics.readability.long_edge_count >= 4 {
                FindingSeverity::Medium
            } else {
                FindingSeverity::Low
            },
            title: "Long edges make local relationships harder to follow.".to_owned(),
            evidence: vec![format!(
                "long edges: {}",
                diagnostics.readability.long_edge_count
            )],
        });
    }

    if diagnostics.readability.backtracking_edge_count > 0 {
        findings.push(SnapshotFinding {
            severity: if diagnostics.readability.backtracking_edge_count >= 3 {
                FindingSeverity::Medium
            } else {
                FindingSeverity::Low
            },
            title: "Backtracking edges weaken top-down task flow.".to_owned(),
            evidence: vec![format!(
                "backtracking edges: {}",
                diagnostics.readability.backtracking_edge_count
            )],
        });
    }

    findings.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.title.cmp(&right.title))
    });
    findings
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossingKinds {
    pub artifact_artifact: usize,
    pub candidate_candidate: usize,
    pub mixed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pair {
    pub x: f32,
    pub y: f32,
}

impl From<Vec2> for Pair {
    fn from(value: Vec2) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use eframe::egui::Vec2;
    use ploke_records::history::{
        ActorRefRecord, ArtifactRefRecord, ProcedureRefRecord, SurfaceCommitmentRecord,
        SurfaceDeltaRecord, SurfaceRecord, SurfaceRootRecord,
    };
    use ploke_records::ids::{
        ArtifactId, BlockHash, BlockId, EntryId, HistoryHash, LineageId, RecordedAt, RuntimeId,
    };
    use ploke_tree::graph::{
        ArtifactIdentity, ArtifactIds, ArtifactIndex, ArtifactKey, ArtifactNode, HistoryBlockNode,
        HistoryIndex, OpeningAuthorityNode, SuccessorNode,
    };
    use ploke_tree::{CampaignRef, Lanes, NodeKey, RunForest, TreeNode};

    use super::*;
    use crate::ui::view::{
        EdgeCrossingsByKind, EdgeLabelDiagnostics, GraphConnectivityDiagnostics,
        GraphReadabilityDiagnostics, GraphSelectionDetail, GraphSelectionRef, GraphViewMode,
        artifact_tree::{Components, Edges, Marks, Nodes, Shape},
    };

    fn diagnostics() -> GraphViewDiagnostics {
        GraphViewDiagnostics {
            mode: GraphViewMode::ArtifactTree,
            node_count: 1,
            edge_count: 0,
            connectivity: GraphConnectivityDiagnostics::default(),
            artifact_tree: Shape::default(),
            graph_size: Vec2::new(100.0, 100.0),
            viewport_size: Vec2::new(200.0, 200.0),
            aspect_ratio: 1.0,
            viewport_aspect_ratio: 1.0,
            fitted_size: Vec2::new(100.0, 100.0),
            fitted_fill: Vec2::new(0.5, 0.5),
            center_offset: Vec2::ZERO,
            edge_labels: EdgeLabelDiagnostics::default(),
            readability: GraphReadabilityDiagnostics::default(),
        }
    }

    #[test]
    fn snapshot_findings_are_ranked_by_severity() {
        let mut diagnostics = diagnostics();
        diagnostics.node_count = 4;
        diagnostics.edge_count = 3;
        diagnostics.graph_size = Vec2::new(400.0, 300.0);
        diagnostics.viewport_size = Vec2::new(800.0, 600.0);
        diagnostics.aspect_ratio = 1.33;
        diagnostics.viewport_aspect_ratio = 1.33;
        diagnostics.fitted_size = Vec2::new(600.0, 450.0);
        diagnostics.fitted_fill = Vec2::new(0.75, 0.75);
        diagnostics.edge_labels = EdgeLabelDiagnostics {
            label_count: 6,
            collision_count: 5,
            edge_intersection_count: 1,
            edge_collision_count: 0,
        };
        diagnostics.readability = GraphReadabilityDiagnostics {
            edge_edge_crossings: 3,
            edge_crossings_by_kind: EdgeCrossingsByKind {
                artifact_artifact: 1,
                candidate_candidate: 0,
                mixed: 2,
            },
            long_edge_count: 1,
            backtracking_edge_count: 0,
            selected_path_crossings: 5,
        };
        let snapshot = Snapshot::from_diagnostics(1, diagnostics);

        let severities: Vec<_> = snapshot
            .findings
            .iter()
            .map(|finding| finding.severity)
            .collect();
        assert_eq!(
            severities,
            vec![
                FindingSeverity::High,
                FindingSeverity::High,
                FindingSeverity::Medium,
                FindingSeverity::Low
            ]
        );
        assert_eq!(
            snapshot.findings[0].title,
            "Edge labels compete with nearby graph geometry."
        );
        assert_eq!(snapshot.candidate_clutter_crossings, 0);
    }

    #[test]
    fn clean_snapshot_has_no_findings() {
        let snapshot = Snapshot::from_diagnostics(1, diagnostics());

        assert!(snapshot.findings.is_empty());
    }

    #[test]
    fn selected_graph_item_snapshot_serializes_reference_and_inspector() {
        let graph = ploke_tree::Graph::default();
        let selection = GraphSelectionDetail {
            kind: "artifact".to_owned(),
            label: "missing".to_owned(),
            detail: "central graph payload".to_owned(),
            reference: GraphSelectionRef::Artifact {
                key: "artifact:missing".to_owned(),
            },
        };

        let snapshot = SelectedGraphItemSnapshot::from_graph(&graph, &selection);
        let json = serde_json::to_value(&snapshot).expect("serialize selected graph item");

        assert_eq!(json["schema_version"], SELECTED_GRAPH_ITEM_VERSION);
        assert_eq!(json["selected"]["detail"], "central graph payload");
        assert_eq!(
            json["selected"]["reference"]["Artifact"]["key"],
            "artifact:missing"
        );
        assert_eq!(
            json["selected_inspector"]["unavailable"],
            "artifact_not_found"
        );
    }

    #[test]
    fn artifact_component_breakdown_uses_artifact_tree_even_when_forest_is_present() {
        let graph = ploke_tree::Graph {
            forest: Some(RunForest {
                campaign: CampaignRef {
                    campaign_id: "campaign".to_owned(),
                    updated_at: "now".to_owned(),
                },
                roots: vec![NodeKey::from("root")],
                nodes: vec![tree_node("root")],
                lanes: Lanes {
                    frontier: Vec::new(),
                    completed: Vec::new(),
                    failed: Vec::new(),
                },
                passive_evidence: Default::default(),
                diagnostics: Vec::new(),
            }),
            artifacts: ArtifactIndex {
                artifacts: BTreeMap::from([
                    artifact_history("artifact:base"),
                    artifact_history("artifact:successor"),
                ]),
            },
            history: HistoryIndex {
                blocks: BTreeMap::from([(
                    BlockHash("block-hash".to_owned()),
                    history_block("artifact:base", "artifact:successor"),
                )]),
                ..HistoryIndex::default()
            },
            ..ploke_tree::Graph::default()
        };

        let components = artifact_component_breakdown(&graph);

        assert_eq!(components.len(), 1);
        assert_eq!(components[0].p_h.len(), 1);
        assert_eq!(components[0].p_o.len(), 1);
    }

    #[test]
    fn thin_horizontal_composition_is_high_severity() {
        let mut diagnostics = diagnostics();
        diagnostics.node_count = 31;
        diagnostics.edge_count = 30;
        diagnostics.graph_size = Vec2::new(7790.0, 282.0);
        diagnostics.viewport_size = Vec2::new(674.0, 584.0);
        diagnostics.aspect_ratio = 27.6;
        diagnostics.viewport_aspect_ratio = 1.15;
        diagnostics.fitted_size = Vec2::new(552.0, 20.0);
        diagnostics.fitted_fill = Vec2::new(0.82, 0.034);

        let snapshot = Snapshot::from_diagnostics(1, diagnostics);

        assert_eq!(snapshot.findings[0].severity, FindingSeverity::High);
        assert_eq!(
            snapshot.findings[0].title,
            "Graph composition collapses into a thin horizontal strip."
        );
    }

    #[test]
    fn default_view_contract_reports_current_layout_shell() {
        let mut diagnostics = diagnostics();
        diagnostics.node_count = 3;
        diagnostics.edge_count = 2;
        diagnostics.artifact_tree = Shape::new(
            Nodes::new(3),
            Edges::new(1, 1, 1, 1),
            Components::new(2, 2, 1),
            Marks::new(1, 1, 1),
        );
        diagnostics.graph_size = Vec2::new(300.0, 200.0);
        diagnostics.viewport_size = Vec2::new(600.0, 400.0);
        diagnostics.aspect_ratio = 1.5;
        diagnostics.viewport_aspect_ratio = 1.5;
        diagnostics.fitted_size = Vec2::new(300.0, 200.0);

        let snapshot = Snapshot::from_diagnostics(1, diagnostics);

        let report = &snapshot.default_view_contract;
        assert!(report.layout.left_sidebar_present);
        assert!(report.layout.center_canvas_present);
        assert!(report.layout.top_strip_present);
        assert!(report.layout.right_inspector_present);
        assert!(report.layout.bottom_timeline_present);
        assert_eq!(
            report.layout.width_budget.default_window_width_logical_px,
            1280
        );
        assert_eq!(
            report.layout.width_budget.left_sidebar_width_logical_px,
            200
        );
        assert_eq!(
            report.layout.width_budget.left_sidebar_max_width_logical_px,
            240
        );
        assert_eq!(
            report.layout.width_budget.right_inspector_width_logical_px,
            300
        );
        assert_eq!(
            report
                .layout
                .width_budget
                .right_inspector_max_width_logical_px,
            360
        );
        assert_eq!(
            report.layout.width_budget.center_canvas_width_logical_px,
            780
        );
        assert_eq!(
            report
                .layout
                .width_budget
                .min_center_canvas_width_logical_px,
            680
        );
        assert_eq!(report.layout.width_budget.center_canvas_width_percent, 61);
        assert_eq!(
            report.layout.width_budget.min_center_canvas_width_percent,
            50
        );
        assert!(report.layout.width_budget.center_canvas_satisfies_minimum());
        assert_eq!(report.center.nodes.a, 3);
        assert_eq!(report.center.edges.p_h, 1);
        assert_eq!(report.center.edges.p_o, 1);
        assert_eq!(report.center.edges.p_b, 1);
        assert_eq!(report.center.edges.total(), 4);
        assert_eq!(report.center.edges.visible_primary_total(), 2);
        assert_eq!(report.center.components.weak, 2);
        assert_eq!(report.center.components.roots, 2);
        assert_eq!(report.center.components.orphan_artifacts, 1);
        assert!(!report.center.components.weakly_connected);
        assert_eq!(report.center.marks.ruler_highlights, 1);
        assert_eq!(report.center.marks.dimmed_children, 1);
        assert_eq!(report.center.marks.dotted_child_edges, 1);
        assert!(report.controls.quick_filters_present);
        assert!(!report.controls.hide_unconsidered_children);

        let statuses = report
            .checks
            .iter()
            .map(|check| (check.id.as_str(), check.status))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            statuses["default-mode-artifact-tree"],
            ContractCheckStatus::Passed
        );
        assert_eq!(
            statuses["synthetic-anchors-hidden"],
            ContractCheckStatus::Passed
        );
        assert_eq!(
            statuses["center-node-set-reported"],
            ContractCheckStatus::Passed
        );
        assert_eq!(
            statuses["center-edge-sets-reported"],
            ContractCheckStatus::Passed
        );
        assert_eq!(
            statuses["artifact-components-reported"],
            ContractCheckStatus::Passed
        );
        assert_eq!(
            statuses["ruler-highlight-count-reported"],
            ContractCheckStatus::Passed
        );
        assert_eq!(
            statuses["center-canvas-width-budget"],
            ContractCheckStatus::Passed
        );
        assert_eq!(statuses["top-strip-present"], ContractCheckStatus::Passed);
        assert_eq!(
            statuses["right-inspector-present"],
            ContractCheckStatus::Passed
        );
        assert_eq!(
            statuses["bottom-timeline-present"],
            ContractCheckStatus::Passed
        );
    }

    #[test]
    fn default_view_contract_text_names_failed_checks() {
        let mut diagnostics = diagnostics();
        diagnostics.mode = GraphViewMode::Lineage;
        diagnostics.node_count = 0;
        diagnostics.connectivity.synthetic_anchors_visible = true;
        diagnostics.viewport_size = Vec2::new(100.0, 100.0);
        diagnostics.fitted_fill = Vec2::new(1.0, 1.0);

        let snapshot = Snapshot::from_observation(
            1,
            SnapshotObservation::new(diagnostics).with_graph_has_content(true),
        );

        let text = snapshot.render_text();
        assert!(text.contains("default-view contract:"));
        assert!(text.contains(
            "layout widths: default_window=1280px, left_sidebar=200px, left_sidebar_max=240px, right_inspector=300px, right_inspector_max=360px, center_canvas=780px"
        ));
        assert!(text.contains("Failed: default-mode-artifact-tree"));
        assert!(text.contains("Failed: non-empty-artifact-run-renders-nodes"));
        assert!(text.contains("Failed: synthetic-anchors-hidden"));
        assert!(text.contains("Passed: right-inspector-present"));
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

    fn artifact_history(value: &str) -> (ArtifactKey, ArtifactNode) {
        let artifact = ArtifactRefRecord::from_artifact_id(ArtifactId(value.to_owned()));
        let key = ArtifactKey::HistoryRef {
            id: artifact.id().0.clone(),
        };
        (
            key.clone(),
            ArtifactNode {
                key,
                identity: ArtifactIdentity::HistoryRef(artifact),
                ids: ArtifactIds {
                    artifact_refs: vec![ArtifactRefRecord::from_artifact_id(ArtifactId(
                        value.to_owned(),
                    ))],
                    ..ArtifactIds::default()
                },
                evidence: Vec::new(),
            },
        )
    }

    fn history_block(active: &str, successor: &str) -> HistoryBlockNode {
        HistoryBlockNode {
            block_hash: BlockHash("block-hash".to_owned()),
            block_id: BlockId("block-id".to_owned()),
            lineage_id: LineageId("lineage".to_owned()),
            block_height: 0,
            parent_block_hashes: Vec::new(),
            opened_from_artifact: ArtifactRefRecord::from_artifact_id(ArtifactId(
                active.to_owned(),
            )),
            active_artifact: ArtifactRefRecord::from_artifact_id(ArtifactId(active.to_owned())),
            selected_successor: SuccessorNode {
                runtime: ActorRefRecord::Runtime(RuntimeId("runtime".to_owned())),
                artifact: ArtifactRefRecord::from_artifact_id(ArtifactId(successor.to_owned())),
            },
            opening_authority: OpeningAuthorityNode::Predecessor {
                predecessor_block_hash: BlockHash("previous".to_owned()),
            },
            ruling_authority: ActorRefRecord::Runtime(RuntimeId("runtime".to_owned())),
            policy_ref: ProcedureRefRecord {
                value: "policy".to_owned(),
            },
            surface: surface(),
            opened_at: RecordedAt(1),
            sealed_at: RecordedAt(2),
            entry_count: 0,
            entries: Vec::<EntryId>::new(),
        }
    }

    fn surface() -> SurfaceCommitmentRecord {
        SurfaceCommitmentRecord {
            immutable: surface_record("immutable"),
            mutated: SurfaceDeltaRecord {
                before: surface_record("mutated-before"),
                after: surface_record("mutated-after"),
            },
            ambient: SurfaceDeltaRecord {
                before: surface_record("ambient-before"),
                after: surface_record("ambient-after"),
            },
        }
    }

    fn surface_record(value: &str) -> SurfaceRecord {
        SurfaceRecord {
            root: SurfaceRootRecord {
                hash: HistoryHash(value.to_owned()),
            },
        }
    }
}
