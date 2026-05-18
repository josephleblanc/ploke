//! Typed allocation-span breakdowns for native benchmark reports.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::allocation::{HeapGroupProfile, HeapProfileSnapshot, HeapProfileTotals};

use super::{BenchmarkReport, ScenarioReport};

const REPORT_FILE_NAME: &str = "report.json";
const SHORT_ROW_LIMIT: usize = 5;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BreakdownRenderOptions {
    pub row_limit: Option<usize>,
    pub fast_only: bool,
}

impl BreakdownRenderOptions {
    pub fn new(short: bool, fast_only: bool) -> Self {
        Self {
            row_limit: short.then_some(SHORT_ROW_LIMIT),
            fast_only,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkAllocationBreakdown {
    pub report_path: PathBuf,
    pub suite: String,
    pub run_root: String,
    pub scenarios: Vec<ScenarioAllocationBreakdown>,
}

impl BenchmarkAllocationBreakdown {
    pub fn load(path: Option<&Path>) -> Result<Self, Box<dyn Error>> {
        let report_path = resolve_report_path(path)?;
        let report: BenchmarkReport = super::read_typed_json(&report_path)?;
        Ok(Self::from_report(report_path, report))
    }

    pub fn from_report(report_path: PathBuf, report: BenchmarkReport) -> Self {
        let scenarios = report
            .scenarios
            .iter()
            .map(|scenario| {
                ScenarioAllocationBreakdown::from_report(&report_path, &report, scenario)
            })
            .collect();
        Self {
            report_path,
            suite: report.suite,
            run_root: report.run_root,
            scenarios,
        }
    }

    pub fn render_text(&self) -> String {
        self.render_text_with_options(BreakdownRenderOptions::default())
    }

    pub fn render_text_with_options(&self, options: BreakdownRenderOptions) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "benchmark allocation breakdown");
        let _ = writeln!(out, "report: {}", self.report_path.display());
        let _ = writeln!(out, "suite: {}", self.suite);
        let _ = writeln!(out, "run_root: {}", self.run_root);
        if let Some(limit) = options.row_limit {
            let _ = writeln!(out, "filter: top {limit} rows per scenario");
        }
        if options.fast_only {
            let _ = writeln!(out, "filter: 30-frame scenarios only");
        }
        let _ = writeln!(
            out,
            "note: scenario span rows use full heap artifacts when available; compact report groups are a fallback"
        );
        let _ = writeln!(
            out,
            "note: percentages are relative to each scenario's total tracked heap allocation"
        );

        for scenario in self.rendered_scenarios(options) {
            let _ = writeln!(out);
            render_scenario_breakdown(&mut out, scenario, options);
        }

        out
    }

    fn rendered_scenarios(
        &self,
        options: BreakdownRenderOptions,
    ) -> impl Iterator<Item = &ScenarioAllocationBreakdown> {
        self.scenarios
            .iter()
            .filter(move |scenario| !options.fast_only || scenario.is_fast_scenario())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkAllocationDelta {
    pub current: BenchmarkAllocationBreakdown,
    pub baseline: Option<BenchmarkAllocationBreakdown>,
}

impl BenchmarkAllocationDelta {
    pub fn load(path: Option<&Path>) -> Result<Self, Box<dyn Error>> {
        let current_path = resolve_report_path(path)?;
        let current_report: BenchmarkReport = super::read_typed_json(&current_path)?;
        let baseline_path = previous_matching_report_path(
            &current_path,
            &current_report.suite,
            &current_report.run_root,
        )?;
        let baseline = baseline_path
            .map(|path| {
                let report: BenchmarkReport = super::read_typed_json(&path)?;
                Ok::<_, Box<dyn Error>>(BenchmarkAllocationBreakdown::from_report(path, report))
            })
            .transpose()?;
        Ok(Self {
            current: BenchmarkAllocationBreakdown::from_report(current_path, current_report),
            baseline,
        })
    }

    pub fn render_text_with_options(&self, options: BreakdownRenderOptions) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "benchmark allocation delta");
        let _ = writeln!(out, "report: {}", self.current.report_path.display());
        match &self.baseline {
            Some(baseline) => {
                let _ = writeln!(out, "baseline: {}", baseline.report_path.display());
            }
            None => {
                let _ = writeln!(out, "baseline: missing");
            }
        }
        let _ = writeln!(out, "suite: {}", self.current.suite);
        let _ = writeln!(out, "run_root: {}", self.current.run_root);
        if let Some(limit) = options.row_limit {
            let _ = writeln!(out, "filter: top {limit} rows per scenario");
        }
        if options.fast_only {
            let _ = writeln!(out, "filter: 30-frame scenarios only");
        }
        let _ = writeln!(
            out,
            "note: rows match the spans that allocation breakdown would print for the selected report"
        );

        for scenario in self.current.rendered_scenarios(options) {
            let _ = writeln!(out);
            self.render_scenario_delta(&mut out, scenario, options);
        }

        out
    }

    fn render_scenario_delta(
        &self,
        out: &mut String,
        current: &ScenarioAllocationBreakdown,
        options: BreakdownRenderOptions,
    ) {
        let baseline = self
            .baseline
            .as_ref()
            .and_then(|baseline| baseline.scenario(&current.name));
        let _ = writeln!(
            out,
            "scenario: {} (frames={}, source={})",
            current.name,
            current.frames,
            current.source.render()
        );
        let Some(baseline) = baseline else {
            let _ = writeln!(out, "  baseline: missing");
            for span in current.rendered_spans(options) {
                let _ = writeln!(out, "  {}: missing", span.display_name());
            }
            return;
        };

        let _ = writeln!(
            out,
            "  totals delta: allocs={} object_bytes={} wrapped_bytes={} live_object_bytes={} live_wrapped_bytes={}",
            render_delta(
                current.totals.allocation_count,
                baseline.totals.allocation_count
            ),
            render_delta(
                current.totals.allocated_object_bytes,
                baseline.totals.allocated_object_bytes
            ),
            render_delta(
                current.totals.allocated_wrapped_bytes,
                baseline.totals.allocated_wrapped_bytes
            ),
            render_delta(
                current.totals.live_object_bytes,
                baseline.totals.live_object_bytes
            ),
            render_delta(
                current.totals.live_wrapped_bytes,
                baseline.totals.live_wrapped_bytes
            )
        );
        let _ = writeln!(
            out,
            "  per-frame median delta: allocs={} object_bytes={} wrapped_bytes={} live_object_bytes={}",
            render_optional_delta(
                current.median_allocation_count,
                baseline.median_allocation_count
            ),
            render_optional_delta(current.median_object_bytes, baseline.median_object_bytes),
            render_optional_delta(current.median_wrapped_bytes, baseline.median_wrapped_bytes),
            render_optional_delta(
                current.median_live_object_bytes,
                baseline.median_live_object_bytes
            )
        );
        let baseline_spans = baseline.span_map();
        let _ = writeln!(
            out,
            "  {:<44} {:>12} {:>16} {:>14} {:>16} {:>14} {:>13}",
            "span",
            "allocs_delta",
            "object_bytes_delta",
            "object/frame",
            "wrapped_bytes_delta",
            "wrapped/frame",
            "live_object"
        );
        for span in current.rendered_spans(options) {
            let name = span.display_name();
            let Some(baseline_span) = baseline_spans.get(name.as_str()) else {
                let _ = writeln!(
                    out,
                    "  {:<44} {:>12} {:>16} {:>14} {:>16} {:>14} {:>13}",
                    name, "missing", "missing", "missing", "missing", "missing", "missing"
                );
                continue;
            };
            let _ = writeln!(
                out,
                "  {:<44} {:>12} {:>16} {:>14} {:>16} {:>14} {:>13}",
                name,
                render_delta(
                    span.totals.allocation_count,
                    baseline_span.totals.allocation_count
                ),
                render_delta(
                    span.totals.allocated_object_bytes,
                    baseline_span.totals.allocated_object_bytes
                ),
                render_rate_delta(
                    span.totals.allocated_object_bytes,
                    current.frames,
                    baseline_span.totals.allocated_object_bytes,
                    baseline.frames
                ),
                render_delta(
                    span.totals.allocated_wrapped_bytes,
                    baseline_span.totals.allocated_wrapped_bytes
                ),
                render_rate_delta(
                    span.totals.allocated_wrapped_bytes,
                    current.frames,
                    baseline_span.totals.allocated_wrapped_bytes,
                    baseline.frames
                ),
                render_delta(
                    span.totals.live_object_bytes,
                    baseline_span.totals.live_object_bytes
                )
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioAllocationBreakdown {
    pub name: String,
    pub frames: usize,
    pub totals: HeapProfileTotals,
    pub median_allocation_count: Option<u64>,
    pub median_object_bytes: Option<u64>,
    pub median_wrapped_bytes: Option<u64>,
    pub median_live_object_bytes: Option<u64>,
    pub source: ScenarioAllocationSource,
    pub spans: Vec<SpanAllocationBreakdown>,
}

impl ScenarioAllocationBreakdown {
    fn from_report(
        report_path: &Path,
        report: &BenchmarkReport,
        scenario: &ScenarioReport,
    ) -> Self {
        let (source, totals, mut spans) =
            match heap_snapshot_for_scenario(report_path, report, scenario) {
                Some((path, snapshot)) => (
                    ScenarioAllocationSource::FullHeapArtifact(path),
                    snapshot.totals,
                    snapshot.groups,
                ),
                None => (
                    ScenarioAllocationSource::CompactReport,
                    scenario.heap_profile.totals,
                    compact_report_groups(scenario),
                ),
            };

        spans.sort_by(|left, right| {
            right
                .totals
                .allocated_object_bytes
                .cmp(&left.totals.allocated_object_bytes)
                .then_with(|| {
                    right
                        .totals
                        .allocated_wrapped_bytes
                        .cmp(&left.totals.allocated_wrapped_bytes)
                })
                .then_with(|| left.group_id.cmp(&right.group_id))
        });

        Self {
            name: scenario.name.clone(),
            frames: scenario.frame_stats.count.max(scenario.target_frames),
            totals,
            median_allocation_count: scenario.allocation_frames.per_frame.allocation_count.median,
            median_object_bytes: scenario
                .allocation_frames
                .per_frame
                .allocated_object_bytes
                .median,
            median_wrapped_bytes: scenario
                .allocation_frames
                .per_frame
                .allocated_wrapped_bytes
                .median,
            median_live_object_bytes: scenario
                .allocation_frames
                .per_frame
                .live_object_bytes
                .median,
            source,
            spans: spans
                .into_iter()
                .map(SpanAllocationBreakdown::from_group)
                .collect(),
        }
    }

    fn is_fast_scenario(&self) -> bool {
        self.name.ends_with("_30") || self.name.ends_with("_alternate_30")
    }

    fn rendered_spans(
        &self,
        options: BreakdownRenderOptions,
    ) -> impl Iterator<Item = &SpanAllocationBreakdown> {
        self.spans
            .iter()
            .take(options.row_limit.unwrap_or(usize::MAX))
    }

    fn span_map(&self) -> BTreeMap<String, &SpanAllocationBreakdown> {
        self.spans
            .iter()
            .map(|span| (span.display_name(), span))
            .collect()
    }
}

impl BenchmarkAllocationBreakdown {
    fn scenario(&self, name: &str) -> Option<&ScenarioAllocationBreakdown> {
        self.scenarios.iter().find(|scenario| scenario.name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioAllocationSource {
    FullHeapArtifact(PathBuf),
    CompactReport,
}

impl ScenarioAllocationSource {
    fn render(&self) -> String {
        match self {
            Self::FullHeapArtifact(path) => format!("full heap artifact {}", path.display()),
            Self::CompactReport => "compact report top groups".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpanAllocationBreakdown {
    pub group_id: usize,
    pub name: Option<String>,
    pub totals: HeapProfileTotals,
}

impl SpanAllocationBreakdown {
    fn from_group(group: HeapGroupProfile) -> Self {
        Self {
            group_id: group.group_id,
            name: group.name,
            totals: group.totals,
        }
    }

    fn display_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("group#{}", self.group_id))
    }
}

fn resolve_report_path(path: Option<&Path>) -> Result<PathBuf, Box<dyn Error>> {
    match path {
        Some(path) if path.is_dir() => {
            let report_path = path.join(REPORT_FILE_NAME);
            ensure_report_file(report_path)
        }
        Some(path) => ensure_report_file(path.to_path_buf()),
        None => latest_benchmark_report_path().map_err(Into::into),
    }
}

fn ensure_report_file(path: PathBuf) -> Result<PathBuf, Box<dyn Error>> {
    if path.is_file() {
        Ok(path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("benchmark report not found at {}", path.display()),
        )
        .into())
    }
}

fn latest_benchmark_report_path() -> io::Result<PathBuf> {
    latest_benchmark_report_path_in(&benchmark_reports_root())
}

fn latest_benchmark_report_path_in(root: &Path) -> io::Result<PathBuf> {
    let mut reports = benchmark_report_paths_in(root)?;
    reports.sort_by(|left, right| right.0.cmp(&left.0));
    reports
        .into_iter()
        .next()
        .map(|(_, path)| path)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "no benchmark report.json files found under {}",
                    root.display()
                ),
            )
        })
}

fn previous_matching_report_path(
    current_path: &Path,
    suite: &str,
    run_root: &str,
) -> Result<Option<PathBuf>, Box<dyn Error>> {
    previous_matching_report_path_in(current_path, suite, run_root, &benchmark_reports_root())
}

fn previous_matching_report_path_in(
    current_path: &Path,
    suite: &str,
    run_root: &str,
    root: &Path,
) -> Result<Option<PathBuf>, Box<dyn Error>> {
    let current_path = normalize_path(current_path);
    let current_modified = fs::metadata(&current_path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::now());
    let mut reports = benchmark_report_paths_in(root)?;
    reports.sort_by(|left, right| right.0.cmp(&left.0));

    for (modified, path) in reports {
        let path = normalize_path(&path);
        if path == current_path || modified >= current_modified {
            continue;
        }
        let Ok(report) = super::read_typed_json::<BenchmarkReport>(&path) else {
            continue;
        };
        if report.suite == suite && report.run_root == run_root {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn benchmark_report_paths_in(root: &Path) -> io::Result<Vec<(SystemTime, PathBuf)>> {
    let mut reports = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let report_path = if path.is_dir() {
            path.join(REPORT_FILE_NAME)
        } else if path.file_name().and_then(|name| name.to_str()) == Some(REPORT_FILE_NAME) {
            path
        } else {
            continue;
        };
        if !report_path.is_file() {
            continue;
        }
        let modified = fs::metadata(&report_path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        reports.push((modified, report_path));
    }
    Ok(reports)
}

fn normalize_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn benchmark_reports_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("profiling")
        .join("benchmarks")
}

fn heap_snapshot_for_scenario(
    report_path: &Path,
    report: &BenchmarkReport,
    scenario: &ScenarioReport,
) -> Option<(PathBuf, HeapProfileSnapshot)> {
    let expected_file_name = format!("{}.heap.json", scenario.name);
    report
        .heap_artifacts
        .iter()
        .filter_map(|artifact| {
            let path = resolve_artifact_path(report_path, &artifact.path);
            let matches = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == expected_file_name);
            matches.then_some(path)
        })
        .find_map(|path| {
            let snapshot = super::read_typed_json::<HeapProfileSnapshot>(&path).ok()?;
            Some((path, snapshot))
        })
}

fn resolve_artifact_path(report_path: &Path, artifact_path: &str) -> PathBuf {
    let path = PathBuf::from(artifact_path);
    if path.is_absolute() {
        return path;
    }

    let workspace_relative = super::workspace_root().join(&path);
    if workspace_relative.exists() {
        return workspace_relative;
    }

    report_path
        .parent()
        .map(|parent| parent.join(&path))
        .unwrap_or(path)
}

fn compact_report_groups(scenario: &ScenarioReport) -> Vec<HeapGroupProfile> {
    let mut groups = BTreeMap::<usize, HeapGroupProfile>::new();
    for group in scenario
        .heap_profile
        .top_groups_by_allocated_bytes
        .iter()
        .chain(&scenario.heap_profile.top_groups_by_allocation_count)
        .chain(&scenario.heap_profile.top_groups_by_retained_bytes)
    {
        groups
            .entry(group.group_id)
            .or_insert_with(|| group.clone());
    }
    groups.into_values().collect()
}

fn render_scenario_breakdown(
    out: &mut String,
    scenario: &ScenarioAllocationBreakdown,
    options: BreakdownRenderOptions,
) {
    let _ = writeln!(
        out,
        "scenario: {} (frames={}, source={})",
        scenario.name,
        scenario.frames,
        scenario.source.render()
    );
    let _ = writeln!(
        out,
        "  totals: allocs={} object_bytes={} wrapped_bytes={} live_object_bytes={} live_wrapped_bytes={}",
        scenario.totals.allocation_count,
        scenario.totals.allocated_object_bytes,
        scenario.totals.allocated_wrapped_bytes,
        scenario.totals.live_object_bytes,
        scenario.totals.live_wrapped_bytes
    );
    let _ = writeln!(
        out,
        "  per-frame median: allocs={} object_bytes={} wrapped_bytes={} live_object_bytes={}",
        render_optional(scenario.median_allocation_count),
        render_optional(scenario.median_object_bytes),
        render_optional(scenario.median_wrapped_bytes),
        render_optional(scenario.median_live_object_bytes)
    );
    let _ = writeln!(
        out,
        "  per-frame mean totals: allocs={} object_bytes={} wrapped_bytes={}",
        render_rate(scenario.totals.allocation_count, scenario.frames),
        render_rate(scenario.totals.allocated_object_bytes, scenario.frames),
        render_rate(scenario.totals.allocated_wrapped_bytes, scenario.frames)
    );
    let _ = writeln!(
        out,
        "  {:<44} {:>10} {:>8} {:>14} {:>8} {:>13} {:>14} {:>8} {:>13} {:>13}",
        "span",
        "allocs",
        "alloc%",
        "object_bytes",
        "obj%",
        "object/frame",
        "wrapped_bytes",
        "wrap%",
        "wrapped/frame",
        "live_object"
    );
    for span in scenario.rendered_spans(options) {
        let _ = writeln!(
            out,
            "  {:<44} {:>10} {:>8} {:>14} {:>8} {:>13} {:>14} {:>8} {:>13} {:>13}",
            span.display_name(),
            span.totals.allocation_count,
            render_percent(
                span.totals.allocation_count,
                scenario.totals.allocation_count
            ),
            span.totals.allocated_object_bytes,
            render_percent(
                span.totals.allocated_object_bytes,
                scenario.totals.allocated_object_bytes
            ),
            render_rate(span.totals.allocated_object_bytes, scenario.frames),
            span.totals.allocated_wrapped_bytes,
            render_percent(
                span.totals.allocated_wrapped_bytes,
                scenario.totals.allocated_wrapped_bytes
            ),
            render_rate(span.totals.allocated_wrapped_bytes, scenario.frames),
            span.totals.live_object_bytes
        );
    }
    if scenario.spans.is_empty() {
        let _ = writeln!(out, "  none");
    }
}

fn render_optional(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_owned())
}

fn render_percent(value: u64, total: u64) -> String {
    if total == 0 {
        return "n/a".to_owned();
    }
    format!("{:.2}%", value as f64 * 100.0 / total as f64)
}

fn render_rate(value: u64, frames: usize) -> String {
    if frames == 0 {
        return "n/a".to_owned();
    }
    format!("{:.1}", value as f64 / frames as f64)
}

fn render_delta(current: u64, baseline: u64) -> String {
    let delta = current as i128 - baseline as i128;
    render_signed(delta)
}

fn render_optional_delta(current: Option<u64>, baseline: Option<u64>) -> String {
    match (current, baseline) {
        (Some(current), Some(baseline)) => render_delta(current, baseline),
        _ => "missing".to_owned(),
    }
}

fn render_rate_delta(
    current_value: u64,
    current_frames: usize,
    baseline_value: u64,
    baseline_frames: usize,
) -> String {
    if current_frames == 0 || baseline_frames == 0 {
        return "n/a".to_owned();
    }
    let current = current_value as f64 / current_frames as f64;
    let baseline = baseline_value as f64 / baseline_frames as f64;
    let delta = current - baseline;
    if delta >= 0.0 {
        format!("+{delta:.1}")
    } else {
        format!("{delta:.1}")
    }
}

fn render_signed(delta: i128) -> String {
    if delta >= 0 {
        format!("+{delta}")
    } else {
        delta.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocation::AllocationSnapshot;
    use crate::benchmark::{
        AllocationFrameReport, AllocationFrameStats, BenchmarkActionReport, BenchmarkArtifact,
        BenchmarkSpan, ComponentTimingReport, DirtyState, DirtyStateClassification, DurationStats,
        GitInfo, HeapScenarioProfile, StartupProfile, ValueStats,
    };

    #[test]
    fn allocation_breakdown_uses_full_heap_artifact() {
        let root = test_root("allocation-breakdown-full-heap");
        let report_dir = root.join("bench");
        let heap_dir = root.join("heap");
        fs::create_dir_all(&report_dir).expect("report dir");
        fs::create_dir_all(&heap_dir).expect("heap dir");
        let heap_path = heap_dir.join("warm_idle_300.heap.json");
        let heap = HeapProfileSnapshot {
            enabled: true,
            totals: HeapProfileTotals {
                allocation_count: 12,
                allocated_object_bytes: 1200,
                allocated_wrapped_bytes: 1500,
                live_object_bytes: 30,
                live_wrapped_bytes: 40,
                ..HeapProfileTotals::default()
            },
            groups: vec![
                heap_group(2, "root_ui_thread", 6, 900, 1000, 10),
                heap_group(10, "selection_inspector", 6, 300, 500, 20),
            ],
            callsites: Vec::new(),
            unmatched_deallocations: 0,
        };
        fs::write(
            &heap_path,
            serde_json::to_vec_pretty(&heap).expect("heap json"),
        )
        .expect("write heap");

        let report = benchmark_report_fixture(heap_path.display().to_string());
        let report_path = report_dir.join(REPORT_FILE_NAME);
        fs::write(
            &report_path,
            serde_json::to_vec_pretty(&report).expect("report json"),
        )
        .expect("write report");

        let breakdown = BenchmarkAllocationBreakdown::load(Some(&report_path)).expect("load");
        assert_eq!(breakdown.scenarios.len(), 1);
        let scenario = &breakdown.scenarios[0];
        assert!(matches!(
            scenario.source,
            ScenarioAllocationSource::FullHeapArtifact(_)
        ));
        assert_eq!(scenario.spans.len(), 2);
        assert_eq!(scenario.spans[0].name.as_deref(), Some("root_ui_thread"));

        let rendered = breakdown.render_text();
        assert!(rendered.contains("object_bytes=1200"));
        assert!(rendered.contains("75.00%"));
        assert!(rendered.contains("100.0"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn allocation_breakdown_accepts_report_directory() {
        let root = test_root("allocation-breakdown-report-directory");
        fs::create_dir_all(&root).expect("root");
        let report_path = root.join(REPORT_FILE_NAME);
        let report = benchmark_report_fixture(root.join("missing.heap.json").display().to_string());
        fs::write(
            &report_path,
            serde_json::to_vec_pretty(&report).expect("report json"),
        )
        .expect("write report");

        let breakdown = BenchmarkAllocationBreakdown::load(Some(&root)).expect("load dir");
        assert!(matches!(
            breakdown.scenarios[0].source,
            ScenarioAllocationSource::CompactReport
        ));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn allocation_breakdown_filters_fast_scenarios_and_short_rows() {
        let breakdown = BenchmarkAllocationBreakdown {
            report_path: PathBuf::from("/tmp/current/report.json"),
            suite: "standard".to_owned(),
            run_root: "/tmp/prototype1".to_owned(),
            scenarios: vec![
                scenario_breakdown(
                    "warm_idle_300",
                    300,
                    vec![heap_group(1, "warm_only", 1, 10, 10, 1)],
                ),
                scenario_breakdown(
                    "inspector_llm_calls_phase_sequence_30",
                    30,
                    (1..=6)
                        .map(|index| {
                            heap_group(
                                index,
                                &format!("fast_span_{index}"),
                                index as u64,
                                100 - index as u64,
                                120 - index as u64,
                                index as u64,
                            )
                        })
                        .collect(),
                ),
            ],
        };

        let rendered = breakdown.render_text_with_options(BreakdownRenderOptions::new(true, true));
        assert!(!rendered.contains("warm_idle_300"));
        assert!(rendered.contains("inspector_llm_calls_phase_sequence_30"));
        assert!(rendered.contains("fast_span_1"));
        assert!(rendered.contains("fast_span_5"));
        assert!(!rendered.contains("fast_span_6"));
    }

    #[test]
    fn allocation_delta_renders_matching_deltas_and_missing_rows() {
        let current = BenchmarkAllocationBreakdown {
            report_path: PathBuf::from("/tmp/current/report.json"),
            suite: "standard".to_owned(),
            run_root: "/tmp/prototype1".to_owned(),
            scenarios: vec![scenario_breakdown(
                "inspector_llm_calls_phase_sequence_30",
                30,
                vec![
                    heap_group(1, "shared_span", 10, 100, 120, 20),
                    heap_group(2, "new_span", 4, 40, 50, 8),
                ],
            )],
        };
        let baseline = BenchmarkAllocationBreakdown {
            report_path: PathBuf::from("/tmp/baseline/report.json"),
            suite: "standard".to_owned(),
            run_root: "/tmp/prototype1".to_owned(),
            scenarios: vec![scenario_breakdown(
                "inspector_llm_calls_phase_sequence_30",
                30,
                vec![heap_group(1, "shared_span", 7, 70, 90, 15)],
            )],
        };
        let delta = BenchmarkAllocationDelta {
            current,
            baseline: Some(baseline),
        };

        let rendered = delta.render_text_with_options(BreakdownRenderOptions::new(true, true));
        assert!(rendered.contains("allocs_delta"));
        assert!(rendered.contains("shared_span"));
        assert!(rendered.contains("+3"));
        assert!(rendered.contains("+30"));
        assert!(rendered.contains("new_span"));
        assert!(rendered.contains("missing"));
    }

    fn benchmark_report_fixture(heap_path: String) -> BenchmarkReport {
        BenchmarkReport {
            schema_version: "ploke-egui.native-benchmark-report.v3".to_owned(),
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
            command: "fixture".to_owned(),
            run_root: "/tmp/prototype1".to_owned(),
            feature_set: vec!["dev".to_owned(), "native-benchmark".to_owned()],
            scenarios_requested: vec!["warm_idle_300".to_owned()],
            callsite_sampling: None,
            run_readiness: None,
            startup: StartupProfile {
                spans: vec![BenchmarkSpan {
                    name: "Graph::from_records".to_owned(),
                    duration_ns: 42,
                }],
                compressed_run_records: Vec::new(),
                notes: Vec::new(),
            },
            scenarios: vec![scenario_fixture()],
            puffin_artifacts: Vec::new(),
            heap_artifacts: vec![BenchmarkArtifact {
                path: heap_path,
                bytes: 1,
                sha256: "00".to_owned(),
                tracked: false,
            }],
            notes: Vec::new(),
        }
    }

    fn scenario_breakdown(
        name: &str,
        frames: usize,
        groups: Vec<HeapGroupProfile>,
    ) -> ScenarioAllocationBreakdown {
        let totals = groups
            .iter()
            .fold(HeapProfileTotals::default(), |mut totals, group| {
                totals.allocation_count += group.totals.allocation_count;
                totals.allocated_object_bytes += group.totals.allocated_object_bytes;
                totals.allocated_wrapped_bytes += group.totals.allocated_wrapped_bytes;
                totals.live_object_bytes += group.totals.live_object_bytes;
                totals.live_wrapped_bytes += group.totals.live_wrapped_bytes;
                totals
            });
        ScenarioAllocationBreakdown {
            name: name.to_owned(),
            frames,
            totals,
            median_allocation_count: Some(1),
            median_object_bytes: Some(10),
            median_wrapped_bytes: Some(12),
            median_live_object_bytes: Some(2),
            source: ScenarioAllocationSource::CompactReport,
            spans: groups
                .into_iter()
                .map(SpanAllocationBreakdown::from_group)
                .collect(),
        }
    }

    fn scenario_fixture() -> ScenarioReport {
        ScenarioReport {
            name: "warm_idle_300".to_owned(),
            target_frames: 12,
            frame_stats: DurationStats {
                count: 12,
                min_ns: Some(1),
                median_ns: Some(1),
                p95_ns: Some(1),
                p99_ns: Some(1),
                max_ns: Some(1),
            },
            top_frames: Vec::new(),
            component_timings: Vec::<ComponentTimingReport>::new(),
            allocation_frames: AllocationFrameReport {
                per_frame: AllocationFrameStats {
                    allocation_count: ValueStats {
                        count: 12,
                        median: Some(1),
                        ..ValueStats::default()
                    },
                    allocated_object_bytes: ValueStats {
                        count: 12,
                        median: Some(100),
                        ..ValueStats::default()
                    },
                    allocated_wrapped_bytes: ValueStats {
                        count: 12,
                        median: Some(125),
                        ..ValueStats::default()
                    },
                    live_object_bytes: ValueStats {
                        count: 12,
                        median: Some(30),
                        ..ValueStats::default()
                    },
                    live_wrapped_bytes: ValueStats::default(),
                },
                ..AllocationFrameReport::default()
            },
            phase_windows: Vec::new(),
            heap_profile: HeapScenarioProfile {
                totals: HeapProfileTotals {
                    allocation_count: 12,
                    allocated_object_bytes: 1200,
                    allocated_wrapped_bytes: 1500,
                    live_object_bytes: 30,
                    live_wrapped_bytes: 40,
                    ..HeapProfileTotals::default()
                },
                top_groups_by_allocated_bytes: vec![heap_group(
                    2,
                    "root_ui_thread",
                    12,
                    1200,
                    1500,
                    30,
                )],
                ..HeapScenarioProfile::default()
            },
            allocations: AllocationSnapshot {
                enabled: true,
                allocation_count: 12,
                allocated_bytes: 1200,
                ..AllocationSnapshot::default()
            }
            .delta_since(AllocationSnapshot::default()),
            action: BenchmarkActionReport::for_action(crate::benchmark::BenchmarkAction::None),
        }
    }

    fn heap_group(
        group_id: usize,
        name: &str,
        allocation_count: u64,
        object_bytes: u64,
        wrapped_bytes: u64,
        live_object_bytes: u64,
    ) -> HeapGroupProfile {
        HeapGroupProfile {
            group_id,
            name: Some(name.to_owned()),
            totals: HeapProfileTotals {
                allocation_count,
                allocated_object_bytes: object_bytes,
                allocated_wrapped_bytes: wrapped_bytes,
                live_object_bytes,
                live_wrapped_bytes: live_object_bytes,
                ..HeapProfileTotals::default()
            },
        }
    }

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ploke-egui-{name}-{}", std::process::id()))
    }
}
