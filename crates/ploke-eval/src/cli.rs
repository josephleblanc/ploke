use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::OnceLock;

use clap::{ArgAction, Parser, Subcommand};
use ploke_llm::Router;
use ploke_llm::request::endpoint::Endpoint;
use ploke_llm::router_only::HasEndpoint;
use ploke_llm::router_only::openrouter::{OpenRouter, OpenRouterModelId};
use ploke_llm::{ModelId, ProviderKey};
use regex::Regex;

use crate::layout::{
    active_model_file, batches_dir, cache_dir, datasets_dir, model_registry_file, models_dir,
    repos_dir, runs_dir, starting_db_cache_dir, workspace_root_for_key,
};
use crate::model_registry::{
    find_models, load_active_model, load_model_registry, refresh_model_registry,
    registry_has_model, save_active_model,
};
use crate::msb::{PrepareMsbBatchRequest, PrepareMsbSingleRunRequest};
use crate::provider_prefs::{
    clear_provider_for_model, load_provider_for_model, set_provider_for_model,
};
use crate::record::read_compressed_record;
use crate::registry::{builtin_dataset_registry_entries, builtin_dataset_registry_entry};
use crate::run_history::print_last_run_assistant_messages;
use crate::runner::{
    ReplayMsbBatchRequest, RunMsbAgentBatchRequest, RunMsbAgentSingleRequest, RunMsbBatchRequest,
    RunMsbSingleRequest, resolve_provider_for_model,
};
use crate::spec::{
    EvalBudget, IssueInput, OutputMode, PrepareError, PrepareSingleRunRequest, PrepareWrite,
};

const CLI_LONG_ABOUT: &str = "\
Minimal evaluation runner scaffolding for ploke.

Defaults:
  PLOKE_EVAL_HOME    ~/.ploke-eval
  datasets cache     ~/.ploke-eval/datasets
  model registry     ~/.ploke-eval/models/registry.json
  active model       ~/.ploke-eval/models/active-model.json
  provider prefs     ~/.ploke-eval/models/provider-preferences.json
  repo cache         ~/.ploke-eval/repos
  run artifacts      ~/.ploke-eval/runs
  batch artifacts    ~/.ploke-eval/batches

The current end-to-end path is:
  1. fetch a benchmark repo
  2. prepare a run manifest from Multi-SWE-bench
  3. run one prepared instance

The runner currently:
  - resets the repo to the benchmark base commit
  - indexes the repo with ploke
  - saves a DB snapshot via the same SaveDb path used by ploke-tui
  - writes run artifacts under the per-instance run directory
";

const CLI_AFTER_HELP: &str = "\
Quick Start: ripgrep single-run example

  cargo run -p ploke-eval -- fetch-msb-repo --dataset-key ripgrep
  cargo run -p ploke-eval -- prepare-msb-single --dataset-key ripgrep --instance BurntSushi__ripgrep-2209
  cargo run -p ploke-eval -- run-msb-single --instance BurntSushi__ripgrep-2209
  cargo run -p ploke-eval -- model providers moonshotai/kimi-k2
  cargo run -p ploke-eval -- run-msb-agent-single --instance BurntSushi__ripgrep-2209 --provider chutes

Batch example

  cargo run -p ploke-eval -- prepare-msb-batch --dataset-key ripgrep --specific 2209
  cargo run -p ploke-eval -- run-msb-agent-batch --batch-id ripgrep-2209

Health check

  cargo run -p ploke-eval -- doctor

Replay a specific embedding batch from a prepared run

  cargo run -p ploke-eval -- replay-msb-batch --instance BurntSushi__ripgrep-2209 --batch 6

Replay notes:
  - batch numbers are 1-based
  - the command writes `replay-batch-<nnn>.json` beside the run manifest
  - the JSON includes the full serialized `TypedEmbedData` batch
  - the command then runs only that batch through the normal embed path

Print assistant messages from the most recent completed run

  cargo run -p ploke-eval -- transcript

Inspect providers for a model

  cargo run -p ploke-eval -- model providers

Set the default provider for the current model

  cargo run -p ploke-eval -- model provider set chutes

Pin a provider for one run only

  cargo run -p ploke-eval -- run-msb-agent-single --instance BurntSushi__ripgrep-2209 --provider chutes

Default read/write locations

  Dataset JSONL cache:
    ~/.ploke-eval/datasets/BurntSushi__ripgrep_dataset.jsonl

  Repo checkout:
    ~/.ploke-eval/repos/BurntSushi/ripgrep

  Run directory:
    ~/.ploke-eval/runs/BurntSushi__ripgrep-2209

  Batch directory:
    ~/.ploke-eval/batches/ripgrep-2209

  Key run artifacts:
    run.json
    repo-state.json
    execution-log.json
    indexing-status.json
    snapshot-status.json
    multi-swe-bench-submission.jsonl

  Benchmark boundary:
    `multi-swe-bench-submission.jsonl` is a candidate patch artifact for the
    official Multi-SWE-bench evaluator.
    Local `ploke-eval` artifacts are run telemetry, not the benchmark verdict.
    Official pass/fail comes from running the external evaluator on the
    exported submission.

Override the root with:
  PLOKE_EVAL_HOME=/some/path
";

#[derive(Debug, Parser)]
#[command(
    name = "ploke-eval",
    about = "Run prepared ploke benchmark/eval instances",
    long_about = CLI_LONG_ABOUT,
    after_help = CLI_AFTER_HELP,
    version = env!("CARGO_PKG_VERSION"),
    propagate_version = true
)]
pub struct Cli {
    /// Enable low-level dbg_tools tracing for tool dispatch and IO write debugging.
    #[arg(long, global = true)]
    pub debug_tools: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Normalize one evaluation instance into a run manifest.
    PrepareSingle(PrepareSingleCommand),
    /// Normalize one Multi-SWE-bench instance into a run manifest.
    PrepareMsbSingle(PrepareMsbSingleCommand),
    /// Normalize many Multi-SWE-bench instances into a batch manifest and per-instance run manifests.
    PrepareMsbBatch(PrepareMsbBatchCommand),
    /// Execute one prepared run through repo reset and initial artifact generation.
    RunMsbSingle(RunMsbSingleCommand),
    /// Execute many prepared Multi-SWE-bench runs from one batch manifest.
    RunMsbBatch(RunMsbBatchCommand),
    /// Execute one prepared run and then run a single agentic benchmark turn.
    RunMsbAgentSingle(RunMsbAgentSingleCommand),
    /// Execute many prepared runs and agentic benchmark turns from one batch manifest.
    RunMsbAgentBatch(RunMsbAgentBatchCommand),
    /// Replay one specific batch from a prepared run and print the exact snippets for it.
    ReplayMsbBatch(ReplayMsbBatchCommand),
    /// Clone or refresh a benchmark repo into ~/.ploke-eval/repos.
    FetchMsbRepo(FetchMsbRepoCommand),
    /// List built-in dataset registry entries.
    ListMsbDatasets,
    /// Inspect the current eval setup and report likely configuration issues.
    Doctor,
    /// Print only assistant messages from the most recent completed run.
    Transcript,
    /// List all agent conversation turns from a run.
    Conversations(ConversationsCommand),
    /// Inspect run and turn data (conversations, tool calls, db snapshots, etc.)
    Inspect(InspectCommand),
    /// Manage the cached OpenRouter model registry and active model selection.
    Model(ModelCommand),
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

impl Cli {
    pub async fn run(self) -> ExitCode {
        match self.command {
            Command::PrepareSingle(cmd) => match cmd.run() {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::PrepareMsbSingle(cmd) => match cmd.run() {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::PrepareMsbBatch(cmd) => match cmd.run() {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::RunMsbSingle(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::RunMsbBatch(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::RunMsbAgentSingle(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::RunMsbAgentBatch(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::ReplayMsbBatch(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::FetchMsbRepo(cmd) => match cmd.run() {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::ListMsbDatasets => {
                for entry in builtin_dataset_registry_entries() {
                    println!("{}\t{}\t{}", entry.key, entry.language, entry.url);
                }
                ExitCode::SUCCESS
            }
            Command::Doctor => match run_doctor() {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::Transcript => match print_last_run_assistant_messages().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::Conversations(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::Inspect(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::Model(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
        }
    }
}

impl PrepareSingleCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let request = PrepareSingleRunRequest {
            task_id: self.task_id,
            repo_root: self.repo,
            issue: IssueInput {
                title: self.issue_title,
                body: self.issue_body,
                body_path: self.issue_file,
            },
            output_dir: self.out_dir,
            base_sha: self.base_sha,
            budget: EvalBudget {
                max_turns: self.max_turns,
                max_tool_calls: self.max_tool_calls,
                wall_clock_secs: self.wall_clock_secs,
            },
        };

        let prepared = request.prepare()?;
        let write = if self.stdout {
            PrepareWrite::Stdout
        } else {
            PrepareWrite::File(prepared.manifest_path())
        };
        prepared.write_manifest(self.output_mode, write)
    }
}

#[derive(Debug, Parser)]
#[command(
    about = "Build one run manifest from a Multi-SWE-bench JSONL instance",
    after_help = "\
Example:

  cargo run -p ploke-eval -- prepare-msb-single \
    --dataset-key ripgrep \
    --instance BurntSushi__ripgrep-2209

Defaults:
  dataset cache: ~/.ploke-eval/datasets
  repo cache:    ~/.ploke-eval/repos
  runs root:     ~/.ploke-eval/runs

Reads:
  Multi-SWE-bench dataset JSONL
  repo checkout under <repo-cache>/<org>/<repo>

Writes:
  ~/.ploke-eval/runs/<instance>/run.json
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
    /// Defaults to ~/.ploke-eval/runs.
    #[arg(long, value_name = "PATH")]
    pub runs_root: Option<PathBuf>,

    /// Print compact or pretty JSON.
    #[arg(long, value_enum, default_value_t = OutputMode::Pretty)]
    pub output_mode: OutputMode,

    /// Write the manifest to stdout instead of runs_root/<task_id>/run.json.
    #[arg(long)]
    pub stdout: bool,

    #[arg(long, default_value_t = 40)]
    pub max_turns: u32,

    #[arg(long, default_value_t = 200)]
    pub max_tool_calls: u32,

    #[arg(long, default_value_t = 1800)]
    pub wall_clock_secs: u32,
}

impl PrepareMsbSingleCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let prepared = PrepareMsbSingleRunRequest {
            dataset_file: self.dataset,
            dataset_key: self.dataset_key,
            instance_id: self.instance,
            repo_cache: self.repo_cache.unwrap_or(repos_dir()?),
            runs_root: self.runs_root.unwrap_or(runs_dir()?),
            budget: EvalBudget {
                max_turns: self.max_turns,
                max_tool_calls: self.max_tool_calls,
                wall_clock_secs: self.wall_clock_secs,
            },
        }
        .prepare()?;

        let write = if self.stdout {
            PrepareWrite::Stdout
        } else {
            PrepareWrite::File(prepared.manifest_path())
        };
        prepared.write_manifest(self.output_mode, write)
    }
}

#[derive(Debug, Parser)]
#[command(
    about = "Build one batch manifest and per-instance run manifests from Multi-SWE-bench JSONL",
    after_help = "\
Examples:

  cargo run -p ploke-eval -- prepare-msb-batch --dataset-key ripgrep --all
  cargo run -p ploke-eval -- prepare-msb-batch --dataset-key ripgrep --specific 2209
  cargo run -p ploke-eval -- prepare-msb-batch --dataset-key ripgrep --instance BurntSushi__ripgrep-2209

Defaults:
  dataset cache: ~/.ploke-eval/datasets
  repo cache:    ~/.ploke-eval/repos
  runs root:     ~/.ploke-eval/runs
  batches root:  ~/.ploke-eval/batches

Writes:
  ~/.ploke-eval/runs/<instance>/run.json for each selected instance
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
    #[arg(long, value_name = "PATH")]
    pub runs_root: Option<PathBuf>,

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

impl PrepareMsbBatchCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let batch_id = self.batch_id.unwrap_or_else(|| {
            default_batch_id(
                self.dataset_key.as_deref(),
                self.dataset.as_ref(),
                self.all,
                &self.instance,
                &self.specific,
            )
        });
        let prepared = PrepareMsbBatchRequest {
            dataset_file: self.dataset,
            dataset_key: self.dataset_key,
            batch_id,
            select_all: self.all,
            instance_ids: self.instance,
            specifics: self.specific,
            limit: self.limit,
            repo_cache: self.repo_cache.unwrap_or(repos_dir()?),
            runs_root: self.runs_root.unwrap_or(runs_dir()?),
            batches_root: self.batches_root.unwrap_or(batches_dir()?),
            budget: EvalBudget {
                max_turns: self.max_turns,
                max_tool_calls: self.max_tool_calls,
                wall_clock_secs: self.wall_clock_secs,
            },
        }
        .prepare()?;

        for run in &prepared.runs {
            run.write_manifest(self.output_mode, PrepareWrite::File(run.manifest_path()))?;
        }
        prepared.batch.write_manifest(self.output_mode)?;
        println!("{}", prepared.batch.manifest_path().display());
        Ok(())
    }
}

#[derive(Debug, Parser)]
#[command(
    about = "Execute one prepared Multi-SWE-bench run",
    after_help = "\
Example:

  cargo run -p ploke-eval -- run-msb-single --instance BurntSushi__ripgrep-2209

Default manifest path:
  ~/.ploke-eval/runs/<instance>/run.json

Default output artifacts under the run directory:
  repo-state.json
  execution-log.json
  indexing-status.json
  snapshot-status.json
  indexing-checkpoint.db
  indexing-failure.db

The runner also creates a per-run config sandbox at:
  ~/.ploke-eval/runs/<instance>/config

That sandbox is used so SaveDb writes its registry and snapshot files into
the run directory instead of your normal user config directory.

Debug snapshots:
  `--no-index-debug-snapshots` disables the eval-only DB snapshots written
  during indexing progress and indexing failure events.

Use `--provider <slug>` to pin a specific OpenRouter provider for the selected model.
"
)]
pub struct RunMsbSingleCommand {
    /// Path to a prepared run manifest. Defaults to ~/.ploke-eval/runs/<instance>/run.json.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub run: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/run.json.
    #[arg(long, conflicts_with = "run")]
    pub instance: Option<String>,

    /// Disable eval-only DB checkpoint/failure snapshots during indexing.
    #[arg(long = "no-index-debug-snapshots", action = ArgAction::SetFalse, default_value_t = true)]
    pub index_debug_snapshots: bool,

    /// Use the default model instead of the persisted active model selection.
    #[arg(long)]
    pub use_default_model: bool,

    /// Explicit provider slug to pin for the selected model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,
}

#[derive(Debug, Parser)]
#[command(
    about = "Execute many prepared Multi-SWE-bench runs",
    after_help = "\
Examples:

  cargo run -p ploke-eval -- run-msb-batch --batch-id ripgrep-all
  cargo run -p ploke-eval -- run-msb-batch --batch ~/.ploke-eval/batches/ripgrep-all/batch.json

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

    /// Explicit provider slug to pin for the selected model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Stop the batch after the first per-instance runner failure.
    #[arg(long)]
    pub stop_on_error: bool,
}

#[derive(Debug, Parser)]
#[command(
    about = "Execute one prepared Multi-SWE-bench run and one benchmark issue turn",
    after_help = "\
This extends the normal run with a single agentic turn that:
  - submits the prepared issue prompt through the real app/state path
  - records prompt construction, tool lifecycle, message updates, and turn completion
  - writes a turn trace and summary beside the run artifacts

Use `--provider <slug>` to pin a specific OpenRouter provider for the selected model.
"
)]
pub struct RunMsbAgentSingleCommand {
    /// Path to a prepared run manifest. Defaults to ~/.ploke-eval/runs/<instance>/run.json.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub run: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/run.json.
    #[arg(long, conflicts_with = "run")]
    pub instance: Option<String>,

    /// Disable eval-only DB checkpoint/failure snapshots during indexing.
    #[arg(long = "no-index-debug-snapshots", action = ArgAction::SetFalse, default_value_t = true)]
    pub index_debug_snapshots: bool,

    /// Use the default model instead of the persisted active model selection.
    #[arg(long)]
    pub use_default_model: bool,

    /// Explicit provider slug to pin for the selected model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,
}

#[derive(Debug, Parser)]
#[command(
    about = "Execute many prepared Multi-SWE-bench runs and one benchmark issue turn for each",
    after_help = "\
Examples:

  cargo run -p ploke-eval -- run-msb-agent-batch --batch-id ripgrep-all
  cargo run -p ploke-eval -- run-msb-agent-batch --batch ~/.ploke-eval/batches/ripgrep-all/batch.json

This reuses the per-instance run manifests listed by the batch manifest,
executes one benchmark issue turn per instance, and writes:
  batch-run-summary.json
  multi-swe-bench-submission.jsonl
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

    /// Explicit provider slug to pin for the selected model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Stop the batch after the first per-instance runner failure.
    #[arg(long)]
    pub stop_on_error: bool,
}

#[derive(Debug, Parser)]
#[command(
    about = "Manage the cached OpenRouter model registry and active model selection",
    after_help = "\
Examples:

  cargo run -p ploke-eval -- model refresh
  cargo run -p ploke-eval -- model list
  cargo run -p ploke-eval -- model find qwen
  cargo run -p ploke-eval -- model providers
  cargo run -p ploke-eval -- model provider current
  cargo run -p ploke-eval -- model provider set chutes
  cargo run -p ploke-eval -- model set moonshotai/kimi-k2
  cargo run -p ploke-eval -- model current
"
)]
pub struct ModelCommand {
    #[command(subcommand)]
    pub command: ModelSubcommand,
}

#[derive(Debug, Parser)]
#[command(
    about = "Manage the persisted default provider selection for a model",
    after_help = "\
Examples:

  cargo run -p ploke-eval -- model provider current
  cargo run -p ploke-eval -- model provider set chutes
  cargo run -p ploke-eval -- model provider clear
"
)]
pub struct ProviderCommand {
    #[command(subcommand)]
    pub command: ProviderSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ProviderSubcommand {
    /// Persist the default provider for the current or specified model.
    Set {
        /// Provider slug to remember for the model.
        provider_slug: String,

        /// Model id to update. Defaults to the current active model.
        #[arg(long)]
        model_id: Option<String>,
    },
    /// Show the persisted default provider for the current or specified model.
    Current {
        /// Model id to inspect. Defaults to the current active model.
        #[arg(long)]
        model_id: Option<String>,
    },
    /// Clear the persisted default provider for the current or specified model.
    Clear {
        /// Model id to update. Defaults to the current active model.
        #[arg(long)]
        model_id: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ModelSubcommand {
    /// Download the latest OpenRouter model catalog into the local registry JSON.
    Refresh,
    /// List cached models with context and pricing columns.
    List,
    /// Find models whose id, name, or canonical id matches the stem.
    Find {
        /// Stem or substring to search for.
        query: String,
    },
    #[command(
        about = "List provider endpoints available for a model",
        long_about = "\
Print the OpenRouter provider endpoints returned for a model.

If no model id is passed, the current active eval model is used.
",
        after_help = "\
Examples:

  ploke-eval model providers
  ploke-eval model providers moonshotai/kimi-k2

The output shows provider slug, provider name, tool support, and context length.
"
    )]
    Providers {
        /// Exact model id to inspect. Defaults to the current active model.
        model_id: Option<String>,
    },
    /// Persist or inspect the default provider for a model.
    Provider(ProviderCommand),
    /// Persist the active model selection.
    Set {
        /// Exact model id to mark active.
        model_id: String,
    },
    /// Show the current active model selection.
    Current,
}

impl ModelCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            ModelSubcommand::Refresh => {
                let registry = refresh_model_registry().await?;
                println!("refreshed {} models", registry.data.len());
                Ok(())
            }
            ModelSubcommand::List => {
                let registry = load_model_registry()?;
                let mut items: Vec<_> = registry.data.iter().collect();
                items.sort_by(|a, b| a.id.cmp(&b.id));
                let id_width = items
                    .iter()
                    .map(|item| item.id.to_string().len())
                    .max()
                    .unwrap_or(0)
                    .max("model_id".len());

                println!(
                    "{:<id_width$}  {:>14}  {:>10}  {:>10}  {}",
                    "model_id",
                    "context_length",
                    "in($/M)",
                    "out($/M)",
                    "size",
                    id_width = id_width
                );
                for item in items {
                    let context = display_context_length(item);
                    let input = display_price_per_million(item.pricing.prompt);
                    let output = display_price_per_million(item.pricing.completion);
                    let size = model_size_string(item);
                    println!(
                        "{:<id_width$}  {:>14}  {:>10}  {:>10}  {}",
                        item.id,
                        context,
                        input,
                        output,
                        size,
                        id_width = id_width
                    );
                }
                Ok(())
            }
            ModelSubcommand::Find { query } => {
                let registry = load_model_registry()?;
                let mut matches = find_models(&registry, &query);
                matches.sort_by(|a, b| a.id.cmp(&b.id));
                for item in matches {
                    println!("{}\t{}", item.id, item.name.as_str());
                }
                Ok(())
            }
            ModelSubcommand::Providers { model_id } => print_model_providers(model_id).await,
            ModelSubcommand::Provider(cmd) => cmd.run().await,
            ModelSubcommand::Set { model_id } => {
                let registry = load_model_registry()?;
                let registry_path = crate::model_registry::model_registry_path()?;
                let selected = registry
                    .data
                    .iter()
                    .find(|item| item.id.to_string() == model_id)
                    .ok_or_else(|| PrepareError::UnknownModelInRegistry {
                        model: model_id.clone(),
                        path: registry_path.clone(),
                    })?;
                save_active_model(&selected.id)?;
                println!("{}", selected.id);
                Ok(())
            }
            ModelSubcommand::Current => {
                let active = load_active_model()?;
                match load_model_registry() {
                    Ok(registry) => {
                        if let Some(item) =
                            registry.data.iter().find(|item| item.id == active.model_id)
                        {
                            println!("{}\t{}", item.id, item.name.as_str());
                        } else {
                            println!("{}", active.model_id);
                        }
                    }
                    Err(PrepareError::MissingModelRegistry(_)) => println!("{}", active.model_id),
                    Err(err) => return Err(err),
                }
                Ok(())
            }
        }
    }
}

impl ProviderCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            ProviderSubcommand::Set {
                provider_slug,
                model_id,
            } => set_persisted_provider(model_id, provider_slug).await,
            ProviderSubcommand::Current { model_id } => {
                let (model_id, provider) = current_provider_for_model(model_id)?;
                match provider {
                    Some(provider) => {
                        println!("{}\t{}", model_id, provider.slug.as_str());
                    }
                    None => {
                        println!("{}\tauto", model_id);
                    }
                }
                Ok(())
            }
            ProviderSubcommand::Clear { model_id } => {
                let model = resolve_provider_model_id(model_id)?;
                clear_provider_for_model(&model)?;
                println!("{}\tauto", model);
                Ok(())
            }
        }
    }
}

async fn print_model_providers(model_id: Option<String>) -> Result<(), PrepareError> {
    let model_id = match model_id {
        Some(model_id) => model_id,
        None => load_active_model()?.model_id.to_string(),
    };

    let model = ModelId::from_str(&model_id).map_err(|err| PrepareError::DatabaseSetup {
        phase: "parse_model_id",
        detail: format!("invalid model id '{model_id}': {err}"),
    })?;
    let client = reqwest::Client::new();
    let typed_model = OpenRouterModelId::from(model.clone());
    let endpoints = OpenRouter::fetch_model_endpoints(&client, typed_model)
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "fetch_model_endpoints",
            detail: err.to_string(),
        })?;
    let selected_provider = load_provider_for_model(&model)?;

    println!("Available endpoints for model '{}':", model);
    println!(
        "  {:<14}  {:<14}  {:<5}  {:<8}  {}",
        "provider_slug", "provider_name", "tools", "selected", "context"
    );
    for ep in endpoints.data.endpoints {
        print_provider_row(&ep, selected_provider.as_ref());
    }
    Ok(())
}

fn print_provider_row(ep: &Endpoint, selected_provider: Option<&ProviderKey>) {
    let provider_slug = ep.tag.provider_name.as_str();
    let provider_name = ep.provider_name.as_str();
    let tools = if ep.supports_tools() { "yes" } else { "no" };
    let selected = if selected_provider.is_some_and(|p| p.slug.as_str() == provider_slug) {
        "yes"
    } else {
        ""
    };
    println!(
        "  {:<14}  {:<14}  {:<5}  {:<8}  {:.0}",
        provider_slug, provider_name, tools, selected, ep.context_length
    );
}

fn parse_provider_key(provider: Option<String>) -> Result<Option<ProviderKey>, PrepareError> {
    provider
        .map(|slug| {
            ProviderKey::new(&slug).map_err(|err| PrepareError::DatabaseSetup {
                phase: "parse_provider_key",
                detail: format!("invalid provider slug '{slug}': {err}"),
            })
        })
        .transpose()
}

fn resolve_provider_model_id(model_id: Option<String>) -> Result<ModelId, PrepareError> {
    match model_id {
        Some(model_id) => ModelId::from_str(&model_id).map_err(|err| PrepareError::DatabaseSetup {
            phase: "parse_model_id",
            detail: format!("invalid model id '{model_id}': {err}"),
        }),
        None => Ok(load_active_model()?.model_id),
    }
}

fn current_provider_for_model(
    model_id: Option<String>,
) -> Result<(ModelId, Option<ProviderKey>), PrepareError> {
    let model = resolve_provider_model_id(model_id)?;
    let provider = load_provider_for_model(&model)?;
    Ok((model, provider))
}

async fn set_persisted_provider(
    model_id: Option<String>,
    provider_slug: String,
) -> Result<(), PrepareError> {
    let model = resolve_provider_model_id(model_id)?;
    let registry = load_model_registry()?;
    let selected = registry
        .data
        .into_iter()
        .find(|item| item.id == model)
        .ok_or_else(|| PrepareError::UnknownModelInRegistry {
            model: model.to_string(),
            path: crate::model_registry::model_registry_path()
                .unwrap_or_else(|_| PathBuf::from("<unknown>")),
        })?;

    let provider_key =
        ProviderKey::new(&provider_slug).map_err(|err| PrepareError::DatabaseSetup {
            phase: "parse_provider_key",
            detail: format!("invalid provider slug '{provider_slug}': {err}"),
        })?;
    let validated = resolve_provider_for_model(&selected, Some(&provider_key)).await?;
    set_provider_for_model(&selected.id, validated.clone())?;
    println!("{}\t{}", selected.id, validated.slug.as_str());
    Ok(())
}

#[derive(Debug, Parser)]
#[command(
    about = "Replay one batch from a prepared Multi-SWE-bench run",
    after_help = "\
Example:

  cargo run -p ploke-eval -- replay-msb-batch --instance BurntSushi__ripgrep-2209 --batch 6

This reuses the prepared run manifest and executes only the selected batch.
It writes `replay-batch-<nnn>.json` into the run directory, logs the full node
metadata for that batch, and then runs the normal embed path so any OpenRouter
failure surfaces with the exact snippets in the eval log.
"
)]
pub struct ReplayMsbBatchCommand {
    /// Path to a prepared run manifest. Defaults to ~/.ploke-eval/runs/<instance>/run.json.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub run: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/run.json.
    #[arg(long, conflicts_with = "run")]
    pub instance: Option<String>,

    /// 1-based batch number to replay.
    #[arg(long)]
    pub batch: usize,
}

#[derive(Debug, Parser)]
#[command(
    about = "List all agent conversation turns from a run record",
    after_help = "\
Example:

  cargo run -p ploke-eval -- conversations --instance BurntSushi__ripgrep-2209
  cargo run -p ploke-eval -- conversations --record ~/.ploke-eval/runs/BurntSushi__ripgrep-2209/record.json.gz

Output includes turn number, timestamps, tool call count, and outcome for each turn.
"
)]
pub struct ConversationsCommand {
    /// Path to a run record file (record.json.gz). Defaults to ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = ConversationsOutputFormat::Table)]
    pub format: ConversationsOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ConversationsOutputFormat {
    Table,
    Json,
}

#[derive(Debug, Parser)]
#[command(
    about = "Inspect run and turn data (conversations, tool calls, db snapshots)",
    after_help = "\
Run-level inspection (matches eval-design.md API):

  cargo run -p ploke-eval -- inspect conversations --instance BurntSushi__ripgrep-2209
  cargo run -p ploke-eval -- inspect tool-calls --instance BurntSushi__ripgrep-2209
  cargo run -p ploke-eval -- inspect db-snapshots --instance BurntSushi__ripgrep-2209
  cargo run -p ploke-eval -- inspect failures --instance BurntSushi__ripgrep-2209
  cargo run -p ploke-eval -- inspect config --instance BurntSushi__ripgrep-2209

Turn-level inspection:

  cargo run -p ploke-eval -- inspect turn --instance BurntSushi__ripgrep-2209 --turn 1
  cargo run -p ploke-eval -- inspect turn --instance BurntSushi__ripgrep-2209 --turn 1 --show messages

Bootstrap questions:

  cargo run -p ploke-eval -- inspect turn --instance BurntSushi__ripgrep-2209 --turn 1 --show db-state
  cargo run -p ploke-eval -- inspect query --instance BurntSushi__ripgrep-2209 --turn 1 --lookup GlobSet
  cargo run -p ploke-eval -- inspect conversations --instance BurntSushi__ripgrep-2209
"
)]
pub struct InspectCommand {
    #[command(subcommand)]
    pub command: InspectSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum InspectSubcommand {
    /// List all agent conversation turns (run.conversations())
    Conversations(InspectConversationsCommand),
    /// List all tool calls from all turns (run.tool_calls())
    ToolCalls(InspectToolCallsCommand),
    /// List DB snapshots at each turn boundary (run.db_snapshots())
    DbSnapshots(InspectDbSnapshotsCommand),
    /// List turns with error outcomes (run.failures())
    Failures(InspectFailuresCommand),
    /// Show run configuration (run.config())
    Config(InspectConfigCommand),
    /// Inspect a specific turn (turn-level inspection)
    Turn(InspectTurnCommand),
    /// Run Cozo queries against historical DB snapshots at turn timestamps
    Query(InspectQueryCommand),
}

#[derive(Debug, Parser)]
pub struct InspectConversationsCommand {
    /// Path to a run record file (record.json.gz). Defaults to ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct InspectToolCallsCommand {
    /// Path to a run record file (record.json.gz). Defaults to ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct InspectDbSnapshotsCommand {
    /// Path to a run record file (record.json.gz). Defaults to ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct InspectFailuresCommand {
    /// Path to a run record file (record.json.gz). Defaults to ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct InspectConfigCommand {
    /// Path to a run record file (record.json.gz). Defaults to ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct InspectTurnCommand {
    /// Path to a run record file (record.json.gz). Defaults to ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Turn number (1-indexed) to inspect.
    #[arg(long)]
    pub turn: u32,

    /// What to show for this turn: all, messages, tool-calls, tool-call, tool-result, db-state.
    #[arg(long, value_enum, default_value_t = TurnShowOption::All)]
    pub show: TurnShowOption,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Tool call index (0-based) when showing specific tool-call or tool-result.
    #[arg(long)]
    pub index: Option<usize>,
}

#[derive(Debug, Parser)]
#[command(
    about = "Run Cozo queries against historical DB snapshots at turn timestamps",
    after_help = "\
Examples:

  # Query using turn number (gets timestamp from turn)
  cargo run -p ploke-eval -- inspect query --instance BurntSushi__ripgrep-2209 --turn 1 '?[name] := *function{name}'

  # Query using explicit timestamp
  cargo run -p ploke-eval -- inspect query --instance BurntSushi__ripgrep-2209 --timestamp 1775963199624424 '?[name] := *function{name}'

  # Convenience: lookup by name (uses db_state.lookup())
  cargo run -p ploke-eval -- inspect query --instance BurntSushi__ripgrep-2209 --turn 1 --lookup GlobSet
"
)]
pub struct InspectQueryCommand {
    /// Path to a run record file (record.json.gz). Defaults to ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Turn number (1-indexed) to get timestamp from. Conflicts with --timestamp.
    #[arg(long, conflicts_with = "timestamp")]
    pub turn: Option<u32>,

    /// Explicit timestamp in microseconds. Conflicts with --turn.
    #[arg(long, conflicts_with = "turn")]
    pub timestamp: Option<i64>,

    /// Convenience: lookup a symbol by name using db_state.lookup().
    #[arg(long, conflicts_with = "query")]
    pub lookup: Option<String>,

    /// Cozo query string. Optional if --lookup is provided.
    #[arg(conflicts_with = "lookup")]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum InspectOutputFormat {
    Table,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TurnShowOption {
    All,
    Messages,
    ToolCalls,
    ToolCall,
    ToolResult,
    DbState,
}

impl RunMsbSingleCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let run_manifest = match (self.run, self.instance) {
            (Some(path), None) => path,
            (None, Some(instance)) => runs_dir()?.join(instance).join("run.json"),
            _ => {
                return Err(PrepareError::MissingRunManifest(
                    runs_dir()?.join("<instance>/run.json"),
                ));
            }
        };

        let artifacts = RunMsbSingleRequest {
            run_manifest,
            index_debug_snapshots: self.index_debug_snapshots,
            use_default_model: self.use_default_model,
            provider: parse_provider_key(self.provider)?,
        }
        .run()
        .await?;
        println!("{}", artifacts.execution_log.display());
        if let Some(path) = artifacts.msb_submission {
            println!("{}", path.display());
        }
        Ok(())
    }
}

impl RunMsbBatchCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let batch_manifest = resolve_batch_manifest(self.batch, self.batch_id)?;
        let artifacts = RunMsbBatchRequest {
            batch_manifest,
            index_debug_snapshots: self.index_debug_snapshots,
            use_default_model: self.use_default_model,
            provider: parse_provider_key(self.provider)?,
            stop_on_error: self.stop_on_error,
        }
        .run()
        .await?;
        println!("{}", artifacts.summary.display());
        if let Some(path) = artifacts.msb_submission {
            println!("{}", path.display());
        }
        Ok(())
    }
}

impl RunMsbAgentSingleCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let run_manifest = match (self.run, self.instance) {
            (Some(path), None) => path,
            (None, Some(instance)) => runs_dir()?.join(instance).join("run.json"),
            _ => {
                return Err(PrepareError::MissingRunManifest(
                    runs_dir()?.join("<instance>/run.json"),
                ));
            }
        };

        let artifacts = RunMsbAgentSingleRequest {
            run_manifest,
            index_debug_snapshots: self.index_debug_snapshots,
            use_default_model: self.use_default_model,
            provider: parse_provider_key(self.provider)?,
        }
        .run()
        .await?;
        println!("{}", artifacts.base.execution_log.display());
        println!("{}", artifacts.turn_summary.display());
        if let Some(path) = artifacts.base.msb_submission {
            println!("{}", path.display());
        }
        Ok(())
    }
}

impl RunMsbAgentBatchCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let batch_manifest = resolve_batch_manifest(self.batch, self.batch_id)?;
        let artifacts = RunMsbAgentBatchRequest {
            batch_manifest,
            index_debug_snapshots: self.index_debug_snapshots,
            use_default_model: self.use_default_model,
            provider: parse_provider_key(self.provider)?,
            stop_on_error: self.stop_on_error,
        }
        .run()
        .await?;
        println!("{}", artifacts.summary.display());
        if let Some(path) = artifacts.msb_submission {
            println!("{}", path.display());
        }
        Ok(())
    }
}

impl ReplayMsbBatchCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let run_manifest = match (self.run, self.instance) {
            (Some(path), None) => path,
            (None, Some(instance)) => runs_dir()?.join(instance).join("run.json"),
            _ => {
                return Err(PrepareError::MissingRunManifest(
                    runs_dir()?.join("<instance>/run.json"),
                ));
            }
        };

        ReplayMsbBatchRequest {
            run_manifest,
            batch_number: self.batch,
        }
        .run()
        .await
        .map(|batch_file| {
            println!("{}", batch_file.display());
        })
    }
}

impl ConversationsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let record_path = match (self.record, self.instance) {
            (Some(path), None) => path,
            (None, Some(instance)) => runs_dir()?.join(instance).join("record.json.gz"),
            _ => {
                return Err(PrepareError::MissingRunManifest(
                    runs_dir()?.join("<instance>/record.json.gz"),
                ));
            }
        };

        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        match self.format {
            ConversationsOutputFormat::Table => {
                println!(
                    "{:<6} {:<24} {:<24} {:<12} {}",
                    "Turn", "Started", "Ended", "Tools", "Outcome"
                );
                println!("{}", "-".repeat(80));
                for turn in record.conversations() {
                    let tool_count = turn.tool_calls().len();
                    let outcome_str = match &turn.outcome {
                        crate::record::TurnOutcome::ToolCalls { count } => {
                            format!("tool_calls({})", count)
                        }
                        crate::record::TurnOutcome::Content => "content".to_string(),
                        crate::record::TurnOutcome::Error { message } => {
                            format!("error: {}", message.chars().take(40).collect::<String>())
                        }
                        crate::record::TurnOutcome::Timeout { elapsed_secs } => {
                            format!("timeout({}s)", elapsed_secs)
                        }
                    };
                    println!(
                        "{:<6} {:<24} {:<24} {:<12} {}",
                        turn.turn_number,
                        turn.started_at.chars().take(23).collect::<String>(),
                        turn.ended_at.chars().take(23).collect::<String>(),
                        tool_count,
                        outcome_str
                    );
                }
                println!("\nTotal turns: {}", record.conversations().count());
            }
            ConversationsOutputFormat::Json => {
                let turns: Vec<_> = record.conversations().collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&turns).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
    }
}

impl InspectCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            InspectSubcommand::Conversations(cmd) => cmd.run().await,
            InspectSubcommand::ToolCalls(cmd) => cmd.run().await,
            InspectSubcommand::DbSnapshots(cmd) => cmd.run().await,
            InspectSubcommand::Failures(cmd) => cmd.run().await,
            InspectSubcommand::Config(cmd) => cmd.run().await,
            InspectSubcommand::Turn(cmd) => cmd.run().await,
            InspectSubcommand::Query(cmd) => cmd.run().await,
        }
    }
}

impl InspectConversationsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let record_path = resolve_record_path(self.record, self.instance)?;
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        match self.format {
            InspectOutputFormat::Table => {
                println!(
                    "{:<6} {:<24} {:<24} {:<12} {}",
                    "Turn", "Started", "Ended", "Tools", "Outcome"
                );
                println!("{}", "-".repeat(80));
                for turn in record.conversations() {
                    let tool_count = turn.tool_calls().len();
                    let outcome_str = match &turn.outcome {
                        crate::record::TurnOutcome::ToolCalls { count } => {
                            format!("tool_calls({})", count)
                        }
                        crate::record::TurnOutcome::Content => "content".to_string(),
                        crate::record::TurnOutcome::Error { message } => {
                            format!("error: {}", message.chars().take(40).collect::<String>())
                        }
                        crate::record::TurnOutcome::Timeout { elapsed_secs } => {
                            format!("timeout({}s)", elapsed_secs)
                        }
                    };
                    println!(
                        "{:<6} {:<24} {:<24} {:<12} {}",
                        turn.turn_number,
                        turn.started_at.chars().take(23).collect::<String>(),
                        turn.ended_at.chars().take(23).collect::<String>(),
                        tool_count,
                        outcome_str
                    );
                }
                println!("\nTotal turns: {}", record.conversations().count());
            }
            InspectOutputFormat::Json => {
                let turns: Vec<_> = record.conversations().collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&turns).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
    }
}

impl InspectToolCallsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let record_path = resolve_record_path(self.record, self.instance)?;
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let tool_calls = record.tool_calls();

        match self.format {
            InspectOutputFormat::Table => {
                println!(
                    "{:<6} {:<20} {:<40} {}",
                    "Turn", "Tool", "Arguments", "Result"
                );
                println!("{}", "-".repeat(100));
                for (turn, call) in record
                    .conversations()
                    .flat_map(|t| t.tool_calls().into_iter().map(move |c| (t.turn_number, c)))
                {
                    let args_preview = call.request.arguments.chars().take(37).collect::<String>();
                    let result_str = match &call.result {
                        crate::record::ToolResult::Completed(_) => "completed".to_string(),
                        crate::record::ToolResult::Failed(f) => {
                            format!("failed: {}", f.error.chars().take(30).collect::<String>())
                        }
                    };
                    println!(
                        "{:<6} {:<20} {:<40} {}",
                        turn,
                        call.request.tool.chars().take(18).collect::<String>(),
                        if args_preview.len() >= 37 {
                            format!("{}...", args_preview)
                        } else {
                            args_preview
                        },
                        result_str
                    );
                }
                println!("\nTotal tool calls: {}", tool_calls.len());
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&tool_calls).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
    }
}

impl InspectDbSnapshotsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let record_path = resolve_record_path(self.record, self.instance)?;
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let snapshots = record.db_snapshots();

        match self.format {
            InspectOutputFormat::Table => {
                println!("{:<6} {}", "Turn", "DB Timestamp (micros)");
                println!("{}", "-".repeat(40));
                for (i, snapshot) in snapshots.iter().enumerate() {
                    println!("{:<6} {}", i + 1, snapshot.timestamp_micros());
                }
                println!("\nTotal snapshots: {}", snapshots.len());
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&snapshots).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
    }
}

impl InspectFailuresCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let record_path = resolve_record_path(self.record, self.instance)?;
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let failures = record.failures();

        match self.format {
            InspectOutputFormat::Table => {
                if failures.is_empty() {
                    println!("No failures found.");
                } else {
                    println!("{:<6} {:<24} {}", "Turn", "Started", "Error");
                    println!("{}", "-".repeat(80));
                    for turn in &failures {
                        let error_str = match &turn.outcome {
                            crate::record::TurnOutcome::Error { message } => {
                                message.chars().take(50).collect::<String>()
                            }
                            _ => "unknown".to_string(),
                        };
                        println!(
                            "{:<6} {:<24} {}",
                            turn.turn_number,
                            turn.started_at.chars().take(23).collect::<String>(),
                            error_str
                        );
                    }
                    println!("\nTotal failures: {}", failures.len());
                }
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&failures).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
    }
}

impl InspectConfigCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let record_path = resolve_record_path(self.record, self.instance)?;
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let config = record.config();

        match self.format {
            InspectOutputFormat::Table => {
                println!("Run Configuration");
                println!("{}", "-".repeat(40));
                println!("Instance ID: {}", config.benchmark.instance_id);
                println!("Repository: {}", config.benchmark.repo_root.display());
                if let Some(sha) = &config.benchmark.base_sha {
                    println!("Base SHA: {}", sha);
                }
                println!("Max Turns: {}", config.budget.max_turns);
                println!("Max Tool Calls: {}", config.budget.max_tool_calls);
                println!("Wall Clock (secs): {}", config.budget.wall_clock_secs);
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(config).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
    }
}

impl InspectTurnCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let record_path = resolve_record_path(self.record, self.instance)?;
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let turn = record
            .turn_record(self.turn)
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "inspect_turn",
                detail: format!("Turn {} not found", self.turn),
            })?;

        match self.show {
            TurnShowOption::All => {
                // Show comprehensive turn info
                println!("Turn {}", turn.turn_number);
                println!("{}", "-".repeat(40));
                println!("Started: {}", turn.started_at);
                println!("Ended: {}", turn.ended_at);
                println!("DB Timestamp: {}", turn.db_timestamp_micros);
                println!("Messages: {}", turn.messages().len());
                println!("Tool Calls: {}", turn.tool_calls().len());
                println!("Outcome: {:?}", turn.outcome);
            }
            TurnShowOption::Messages => {
                println!("{}", render_messages_json(&turn.messages())?);
            }
            TurnShowOption::ToolCalls => {
                let tool_calls = turn.tool_calls();
                if tool_calls.is_empty() {
                    println!("No tool calls in this turn.");
                } else {
                    match self.format {
                        InspectOutputFormat::Table => {
                            println!("{:<20} {:<40} {}", "Tool", "Arguments", "Result");
                            println!("{}", "-".repeat(80));
                            for call in &tool_calls {
                                let args_preview =
                                    call.request.arguments.chars().take(37).collect::<String>();
                                let result_str = match &call.result {
                                    crate::record::ToolResult::Completed(_) => {
                                        "completed".to_string()
                                    }
                                    crate::record::ToolResult::Failed(f) => format!(
                                        "failed: {}",
                                        f.error.chars().take(20).collect::<String>()
                                    ),
                                };
                                println!(
                                    "{:<20} {:<40} {}",
                                    call.request.tool.chars().take(18).collect::<String>(),
                                    if args_preview.len() >= 37 {
                                        format!("{}...", args_preview)
                                    } else {
                                        args_preview
                                    },
                                    result_str
                                );
                            }
                        }
                        InspectOutputFormat::Json => {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&tool_calls)
                                    .map_err(PrepareError::Serialize)?
                            );
                        }
                    }
                }
            }
            TurnShowOption::ToolCall => {
                let tool_calls = turn.tool_calls();
                match (self.index, tool_calls.len()) {
                    (Some(idx), len) if idx < len => {
                        let tool_call = &tool_calls[idx];
                        match self.format {
                            InspectOutputFormat::Table => {
                                println!("Tool: {}", tool_call.request.tool);
                                println!("Arguments: {}", tool_call.request.arguments);
                                println!("Result: {:?}", tool_call.result);
                            }
                            InspectOutputFormat::Json => {
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&tool_call)
                                        .map_err(PrepareError::Serialize)?
                                );
                            }
                        }
                    }
                    (Some(idx), len) => {
                        println!(
                            "Error: Index {} out of range. Turn has {} tool call{} (valid indices: 0..{}).",
                            idx,
                            len,
                            if len == 1 { "" } else { "s" },
                            len.saturating_sub(1)
                        );
                    }
                    (None, 1) => {
                        let tool_call = &tool_calls[0];
                        match self.format {
                            InspectOutputFormat::Table => {
                                println!("Tool: {}", tool_call.request.tool);
                                println!("Arguments: {}", tool_call.request.arguments);
                                println!("Result: {:?}", tool_call.result);
                            }
                            InspectOutputFormat::Json => {
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&tool_call)
                                        .map_err(PrepareError::Serialize)?
                                );
                            }
                        }
                    }
                    (None, len) => {
                        println!(
                            "Turn has {} tool call{}. Use --index 0..{} to select one.",
                            len,
                            if len == 1 { "" } else { "s" },
                            len.saturating_sub(1)
                        );
                    }
                }
            }
            TurnShowOption::ToolResult => {
                let tool_calls = turn.tool_calls();
                match (self.index, tool_calls.len()) {
                    (Some(idx), len) if idx < len => {
                        let result = &tool_calls[idx].result;
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&result)
                                .map_err(PrepareError::Serialize)?
                        );
                    }
                    (Some(idx), len) => {
                        println!(
                            "Error: Index {} out of range. Turn has {} tool call{} (valid indices: 0..{}).",
                            idx,
                            len,
                            if len == 1 { "" } else { "s" },
                            len.saturating_sub(1)
                        );
                    }
                    (None, 1) => {
                        let result = &tool_calls[0].result;
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&result)
                                .map_err(PrepareError::Serialize)?
                        );
                    }
                    (None, len) => {
                        println!(
                            "Turn has {} tool call{}. Use --index 0..{} to select one.",
                            len,
                            if len == 1 { "" } else { "s" },
                            len.saturating_sub(1)
                        );
                    }
                }
            }
            TurnShowOption::DbState => {
                let db_state = turn.db_state();
                println!("DB State for Turn {}", turn.turn_number);
                println!("Timestamp (micros): {}", db_state.timestamp_micros());
                println!(
                    "\nUse this timestamp with 'inspect query' to run queries against this state."
                );
            }
        }

        Ok(())
    }
}

impl InspectQueryCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        // Validate that we have either --turn or --timestamp
        if self.turn.is_none() && self.timestamp.is_none() {
            return Err(PrepareError::DatabaseSetup {
                phase: "inspect_query",
                detail: "Either --turn or --timestamp must be provided".to_string(),
            });
        }

        // Validate that we have either --lookup or query string
        if self.lookup.is_none() && self.query.is_none() {
            return Err(PrepareError::DatabaseSetup {
                phase: "inspect_query",
                detail: "Either --lookup <name> or a query string must be provided".to_string(),
            });
        }

        // Load the record
        let record_path = resolve_record_path(self.record, self.instance)?;
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        // Resolve timestamp
        let timestamp_micros = if let Some(turn) = self.turn {
            record
                .timestamp_for_turn(turn)
                .ok_or_else(|| PrepareError::DatabaseSetup {
                    phase: "inspect_query",
                    detail: format!("Turn {} not found in record", turn),
                })?
        } else {
            self.timestamp.unwrap() // Safe because we validated above
        };

        // Find DB path: look in run directory for final-snapshot.db first, fallback to indexing-checkpoint.db
        let run_dir = record_path
            .parent()
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "inspect_query",
                detail: "Could not determine run directory from record path".to_string(),
            })?;

        let final_snapshot = run_dir.join("final-snapshot.db");
        let checkpoint_db = run_dir.join("indexing-checkpoint.db");

        let db_path = if final_snapshot.exists() {
            final_snapshot
        } else if checkpoint_db.exists() {
            checkpoint_db
        } else {
            return Err(PrepareError::DatabaseSetup {
                phase: "inspect_query",
                detail: format!(
                    "No DB snapshot found in {}. Looked for final-snapshot.db and indexing-checkpoint.db",
                    run_dir.display()
                ),
            });
        };

        // Open the database (async method)
        let db = ploke_db::Database::create_new_backup_default(&db_path)
            .await
            .map_err(|e| PrepareError::DatabaseSetup {
                phase: "inspect_query",
                detail: format!("Failed to open database at {}: {}", db_path.display(), e),
            })?;

        // Create DbState with the timestamp
        let db_state = crate::record::DbState::new(timestamp_micros);

        // Execute query or lookup
        if let Some(name) = self.lookup {
            // Use db_state.lookup() for name-based lookup
            match db_state.lookup(&db, &name) {
                Ok(Some(node_info)) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&node_info)
                            .map_err(PrepareError::Serialize)?
                    );
                }
                Ok(None) => {
                    println!(
                        "Symbol '{}' not found at timestamp {}",
                        name, timestamp_micros
                    );
                }
                Err(e) => {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "inspect_query",
                        detail: format!("Lookup failed: {}", e),
                    });
                }
            }
        } else if let Some(query) = self.query {
            // Use db_state.query() for raw Cozo queries
            match db_state.query(&db, &query) {
                Ok(result) => {
                    // Convert QueryResult to JSON-serializable format
                    let rows: Vec<Vec<serde_json::Value>> = result
                        .rows
                        .iter()
                        .map(|row| row.iter().map(|val| cozo_data_to_json(val)).collect())
                        .collect();

                    let output = serde_json::json!({
                        "headers": result.headers,
                        "rows": rows,
                    });
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&output).map_err(PrepareError::Serialize)?
                    );
                }
                Err(e) => {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "inspect_query",
                        detail: format!("Query failed: {}", e),
                    });
                }
            }
        }

        Ok(())
    }
}

/// Convert a Cozo DataValue to a JSON Value
fn cozo_data_to_json(val: &cozo::DataValue) -> serde_json::Value {
    use cozo::DataValue;

    match val {
        DataValue::Null => serde_json::Value::Null,
        DataValue::Str(s) => serde_json::Value::String(s.to_string()),
        DataValue::Bytes(b) => serde_json::Value::String(format!("{:?}", b)),
        DataValue::Uuid(u) => serde_json::Value::String(u.0.to_string()),
        DataValue::Num(n) => match n {
            cozo::Num::Int(i) => serde_json::Value::Number((*i).into()),
            cozo::Num::Float(f) => serde_json::Value::Number(
                serde_json::Number::from_f64(*f).unwrap_or(serde_json::Number::from(0)),
            ),
        },
        DataValue::Bool(b) => serde_json::Value::Bool(*b),
        DataValue::List(l) => serde_json::Value::Array(l.iter().map(cozo_data_to_json).collect()),
        DataValue::Set(s) => serde_json::Value::Array(s.iter().map(cozo_data_to_json).collect()),
        DataValue::Vec(v) => {
            // Vec is an embedding vector - convert to array of floats
            let vec_values: Vec<serde_json::Value> = match v {
                cozo::Vector::F32(f32_vec) => f32_vec
                    .iter()
                    .map(|f| {
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(*f as f64)
                                .unwrap_or(serde_json::Number::from(0)),
                        )
                    })
                    .collect(),
                cozo::Vector::F64(f64_vec) => f64_vec
                    .iter()
                    .map(|f| {
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(*f).unwrap_or(serde_json::Number::from(0)),
                        )
                    })
                    .collect(),
            };
            serde_json::Value::Array(vec_values)
        }
        DataValue::Validity(v) => serde_json::json!({
            "type": "validity",
            "timestamp": v.timestamp,
        }),
        // Handle remaining variants with a catch-all
        other => serde_json::json!({
            "type": "unsupported",
            "debug": format!("{:?}", other),
        }),
    }
}

fn render_messages_json(
    messages: &[crate::record::ConversationMessage],
) -> Result<String, PrepareError> {
    serde_json::to_string_pretty(messages).map_err(PrepareError::Serialize)
}

fn resolve_record_path(
    record: Option<PathBuf>,
    instance: Option<String>,
) -> Result<PathBuf, PrepareError> {
    match (record, instance) {
        (Some(path), None) => Ok(path),
        (None, Some(instance)) => Ok(runs_dir()?.join(instance).join("record.json.gz")),
        _ => Err(PrepareError::MissingRunManifest(
            runs_dir()?.join("<instance>/record.json.gz"),
        )),
    }
}

fn resolve_batch_manifest(
    batch: Option<PathBuf>,
    batch_id: Option<String>,
) -> Result<PathBuf, PrepareError> {
    match (batch, batch_id) {
        (Some(path), None) => Ok(path),
        (None, Some(batch_id)) => Ok(batches_dir()?.join(batch_id).join("batch.json")),
        _ => Err(PrepareError::MissingBatchManifest(
            batches_dir()?.join("<batch-id>/batch.json"),
        )),
    }
}

fn default_batch_id(
    dataset_key: Option<&str>,
    dataset: Option<&PathBuf>,
    select_all: bool,
    instances: &[String],
    specifics: &[String],
) -> String {
    let dataset_stem = dataset_key
        .map(str::to_string)
        .or_else(|| {
            dataset
                .and_then(|path| path.file_stem())
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "msb".to_string());
    let selector = if select_all {
        "all".to_string()
    } else if instances.len() == 1 && specifics.is_empty() {
        sanitize_batch_component(&instances[0])
    } else if specifics.len() == 1 && instances.is_empty() {
        sanitize_batch_component(&specifics[0])
    } else {
        format!("selection-{}", instances.len() + specifics.len())
    };
    format!("{}-{}", sanitize_batch_component(&dataset_stem), selector)
}

fn sanitize_batch_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_dash = false;
    for ch in input.chars() {
        let normalized = if ch.is_ascii_alphanumeric() { ch } else { '-' };
        if normalized == '-' {
            if last_was_dash {
                continue;
            }
            last_was_dash = true;
            out.push('-');
        } else {
            last_was_dash = false;
            out.push(normalized.to_ascii_lowercase());
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "batch".to_string()
    } else {
        trimmed.to_string()
    }
}

fn run_doctor() -> Result<(), PrepareError> {
    let mut ok = 0usize;
    let mut warn = 0usize;
    let mut note = 0usize;

    println!("ploke-eval doctor");
    println!();

    let home = crate::layout::ploke_eval_home()?;
    println!("home: {}", home.display());

    let builtins = builtin_dataset_registry_entries();
    println!(
        "built-in datasets: {}{}",
        builtins.len(),
        if builtins.is_empty() {
            String::new()
        } else {
            format!(
                " ({})",
                builtins
                    .iter()
                    .map(|entry| entry.key)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    );

    check_dir(
        "datasets dir",
        datasets_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "models dir",
        models_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "repo cache dir",
        repos_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "run artifacts dir",
        runs_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "batch artifacts dir",
        batches_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Note,
    );
    check_dir(
        "starting-db cache dir",
        starting_db_cache_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Note,
    );
    check_dir(
        "cache dir",
        cache_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Note,
    );

    match load_model_registry() {
        Ok(registry) => {
            ok += 1;
            println!(
                "[ok] model registry: {} models ({})",
                registry.data.len(),
                model_registry_file()?.display()
            );
        }
        Err(PrepareError::MissingModelRegistry(path)) => {
            warn += 1;
            println!("[warn] model registry: missing ({})", path.display());
            print_advice(&[
                "cargo run -p ploke-eval -- model refresh",
                "cargo run -p ploke-eval -- model list",
            ]);
        }
        Err(err) => return Err(err),
    }

    match load_active_model() {
        Ok(active) => match load_model_registry() {
            Ok(registry) => {
                if registry_has_model(&registry, &active.model_id) {
                    ok += 1;
                    println!(
                        "[ok] active model: {} ({})",
                        active.model_id,
                        active_model_file()?.display()
                    );
                } else {
                    warn += 1;
                    println!(
                        "[warn] active model: {} is not present in the current registry ({})",
                        active.model_id,
                        active_model_file()?.display()
                    );
                    print_advice(&[
                        "cargo run -p ploke-eval -- model refresh",
                        "cargo run -p ploke-eval -- model set <model_id>",
                    ]);
                }
            }
            Err(PrepareError::MissingModelRegistry(_)) => {
                warn += 1;
                println!(
                    "[warn] active model: {} ({})",
                    active.model_id,
                    active_model_file()?.display()
                );
            }
            Err(err) => return Err(err),
        },
        Err(PrepareError::MissingActiveModel(path)) => {
            warn += 1;
            println!("[warn] active model: missing ({})", path.display());
            print_advice(&[
                "cargo run -p ploke-eval -- model refresh",
                "cargo run -p ploke-eval -- model set <model_id>",
            ]);
        }
        Err(err) => return Err(err),
    }

    match OpenRouter::resolve_api_key() {
        Ok(_) => {
            ok += 1;
            println!("[ok] OpenRouter API key: present");
        }
        Err(err) => {
            warn += 1;
            println!("[warn] OpenRouter API key: unavailable ({err})");
        }
    }

    match std::process::Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            ok += 1;
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("[ok] git: {version}");
        }
        Ok(output) => {
            warn += 1;
            println!(
                "[warn] git: command exited with status {}",
                output.status.code().unwrap_or(-1)
            );
        }
        Err(err) => {
            warn += 1;
            println!("[warn] git: unavailable ({err})");
        }
    }

    println!();
    println!(
        "summary: {ok} ok, {warn} warning{}, {note} note{}",
        if warn == 1 { "" } else { "s" },
        if note == 1 { "" } else { "s" }
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum MissingDirStatus {
    Warn,
    Note,
}

fn check_dir(
    label: &str,
    path: PathBuf,
    ok: &mut usize,
    warn: &mut usize,
    note: &mut usize,
    missing_status: MissingDirStatus,
) {
    if path.exists() {
        if path.is_dir() {
            *ok += 1;
            println!("[ok] {label}: {}", path.display());
        } else {
            *warn += 1;
            println!(
                "[warn] {label}: exists but is not a directory ({})",
                path.display()
            );
        }
    } else {
        match missing_status {
            MissingDirStatus::Warn => {
                *warn += 1;
                println!("[warn] {label}: missing ({})", path.display());
            }
            MissingDirStatus::Note => {
                *note += 1;
                println!("[note] {label}: missing ({})", path.display());
            }
        }
    }
}

fn print_advice(lines: &[&str]) {
    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 {
            println!("  next: {line}");
        } else {
            println!("  then: {line}");
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    about = "Clone or refresh one built-in benchmark repo under ~/.ploke-eval/repos",
    after_help = "\
Example:

  cargo run -p ploke-eval -- fetch-msb-repo --dataset-key ripgrep

Default destination:
  ~/.ploke-eval/repos/<org>/<repo>
"
)]
pub struct FetchMsbRepoCommand {
    /// Built-in dataset registry key, for example ripgrep.
    #[arg(long)]
    pub dataset_key: String,
}

impl FetchMsbRepoCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let entry = builtin_dataset_registry_entry(&self.dataset_key)
            .ok_or_else(|| PrepareError::UnknownDatasetKey(self.dataset_key.clone()))?;

        let repo_root = workspace_root_for_key(&self.dataset_key)?;
        let parent = repo_root
            .parent()
            .expect("repo root built from repos_dir/org/repo always has a parent");
        std::fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;

        if repo_root.join(".git").exists() {
            run_git(
                &[
                    "-C",
                    repo_root.to_string_lossy().as_ref(),
                    "fetch",
                    "--all",
                    "--tags",
                    "--prune",
                ],
                format!("git -C {} fetch --all --tags --prune", repo_root.display()),
            )?;
        } else {
            run_git(
                &[
                    "clone",
                    entry.clone_url().as_str(),
                    repo_root.to_string_lossy().as_ref(),
                ],
                format!("git clone {} {}", entry.clone_url(), repo_root.display()),
            )?;
        }

        println!("{}", repo_root.display());
        Ok(())
    }
}

fn run_git(args: &[&str], command_label: String) -> Result<(), PrepareError> {
    let status = std::process::Command::new("git")
        .args(args)
        .status()
        .map_err(|source| PrepareError::GitCommand {
            command: command_label.clone(),
            source,
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(PrepareError::GitCommandStatus {
            command: command_label,
            status: status.code().unwrap_or(-1),
        })
    }
}

fn display_context_length(item: &ploke_llm::request::models::ResponseItem) -> String {
    item.context_length
        .or(item.top_provider.context_length)
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn display_price_per_million(value: f64) -> String {
    format!("${:.2}/M", value * 1_000_000.0)
}

fn model_size_string(item: &ploke_llm::request::models::ResponseItem) -> String {
    extract_model_size(item.description.as_ref())
        .or_else(|| extract_model_size(item.name.as_str()))
        .unwrap_or_default()
}

fn extract_model_size(text: &str) -> Option<String> {
    static MIXTURE_RE: OnceLock<Regex> = OnceLock::new();
    static BILLION_PARAMS_RE: OnceLock<Regex> = OnceLock::new();
    static MILLION_PARAMS_RE: OnceLock<Regex> = OnceLock::new();
    static SUFFIX_RE: OnceLock<Regex> = OnceLock::new();

    let mix_match = MIXTURE_RE
        .get_or_init(|| Regex::new(r"(?i)\b\d+x\d+(?:\.\d+)?[BM]\b").expect("valid regex"))
        .find(text)
        .map(|m| m.as_str().to_string());
    if mix_match.is_some() {
        return mix_match;
    }

    if let Some(caps) = BILLION_PARAMS_RE
        .get_or_init(|| {
            Regex::new(r"(?i)\b(\d+(?:\.\d+)?)\s*billion\s+parameters?\b").expect("valid regex")
        })
        .captures(text)
    {
        return Some(format!("{}B", &caps[1]));
    }

    if let Some(caps) = MILLION_PARAMS_RE
        .get_or_init(|| {
            Regex::new(r"(?i)\b(\d+(?:\.\d+)?)\s*million\s+parameters?\b").expect("valid regex")
        })
        .captures(text)
    {
        return Some(format!("{}M", &caps[1]));
    }

    SUFFIX_RE
        .get_or_init(|| Regex::new(r"(?i)\b\d+(?:\.\d+)?[BM]\b").expect("valid regex"))
        .find(text)
        .map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_size_from_parameter_phrase() {
        let text = "Cogito v2 is a multilingual, instruction-tuned Mixture of Experts (MoE) large language model with 671 billion parameters.";
        assert_eq!(extract_model_size(text), Some("671B".to_string()));
    }

    #[test]
    fn extracts_size_from_suffix_notation() {
        let text = "Meta's latest class of model (Llama 3.1) launched with a variety of sizes & flavors. This 405B instruct-tuned version is optimized for high quality dialogue usecases.";
        assert_eq!(extract_model_size(text), Some("405B".to_string()));
    }

    #[test]
    fn formats_pricing_per_million_tokens() {
        assert_eq!(display_price_per_million(0.00000018), "$0.18/M");
        assert_eq!(display_price_per_million(0.00000059), "$0.59/M");
    }

    #[test]
    fn default_batch_id_uses_dataset_key_and_specific() {
        let batch_id = default_batch_id(Some("ripgrep"), None, false, &[], &["2209".to_string()]);
        assert_eq!(batch_id, "ripgrep-2209");
    }

    #[test]
    fn sanitize_batch_component_collapses_non_alnum_runs() {
        assert_eq!(
            sanitize_batch_component("BurntSushi/ripgrep:pr-2209"),
            "burntsushi-ripgrep-pr-2209"
        );
    }

    #[test]
    fn render_messages_json_renders_empty_lists_as_json_array() {
        let messages: &[crate::record::ConversationMessage] = &[];
        assert_eq!(render_messages_json(messages).expect("render should succeed"), "[]");
    }
}
