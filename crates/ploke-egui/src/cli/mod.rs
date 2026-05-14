//! Native command-line parsing for the operator UI.
//!
//! This module only parses startup options. Runtime graph loading, UI state, and
//! diagnostic interpretation live in the surrounding library modules.

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

    #[arg(
        long,
        help = "Print a text default-view contract report without opening the native UI"
    )]
    contract_report: bool,

    #[arg(
        long,
        help = "Print run-picker dropdown labels without opening the native UI"
    )]
    run_picker_report: bool,

    #[arg(
        long,
        help = "Print artifact-tree connectivity for all discovered runs without opening the native UI"
    )]
    artifact_connectivity_report: bool,

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
