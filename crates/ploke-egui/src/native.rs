//! Native entry point for the operator graph UI.

use std::error::Error;
use std::path::PathBuf;

#[cfg(feature = "dev")]
use clap::Parser;

use crate::demo::sample_graph;
#[cfg(feature = "dev")]
use crate::diagnostics::SnapshotSink;
use crate::import::graph_from_run_root;
use crate::ui::app::OperatorApp;
use crate::ui::view::GraphViewMode;
use ploke_tree::Graph;

pub fn run() -> Result<(), Box<dyn Error>> {
    let run = Run::from_env();
    let options = eframe::NativeOptions::default();
    let graph = initial_graph(run.run_root)?;
    let app = app(graph, run.mode, run.snapshot)?;
    eframe::run_native("ploke-egui", options, Box::new(|_cc| Ok(Box::new(app))))?;
    Ok(())
}

fn initial_graph(run_root: Option<PathBuf>) -> Result<Graph, Box<dyn Error>> {
    let Some(run_root) = run_root else {
        return Ok(sample_graph());
    };

    Ok(graph_from_run_root(run_root)?)
}

fn app(graph: Graph, mode: GraphViewMode, snapshot: bool) -> Result<OperatorApp, Box<dyn Error>> {
    let app = OperatorApp::new(graph).with_mode(mode);
    #[cfg(feature = "dev")]
    {
        if !snapshot {
            return Ok(app);
        }
        return Ok(app.with_snapshot_sink(SnapshotSink::new(default_diagnostics_dir())?));
    }

    #[cfg(not(feature = "dev"))]
    {
        let _ = snapshot;
        Ok(app)
    }
}

#[derive(Debug)]
struct Run {
    run_root: Option<PathBuf>,
    mode: GraphViewMode,
    snapshot: bool,
}

impl Run {
    fn from_env() -> Self {
        #[cfg(feature = "dev")]
        {
            let args = Args::parse();
            return Self {
                run_root: args.run_root.or(args.positional_run_root),
                mode: args.mode.unwrap_or_default(),
                snapshot: args.snapshot,
            };
        }

        #[cfg(not(feature = "dev"))]
        {
            Self {
                run_root: std::env::args_os().nth(1).map(PathBuf::from),
                mode: GraphViewMode::ArtifactTree,
                snapshot: false,
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

#[cfg(feature = "dev")]
fn default_diagnostics_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/diagnostics")
}
