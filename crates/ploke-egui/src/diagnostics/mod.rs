//! Native diagnostic snapshots emitted by the running operator UI.
//!
//! This module persists observations made by the live egui view. It does not
//! recompute layout or interpret run records.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use eframe::egui::Vec2;
use serde::{Deserialize, Serialize};

use crate::ui::view::GraphViewDiagnostics;

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
        if self.last == Some(diagnostics) {
            return Ok(false);
        }

        self.sequence = self.sequence.saturating_add(1);
        self.last = Some(diagnostics);

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
    pub node_count: usize,
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
    pub long_edge_count: usize,
    pub backtracking_edge_count: usize,
    pub selected_path_crossings: usize,
}

impl Snapshot {
    fn from_diagnostics(sequence: u64, diagnostics: GraphViewDiagnostics) -> Self {
        Self {
            schema_version: SNAPSHOT_VERSION.to_owned(),
            sequence,
            node_count: diagnostics.node_count,
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
                candidate_candidate: diagnostics
                    .readability
                    .edge_crossings_by_kind
                    .candidate_candidate,
                candidate_history: diagnostics
                    .readability
                    .edge_crossings_by_kind
                    .candidate_history,
                history_history: diagnostics
                    .readability
                    .edge_crossings_by_kind
                    .history_history,
            },
            long_edge_count: diagnostics.readability.long_edge_count,
            backtracking_edge_count: diagnostics.readability.backtracking_edge_count,
            selected_path_crossings: diagnostics.readability.selected_path_crossings,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossingKinds {
    pub candidate_candidate: usize,
    pub candidate_history: usize,
    pub history_history: usize,
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
