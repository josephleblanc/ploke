use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand};
use ploke_llm::request::models::ModelRouteSource;
use serde::Serialize;

use super::common::{InspectOutputFormat, parse_model_route_source};

#[derive(Debug, Parser)]
#[command(about = "Run higher-level loop wrappers over eval, protocol, and intervention stages")]
pub struct LoopCommand {
    #[command(subcommand)]
    pub command: LoopSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum LoopSubcommand {
    /// Run Prototype 1 through eval configuration, baseline arm, synthesis, treatment, and compare.
    Prototype1(Prototype1LoopCommand),
    /// Create a Prototype 1 campaign and admit the current checkout as Parent(0).
    Prototype1Setup(Prototype1LoopCommand),
    /// Diagnose the active Prototype 1 parent checkout and print the next exact commands.
    Prototype1Doctor(Prototype1DoctorCommand),
    /// Print the broad-harness prompt for the active Prototype 1 parent checkout.
    Prototype1Prompt(Prototype1PromptCommand),
    /// Resume the active Prototype 1 parent checkout until the current turn completes or hands off.
    Prototype1Continue(Prototype1ControlCommand),
    /// Advance exactly one diagnosed Prototype 1 parent phase.
    Prototype1Step(Prototype1ControlCommand),
    /// Drive the typed Prototype 1 parent runtime path.
    Prototype1State(Prototype1StateCommand),
    /// Inspect or execute one staged Prototype 1 runner invocation.
    #[command(hide = true)]
    Prototype1Runner(Prototype1RunnerCommand),
    /// Replay published broad headless-TUI harness requests outside the full parent loop.
    #[command(hide = true)]
    Prototype1Harness(Prototype1HarnessCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1StateStopAfter {
    Materialize,
    Build,
    Spawn,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1SuccessorSelection {
    GenerationLocal,
    HistoryFrontierMax,
    HistoryScoreChildProp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1TraversalMetrics {
    Operational,
    OperationalAndProtocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1CandidateGenerator {
    Legacy,
    BroadHarnessRequest,
    DeterministicTuiTools,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1EditSurface {
    PlokeTuiTools,
    WorkspaceExceptPlokeEval,
}

#[derive(Debug, Parser)]
#[command(about = "Run the typed Prototype 1 state transitions for the active parent checkout")]
pub struct Prototype1StateCommand {
    /// Campaign id. Defaults to parent identity, then active `select campaign`.
    #[arg(long)]
    pub campaign: Option<String>,

    /// Candidate node id to materialize/evaluate. During --init-parent-identity only, this is the generation-0 parent node.
    #[arg(long)]
    pub node_id: Option<String>,

    /// Parent checkout root. Defaults to the current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Bootstrap the active checkout by writing and committing parent identity.
    #[arg(long)]
    pub init_parent_identity: bool,

    /// Branch to create or switch to before writing initial parent identity.
    #[arg(long, value_name = "BRANCH", requires = "init_parent_identity")]
    pub identity_branch: Option<String>,

    /// Prepared instance id for the generation-0 parent identity.
    #[arg(long, value_name = "INSTANCE", requires = "init_parent_identity")]
    pub identity_instance: Option<String>,

    /// Successor handoff token written by the previous parent runtime.
    #[arg(long, value_name = "PATH")]
    pub handoff_invocation: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = Prototype1StateStopAfter::Complete)]
    pub stop_after: Prototype1StateStopAfter,

    /// Successor-selection strategy. Active selection defaults to History traversal with current-generation candidates appended before scoring.
    #[arg(long, value_enum, default_value_t = Prototype1SuccessorSelection::HistoryScoreChildProp)]
    pub successor_selection: Prototype1SuccessorSelection,

    /// Replay seed committed by History-backed traversal selection.
    #[arg(long, default_value_t = 0)]
    pub successor_selection_seed: u64,

    /// Metric-bearing states used by History-backed traversal scoring.
    #[arg(long, value_enum, default_value_t = Prototype1TraversalMetrics::Operational)]
    pub successor_selection_metrics: Prototype1TraversalMetrics,

    /// Candidate generator used before publishing the child plan.
    #[arg(long, value_enum, default_value_t = Prototype1CandidateGenerator::BroadHarnessRequest)]
    pub candidate_generator: Prototype1CandidateGenerator,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Clone, Parser)]
#[command(about = "Diagnose or control the active Prototype 1 parent checkout")]
pub struct Prototype1ControlCommand {
    /// Active parent checkout root. Defaults to the current directory.
    ///
    /// Use this to diagnose or control a parent checkout from another cwd. The
    /// path must contain `.ploke/prototype1/parent_identity.json`.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Clone, Parser)]
#[command(about = "Diagnose the active Prototype 1 parent checkout")]
pub struct Prototype1DoctorCommand {
    #[command(flatten)]
    pub control: Prototype1ControlCommand,

    /// Run a tiny live protocol JSON request using the admitted model/provider/reasoning policy.
    #[arg(long)]
    pub live_protocol_preflight: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(about = "Print the broad-harness prompt for the active Prototype 1 parent checkout")]
pub struct Prototype1PromptCommand {
    /// Active parent checkout root. Defaults to the current directory.
    ///
    /// Use this to print the prompt for a parent checkout from another cwd. The
    /// path must contain `.ploke/prototype1/parent_identity.json`.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,
}

#[derive(Debug, Parser)]
#[command(
    about = "Inspect one Prototype 1 runner invocation or execute one persisted child runtime"
)]
pub struct Prototype1RunnerCommand {
    #[arg(long)]
    pub campaign: Option<String>,

    #[arg(long)]
    pub node_id: Option<String>,

    #[arg(long, value_name = "PATH")]
    pub invocation: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub execute: bool,

    #[arg(long, action = ArgAction::Set, default_value_t = false)]
    pub stop_on_error: bool,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
#[command(about = "Run published Prototype 1 broad harness request slots")]
pub struct Prototype1HarnessCommand {
    #[command(subcommand)]
    pub command: Prototype1HarnessSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum Prototype1HarnessSubcommand {
    /// Run one published broad harness request slot.
    Attempt(Prototype1HarnessAttemptCommand),
    /// Run multiple distinct published broad harness request slots concurrently.
    Sweep(Prototype1HarnessSweepCommand),
}

#[derive(Debug, Parser)]
pub struct Prototype1HarnessAttemptCommand {
    /// Path to one published broad harness request JSON.
    #[arg(long, value_name = "PATH")]
    pub request: PathBuf,

    /// Model id for this attempt. Defaults to the headless TUI harness default.
    #[arg(long, value_name = "MODEL")]
    pub model_id: Option<String>,

    /// Provider slug to pin for the selected model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Override the published attempt count for fast live probes.
    #[arg(long)]
    pub max_attempts: Option<u32>,

    /// Override the published turn timeout for fast live probes.
    #[arg(long)]
    pub timeout_secs: Option<u64>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct Prototype1HarnessSweepCommand {
    /// Published broad harness request JSON. Repeat for multiple lanes.
    #[arg(long = "request", value_name = "PATH", action = ArgAction::Append)]
    pub requests: Vec<PathBuf>,

    /// Directory containing published broad harness request JSON files.
    #[arg(long, value_name = "DIR")]
    pub requests_dir: Option<PathBuf>,

    /// Model id for lanes. Pass one model for all requests, or one per request.
    #[arg(long = "model-id", value_name = "MODEL", action = ArgAction::Append)]
    pub model_ids: Vec<String>,

    /// Provider slug to pin for every selected model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Override the published attempt count for fast live probes.
    #[arg(long)]
    pub max_attempts: Option<u32>,

    /// Override the published turn timeout for fast live probes.
    #[arg(long)]
    pub timeout_secs: Option<u64>,

    /// Maximum concurrent lanes.
    #[arg(long, default_value_t = 2)]
    pub parallel: usize,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1LoopStopAfter {
    BaselineEval,
    BaselineProtocol,
    TargetSelection,
    InterventionApply,
    Compare,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Prototype1ChildScheduleMode {
    FullBatch,
    AdaptiveBatch,
}

#[derive(Debug, Parser)]
#[command(
    about = "Run the Prototype 1 loop through baseline, treatment, and compare",
    after_help = "\
Flow:
  eval configuration
    -> baseline arm (= eval run -> protocol run)
    -> target selection
    -> intervention apply
    -> treatment arm
    -> compare

Use either an existing prepared batch (--batch/--batch-id) or define the slice
inline with dataset selectors (--dataset or --dataset-key plus --instance/--specific/--all).
"
)]
pub struct Prototype1LoopCommand {
    /// Path to a prepared batch manifest. Defaults to ~/.ploke-eval/batches/<batch-id>/batch.json.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["batch_id", "dataset", "dataset_key"])]
    pub batch: Option<PathBuf>,

    /// Batch id, used to resolve ~/.ploke-eval/batches/<batch-id>/batch.json.
    #[arg(long, conflicts_with_all = ["batch", "dataset", "dataset_key"])]
    pub batch_id: Option<String>,

    /// Multi-SWE-bench dataset JSONL file.
    #[arg(long, value_name = "PATH", conflicts_with = "dataset_key")]
    pub dataset: Option<PathBuf>,

    /// Built-in dataset registry key, for example ripgrep.
    #[arg(long, conflicts_with = "dataset")]
    pub dataset_key: Option<String>,

    /// Prepare every instance in the dataset.
    #[arg(long, conflicts_with_all = ["instance", "specific"])]
    pub all: bool,

    /// Exact benchmark instance id to include. Repeat for multiple instances.
    #[arg(long)]
    pub instance: Vec<String>,

    /// Substring selector matched against task id, benchmark id, org, repo, and title.
    #[arg(long)]
    pub specific: Vec<String>,

    /// Stop selecting after this many matched instances.
    #[arg(long)]
    pub limit: Option<usize>,

    /// Stable identifier for the batch manifest directory when preparing inline.
    #[arg(long)]
    pub prepare_batch_id: Option<String>,

    /// Stable human campaign/profile id for Prototype 1 state.
    #[arg(long)]
    pub campaign: Option<String>,

    /// Prototype 1 run profile name or TOML path. Profile admission is owned by setup.
    #[arg(long, value_name = "NAME_OR_PATH")]
    pub profile: Option<String>,

    /// Root directory containing repo checkouts at <repo-cache>/<org>/<repo>.
    #[arg(long, value_name = "PATH")]
    pub repo_cache: Option<PathBuf>,

    /// Root directory where ploke-eval should create per-run directories.
    #[arg(long = "instances-root", alias = "runs-root", value_name = "PATH")]
    pub instances_root: Option<PathBuf>,

    /// Root directory where ploke-eval should create batch manifests and summaries.
    #[arg(long, value_name = "PATH")]
    pub batches_root: Option<PathBuf>,

    #[arg(long, default_value_t = 40)]
    pub max_turns: u32,

    #[arg(long, default_value_t = 200)]
    pub max_tool_calls: u32,

    #[arg(long, default_value_t = 1800)]
    pub wall_clock_secs: u32,

    /// Disable eval-only DB checkpoint/failure snapshots during indexing.
    #[arg(long = "no-index-debug-snapshots", action = ArgAction::SetFalse, default_value_t = true)]
    pub index_debug_snapshots: bool,

    /// Use the default model instead of the persisted active model selection.
    #[arg(long)]
    pub use_default_model: bool,

    /// Explicit model id to use for the baseline eval batch.
    #[arg(long)]
    pub model_id: Option<String>,

    /// Explicit provider slug to pin for the selected eval model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Explicit route source for the selected eval model.
    #[arg(long, value_name = "ROUTE", value_parser = parse_model_route_source)]
    pub route_source: Option<ModelRouteSource>,

    /// Explicit embedding model id to use for eval indexing/retrieval.
    #[arg(long)]
    pub embedding_model_id: Option<String>,

    /// Explicit provider slug to pin for the embedding model.
    #[arg(long, value_name = "PROVIDER")]
    pub embedding_provider: Option<String>,

    /// Stop the baseline batch after the first per-instance runner failure.
    #[arg(long)]
    pub stop_on_error: bool,

    /// Override the model id used for baseline protocol review.
    #[arg(long)]
    pub protocol_model_id: Option<String>,

    /// Override the provider slug used for baseline protocol review.
    #[arg(long, value_name = "PROVIDER")]
    pub protocol_provider: Option<String>,

    /// Override the route source used for baseline protocol review.
    #[arg(long, value_name = "ROUTE", value_parser = parse_model_route_source)]
    pub protocol_route_source: Option<ModelRouteSource>,

    /// Continue the loop from a previously synthesized/applied branch in another Prototype 1 campaign.
    #[arg(long, requires = "source_branch_id")]
    pub source_campaign: Option<String>,

    /// Branch id to materialize as the starting source content state for this loop generation.
    #[arg(long, requires = "source_campaign")]
    pub source_branch_id: Option<String>,

    /// Maximum generation index the search controller is allowed to continue to.
    #[arg(long, default_value_t = 1)]
    pub max_generations: u32,

    /// Maximum total staged nodes the search controller may create for this campaign.
    #[arg(long, default_value_t = 32)]
    pub max_total_nodes: u32,

    /// Minimum direct child candidates to evaluate before generation-level fallback selection.
    #[arg(long, default_value_t = 2)]
    pub min_children: u32,

    /// Maximum direct child candidates to evaluate for one parent generation.
    #[arg(long, default_value_t = 6)]
    pub max_children: u32,

    /// Child scheduling mode: run all planned children (`full-batch`) or run in `min-children` batches (`adaptive-batch`).
    #[arg(long, value_enum, default_value_t = Prototype1ChildScheduleMode::FullBatch)]
    pub child_schedule_mode: Prototype1ChildScheduleMode,

    /// Stop search continuation once a keep-worthy branch is found.
    #[arg(long)]
    pub stop_on_first_keep: bool,

    /// Require the selected next branch to have overall disposition=keep before continuation.
    #[arg(long, action = ArgAction::Set, default_value_t = true)]
    pub require_keep_for_continuation: bool,

    /// Permit the best rejected child to become the next exploration parent when no child is acceptable.
    #[arg(long, action = ArgAction::Set, default_value_t = true)]
    pub explore_from_rejected: bool,

    /// Stop the wrapper after the selected implemented stage.
    #[arg(long, value_enum, default_value_t = Prototype1LoopStopAfter::InterventionApply)]
    pub stop_after: Prototype1LoopStopAfter,

    /// Synthesize/select the intervention candidate, but do not overwrite the target file.
    #[arg(long)]
    pub dry_run: bool,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}
