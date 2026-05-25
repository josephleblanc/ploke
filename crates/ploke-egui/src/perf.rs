//! Typed performance snapshots for `ploke-egui` projection baselines.
//!
//! These logs are deliberately coarse and persisted through named records.
//! Interactive profiler backends receive finer spans through `profiling::scope!`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
use std::{fmt, fs::File};

use eframe::egui::Vec2;
use ploke_tree::Graph;
use serde::{Deserialize, Serialize};

use crate::diagnostics::Pair;
use crate::import::graph_from_run_root;
use crate::ui::view::{GraphView, GraphViewDiagnostics, GraphViewMode};

const PERFORMANCE_LOG_VERSION: &str = "ploke-egui.performance-log.v1";
const DEFAULT_MAX_LOGS: u64 = 5;
#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
const DEFAULT_MAX_PUFFIN_CAPTURES: u64 = 5;

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

#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
pub struct PuffinCapture {
    view: puffin::GlobalFrameView,
    sink: RollingPuffinSink,
    frame_target: usize,
    run_root: PathBuf,
    mode: GraphViewMode,
    close_when_done: bool,
    written: bool,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
impl fmt::Debug for PuffinCapture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PuffinCapture")
            .field("frame_target", &self.frame_target)
            .field("run_root", &self.run_root)
            .field("mode", &self.mode)
            .field("close_when_done", &self.close_when_done)
            .field("written", &self.written)
            .finish_non_exhaustive()
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
impl PuffinCapture {
    pub fn new(
        frame_target: usize,
        root: impl Into<PathBuf>,
        run_root: PathBuf,
        mode: GraphViewMode,
        close_when_done: bool,
    ) -> io::Result<Self> {
        if frame_target == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "--puffin-capture-frames must be greater than zero",
            ));
        }

        puffin::set_scopes_on(true);
        let view = puffin::GlobalFrameView::default();
        {
            let mut view = view.lock();
            view.set_max_recent(frame_target);
            view.set_max_slow(frame_target.min(64));
        }

        Ok(Self {
            view,
            sink: RollingPuffinSink::new(root)?,
            frame_target,
            run_root,
            mode,
            close_when_done,
            written: false,
        })
    }

    pub fn observe_frame(
        &mut self,
        diagnostics: Option<&GraphViewDiagnostics>,
    ) -> io::Result<PuffinCaptureStatus> {
        profiling::scope!("ploke-egui.puffin-capture.observe-frame");
        if self.written {
            return Ok(PuffinCaptureStatus::AlreadyComplete);
        }

        let view = self.view.lock();
        let frame_count = view.stats_full().frames();
        if frame_count < self.frame_target {
            return Ok(PuffinCaptureStatus::Collecting { frame_count });
        }

        let summary = PuffinCaptureSummary::from_view(
            &view,
            &self.run_root,
            self.mode,
            self.frame_target,
            diagnostics,
        );
        self.sink.write_capture(&view, &summary)?;
        self.written = true;

        Ok(PuffinCaptureStatus::Complete {
            close: self.close_when_done,
        })
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PuffinCaptureStatus {
    Collecting { frame_count: usize },
    Complete { close: bool },
    AlreadyComplete,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
#[derive(Debug)]
struct RollingPuffinSink {
    root: PathBuf,
    max_captures: u64,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
impl RollingPuffinSink {
    fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("runs"))?;
        Ok(Self {
            root,
            max_captures: DEFAULT_MAX_PUFFIN_CAPTURES,
        })
    }

    fn write_capture(
        &self,
        view: &puffin::FrameView,
        summary: &PuffinCaptureSummary,
    ) -> io::Result<()> {
        let sequence = self.latest_sequence().saturating_add(1);
        let slot = ((sequence - 1) % self.max_captures) + 1;
        let latest_capture = self.root.join("latest.puffin");
        let latest_summary = self.root.join("latest.txt");
        let run_capture = self.root.join("runs").join(format!("{slot:02}.puffin"));
        let run_summary = self.root.join("runs").join(format!("{slot:02}.txt"));
        let sequence_path = self.root.join("latest-sequence.txt");
        let rendered = summary.render_text(sequence);

        write_puffin_file(view, &latest_capture)?;
        write_puffin_file(view, &run_capture)?;
        fs::write(latest_summary, &rendered)?;
        fs::write(run_summary, rendered)?;
        fs::write(sequence_path, sequence.to_string())
    }

    fn latest_sequence(&self) -> u64 {
        let Ok(text) = fs::read_to_string(self.root.join("latest-sequence.txt")) else {
            return 0;
        };
        text.trim().parse().unwrap_or(0)
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
fn write_puffin_file(view: &puffin::FrameView, path: &Path) -> io::Result<()> {
    let mut file = File::create(path)?;
    view.write(&mut file).map_err(io::Error::other)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
#[derive(Debug)]
struct PuffinCaptureSummary {
    run_root: String,
    mode: GraphViewMode,
    target_frames: usize,
    captured_frames: usize,
    min_frame_ns: Option<i64>,
    median_frame_ns: Option<i64>,
    p95_frame_ns: Option<i64>,
    max_frame_ns: Option<i64>,
    graph: Option<PerformanceGraphFacts>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
impl PuffinCaptureSummary {
    fn from_view(
        view: &puffin::FrameView,
        run_root: &Path,
        mode: GraphViewMode,
        target_frames: usize,
        diagnostics: Option<&GraphViewDiagnostics>,
    ) -> Self {
        let mut durations = view
            .recent_frames()
            .map(|frame| frame.duration_ns())
            .collect::<Vec<_>>();
        durations.sort_unstable();
        Self {
            run_root: run_root.display().to_string(),
            mode,
            target_frames,
            captured_frames: durations.len(),
            min_frame_ns: durations.first().copied(),
            median_frame_ns: percentile(&durations, 50),
            p95_frame_ns: percentile(&durations, 95),
            max_frame_ns: durations.last().copied(),
            graph: diagnostics.map(PerformanceGraphFacts::from_diagnostics),
        }
    }

    fn render_text(&self, sequence: u64) -> String {
        let mut text = String::new();
        text.push_str("ploke-egui puffin capture\n");
        text.push_str(&format!("sequence: {sequence}\n"));
        text.push_str(&format!("run_root: {}\n", self.run_root));
        text.push_str(&format!("mode: {}\n", self.mode.as_str()));
        text.push_str(&format!("target_frames: {}\n", self.target_frames));
        text.push_str(&format!("captured_frames: {}\n", self.captured_frames));
        text.push_str(&format!(
            "min_frame_ms: {}\n",
            render_millis(self.min_frame_ns)
        ));
        text.push_str(&format!(
            "median_frame_ms: {}\n",
            render_millis(self.median_frame_ns)
        ));
        text.push_str(&format!(
            "p95_frame_ms: {}\n",
            render_millis(self.p95_frame_ns)
        ));
        text.push_str(&format!(
            "max_frame_ms: {}\n",
            render_millis(self.max_frame_ns)
        ));
        if let Some(graph) = &self.graph {
            text.push_str(&format!("graph_nodes: {}\n", graph.visible_node_count));
            text.push_str(&format!("graph_edges: {}\n", graph.visible_edge_count));
            text.push_str(&format!(
                "graph_size: {:.0} x {:.0}\n",
                graph.graph_size.x, graph.graph_size.y
            ));
        }
        text.push_str("capture: latest.puffin\n");
        text
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
fn percentile(sorted: &[i64], percentile: usize) -> Option<i64> {
    if sorted.is_empty() {
        return None;
    }
    let index = ((sorted.len() * percentile).div_ceil(100)).saturating_sub(1);
    sorted.get(index.min(sorted.len() - 1)).copied()
}

#[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
fn render_millis(nanos: Option<i64>) -> String {
    match nanos {
        Some(nanos) => format!("{:.3}", nanos as f64 / 1_000_000.0),
        None => "not_available".to_owned(),
    }
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

    #[cfg(all(not(target_arch = "wasm32"), feature = "profile-with-puffin"))]
    #[test]
    fn puffin_sink_writes_capture_and_text_summary() {
        let root = unique_temp_dir("ploke-egui-puffin-capture");
        let sink = RollingPuffinSink::new(&root).expect("sink");

        puffin::set_scopes_on(true);
        let view = puffin::GlobalFrameView::default();
        {
            puffin::profile_scope!("ploke-egui.test-frame");
        }
        puffin::GlobalProfiler::lock().new_frame();

        let view = view.lock();
        let summary = PuffinCaptureSummary::from_view(
            &view,
            Path::new("/runs/p1-five-gen-1x3-20260516-1/prototype1"),
            GraphViewMode::ArtifactTree,
            1,
            None,
        );
        sink.write_capture(&view, &summary).expect("write capture");

        assert!(root.join("latest.puffin").is_file());
        assert!(root.join("latest.txt").is_file());
        assert!(root.join("runs/01.puffin").is_file());
        assert!(root.join("runs/01.txt").is_file());
        let summary = fs::read_to_string(root.join("latest.txt")).expect("summary");
        assert!(summary.contains("ploke-egui puffin capture"));
        assert!(summary.contains("target_frames: 1"));
        assert!(summary.contains("capture: latest.puffin"));
        fs::remove_dir_all(root).expect("cleanup");
    }
}
