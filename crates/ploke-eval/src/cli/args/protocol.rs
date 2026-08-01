use std::path::PathBuf;

use clap::{Parser, Subcommand};

use ploke_llm::request::models::ModelRouteSource;

use super::common::{InspectOutputFormat, parse_model_route_source};

#[derive(Debug, Parser)]
#[command(about = "Run bounded review/adjudication protocols over eval artifacts")]
pub struct ProtocolCommand {
    #[command(subcommand)]
    pub command: ProtocolSubcommand,
}
#[derive(Debug, Subcommand)]
pub enum ProtocolSubcommand {
    /// Show protocol eligibility, existing artifacts, missing steps, and the next command to run.
    Status(ProtocolStatusCommand),
    /// Advance the selected run through the next missing protocol step.
    Run(ProtocolRunCommand),
    /// Detect bounded intervention issue cases from one completed run and persist the result.
    IssueDetection(ProtocolIssueDetectionCommand),
    /// Review one indexed tool call using a bounded neighborhood, forked judgments, and merged assessment.
    ToolCallReview(ProtocolToolCallReviewCommand),
    /// Segment an ordered tool-call sequence into contiguous intent episodes.
    ToolCallIntentSegments(ProtocolToolCallIntentSegmentsCommand),
    /// Review one intent segment using the shared local-analysis packet over segmented trace state.
    ToolCallSegmentReview(ProtocolToolCallSegmentReviewCommand),
}

#[derive(Debug, Parser)]
pub struct ProtocolStatusCommand {
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct ProtocolRunCommand {
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Override the model id. Defaults to the current active eval model.
    #[arg(long)]
    pub model_id: Option<String>,

    /// Override the provider slug. Defaults to the persisted provider for the chosen model, if any.
    #[arg(long)]
    pub provider: Option<String>,

    /// Override the route source: openrouter or direct-google.
    #[arg(long, value_name = "ROUTE", value_parser = parse_model_route_source)]
    pub route_source: Option<ModelRouteSource>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct ProtocolIssueDetectionCommand {
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct ProtocolToolCallReviewCommand {
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Indexed tool call to review, matching `inspect tool-calls <INDEX>`.
    #[arg(value_name = "INDEX")]
    pub index: usize,

    /// Override the model id. Defaults to the current active eval model.
    #[arg(long)]
    pub model_id: Option<String>,

    /// Override the provider slug. Defaults to the persisted provider for the chosen model, if any.
    #[arg(long)]
    pub provider: Option<String>,

    /// Override the route source: openrouter or direct-google.
    #[arg(long, value_name = "ROUTE", value_parser = parse_model_route_source)]
    pub route_source: Option<ModelRouteSource>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct ProtocolToolCallIntentSegmentsCommand {
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Override the model id. Defaults to the current active eval model.
    #[arg(long)]
    pub model_id: Option<String>,

    /// Override the provider slug. Defaults to the persisted provider for the chosen model, if any.
    #[arg(long)]
    pub provider: Option<String>,

    /// Override the route source: openrouter or direct-google.
    #[arg(long, value_name = "ROUTE", value_parser = parse_model_route_source)]
    pub route_source: Option<ModelRouteSource>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct ProtocolToolCallSegmentReviewCommand {
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Segment index from `ploke-eval protocol tool-call-intent-segments`.
    #[arg(value_name = "SEGMENT")]
    pub segment_index: usize,

    /// Override the model id. Defaults to the current active eval model.
    #[arg(long)]
    pub model_id: Option<String>,

    /// Override the provider slug. Defaults to the persisted provider for the chosen model, if any.
    #[arg(long)]
    pub provider: Option<String>,

    /// Override the route source: openrouter or direct-google.
    #[arg(long, value_name = "ROUTE", value_parser = parse_model_route_source)]
    pub route_source: Option<ModelRouteSource>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}
