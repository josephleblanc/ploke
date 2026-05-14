use std::fmt::Write as _;
use std::path::PathBuf;

use eframe::egui::Vec2;

use crate::import::graph_from_run_root;
use crate::ui::app::layout;
use crate::ui::view::{GraphView, GraphViewMode};

use super::RunPicker;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactConnectivityBatch {
    pub root: PathBuf,
    pub total_options: usize,
    pub artifact_options: usize,
    pub weakly_connected_options: usize,
    pub disconnected_options: usize,
    pub zero_artifact_options: usize,
    pub unreadable_options: usize,
    pub entries: Vec<ArtifactConnectivityEntry>,
    pub errors: Vec<ArtifactConnectivityError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactConnectivityEntry {
    pub index: usize,
    pub name: String,
    pub path: PathBuf,
    pub artifacts: usize,
    pub history_patches: usize,
    pub applied_patch_edges: usize,
    pub weak_components: usize,
    pub roots: usize,
    pub orphan_artifacts: usize,
    pub weakly_connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactConnectivityError {
    pub index: usize,
    pub name: String,
    pub path: PathBuf,
    pub message: String,
}

impl RunPicker {
    pub fn artifact_connectivity_batch(&self) -> ArtifactConnectivityBatch {
        let mut entries = Vec::new();
        let mut errors = Vec::new();
        let mut zero_artifact_options = 0;

        for (index, entry) in self.entries.iter().enumerate() {
            match graph_from_run_root(&entry.path) {
                Ok(graph) => {
                    if graph.artifacts.artifacts.is_empty() {
                        zero_artifact_options += 1;
                        continue;
                    }

                    let Some(diagnostics) = GraphView::contract_diagnostics(
                        &graph,
                        GraphViewMode::ArtifactTree,
                        Vec2::new(
                            layout::DEFAULT_CENTER_CANVAS_WIDTH,
                            layout::DEFAULT_WINDOW_HEIGHT,
                        ),
                    ) else {
                        errors.push(ArtifactConnectivityError {
                            index,
                            name: entry.name.clone(),
                            path: entry.path.clone(),
                            message: "artifact-tree diagnostics unavailable".to_owned(),
                        });
                        continue;
                    };

                    let shape = diagnostics.artifact_tree;
                    let nodes = shape.nodes();
                    if nodes.artifacts == 0 {
                        zero_artifact_options += 1;
                        continue;
                    }

                    let edges = shape.edges();
                    let components = shape.components();
                    entries.push(ArtifactConnectivityEntry {
                        index,
                        name: entry.name.clone(),
                        path: entry.path.clone(),
                        artifacts: nodes.artifacts,
                        history_patches: edges.history_patches,
                        applied_patch_edges: edges.applied_patch_edges,
                        weak_components: components.weak,
                        roots: components.roots,
                        orphan_artifacts: components.orphan_artifacts,
                        weakly_connected: components.weakly_connected(nodes.artifacts),
                    });
                }
                Err(error) => errors.push(ArtifactConnectivityError {
                    index,
                    name: entry.name.clone(),
                    path: entry.path.clone(),
                    message: error.to_string(),
                }),
            }
        }

        let artifact_options = entries.len();
        let weakly_connected_options = entries
            .iter()
            .filter(|entry| entry.weakly_connected)
            .count();
        let disconnected_options = artifact_options.saturating_sub(weakly_connected_options);

        ArtifactConnectivityBatch {
            root: self.root.clone(),
            total_options: self.entries.len(),
            artifact_options,
            weakly_connected_options,
            disconnected_options,
            zero_artifact_options,
            unreadable_options: errors.len(),
            entries,
            errors,
        }
    }
}

impl ArtifactConnectivityBatch {
    pub fn render_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "artifact connectivity batch:");
        let _ = writeln!(out, "root: {}", self.root.display());
        let _ = writeln!(
            out,
            "options: total={}, artifact_options={}, weakly_connected={}, disconnected={}, zero_artifact={}, unreadable={}",
            self.total_options,
            self.artifact_options,
            self.weakly_connected_options,
            self.disconnected_options,
            self.zero_artifact_options,
            self.unreadable_options
        );
        let _ = writeln!(
            out,
            "answer: {}/{} artifact options are weakly connected",
            self.weakly_connected_options, self.artifact_options
        );

        let _ = writeln!(out, "artifact options:");
        for entry in &self.entries {
            let _ = writeln!(
                out,
                "- [{}] {}: A={}, P_H={}, P_B={}, weak={}, roots={}, orphan_artifacts={}, weakly_connected={}",
                entry.index,
                entry.name,
                entry.artifacts,
                entry.history_patches,
                entry.applied_patch_edges,
                entry.weak_components,
                entry.roots,
                entry.orphan_artifacts,
                entry.weakly_connected
            );
        }

        if !self.errors.is_empty() {
            let _ = writeln!(out, "unreadable options:");
            for error in &self.errors {
                let _ = writeln!(out, "- [{}] {}: {}", error.index, error.name, error.message);
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_counts_artifact_options_by_weak_connectivity() {
        let batch = ArtifactConnectivityBatch {
            root: PathBuf::from("/tmp/runs"),
            total_options: 4,
            artifact_options: 2,
            weakly_connected_options: 1,
            disconnected_options: 1,
            zero_artifact_options: 1,
            unreadable_options: 1,
            entries: vec![entry(0, true), entry(1, false)],
            errors: vec![ArtifactConnectivityError {
                index: 3,
                name: "unreadable-run".to_owned(),
                path: PathBuf::from("/tmp/runs/unreadable-run/prototype1"),
                message: "parse error".to_owned(),
            }],
        };

        let rendered = batch.render_text();

        assert!(rendered.contains("answer: 1/2 artifact options are weakly connected"));
        assert!(rendered.contains("disconnected=1"));
        assert!(rendered.contains("unreadable=1"));
    }

    fn entry(index: usize, weakly_connected: bool) -> ArtifactConnectivityEntry {
        ArtifactConnectivityEntry {
            index,
            name: format!("run-{index}"),
            path: PathBuf::from(format!("/tmp/runs/run-{index}/prototype1")),
            artifacts: 3,
            history_patches: 1,
            applied_patch_edges: 1,
            weak_components: if weakly_connected { 1 } else { 2 },
            roots: 1,
            orphan_artifacts: 0,
            weakly_connected,
        }
    }
}
