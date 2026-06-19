use std::path::PathBuf;

use ploke_records::ids::CampaignId;

use clap::{ArgAction, Parser, Subcommand};
use ploke_llm::request::models::ModelRouteSource;
use serde::{Deserialize, Serialize};

use super::common::{InspectOutputFormat, parse_model_route_source};
use crate::cli::prototype1_state::walk::phase::WalkPhase;

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
    /// Debug-only local server for stepping Prototype 1 typestate transitions.
    #[command(
        name = "walk",
        about = "Debug-step Prototype 1 typestate transitions through a local walk server",
        long_about = "Debug-step Prototype 1 typestate transitions through a local walk server.\n\nThe walk server is a local debugging harness over live Prototype 1 transition edges. It is not production loop authority. By default it uses the active walk context if one was set with `walk use`, otherwise the current directory, and a repo-hashed socket under the runtime directory.",
        after_help = "Common workflows:\n  Set context:       ploke-eval loop walk use /path/to/parent-worktree\n  Start live walk:   ploke-eval loop walk start\n  Inspect progress:  ploke-eval loop walk summary -v\n  Replay history:    ploke-eval loop walk replay --index 0\n  Move replay:       ploke-eval loop walk forward --steps 10 --tail 20\n  Live step:         ploke-eval loop walk step --until r6\n\nSafety notes:\n  replay/back/forward are read-only historical cursor commands.\n  step drives live typestate edges; long live edges require --watch.\n  R12 -> R13b successor handoff mutates checkout state and requires --allow git-changes.\n  branch-live writes only explicit provenance and requires --allow provenance-record."
    )]
    Prototype1StateWalk(Prototype1StateWalkCommand),
    /// Inspect or execute one staged Prototype 1 runner invocation.
    #[command(hide = true)]
    Prototype1Runner(Prototype1RunnerCommand),
    /// Replay published broad headless-TUI harness requests outside the full parent loop.
    #[command(hide = true)]
    Prototype1Harness(Prototype1HarnessCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1StateStopAfter {
    Materialize,
    Build,
    Spawn,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1SuccessorSelection {
    GenerationLocal,
    HistoryFrontierMax,
    HistoryScoreChildProp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1TraversalMetrics {
    Operational,
    OperationalAndProtocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
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
    pub campaign: Option<CampaignId>,

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

#[derive(Debug, Parser)]
#[command(about = "Debug-step Prototype 1 typestate transitions through a local walk server")]
pub struct Prototype1StateWalkCommand {
    #[command(subcommand)]
    pub command: Prototype1StateWalkSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum Prototype1StateWalkSubcommand {
    /// Run the local walk server on a Unix socket.
    Serve(Prototype1StateWalkServeCommand),
    /// Save the active parent checkout for later walk commands.
    Use(Prototype1StateWalkUseCommand),
    /// Start a new in-memory walk, defaulting to R0.
    Start(Prototype1StateWalkStartCommand),
    /// Advance the current in-memory walk by one step or until a target phase.
    Step(Prototype1StateWalkStepCommand),
    /// Reset the current in-memory walk without stopping the server.
    Reset(Prototype1StateWalkControlCommand),
    /// Print tracked output files for the current walk.
    Files(Prototype1StateWalkControlCommand),
    /// Show current in-memory walk state or the last step delta.
    Show(Prototype1StateWalkShowCommand),
    /// Inspect nested LLM/tool-loop debugger checkpoints.
    Llm(Prototype1StateWalkLlmCommand),
    /// Summarize durable campaign progress without contacting the walk server.
    Summary(Prototype1StateWalkSummaryCommand),
    /// Show or jump within the read-only historical replay cursor.
    Replay(Prototype1StateWalkReplayCommand),
    /// Move the read-only historical replay cursor backward without undoing side effects.
    Back(Prototype1StateWalkReplayMoveCommand),
    /// Move the read-only historical replay cursor forward.
    Forward(Prototype1StateWalkReplayMoveCommand),
    /// Record explicit provenance before branching from replay toward live work.
    BranchLive(Prototype1StateWalkBranchLiveCommand),
    /// Check whether the local walk server is alive.
    Status(Prototype1StateWalkControlCommand),
    /// Stop the local walk server.
    Stop(Prototype1StateWalkControlCommand),
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkServeCommand {
    /// Parent checkout root. Defaults to active walk context, then current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Explicit Unix socket path. Defaults to a repo-hashed path under the runtime directory.
    #[arg(long, value_name = "PATH")]
    pub socket: Option<PathBuf>,

    /// Idle seconds before the server exits. Defaults to 1800 seconds.
    #[arg(long, value_name = "SECS", conflicts_with = "no_ttl")]
    pub ttl_secs: Option<u64>,

    /// Keep the server alive until an explicit stop request.
    #[arg(long)]
    pub no_ttl: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Remember the parent checkout/socket used by later walk commands",
    after_help = "Examples:\n  ploke-eval loop walk use /path/to/parent-worktree\n  ploke-eval loop walk use /path/to/parent-worktree --socket /tmp/ploke-walk.sock\n\nLater walk commands default to this context when --repo-root/--socket are omitted."
)]
pub struct Prototype1StateWalkUseCommand {
    /// Parent checkout root to remember. Defaults to the current directory.
    #[arg(value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Explicit Unix socket path to remember for this walk context.
    #[arg(long, value_name = "PATH")]
    pub socket: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkControlCommand {
    /// Parent checkout root. Defaults to active walk context, then current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Explicit Unix socket path. Defaults to a repo-hashed path under the runtime directory.
    #[arg(long, value_name = "PATH")]
    pub socket: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Include protocol and transition-graph versions in table output.
    #[arg(long)]
    pub with_version: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Show current in-memory walk state or the last step delta",
    after_help = "Examples:\n  ploke-eval loop walk show\n  ploke-eval loop walk show --with-version\n  ploke-eval loop walk show delta --verbose\n\nUse summary/replay for durable historical inspection after a run has completed."
)]
pub struct Prototype1StateWalkShowCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    #[command(subcommand)]
    pub command: Option<Prototype1StateWalkShowSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Prototype1StateWalkShowSubcommand {
    /// Show only the last successful step delta.
    Delta(Prototype1StateWalkShowDeltaCommand),
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkShowDeltaCommand {
    /// Include changed axis values plus added/removed nested type structures.
    #[arg(long)]
    pub verbose: bool,

    /// Disable ANSI colors in table output.
    #[arg(long)]
    pub no_color: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Inspect nested LLM/tool-loop debugger checkpoints",
    after_help = "Examples:\n  ploke-eval loop walk llm show\n  ploke-eval loop walk llm show --session-id tool-loop-...\n  ploke-eval loop walk llm show --step 3\n\nThis is a read-only checkpoint inspection surface. It does not call providers, execute tools, mutate the checkout, or advance the outer typestate walk."
)]
pub struct Prototype1StateWalkLlmCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    #[command(subcommand)]
    pub command: Prototype1StateWalkLlmSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Prototype1StateWalkLlmSubcommand {
    /// Show the latest or selected LLM/tool-loop checkpoint.
    Show(Prototype1StateWalkLlmShowCommand),
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmShowCommand {
    /// Specific tool-loop session id to inspect. Defaults to the latest session.
    #[arg(long)]
    pub session_id: Option<String>,

    /// Specific response step to inspect. Defaults to the latest recorded step.
    #[arg(long)]
    pub step: Option<usize>,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Summarize durable campaign progress without contacting the walk server",
    after_help = "Field guide:\n  branch: selected branch disposition when the final report recorded one.\n  decision: candidate-local selection outcome; decision=Stop does not necessarily mean the campaign stopped.\n  handoff: whether the selected successor handoff was acknowledged, skipped, or absent.\n\nUse --verbose/-v for run-specific notes and next inspection commands."
)]
pub struct Prototype1StateWalkSummaryCommand {
    /// Parent checkout root. Defaults to active walk context, then current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Include field meanings and next typed inspection commands in table output.
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Show or jump within the read-only historical replay cursor",
    after_help = "Examples:\n  ploke-eval loop walk replay\n  ploke-eval loop walk replay --index 0\n  ploke-eval loop walk replay --index 120 --tail 20\n\nThe recent-entry window shows the latest journal entries by default. Use --tail N to show more entries. This command does not undo or replay durable side effects."
)]
pub struct Prototype1StateWalkReplayCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    /// Jump to this zero-based durable journal index before rendering.
    #[arg(long)]
    pub index: Option<usize>,

    /// Number of trailing journal entries to render.
    #[arg(long, default_value_t = 3)]
    pub tail: usize,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Move the read-only historical replay cursor backward or forward",
    after_help = "Examples:\n  ploke-eval loop walk forward\n  ploke-eval loop walk forward --steps 10 --tail 20\n  ploke-eval loop walk back --steps 1\n\nCursor movement is read-only. It changes only the server's in-memory replay cursor and never mutates the parent checkout or journal. Use --tail N to expand the recent-entry window."
)]
pub struct Prototype1StateWalkReplayMoveCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    /// Number of replay entries to move.
    #[arg(long, default_value_t = 1)]
    pub steps: usize,

    /// Number of trailing journal entries to render.
    #[arg(long, default_value_t = 3)]
    pub tail: usize,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Record explicit provenance before leaving replay toward live work",
    after_help = "Example:\n  ploke-eval loop walk branch-live --reason \"investigate cursor 42\" --allow provenance-record\n\nThis command records provenance only. It does not materialize a new live branch, run providers, or mutate checkout content."
)]
pub struct Prototype1StateWalkBranchLiveCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    /// Operator reason for leaving read-only historical replay.
    #[arg(long)]
    pub reason: String,

    /// Explicitly admit writing a replay-to-live provenance record.
    #[arg(long = "allow", value_name = "CAPABILITY", value_parser = ["provenance-record"])]
    pub allow: Vec<String>,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Advance the current in-memory walk by one live typestate edge",
    after_help = "Examples:\n  ploke-eval loop walk step\n  ploke-eval loop walk step --until r6\n  ploke-eval loop walk step --until r8 --watch\n  ploke-eval loop walk step --until r13b --watch --allow git-changes\n\nUse replay/back/forward for read-only historical inspection. Use step only when you intend to drive live typestate edges. Long live edges require --watch; checkout-mutating successor handoff requires --allow git-changes."
)]
pub struct Prototype1StateWalkStepCommand {
    /// Parent checkout root. Defaults to active walk context, then current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Explicit Unix socket path. Defaults to a repo-hashed path under the runtime directory.
    #[arg(long, value_name = "PATH")]
    pub socket: Option<PathBuf>,

    /// Advance repeatedly until this phase instead of exactly one step.
    #[arg(long, value_enum)]
    pub until: Option<WalkPhase>,

    /// Wait for a long live edge instead of returning at the safe boundary.
    #[arg(long)]
    pub watch: bool,

    /// Admit typed edges that intentionally install the selected successor into
    /// the active checkout. Required for R12 -> R13b handoff.
    #[arg(long = "allow", value_name = "CAPABILITY", value_parser = ["git-changes"])]
    pub allow: Vec<String>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Include protocol and transition-graph versions in table output.
    #[arg(long)]
    pub with_version: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Start a live in-memory walk, defaulting to R0",
    after_help = "Examples:\n  ploke-eval loop walk start\n  ploke-eval loop walk start --until r6\n  ploke-eval loop walk start --no-ttl\n\nStart creates or contacts the local walk server for the selected parent checkout. Use summary/replay when you only need to inspect a completed historical run."
)]
pub struct Prototype1StateWalkStartCommand {
    /// Campaign id. Defaults to parent identity, then active `select campaign`.
    #[arg(long)]
    pub campaign: Option<CampaignId>,

    /// Candidate node id to materialize/evaluate. During --init-parent-identity only, this is the generation-0 parent node.
    #[arg(long)]
    pub node_id: Option<String>,

    /// Parent checkout root. Defaults to active walk context, then current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Explicit Unix socket path. Defaults to a repo-hashed path under the runtime directory.
    #[arg(long, value_name = "PATH")]
    pub socket: Option<PathBuf>,

    /// Idle seconds before an auto-started server exits. Defaults to 1800 seconds.
    #[arg(long, value_name = "SECS", conflicts_with = "no_ttl")]
    pub ttl_secs: Option<u64>,

    /// Keep an auto-started server alive until an explicit stop request.
    #[arg(long)]
    pub no_ttl: bool,

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

    /// Stop after this admitted typestate phase. Defaults to R0.
    #[arg(long, value_enum, default_value_t = WalkPhase::R0)]
    pub until: WalkPhase,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Include protocol and transition-graph versions in table output.
    #[arg(long)]
    pub with_version: bool,
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

    /// Extra setup check: initialize the headless TUI sparse/BM25 runtime for this parent checkout without making model calls.
    #[arg(long)]
    pub headless_tui_setup_preflight: bool,
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
    pub campaign: Option<CampaignId>,

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
    pub campaign: Option<CampaignId>,

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
    pub source_campaign: Option<CampaignId>,

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
