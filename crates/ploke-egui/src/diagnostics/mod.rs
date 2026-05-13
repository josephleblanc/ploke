//! Native diagnostic snapshots emitted by the running operator UI.
//!
//! This module persists render-only observations made by the live egui view.
//! It does not recompute layout or interpret run records.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use eframe::egui::Vec2;
use serde::{Deserialize, Serialize};

use crate::ui::view::{GraphViewDiagnostics, GraphViewMode};

const SNAPSHOT_VERSION: &str = "ploke-egui.graph-diagnostics.v1";
const DEFAULT_MAX_SNAPSHOTS: u64 = 10;

#[derive(Debug)]
pub struct SnapshotSink {
    root: PathBuf,
    max_snapshots: u64,
    sequence: u64,
    last: Option<GraphViewDiagnostics>,
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

    pub fn observe(&mut self, diagnostics: GraphViewDiagnostics) -> io::Result<bool> {
        if self.last.as_ref() == Some(&diagnostics) {
            return Ok(false);
        }

        self.sequence = self.sequence.saturating_add(1);
        self.last = Some(diagnostics.clone());

        let snapshot = Snapshot::from_diagnostics(self.sequence, diagnostics);
        self.write_snapshot(&snapshot)?;
        Ok(true)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn write_snapshot(&self, snapshot: &Snapshot) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(snapshot).map_err(io::Error::other)?;
        fs::write(self.root.join("latest.json"), &bytes)?;
        let slot = ((snapshot.sequence - 1) % self.max_snapshots) + 1;
        fs::write(
            self.root.join("snapshots").join(format!("{slot:02}.json")),
            bytes,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: String,
    pub sequence: u64,
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
    pub findings: Vec<SnapshotFinding>,
}

impl Snapshot {
    fn from_diagnostics(sequence: u64, diagnostics: GraphViewDiagnostics) -> Self {
        let findings = ranked_findings(&diagnostics);

        Self {
            schema_version: SNAPSHOT_VERSION.to_owned(),
            sequence,
            view_mode: diagnostics.mode.as_str().to_owned(),
            node_count: diagnostics.node_count,
            edge_count: diagnostics.edge_count,
            component_count_before_anchoring: diagnostics
                .connectivity
                .component_count_before_anchoring,
            component_roots_before_anchoring: if diagnostics.mode == GraphViewMode::FullDebug {
                diagnostics
                    .connectivity
                    .component_roots_before_anchoring
                    .iter()
                    .map(|root| format!("{} {}", root.kind, root.label))
                    .collect()
            } else {
                Vec::new()
            },
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
            findings,
        }
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
    };

    #[test]
    fn snapshot_findings_are_ranked_by_severity() {
        let snapshot = Snapshot::from_diagnostics(
            1,
            GraphViewDiagnostics {
                mode: GraphViewMode::ArtifactTree,
                node_count: 4,
                edge_count: 3,
                connectivity: GraphConnectivityDiagnostics::default(),
                graph_size: Vec2::new(400.0, 300.0),
                viewport_size: Vec2::new(800.0, 600.0),
                aspect_ratio: 1.33,
                viewport_aspect_ratio: 1.33,
                fitted_size: Vec2::new(600.0, 450.0),
                fitted_fill: Vec2::new(0.75, 0.75),
                center_offset: Vec2::ZERO,
                edge_labels: EdgeLabelDiagnostics {
                    label_count: 6,
                    collision_count: 5,
                    edge_intersection_count: 1,
                    edge_collision_count: 0,
                },
                readability: GraphReadabilityDiagnostics {
                    edge_edge_crossings: 3,
                    edge_crossings_by_kind: EdgeCrossingsByKind {
                        artifact_artifact: 1,
                        candidate_candidate: 0,
                        mixed: 2,
                    },
                    long_edge_count: 1,
                    backtracking_edge_count: 0,
                    selected_path_crossings: 5,
                },
            },
        );

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
        let snapshot = Snapshot::from_diagnostics(
            1,
            GraphViewDiagnostics {
                mode: GraphViewMode::ArtifactTree,
                node_count: 1,
                edge_count: 0,
                connectivity: GraphConnectivityDiagnostics::default(),
                graph_size: Vec2::new(100.0, 100.0),
                viewport_size: Vec2::new(200.0, 200.0),
                aspect_ratio: 1.0,
                viewport_aspect_ratio: 1.0,
                fitted_size: Vec2::new(100.0, 100.0),
                fitted_fill: Vec2::new(0.5, 0.5),
                center_offset: Vec2::ZERO,
                edge_labels: EdgeLabelDiagnostics::default(),
                readability: GraphReadabilityDiagnostics::default(),
            },
        );

        assert!(snapshot.findings.is_empty());
    }

    #[test]
    fn thin_horizontal_composition_is_high_severity() {
        let snapshot = Snapshot::from_diagnostics(
            1,
            GraphViewDiagnostics {
                mode: GraphViewMode::ArtifactTree,
                node_count: 31,
                edge_count: 30,
                connectivity: GraphConnectivityDiagnostics::default(),
                graph_size: Vec2::new(7790.0, 282.0),
                viewport_size: Vec2::new(674.0, 584.0),
                aspect_ratio: 27.6,
                viewport_aspect_ratio: 1.15,
                fitted_size: Vec2::new(552.0, 20.0),
                fitted_fill: Vec2::new(0.82, 0.034),
                center_offset: Vec2::ZERO,
                edge_labels: EdgeLabelDiagnostics::default(),
                readability: GraphReadabilityDiagnostics::default(),
            },
        );

        assert_eq!(snapshot.findings[0].severity, FindingSeverity::High);
        assert_eq!(
            snapshot.findings[0].title,
            "Graph composition collapses into a thin horizontal strip."
        );
    }
}
