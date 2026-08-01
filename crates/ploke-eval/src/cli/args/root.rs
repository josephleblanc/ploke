use clap::{Parser, Subcommand};

use super::{
    CampaignCommand, ClosureCommand, ConversationsCommand, HistoryCommand, InspectCommand,
    JustCommand, LoopCommand, MbeCommand, ModelCommand, ProtocolCommand, RegistryCommand,
    RunCommand, SelectCommand, TranscriptCommand,
};

// ANCHOR: ploke_eval_cli_trust_order_help
const CLI_BEFORE_LONG_HELP: &str = "\
Minimal evaluation runner and artifact inspector for ploke.

Default home:
  PLOKE_EVAL_HOME    ~/.ploke-eval

Choose an operator path:
  one instance       run repo fetch -> run prepare instance -> run single agent
  flat shortcuts     just <favorite>
  family progress    campaign / closure
  inspect a run      transcript / conversations / inspect
  setup and models   doctor / model
  target inventory   registry
  active selectors   select

Trust order:
  per-run artifacts > campaign export-submissions > closure state > batch aggregate JSONL

Use `ploke-eval help <command>` for examples, artifact paths, and command-specific defaults.
";
// ANCHOR_END: ploke_eval_cli_trust_order_help

#[derive(Debug, Parser)]
#[command(
    name = "ploke-eval",
    about = "Run prepared ploke benchmark/eval instances",
    before_long_help = CLI_BEFORE_LONG_HELP,
    version = env!("CARGO_PKG_VERSION"),
    propagate_version = true
)]
pub struct Cli {
    /// Enable cross-crate execution debug logs in ~/.ploke-eval/logs.
    #[arg(long, global = true)]
    pub debug_tools: bool,

    #[command(subcommand)]
    pub command: Command,
}

// ANCHOR: ploke_eval_cli_command_families
#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(display_order = 10)]
    /// Traverse the eval execution tree: repo fetch, datasets, prepare, single, batch, replay.
    Run(RunCommand),
    #[command(display_order = 11)]
    /// Flat shortcuts for common eval commands that also exist under `run ...`.
    Just(JustCommand),
    #[command(display_order = 20)]
    /// Show and set model/provider defaults for eval runs.
    Model(ModelCommand),
    #[command(display_order = 30)]
    /// Print assistant messages from one resolved run.
    Transcript(TranscriptCommand),
    #[command(display_order = 31)]
    /// List conversation turns for a run.
    Conversations(ConversationsCommand),
    #[command(display_order = 32)]
    /// Inspect run artifacts, failures, tool calls, and stored snapshots.
    Inspect(InspectCommand),
    #[command(display_order = 33)]
    /// Inspect History-shaped projections and metrics from persisted evidence.
    History(HistoryCommand),
    #[command(display_order = 40)]
    /// Operate on named eval campaigns and export campaign-level submissions.
    Campaign(CampaignCommand),
    #[command(display_order = 41)]
    /// Show and advance campaign progress across eval and protocol work.
    Closure(ClosureCommand),
    #[command(display_order = 42)]
    /// Show and recompute the persisted target inventory.
    Registry(RegistryCommand),
    #[command(display_order = 43)]
    /// Persist and inspect the active operator selection context.
    Select(SelectCommand),
    #[command(display_order = 44)]
    /// Prepare MBE oracle configs and inspect MBE oracle reports.
    Mbe(MbeCommand),
    #[command(display_order = 50)]
    /// Check eval setup and point out likely configuration problems.
    Doctor,
    #[command(display_order = 51)]
    /// Review or adjudicate protocol artifacts from eval runs.
    Protocol(ProtocolCommand),
    #[command(display_order = 52)]
    /// Run the prototype intervention loop through the currently implemented frontier.
    Loop(LoopCommand),
}
// ANCHOR_END: ploke_eval_cli_command_families
