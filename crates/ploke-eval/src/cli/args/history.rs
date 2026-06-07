use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::Serialize;

use super::common::InspectOutputFormat;

#[derive(Debug, Parser)]
#[command(about = "Inspect History-shaped projections and metrics from persisted evidence")]
pub struct HistoryCommand {
    #[arg(long, global = true)]
    pub campaign: Option<String>,

    #[arg(long, global = true, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    #[command(subcommand)]
    pub command: HistorySubcommand,
}

#[derive(Debug, Subcommand)]
pub enum HistorySubcommand {
    /// Print a read-only History-shaped preview from current campaign records.
    Preview(Prototype1HistoryPreviewCommand),
    /// Print read-only child evidence grouped from current campaign records.
    ChildEvidence(Prototype1ChildEvidenceCommand),
    /// Print read-only metric projections from current evidence.
    Metrics(Prototype1MetricsCommand),
    /// Print read-only score projections from current evidence.
    #[command(alias = "score")]
    Scores(Prototype1ScoreCommand),
    /// Print read-only score/selection review projections from current evidence.
    ScoreSelectionReview(Prototype1ScoreCommand),
    /// Print one sealed successor-selection decision and optional traversal replay.
    SelectionShow(Prototype1SelectionShowCommand),
    /// Print the canonical Prototype 1 evidence surface inventory.
    EvidenceInventory(Prototype1EvidenceInventoryCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum MetricSlice {
    Summary,
    Cohorts,
    Trajectory,
}

#[derive(Debug, Parser)]
pub struct Prototype1MetricsCommand {
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Maximum rows to print.
    #[arg(long, default_value_t = 20, value_parser = parse_metric_rows)]
    pub rows: usize,

    /// Restrict rows to one generation.
    #[arg(long)]
    pub generation: Option<u32>,

    /// Metrics view to print.
    #[arg(long, value_enum, default_value_t = MetricSlice::Summary)]
    pub view: MetricSlice,
}

fn parse_metric_rows(raw: &str) -> Result<usize, String> {
    let rows = raw
        .parse::<usize>()
        .map_err(|source| format!("invalid row count '{raw}': {source}"))?;
    if (1..=500).contains(&rows) {
        Ok(rows)
    } else {
        Err(format!("rows must be between 1 and 500, got {rows}"))
    }
}

#[derive(Debug, Parser)]
pub struct Prototype1HistoryPreviewCommand {
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct Prototype1ChildEvidenceCommand {
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct Prototype1EvidenceInventoryCommand {
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct Prototype1ScoreCommand {
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Maximum child score rows to print.
    #[arg(long, default_value_t = 20, value_parser = parse_metric_rows)]
    pub rows: usize,

    /// Restrict child scores to one generation.
    #[arg(long)]
    pub generation: Option<u32>,
}

#[derive(Debug, Parser)]
pub struct Prototype1SelectionShowCommand {
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Zero-based sealed selection decision row to inspect.
    #[arg(long, default_value_t = 0, value_name = "INDEX")]
    pub row: usize,

    /// Include deterministic HistoryScoreChildProp replay weights when available.
    #[arg(long)]
    pub replay: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1HistoryPlaybackGranularity {
    Coarse,
    Fine,
}

#[derive(Debug, Parser)]
pub struct Prototype1MonitorTimingCommand {
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Restrict output to one node id.
    #[arg(long)]
    pub node: Option<String>,

    /// How far to open the timing tree in table output.
    #[arg(long, value_enum, default_value_t = Depth::Node)]
    pub depth: Depth,

    /// Include evidence paths in table output.
    #[arg(long)]
    pub show_paths: bool,

    /// Poll and redraw the timing projection until interrupted or the loop reaches a terminal state.
    #[arg(long)]
    pub watch: bool,

    /// Print narrow fixed-width rows for phone/tmux viewing.
    #[arg(long, requires = "watch")]
    pub phone: bool,

    /// Polling interval in milliseconds for --watch.
    #[arg(long, default_value_t = 1000)]
    pub interval_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Depth {
    /// One compact row per node.
    Node,
    /// Expand each node into major phase and run summaries.
    Phase,
    /// Expand agent runs into turns.
    Turn,
    /// Expand turns into bounded response/tool call rows.
    Call,
}

#[derive(Debug, Parser)]
pub struct Prototype1MonitorPeekCommand {
    /// Maximum trailing lines to show per text file.
    #[arg(long, default_value_t = 20)]
    pub lines: usize,

    /// Maximum bytes to read per file.
    #[arg(long, default_value_t = 8192)]
    pub bytes: usize,
}

#[derive(Debug, Parser)]
pub struct Prototype1MonitorWatchCommand {
    /// Polling interval in milliseconds.
    #[arg(long, default_value_t = 50)]
    pub interval_ms: u64,

    /// Include a one-time initial snapshot of existing files.
    #[arg(long)]
    pub print_initial: bool,
}
