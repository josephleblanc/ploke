//! Typed performance snapshots for `ploke-egui` projection baselines.
//!
//! These logs are deliberately coarse and persisted through named records.
//! Interactive profiler backends receive finer spans through `profiling::scope!`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use eframe::egui::Vec2;
use ploke_tree::Graph;
use serde::{Deserialize, Serialize};

use crate::diagnostics::Pair;
use crate::import::graph_from_run_root;
use crate::ui::view::{GraphView, GraphViewDiagnostics, GraphViewMode};

const PERFORMANCE_LOG_VERSION: &str = "ploke-egui.performance-log.v1";
const DEFAULT_MAX_LOGS: u64 = 5;

#[derive(Debug, Clone)]
pub struct PerformanceRun {
    pub run_root: PathBuf,
    pub mode: GraphViewMode,
    pub viewport_size: Vec2,
}

impl PerformanceRun {
    pub fn new(run_root: PathBuf, mode: GraphViewMode, viewport_size: Vec2) -> Self {
        Self {
            run_root,
            mode,
            viewport_size,
        }
    }
}

#[derive(Debug)]
pub struct PerformanceLogSink {
    root: PathBuf,
    max_logs: u64,
}

impl PerformanceLogSink {
    pub fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("runs"))?;
        Ok(Self {
            root,
            max_logs: DEFAULT_MAX_LOGS,
        })
    }

    pub fn observe(&self, run: PerformanceRun) -> io::Result<PathBuf> {
        profiling::scope!("ploke-egui.perf-log.observe");
        let latest_sequence = self.latest_sequence();
        let sequence = latest_sequence.saturating_add(1);
        let log = measure_performance(sequence, run)?;
        self.write_log(&log)?;
        Ok(self.root.join("latest.txt"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn latest_sequence(&self) -> u64 {
        let Ok(bytes) = fs::read(self.root.join("latest.json")) else {
            return 0;
        };
        serde_json::from_slice::<PerformanceLog>(&bytes)
            .map(|log| log.sequence)
            .unwrap_or(0)
    }

    fn write_log(&self, log: &PerformanceLog) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(log).map_err(io::Error::other)?;
        fs::write(self.root.join("latest.json"), &bytes)?;
        fs::write(self.root.join("latest.txt"), log.render_text())?;

        let slot = ((log.sequence - 1) % self.max_logs) + 1;
        fs::write(
            self.root.join("runs").join(format!("{slot:02}.json")),
            bytes,
        )?;
        fs::write(
            self.root.join("runs").join(format!("{slot:02}.txt")),
            log.render_text(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceLog {
    pub version: String,
    pub sequence: u64,
    pub created_at_unix_ms: u64,
    pub source: PerformanceSource,
    pub graph: PerformanceGraphFacts,
    pub measurements: Vec<PerformanceMeasurement>,
}

impl PerformanceLog {
    pub fn render_text(&self) -> String {
        let mut text = String::new();
        text.push_str("ploke-egui performance log\n");
        text.push_str(&format!("version: {}\n", self.version));
        text.push_str(&format!("sequence: {}\n", self.sequence));
        text.push_str(&format!(
            "created_at_unix_ms: {}\n",
            self.created_at_unix_ms
        ));
        text.push_str(&format!("source_kind: {}\n", self.source.kind));
        if let Some(run_root) = &self.source.run_root {
            text.push_str(&format!("run_root: {run_root}\n"));
        }
        text.push_str(&format!("mode: {}\n", self.graph.mode));
        text.push_str(&format!("graph_nodes: {}\n", self.graph.visible_node_count));
        text.push_str(&format!("graph_edges: {}\n", self.graph.visible_edge_count));
        text.push_str(&format!(
            "graph_size: {:.0} x {:.0}\n",
            self.graph.graph_size.x, self.graph.graph_size.y
        ));
        text.push_str("measurements_ns:\n");
        for measurement in &self.measurements {
            text.push_str(&format!(
                "- {}: {}\n",
                measurement.name, measurement.duration_ns
            ));
        }
        text
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceSource {
    pub kind: String,
    pub run_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceGraphFacts {
    pub mode: String,
    pub visible_node_count: usize,
    pub visible_edge_count: usize,
    pub component_count_before_anchoring: usize,
    pub hidden_edge_count: usize,
    pub edge_label_count: usize,
    pub edge_edge_crossings: usize,
    pub graph_size: Pair,
    pub viewport_size: Pair,
    pub fitted_fill: Pair,
}

impl PerformanceGraphFacts {
    fn from_diagnostics(diagnostics: &GraphViewDiagnostics) -> Self {
        Self {
            mode: diagnostics.mode.as_str().to_owned(),
            visible_node_count: diagnostics.node_count,
            visible_edge_count: diagnostics.edge_count,
            component_count_before_anchoring: diagnostics
                .connectivity
                .component_count_before_anchoring,
            hidden_edge_count: diagnostics.connectivity.hidden_edge_count,
            edge_label_count: diagnostics.edge_labels.label_count,
            edge_edge_crossings: diagnostics.readability.edge_edge_crossings,
            graph_size: Pair::from(diagnostics.graph_size),
            viewport_size: Pair::from(diagnostics.viewport_size),
            fitted_fill: Pair::from(diagnostics.fitted_fill),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceMeasurement {
    pub name: String,
    pub duration_ns: u128,
}

#[derive(Debug)]
struct MeasurementRecorder {
    measurements: Vec<PerformanceMeasurement>,
}

impl MeasurementRecorder {
    fn new() -> Self {
        Self {
            measurements: Vec::new(),
        }
    }

    fn measure<T, E>(
        &mut self,
        name: &'static str,
        f: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        profiling::scope!("ploke-egui.perf-log.measure", name);
        let start = Instant::now();
        let result = f();
        self.measurements.push(PerformanceMeasurement {
            name: name.to_owned(),
            duration_ns: start.elapsed().as_nanos(),
        });
        result
    }

    fn finish(self) -> Vec<PerformanceMeasurement> {
        self.measurements
    }
}

fn measure_performance(sequence: u64, run: PerformanceRun) -> io::Result<PerformanceLog> {
    let created_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    let mut recorder = MeasurementRecorder::new();

    let (source, graph) = recorder.measure("load_graph", || load_graph(&run.run_root))?;
    let diagnostics = recorder.measure("contract_diagnostics", || {
        GraphView::contract_diagnostics(&graph, run.mode, run.viewport_size)
            .ok_or_else(|| io::Error::other("graph contract diagnostics unavailable"))
    })?;
    let graph_facts = PerformanceGraphFacts::from_diagnostics(&diagnostics);

    Ok(PerformanceLog {
        version: PERFORMANCE_LOG_VERSION.to_owned(),
        sequence,
        created_at_unix_ms,
        source,
        graph: graph_facts,
        measurements: recorder.finish(),
    })
}

fn load_graph(run_root: &Path) -> io::Result<(PerformanceSource, Graph)> {
    profiling::scope!("ploke-egui.perf-log.load-graph");
    let graph =
        graph_from_run_root(run_root).map_err(|error| io::Error::other(error.to_string()))?;
    Ok((
        PerformanceSource {
            kind: "run-root".to_owned(),
            run_root: Some(run_root.display().to_string()),
        },
        graph,
    ))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn performance_log_sink_rotates_five_slots() {
        let root = unique_temp_dir("ploke-egui-perf-log-rotation");
        let sink = PerformanceLogSink::new(&root).expect("sink");

        for sequence in 1..=6 {
            sink.write_log(&fixture_log(sequence)).expect("write log");
        }

        let latest = fs::read(root.join("latest.json")).expect("latest");
        let latest: PerformanceLog = serde_json::from_slice(&latest).expect("typed latest");
        assert_eq!(latest.sequence, 6);
        assert!(root.join("runs/01.json").exists());
        assert!(root.join("runs/02.json").exists());
        assert!(root.join("runs/03.json").exists());
        assert!(root.join("runs/04.json").exists());
        assert!(root.join("runs/05.json").exists());

        let slot_one = fs::read(root.join("runs/01.json")).expect("slot one");
        let slot_one: PerformanceLog = serde_json::from_slice(&slot_one).expect("typed slot");
        assert_eq!(slot_one.sequence, 6);
        fs::remove_dir_all(root).expect("cleanup");
    }

    fn fixture_log(sequence: u64) -> PerformanceLog {
        PerformanceLog {
            version: PERFORMANCE_LOG_VERSION.to_owned(),
            sequence,
            created_at_unix_ms: sequence,
            source: PerformanceSource {
                kind: "fixture".to_owned(),
                run_root: None,
            },
            graph: PerformanceGraphFacts {
                mode: GraphViewMode::ArtifactTree.as_str().to_owned(),
                visible_node_count: 1,
                visible_edge_count: 0,
                component_count_before_anchoring: 1,
                hidden_edge_count: 0,
                edge_label_count: 0,
                edge_edge_crossings: 0,
                graph_size: Pair { x: 1.0, y: 1.0 },
                viewport_size: Pair { x: 800.0, y: 600.0 },
                fitted_fill: Pair { x: 1.0, y: 1.0 },
            },
            measurements: vec![PerformanceMeasurement {
                name: "fixture".to_owned(),
                duration_ns: sequence as u128,
            }],
        }
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("{label}-{nanos}"))
    }
}
