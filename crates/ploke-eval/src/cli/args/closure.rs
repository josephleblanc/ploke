use clap::{Parser, Subcommand};

use ploke_records::ids::CampaignId;

use super::campaign::CampaignOverrideArgs;
use super::common::InspectOutputFormat;

#[derive(Debug, Parser)]
#[command(
    about = "Track campaign progress across registry inventory, eval work, and protocol coverage",
    after_help = "\
Default closure state path:
  ~/.ploke-eval/campaigns/<campaign>/closure-state.json

Use:
  closure status --campaign <campaign>
    inspect current reduced campaign progress
  closure advance eval --campaign <campaign>
    produce missing eval work from campaign config
  closure advance protocol --campaign <campaign>
    produce missing protocol work from completed eval runs
"
)]
pub struct ClosureCommand {
    #[command(subcommand)]
    pub command: ClosureSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ClosureSubcommand {
    /// Recompute reduced campaign closure state from existing datasets and artifacts.
    Recompute(ClosureRecomputeCommand),
    /// Print the current reduced closure state for a campaign.
    Status(ClosureStatusCommand),
    /// Advance closure by producing missing eval or protocol artifacts from campaign config.
    Advance(ClosureAdvanceCommand),
}

#[derive(Debug, Parser)]
pub struct ClosureAdvanceCommand {
    #[command(subcommand)]
    pub command: ClosureAdvanceSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ClosureAdvanceSubcommand {
    /// Prepare and execute eval batches for rows whose eval state is still missing.
    Eval(ClosureAdvanceEvalCommand),
    /// Produce protocol artifacts for completed eval runs whose protocol state is not yet complete.
    Protocol(ClosureAdvanceProtocolCommand),
    /// Run eval advancement first, then protocol advancement.
    All(ClosureAdvanceAllCommand),
}

#[derive(Debug, Parser)]
pub struct ClosureRecomputeCommand {
    /// Stable campaign identifier, used under ~/.ploke-eval/campaigns/<campaign>.
    #[arg(long)]
    pub campaign: CampaignId,

    #[command(flatten)]
    pub overrides: CampaignOverrideArgs,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct ClosureStatusCommand {
    /// Stable campaign identifier, used under ~/.ploke-eval/campaigns/<campaign>.
    #[arg(long)]
    pub campaign: CampaignId,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct ClosureAdvanceEvalCommand {
    /// Stable campaign identifier, used under ~/.ploke-eval/campaigns/<campaign>.
    #[arg(long)]
    pub campaign: CampaignId,

    #[command(flatten)]
    pub overrides: CampaignOverrideArgs,

    /// Show the batches and instances that would be selected without executing them.
    #[arg(long)]
    pub dry_run: bool,

    /// Override the eval selection limit for this invocation.
    #[arg(long)]
    pub limit: Option<usize>,

    /// Stop eval advancement after the first batch failure.
    #[arg(long)]
    pub stop_on_error: bool,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct ClosureAdvanceProtocolCommand {
    /// Stable campaign identifier, used under ~/.ploke-eval/campaigns/<campaign>.
    #[arg(long)]
    pub campaign: CampaignId,

    #[command(flatten)]
    pub overrides: CampaignOverrideArgs,

    /// Show the selected runs and missing protocol units without executing them.
    #[arg(long)]
    pub dry_run: bool,

    /// Override the protocol selection limit for this invocation.
    #[arg(long)]
    pub limit_runs: Option<usize>,

    /// Override the maximum number of protocol runs processed concurrently.
    #[arg(long)]
    pub max_concurrency: Option<usize>,

    /// Stop protocol advancement after the first run-level failure.
    #[arg(long)]
    pub stop_on_error: bool,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct ClosureAdvanceAllCommand {
    /// Stable campaign identifier, used under ~/.ploke-eval/campaigns/<campaign>.
    #[arg(long)]
    pub campaign: CampaignId,

    #[command(flatten)]
    pub overrides: CampaignOverrideArgs,

    /// Show the selected eval batches and protocol runs without executing them.
    #[arg(long)]
    pub dry_run: bool,

    /// Override the eval selection limit for this invocation.
    #[arg(long)]
    pub eval_limit: Option<usize>,

    /// Override the protocol selection limit for this invocation.
    #[arg(long)]
    pub protocol_limit_runs: Option<usize>,

    /// Override the maximum number of protocol runs processed concurrently.
    #[arg(long)]
    pub protocol_max_concurrency: Option<usize>,

    /// Stop as soon as eval or protocol advancement hits a failure.
    #[arg(long)]
    pub stop_on_error: bool,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}
