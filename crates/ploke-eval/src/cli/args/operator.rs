use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::selection::ActiveSelectionSlot;

use super::common::InspectOutputFormat;

#[derive(Debug, Parser)]
#[command(
    about = "List all agent conversation turns from a run record",
    after_help = "\
Example:

  cargo run -p ploke-eval -- conversations --instance BurntSushi__ripgrep-2209
  cargo run -p ploke-eval -- conversations --record ~/.ploke-eval/instances/BurntSushi__ripgrep-2209/runs/run-<timestamp>-<arm>-<suffix>/record.json.gz

Output includes turn number, timestamps, tool call count, and outcome for each turn.
"
)]
pub struct ConversationsCommand {
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = ConversationsOutputFormat::Table)]
    pub format: ConversationsOutputFormat,
}

#[derive(Debug, Parser)]
#[command(about = "Print assistant messages from one resolved run")]
pub struct TranscriptCommand {
    /// Benchmark instance id. Defaults to the selected instance when set.
    #[arg(long)]
    pub instance: Option<String>,
}

#[derive(Debug, Parser)]
#[command(about = "Persist and inspect the active operator selection context")]
pub struct SelectCommand {
    #[command(subcommand)]
    pub command: SelectSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum SelectSubcommand {
    /// Show the current active selection context and any scope conflicts.
    Status(SelectStatusCommand),
    /// Select the active campaign context.
    Campaign(SelectCampaignCommand),
    /// Select the active batch context.
    Batch(SelectBatchCommand),
    /// Select the active instance context. Clears any active attempt.
    Instance(SelectInstanceCommand),
    /// Select the active attempt number for the active instance.
    Attempt(SelectAttemptCommand),
    /// Unset one active selection scope.
    Unset(SelectUnsetCommand),
    /// Clear the entire active selection context.
    Clear(SelectClearCommand),
}

#[derive(Debug, Parser)]
pub struct SelectStatusCommand {
    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct SelectCampaignCommand {
    /// Stable campaign identifier.
    pub campaign: String,
}

#[derive(Debug, Parser)]
pub struct SelectBatchCommand {
    /// Stable batch identifier.
    pub batch: String,
}

#[derive(Debug, Parser)]
pub struct SelectInstanceCommand {
    /// Stable benchmark instance identifier.
    pub instance: String,
}

#[derive(Debug, Parser)]
pub struct SelectAttemptCommand {
    /// 1-based attempt number for the selected instance.
    pub attempt: u32,

    /// Override the active instance while setting the attempt.
    #[arg(long)]
    pub instance: Option<String>,
}

#[derive(Debug, Parser)]
pub struct SelectUnsetCommand {
    /// Which scope to unset.
    pub scope: ActiveSelectionSlot,
}

#[derive(Debug, Parser)]
pub struct SelectClearCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ConversationsOutputFormat {
    Table,
    Json,
}
