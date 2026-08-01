use std::path::PathBuf;

use clap::{Parser, Subcommand};

use super::common::InspectOutputFormat;

#[derive(Debug, Parser)]
#[command(
    about = "Manage the persisted benchmark target inventory",
    after_help = "\
This registry is separate from the model registry.

Default path:
  ~/.ploke-eval/registries/multi-swe-bench-rust.json

Use:
  registry status
    inspect the current persisted inventory
  registry show --dataset sharkdp__fd
    list the concrete instance ids for one dataset family
  registry recompute
    rebuild the inventory from dataset sources
"
)]
pub struct RegistryCommand {
    #[command(subcommand)]
    pub command: RegistrySubcommand,
}
#[derive(Debug, Subcommand)]
pub enum RegistrySubcommand {
    /// Recompute the persisted target registry from dataset sources.
    Recompute(RegistryRecomputeCommand),
    /// Print the current persisted target registry.
    Status(RegistryStatusCommand),
    /// Show the concrete registry entries for one dataset family.
    Show(RegistryShowCommand),
}

#[derive(Debug, Parser)]
pub struct RegistryRecomputeCommand {
    /// Built-in dataset registry key. Repeat for multiple datasets.
    #[arg(long)]
    pub dataset_key: Vec<String>,

    /// Explicit dataset JSONL file. Repeat for multiple datasets.
    #[arg(long, value_name = "PATH")]
    pub dataset: Vec<PathBuf>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct RegistryStatusCommand {
    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct RegistryShowCommand {
    /// Exact dataset family label from `registry status`, for example sharkdp__fd.
    #[arg(long)]
    pub dataset: String,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}
