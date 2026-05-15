//! Native diagnostic snapshots emitted by the running operator UI.
//!
//! This module persists render-only observations made by the live egui view.
//! It does not recompute layout or interpret run records.

mod default_view;

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use eframe::egui::Vec2;
use serde::{Deserialize, Serialize};

use crate::ui::inspector::SelectionInspectorSnapshot;
use crate::ui::view::{GraphSelectionDetail, GraphViewDiagnostics};
pub use default_view::{
    CheckStatus as ContractCheckStatus, ComponentBreakdown, Layout as DefaultViewLayout,
    Report as DefaultViewContractReport, WidthBudget as DefaultViewWidthBudget,
};

const SNAPSHOT_VERSION: &str = "ploke-egui.graph-diagnostics.v1";
const DEFAULT_MAX_SNAPSHOTS: u64 = 10;

#[derive(Debug)]
pub struct SnapshotSink {
    root: PathBuf,
    max_snapshots: u64,
    sequence: u64,
    last: Option<SnapshotObservation>,
}

impl SnapshotSink {
    pub fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("snapshots"))?;
        Ok(Self {
            root,
            max_snapshots: DEFAULT_MAX_SNAPSHOTS,
            sequence: 0,
            last: None,
        })
    }

    pub fn observe(&mut self, observation: SnapshotObservation) -> io::Result<bool> {
        if self.last.as_ref() == Some(&observation) {
            return Ok(false);
        }

        self.sequence = self.sequence.saturating_add(1);
        self.last = Some(observation.clone());

        let snapshot = Snapshot::from_observation(self.sequence, observation);
        self.write_snapshot(&snapshot)?;
        Ok(true)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn write_snapshot(&self, snapshot: &Snapshot) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(snapshot).map_err(io::Error::other)?;
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

pub fn write_manual_snapshot(observation: SnapshotObservation) -> io::Result<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/manual-diagnostics");
    fs::create_dir_all(&root)?;

    let snapshot = Snapshot::from_observation(1, observation);
    let bytes = serde_json::to_vec_pretty(&snapshot).map_err(io::Error::other)?;
    fs::write(root.join("latest.json"), bytes)?;
    fs::write(root.join("latest.txt"), snapshot.render_text())?;
    Ok(root.join("latest.txt"))
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotObservation {
    pub diagnostics: GraphViewDiagnostics,
    pub artifact_components: Vec<ComponentBreakdown>,
    pub graph_has_content: bool,
    pub run_error: Option<String>,
    pub run: Option<RunSnapshot>,
    pub selected: Option<SelectionSnapshot>,
    pub selected_inspector: Option<SelectionInspectorSnapshot>,
}

impl SnapshotObservation {
    pub fn new(diagnostics: GraphViewDiagnostics) -> Self {
        Self {
            graph_has_content: diagnostics.node_count > 0,
            diagnostics,
            artifact_components: Vec::new(),
            run_error: None,
            run: None,
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

    pub fn with_run(mut self, run: Option<RunSnapshot>) -> Self {
        self.run = run;
        self
    }

    pub fn with_selected(mut self, selected: Option<GraphSelectionDetail>) -> Self {
        self.selected = selected.map(SelectionSnapshot::from);
        self
    }

    pub(crate) fn with_selected_inspector(
        mut self,
        inspector: Option<SelectionInspectorSnapshot>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSnapshot {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionSnapshot {
    pub kind: String,
    pub label: String,
    pub detail: String,
}

impl From<GraphSelectionDetail> for SelectionSnapshot {
    fn from(value: GraphSelectionDetail) -> Self {
        Self {
            kind: value.kind,
            label: value.label,
            detail: value.detail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
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
    pub default_view_contract: DefaultViewContractReport,
    pub findings: Vec<SnapshotFinding>,
}

impl Snapshot {
    pub fn from_observation(sequence: u64, observation: SnapshotObservation) -> Self {
        let SnapshotObservation {
            diagnostics,
            artifact_components,
            graph_has_content,
            run_error,
            run,
            selected,
            selected_inspector,
        } = observation;
        let default_view_contract = DefaultViewContractReport::from_parts(
            &diagnostics,
            graph_has_content,
            run_error,
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
        default_view_contract: DefaultViewContractReport,
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
    use eframe::egui::Vec2;

    use super::*;
    use crate::ui::view::{
        EdgeCrossingsByKind, EdgeLabelDiagnostics, GraphConnectivityDiagnostics,
        GraphReadabilityDiagnostics, GraphViewMode,
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
            Edges::new(1, 1),
            Components::new(2, 2, 1),
            Marks::new(1),
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
        assert_eq!(report.center.edges.p_b, 1);
        assert_eq!(report.center.edges.total(), 2);
        assert_eq!(report.center.components.weak, 2);
        assert_eq!(report.center.components.roots, 2);
        assert_eq!(report.center.components.orphan_artifacts, 1);
        assert!(!report.center.components.weakly_connected);
        assert_eq!(report.center.marks.ruler_highlights, 1);

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
}
