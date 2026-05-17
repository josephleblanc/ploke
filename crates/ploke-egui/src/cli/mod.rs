//! Native command-line parsing for the operator UI.
//!
//! This module only parses startup options. Runtime graph loading, UI state, and
//! diagnostic interpretation live in the surrounding library modules.

pub mod report;

use std::path::PathBuf;

#[cfg(feature = "dev")]
use clap::Parser;

use crate::ui::view::GraphViewMode;

#[derive(Debug)]
pub struct Run {
    pub run_root: Option<PathBuf>,
    pub mode: GraphViewMode,
    pub snapshot: bool,
    pub contract_report: bool,
    pub run_picker_report: bool,
    pub artifact_connectivity_report: bool,
    #[cfg(feature = "dev")]
    pub perf_log: bool,
    #[cfg(feature = "dev")]
    pub puffin_capture_frames: Option<usize>,
    #[cfg(feature = "dev")]
    pub puffin_capture_close: bool,
    #[cfg(feature = "dev")]
    pub artifact_edges_report: bool,
    #[cfg(feature = "dev")]
    pub inspect_node: Option<String>,
    #[cfg(feature = "dev")]
    pub artifact_ids_report: Option<String>,
    #[cfg(feature = "dev")]
    pub benchmark_suite: Option<String>,
    #[cfg(feature = "dev")]
    pub benchmark_scenarios: Vec<String>,
    #[cfg(feature = "dev")]
    pub benchmark_output: Option<PathBuf>,
}

impl Run {
    pub fn from_env() -> Self {
        #[cfg(feature = "dev")]
        {
            let args = Args::parse();
            return Self {
                run_root: args.run_root.or(args.positional_run_root),
                mode: args.mode.unwrap_or_default(),
                snapshot: args.snapshot,
                contract_report: args.contract_report,
                run_picker_report: args.run_picker_report,
                artifact_connectivity_report: args.artifact_connectivity_report,
                perf_log: args.perf_log,
                puffin_capture_frames: args.puffin_capture_frames,
                puffin_capture_close: args.puffin_capture_close,
                artifact_edges_report: args.artifact_edges_report,
                inspect_node: args.inspect_node,
                artifact_ids_report: args.artifact_ids_report,
                benchmark_suite: args.benchmark_suite,
                benchmark_scenarios: args.benchmark_scenario,
                benchmark_output: args.benchmark_output,
            };
        }

        #[cfg(not(feature = "dev"))]
        {
            Self {
                run_root: std::env::args_os().nth(1).map(PathBuf::from),
                mode: GraphViewMode::ArtifactTree,
                snapshot: false,
                contract_report: false,
                run_picker_report: false,
                artifact_connectivity_report: false,
            }
        }
    }
}

#[cfg(feature = "dev")]
#[derive(Debug, Parser)]
#[command(about = "Run the ploke operator graph UI")]
struct Args {
    #[arg(value_name = "RUN_ROOT")]
    positional_run_root: Option<PathBuf>,

    #[arg(long, value_name = "RUN_ROOT")]
    run_root: Option<PathBuf>,

    #[arg(long)]
    snapshot: bool,

    #[arg(long, help = "Print a text default-view contract report")]
    contract_report: bool,

    #[arg(long, help = "Print run-picker dropdown labels")]
    run_picker_report: bool,

    #[arg(long, help = "Print artifact-tree connectivity for discovered runs")]
    artifact_connectivity_report: bool,

    #[arg(long, help = "Write the rolling import/projection profiling log")]
    perf_log: bool,

    #[arg(
        long,
        value_name = "FRAMES",
        help = "Capture this many native frames to rolling Puffin files; requires profile-with-puffin"
    )]
    puffin_capture_frames: Option<usize>,

    #[arg(
        long,
        requires = "puffin_capture_frames",
        help = "Close the native window after writing a Puffin capture"
    )]
    puffin_capture_close: bool,

    #[arg(long, help = "Print artifact-tree component edges for the loaded run")]
    artifact_edges_report: bool,

    #[arg(
        long,
        value_name = "NODE",
        help = "Print the graph-resolved inspector for a visible node label like A1 or a graph key"
    )]
    inspect_node: Option<String>,

    #[arg(
        long,
        value_name = "NODE",
        help = "Print artifact-id section applicability for a visible node label like A1 or a graph key"
    )]
    artifact_ids_report: Option<String>,

    #[arg(
        long,
        value_name = "SUITE",
        requires = "run_root",
        help = "Run a native benchmark suite; standard requires explicit --run-root"
    )]
    benchmark_suite: Option<String>,

    #[arg(
        long,
        value_name = "NAME",
        requires = "benchmark_suite",
        help = "Filter benchmark scenarios; repeatable, defaults to all standard scenarios"
    )]
    benchmark_scenario: Vec<String>,

    #[arg(
        long,
        value_name = "DIR",
        requires = "benchmark_suite",
        help = "Write benchmark README.md and report.json to this directory"
    )]
    benchmark_output: Option<PathBuf>,

    #[arg(
        long,
        value_name = "MODE",
        value_parser = parse_mode,
        help = "Initial graph mode: artifact-tree, lineage, both, or none"
    )]
    mode: Option<GraphViewMode>,
}

#[cfg(feature = "dev")]
fn parse_mode(value: &str) -> Result<GraphViewMode, String> {
    match value {
        "artifact-tree" | "artifact" => Ok(GraphViewMode::ArtifactTree),
        "lineage" => Ok(GraphViewMode::Lineage),
        "artifact-and-lineage" | "both" => Ok(GraphViewMode::ArtifactAndLineage),
        "empty" | "none" => Ok(GraphViewMode::Empty),
        other => Err(format!(
            "unknown graph mode '{other}' (expected artifact-tree, lineage, both, or none)"
        )),
    }
}

#[cfg(all(test, feature = "dev"))]
mod tests {
    use super::*;

    #[test]
    fn benchmark_suite_requires_explicit_run_root() {
        let error = Args::try_parse_from(["ploke-egui", "--benchmark-suite", "standard"])
            .expect_err("benchmark requires explicit run-root");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
    }

    #[test]
    fn benchmark_flags_parse_with_explicit_run_root() {
        let args = Args::try_parse_from([
            "ploke-egui",
            "--run-root",
            "/tmp/prototype1",
            "--benchmark-suite",
            "standard",
            "--benchmark-scenario",
            "startup_frames_300",
            "--benchmark-scenario",
            "warm_idle_300",
            "--benchmark-output",
            "/tmp/report",
        ])
        .expect("parse benchmark flags");

        assert_eq!(args.benchmark_suite.as_deref(), Some("standard"));
        assert_eq!(
            args.benchmark_scenario,
            vec!["startup_frames_300".to_owned(), "warm_idle_300".to_owned()]
        );
        assert_eq!(args.benchmark_output, Some(PathBuf::from("/tmp/report")));
    }
}
