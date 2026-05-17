//! Native benchmark suite driver and typed report records.
//!
//! The CLI selects a benchmark run; this module owns interpretation, timing,
//! and report persistence.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::io;
#[cfg(feature = "native-benchmark")]
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use ploke_records::run_record::read_compressed_record_profiled;
use ploke_tree::{FsRunStore, Graph, RunRecordSet};
use serde::{Deserialize, Serialize};
#[cfg(feature = "native-benchmark")]
use sha2::{Digest, Sha256};

use crate::allocation::{self, AllocationDelta, AllocationSnapshot};
use crate::ui::view::GraphViewMode;

pub const STANDARD_RUN_ROOT: &str =
    "/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1";

const BENCHMARK_REPORT_VERSION: &str = "ploke-egui.native-benchmark-report.v2";
const STANDARD_FRAME_TARGET: usize = 300;
const TOP_FRAME_LIMIT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkSuite {
    Standard,
}

impl BenchmarkSuite {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "standard" => Ok(Self::Standard),
            other => Err(format!(
                "unknown benchmark suite '{other}' (expected standard)"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkScenario {
    StartupFrames300,
    WarmIdle300,
    SelectArtifactInspector300,
    PatchDebugCold300,
    PatchDebugWarm300,
    ModeLineage300,
    ModeArtifactTree300,
    ToggleHideUnconsideredChildren300,
}

impl BenchmarkScenario {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "startup_frames_300" => Ok(Self::StartupFrames300),
            "warm_idle_300" => Ok(Self::WarmIdle300),
            "select_artifact_inspector_300" => Ok(Self::SelectArtifactInspector300),
            "patch_debug_cold_300" => Ok(Self::PatchDebugCold300),
            "patch_debug_warm_300" => Ok(Self::PatchDebugWarm300),
            "mode_lineage_300" => Ok(Self::ModeLineage300),
            "mode_artifact_tree_300" => Ok(Self::ModeArtifactTree300),
            "toggle_hide_unconsidered_children_300" => Ok(Self::ToggleHideUnconsideredChildren300),
            other => Err(format!("unknown benchmark scenario '{other}'")),
        }
    }

    pub fn standard() -> Vec<Self> {
        vec![
            Self::StartupFrames300,
            Self::WarmIdle300,
            Self::SelectArtifactInspector300,
            Self::PatchDebugCold300,
            Self::PatchDebugWarm300,
            Self::ModeLineage300,
            Self::ModeArtifactTree300,
            Self::ToggleHideUnconsideredChildren300,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::StartupFrames300 => "startup_frames_300",
            Self::WarmIdle300 => "warm_idle_300",
            Self::SelectArtifactInspector300 => "select_artifact_inspector_300",
            Self::PatchDebugCold300 => "patch_debug_cold_300",
            Self::PatchDebugWarm300 => "patch_debug_warm_300",
            Self::ModeLineage300 => "mode_lineage_300",
            Self::ModeArtifactTree300 => "mode_artifact_tree_300",
            Self::ToggleHideUnconsideredChildren300 => "toggle_hide_unconsidered_children_300",
        }
    }

    fn frame_target(self) -> usize {
        STANDARD_FRAME_TARGET
    }

    pub fn action(self) -> BenchmarkAction {
        match self {
            Self::StartupFrames300 | Self::WarmIdle300 => BenchmarkAction::None,
            Self::SelectArtifactInspector300 => BenchmarkAction::SelectArtifact {
                patch_debug_open: false,
                reset_patch_cache: false,
            },
            Self::PatchDebugCold300 => BenchmarkAction::SelectArtifact {
                patch_debug_open: true,
                reset_patch_cache: true,
            },
            Self::PatchDebugWarm300 => BenchmarkAction::SelectArtifact {
                patch_debug_open: true,
                reset_patch_cache: false,
            },
            Self::ModeLineage300 => BenchmarkAction::SetMode(GraphViewMode::Lineage),
            Self::ModeArtifactTree300 => BenchmarkAction::SetMode(GraphViewMode::ArtifactTree),
            Self::ToggleHideUnconsideredChildren300 => {
                BenchmarkAction::ToggleHideUnconsideredChildren
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkAction {
    None,
    SelectArtifact {
        patch_debug_open: bool,
        reset_patch_cache: bool,
    },
    SetMode(GraphViewMode),
    ToggleHideUnconsideredChildren,
}

impl BenchmarkAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SelectArtifact {
                patch_debug_open: false,
                ..
            } => "select_artifact_inspector",
            Self::SelectArtifact {
                patch_debug_open: true,
                reset_patch_cache: true,
            } => "patch_debug_cold",
            Self::SelectArtifact {
                patch_debug_open: true,
                reset_patch_cache: false,
            } => "patch_debug_warm",
            Self::SetMode(GraphViewMode::Lineage) => "set_mode_lineage",
            Self::SetMode(GraphViewMode::ArtifactTree) => "set_mode_artifact_tree",
            Self::SetMode(GraphViewMode::ArtifactAndLineage) => "set_mode_artifact_and_lineage",
            Self::SetMode(GraphViewMode::Empty) => "set_mode_empty",
            Self::ToggleHideUnconsideredChildren => "toggle_hide_unconsidered_children",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub suite: BenchmarkSuite,
    pub run_root: PathBuf,
    pub output_dir: PathBuf,
    pub scenarios: Vec<BenchmarkScenario>,
    pub command: String,
}

impl BenchmarkConfig {
    pub fn new(
        suite: BenchmarkSuite,
        run_root: PathBuf,
        output_dir: Option<PathBuf>,
        scenario_filters: Vec<String>,
    ) -> Result<Self, String> {
        if suite == BenchmarkSuite::Standard && run_root != Path::new(STANDARD_RUN_ROOT) {
            return Err(format!(
                "benchmark suite standard v1 requires --run-root {STANDARD_RUN_ROOT}"
            ));
        }

        let scenarios = if scenario_filters.is_empty() {
            BenchmarkScenario::standard()
        } else {
            scenario_filters
                .iter()
                .map(|value| BenchmarkScenario::parse(value))
                .collect::<Result<Vec<_>, _>>()?
        };
        if scenarios.is_empty() {
            return Err("benchmark scenario list is empty".to_owned());
        }

        Ok(Self {
            suite,
            run_root,
            output_dir: output_dir.unwrap_or_else(|| default_output_dir(suite)),
            scenarios,
            command: std::env::args().collect::<Vec<_>>().join(" "),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub schema_version: String,
    pub suite: String,
    pub created_at_unix_ms: u64,
    pub commit: GitInfo,
    pub dirty_state: DirtyState,
    pub command: String,
    pub run_root: String,
    pub feature_set: Vec<String>,
    pub scenarios_requested: Vec<String>,
    pub startup: StartupProfile,
    pub scenarios: Vec<ScenarioReport>,
    pub puffin_artifacts: Vec<BenchmarkArtifact>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitInfo {
    pub short_sha: Option<String>,
    pub full_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtyState {
    pub dirty: Option<bool>,
    pub classification: DirtyStateClassification,
    pub relevant_dirty: Option<bool>,
    pub paths: Vec<String>,
    pub relevant_paths: Vec<String>,
    pub unrelated_paths: Vec<String>,
    pub scope: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirtyStateClassification {
    Clean,
    DirtyRelevant,
    DirtyUnrelated,
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupProfile {
    pub spans: Vec<BenchmarkSpan>,
    pub compressed_run_records: Vec<CompressedRunRecordRead>,
    pub notes: Vec<String>,
}

impl StartupProfile {
    pub fn push_span(&mut self, name: impl Into<String>, duration_ns: u64) {
        self.spans.push(BenchmarkSpan {
            name: name.into(),
            duration_ns,
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkSpan {
    pub name: String,
    pub duration_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressedRunRecordRead {
    pub path: String,
    pub compressed_bytes: u64,
    pub decompressed_bytes: u64,
    pub turn_count: usize,
    pub tool_call_count: usize,
    pub failed_tool_call_count: usize,
    pub open_read_ns: u64,
    pub gzip_decompress_ns: u64,
    pub typed_json_deserialize_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioReport {
    pub name: String,
    pub target_frames: usize,
    pub frame_stats: DurationStats,
    pub top_frames: Vec<TopFrame>,
    pub component_timings: Vec<ComponentTimingReport>,
    pub allocations: AllocationDelta,
    pub action: BenchmarkActionReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkActionReport {
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl BenchmarkActionReport {
    pub fn for_action(action: BenchmarkAction) -> Self {
        Self {
            action: action.as_str().to_owned(),
            target_label: None,
            target_key: None,
            fallback: None,
            notes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopFrame {
    pub frame_index: usize,
    pub duration_ns: u64,
    pub components: Vec<ComponentTiming>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentTimingReport {
    pub component: String,
    pub stats: DurationStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentTiming {
    pub component: String,
    pub duration_ns: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurationStats {
    pub count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p95_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p99_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_ns: Option<u64>,
}

impl DurationStats {
    pub fn from_durations(durations: &[u64]) -> Self {
        let mut sorted = durations.to_vec();
        sorted.sort_unstable();
        Self {
            count: sorted.len(),
            min_ns: sorted.first().copied(),
            median_ns: percentile(&sorted, 50),
            p95_ns: percentile(&sorted, 95),
            p99_ns: percentile(&sorted, 99),
            max_ns: sorted.last().copied(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkArtifact {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub tracked: bool,
}

#[derive(Debug)]
pub struct BenchmarkWriteResult {
    pub report_path: PathBuf,
    pub readme_path: PathBuf,
}

pub struct BenchmarkController {
    config: BenchmarkConfig,
    startup: StartupProfile,
    created_at_unix_ms: u64,
    git: GitInfo,
    dirty_state: DirtyState,
    feature_set: Vec<String>,
    scenario_index: usize,
    current: Option<ScenarioCapture>,
    completed: Vec<ScenarioReport>,
    current_components: Vec<ComponentTiming>,
    frame_start: Option<Instant>,
    finished: bool,
    #[cfg(feature = "native-benchmark")]
    puffin_view: puffin::GlobalFrameView,
}

impl std::fmt::Debug for BenchmarkController {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BenchmarkController")
            .field("config", &self.config)
            .field("startup", &self.startup)
            .field("created_at_unix_ms", &self.created_at_unix_ms)
            .field("git", &self.git)
            .field("dirty_state", &self.dirty_state)
            .field("feature_set", &self.feature_set)
            .field("scenario_index", &self.scenario_index)
            .field("current", &self.current)
            .field("completed", &self.completed)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl BenchmarkController {
    pub fn new(config: BenchmarkConfig, startup: StartupProfile) -> Self {
        #[cfg(feature = "native-benchmark")]
        let puffin_view = {
            puffin::set_scopes_on(true);
            let view = puffin::GlobalFrameView::default();
            {
                let mut locked = view.lock();
                let frames = config.scenarios.len() * STANDARD_FRAME_TARGET;
                locked.set_max_recent(frames.saturating_add(64));
                locked.set_max_slow(128);
            }
            view
        };

        Self {
            config,
            startup,
            created_at_unix_ms: unix_ms_now(),
            git: git_info(),
            dirty_state: dirty_state(),
            feature_set: feature_set(),
            scenario_index: 0,
            current: None,
            completed: Vec::new(),
            current_components: Vec::new(),
            frame_start: None,
            finished: false,
            #[cfg(feature = "native-benchmark")]
            puffin_view,
        }
    }

    pub fn begin_frame(&mut self) -> Option<BenchmarkAction> {
        if self.finished {
            return None;
        }

        self.current_components.clear();
        self.frame_start = Some(Instant::now());

        if self.current.is_none() {
            let scenario = *self.config.scenarios.get(self.scenario_index)?;
            self.scenario_index += 1;
            let action = scenario.action();
            self.current = Some(ScenarioCapture::new(scenario, allocation::snapshot()));
            return Some(action);
        }

        None
    }

    pub fn record_action(&mut self, report: BenchmarkActionReport) {
        if let Some(current) = &mut self.current {
            current.action = report;
        }
    }

    pub fn record_component(&mut self, component: &'static str, duration_ns: u64) {
        if self.current.is_some() {
            self.current_components.push(ComponentTiming {
                component: component.to_owned(),
                duration_ns,
            });
        }
    }

    pub fn end_frame(&mut self) -> Result<Option<BenchmarkWriteResult>, Box<dyn Error>> {
        if self.finished {
            return Ok(None);
        }
        let Some(frame_start) = self.frame_start.take() else {
            return Ok(None);
        };
        let frame_duration = elapsed_ns(frame_start);
        let components = std::mem::take(&mut self.current_components);
        let Some(current) = &mut self.current else {
            return Ok(None);
        };
        current.frames.push(FrameSample {
            frame_index: current.frames.len() + 1,
            duration_ns: frame_duration,
            components,
        });

        if current.frames.len() < current.scenario.frame_target() {
            return Ok(None);
        }

        let capture = self
            .current
            .take()
            .expect("current scenario exists after frame push");
        self.completed.push(capture.finish(allocation::snapshot()));
        if self.completed.len() == self.config.scenarios.len() {
            self.finished = true;
            return self.write_report().map(Some);
        }

        Ok(None)
    }

    fn write_report(&self) -> Result<BenchmarkWriteResult, Box<dyn Error>> {
        #[cfg(feature = "native-benchmark")]
        let puffin_artifacts = vec![self.write_puffin_capture()?];
        #[cfg(not(feature = "native-benchmark"))]
        let puffin_artifacts = Vec::new();

        let report = BenchmarkReport {
            schema_version: BENCHMARK_REPORT_VERSION.to_owned(),
            suite: self.config.suite.as_str().to_owned(),
            created_at_unix_ms: self.created_at_unix_ms,
            commit: self.git.clone(),
            dirty_state: self.dirty_state.clone(),
            command: self.config.command.clone(),
            run_root: self.config.run_root.display().to_string(),
            feature_set: self.feature_set.clone(),
            scenarios_requested: self
                .config
                .scenarios
                .iter()
                .map(|scenario| scenario.as_str().to_owned())
                .collect(),
            startup: self.startup.clone(),
            scenarios: self.completed.clone(),
            puffin_artifacts,
            notes: benchmark_notes(&self.dirty_state),
        };

        fs::create_dir_all(&self.config.output_dir)?;
        let report_path = self.config.output_dir.join("report.json");
        let readme_path = self.config.output_dir.join("README.md");
        fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
        fs::write(&readme_path, render_benchmark_readme(&report))?;
        Ok(BenchmarkWriteResult {
            report_path,
            readme_path,
        })
    }

    #[cfg(feature = "native-benchmark")]
    fn write_puffin_capture(&self) -> io::Result<BenchmarkArtifact> {
        let root = default_puffin_benchmark_dir(&self.config.output_dir);
        fs::create_dir_all(&root)?;
        let path = root.join(format!("{}.puffin", self.config.suite.as_str()));
        let view = self.puffin_view.lock();
        let mut file = fs::File::create(&path)?;
        view.write(&mut file).map_err(io::Error::other)?;
        file_artifact(&path, false)
    }
}

fn benchmark_notes(dirty_state: &DirtyState) -> Vec<String> {
    let mut notes = vec![
        "reporting-only benchmark; no pass/fail thresholds applied".to_owned(),
        "allocation deltas are process-wide and exclude GPU/driver memory".to_owned(),
        "puffin captures are local ignored artifacts under crates/ploke-egui/data/".to_owned(),
    ];
    if dirty_state.classification == DirtyStateClassification::DirtyUnrelated {
        notes.push(
            "git worktree was dirty only outside benchmark-relevant paths at benchmark start"
                .to_owned(),
        );
    }
    notes
}

#[derive(Debug)]
struct ScenarioCapture {
    scenario: BenchmarkScenario,
    frames: Vec<FrameSample>,
    action: BenchmarkActionReport,
    allocation_start: AllocationSnapshot,
}

impl ScenarioCapture {
    fn new(scenario: BenchmarkScenario, allocation_start: AllocationSnapshot) -> Self {
        Self {
            scenario,
            frames: Vec::new(),
            action: BenchmarkActionReport::for_action(scenario.action()),
            allocation_start,
        }
    }

    fn finish(self, allocation_end: AllocationSnapshot) -> ScenarioReport {
        let durations = self
            .frames
            .iter()
            .map(|frame| frame.duration_ns)
            .collect::<Vec<_>>();
        ScenarioReport {
            name: self.scenario.as_str().to_owned(),
            target_frames: self.scenario.frame_target(),
            frame_stats: DurationStats::from_durations(&durations),
            top_frames: top_frames(&self.frames),
            component_timings: component_reports(&self.frames),
            allocations: allocation_end.delta_since(self.allocation_start),
            action: self.action,
        }
    }
}

#[derive(Debug)]
struct FrameSample {
    frame_index: usize,
    duration_ns: u64,
    components: Vec<ComponentTiming>,
}

pub fn load_graph_with_startup_profile(
    run_root: &Path,
    mut startup: StartupProfile,
) -> Result<(Graph, StartupProfile), Box<dyn Error>> {
    let total_start = Instant::now();
    let store = FsRunStore::new(run_root);

    let load_record_set_start = Instant::now();
    let load_start = Instant::now();
    let forest_input = store.load()?;
    startup.push_span("FsRunStore::load", elapsed_ns(load_start));

    let history_start = Instant::now();
    let history_blocks = store.load_history_blocks()?;
    startup.push_span("FsRunStore::load_history_blocks", elapsed_ns(history_start));

    let journal_start = Instant::now();
    let transition_journal = store.load_transition_journal()?;
    startup.push_span(
        "FsRunStore::load_transition_journal",
        elapsed_ns(journal_start),
    );

    let records = RunRecordSet {
        forest_input,
        history_blocks,
        transition_journal,
    };
    startup.push_span(
        "FsRunStore::load_record_set",
        elapsed_ns(load_record_set_start),
    );

    let compressed_start = Instant::now();
    startup.compressed_run_records = compressed_record_reads(&records)?;
    startup.push_span(
        "compressed_run_record_profile_probe",
        elapsed_ns(compressed_start),
    );

    let graph_start = Instant::now();
    let graph = Graph::from_records(&records);
    startup.push_span("Graph::from_records", elapsed_ns(graph_start));
    startup.push_span("graph_load_total", elapsed_ns(total_start));
    Ok((graph, startup))
}

pub fn span_from_start(name: &'static str, start: Instant) -> BenchmarkSpan {
    BenchmarkSpan {
        name: name.to_owned(),
        duration_ns: elapsed_ns(start),
    }
}

pub fn default_output_dir(suite: BenchmarkSuite) -> PathBuf {
    let date = Utc::now().format("%Y%m%d");
    let short = git_info()
        .short_sha
        .unwrap_or_else(|| "unknownsha".to_owned());
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("profiling")
        .join("benchmarks")
        .join(format!("{date}-{short}-{}", suite.as_str()))
}

fn compressed_record_reads(
    records: &RunRecordSet,
) -> Result<Vec<CompressedRunRecordRead>, io::Error> {
    let mut paths = BTreeSet::new();
    if let Some(run_records) = &records.forest_input.passive_evidence.run_records {
        for refs in run_records.refs_by_branch.values() {
            for record_ref in refs {
                paths.insert(record_ref.record_path.clone());
            }
        }
    }

    paths
        .iter()
        .map(|path| {
            let profiled = read_compressed_record_profiled(path)?;
            Ok(CompressedRunRecordRead {
                path: path.display().to_string(),
                compressed_bytes: profiled.profile.compressed_bytes,
                decompressed_bytes: profiled.profile.decompressed_bytes,
                turn_count: profiled.record.turn_count(),
                tool_call_count: profiled.record.tool_call_count(),
                failed_tool_call_count: profiled.record.failed_tool_call_count(),
                open_read_ns: profiled.profile.open_read_ns,
                gzip_decompress_ns: profiled.profile.decompress_ns,
                typed_json_deserialize_ns: profiled.profile.deserialize_ns,
            })
        })
        .collect()
}

fn top_frames(frames: &[FrameSample]) -> Vec<TopFrame> {
    let mut frames = frames
        .iter()
        .map(|frame| TopFrame {
            frame_index: frame.frame_index,
            duration_ns: frame.duration_ns,
            components: frame.components.clone(),
        })
        .collect::<Vec<_>>();
    frames.sort_by(|left, right| {
        right
            .duration_ns
            .cmp(&left.duration_ns)
            .then_with(|| left.frame_index.cmp(&right.frame_index))
    });
    frames.truncate(TOP_FRAME_LIMIT);
    frames
}

fn component_reports(frames: &[FrameSample]) -> Vec<ComponentTimingReport> {
    let mut durations: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for frame in frames {
        for component in &frame.components {
            durations
                .entry(component.component.clone())
                .or_default()
                .push(component.duration_ns);
        }
    }
    durations
        .into_iter()
        .map(|(component, durations)| ComponentTimingReport {
            component,
            stats: DurationStats::from_durations(&durations),
        })
        .collect()
}

fn percentile(sorted: &[u64], percentile: usize) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let index = ((sorted.len() * percentile).div_ceil(100)).saturating_sub(1);
    sorted.get(index.min(sorted.len() - 1)).copied()
}

fn render_benchmark_readme(report: &BenchmarkReport) -> String {
    let mut text = String::new();
    text.push_str("# ploke-egui Native Benchmark\n\n");
    text.push_str(&format!("suite: `{}`\n\n", report.suite));
    text.push_str(&format!("run_root: `{}`\n\n", report.run_root));
    if let Some(short) = &report.commit.short_sha {
        text.push_str(&format!("commit: `{short}`\n\n"));
    }
    text.push_str(&format!(
        "dirty_state: `{}`\n\n",
        report.dirty_state.classification.as_str()
    ));
    if !report.dirty_state.relevant_paths.is_empty() {
        text.push_str("benchmark-relevant dirty paths:\n");
        for path in &report.dirty_state.relevant_paths {
            text.push_str(&format!("- `{path}`\n"));
        }
        text.push('\n');
    }
    if !report.dirty_state.unrelated_paths.is_empty() {
        text.push_str("unrelated dirty paths:\n");
        for path in &report.dirty_state.unrelated_paths {
            text.push_str(&format!("- `{path}`\n"));
        }
        text.push('\n');
    }
    text.push_str("## Scenarios\n\n");
    for scenario in &report.scenarios {
        text.push_str(&format!(
            "- `{}`: frames={}, median={} ns, p95={} ns, max={} ns\n",
            scenario.name,
            scenario.frame_stats.count,
            render_optional_ns(scenario.frame_stats.median_ns),
            render_optional_ns(scenario.frame_stats.p95_ns),
            render_optional_ns(scenario.frame_stats.max_ns)
        ));
    }
    text.push_str("\n## Local Puffin Captures\n\n");
    if report.puffin_artifacts.is_empty() {
        text.push_str("- none recorded\n");
    } else {
        for artifact in &report.puffin_artifacts {
            text.push_str(&format!(
                "- `{}`: {} bytes, sha256 `{}`\n",
                artifact.path, artifact.bytes, artifact.sha256
            ));
        }
    }
    text.push_str("\nSee `report.json` for typed timings and allocation deltas.\n");
    text
}

fn render_optional_ns(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_owned())
}

fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn elapsed_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().try_into().unwrap_or(u64::MAX)
}

fn git_info() -> GitInfo {
    GitInfo {
        short_sha: git_output(["rev-parse", "--short=12", "HEAD"]),
        full_sha: git_output(["rev-parse", "HEAD"]),
    }
}

fn dirty_state() -> DirtyState {
    let Some(status) = git_output(["status", "--short"]) else {
        return DirtyState {
            dirty: None,
            classification: DirtyStateClassification::Unknown,
            relevant_dirty: None,
            paths: Vec::new(),
            relevant_paths: Vec::new(),
            unrelated_paths: Vec::new(),
            scope: "git status --short at benchmark start".to_owned(),
        };
    };
    let paths = git_status_paths(&status);
    let relevant_paths: Vec<_> = paths
        .iter()
        .filter(|path| is_benchmark_relevant_dirty_path(path))
        .cloned()
        .collect();
    let unrelated_paths: Vec<_> = paths
        .iter()
        .filter(|path| !is_benchmark_relevant_dirty_path(path))
        .cloned()
        .collect();
    let dirty = !paths.is_empty();
    let relevant_dirty = !relevant_paths.is_empty();
    let classification = match (dirty, relevant_dirty) {
        (false, _) => DirtyStateClassification::Clean,
        (true, true) => DirtyStateClassification::DirtyRelevant,
        (true, false) => DirtyStateClassification::DirtyUnrelated,
    };

    DirtyState {
        dirty: Some(dirty),
        classification,
        relevant_dirty: Some(relevant_dirty),
        paths,
        relevant_paths,
        unrelated_paths,
        scope: "git status --short at benchmark start; benchmark-relevant prefixes are Cargo manifests, ploke-egui, ploke-tree, and ploke-records".to_owned(),
    }
}

impl DirtyStateClassification {
    fn as_str(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::DirtyRelevant => "dirty_relevant",
            Self::DirtyUnrelated => "dirty_unrelated",
            Self::Unknown => "unknown",
        }
    }
}

fn git_status_paths(status: &str) -> Vec<String> {
    status
        .lines()
        .filter_map(|line| line.get(3..).map(str::trim))
        .filter(|path| !path.is_empty())
        .map(|path| path.split(" -> ").last().unwrap_or(path).to_owned())
        .collect()
}

fn is_benchmark_relevant_dirty_path(path: &str) -> bool {
    path == "Cargo.lock"
        || path == "Cargo.toml"
        || path.starts_with("crates/ploke-egui/")
        || path.starts_with("crates/ploke-tree/")
        || path.starts_with("crates/ploke-records/")
}

fn git_output<const N: usize>(args: [&str; N]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(workspace_root())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn feature_set() -> Vec<String> {
    let mut features = Vec::new();
    if cfg!(feature = "dev") {
        features.push("dev".to_owned());
    }
    if cfg!(feature = "native-benchmark") {
        features.push("native-benchmark".to_owned());
    }
    if cfg!(feature = "profile-with-puffin") {
        features.push("profile-with-puffin".to_owned());
    }
    features
}

#[cfg(feature = "native-benchmark")]
fn default_puffin_benchmark_dir(output_dir: &Path) -> PathBuf {
    let name = output_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "custom-output".to_owned());
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("profiling")
        .join("puffin")
        .join("benchmarks")
        .join(name)
}

#[cfg(feature = "native-benchmark")]
fn file_artifact(path: &Path, tracked: bool) -> io::Result<BenchmarkArtifact> {
    let metadata = fs::metadata(path)?;
    Ok(BenchmarkArtifact {
        path: path.display().to_string(),
        bytes: metadata.len(),
        sha256: sha256_file(path)?,
        tracked,
    })
}

#[cfg(feature = "native-benchmark")]
fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_report_json_roundtrips() {
        let report = BenchmarkReport {
            schema_version: BENCHMARK_REPORT_VERSION.to_owned(),
            suite: "standard".to_owned(),
            created_at_unix_ms: 1,
            commit: GitInfo {
                short_sha: Some("abc123".to_owned()),
                full_sha: Some("abc123def".to_owned()),
            },
            dirty_state: DirtyState {
                dirty: Some(false),
                classification: DirtyStateClassification::Clean,
                relevant_dirty: Some(false),
                paths: Vec::new(),
                relevant_paths: Vec::new(),
                unrelated_paths: Vec::new(),
                scope: "fixture".to_owned(),
            },
            command: "ploke-egui --benchmark-suite standard".to_owned(),
            run_root: STANDARD_RUN_ROOT.to_owned(),
            feature_set: vec!["dev".to_owned(), "native-benchmark".to_owned()],
            scenarios_requested: vec!["startup_frames_300".to_owned()],
            startup: StartupProfile {
                spans: vec![BenchmarkSpan {
                    name: "Graph::from_records".to_owned(),
                    duration_ns: 42,
                }],
                compressed_run_records: Vec::new(),
                notes: Vec::new(),
            },
            scenarios: vec![ScenarioReport {
                name: "startup_frames_300".to_owned(),
                target_frames: 300,
                frame_stats: DurationStats::from_durations(&[1, 2, 3]),
                top_frames: vec![TopFrame {
                    frame_index: 3,
                    duration_ns: 3,
                    components: vec![ComponentTiming {
                        component: "central_graph".to_owned(),
                        duration_ns: 2,
                    }],
                }],
                component_timings: vec![ComponentTimingReport {
                    component: "central_graph".to_owned(),
                    stats: DurationStats::from_durations(&[2]),
                }],
                allocations: AllocationSnapshot {
                    enabled: true,
                    allocation_count: 10,
                    deallocation_count: 4,
                    allocated_bytes: 100,
                    deallocated_bytes: 40,
                }
                .delta_since(AllocationSnapshot {
                    enabled: true,
                    allocation_count: 1,
                    deallocation_count: 1,
                    allocated_bytes: 10,
                    deallocated_bytes: 10,
                }),
                action: BenchmarkActionReport::for_action(BenchmarkAction::None),
            }],
            puffin_artifacts: vec![BenchmarkArtifact {
                path: "crates/ploke-egui/data/profiling/puffin/benchmarks/x/standard.puffin"
                    .to_owned(),
                bytes: 12,
                sha256: "00".to_owned(),
                tracked: false,
            }],
            notes: vec!["fixture".to_owned()],
        };

        let encoded = serde_json::to_string_pretty(&report).expect("serialize report");
        let decoded: BenchmarkReport = serde_json::from_str(&encoded).expect("deserialize report");
        assert_eq!(decoded, report);
    }

    #[test]
    fn benchmark_duration_stats_percentiles_and_top_frames() {
        let stats = DurationStats::from_durations(&[10, 20, 30, 40, 50]);
        assert_eq!(stats.min_ns, Some(10));
        assert_eq!(stats.median_ns, Some(30));
        assert_eq!(stats.p95_ns, Some(50));
        assert_eq!(stats.p99_ns, Some(50));
        assert_eq!(stats.max_ns, Some(50));

        let frames = vec![
            FrameSample {
                frame_index: 1,
                duration_ns: 10,
                components: Vec::new(),
            },
            FrameSample {
                frame_index: 2,
                duration_ns: 50,
                components: Vec::new(),
            },
            FrameSample {
                frame_index: 3,
                duration_ns: 30,
                components: Vec::new(),
            },
        ];
        let top = top_frames(&frames);
        assert_eq!(top[0].frame_index, 2);
        assert_eq!(top[1].frame_index, 3);
        assert_eq!(top[2].frame_index, 1);
    }

    #[test]
    fn dirty_state_path_classification_keeps_docs_unrelated() {
        let paths = git_status_paths(
            " M docs/workflow/evalnomicon/drafts/eval/report.md\n?? crates/ploke-egui/tmp.txt\n",
        );

        assert_eq!(
            paths,
            vec![
                "docs/workflow/evalnomicon/drafts/eval/report.md",
                "crates/ploke-egui/tmp.txt"
            ]
        );
        assert!(!is_benchmark_relevant_dirty_path(&paths[0]));
        assert!(is_benchmark_relevant_dirty_path(&paths[1]));
    }

    #[test]
    fn benchmark_controller_transitions_and_autoclose_report_gate() {
        let config = BenchmarkConfig {
            suite: BenchmarkSuite::Standard,
            run_root: PathBuf::from(STANDARD_RUN_ROOT),
            output_dir: std::env::temp_dir().join("ploke-egui-benchmark-controller-fixture"),
            scenarios: vec![BenchmarkScenario::StartupFrames300],
            command: "fixture".to_owned(),
        };
        let mut controller = BenchmarkController::new(config, StartupProfile::default());
        assert_eq!(controller.begin_frame(), Some(BenchmarkAction::None));
        controller.record_action(BenchmarkActionReport::for_action(BenchmarkAction::None));
        for _ in 0..(STANDARD_FRAME_TARGET - 1) {
            assert!(controller.end_frame().expect("frame").is_none());
            assert_eq!(controller.begin_frame(), None);
        }
        assert!(controller.end_frame().expect("last frame").is_some());
        assert!(controller.finished);
        let _ = fs::remove_dir_all(
            std::env::temp_dir().join("ploke-egui-benchmark-controller-fixture"),
        );
    }
}
