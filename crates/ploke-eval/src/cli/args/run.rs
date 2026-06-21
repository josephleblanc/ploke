use std::path::PathBuf;

use ploke_records::ids::CampaignId;

use clap::{ArgAction, ArgGroup, Parser, Subcommand};

use crate::spec::OutputMode;

use super::common::InspectOutputFormat;

#[derive(Debug, Parser)]
#[command(
    about = "Traverse the eval execution tree for repo fetch, preparation, execution, and replay"
)]
pub struct RunCommand {
    #[command(subcommand)]
    pub command: RunSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum RunSubcommand {
    #[command(display_order = 10)]
    /// Ensure benchmark repo checkouts exist and refresh remote refs.
    Repo(RunRepoCommand),
    #[command(display_order = 11)]
    /// List built-in benchmark dataset keys and sources.
    Datasets(RunDatasetsCommand),
    #[command(display_order = 20)]
    /// Prepare ad hoc, single-instance, or batch manifests before execution.
    Prepare(RunPrepareCommand),
    #[command(display_order = 25)]
    /// List concrete run attempts for one instance and identify the latest attempt.
    List(RunListCommand),
    #[command(display_order = 30)]
    /// Execute one prepared instance through setup-only or agent-turn paths.
    Single(RunSingleWorkflowCommand),
    #[command(display_order = 31)]
    /// Execute one prepared batch through setup-only or agent-turn paths.
    Batch(RunBatchWorkflowCommand),
    #[command(display_order = 40)]
    /// Replay a prepared execution artifact such as one embedding batch.
    Replay(RunReplayCommand),
}

#[derive(Debug, Parser)]
#[command(about = "Operate on benchmark repo checkouts used by eval runs")]
pub struct RunRepoCommand {
    #[command(subcommand)]
    pub command: RunRepoSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum RunRepoSubcommand {
    /// Ensure one built-in benchmark repo exists and refresh its remote refs.
    Fetch(FetchMsbRepoCommand),
}

#[derive(Debug, Parser)]
#[command(about = "Operate on dataset discovery commands used by eval runs")]
pub struct RunDatasetsCommand {
    #[command(subcommand)]
    pub command: RunDatasetsSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum RunDatasetsSubcommand {
    /// List built-in dataset keys and source URLs.
    List,
}

#[derive(Debug, Parser)]
#[command(
    about = "Prepare manifests for custom tasks, one benchmark instance, or a selected batch"
)]
pub struct RunPrepareCommand {
    #[command(subcommand)]
    pub command: RunPrepareSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum RunPrepareSubcommand {
    /// Prepare one custom run manifest outside Multi-SWE-bench.
    Custom(PrepareSingleCommand),
    /// Prepare one Multi-SWE-bench instance for the normal single-run workflow.
    Instance(PrepareMsbSingleCommand),
    /// [advanced/manual] Prepare raw Multi-SWE-bench batch manifests.
    Batch(PrepareMsbBatchCommand),
}

#[derive(Debug, Parser)]
#[command(
    about = "Execute one prepared instance either through setup-only or the normal agent-turn path"
)]
pub struct RunSingleWorkflowCommand {
    #[command(subcommand)]
    pub command: RunSingleWorkflowSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum RunSingleWorkflowSubcommand {
    /// Run one prepared instance through setup only, without the agent turn.
    Setup(RunMsbSingleCommand),
    /// Run one prepared instance through the normal agent eval path.
    Agent(RunMsbAgentSingleCommand),
}

#[derive(Debug, Parser)]
#[command(
    about = "Execute one prepared batch either through setup-only or the normal agent-turn path"
)]
pub struct RunBatchWorkflowCommand {
    #[command(subcommand)]
    pub command: RunBatchWorkflowSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum RunBatchWorkflowSubcommand {
    /// [advanced/manual] Execute a prepared raw batch without agent turns.
    Setup(RunMsbBatchCommand),
    /// [advanced/manual] Execute a prepared raw batch with agent turns.
    Agent(RunMsbAgentBatchCommand),
}

#[derive(Debug, Parser)]
#[command(about = "Replay one prepared execution artifact for debugging")]
pub struct RunReplayCommand {
    #[command(subcommand)]
    pub command: RunReplaySubcommand,
}

#[derive(Debug, Subcommand)]
pub enum RunReplaySubcommand {
    /// [debug/manual] Replay one embedding batch from a prepared run.
    Batch(ReplayMsbBatchCommand),
    /// [debug/manual] Inspect replayable agent-turn cursors and response tapes.
    Inspect(ReplayInspectCommand),
    /// [debug/manual] Replay historical broad-harness self-edit tool requests.
    SelfEditLive(ReplaySelfEditLiveCommand),
    /// [debug/manual] Replay a recorded agent turn prefix, then continue live in one workspace.
    TurnLive(ReplayTurnLiveCommand),
}
#[derive(Debug, Parser)]
#[command(
    about = "Flat shortcut commands for common eval workflows; each mirrors a canonical `run ...` path"
)]
pub struct JustCommand {
    #[command(subcommand)]
    pub command: JustSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum JustSubcommand {
    #[command(display_order = 10, alias = "fetch-msb-repo")]
    /// Shortcut for `run repo fetch` (clone if missing; otherwise fetch remote refs).
    FetchRepo(FetchMsbRepoCommand),
    #[command(display_order = 11, alias = "list-msb-datasets")]
    /// Shortcut for `run datasets list`.
    ListDatasets,
    #[command(display_order = 20, alias = "prepare-single")]
    /// Shortcut for `run prepare custom`.
    PrepareCustom(PrepareSingleCommand),
    #[command(display_order = 21, alias = "prepare-msb-single")]
    /// Shortcut for `run prepare instance`.
    PrepareInstance(PrepareMsbSingleCommand),
    #[command(display_order = 22, alias = "prepare-msb-batch")]
    /// Shortcut for `run prepare batch`.
    PrepareBatch(PrepareMsbBatchCommand),
    #[command(display_order = 30, alias = "run-msb-single")]
    /// Shortcut for `run single setup`.
    SingleSetup(RunMsbSingleCommand),
    #[command(display_order = 31, alias = "run-msb-agent-single")]
    /// Shortcut for `run single agent`.
    Single(RunMsbAgentSingleCommand),
    #[command(display_order = 40, alias = "run-msb-batch")]
    /// Shortcut for `run batch setup`.
    BatchSetup(RunMsbBatchCommand),
    #[command(display_order = 41, alias = "run-msb-agent-batch")]
    /// Shortcut for `run batch agent`.
    Batch(RunMsbAgentBatchCommand),
    #[command(display_order = 50, alias = "replay-msb-batch")]
    /// Shortcut for `run replay batch`.
    ReplayBatch(ReplayMsbBatchCommand),
}

#[derive(Debug, Parser)]
pub struct JustWatchCommand {
    /// Campaign id. Defaults to parent identity, then active `select campaign`.
    #[arg(long)]
    pub campaign: Option<CampaignId>,

    /// Parent checkout root. Defaults to the current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Print narrow fixed-width rows for phone/tmux viewing.
    #[arg(long)]
    pub phone: bool,

    /// Polling interval in milliseconds.
    #[arg(long, default_value_t = 1000)]
    pub interval_ms: u64,
}
#[derive(Debug, Parser)]
#[command(about = "Normalize one ad hoc evaluation instance into a run manifest")]
pub struct PrepareSingleCommand {
    /// Stable task identifier for this run.
    #[arg(long)]
    pub task_id: String,

    /// Path to the repo checkout to evaluate.
    #[arg(long, value_name = "PATH")]
    pub repo: PathBuf,

    /// Title or short problem statement for the task.
    #[arg(long)]
    pub issue_title: Option<String>,

    /// Markdown or text file containing the issue body.
    #[arg(long, value_name = "PATH")]
    pub issue_file: Option<PathBuf>,

    /// Inline issue body text. Prefer --issue-file for longer prompts.
    #[arg(long)]
    pub issue_body: Option<String>,

    /// Optional benchmark base commit SHA.
    #[arg(long)]
    pub base_sha: Option<String>,

    /// Output directory for run artifacts.
    #[arg(long, value_name = "PATH")]
    pub out_dir: PathBuf,

    /// Print compact or pretty JSON.
    #[arg(long, value_enum, default_value_t = OutputMode::Pretty)]
    pub output_mode: OutputMode,

    /// Write the manifest to stdout instead of out_dir/run.json.
    #[arg(long)]
    pub stdout: bool,

    #[arg(long, default_value_t = 40)]
    pub max_turns: u32,

    #[arg(long, default_value_t = 200)]
    pub max_tool_calls: u32,

    #[arg(long, default_value_t = 1800)]
    pub wall_clock_secs: u32,
}
#[derive(Debug, Parser)]
#[command(
    about = "Build one run manifest from a Multi-SWE-bench JSONL instance",
    after_help = "\
Example:

  cargo run -p ploke-eval -- run prepare instance \
    --dataset-key ripgrep \
    --instance BurntSushi__ripgrep-2209

Defaults:
  dataset cache: ~/.ploke-eval/datasets
  repo cache:    ~/.ploke-eval/repos
  instances root: ~/.ploke-eval/instances

Reads:
  Multi-SWE-bench dataset JSONL
  repo checkout under <repo-cache>/<org>/<repo>

Writes:
  ~/.ploke-eval/instances/<instance>/run.json
"
)]
pub struct PrepareMsbSingleCommand {
    /// Multi-SWE-bench dataset JSONL file.
    #[arg(long, value_name = "PATH", conflicts_with = "dataset_key")]
    pub dataset: Option<PathBuf>,

    /// Built-in dataset registry key, for example ripgrep.
    #[arg(long, conflicts_with = "dataset")]
    pub dataset_key: Option<String>,

    /// Benchmark instance id, for example clap-rs__clap-1234.
    #[arg(long)]
    pub instance: String,

    /// Root directory containing repo checkouts at <repo-cache>/<org>/<repo>.
    /// Defaults to ~/.ploke-eval/repos.
    #[arg(long, value_name = "PATH")]
    pub repo_cache: Option<PathBuf>,

    /// Root directory where ploke-eval should create per-run directories.
    /// Defaults to ~/.ploke-eval/instances.
    #[arg(long = "instances-root", alias = "runs-root", value_name = "PATH")]
    pub instances_root: Option<PathBuf>,

    /// Print compact or pretty JSON.
    #[arg(long, value_enum, default_value_t = OutputMode::Pretty)]
    pub output_mode: OutputMode,

    /// Write the manifest to stdout instead of instances_root/<task_id>/run.json.
    #[arg(long)]
    pub stdout: bool,

    #[arg(long, default_value_t = 40)]
    pub max_turns: u32,

    #[arg(long, default_value_t = 200)]
    pub max_tool_calls: u32,

    #[arg(long, default_value_t = 1800)]
    pub wall_clock_secs: u32,
}
#[derive(Debug, Parser)]
#[command(
    about = "Build one batch manifest and per-instance run manifests from Multi-SWE-bench JSONL",
    after_help = "\
Examples:

  cargo run -p ploke-eval -- run prepare batch --dataset-key ripgrep --all
  cargo run -p ploke-eval -- run prepare batch --dataset-key ripgrep --specific 2209
  cargo run -p ploke-eval -- run prepare batch --dataset-key ripgrep --instance BurntSushi__ripgrep-2209

Defaults:
  dataset cache: ~/.ploke-eval/datasets
  repo cache:    ~/.ploke-eval/repos
  instances root: ~/.ploke-eval/instances
  batches root:  ~/.ploke-eval/batches

Writes:
  ~/.ploke-eval/instances/<instance>/run.json for each selected instance
  ~/.ploke-eval/batches/<batch-id>/batch.json
"
)]
pub struct PrepareMsbBatchCommand {
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

    /// Stable identifier for the batch manifest directory.
    #[arg(long)]
    pub batch_id: Option<String>,

    /// Root directory containing repo checkouts at <repo-cache>/<org>/<repo>.
    #[arg(long, value_name = "PATH")]
    pub repo_cache: Option<PathBuf>,

    /// Root directory where ploke-eval should create per-run directories.
    #[arg(long = "instances-root", alias = "runs-root", value_name = "PATH")]
    pub instances_root: Option<PathBuf>,

    /// Root directory where ploke-eval should create batch manifests and summaries.
    #[arg(long, value_name = "PATH")]
    pub batches_root: Option<PathBuf>,

    /// Print compact or pretty JSON.
    #[arg(long, value_enum, default_value_t = OutputMode::Pretty)]
    pub output_mode: OutputMode,

    #[arg(long, default_value_t = 40)]
    pub max_turns: u32,

    #[arg(long, default_value_t = 200)]
    pub max_tool_calls: u32,

    #[arg(long, default_value_t = 1800)]
    pub wall_clock_secs: u32,
}
#[derive(Debug, Parser)]
#[command(
    about = "Execute one prepared Multi-SWE-bench run",
    after_help = "\
Example:

  cargo run -p ploke-eval -- run single setup --instance BurntSushi__ripgrep-2209

Default manifest path:
  ~/.ploke-eval/instances/<instance>/run.json

Default output artifacts under the run directory:
  repo-state.json
  execution-log.json
  indexing-status.json
  snapshot-status.json
  indexing-checkpoint.db
  indexing-failure.db

The runner also creates a per-run config sandbox at:
  ~/.ploke-eval/instances/<instance>/config

That sandbox is used so SaveDb writes its registry and snapshot files into
the run directory instead of your normal user config directory.

Debug snapshots:
  `--no-index-debug-snapshots` disables the eval-only DB snapshots written
  during indexing progress and indexing failure events.

Use `--provider <slug>` to pin a specific OpenRouter provider for the selected model.
"
)]
pub struct RunMsbSingleCommand {
    /// Path to a prepared run manifest. Defaults to ~/.ploke-eval/instances/<instance>/run.json.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub run: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/instances/<instance>/run.json.
    #[arg(long, conflicts_with = "run")]
    pub instance: Option<String>,

    /// Disable eval-only DB checkpoint/failure snapshots during indexing.
    #[arg(long = "no-index-debug-snapshots", action = ArgAction::SetFalse, default_value_t = true)]
    pub index_debug_snapshots: bool,

    /// Use the default model instead of the persisted active model selection.
    #[arg(long)]
    pub use_default_model: bool,

    /// Explicit model id to use for this run.
    #[arg(long)]
    pub model_id: Option<String>,

    /// Explicit provider slug to pin for the selected model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Explicit embedding model id to use for eval indexing/retrieval on this run.
    #[arg(long)]
    pub embedding_model_id: Option<String>,

    /// Explicit provider slug to pin for the embedding model on this run.
    #[arg(long, value_name = "PROVIDER")]
    pub embedding_provider: Option<String>,
}

#[derive(Debug, Parser)]
#[command(
    about = "Execute many prepared Multi-SWE-bench runs",
    after_help = "\
Examples:

  cargo run -p ploke-eval -- run batch setup --batch-id ripgrep-all
  cargo run -p ploke-eval -- run batch setup --batch ~/.ploke-eval/batches/ripgrep-all/batch.json

The command reuses the per-instance run manifests listed by the batch manifest,
executes them sequentially, and writes:
  batch-run-summary.json
  multi-swe-bench-submission.jsonl
"
)]
pub struct RunMsbBatchCommand {
    /// Path to a prepared batch manifest. Defaults to ~/.ploke-eval/batches/<batch-id>/batch.json.
    #[arg(long, value_name = "PATH", conflicts_with = "batch_id")]
    pub batch: Option<PathBuf>,

    /// Batch id, used to resolve ~/.ploke-eval/batches/<batch-id>/batch.json.
    #[arg(long, conflicts_with = "batch")]
    pub batch_id: Option<String>,

    /// Disable eval-only DB checkpoint/failure snapshots during indexing.
    #[arg(long = "no-index-debug-snapshots", action = ArgAction::SetFalse, default_value_t = true)]
    pub index_debug_snapshots: bool,

    /// Use the default model instead of the persisted active model selection.
    #[arg(long)]
    pub use_default_model: bool,

    /// Explicit model id to use for this batch.
    #[arg(long)]
    pub model_id: Option<String>,

    /// Explicit provider slug to pin for the selected model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Stop the batch after the first per-instance runner failure.
    #[arg(long)]
    pub stop_on_error: bool,
}

// ANCHOR: ploke_eval_run_msb_agent_single_command
#[derive(Debug, Parser)]
#[command(
    about = "Execute one prepared Multi-SWE-bench run and one benchmark issue turn",
    after_help = "\
This extends the normal run with a single agentic turn that:
  - submits the prepared issue prompt through the real app/state path
  - records prompt construction, tool lifecycle, message updates, and turn completion
  - writes a turn trace and summary beside the run artifacts

Use `--provider <slug>` to pin a specific OpenRouter provider for the selected model.

Outputs:
  The prepared instance root is ~/.ploke-eval/instances/<instance>.
  Each invocation writes a unique nested run directory under:
    ~/.ploke-eval/instances/<instance>/runs/run-<timestamp>-<arm>-<suffix>

  Key agent-run files inside that nested run directory:
    execution-log.json
    repo-state.json
    indexing-status.json
    snapshot-status.json
    agent-turn-trace.json
    agent-turn-summary.json
    llm-full-responses.jsonl
    multi-swe-bench-submission.jsonl
"
)]
pub struct RunMsbAgentSingleCommand {
    /// Path to a prepared run manifest. Defaults to ~/.ploke-eval/instances/<instance>/run.json.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub run: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/instances/<instance>/run.json.
    #[arg(long, conflicts_with = "run")]
    pub instance: Option<String>,

    /// Disable eval-only DB checkpoint/failure snapshots during indexing.
    #[arg(long = "no-index-debug-snapshots", action = ArgAction::SetFalse, default_value_t = true)]
    pub index_debug_snapshots: bool,

    /// Use the default model instead of the persisted active model selection.
    #[arg(long)]
    pub use_default_model: bool,

    /// Explicit model id to use for this run.
    #[arg(long)]
    pub model_id: Option<String>,

    /// Explicit provider slug to pin for the selected model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Explicit embedding model id to use for eval indexing/retrieval on this run.
    #[arg(long)]
    pub embedding_model_id: Option<String>,

    /// Explicit provider slug to pin for the embedding model on this run.
    #[arg(long, value_name = "PROVIDER")]
    pub embedding_provider: Option<String>,
}
// ANCHOR_END: ploke_eval_run_msb_agent_single_command

// ANCHOR: ploke_eval_run_msb_agent_batch_command
#[derive(Debug, Parser)]
#[command(
    about = "Execute many prepared Multi-SWE-bench runs and one benchmark issue turn for each",
    after_help = "\
Examples:

  cargo run -p ploke-eval -- run batch agent --batch-id ripgrep-all
  cargo run -p ploke-eval -- run batch agent --batch ~/.ploke-eval/batches/ripgrep-all/batch.json

This reuses the per-instance run manifests listed by the batch manifest,
executes one benchmark issue turn per instance, and writes:
  batch-run-summary.json
  multi-swe-bench-submission.jsonl

Paths:
  Batch summary and batch aggregate submission live under:
    ~/.ploke-eval/batches/<batch-id>/

Operational caution:
  Treat the batch aggregate multi-swe-bench-submission.jsonl as a convenience output.
  For stronger local truth, inspect per-run submission files under:
    ~/.ploke-eval/instances/<instance>/runs/run-*/multi-swe-bench-submission.jsonl
  or use:
    cargo run -p ploke-eval -- campaign export-submissions --campaign <campaign>
"
)]
pub struct RunMsbAgentBatchCommand {
    /// Path to a prepared batch manifest. Defaults to ~/.ploke-eval/batches/<batch-id>/batch.json.
    #[arg(long, value_name = "PATH", conflicts_with = "batch_id")]
    pub batch: Option<PathBuf>,

    /// Batch id, used to resolve ~/.ploke-eval/batches/<batch-id>/batch.json.
    #[arg(long, conflicts_with = "batch")]
    pub batch_id: Option<String>,

    /// Disable eval-only DB checkpoint/failure snapshots during indexing.
    #[arg(long = "no-index-debug-snapshots", action = ArgAction::SetFalse, default_value_t = true)]
    pub index_debug_snapshots: bool,

    /// Use the default model instead of the persisted active model selection.
    #[arg(long)]
    pub use_default_model: bool,

    /// Explicit model id to use for this batch.
    #[arg(long)]
    pub model_id: Option<String>,

    /// Explicit provider slug to pin for the selected model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Stop the batch after the first per-instance runner failure.
    #[arg(long)]
    pub stop_on_error: bool,
}
// ANCHOR_END: ploke_eval_run_msb_agent_batch_command
#[derive(Debug, Parser)]
#[command(about = "List concrete run attempts for one instance")]
pub struct RunListCommand {
    /// Benchmark instance id. Defaults to the selected instance when set.
    #[arg(long)]
    pub instance: Option<String>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
#[command(
    about = "Replay one batch from a prepared Multi-SWE-bench run",
    after_help = "\
Example:

  cargo run -p ploke-eval -- run replay batch --instance BurntSushi__ripgrep-2209 --batch 6

This reuses the prepared run manifest and executes only the selected batch.
It writes `replay-batch-<nnn>.json` into the run directory, logs the full node
metadata for that batch, and then runs the normal embed path so any OpenRouter
failure surfaces with the exact snippets in the eval log.
"
)]
pub struct ReplayMsbBatchCommand {
    /// Path to a prepared run manifest. Defaults to ~/.ploke-eval/instances/<instance>/run.json.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub run: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/instances/<instance>/run.json.
    #[arg(long, conflicts_with = "run")]
    pub instance: Option<String>,

    /// 1-based batch number to replay.
    #[arg(long)]
    pub batch: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReplayTurnArtifactArg {
    Trace,
    Summary,
}

impl From<ReplayTurnArtifactArg> for ploke_tree::TurnArtifactKind {
    fn from(value: ReplayTurnArtifactArg) -> Self {
        match value {
            ReplayTurnArtifactArg::Trace => Self::Trace,
            ReplayTurnArtifactArg::Summary => Self::Summary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReplayTailArg {
    Stop,
    Live,
    LiveStep,
}

impl From<ReplayTailArg> for crate::replay::turn::ReplayTail {
    fn from(value: ReplayTailArg) -> Self {
        match value {
            ReplayTailArg::Stop => Self::Stop,
            ReplayTailArg::Live => Self::Live,
            ReplayTailArg::LiveStep => Self::LiveStep,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    about = "Replay one recorded agent-turn prefix, then continue through the live TUI tool loop",
    after_help = "\
Example:

  cargo run -p ploke-eval -- run replay turn-live \\
    --run-dir ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node>/output \\
    --workspace ~/.ploke-eval/campaigns/<campaign>/prototype1/workspaces/<workspace> \\
    --event-index 0 --max-attempts 1 --format table

This installs the recorded provider response tape resolved from the agent-turn
cursor, submits the recorded issue prompt to the target workspace, and consumes
the selected prefix through the normal ploke-tui session/tool path. By default
the prefix is the full assistant-message tape and the next provider request goes
live. Use --through-response-index or --through-event to slice the prefix,
--tail stop to stop before any live provider call, or --tail live-step to take
one live provider step and stop at the next provider boundary. Use --branch-out
to save that live response as replayable branch tape, then --branch-in on the
next invocation to continue one more step from the same branch.

The command is intentionally explicit about --run-dir and --workspace. The run
directory supplies the historical typed records and response sidecar; the
workspace supplies current search/code/patch behavior. Keeping those separate
lets the operator replay a historical model prefix against the workspace they
actually want to test.
"
)]
pub struct ReplayTurnLiveCommand {
    /// Directory containing agent-turn artifacts and llm-full-responses.jsonl.
    #[arg(long, value_name = "DIR")]
    pub run_dir: PathBuf,

    /// Workspace whose current files, search index, and tool behavior should be used.
    #[arg(long, value_name = "DIR")]
    pub workspace: PathBuf,

    /// Agent-turn artifact family.
    #[arg(long, value_enum, default_value_t = ReplayTurnArtifactArg::Trace)]
    pub artifact_kind: ReplayTurnArtifactArg,

    /// Relative agent-turn artifact path inside --run-dir.
    #[arg(long, default_value = "agent-turn-trace.json")]
    pub artifact_path: String,

    /// Zero-based event index inside the selected agent-turn artifact.
    #[arg(long, default_value_t = 0)]
    pub event_index: usize,

    /// Install responses from index 0 through this provider response index.
    #[arg(long, value_name = "N", conflicts_with = "through_event")]
    pub through_response_index: Option<usize>,

    /// Use --event-index as the breakpoint event and install the prefix needed to replay through it.
    #[arg(long, conflicts_with = "through_response_index")]
    pub through_event: bool,

    /// What to do after the selected recorded/branch prefix is exhausted.
    #[arg(long, value_enum, default_value_t = ReplayTailArg::Live)]
    pub tail: ReplayTailArg,

    /// Replay branch tape to append after the selected historical prefix.
    ///
    /// Use the file written by a previous `--tail live-step --branch-out`
    /// invocation to continue stepping from that live branch. The branch holds
    /// provider responses only; tool behavior is still re-executed in the
    /// current `--workspace`.
    #[arg(long, value_name = "FILE")]
    pub branch_in: Option<PathBuf>,

    /// Write the replay branch tape after appending newly observed live responses.
    ///
    /// This is most useful with `--tail live-step`: each invocation can write a
    /// new branch file, and the next invocation can pass it as `--branch-in` to
    /// advance one more provider response through the same current tool loop.
    #[arg(long, value_name = "FILE")]
    pub branch_out: Option<PathBuf>,

    /// Model id for the live tail. Defaults to the parent-patcher model selection.
    #[arg(long, value_name = "MODEL")]
    pub model_id: Option<String>,

    /// Provider slug to pin for the selected model. Requires --model-id.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Maximum headless retry attempts around the live tail.
    #[arg(long, default_value_t = 1)]
    pub max_attempts: u32,

    /// Overall timeout for the headless live probe.
    #[arg(long, default_value_t = 300)]
    pub timeout_secs: u64,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
#[command(
    about = "Replay broad-harness self-edit tool requests through the live TUI tool loop",
    group(
        ArgGroup::new("self_edit_source")
            .required(true)
            .args(["result", "raw_full_response"])
    ),
    after_help = "\
Example:

  cargo run -p ploke-eval -- run replay self-edit-live \\
    --request ~/.ploke-eval/campaigns/<campaign>/prototype1/messages/edit-harness-request/<node>.json \\
    --result ~/.ploke-eval/campaigns/<campaign>/prototype1/messages/edit-harness-result/<node>.headless-tui.json \\
    --workspace ~/.ploke-eval/campaigns/<campaign>/prototype1/workspaces/edit-harness/<node> \\
    --event-index 7 --through-event --tail stop --format table

This command is for self-edit attempts, not benchmark eval runs. It reads the
published broad-harness request and historical headless-TUI evidence, rebuilds
recorded assistant tool-call responses from historical ToolRequest events, and
executes those requests through the current TUI tools in --workspace. It does
not replay historical ToolCompleted or ToolFailed events.

When no .headless-tui.json result exists, pass --raw-full-response with a
single-attempt llm_full_response*.log JSONL sidecar. Raw sidecars already hold
provider response envelopes, so replay installs those records directly. Use
--through-response-index to stop a raw-sidecar replay at a provider response
boundary.
"
)]
pub struct ReplaySelfEditLiveCommand {
    /// Published broad-harness request JSON.
    #[arg(long, value_name = "FILE")]
    pub request: PathBuf,

    /// Historical .headless-tui.json result containing compact tool events.
    #[arg(long, value_name = "FILE")]
    pub result: Option<PathBuf>,

    /// Raw llm_full_response*.log JSONL sidecar for one self-edit attempt.
    #[arg(long, value_name = "FILE")]
    pub raw_full_response: Option<PathBuf>,

    /// Workspace whose current files, search index, and tool behavior should be used.
    #[arg(long, value_name = "DIR")]
    pub workspace: PathBuf,

    /// Zero-based event index inside the historical headless-TUI event list.
    ///
    /// Applies only to --result sources. Defaults to 0 when omitted.
    #[arg(long, value_name = "N", conflicts_with = "raw_full_response")]
    pub event_index: Option<usize>,

    /// Replay historical tool requests from event 0 through --event-index.
    #[arg(long, conflicts_with = "raw_full_response")]
    pub through_event: bool,

    /// Replay raw provider responses from response_index 0 through this index.
    #[arg(long, value_name = "N", requires = "raw_full_response")]
    pub through_response_index: Option<usize>,

    /// What to do after the selected recorded prefix is exhausted.
    #[arg(long, value_enum, default_value_t = ReplayTailArg::Stop)]
    pub tail: ReplayTailArg,

    /// Model id for the live tail. Defaults to the parent-patcher model selection.
    #[arg(long, value_name = "MODEL")]
    pub model_id: Option<String>,

    /// Provider slug to pin for the selected model. Requires --model-id.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Maximum headless retry attempts around the replay/live tail.
    #[arg(long, default_value_t = 1)]
    pub max_attempts: u32,

    /// Overall timeout for the headless replay probe.
    #[arg(long, default_value_t = 300)]
    pub timeout_secs: u64,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
#[command(
    about = "Inspect replayable agent-turn cursors and provider-response sidecars",
    after_help = "\
Example:

  cargo run -p ploke-eval -- run replay inspect \\
    --run-dir ~/.ploke-eval/instances/prototype1/<campaign>/<target>/runs/<run> \\
    --workspace ~/.ploke-eval/repos/<owner>/<repo> \\
    --format table

This command reads typed agent-turn artifacts and llm-full-responses.jsonl
without executing the TUI loop or calling a live provider. Use it to choose a
cursor and catch obvious replay health problems such as missing response
indexes or prompt paths that point at an old workspace.
"
)]
pub struct ReplayInspectCommand {
    /// Directory containing agent-turn artifacts and llm-full-responses.jsonl.
    #[arg(long, value_name = "DIR")]
    pub run_dir: PathBuf,

    /// Optional target workspace used only for prompt-path mismatch detection.
    #[arg(long, value_name = "DIR")]
    pub workspace: Option<PathBuf>,

    /// Optional maximum number of rows to render in table mode.
    #[arg(long)]
    pub limit: Option<usize>,

    /// In table mode, show only requested/completed/failed tool events.
    #[arg(long)]
    pub tool_events_only: bool,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}
#[derive(Debug, Parser)]
#[command(
    about = "Ensure one built-in benchmark repo exists under ~/.ploke-eval/repos",
    after_help = "\
Example:

  cargo run -p ploke-eval -- run repo fetch --dataset-key ripgrep

Default destination:
  ~/.ploke-eval/repos/<org>/<repo>

Behavior:
  If the repo checkout is missing, this clones it into the default destination.
  If the repo checkout already exists, this runs:
    git fetch --all --tags --prune
  in that checkout to refresh remote refs.

This does not reset the working tree, switch branches, or check out a benchmark SHA.
"
)]
pub struct FetchMsbRepoCommand {
    /// Built-in dataset registry key, for example ripgrep.
    #[arg(long)]
    pub dataset_key: String,
}
