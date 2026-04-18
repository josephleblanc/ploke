use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::OnceLock;

use clap::{ArgAction, Parser, Subcommand};
use ploke_llm::Router;
use ploke_llm::request::endpoint::Endpoint;
use ploke_llm::router_only::HasEndpoint;
use ploke_llm::router_only::openrouter::{OpenRouter, OpenRouterModelId};
use ploke_llm::{ModelId, ProviderKey};
use ploke_protocol::tool_calls::trace::NeighborhoodSource;
use ploke_protocol::tool_calls::{review, segment, trace};
use ploke_protocol::{JsonAdjudicator, JsonLlmConfig, Procedure};
use regex::Regex;
use serde::Serialize;
use tokio::task::JoinSet;

use crate::campaign::{
    CampaignManifest, CampaignOverrides, CampaignValidationCheck, EvalCampaignPolicy,
    ProtocolCampaignPolicy, ResolvedCampaignConfig, adopt_campaign_manifest_from_closure_state,
    adopt_campaign_manifest_from_registry, apply_campaign_overrides, campaign_closure_state_path,
    campaign_manifest_path, dataset_files_from_sources, dataset_keys_from_sources, list_campaigns,
    render_resolved_campaign_config, resolve_campaign_config, save_campaign_manifest,
    validate_campaign_config,
};
use crate::closure::{
    ClosureClass, ClosureRecomputeRequest, closure_state_path, load_closure_state,
    recompute_closure_state, render_closure_status,
};
use crate::layout::{
    active_model_file, batches_dir, cache_dir, datasets_dir, model_registry_file, models_dir,
    repos_dir, runs_dir, starting_db_cache_dir, workspace_root_for_key,
};
use crate::model_registry::{
    find_models, load_active_model, load_model_registry, refresh_model_registry,
    registry_has_model, save_active_model,
};
use crate::msb::{PrepareMsbBatchRequest, PrepareMsbSingleRunRequest};
use crate::protocol::protocol_aggregate::{
    ProtocolAggregate, ProtocolAggregateError, ProtocolCallReviewRow, load_protocol_aggregate,
    load_protocol_aggregate_from_artifacts,
};
use crate::protocol_artifacts::{
    StoredProtocolArtifactFile, list_protocol_artifacts, load_protocol_artifact,
    protocol_artifact_preview, protocol_artifact_summary, write_protocol_artifact,
};
use crate::protocol_report::{
    ProtocolAggregateCallIssueRow, ProtocolAggregateCoverage, ProtocolAggregateReport,
    ProtocolAggregateSegmentRow, ProtocolColorProfile, ProtocolReportRenderOptions,
    render_protocol_aggregate_report_with_options,
};
use crate::protocol_triage_report::{
    ProtocolCampaignCountRow, ProtocolCampaignEvidence, ProtocolCampaignExemplarRow,
    ProtocolCampaignFamilyRow, ProtocolCampaignSummary, ProtocolCampaignTriageReport,
    render_protocol_campaign_triage_report, sort_count_rows,
};
use crate::provider_prefs::{
    clear_provider_for_model, load_provider_for_model, set_provider_for_model,
};
use crate::record::{RawFullResponseRecord, read_compressed_record};
use crate::registry::{builtin_dataset_registry_entries, builtin_dataset_registry_entry};
use crate::run_history::print_last_run_assistant_messages;
use crate::runner::{
    BatchRunSummary, ReplayMsbBatchRequest, RunMsbAgentBatchRequest, RunMsbAgentSingleRequest,
    RunMsbBatchRequest, RunMsbSingleRequest, resolve_provider_for_model,
};
use crate::spec::{
    EvalBudget, IssueInput, OutputMode, PrepareError, PrepareSingleRunRequest, PrepareWrite,
    PreparedCampaignContext,
};
use crate::target_registry::{
    BenchmarkFamily, RegistryEntry, RegistryRecomputeRequest, TargetRegistry, load_target_registry,
    recompute_target_registry, render_target_registry_status, target_registry_path,
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
    /// Run bounded review/adjudication protocols over eval artifacts.
    Protocol(ProtocolCommand),
    /// Manage the cached OpenRouter model registry and active model selection.
    Model(ModelCommand),
    /// Manage campaign manifests used by closure-driven operator workflows.
    Campaign(CampaignCommand),
    /// Manage the local typed benchmark target registry.
    Registry(RegistryCommand),
    /// Track staged closure of registry, eval, and protocol coverage for a campaign.
    Closure(ClosureCommand),
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
            Command::Protocol(cmd) => match cmd.run().await {
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
            Command::Campaign(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::Registry(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::Closure(cmd) => match cmd.run().await {
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

    /// Explicit model id to use for this run.
    #[arg(long)]
    pub model_id: Option<String>,

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

    /// Explicit model id to use for this run.
    #[arg(long)]
    pub model_id: Option<String>,

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
    set_provider_for_model(&selected.id, validated.provider.clone())?;
    println!("{}\t{}", selected.id, validated.provider.slug.as_str());
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
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
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

  cargo run -p ploke-eval -- inspect turn --instance BurntSushi__ripgrep-2209 1
  cargo run -p ploke-eval -- inspect turn --instance BurntSushi__ripgrep-2209 1 --show messages

Bootstrap questions:

  cargo run -p ploke-eval -- inspect turn --instance BurntSushi__ripgrep-2209 1 --show db-state
  cargo run -p ploke-eval -- inspect query --instance BurntSushi__ripgrep-2209 --turn 1 --lookup GlobSet
  cargo run -p ploke-eval -- inspect conversations --instance BurntSushi__ripgrep-2209
  cargo run -p ploke-eval -- inspect proto --instance BurntSushi__ripgrep-2209
  cargo run -p ploke-eval -- inspect proto --all-runs

Default target:

  If you omit --record and --instance, inspect uses the most recent completed run.
"
)]
pub struct InspectCommand {
    #[command(subcommand)]
    pub command: InspectSubcommand,
}

#[derive(Debug, Parser)]
#[command(about = "Run bounded review/adjudication protocols over eval artifacts")]
pub struct ProtocolCommand {
    #[command(subcommand)]
    pub command: ProtocolSubcommand,
}

#[derive(Debug, Parser)]
#[command(about = "Manage the local typed benchmark target registry")]
pub struct RegistryCommand {
    #[command(subcommand)]
    pub command: RegistrySubcommand,
}

#[derive(Debug, Parser)]
#[command(about = "Manage campaign manifests and resolved campaign configuration")]
pub struct CampaignCommand {
    #[command(subcommand)]
    pub command: CampaignSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum CampaignSubcommand {
    /// List campaign directories and indicate whether they have a manifest, closure state, or both.
    List(CampaignListCommand),
    /// Create or overwrite a campaign manifest under ~/.ploke-eval/campaigns/<campaign>/campaign.json.
    Init(CampaignInitCommand),
    /// Print the resolved campaign configuration.
    Show(CampaignShowCommand),
    /// Validate the resolved campaign configuration against local state and provider routing.
    Validate(CampaignValidateCommand),
}

#[derive(Debug, Parser, Clone, Default)]
pub struct CampaignOverrideArgs {
    /// Built-in dataset registry key. Repeat for multiple datasets.
    #[arg(long)]
    pub dataset_key: Vec<String>,

    /// Explicit dataset JSONL file. Repeat for multiple datasets.
    #[arg(long, value_name = "PATH")]
    pub dataset: Vec<PathBuf>,

    /// Override the selected model id.
    #[arg(long)]
    pub model_id: Option<String>,

    /// Override the selected provider slug.
    #[arg(long)]
    pub provider: Option<String>,

    /// Override required protocol procedures. Repeat for multiple values.
    #[arg(long)]
    pub required_procedure: Vec<String>,

    /// Override the runs root.
    #[arg(long, value_name = "PATH")]
    pub runs_root: Option<PathBuf>,

    /// Override the batches root.
    #[arg(long, value_name = "PATH")]
    pub batches_root: Option<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct CampaignInitCommand {
    /// Stable campaign identifier, used under ~/.ploke-eval/campaigns/<campaign>.
    #[arg(long)]
    pub campaign: String,

    #[command(flatten)]
    pub overrides: CampaignOverrideArgs,

    /// Seed the manifest from ~/.ploke-eval/campaigns/<campaign>/closure-state.json.
    #[arg(long, conflicts_with = "from_registry")]
    pub from_closure_state: bool,

    /// Seed the manifest from the persisted target registry and active model settings.
    #[arg(long, conflicts_with = "from_closure_state")]
    pub from_registry: bool,

    /// Overwrite an existing campaign manifest.
    #[arg(long)]
    pub force: bool,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct CampaignListCommand {
    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct CampaignShowCommand {
    /// Stable campaign identifier, used under ~/.ploke-eval/campaigns/<campaign>.
    #[arg(long)]
    pub campaign: String,

    #[command(flatten)]
    pub overrides: CampaignOverrideArgs,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct CampaignValidateCommand {
    /// Stable campaign identifier, used under ~/.ploke-eval/campaigns/<campaign>.
    #[arg(long)]
    pub campaign: String,

    #[command(flatten)]
    pub overrides: CampaignOverrideArgs,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Subcommand)]
pub enum RegistrySubcommand {
    /// Recompute the persisted target registry from dataset sources.
    Recompute(RegistryRecomputeCommand),
    /// Print the current persisted target registry.
    Status(RegistryStatusCommand),
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
#[command(about = "Track staged closure across registry, eval, and protocol layers")]
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
    pub campaign: String,

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
    pub campaign: String,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct ClosureAdvanceEvalCommand {
    /// Stable campaign identifier, used under ~/.ploke-eval/campaigns/<campaign>.
    #[arg(long)]
    pub campaign: String,

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
    pub campaign: String,

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
    pub campaign: String,

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

#[derive(Debug, Subcommand)]
pub enum ProtocolSubcommand {
    /// Review one indexed tool call using a bounded neighborhood, forked judgments, and merged assessment.
    ToolCallReview(ProtocolToolCallReviewCommand),
    /// Segment an ordered tool-call sequence into contiguous intent episodes.
    ToolCallIntentSegments(ProtocolToolCallIntentSegmentsCommand),
    /// Review one intent segment using the shared local-analysis packet over segmented trace state.
    ToolCallSegmentReview(ProtocolToolCallSegmentReviewCommand),
}

#[derive(Debug, Parser)]
pub struct ProtocolToolCallReviewCommand {
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
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

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct ProtocolToolCallIntentSegmentsCommand {
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Override the model id. Defaults to the current active eval model.
    #[arg(long)]
    pub model_id: Option<String>,

    /// Override the provider slug. Defaults to the persisted provider for the chosen model, if any.
    #[arg(long)]
    pub provider: Option<String>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct ProtocolToolCallSegmentReviewCommand {
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
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

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Subcommand)]
pub enum InspectSubcommand {
    /// List all agent conversation turns (run.conversations())
    #[command(alias = "turns")]
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
    /// List or inspect persisted protocol artifacts for a run.
    #[command(alias = "protocols")]
    ProtocolArtifacts(InspectProtocolArtifactsCommand),
    /// Aggregate persisted protocol artifacts into a human-facing report.
    #[command(alias = "proto", alias = "pview")]
    ProtocolOverview(InspectProtocolOverviewCommand),
}

#[derive(Debug, Parser)]
pub struct InspectConversationsCommand {
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
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
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Show one tool call in detail by its list index, e.g. `ploke-eval inspect tool-calls 5`.
    #[arg(value_name = "INDEX")]
    pub index: Option<usize>,

    /// Expand detail output to include full argument/result payloads.
    #[arg(long)]
    pub full: bool,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct InspectDbSnapshotsCommand {
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
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
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
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
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
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
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Turn number (1-indexed) to inspect.
    #[arg(value_name = "TURN", conflicts_with = "turn_flag")]
    pub turn: Option<u32>,

    /// Turn number (1-indexed) to inspect.
    #[arg(long = "turn", hide = true, conflicts_with = "turn")]
    pub turn_flag: Option<u32>,

    /// What to show for this turn: all, messages, responses, loop, tool-calls, tool-call, tool-result, db-state.
    #[arg(long, value_enum, default_value_t = TurnShowOption::All)]
    pub show: TurnShowOption,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Tool call index (0-based) when showing specific tool-call or tool-result.
    #[arg(long)]
    pub index: Option<usize>,

    /// Only include selected message roles when using --show messages.
    #[arg(long, value_enum, value_delimiter = ',')]
    pub roles: Vec<InspectMessageRole>,

    /// Exclude selected message roles when using --show messages.
    #[arg(long, value_enum, value_delimiter = ',', conflicts_with = "roles")]
    pub exclude_roles: Vec<InspectMessageRole>,
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
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
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

#[derive(Debug, Parser)]
pub struct InspectProtocolOverviewCommand {
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["instance", "all_runs", "campaign"])]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with_all = ["record", "all_runs", "campaign"])]
    pub instance: Option<String>,

    /// Aggregate all finished runs instead of one run.
    #[arg(long, conflicts_with_all = ["record", "instance", "campaign"])]
    pub all_runs: bool,

    /// Inspect one campaign-scoped protocol triage surface instead of one run or all visible runs.
    #[arg(long, conflicts_with_all = ["record", "instance", "all_runs"])]
    pub campaign: Option<String>,

    /// Which panel to emphasize for a single-run report.
    #[arg(long, value_enum, default_value_t = ProtocolOverviewView::Overview)]
    pub view: ProtocolOverviewView,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Only show issue-oriented rows where possible.
    #[arg(long)]
    pub only_issues: bool,

    /// Filter by overall verdict (e.g. mixed, focused_progress).
    #[arg(long)]
    pub overall: Option<String>,

    /// Filter campaign triage by issue kind (e.g. search_thrash, partial_next_step).
    #[arg(long, requires = "campaign")]
    pub issue: Option<String>,

    /// Filter segments by label (e.g. refine_search).
    #[arg(long)]
    pub segment_label: Option<String>,

    /// Filter call issues by tool name.
    #[arg(long)]
    pub tool: Option<String>,

    /// Filter campaign triage by protocol status (full, partial, error, missing, ineligible).
    #[arg(long, requires = "campaign")]
    pub status: Option<String>,

    /// Expand the exemplar list in campaign triage mode.
    #[arg(long, requires = "campaign")]
    pub examples: bool,

    /// Maximum number of rows to show in detail tables, exemplar lists, or all-runs summaries.
    #[arg(long, default_value_t = 8)]
    pub limit: usize,

    /// Target render width for table output.
    #[arg(long, default_value_t = 100)]
    pub width: usize,

    /// Color mode for table output.
    #[arg(long, value_enum, default_value_t = ProtocolColorMode::Auto)]
    pub color: ProtocolColorMode,

    /// Semantic color profile for table output.
    #[arg(long, value_enum, default_value_t = ProtocolColorProfileOption::TokioNight)]
    pub color_profile: ProtocolColorProfileOption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ProtocolOverviewView {
    Overview,
    Segments,
    Calls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ProtocolColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ProtocolColorProfileOption {
    TokioNight,
    Gruvbox,
    MonoDark,
}

impl From<ProtocolColorProfileOption> for ProtocolColorProfile {
    fn from(value: ProtocolColorProfileOption) -> Self {
        match value {
            ProtocolColorProfileOption::TokioNight => ProtocolColorProfile::TokioNight,
            ProtocolColorProfileOption::Gruvbox => ProtocolColorProfile::Gruvbox,
            ProtocolColorProfileOption::MonoDark => ProtocolColorProfile::MonoDark,
        }
    }
}

#[derive(Debug, Parser)]
pub struct InspectProtocolArtifactsCommand {
    /// Path to a run record file (record.json.gz). Defaults to the most recent run's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve ~/.ploke-eval/runs/<instance>/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Show one protocol artifact in detail by its list index.
    #[arg(value_name = "INDEX")]
    pub index: Option<usize>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Show the full nested JSON payloads instead of bounded previews.
    #[arg(long)]
    pub full: bool,
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
    Responses,
    Loop,
    ToolCalls,
    ToolCall,
    ToolResult,
    DbState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum InspectMessageRole {
    System,
    User,
    Assistant,
    Tool,
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
            model_id: self.model_id,
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
            model_id: self.model_id,
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
            model_id: self.model_id,
            provider: parse_provider_key(self.provider)?,
        }
        .run()
        .await?;
        println!("{}", artifacts.base.execution_log.display());
        println!("{}", artifacts.turn_summary.display());
        if let Some(path) = artifacts.base.full_response_trace {
            println!("{}", path.display());
        }
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
            model_id: self.model_id,
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
        let record_path = resolve_record_path(self.record, self.instance)?;

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
            InspectSubcommand::ProtocolArtifacts(cmd) => cmd.run().await,
            InspectSubcommand::ProtocolOverview(cmd) => cmd.run().await,
        }
    }
}

impl ProtocolCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            ProtocolSubcommand::ToolCallReview(cmd) => cmd.run().await,
            ProtocolSubcommand::ToolCallIntentSegments(cmd) => cmd.run().await,
            ProtocolSubcommand::ToolCallSegmentReview(cmd) => cmd.run().await,
        }
    }
}

impl CampaignCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            CampaignSubcommand::List(cmd) => cmd.run().await,
            CampaignSubcommand::Init(cmd) => cmd.run().await,
            CampaignSubcommand::Show(cmd) => cmd.run().await,
            CampaignSubcommand::Validate(cmd) => cmd.run().await,
        }
    }
}

impl RegistryCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RegistrySubcommand::Recompute(cmd) => cmd.run().await,
            RegistrySubcommand::Status(cmd) => cmd.run().await,
        }
    }
}

impl ClosureCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            ClosureSubcommand::Recompute(cmd) => cmd.run().await,
            ClosureSubcommand::Status(cmd) => cmd.run().await,
            ClosureSubcommand::Advance(cmd) => cmd.run().await,
        }
    }
}

impl ClosureAdvanceCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            ClosureAdvanceSubcommand::Eval(cmd) => cmd.run().await,
            ClosureAdvanceSubcommand::Protocol(cmd) => cmd.run().await,
            ClosureAdvanceSubcommand::All(cmd) => cmd.run().await,
        }
    }
}

impl CampaignOverrideArgs {
    fn into_overrides(self) -> CampaignOverrides {
        CampaignOverrides {
            dataset_keys: self.dataset_key,
            dataset_files: self.dataset,
            model_id: self.model_id,
            provider_slug: self.provider,
            required_procedures: self.required_procedure,
            runs_root: self.runs_root,
            batches_root: self.batches_root,
        }
    }
}

impl CampaignInitCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let path = campaign_manifest_path(&self.campaign)?;
        if path.exists() && !self.force {
            return Err(PrepareError::DatabaseSetup {
                phase: "campaign_init",
                detail: format!(
                    "campaign manifest already exists at '{}' (pass --force to overwrite)",
                    path.display()
                ),
            });
        }

        let overrides = self.overrides.into_overrides();
        let mut manifest = if self.from_closure_state {
            adopt_campaign_manifest_from_closure_state(&self.campaign)?
        } else if self.from_registry {
            adopt_campaign_manifest_from_registry(&self.campaign)?
        } else {
            let closure_path = campaign_closure_state_path(&self.campaign)?;
            if closure_path.exists() && overrides.is_empty() {
                return Err(PrepareError::DatabaseSetup {
                    phase: "campaign_init",
                    detail: format!(
                        "closure state exists at '{}' but no manifest exists; pass --from-closure-state to adopt it",
                        closure_path.display()
                    ),
                });
            }
            CampaignManifest::new(self.campaign.clone())
        };
        apply_campaign_overrides(&mut manifest, &overrides)?;
        let saved_path = save_campaign_manifest(&manifest)?;
        let resolved = resolve_campaign_config(&self.campaign, &CampaignOverrides::default())?;

        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_resolved_campaign_config(&resolved));
                println!("manifest: {}", saved_path.display());
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "manifest_path": saved_path,
                    "config": resolved,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl CampaignListCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let campaigns = list_campaigns()?;
        match self.format {
            InspectOutputFormat::Table => {
                if campaigns.is_empty() {
                    println!("campaigns: none");
                } else {
                    println!("campaigns");
                    for campaign in &campaigns {
                        let status = match (campaign.has_manifest, campaign.has_closure_state) {
                            (true, true) => "manifest+closure",
                            (true, false) => "manifest-only",
                            (false, true) => "closure-only",
                            (false, false) => "empty",
                        };
                        println!("  - {} | {}", campaign.campaign_id, status);
                    }
                }
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&campaigns).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl CampaignShowCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_resolved_campaign_config(&resolved));
                println!(
                    "manifest: {}",
                    campaign_manifest_path(&self.campaign)?.display()
                );
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&resolved).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl CampaignValidateCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        let checks = validate_campaign_config(&resolved).await?;
        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_resolved_campaign_config(&resolved));
                println!("\nvalidation");
                for check in &checks {
                    println!("  - {}: {}", check.label, check.detail);
                }
            }
            InspectOutputFormat::Json => {
                let payload = CampaignValidationView {
                    config: resolved,
                    checks,
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl RegistryRecomputeCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let (path, registry) = recompute_target_registry(RegistryRecomputeRequest {
            benchmark_family: BenchmarkFamily::MultiSweBenchRust,
            dataset_keys: self.dataset_key,
            dataset_files: self.dataset,
        })?;

        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_target_registry_status(&registry));
                println!("state: {}", path.display());
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&registry).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl RegistryStatusCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let registry = load_target_registry(BenchmarkFamily::MultiSweBenchRust)?;
        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_target_registry_status(&registry));
                println!(
                    "state: {}",
                    target_registry_path(BenchmarkFamily::MultiSweBenchRust)?.display()
                );
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&registry).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl ClosureRecomputeCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        let (path, state) = recompute_closure_state(closure_request_from_campaign(&resolved))?;

        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_closure_status(&state));
                println!("state: {}", path.display());
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl ClosureStatusCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let state = load_closure_state(&self.campaign)?;
        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_closure_status(&state));
                println!("state: {}", closure_state_path(&self.campaign)?.display());
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl ProtocolToolCallReviewCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let record_path = resolve_record_path(self.record, self.instance)?;
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let subject = build_tool_call_review_subject(&record, &record_path, self.index)?;
        let subject_id = subject.subject_id.clone();
        let persisted_input = subject.clone();

        let model_id = resolve_protocol_model_id(self.model_id)?;
        let provider_slug = resolve_protocol_provider_slug(&model_id, self.provider)?;
        let client = reqwest::Client::new();
        let cfg = JsonLlmConfig {
            model_id: model_id.to_string(),
            provider_slug,
            timeout_secs: 30,
            max_tokens: 400,
        };
        let protocol = review::ToolCallReview::new(JsonAdjudicator::new(client, cfg.clone()));
        let reviewed = protocol
            .run(subject)
            .await
            .map_err(tool_call_review_error_to_prepare)?;
        let persisted_path = write_protocol_artifact(
            &record_path,
            &reviewed.procedure_name,
            &subject_id,
            Some(cfg.model_id.as_str()),
            cfg.provider_slug.as_deref(),
            &persisted_input,
            &reviewed.output,
            &reviewed.artifact,
        )?;
        let review = &reviewed.output;

        match self.format {
            InspectOutputFormat::Table => {
                println!("Protocol: {}", reviewed.procedure_name);
                println!("{}", "-".repeat(40));
                println!("Model: {}", cfg.model_id);
                println!(
                    "Provider: {}",
                    cfg.provider_slug.as_deref().unwrap_or("auto/openrouter")
                );
                println!("Artifact: {}", persisted_path.display());
                println!(
                    "Target: {:?} {}",
                    review.packet.target_kind, review.packet.target_id
                );
                println!("Scope: {}", review.packet.scope_summary);
                println!("Turns: {}", join_turns(&review.packet.turn_span));
                println!("Calls in scope: {}", review.packet.total_calls_in_scope);
                if let Some(focal_index) = review.packet.focal_call_index {
                    println!("Focal call index: {}", focal_index);
                }
                println!("Calls:");
                for call in &review.packet.calls {
                    let marker = if Some(call.index) == review.packet.focal_call_index {
                        "focal"
                    } else {
                        "scope"
                    };
                    println!(
                        "  {:<6} [{}] {} | {}",
                        marker, call.index, call.tool_name, call.summary
                    );
                }
                println!();
                println!("Signals");
                println!("{}", "-".repeat(40));
                println!(
                    "Repeated tool calls: {}",
                    review.signals.repeated_tool_name_count
                );
                println!("Distinct tools: {}", review.signals.distinct_tool_count);
                println!(
                    "Similar searches: {}",
                    review.signals.similar_search_neighbors
                );
                println!("Directory pivots: {}", review.signals.directory_pivots);
                println!("Search calls: {}", review.signals.search_calls_in_scope);
                println!("Read calls: {}", review.signals.read_calls_in_scope);
                println!("Browse calls: {}", review.signals.browse_calls_in_scope);
                println!(
                    "Candidate concerns: {:?}",
                    review.signals.candidate_concerns
                );
                println!();
                println!("Assessments");
                println!("{}", "-".repeat(40));
                println!(
                    "Usefulness: {:?} ({:?})",
                    review.usefulness.verdict, review.usefulness.confidence
                );
                println!("  {}", review.usefulness.rationale);
                println!(
                    "Redundancy: {:?} ({:?})",
                    review.redundancy.verdict, review.redundancy.confidence
                );
                println!("  {}", review.redundancy.rationale);
                println!(
                    "Recoverability: {:?} ({:?})",
                    review.recoverability.verdict, review.recoverability.confidence
                );
                println!("  {}", review.recoverability.rationale);
                println!();
                println!(
                    "Overall: {:?} ({:?})",
                    review.overall, review.overall_confidence
                );
                println!("Synthesis: {}", review.synthesis_rationale);
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "procedure": reviewed.procedure_name,
                    "persisted_artifact_path": persisted_path,
                    "output": reviewed.output,
                    "artifact": reviewed.artifact,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
    }
}

impl ClosureAdvanceEvalCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        let mut policy = resolved.eval.clone();
        if let Some(limit) = self.limit {
            policy.limit = Some(limit);
        }
        if self.stop_on_error {
            policy.stop_on_error = true;
        }
        let report = advance_eval_closure(&resolved, &policy, self.dry_run).await?;
        render_advance_eval_report(report, self.format)
    }
}

impl ClosureAdvanceProtocolCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        let mut policy = resolved.protocol.clone();
        if let Some(limit_runs) = self.limit_runs {
            policy.limit_runs = Some(limit_runs);
        }
        if let Some(max_concurrency) = self.max_concurrency {
            policy.max_concurrency = max_concurrency.max(1);
        }
        if self.stop_on_error {
            policy.stop_on_error = true;
        }
        let report = advance_protocol_closure(&resolved, &policy, self.dry_run).await?;
        render_advance_protocol_report(report, self.format)
    }
}

impl ClosureAdvanceAllCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        let mut eval_policy = resolved.eval.clone();
        if let Some(limit) = self.eval_limit {
            eval_policy.limit = Some(limit);
        }
        if self.stop_on_error {
            eval_policy.stop_on_error = true;
        }
        let mut protocol_policy = resolved.protocol.clone();
        if let Some(limit_runs) = self.protocol_limit_runs {
            protocol_policy.limit_runs = Some(limit_runs);
        }
        if let Some(max_concurrency) = self.protocol_max_concurrency {
            protocol_policy.max_concurrency = max_concurrency.max(1);
        }
        if self.stop_on_error {
            protocol_policy.stop_on_error = true;
        }

        let eval_report = advance_eval_closure(&resolved, &eval_policy, self.dry_run).await?;
        let protocol_report =
            advance_protocol_closure(&resolved, &protocol_policy, self.dry_run).await?;
        render_advance_all_report(
            ClosureAdvanceAllReport {
                campaign_id: resolved.campaign_id,
                dry_run: self.dry_run,
                eval: eval_report,
                protocol: protocol_report,
            },
            self.format,
        )
    }
}

#[derive(Debug, Serialize)]
struct CampaignValidationView {
    config: ResolvedCampaignConfig,
    checks: Vec<CampaignValidationCheck>,
}

#[derive(Debug, Clone, Serialize)]
struct EvalBatchPlan {
    batch_id: String,
    dataset_label: String,
    dataset_path: PathBuf,
    instances: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ClosureAdvanceEvalReport {
    campaign_id: String,
    dry_run: bool,
    before: crate::closure::EvalClosureSummary,
    after: crate::closure::EvalClosureSummary,
    selected_instances: Vec<String>,
    selected_batches: Vec<EvalBatchPlan>,
    executed_batches: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ProtocolRunPlan {
    instance_id: String,
    segmentation_needed: bool,
    missing_call_indices: Vec<usize>,
    missing_segment_indices: Vec<usize>,
}

#[derive(Debug, Clone)]
struct ProtocolRunTask {
    instance_id: String,
    record_path: PathBuf,
}

#[derive(Debug)]
struct ProtocolRunExecution {
    instance_id: String,
    plan: ProtocolRunPlan,
    segmentations_created: usize,
    call_reviews_created: usize,
    segment_reviews_created: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ClosureAdvanceProtocolReport {
    campaign_id: String,
    dry_run: bool,
    before: crate::closure::ProtocolClosureSummary,
    after: crate::closure::ProtocolClosureSummary,
    selected_runs: Vec<ProtocolRunPlan>,
    executed_runs: usize,
    segmentations_created: usize,
    call_reviews_created: usize,
    segment_reviews_created: usize,
    failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ClosureAdvanceAllReport {
    campaign_id: String,
    dry_run: bool,
    eval: ClosureAdvanceEvalReport,
    protocol: ClosureAdvanceProtocolReport,
}

fn closure_request_from_campaign(config: &ResolvedCampaignConfig) -> ClosureRecomputeRequest {
    ClosureRecomputeRequest {
        campaign_id: config.campaign_id.clone(),
        benchmark_family: Some(config.benchmark_family),
        model_id: Some(config.model_id.clone()),
        provider_slug: config.provider_slug.clone(),
        dataset_keys: dataset_keys_from_sources(&config.dataset_sources),
        dataset_files: dataset_files_from_sources(&config.dataset_sources),
        required_procedures: config.required_procedures.clone(),
        runs_root: Some(config.runs_root.clone()),
        batches_root: Some(config.batches_root.clone()),
        framework: Some(config.framework.clone()),
    }
}

fn campaign_context_from_config(config: &ResolvedCampaignConfig) -> PreparedCampaignContext {
    PreparedCampaignContext {
        campaign_id: config.campaign_id.clone(),
        model_id: Some(config.model_id.clone()),
        provider_slug: config.provider_slug.clone(),
        framework: config.framework.clone(),
    }
}

fn select_eval_rows<'a>(
    state: &'a crate::closure::ClosureState,
    policy: &EvalCampaignPolicy,
) -> Vec<&'a crate::closure::ClosureInstanceRow> {
    let mut selected = state
        .instances
        .iter()
        .filter(|row| match row.eval_status {
            crate::closure::ClosureClass::Missing => true,
            crate::closure::ClosureClass::Partial => policy.include_partial,
            _ => false,
        })
        .filter(|row| {
            (policy.include_dataset_labels.is_empty()
                || policy
                    .include_dataset_labels
                    .iter()
                    .any(|label| label == &row.dataset_label))
                && !policy
                    .exclude_dataset_labels
                    .iter()
                    .any(|label| label == &row.dataset_label)
        })
        .collect::<Vec<_>>();
    if let Some(limit) = policy.limit {
        selected.truncate(limit);
    }
    selected
}

fn select_protocol_rows<'a>(
    state: &'a crate::closure::ClosureState,
    policy: &ProtocolCampaignPolicy,
) -> Vec<&'a crate::closure::ClosureInstanceRow> {
    let mut selected = state
        .instances
        .iter()
        .filter(|row| row.eval_status == crate::closure::ClosureClass::Complete)
        .filter(|row| match row.protocol_status {
            crate::closure::ClosureClass::Missing => true,
            crate::closure::ClosureClass::Partial => policy.include_partial,
            crate::closure::ClosureClass::Incompatible => policy.include_incompatible,
            crate::closure::ClosureClass::Failed => policy.include_failed,
            _ => false,
        })
        .collect::<Vec<_>>();
    if let Some(limit) = policy.limit_runs {
        selected.truncate(limit);
    }
    selected
}

fn build_eval_batch_plans(
    registry: &TargetRegistry,
    selected_rows: &[&crate::closure::ClosureInstanceRow],
    campaign_id: &str,
    batch_prefix: Option<&str>,
) -> Result<Vec<EvalBatchPlan>, PrepareError> {
    let mut by_instance = BTreeMap::<String, RegistryEntry>::new();
    for entry in &registry.entries {
        by_instance.insert(entry.instance_id.clone(), entry.clone());
    }

    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
    let prefix = batch_prefix.unwrap_or(campaign_id);
    let mut grouped = BTreeMap::<(String, PathBuf), Vec<String>>::new();
    for row in selected_rows {
        let entry =
            by_instance
                .get(&row.instance_id)
                .ok_or_else(|| PrepareError::DatabaseSetup {
                    phase: "closure_advance_eval",
                    detail: format!("registry entry missing for '{}'", row.instance_id),
                })?;
        grouped
            .entry((row.dataset_label.clone(), entry.source.dataset_path.clone()))
            .or_default()
            .push(row.instance_id.clone());
    }

    let mut plans = Vec::new();
    for ((dataset_label, dataset_path), mut instances) in grouped {
        instances.sort();
        let batch_id = format!(
            "{}-eval-{}-{}",
            sanitize_batch_component(prefix),
            sanitize_batch_component(&dataset_label),
            timestamp
        );
        plans.push(EvalBatchPlan {
            batch_id,
            dataset_label,
            dataset_path,
            instances,
        });
    }
    plans.sort_by(|left, right| left.batch_id.cmp(&right.batch_id));
    Ok(plans)
}

async fn advance_eval_closure(
    config: &ResolvedCampaignConfig,
    policy: &EvalCampaignPolicy,
    dry_run: bool,
) -> Result<ClosureAdvanceEvalReport, PrepareError> {
    let before_state = recompute_closure_state(closure_request_from_campaign(config))?.1;
    let selected_rows = select_eval_rows(&before_state, policy);
    let registry = recompute_target_registry(RegistryRecomputeRequest {
        benchmark_family: config.benchmark_family,
        dataset_keys: dataset_keys_from_sources(&config.dataset_sources),
        dataset_files: dataset_files_from_sources(&config.dataset_sources),
    })?
    .1;
    let plans = build_eval_batch_plans(
        &registry,
        &selected_rows,
        &config.campaign_id,
        policy.batch_prefix.as_deref(),
    )?;
    let selected_instances = plans
        .iter()
        .flat_map(|plan| plan.instances.iter().cloned())
        .collect::<Vec<_>>();

    let mut executed_batches = 0usize;
    if !dry_run {
        let campaign_context = campaign_context_from_config(config);
        let provider = parse_provider_key(config.provider_slug.clone())?;
        for plan in &plans {
            let mut prepared = PrepareMsbBatchRequest {
                dataset_file: Some(plan.dataset_path.clone()),
                dataset_key: None,
                batch_id: plan.batch_id.clone(),
                select_all: false,
                instance_ids: plan.instances.clone(),
                specifics: Vec::new(),
                limit: None,
                repo_cache: repos_dir()?,
                runs_root: config.runs_root.clone(),
                batches_root: config.batches_root.clone(),
                budget: policy.budget.clone(),
            }
            .prepare()?;

            for run in &mut prepared.runs {
                run.campaign = Some(campaign_context.clone());
                run.write_manifest(OutputMode::Pretty, PrepareWrite::File(run.manifest_path()))?;
            }
            prepared.batch.campaign = Some(campaign_context.clone());
            prepared.batch.write_manifest(OutputMode::Pretty)?;

            let artifacts = RunMsbAgentBatchRequest {
                batch_manifest: prepared.batch.manifest_path(),
                index_debug_snapshots: true,
                use_default_model: false,
                model_id: Some(config.model_id.clone()),
                provider: provider.clone(),
                stop_on_error: policy.stop_on_error,
            }
            .run()
            .await?;
            executed_batches += 1;

            if policy.stop_on_error && batch_summary_has_failure(&artifacts.summary)? {
                break;
            }
        }
    }

    let after_state = recompute_closure_state(closure_request_from_campaign(config))?.1;
    Ok(ClosureAdvanceEvalReport {
        campaign_id: config.campaign_id.clone(),
        dry_run,
        before: before_state.eval,
        after: after_state.eval,
        selected_instances,
        selected_batches: plans,
        executed_batches,
    })
}

async fn advance_protocol_closure(
    config: &ResolvedCampaignConfig,
    policy: &ProtocolCampaignPolicy,
    dry_run: bool,
) -> Result<ClosureAdvanceProtocolReport, PrepareError> {
    let before_state = recompute_closure_state(closure_request_from_campaign(config))?.1;
    let selected_rows = select_protocol_rows(&before_state, policy);
    let selected_order = selected_rows
        .iter()
        .map(|row| row.instance_id.clone())
        .collect::<Vec<_>>();
    let tasks =
        selected_rows
            .iter()
            .map(|row| {
                let record_path = row.artifacts.record_path.clone().ok_or_else(|| {
                    PrepareError::DatabaseSetup {
                        phase: "closure_advance_protocol",
                        detail: format!("record path missing for '{}'", row.instance_id),
                    }
                })?;
                Ok(ProtocolRunTask {
                    instance_id: row.instance_id.clone(),
                    record_path,
                })
            })
            .collect::<Result<Vec<_>, PrepareError>>()?;
    let mut plans_by_instance = BTreeMap::<String, ProtocolRunPlan>::new();
    let mut executed_runs = 0usize;
    let mut segmentations_created = 0usize;
    let mut call_reviews_created = 0usize;
    let mut segment_reviews_created = 0usize;
    let mut failures = Vec::new();

    if dry_run {
        for task in tasks {
            let plan = protocol_run_plan(&task.instance_id, &task.record_path)?;
            plans_by_instance.insert(task.instance_id, plan);
        }
    } else {
        let max_concurrency = policy.max_concurrency.max(1);
        let mut pending = tasks.into_iter().collect::<VecDeque<_>>();
        let mut join_set = JoinSet::new();
        while join_set.len() < max_concurrency {
            let Some(task) = pending.pop_front() else {
                break;
            };
            spawn_protocol_run_task(
                &mut join_set,
                task,
                config.model_id.clone(),
                config.provider_slug.clone(),
            );
        }

        while let Some(joined) = join_set.join_next().await {
            match joined {
                Ok(Ok(execution)) => {
                    executed_runs += 1;
                    segmentations_created += execution.segmentations_created;
                    call_reviews_created += execution.call_reviews_created;
                    segment_reviews_created += execution.segment_reviews_created;
                    plans_by_instance.insert(execution.instance_id.clone(), execution.plan);
                    if let Some(task) = pending.pop_front() {
                        spawn_protocol_run_task(
                            &mut join_set,
                            task,
                            config.model_id.clone(),
                            config.provider_slug.clone(),
                        );
                    }
                }
                Ok(Err(err)) => {
                    failures.push(err.to_string());
                    if policy.stop_on_error {
                        join_set.abort_all();
                        return Err(err);
                    }
                    if let Some(task) = pending.pop_front() {
                        spawn_protocol_run_task(
                            &mut join_set,
                            task,
                            config.model_id.clone(),
                            config.provider_slug.clone(),
                        );
                    }
                }
                Err(err) => {
                    let detail = PrepareError::DatabaseSetup {
                        phase: "closure_advance_protocol",
                        detail: format!("protocol worker task failed: {err}"),
                    };
                    failures.push(detail.to_string());
                    if policy.stop_on_error {
                        join_set.abort_all();
                        return Err(detail);
                    }
                    if let Some(task) = pending.pop_front() {
                        spawn_protocol_run_task(
                            &mut join_set,
                            task,
                            config.model_id.clone(),
                            config.provider_slug.clone(),
                        );
                    }
                }
            }
        }
    }

    let plans = selected_order
        .into_iter()
        .filter_map(|instance_id| plans_by_instance.remove(&instance_id))
        .collect::<Vec<_>>();

    let after_state = recompute_closure_state(closure_request_from_campaign(config))?.1;
    Ok(ClosureAdvanceProtocolReport {
        campaign_id: config.campaign_id.clone(),
        dry_run,
        before: before_state.protocol,
        after: after_state.protocol,
        selected_runs: plans,
        executed_runs,
        segmentations_created,
        call_reviews_created,
        segment_reviews_created,
        failures,
    })
}

fn spawn_protocol_run_task(
    join_set: &mut JoinSet<Result<ProtocolRunExecution, PrepareError>>,
    task: ProtocolRunTask,
    model_id: String,
    provider_slug: Option<String>,
) {
    join_set.spawn(async move { execute_protocol_run_task(task, model_id, provider_slug).await });
}

async fn execute_protocol_run_task(
    task: ProtocolRunTask,
    model_id: String,
    provider_slug: Option<String>,
) -> Result<ProtocolRunExecution, PrepareError> {
    let mut plan = protocol_run_plan(&task.instance_id, &task.record_path).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "closure_advance_protocol",
            detail: format!("{}: {err}", task.instance_id),
        }
    })?;
    let mut segmentations_created = 0usize;
    let mut call_reviews_created = 0usize;
    let mut segment_reviews_created = 0usize;

    if plan.segmentation_needed {
        execute_protocol_intent_segments_quiet(
            &task.record_path,
            Some(model_id.clone()),
            provider_slug.clone(),
        )
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "closure_advance_protocol",
            detail: format!("{}: {err}", task.instance_id),
        })?;
        segmentations_created += 1;
        plan = protocol_run_plan(&task.instance_id, &task.record_path).map_err(|err| {
            PrepareError::DatabaseSetup {
                phase: "closure_advance_protocol",
                detail: format!("{}: {err}", task.instance_id),
            }
        })?;
    }

    for call_index in plan.missing_call_indices.clone() {
        execute_protocol_tool_call_review_quiet(
            &task.record_path,
            Some(model_id.clone()),
            provider_slug.clone(),
            call_index,
        )
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "closure_advance_protocol",
            detail: format!("{}: {err}", task.instance_id),
        })?;
        call_reviews_created += 1;
    }

    plan = protocol_run_plan(&task.instance_id, &task.record_path).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "closure_advance_protocol",
            detail: format!("{}: {err}", task.instance_id),
        }
    })?;
    for segment_index in plan.missing_segment_indices.clone() {
        execute_protocol_tool_call_segment_review_quiet(
            &task.record_path,
            Some(model_id.clone()),
            provider_slug.clone(),
            segment_index,
        )
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "closure_advance_protocol",
            detail: format!("{}: {err}", task.instance_id),
        })?;
        segment_reviews_created += 1;
    }

    let plan = protocol_run_plan(&task.instance_id, &task.record_path).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "closure_advance_protocol",
            detail: format!("{}: {err}", task.instance_id),
        }
    })?;
    Ok(ProtocolRunExecution {
        instance_id: task.instance_id,
        plan,
        segmentations_created,
        call_reviews_created,
        segment_reviews_created,
    })
}

fn protocol_run_plan(
    instance_id: &str,
    record_path: &Path,
) -> Result<ProtocolRunPlan, PrepareError> {
    match load_protocol_aggregate(record_path) {
        Ok(aggregate) => Ok(ProtocolRunPlan {
            instance_id: instance_id.to_string(),
            segmentation_needed: false,
            missing_call_indices: aggregate.coverage.missing_call_indices,
            missing_segment_indices: aggregate.coverage.missing_segment_indices,
        }),
        Err(err) => match err {
            crate::protocol::protocol_aggregate::ProtocolAggregateError::MissingAnchor {
                ..
            } => Ok(ProtocolRunPlan {
                instance_id: instance_id.to_string(),
                segmentation_needed: true,
                missing_call_indices: Vec::new(),
                missing_segment_indices: Vec::new(),
            }),
            other => Err(PrepareError::DatabaseSetup {
                phase: "closure_advance_protocol",
                detail: format!("{}: {other}", instance_id),
            }),
        },
    }
}

fn batch_summary_has_failure(path: &Path) -> Result<bool, PrepareError> {
    let text = std::fs::read_to_string(path).map_err(|source| PrepareError::ReadBatchManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let summary: BatchRunSummary =
        serde_json::from_str(&text).map_err(|source| PrepareError::ParseBatchManifest {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(summary.instances_failed > 0)
}

async fn execute_protocol_intent_segments_quiet(
    record_path: &Path,
    model_id: Option<String>,
    provider: Option<String>,
) -> Result<segment::SegmentedToolCallSequence, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject = build_tool_call_sequence_subject(&record, record_path)?;
    let subject_id = subject.subject_id.clone();
    let persisted_input = subject.clone();
    let model_id = resolve_protocol_model_id(model_id)?;
    let provider_slug = resolve_protocol_provider_slug(&model_id, provider)?;
    let client = reqwest::Client::new();
    let cfg = JsonLlmConfig {
        model_id: model_id.to_string(),
        provider_slug,
        timeout_secs: 45,
        max_tokens: 1200,
    };
    let protocol =
        segment::ToolCallIntentSegmentation::new(JsonAdjudicator::new(client, cfg.clone()));
    let segmented = protocol
        .run(subject)
        .await
        .map_err(tool_call_intent_segmentation_error_to_prepare)?;
    write_protocol_artifact(
        record_path,
        &segmented.procedure_name,
        &subject_id,
        Some(cfg.model_id.as_str()),
        cfg.provider_slug.as_deref(),
        &persisted_input,
        &segmented.output,
        &segmented.artifact,
    )?;
    Ok(segmented.output)
}

async fn execute_protocol_tool_call_review_quiet(
    record_path: &Path,
    model_id: Option<String>,
    provider: Option<String>,
    index: usize,
) -> Result<(), PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject = build_tool_call_review_subject(&record, record_path, index)?;
    let subject_id = subject.subject_id.clone();
    let persisted_input = subject.clone();
    let model_id = resolve_protocol_model_id(model_id)?;
    let provider_slug = resolve_protocol_provider_slug(&model_id, provider)?;
    let client = reqwest::Client::new();
    let cfg = JsonLlmConfig {
        model_id: model_id.to_string(),
        provider_slug,
        timeout_secs: 30,
        max_tokens: 400,
    };
    let protocol = review::ToolCallReview::new(JsonAdjudicator::new(client, cfg.clone()));
    let reviewed = protocol
        .run(subject)
        .await
        .map_err(tool_call_review_error_to_prepare)?;
    write_protocol_artifact(
        record_path,
        &reviewed.procedure_name,
        &subject_id,
        Some(cfg.model_id.as_str()),
        cfg.provider_slug.as_deref(),
        &persisted_input,
        &reviewed.output,
        &reviewed.artifact,
    )?;
    Ok(())
}

fn load_latest_segmented_sequence(
    record_path: &Path,
) -> Result<Option<segment::SegmentedToolCallSequence>, PrepareError> {
    let artifacts = list_protocol_artifacts(record_path)?;
    let latest = artifacts
        .into_iter()
        .filter(|entry| entry.stored.procedure_name == "tool_call_intent_segmentation")
        .max_by_key(|entry| entry.stored.created_at_ms);
    latest
        .map(|entry| {
            serde_json::from_value(entry.stored.output).map_err(|source| {
                PrepareError::DatabaseSetup {
                    phase: "protocol_load_segmented_sequence",
                    detail: source.to_string(),
                }
            })
        })
        .transpose()
}

async fn execute_protocol_tool_call_segment_review_quiet(
    record_path: &Path,
    model_id: Option<String>,
    provider: Option<String>,
    segment_index: usize,
) -> Result<(), PrepareError> {
    let segmented = match load_latest_segmented_sequence(record_path)? {
        Some(segmented) => segmented,
        None => {
            execute_protocol_intent_segments_quiet(record_path, model_id.clone(), provider.clone())
                .await?
        }
    };
    let subject = build_segment_review_subject(&segmented, segment_index)?;
    let subject_id = subject.subject_id.clone();
    let persisted_input = subject.clone();
    let model_id = resolve_protocol_model_id(model_id)?;
    let provider_slug = resolve_protocol_provider_slug(&model_id, provider)?;
    let client = reqwest::Client::new();
    let cfg = JsonLlmConfig {
        model_id: model_id.to_string(),
        provider_slug,
        timeout_secs: 45,
        max_tokens: 1200,
    };
    let protocol = review::ToolCallSegmentReview::new(JsonAdjudicator::new(client, cfg.clone()));
    let reviewed = protocol
        .run(subject)
        .await
        .map_err(tool_call_segment_review_error_to_prepare)?;
    write_protocol_artifact(
        record_path,
        &reviewed.procedure_name,
        &subject_id,
        Some(cfg.model_id.as_str()),
        cfg.provider_slug.as_deref(),
        &persisted_input,
        &reviewed.output,
        &reviewed.artifact,
    )?;
    Ok(())
}

fn render_advance_eval_report(
    report: ClosureAdvanceEvalReport,
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Table => {
            println!(
                "campaign {} | eval before {} complete / {} fail / {} partial / {} missing",
                report.campaign_id,
                report.before.complete_total,
                report.before.failed_total,
                report.before.partial_total,
                report.before.missing_total
            );
            println!(
                "selected {} instance(s) in {} batch(es){}",
                report.selected_instances.len(),
                report.selected_batches.len(),
                if report.dry_run { " [dry-run]" } else { "" }
            );
            for batch in &report.selected_batches {
                println!(
                    "  - {} | {} | {} instance(s)",
                    batch.batch_id,
                    batch.dataset_label,
                    batch.instances.len()
                );
            }
            println!(
                "eval after {} complete / {} fail / {} partial / {} missing",
                report.after.complete_total,
                report.after.failed_total,
                report.after.partial_total,
                report.after.missing_total
            );
        }
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
            );
        }
    }
    Ok(())
}

fn render_advance_protocol_report(
    report: ClosureAdvanceProtocolReport,
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Table => {
            println!(
                "campaign {} | protocol before {} full / {} partial / {} incompatible / {} fail / {} missing / {} ineligible",
                report.campaign_id,
                report.before.full_total,
                report.before.partial_total,
                report.before.incompatible_total,
                report.before.failed_total,
                report.before.missing_total,
                report.before.ineligible_total
            );
            println!(
                "selected {} run(s){}",
                report.selected_runs.len(),
                if report.dry_run { " [dry-run]" } else { "" }
            );
            for run in &report.selected_runs {
                println!(
                    "  - {} | segmentation {} | missing calls {} | missing segments {}",
                    run.instance_id,
                    if run.segmentation_needed { "yes" } else { "no" },
                    run.missing_call_indices.len(),
                    run.missing_segment_indices.len()
                );
            }
            println!(
                "protocol after {} full / {} partial / {} incompatible / {} fail / {} missing / {} ineligible",
                report.after.full_total,
                report.after.partial_total,
                report.after.incompatible_total,
                report.after.failed_total,
                report.after.missing_total,
                report.after.ineligible_total
            );
            if !report.failures.is_empty() {
                println!("\nfailures");
                for failure in &report.failures {
                    println!("  - {}", failure);
                }
            }
        }
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
            );
        }
    }
    Ok(())
}

fn render_advance_all_report(
    report: ClosureAdvanceAllReport,
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Table => {
            println!(
                "campaign {}{}",
                report.campaign_id,
                if report.dry_run { " [dry-run]" } else { "" }
            );
            println!(
                "eval: selected {} instance(s), missing now {}",
                report.eval.selected_instances.len(),
                report.eval.after.missing_total
            );
            println!(
                "protocol: selected {} run(s), missing now {}",
                report.protocol.selected_runs.len(),
                report.protocol.after.missing_total
            );
        }
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
            );
        }
    }
    Ok(())
}

impl ProtocolToolCallSegmentReviewCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let record_path = resolve_record_path(self.record, self.instance)?;
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let sequence_subject = build_tool_call_sequence_subject(&record, &record_path)?;
        let model_id = resolve_protocol_model_id(self.model_id)?;
        let provider_slug = resolve_protocol_provider_slug(&model_id, self.provider)?;
        let client = reqwest::Client::new();
        let cfg = JsonLlmConfig {
            model_id: model_id.to_string(),
            provider_slug,
            timeout_secs: 45,
            max_tokens: 1200,
        };
        let adjudicator = JsonAdjudicator::new(client, cfg.clone());
        let segmentation = segment::ToolCallIntentSegmentation::new(adjudicator.clone())
            .run(sequence_subject)
            .await
            .map_err(tool_call_intent_segmentation_error_to_prepare)?;

        let subject = build_segment_review_subject(&segmentation.output, self.segment_index)?;
        let subject_id = subject.subject_id.clone();
        let persisted_input = subject.clone();
        let protocol = review::ToolCallSegmentReview::new(adjudicator);
        let reviewed = protocol
            .run(subject)
            .await
            .map_err(tool_call_segment_review_error_to_prepare)?;
        let persisted_path = write_protocol_artifact(
            &record_path,
            &reviewed.procedure_name,
            &subject_id,
            Some(cfg.model_id.as_str()),
            cfg.provider_slug.as_deref(),
            &persisted_input,
            &reviewed.output,
            &reviewed.artifact,
        )?;
        let review = &reviewed.output;

        match self.format {
            InspectOutputFormat::Table => {
                println!("Protocol: {}", reviewed.procedure_name);
                println!("{}", "-".repeat(40));
                println!("Model: {}", cfg.model_id);
                println!(
                    "Provider: {}",
                    cfg.provider_slug.as_deref().unwrap_or("auto/openrouter")
                );
                println!("Artifact: {}", persisted_path.display());
                println!(
                    "Target: {:?} {}",
                    review.packet.target_kind, review.packet.target_id
                );
                println!("Scope: {}", review.packet.scope_summary);
                println!("Turns: {}", join_turns(&review.packet.turn_span));
                println!("Calls in scope: {}", review.packet.total_calls_in_scope);
                println!("Calls:");
                for call in &review.packet.calls {
                    println!("  [{}] {} | {}", call.index, call.tool_name, call.summary);
                }
                println!();
                println!("Signals");
                println!("{}", "-".repeat(40));
                println!(
                    "Repeated tool calls: {}",
                    review.signals.repeated_tool_name_count
                );
                println!("Distinct tools: {}", review.signals.distinct_tool_count);
                println!("Directory pivots: {}", review.signals.directory_pivots);
                println!("Search calls: {}", review.signals.search_calls_in_scope);
                println!("Read calls: {}", review.signals.read_calls_in_scope);
                println!("Browse calls: {}", review.signals.browse_calls_in_scope);
                println!(
                    "Source labeled segments: {}",
                    review.signals.labeled_segments_in_source.unwrap_or(0)
                );
                println!(
                    "Source ambiguous segments: {}",
                    review.signals.ambiguous_segments_in_source.unwrap_or(0)
                );
                println!(
                    "Source uncovered calls: {}",
                    review.signals.uncovered_calls_in_source.unwrap_or(0)
                );
                println!(
                    "Candidate concerns: {:?}",
                    review.signals.candidate_concerns
                );
                println!();
                println!("Assessments");
                println!("{}", "-".repeat(40));
                println!(
                    "Usefulness: {:?} ({:?})",
                    review.usefulness.verdict, review.usefulness.confidence
                );
                println!("  {}", review.usefulness.rationale);
                println!(
                    "Redundancy: {:?} ({:?})",
                    review.redundancy.verdict, review.redundancy.confidence
                );
                println!("  {}", review.redundancy.rationale);
                println!(
                    "Recoverability: {:?} ({:?})",
                    review.recoverability.verdict, review.recoverability.confidence
                );
                println!("  {}", review.recoverability.rationale);
                println!();
                println!(
                    "Overall: {:?} ({:?})",
                    review.overall, review.overall_confidence
                );
                println!("Synthesis: {}", review.synthesis_rationale);
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "procedure": reviewed.procedure_name,
                    "persisted_artifact_path": persisted_path,
                    "output": reviewed.output,
                    "artifact": reviewed.artifact,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
    }
}

impl ProtocolToolCallIntentSegmentsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let record_path = resolve_record_path(self.record, self.instance)?;
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let subject = build_tool_call_sequence_subject(&record, &record_path)?;
        let subject_id = subject.subject_id.clone();
        let persisted_input = subject.clone();

        let model_id = resolve_protocol_model_id(self.model_id)?;
        let provider_slug = resolve_protocol_provider_slug(&model_id, self.provider)?;
        let client = reqwest::Client::new();
        let cfg = JsonLlmConfig {
            model_id: model_id.to_string(),
            provider_slug,
            timeout_secs: 45,
            max_tokens: 1200,
        };
        let protocol =
            segment::ToolCallIntentSegmentation::new(JsonAdjudicator::new(client, cfg.clone()));
        let segmented = protocol
            .run(subject)
            .await
            .map_err(tool_call_intent_segmentation_error_to_prepare)?;
        let persisted_path = write_protocol_artifact(
            &record_path,
            &segmented.procedure_name,
            &subject_id,
            Some(cfg.model_id.as_str()),
            cfg.provider_slug.as_deref(),
            &persisted_input,
            &segmented.output,
            &segmented.artifact,
        )?;
        let output = &segmented.output;

        match self.format {
            InspectOutputFormat::Table => {
                println!("Protocol: {}", segmented.procedure_name);
                println!("{}", "-".repeat(40));
                println!("Model: {}", cfg.model_id);
                println!(
                    "Provider: {}",
                    cfg.provider_slug.as_deref().unwrap_or("auto/openrouter")
                );
                println!("Artifact: {}", persisted_path.display());
                println!("Turns: {}", output.sequence.total_turns);
                println!("Tool calls: {}", output.sequence.total_calls_in_run);
                println!("Segments: {}", output.segments.len());
                println!("Labeled segments: {}", output.coverage.labeled_segments);
                println!("Ambiguous segments: {}", output.coverage.ambiguous_segments);
                println!("Labeled calls: {}", output.coverage.labeled_calls);
                println!("Ambiguous calls: {}", output.coverage.ambiguous_calls);
                println!("Uncovered calls: {}", output.coverage.uncovered_calls);
                if !output.uncovered_call_indices.is_empty() {
                    println!(
                        "Uncovered indices: {}",
                        join_indices(&output.uncovered_call_indices)
                    );
                }
                if !output.uncovered_spans.is_empty() {
                    println!(
                        "Uncovered spans: {}",
                        output
                            .uncovered_spans
                            .iter()
                            .map(|span| format!("{}..={}", span.start_index, span.end_index))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                println!();
                println!("Sequence Signals");
                println!("{}", "-".repeat(40));
                println!("Search calls: {}", output.signals.search_calls);
                println!("Read calls: {}", output.signals.read_calls);
                println!("Browse calls: {}", output.signals.browse_calls);
                println!("Edit calls: {}", output.signals.edit_calls);
                println!("Failed calls: {}", output.signals.failed_calls);
                println!(
                    "Repeated search runs: {}",
                    output.signals.repeated_search_runs
                );
                println!("Directory pivots: {}", output.signals.directory_pivots);
                println!();
                println!("Intent Segments");
                println!("{}", "-".repeat(40));
                for segment in &output.segments {
                    println!(
                        "[{}] {} {}..={} confidence={:?}",
                        segment.segment_index,
                        format_segment_descriptor(segment.status, segment.label),
                        segment.start_index,
                        segment.end_index,
                        segment.confidence
                    );
                    println!("  turns ......... {}", join_turns(&segment.turns));
                    println!("  rationale ..... {}", segment.rationale);
                    for call in &segment.calls {
                        println!(
                            "  call .......... [{}] {} | {}",
                            call.index, call.tool_name, call.summary
                        );
                    }
                    println!();
                }
                if !output.uncovered_spans.is_empty() {
                    println!("Uncovered Regions");
                    println!("{}", "-".repeat(40));
                    for span in &output.uncovered_spans {
                        println!(
                            "{}..={} calls={}",
                            span.start_index,
                            span.end_index,
                            join_indices(&span.call_indices)
                        );
                        println!("  rationale ..... {}", span.rationale);
                    }
                    println!();
                }
                println!("Overall rationale");
                println!("{}", "-".repeat(40));
                println!("{}", output.overall_rationale);
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "persisted_artifact_path": persisted_path,
                    "run": segmented,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
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
                let summary = record.outcome_summary();
                let raw_usage = if summary.total_token_usage.total_tokens == 0 {
                    load_full_response_usage_totals(&record_path, &record)
                        .ok()
                        .flatten()
                } else {
                    None
                };
                let display_usage = raw_usage.as_ref().unwrap_or(&summary.total_token_usage);
                println!("Run summary");
                println!("{}", "-".repeat(40));
                println!("Turns: {}", summary.turn_count);
                println!(
                    "Token usage: {}",
                    format_usage_triplet(
                        display_usage.prompt_tokens,
                        display_usage.completion_tokens,
                        display_usage.total_tokens,
                    )
                );
                println!("Token cost: ${:.6}", summary.total_token_cost);
                if raw_usage.is_some() {
                    println!("Usage source: raw response sidecar");
                    println!("Usage note: may undercount if final stop response was not captured");
                }
                println!("Wall time: {:.3}s", summary.wall_clock_secs);
                println!();
                println!("{:<6} {:<6} {:<7} {}", "Turn", "Tools", "Failed", "Outcome");
                println!("{}", "-".repeat(40));
                for turn in record.conversations() {
                    let tool_count = turn.tool_calls().len();
                    println!(
                        "{:<6} {:<6} {:<7} {}",
                        turn.turn_number,
                        tool_count,
                        failed_tool_count(turn),
                        truncate_for_table(&summarize_turn_outcome(turn), 18),
                    );
                }
                println!("\nTotal turns: {}", record.conversations().count());
                if let Some(first_turn) = record.conversations().next() {
                    println!("Next:");
                    println!("  ploke-eval inspect turn {}", first_turn.turn_number);
                    println!(
                        "  ploke-eval inspect turn {} --show tool-calls",
                        first_turn.turn_number
                    );
                    println!(
                        "  ploke-eval inspect turn {} --show messages",
                        first_turn.turn_number
                    );
                }
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

        let indexed_tool_calls = indexed_tool_calls(&record);

        if let Some(index) = self.index {
            match self.format {
                InspectOutputFormat::Table => match indexed_tool_calls.get(index) {
                    Some((_, turn, call)) => {
                        print_tool_call_detail(*turn, index, call, self.full);
                    }
                    None => {
                        println!(
                            "Error: Index {} out of range. Run has {} tool call{} (valid indices: 0..{}).",
                            index,
                            indexed_tool_calls.len(),
                            if indexed_tool_calls.len() == 1 {
                                ""
                            } else {
                                "s"
                            },
                            indexed_tool_calls.len().saturating_sub(1)
                        );
                    }
                },
                InspectOutputFormat::Json => match indexed_tool_calls.get(index) {
                    Some((_, turn, call)) => {
                        let payload = serde_json::json!({
                            "index": index,
                            "turn": turn,
                            "tool_call": call,
                        });
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&payload)
                                .map_err(PrepareError::Serialize)?
                        );
                    }
                    None => {
                        println!(
                            "Error: Index {} out of range. Run has {} tool call{} (valid indices: 0..{}).",
                            index,
                            indexed_tool_calls.len(),
                            if indexed_tool_calls.len() == 1 {
                                ""
                            } else {
                                "s"
                            },
                            indexed_tool_calls.len().saturating_sub(1)
                        );
                    }
                },
            }
            return Ok(());
        }

        match self.format {
            InspectOutputFormat::Table => {
                println!(
                    "{:<5} {:<6} {:<20} {:<48} {}",
                    "Idx", "Turn", "Tool", "Context", "Result"
                );
                println!("{}", "-".repeat(120));
                for (index, turn, call) in &indexed_tool_calls {
                    println!(
                        "{:<5} {:<6} {:<20} {:<48} {}",
                        index,
                        turn,
                        truncate_for_table(&call.request.tool, 18),
                        truncate_for_table(&summarize_tool_inputs(&call.request.arguments), 46),
                        truncate_for_table(&summarize_tool_result(&call.result), 28),
                    );
                }
                println!("\nTotal tool calls: {}", indexed_tool_calls.len());
                if !indexed_tool_calls.is_empty() {
                    println!("Next:");
                    let next_index = tool_call_next_step_index(indexed_tool_calls.len())
                        .expect("non-empty tool call list should have a next-step index");
                    println!("  ploke-eval inspect tool-calls {}", next_index);
                    println!("  ploke-eval inspect tool-calls --full {}", next_index);
                }
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &indexed_tool_calls
                            .iter()
                            .map(|(index, turn, call)| {
                                serde_json::json!({
                                    "index": index,
                                    "turn": turn,
                                    "tool_call": call,
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                    .map_err(PrepareError::Serialize)?
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

        let turn_number =
            self.turn
                .or(self.turn_flag)
                .ok_or_else(|| PrepareError::DatabaseSetup {
                    phase: "inspect_turn",
                    detail: "Turn number is required (for example: `inspect turn 1`)".to_string(),
                })?;

        let turn = record
            .turn_record(turn_number)
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "inspect_turn",
                detail: format!("Turn {} not found", turn_number),
            })?;

        match self.show {
            TurnShowOption::All => {
                print_turn_summary(turn);
                println!();
                println!("Next:");
                println!(
                    "  ploke-eval inspect turn {} --show responses",
                    turn.turn_number
                );
                println!("  ploke-eval inspect turn {} --show loop", turn.turn_number);
                println!(
                    "  ploke-eval inspect turn {} --show tool-calls",
                    turn.turn_number
                );
                println!(
                    "  ploke-eval inspect turn {} --show messages",
                    turn.turn_number
                );
                println!(
                    "  ploke-eval inspect turn {} --show messages --exclude-roles system,user",
                    turn.turn_number
                );
            }
            TurnShowOption::Messages => {
                let messages = filter_messages(turn.messages(), &self.roles, &self.exclude_roles);
                match self.format {
                    InspectOutputFormat::Table => {
                        println!("{}", render_messages_table(&messages));
                        println!(
                            "\nNext:\n  ploke-eval inspect turn {} --show messages --exclude-roles system,user",
                            turn.turn_number
                        );
                    }
                    InspectOutputFormat::Json => {
                        println!("{}", render_messages_json(&messages)?);
                    }
                }
            }
            TurnShowOption::Responses => {
                let trace_path = resolve_full_response_trace_path(&record_path, &record)?;
                let assistant_message_id =
                    assistant_message_id_for_turn(turn).ok_or_else(|| {
                        PrepareError::DatabaseSetup {
                            phase: "inspect_turn",
                            detail: format!(
                                "Turn {} does not expose an assistant_message_id in its artifact",
                                turn.turn_number
                            ),
                        }
                    })?;
                let responses =
                    load_full_response_records_for_turn(&trace_path, assistant_message_id)?;
                if responses.is_empty() {
                    println!("No raw full responses captured for this turn.");
                } else {
                    match self.format {
                        InspectOutputFormat::Table => {
                            println!(
                                "{}",
                                render_full_response_table(turn.turn_number, &responses)
                            );
                        }
                        InspectOutputFormat::Json => {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&responses)
                                    .map_err(PrepareError::Serialize)?
                            );
                        }
                    }
                }
            }
            TurnShowOption::Loop => {
                let tool_calls = turn.tool_calls();
                match self.format {
                    InspectOutputFormat::Table => {
                        println!("{}", render_tool_loop_table(turn.turn_number, &tool_calls));
                        if !tool_calls.is_empty() {
                            println!("\nNext:");
                            println!(
                                "  ploke-eval inspect turn {} --show tool-call --index 0",
                                turn.turn_number
                            );
                            println!(
                                "  ploke-eval inspect turn {} --show tool-result --index 0",
                                turn.turn_number
                            );
                        }
                    }
                    InspectOutputFormat::Json => {
                        println!("{}", render_tool_loop_json(turn.turn_number, &tool_calls)?);
                    }
                }
            }
            TurnShowOption::ToolCalls => {
                let tool_calls = turn.tool_calls();
                if tool_calls.is_empty() {
                    println!("No tool calls in this turn.");
                } else {
                    match self.format {
                        InspectOutputFormat::Table => {
                            println!("{:<5} {:<20} {:<48} {}", "Idx", "Tool", "Context", "Result");
                            println!("{}", "-".repeat(110));
                            for (index, call) in tool_calls.iter().enumerate() {
                                println!(
                                    "{:<5} {:<20} {:<48} {}",
                                    index,
                                    truncate_for_table(&call.request.tool, 18),
                                    truncate_for_table(
                                        &summarize_tool_inputs(&call.request.arguments),
                                        46
                                    ),
                                    truncate_for_table(&summarize_tool_result(&call.result), 28),
                                );
                            }
                            println!("\nNext:");
                            println!(
                                "  ploke-eval inspect turn {} --show tool-call --index 0",
                                turn.turn_number
                            );
                            println!(
                                "  ploke-eval inspect turn {} --show tool-result --index 0",
                                turn.turn_number
                            );
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
                                print_tool_call_detail(turn.turn_number, idx, tool_call, false);
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
                                print_tool_call_detail(turn.turn_number, 0, tool_call, false);
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
                        let tool_call = &tool_calls[idx];
                        match self.format {
                            InspectOutputFormat::Table => {
                                print_tool_result_detail(idx, &tool_call.result, false);
                            }
                            InspectOutputFormat::Json => {
                                let result = &tool_calls[idx].result;
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&result)
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
                    (None, 1) => match self.format {
                        InspectOutputFormat::Table => {
                            print_tool_result_detail(0, &tool_calls[0].result, false);
                        }
                        InspectOutputFormat::Json => {
                            let result = &tool_calls[0].result;
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&result)
                                    .map_err(PrepareError::Serialize)?
                            );
                        }
                    },
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

impl InspectProtocolArtifactsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let record_path = resolve_record_path(self.record, self.instance)?;
        let artifacts = list_protocol_artifacts(&record_path)?;

        if let Some(index) = self.index {
            let selected = artifacts
                .get(index)
                .ok_or_else(|| PrepareError::DatabaseSetup {
                    phase: "inspect_protocol_artifacts",
                    detail: format!(
                        "protocol artifact index {} out of range ({} artifact{})",
                        index,
                        artifacts.len(),
                        if artifacts.len() == 1 { "" } else { "s" }
                    ),
                })?;
            match self.format {
                InspectOutputFormat::Table => {
                    print_protocol_artifact_detail(index, selected, self.full);
                }
                InspectOutputFormat::Json => {
                    if self.full {
                        let payload = serde_json::json!({
                            "index": index,
                            "path": selected.path,
                            "artifact": selected.stored,
                        });
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&payload)
                                .map_err(PrepareError::Serialize)?
                        );
                    } else {
                        let payload = serde_json::json!({
                            "index": index,
                            "path": selected.path,
                            "procedure_name": selected.stored.procedure_name,
                            "subject_id": selected.stored.subject_id,
                            "run_id": selected.stored.run_id,
                            "created_at_ms": selected.stored.created_at_ms,
                            "model_id": selected.stored.model_id,
                            "provider_slug": selected.stored.provider_slug,
                            "summary": protocol_artifact_summary(selected),
                            "input_preview": protocol_artifact_preview(&selected.stored.input),
                            "output_preview": protocol_artifact_preview(&selected.stored.output),
                            "artifact_preview": protocol_artifact_preview(&selected.stored.artifact),
                        });
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&payload)
                                .map_err(PrepareError::Serialize)?
                        );
                    }
                }
            }
            return Ok(());
        }

        match self.format {
            InspectOutputFormat::Table => {
                if artifacts.is_empty() {
                    println!("No persisted protocol artifacts for this run.");
                } else {
                    println!(
                        "{:<5} {:<30} {:<28} {:<16} Summary",
                        "Idx", "Procedure", "Subject", "Model"
                    );
                    println!("{}", "-".repeat(120));
                    for (index, entry) in artifacts.iter().enumerate() {
                        println!(
                            "{:<5} {:<30} {:<28} {:<16} {}",
                            index,
                            truncate_for_table(&entry.stored.procedure_name, 28),
                            truncate_for_table(&entry.stored.subject_id, 26),
                            truncate_for_table(entry.stored.model_id.as_deref().unwrap_or("-"), 14),
                            truncate_for_table(&protocol_artifact_summary(entry), 42),
                        );
                    }
                    println!("\nTotal protocol artifacts: {}", artifacts.len());
                    println!("Next:");
                    println!("  ploke-eval inspect protocol-artifacts 0");
                    println!("  ploke-eval inspect protocol-artifacts 0 --full");
                }
            }
            InspectOutputFormat::Json => {
                let payload: Vec<_> = artifacts
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        serde_json::json!({
                            "index": index,
                            "path": entry.path,
                            "procedure_name": entry.stored.procedure_name,
                            "subject_id": entry.stored.subject_id,
                            "created_at_ms": entry.stored.created_at_ms,
                            "model_id": entry.stored.model_id,
                            "provider_slug": entry.stored.provider_slug,
                            "summary": protocol_artifact_summary(entry),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
    }
}

impl InspectProtocolOverviewCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        if let Some(campaign_id) = self.campaign.clone() {
            let report = collect_protocol_campaign_triage_report(&campaign_id, &self)?;
            return match self.format {
                InspectOutputFormat::Table => {
                    print!(
                        "{}",
                        render_protocol_campaign_triage_report(&report, self.width.max(88),)
                    );
                    Ok(())
                }
                InspectOutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                    );
                    Ok(())
                }
            };
        }

        if self.all_runs {
            let summaries = collect_protocol_run_summaries()?;
            return match self.format {
                InspectOutputFormat::Table => {
                    print_protocol_run_summaries(&summaries, &self);
                    Ok(())
                }
                InspectOutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&summaries)
                            .map_err(PrepareError::Serialize)?
                    );
                    Ok(())
                }
            };
        }

        let record_path = resolve_record_path(self.record.clone(), self.instance.clone())?;
        let aggregate =
            load_protocol_aggregate(&record_path).map_err(|err| PrepareError::DatabaseSetup {
                phase: "inspect_protocol_overview",
                detail: err.to_string(),
            })?;
        let mut report = build_protocol_report(&aggregate)?;
        apply_protocol_report_filters(&mut report, &self);

        match self.format {
            InspectOutputFormat::Table => {
                let use_color = match self.color {
                    ProtocolColorMode::Always => true,
                    ProtocolColorMode::Never => false,
                    ProtocolColorMode::Auto => std::io::stdout().is_terminal(),
                };
                let rendered = render_protocol_aggregate_report_with_options(
                    &report,
                    ProtocolReportRenderOptions {
                        width: self.width,
                        use_color,
                        top_call_issues: self.limit,
                        color_profile: self.color_profile.into(),
                    },
                );
                print!("{rendered}");
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
struct ProtocolCampaignQuery {
    issue: Option<String>,
    tool: Option<String>,
    status: Option<String>,
}

impl ProtocolCampaignQuery {
    fn from_command(command: &InspectProtocolOverviewCommand) -> Self {
        Self {
            issue: command.issue.as_deref().map(normalize_filter_value),
            tool: command.tool.as_deref().map(normalize_filter_value),
            status: command.status.as_deref().map(normalize_filter_value),
        }
    }

    fn matches_entry(&self, entry: &ProtocolRunSummaryRecord) -> bool {
        if let Some(status) = self.status.as_deref() {
            if entry.summary.protocol_status != status {
                return false;
            }
        }

        if self.issue.is_none() && self.tool.is_none() {
            return true;
        }

        entry
            .report
            .as_ref()
            .map(|report| {
                report
                    .call_issues
                    .iter()
                    .any(|row| self.matches_call_issue(row))
            })
            .unwrap_or(false)
    }

    fn matches_call_issue(&self, row: &ProtocolAggregateCallIssueRow) -> bool {
        if let Some(issue) = self.issue.as_deref() {
            let row_issue = row
                .issue
                .as_deref()
                .map(normalize_filter_value)
                .unwrap_or_default();
            if row_issue != issue {
                return false;
            }
        }

        if let Some(tool) = self.tool.as_deref() {
            let row_tool = row
                .tool_name
                .as_deref()
                .map(normalize_filter_value)
                .unwrap_or_default();
            if !row_tool.contains(tool) {
                return false;
            }
        }

        true
    }

    fn scope_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(status) = self.status.as_deref() {
            parts.push(format!("status={status}"));
        }
        if let Some(issue) = self.issue.as_deref() {
            parts.push(format!("issue={issue}"));
        }
        if let Some(tool) = self.tool.as_deref() {
            parts.push(format!("tool={tool}"));
        }
        if parts.is_empty() {
            "campaign-wide protocol triage".to_string()
        } else {
            format!("campaign protocol triage filtered by {}", parts.join(", "))
        }
    }
}

#[derive(Debug, Clone)]
struct ProtocolRunSummaryRecord {
    summary: ProtocolRunSummaryRow,
    report: Option<ProtocolAggregateReport>,
}

fn collect_protocol_campaign_triage_report(
    campaign_id: &str,
    command: &InspectProtocolOverviewCommand,
) -> Result<ProtocolCampaignTriageReport, PrepareError> {
    let state = load_closure_state(campaign_id)?;
    let query = ProtocolCampaignQuery::from_command(command);

    let mut entries = Vec::new();
    for row in state
        .instances
        .iter()
        .filter(|row| row.eval_status == ClosureClass::Complete)
    {
        entries.push(collect_protocol_campaign_summary_record(row)?);
    }

    let campaign_runs = entries.len();
    let selected_entries = entries
        .iter()
        .filter(|entry| query.matches_entry(entry))
        .collect::<Vec<_>>();

    let mut issue_kind_counts = BTreeMap::<String, (usize, BTreeSet<String>)>::new();
    let mut issue_tool_counts = BTreeMap::<String, (usize, BTreeSet<String>)>::new();
    let mut segment_label_counts = BTreeMap::<String, (usize, BTreeSet<String>)>::new();
    let mut segment_status_counts = BTreeMap::<String, (usize, BTreeSet<String>)>::new();
    let mut summary = ProtocolCampaignSummary::default();
    let mut evidence = ProtocolCampaignEvidence::default();
    let mut problem_families = Vec::new();
    let mut exemplars = Vec::new();
    let mut filtered_matching_runs = BTreeSet::new();

    let mut artifact_error_runs = Vec::new();
    let mut missing_coverage_runs = Vec::new();
    let mut ineligible_runs = Vec::new();
    let mut high_issue_runs = Vec::new();

    for entry in selected_entries.iter().copied() {
        match entry.summary.protocol_status.as_str() {
            "ineligible" => summary.ineligible_runs += 1,
            _ => {
                summary.eligible_runs += 1;
                match entry.summary.protocol_status.as_str() {
                    "full" => summary.full_runs += 1,
                    "partial" => summary.partial_runs += 1,
                    "error" => summary.error_runs += 1,
                    "missing" => summary.missing_runs += 1,
                    _ => {}
                }
            }
        }

        evidence.total_tool_calls += entry.summary.tool_calls_total;
        evidence.missing_tool_call_reviews += entry.summary.missing_call_reviews;
        if entry.summary.protocol_status == "error" {
            evidence.artifact_failure_runs += 1;
        }

        if let Some(report) = entry.report.as_ref() {
            evidence.reviewed_tool_calls += report.coverage.reviewed_tool_calls;
            evidence.known_segments += report.coverage.total_segments;
            evidence.usable_segment_reviews += report.coverage.usable_segment_reviews;
            evidence.mismatched_segment_reviews += report.coverage.mismatched_segment_reviews;
            evidence.missing_segment_reviews += report.coverage.missing_segment_indices.len();
            evidence.duplicate_artifacts += report.coverage.duplicate_artifacts.unwrap_or(0);

            let matching_issues = report
                .call_issues
                .iter()
                .filter(|row| query.matches_call_issue(row))
                .collect::<Vec<_>>();

            if !matching_issues.is_empty() {
                filtered_matching_runs.insert(entry.summary.run_id.clone());
                let segment_rows = report
                    .segments
                    .iter()
                    .map(|row| (row.index, row))
                    .collect::<BTreeMap<_, _>>();
                for row in matching_issues {
                    if let Some(issue) = row.issue.as_deref() {
                        let (count, runs) = issue_kind_counts
                            .entry(issue.to_string())
                            .or_insert_with(|| (0usize, BTreeSet::new()));
                        *count += 1;
                        runs.insert(entry.summary.run_id.clone());
                    }
                    if let Some(tool) = row.tool_name.as_deref() {
                        let (count, runs) = issue_tool_counts
                            .entry(tool.to_string())
                            .or_insert_with(|| (0usize, BTreeSet::new()));
                        *count += 1;
                        runs.insert(entry.summary.run_id.clone());
                    }
                    if let Some(segment_index) = row.segment_index {
                        if let Some(segment) = segment_rows.get(&segment_index) {
                            if let Some(label) = segment.label.as_deref() {
                                let (count, runs) = segment_label_counts
                                    .entry(label.to_string())
                                    .or_insert_with(|| (0usize, BTreeSet::new()));
                                *count += 1;
                                runs.insert(entry.summary.run_id.clone());
                            }
                            if let Some(status) = segment.status.as_deref() {
                                let (count, runs) = segment_status_counts
                                    .entry(status.to_string())
                                    .or_insert_with(|| (0usize, BTreeSet::new()));
                                *count += 1;
                                runs.insert(entry.summary.run_id.clone());
                            }
                        }
                    }
                }
            }

            if entry.summary.call_issues > 0 {
                high_issue_runs.push(entry);
            }
        }

        if entry.summary.protocol_status == "error" {
            artifact_error_runs.push(entry);
        } else if entry.summary.protocol_status == "partial"
            || entry.summary.protocol_status == "missing"
        {
            missing_coverage_runs.push(entry);
        } else if entry.summary.protocol_status == "ineligible" {
            ineligible_runs.push(entry);
        }
    }

    summary.runs_with_issue_calls = if query.issue.is_some() || query.tool.is_some() {
        filtered_matching_runs.len()
    } else {
        selected_entries
            .iter()
            .filter(|entry| entry.summary.call_issues > 0)
            .count()
    };

    let mut issue_kinds = count_rows_from_map(issue_kind_counts);
    let mut issue_tools = count_rows_from_map(issue_tool_counts);
    let nearby_segment_labels = count_rows_from_map(segment_label_counts);
    let nearby_segment_statuses = count_rows_from_map(segment_status_counts);

    if query.issue.is_none() && query.tool.is_none() {
        if !artifact_error_runs.is_empty() {
            problem_families.push(problem_family_for_status_group(
                "artifact/schema failures",
                "artifact_schema_failure",
                &artifact_error_runs,
                "ploke-protocol / ploke-eval",
                "protocol artifact compatibility is blocking interpretation",
                "reduce error-status runs to zero",
            ));
        }
        if !missing_coverage_runs.is_empty() {
            problem_families.push(problem_family_for_status_group(
                "missing review coverage",
                "missing_review_coverage",
                &missing_coverage_runs,
                "ploke-eval protocol execution",
                "reviews are incomplete, so the campaign cannot fully explain tool behavior yet",
                "reduce partial+missing protocol rows",
            ));
        }
        for row in issue_kinds.iter().take(3) {
            let exemplars_for_issue = selected_entries
                .iter()
                .filter(|entry| {
                    entry
                        .report
                        .as_ref()
                        .map(|report| {
                            report.call_issues.iter().any(|issue| {
                                issue
                                    .issue
                                    .as_deref()
                                    .map(|value| value == row.label)
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>();
            if !exemplars_for_issue.is_empty() {
                problem_families.push(ProtocolCampaignFamilyRow {
                    label: format!("issue: {}", row.label),
                    family_kind: "issue_family".to_string(),
                    affected_runs: row.affected_runs,
                    affected_calls: row.count,
                    likely_owner: "ploke-tui tool harness".to_string(),
                    exemplar_run: exemplars_for_issue
                        .first()
                        .map(|entry| entry.summary.run_id.clone()),
                    note: Some(
                        "completed protocol data still shows friction in this call family"
                            .to_string(),
                    ),
                    success_metric: Some(format!(
                        "lower {} issue-call count and affected runs",
                        row.label
                    )),
                });
            }
        }
        if !ineligible_runs.is_empty() {
            problem_families.push(problem_family_for_status_group(
                "ineligible zero-tool runs",
                "ineligible",
                &ineligible_runs,
                "campaign/selection hygiene",
                "these rows do not speak to tool friction and should stay outside the main frontier",
                "keep ineligible rows out of runnable protocol work",
            ));
        }
    } else {
        let label = describe_selected_family(&query);
        let affected_calls = selected_entries
            .iter()
            .map(|entry| matching_issue_count(entry, &query))
            .sum();
        problem_families.push(ProtocolCampaignFamilyRow {
            label,
            family_kind: "filtered_family".to_string(),
            affected_runs: selected_entries.len(),
            affected_calls,
            likely_owner: "ploke-tui tool harness".to_string(),
            exemplar_run: selected_entries
                .first()
                .map(|entry| entry.summary.run_id.clone()),
            note: Some("this slice is the current drilldown target".to_string()),
            success_metric: Some(
                "reduce matching calls and affected runs after the next tool/harness change"
                    .to_string(),
            ),
        });
    }

    sort_problem_families(&mut problem_families);

    if query.issue.is_none() && query.tool.is_none() {
        high_issue_runs.sort_by(|left, right| {
            right
                .summary
                .call_issues
                .cmp(&left.summary.call_issues)
                .then_with(|| {
                    right
                        .summary
                        .tool_calls_total
                        .cmp(&left.summary.tool_calls_total)
                })
        });
        exemplars.extend(
            artifact_error_runs
                .iter()
                .take(1)
                .chain(missing_coverage_runs.iter().take(1))
                .chain(
                    high_issue_runs
                        .iter()
                        .take(command.limit.max(4).saturating_sub(2)),
                )
                .map(|entry| exemplar_row_for_entry(entry, None)),
        );
    } else {
        let mut filtered_entries = selected_entries.clone();
        filtered_entries.sort_by(|left, right| {
            matching_issue_count(right, &query)
                .cmp(&matching_issue_count(left, &query))
                .then_with(|| right.summary.call_issues.cmp(&left.summary.call_issues))
                .then_with(|| {
                    right
                        .summary
                        .tool_calls_total
                        .cmp(&left.summary.tool_calls_total)
                })
        });
        exemplars.extend(
            filtered_entries
                .into_iter()
                .take(if command.examples {
                    command.limit.max(6)
                } else {
                    command.limit.min(5).max(3)
                })
                .map(|entry| exemplar_row_for_entry(entry, Some(&query))),
        );
    }

    dedupe_exemplars(&mut exemplars);
    issue_kinds.truncate(command.limit.max(3));
    issue_tools.truncate(command.limit.max(3));
    let mut nearby_segment_labels = nearby_segment_labels;
    let mut nearby_segment_statuses = nearby_segment_statuses;
    nearby_segment_labels.truncate(command.limit.max(3));
    nearby_segment_statuses.truncate(command.limit.max(3));
    problem_families.truncate(command.limit.max(4));
    if !command.examples {
        exemplars.truncate(command.limit.min(5).max(4));
    }

    let next_steps = build_triage_next_steps(
        campaign_id,
        &query,
        &problem_families,
        &issue_kinds,
        &issue_tools,
        &exemplars,
    );

    Ok(ProtocolCampaignTriageReport {
        campaign_id: campaign_id.to_string(),
        scope: query.scope_label(),
        issue_filter: command.issue.clone(),
        tool_filter: command.tool.clone(),
        status_filter: command.status.clone(),
        selected_runs: selected_entries.len(),
        campaign_runs,
        summary,
        evidence,
        issue_kinds,
        issue_tools,
        nearby_segment_labels,
        nearby_segment_statuses,
        problem_families,
        exemplars,
        next_steps,
    })
}

fn collect_protocol_campaign_summary_record(
    row: &crate::closure::ClosureInstanceRow,
) -> Result<ProtocolRunSummaryRecord, PrepareError> {
    if let Some(record_path) = row.artifacts.record_path.as_deref() {
        if record_path.exists() {
            return collect_protocol_run_summary_record(record_path);
        }
    }
    Ok(ProtocolRunSummaryRecord {
        summary: protocol_summary_row_from_closure_row(row),
        report: None,
    })
}

fn protocol_summary_row_from_closure_row(
    row: &crate::closure::ClosureInstanceRow,
) -> ProtocolRunSummaryRow {
    let protocol_status = match row.protocol_status {
        ClosureClass::Complete => "full",
        ClosureClass::Partial => "partial",
        ClosureClass::Missing => "missing",
        ClosureClass::Ineligible => "ineligible",
        ClosureClass::Failed | ClosureClass::Incompatible => "error",
    };
    let counts = row.protocol_counts.as_ref();
    let total_calls = counts.map(|counts| counts.total_calls).unwrap_or(0);
    let reviewed_calls = counts.map(|counts| counts.reviewed_calls).unwrap_or(0);
    let total_segments = counts.map(|counts| counts.total_segments).unwrap_or(0);
    let usable_segments = counts.map(|counts| counts.usable_segments).unwrap_or(0);
    ProtocolRunSummaryRow {
        run_id: row.instance_id.clone(),
        subject_id: row.instance_id.clone(),
        protocol_status: protocol_status.to_string(),
        call_review_ratio: ratio(reviewed_calls, total_calls),
        usable_segment_ratio: ratio(usable_segments, total_segments),
        tool_calls_total: total_calls,
        segments_total: total_segments,
        call_issues: 0,
        mismatched_segment_reviews: counts.map(|counts| counts.mismatched_segments).unwrap_or(0),
        missing_call_reviews: total_calls.saturating_sub(reviewed_calls),
        missing_segment_reviews: counts.map(|counts| counts.missing_segments).unwrap_or(0),
        note: row.protocol_failure.clone(),
    }
}

fn count_rows_from_map(
    rows: BTreeMap<String, (usize, BTreeSet<String>)>,
) -> Vec<ProtocolCampaignCountRow> {
    let mut counts = rows
        .into_iter()
        .map(|(label, (count, runs))| ProtocolCampaignCountRow {
            label,
            count,
            affected_runs: runs.len(),
        })
        .collect::<Vec<_>>();
    sort_count_rows(&mut counts);
    counts
}

fn problem_family_for_status_group(
    label: &str,
    family_kind: &str,
    entries: &[&ProtocolRunSummaryRecord],
    likely_owner: &str,
    note: &str,
    success_metric: &str,
) -> ProtocolCampaignFamilyRow {
    let exemplar_run = entries.first().map(|entry| entry.summary.run_id.clone());
    let affected_calls = entries
        .iter()
        .map(|entry| {
            if family_kind == "issue_family" || family_kind == "filtered_family" {
                entry.summary.call_issues
            } else {
                entry.summary.missing_call_reviews + entry.summary.missing_segment_reviews
            }
        })
        .sum();
    ProtocolCampaignFamilyRow {
        label: label.to_string(),
        family_kind: family_kind.to_string(),
        affected_runs: entries.len(),
        affected_calls,
        likely_owner: likely_owner.to_string(),
        exemplar_run,
        note: Some(note.to_string()),
        success_metric: Some(success_metric.to_string()),
    }
}

fn sort_problem_families(rows: &mut [ProtocolCampaignFamilyRow]) {
    rows.sort_by(|left, right| {
        right
            .affected_runs
            .cmp(&left.affected_runs)
            .then_with(|| right.affected_calls.cmp(&left.affected_calls))
            .then_with(|| left.label.cmp(&right.label))
    });
}

fn matching_issue_count(entry: &ProtocolRunSummaryRecord, query: &ProtocolCampaignQuery) -> usize {
    entry
        .report
        .as_ref()
        .map(|report| {
            report
                .call_issues
                .iter()
                .filter(|row| query.matches_call_issue(row))
                .count()
        })
        .unwrap_or(0)
}

fn exemplar_row_for_entry(
    entry: &ProtocolRunSummaryRecord,
    query: Option<&ProtocolCampaignQuery>,
) -> ProtocolCampaignExemplarRow {
    let matching_calls = query
        .map(|query| matching_issue_count(entry, query))
        .unwrap_or(entry.summary.call_issues);
    let focus = if let Some(query) = query {
        Some(describe_selected_family(query))
    } else if entry.summary.protocol_status == "error" {
        Some("artifact/schema failure".to_string())
    } else if entry.summary.protocol_status == "partial"
        || entry.summary.protocol_status == "missing"
    {
        Some("missing protocol coverage".to_string())
    } else if entry.summary.call_issues > 0 {
        entry.report.as_ref().and_then(|report| {
            report
                .call_issues
                .iter()
                .find_map(|row| row.issue.as_deref().map(|value| value.to_string()))
        })
    } else {
        None
    };
    ProtocolCampaignExemplarRow {
        run_id: entry.summary.run_id.clone(),
        protocol_status: entry.summary.protocol_status.clone(),
        matching_calls,
        total_issues: entry.summary.call_issues,
        tool_calls_total: entry.summary.tool_calls_total,
        focus,
        note: entry.summary.note.clone(),
    }
}

fn dedupe_exemplars(rows: &mut Vec<ProtocolCampaignExemplarRow>) {
    let mut seen = BTreeSet::new();
    rows.retain(|row| seen.insert(row.run_id.clone()));
}

fn build_triage_next_steps(
    campaign_id: &str,
    query: &ProtocolCampaignQuery,
    problem_families: &[ProtocolCampaignFamilyRow],
    issue_kinds: &[ProtocolCampaignCountRow],
    issue_tools: &[ProtocolCampaignCountRow],
    exemplars: &[ProtocolCampaignExemplarRow],
) -> Vec<String> {
    let mut steps = Vec::new();
    if query.issue.is_none() {
        if let Some(top_issue) = issue_kinds.first() {
            steps.push(format!(
                "if you want exemplar runs for the top issue family, try `ploke-eval inspect protocol-overview --campaign {campaign_id} --issue {}`",
                top_issue.label
            ));
        }
    }
    if query.tool.is_none() {
        if let Some(top_tool) = issue_tools.first() {
            steps.push(format!(
                "if you want the most suspicious tool slice next, try `ploke-eval inspect protocol-overview --campaign {campaign_id} --tool {}`",
                top_tool.label
            ));
        }
    }
    if query.status.is_none()
        && problem_families
            .iter()
            .any(|family| family.family_kind == "artifact_schema_failure")
    {
        steps.push(format!(
            "if you want only artifact/schema failures, try `ploke-eval inspect protocol-overview --campaign {campaign_id} --status error`"
        ));
    }
    if let Some(exemplar) = exemplars.first() {
        steps.push(format!(
            "if you want the protocol report for the top exemplar, try `ploke-eval inspect protocol-overview --instance {}`",
            exemplar.run_id
        ));
        if exemplar.protocol_status == "error" {
            steps.push(format!(
                "if you want the raw artifact failure for that exemplar, try `ploke-eval inspect protocol-artifacts --instance {} --full`",
                exemplar.run_id
            ));
        } else {
            steps.push(format!(
                "if you want the local tool trace around that exemplar, try `ploke-eval inspect tool-calls --instance {}`",
                exemplar.run_id
            ));
        }
    }
    steps.truncate(4);
    steps
}

fn describe_selected_family(query: &ProtocolCampaignQuery) -> String {
    let mut parts = Vec::new();
    if let Some(status) = query.status.as_deref() {
        parts.push(format!("status={status}"));
    }
    if let Some(issue) = query.issue.as_deref() {
        parts.push(format!("issue={issue}"));
    }
    if let Some(tool) = query.tool.as_deref() {
        parts.push(format!("tool={tool}"));
    }
    if parts.is_empty() {
        "campaign slice".to_string()
    } else {
        parts.join(" + ")
    }
}

fn normalize_filter_value(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[derive(Debug, Clone, Serialize)]
struct ProtocolRunSummaryRow {
    run_id: String,
    subject_id: String,
    protocol_status: String,
    call_review_ratio: f32,
    usable_segment_ratio: f32,
    tool_calls_total: usize,
    segments_total: usize,
    call_issues: usize,
    mismatched_segment_reviews: usize,
    missing_call_reviews: usize,
    missing_segment_reviews: usize,
    note: Option<String>,
}

#[derive(Debug, Clone)]
struct SegmentEvidenceCounts {
    usable: usize,
    mismatched: usize,
    missing: usize,
    missing_indices: Vec<usize>,
}

fn collect_protocol_run_summaries() -> Result<Vec<ProtocolRunSummaryRow>, PrepareError> {
    let mut summaries = Vec::new();
    for record_path in collect_finished_record_paths()? {
        summaries.push(collect_protocol_run_summary_record(&record_path)?.summary);
    }
    summaries.sort_by(|left, right| {
        let left_evidence_concerns = left.mismatched_segment_reviews
            + left.missing_segment_reviews
            + left.missing_call_reviews;
        let right_evidence_concerns = right.mismatched_segment_reviews
            + right.missing_segment_reviews
            + right.missing_call_reviews;
        protocol_summary_status_rank(&left.protocol_status)
            .cmp(&protocol_summary_status_rank(&right.protocol_status))
            .then_with(|| right_evidence_concerns.cmp(&left_evidence_concerns))
            .then_with(|| right.call_issues.cmp(&left.call_issues))
            .then_with(|| right.tool_calls_total.cmp(&left.tool_calls_total))
    });
    Ok(summaries)
}

fn collect_protocol_run_summary_record(
    record_path: &Path,
) -> Result<ProtocolRunSummaryRecord, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let run_id = record_path
        .parent()
        .and_then(|path| path.file_name())
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| record.manifest_id.clone());
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let tool_calls_total = record.tool_calls().len();
    let artifacts =
        list_protocol_artifacts(record_path).map_err(|err| PrepareError::DatabaseSetup {
            phase: "inspect_protocol_overview",
            detail: err.to_string(),
        })?;

    match load_protocol_aggregate_from_artifacts(record_path, artifacts) {
        Ok(aggregate) => {
            let report = build_protocol_report(&aggregate)?;
            let summary = protocol_summary_row_from_aggregate_with_report(&aggregate, &report);
            Ok(ProtocolRunSummaryRecord {
                summary,
                report: Some(report),
            })
        }
        Err(ProtocolAggregateError::MissingAnchor { .. }) if tool_calls_total == 0 => {
            Ok(ProtocolRunSummaryRecord {
                summary: ProtocolRunSummaryRow {
                    run_id,
                    subject_id,
                    protocol_status: "ineligible".to_string(),
                    call_review_ratio: 0.0,
                    usable_segment_ratio: 0.0,
                    tool_calls_total,
                    segments_total: 0,
                    call_issues: 0,
                    mismatched_segment_reviews: 0,
                    missing_call_reviews: 0,
                    missing_segment_reviews: 0,
                    note: Some("zero tool calls".to_string()),
                },
                report: None,
            })
        }
        Err(ProtocolAggregateError::MissingAnchor { .. }) => Ok(ProtocolRunSummaryRecord {
            summary: ProtocolRunSummaryRow {
                run_id,
                subject_id,
                protocol_status: "missing".to_string(),
                call_review_ratio: 0.0,
                usable_segment_ratio: 0.0,
                tool_calls_total,
                segments_total: 0,
                call_issues: 0,
                mismatched_segment_reviews: 0,
                missing_call_reviews: tool_calls_total,
                missing_segment_reviews: 0,
                note: Some("no intent segmentation artifact".to_string()),
            },
            report: None,
        }),
        Err(err) => Ok(ProtocolRunSummaryRecord {
            summary: ProtocolRunSummaryRow {
                run_id,
                subject_id,
                protocol_status: "error".to_string(),
                call_review_ratio: 0.0,
                usable_segment_ratio: 0.0,
                tool_calls_total,
                segments_total: 0,
                call_issues: 0,
                mismatched_segment_reviews: 0,
                missing_call_reviews: 0,
                missing_segment_reviews: 0,
                note: Some(err.to_string()),
            },
            report: None,
        }),
    }
}

fn protocol_summary_row_from_aggregate_with_report(
    aggregate: &ProtocolAggregate,
    report: &ProtocolAggregateReport,
) -> ProtocolRunSummaryRow {
    let segment_evidence = segment_evidence_counts(aggregate);
    let missing_call_reviews = aggregate.coverage.missing_call_indices.len();
    let missing_segment_reviews = segment_evidence.missing;
    let mismatched_segment_reviews = segment_evidence.mismatched;
    let protocol_status = if missing_call_reviews == 0
        && missing_segment_reviews == 0
        && mismatched_segment_reviews == 0
    {
        "full"
    } else {
        "partial"
    };

    ProtocolRunSummaryRow {
        run_id: aggregate.run.run_id.clone(),
        subject_id: aggregate.run.subject_id.clone(),
        protocol_status: protocol_status.to_string(),
        call_review_ratio: ratio(
            aggregate.coverage.reviewed_call_count,
            aggregate.coverage.total_calls_in_run,
        ),
        usable_segment_ratio: ratio(
            segment_evidence.usable,
            aggregate.coverage.total_segments_in_anchor,
        ),
        tool_calls_total: aggregate.coverage.total_calls_in_run,
        segments_total: aggregate.coverage.total_segments_in_anchor,
        call_issues: report.call_issues.len(),
        mismatched_segment_reviews,
        missing_call_reviews,
        missing_segment_reviews,
        note: None,
    }
}

fn protocol_summary_status_rank(status: &str) -> u8 {
    match status {
        "error" => 0,
        "missing" => 1,
        "partial" => 2,
        "full" => 3,
        "ineligible" => 4,
        _ => 5,
    }
}

fn segment_evidence_counts(aggregate: &ProtocolAggregate) -> SegmentEvidenceCounts {
    let accepted_segment_indices = aggregate
        .segment_reviews
        .iter()
        .map(|row| row.basis.segment_index)
        .collect::<std::collections::BTreeSet<_>>();
    let mismatched_segment_indices = aggregate
        .skipped_segment_reviews
        .iter()
        .map(|row| row.segment_index)
        .collect::<std::collections::BTreeSet<_>>();

    let mut usable = 0usize;
    let mut mismatched = 0usize;
    let mut missing_indices = Vec::new();

    for basis in &aggregate.segmentation.segments {
        if accepted_segment_indices.contains(&basis.segment_index) {
            usable += 1;
        } else if mismatched_segment_indices.contains(&basis.segment_index) {
            mismatched += 1;
        } else {
            missing_indices.push(basis.segment_index);
        }
    }

    SegmentEvidenceCounts {
        usable,
        mismatched,
        missing: missing_indices.len(),
        missing_indices,
    }
}

fn collect_finished_record_paths() -> Result<Vec<PathBuf>, PrepareError> {
    let root = runs_dir()?;
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(&root).map_err(|source| PrepareError::ReadManifest {
        path: root.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: root.clone(),
            source,
        })?;
        let path = entry.path().join("record.json.gz");
        if path.exists() {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn build_protocol_report(
    aggregate: &ProtocolAggregate,
) -> Result<ProtocolAggregateReport, PrepareError> {
    let segment_evidence = segment_evidence_counts(aggregate);
    let call_details = load_anchor_call_details(aggregate)?;
    let segment_review_by_index = aggregate
        .segment_reviews
        .iter()
        .map(|row| (row.basis.segment_index, row))
        .collect::<BTreeMap<_, _>>();

    let segments = aggregate
        .segmentation
        .segments
        .iter()
        .map(|basis| {
            let reviewed_call_count = aggregate
                .call_reviews
                .iter()
                .filter(|row| row.segment_index == Some(basis.segment_index))
                .count();
            let segment_review = segment_review_by_index.get(&basis.segment_index);
            let note = if aggregate
                .skipped_segment_reviews
                .iter()
                .any(|row| row.segment_index == basis.segment_index)
            {
                Some("mismatch with current anchor".to_string())
            } else if segment_review.is_none() {
                Some("missing segment review".to_string())
            } else if reviewed_call_count < basis.call_count {
                Some(format!(
                    "call coverage {reviewed_call_count}/{}",
                    basis.call_count
                ))
            } else {
                None
            };
            ProtocolAggregateSegmentRow {
                index: basis.segment_index,
                label: Some(basis.label.clone()),
                call_span: Some(format!("{}..{}", basis.start_index, basis.end_index)),
                status: Some(
                    segment_review
                        .map(|review| review.overall.clone())
                        .unwrap_or_else(|| basis.status.clone()),
                ),
                evidence: Some(
                    if aggregate
                        .skipped_segment_reviews
                        .iter()
                        .any(|row| row.segment_index == basis.segment_index)
                    {
                        "mismatched".to_string()
                    } else if segment_review.is_some() {
                        "usable".to_string()
                    } else {
                        "missing".to_string()
                    },
                ),
                confidence: segment_review
                    .and_then(|review| {
                        confidence_fraction(Some(review.overall_confidence.as_str()))
                    })
                    .or_else(|| confidence_fraction(basis.confidence.as_deref())),
                note,
                call_refs: basis.call_indices.clone(),
            }
        })
        .collect::<Vec<_>>();

    let call_issues = aggregate
        .call_reviews
        .iter()
        .filter_map(|row| {
            let detail = call_details.get(&row.focal_call_index);
            let issue = primary_call_issue(row);
            if issue.is_none() {
                return None;
            }
            Some(ProtocolAggregateCallIssueRow {
                index: row.focal_call_index,
                turn: row.turn_span.first().copied().map(|turn| turn as u32),
                segment_index: row.segment_index,
                tool_name: detail.map(|detail| detail.tool_name.clone()),
                issue,
                overall: Some(row.overall.clone()),
                detail: detail
                    .map(|detail| detail.summary.clone())
                    .or_else(|| Some(format!("scope={}", join_indices(&row.scope_call_indices)))),
                severity: Some(call_review_severity(row)),
                confidence: confidence_fraction(Some(row.overall_confidence.as_str())),
            })
        })
        .collect::<Vec<_>>();

    let duplicate_artifacts = Some(
        aggregate
            .coverage
            .artifact_counts
            .get("tool_call_review")
            .copied()
            .unwrap_or(0)
            .saturating_sub(aggregate.coverage.reviewed_call_count)
            + aggregate
                .coverage
                .artifact_counts
                .get("tool_call_segment_review")
                .copied()
                .unwrap_or(0)
                .saturating_sub(aggregate.coverage.reviewed_segment_count),
    );

    Ok(ProtocolAggregateReport {
        run_id: aggregate.run.run_id.clone(),
        subject_id: aggregate.run.subject_id.clone(),
        title: Some("Evidence reliability".to_string()),
        generated_at: None,
        scope: Some("protocol-derived evidence".to_string()),
        provenance: vec![
            format!(
                "anchor={} {}",
                aggregate.segmentation.artifact.created_at_ms,
                truncate_middle(
                    aggregate
                        .segmentation
                        .artifact
                        .path
                        .to_string_lossy()
                        .as_ref(),
                    44
                )
            ),
            "derived from intent segmentation + tool call review + segment review".to_string(),
        ],
        coverage: ProtocolAggregateCoverage {
            total_tool_calls: aggregate.coverage.total_calls_in_run,
            reviewed_tool_calls: aggregate.coverage.reviewed_call_count,
            total_segments: aggregate.coverage.total_segments_in_anchor,
            usable_segment_reviews: segment_evidence.usable,
            mismatched_segment_reviews: segment_evidence.mismatched,
            missing_tool_call_indices: aggregate.coverage.missing_call_indices.clone(),
            missing_segment_indices: segment_evidence.missing_indices.clone(),
            duplicate_artifacts,
        },
        segments,
        call_issues,
        notes: if segment_evidence.mismatched > 0 {
            vec![format!(
                "{} segment review artifacts excluded because they do not match the current anchor",
                segment_evidence.mismatched
            )]
        } else {
            Vec::new()
        },
    })
}

fn apply_protocol_report_filters(
    report: &mut ProtocolAggregateReport,
    command: &InspectProtocolOverviewCommand,
) {
    let overall_filter = command.overall.as_deref().map(str::to_lowercase);
    let label_filter = command.segment_label.as_deref().map(str::to_lowercase);
    let tool_filter = command.tool.as_deref().map(str::to_lowercase);

    if let Some(label) = label_filter.as_deref() {
        report.segments.retain(|row| {
            row.label
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case(label))
                .unwrap_or(false)
        });
    }

    if let Some(overall) = overall_filter.as_deref() {
        report.segments.retain(|row| {
            row.status
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case(overall))
                .unwrap_or(false)
        });
        report.call_issues.retain(|row| {
            row.overall
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case(overall))
                .unwrap_or(false)
        });
    }

    if let Some(tool) = tool_filter.as_deref() {
        report.call_issues.retain(|row| {
            row.tool_name
                .as_deref()
                .map(|value| value.to_ascii_lowercase().contains(tool))
                .unwrap_or(false)
        });
    }

    if command.only_issues {
        report.segments.retain(|row| {
            row.note.is_some()
                || row
                    .status
                    .as_deref()
                    .map(|value| value != "focused_progress")
                    .unwrap_or(true)
                || row
                    .evidence
                    .as_deref()
                    .map(|value| value != "usable")
                    .unwrap_or(true)
        });
    }

    match command.view {
        ProtocolOverviewView::Overview => {}
        ProtocolOverviewView::Segments => {
            report.call_issues.clear();
        }
        ProtocolOverviewView::Calls => {
            report.segments.clear();
        }
    }
}

fn print_protocol_run_summaries(
    summaries: &[ProtocolRunSummaryRow],
    command: &InspectProtocolOverviewCommand,
) {
    let mut rows = summaries.to_vec();
    if command.only_issues {
        rows.retain(|row| {
            row.protocol_status != "full" && row.protocol_status != "ineligible"
                || row.call_issues > 0
                || row.missing_call_reviews > 0
                || row.missing_segment_reviews > 0
                || row.mismatched_segment_reviews > 0
        });
    }
    rows.truncate(command.limit.max(1));

    println!("Segment evidence legend: u=usable, m=mismatched, x=missing");
    println!(
        "{:<28} {:<10} {:<12} {:<18} {:<8} Summary",
        "Run", "Calls", "Status", "Segment evidence", "Issues"
    );
    println!("{}", "─".repeat(command.width.min(120)));
    for row in rows {
        let missing_segments = row.missing_segment_reviews;
        let segment_evidence = if row.segments_total == 0 {
            "-".to_string()
        } else {
            format!(
                "u{} m{} x{}",
                (row.usable_segment_ratio * row.segments_total as f32).round() as usize,
                row.mismatched_segment_reviews,
                missing_segments
            )
        };
        let summary = if row.protocol_status == "full" || row.protocol_status == "partial" {
            format!(
                "{} {}",
                progress_bar(row.call_review_ratio, 6),
                summary_segment_bar(
                    (row.usable_segment_ratio * row.segments_total as f32).round() as usize,
                    row.mismatched_segment_reviews,
                    missing_segments,
                    6,
                )
            )
        } else {
            truncate_for_table(row.note.as_deref().unwrap_or("-"), command.width.min(40))
        };
        println!(
            "{:<28} {:<10} {:<12} {:<18} {:<8} {}",
            truncate_for_table(&row.run_id, 26),
            format!(
                "{}/{}",
                (row.call_review_ratio * row.tool_calls_total as f32).round() as usize,
                row.tool_calls_total
            ),
            row.protocol_status,
            segment_evidence,
            row.call_issues,
            summary,
        );
    }
}

#[derive(Debug, Clone)]
struct AnchorCallDetail {
    tool_name: String,
    summary: String,
}

fn load_anchor_call_details(
    aggregate: &ProtocolAggregate,
) -> Result<BTreeMap<usize, AnchorCallDetail>, PrepareError> {
    let artifact = load_protocol_artifact(&aggregate.segmentation.artifact.path)?;
    let mut details = BTreeMap::new();
    if let Some(calls) = artifact
        .stored
        .input
        .get("calls")
        .and_then(|value| value.as_array())
    {
        for call in calls {
            if let Some(index) = call.get("index").and_then(|value| value.as_u64()) {
                details.insert(
                    index as usize,
                    AnchorCallDetail {
                        tool_name: call
                            .get("tool_name")
                            .and_then(|value| value.as_str())
                            .unwrap_or("-")
                            .to_string(),
                        summary: call
                            .get("summary")
                            .and_then(|value| value.as_str())
                            .unwrap_or("-")
                            .to_string(),
                    },
                );
            }
        }
    }
    Ok(details)
}

fn primary_call_issue(row: &ProtocolCallReviewRow) -> Option<String> {
    if row.redundancy.verdict == "search_thrash" {
        Some("search_thrash".to_string())
    } else if row.recoverability.verdict == "partial_next_step" {
        Some("partial_next_step".to_string())
    } else if row.overall != "focused_progress" {
        Some(row.overall.clone())
    } else {
        None
    }
}

fn call_review_severity(row: &ProtocolCallReviewRow) -> f32 {
    if row.redundancy.verdict == "search_thrash" {
        0.95
    } else if row.recoverability.verdict == "no_clear_recovery" {
        0.85
    } else if row.recoverability.verdict == "partial_next_step" {
        0.7
    } else if row.overall == "mixed" {
        0.55
    } else {
        0.25
    }
}

fn confidence_fraction(value: Option<&str>) -> Option<f32> {
    match value? {
        "high" | "High" => Some(0.9),
        "medium" | "Medium" => Some(0.6),
        "low" | "Low" => Some(0.3),
        _ => None,
    }
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f32 / denominator as f32
    }
}

fn progress_bar(value: f32, width: usize) -> String {
    let width = width.max(3);
    let filled = ((value.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(width - filled))
}

fn summary_segment_bar(usable: usize, mismatched: usize, missing: usize, width: usize) -> String {
    let total = usable + mismatched + missing;
    if total == 0 {
        return "[------]".to_string();
    }
    let width = width.max(3);
    let usable_width = ((usable as f32 / total as f32) * width as f32).round() as usize;
    let mismatch_width = ((mismatched as f32 / total as f32) * width as f32).round() as usize;
    let mut missing_width = width.saturating_sub(usable_width + mismatch_width);
    let mut usable_width = usable_width.min(width);
    let mut mismatch_width = mismatch_width.min(width.saturating_sub(usable_width));
    missing_width = missing_width.min(width.saturating_sub(usable_width + mismatch_width));
    while usable_width + mismatch_width + missing_width < width {
        if missing > 0 {
            missing_width += 1;
        } else if mismatched > 0 {
            mismatch_width += 1;
        } else {
            usable_width += 1;
        }
    }
    format!(
        "[{}{}{}]",
        "█".repeat(usable_width),
        "▓".repeat(mismatch_width),
        "░".repeat(missing_width)
    )
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

fn render_messages_table(messages: &[crate::record::ConversationMessage]) -> String {
    if messages.is_empty() {
        return "No messages in this turn.".to_string();
    }

    let mut out = String::new();
    for (index, message) in messages.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }

        out.push_str(&format!("Message {}\n", index + 1));
        out.push_str(&format!(
            "  role .......... {}\n",
            message_role_label(message_role(message))
        ));
        if let Some(tool_call_id) = &message.tool_call_id {
            out.push_str(&format!("  tool call id ... {}\n", tool_call_id));
        }
        out.push_str(&format!(
            "  content ....... {}\n",
            summarize_message_content(&message.content)
        ));
    }

    out.trim_end().to_string()
}

#[derive(Debug, Clone, Serialize)]
struct ToolLoopDetail {
    label: String,
    value: String,
}

#[derive(Debug, Clone, Serialize)]
struct ToolLoopEntry {
    index: usize,
    tool: String,
    input: String,
    status: String,
    summary: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    details: Vec<ToolLoopDetail>,
}

fn render_tool_loop_table(turn: u32, tool_calls: &[crate::record::ToolExecutionRecord]) -> String {
    if tool_calls.is_empty() {
        return format!("Turn {}\n  No tool calls in this turn.", turn);
    }

    let mut out = String::new();
    out.push_str(&format!("Turn {}\n", turn));
    for (index, entry) in tool_loop_entries(tool_calls).iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&format!("[{}] {}\n", entry.index, entry.tool));
        out.push_str(&dotted_loop_line("input", &entry.input));
        out.push_str(&dotted_loop_line("status", &entry.status));
        for detail in &entry.details {
            out.push_str(&dotted_loop_line(&detail.label, &detail.value));
        }
        out.push_str(&dotted_loop_line("summary", &entry.summary));
    }

    out.trim_end().to_string()
}

fn render_tool_loop_json(
    turn: u32,
    tool_calls: &[crate::record::ToolExecutionRecord],
) -> Result<String, PrepareError> {
    let payload = serde_json::json!({
        "turn": turn,
        "tool_calls": tool_loop_entries(tool_calls),
    });
    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)
}

fn tool_loop_entries(tool_calls: &[crate::record::ToolExecutionRecord]) -> Vec<ToolLoopEntry> {
    tool_calls
        .iter()
        .enumerate()
        .map(|(index, call)| tool_loop_entry(index, call))
        .collect()
}

fn tool_loop_entry(index: usize, call: &crate::record::ToolExecutionRecord) -> ToolLoopEntry {
    let input = summarize_tool_inputs(&call.request.arguments);
    match &call.result {
        crate::record::ToolResult::Completed(completed) => ToolLoopEntry {
            index,
            tool: call.request.tool.clone(),
            input: normalize_loop_input(&input),
            status: "completed".to_string(),
            summary: summarize_loop_success(completed),
            details: summarize_loop_success_details(completed),
        },
        crate::record::ToolResult::Failed(failed) => ToolLoopEntry {
            index,
            tool: call.request.tool.clone(),
            input: normalize_loop_input(&input),
            status: "failed".to_string(),
            summary: summarize_loop_failure(failed),
            details: summarize_loop_failure_details(failed),
        },
    }
}

fn normalize_loop_input(input: &str) -> String {
    if input.trim().is_empty() {
        "(none)".to_string()
    } else {
        truncate_middle(input, 96)
    }
}

fn summarize_loop_success(completed: &crate::runner::ToolCompletedRecord) -> String {
    if let Some(ui_payload) = &completed.ui_payload {
        if !ui_payload.summary.trim().is_empty() {
            return truncate_middle(&ui_payload.summary, 96);
        }
    }

    let first_line = completed.content.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        "ok".to_string()
    } else {
        truncate_middle(first_line, 96)
    }
}

fn summarize_loop_failure(failed: &crate::runner::ToolFailedRecord) -> String {
    if let Some(ui_payload) = &failed.ui_payload {
        if !ui_payload.summary.trim().is_empty() {
            return truncate_middle(&ui_payload.summary, 96);
        }
    }

    let first_line = failed.error.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        "failed".to_string()
    } else {
        truncate_middle(first_line, 96)
    }
}

fn summarize_loop_success_details(
    completed: &crate::runner::ToolCompletedRecord,
) -> Vec<ToolLoopDetail> {
    let Some(ui_payload) = &completed.ui_payload else {
        return Vec::new();
    };

    let mut details: Vec<ToolLoopDetail> = ui_payload
        .fields
        .iter()
        .filter(|field| field.name.as_ref() != "status")
        .take(2)
        .map(|field| ToolLoopDetail {
            label: field.name.to_string(),
            value: truncate_middle(&prettify_field_value(&field.value), 96),
        })
        .collect();

    if details.is_empty() {
        if let Some(details_text) = &ui_payload.details {
            details.push(ToolLoopDetail {
                label: "details".to_string(),
                value: truncate_middle(details_text, 96),
            });
        }
    }

    details
}

fn summarize_loop_failure_details(failed: &crate::runner::ToolFailedRecord) -> Vec<ToolLoopDetail> {
    let Some(ui_payload) = &failed.ui_payload else {
        return Vec::new();
    };

    let mut details = Vec::new();
    if let Some(error_code) = ui_payload.error_code {
        details.push(ToolLoopDetail {
            label: "code".to_string(),
            value: tool_error_code_label(error_code).to_string(),
        });
    }

    for field in ui_payload
        .fields
        .iter()
        .filter(|field| field.name.as_ref() != "code")
        .take(2)
    {
        details.push(ToolLoopDetail {
            label: field.name.to_string(),
            value: truncate_middle(&prettify_field_value(&field.value), 96),
        });
    }

    details
}

fn dotted_loop_line(label: &str, value: &str) -> String {
    const LABEL_WIDTH: usize = 14;
    let dots = LABEL_WIDTH.saturating_sub(label.chars().count()).max(2);
    format!("  {} {} {}\n", label, ".".repeat(dots), value)
}

fn filter_messages(
    messages: Vec<crate::record::ConversationMessage>,
    roles: &[InspectMessageRole],
    exclude_roles: &[InspectMessageRole],
) -> Vec<crate::record::ConversationMessage> {
    messages
        .into_iter()
        .filter(|message| {
            let role = message_role(message);
            (roles.is_empty() || roles.contains(&role)) && !exclude_roles.contains(&role)
        })
        .collect()
}

fn message_role(message: &crate::record::ConversationMessage) -> InspectMessageRole {
    use ploke_tui::chat_history::MessageKind;

    match message.kind {
        MessageKind::System => InspectMessageRole::System,
        MessageKind::SysInfo => InspectMessageRole::System,
        MessageKind::User => InspectMessageRole::User,
        MessageKind::Assistant => InspectMessageRole::Assistant,
        MessageKind::Tool => InspectMessageRole::Tool,
    }
}

fn message_role_label(role: InspectMessageRole) -> &'static str {
    match role {
        InspectMessageRole::System => "system",
        InspectMessageRole::User => "user",
        InspectMessageRole::Assistant => "assistant",
        InspectMessageRole::Tool => "tool",
    }
}

fn indexed_tool_calls(
    record: &crate::record::RunRecord,
) -> Vec<(usize, u32, crate::record::ToolExecutionRecord)> {
    record
        .conversations()
        .flat_map(|turn| {
            turn.tool_calls()
                .into_iter()
                .map(move |call| (turn.turn_number, call))
        })
        .enumerate()
        .map(|(index, (turn, call))| (index, turn, call))
        .collect()
}

fn build_tool_call_sequence_subject(
    record: &crate::record::RunRecord,
    record_path: &std::path::Path,
) -> Result<trace::ToolCallSequence, PrepareError> {
    let subject_id = record_path
        .parent()
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| record.manifest_id.clone());
    let indexed = indexed_tool_calls(record);

    let turns = record
        .conversations()
        .map(|turn| trace::TurnContext {
            turn: turn.turn_number,
            tool_count: turn.tool_calls().len(),
            failed_tool_count: failed_tool_count(turn),
            patch_proposed: turn
                .tool_calls()
                .iter()
                .any(|call| call.request.tool == "non_semantic_patch"),
            patch_applied: summarize_patch_state(&turn.tool_calls()) == "yes",
        })
        .collect();

    let calls = indexed
        .iter()
        .map(|(index, turn, call)| summarize_neighborhood_call(*index, *turn, call))
        .collect();

    Ok(trace::ToolCallSequence {
        subject_id,
        total_turns: record.conversations().count(),
        total_calls_in_run: indexed.len(),
        turns,
        calls,
    })
}

struct RecordToolCallNeighborhoodAdapter<'a> {
    record: &'a crate::record::RunRecord,
    subject_id: String,
}

#[derive(Debug, thiserror::Error)]
enum RecordToolCallNeighborhoodError {
    #[error("selected tool call index {0} not found")]
    MissingIndex(usize),
    #[error("turn {0} not found for selected tool call")]
    MissingTurn(u32),
}

impl trace::NeighborhoodSource for RecordToolCallNeighborhoodAdapter<'_> {
    type Error = RecordToolCallNeighborhoodError;

    fn neighborhood(
        &self,
        request: &trace::NeighborhoodRequest,
    ) -> Result<trace::ToolCallNeighborhood, Self::Error> {
        let indexed = indexed_tool_calls(self.record);
        let focal_turn = indexed
            .iter()
            .find(|(index, _, _)| *index == request.focal_index)
            .map(|(_, turn, _)| *turn)
            .ok_or(RecordToolCallNeighborhoodError::MissingIndex(
                request.focal_index,
            ))?;

        let turn_record = self
            .record
            .turn_record(focal_turn)
            .ok_or(RecordToolCallNeighborhoodError::MissingTurn(focal_turn))?;
        let turn_calls: Vec<_> = indexed
            .iter()
            .filter(|(_, turn, _)| *turn == focal_turn)
            .cloned()
            .collect();
        let turn_position = turn_calls
            .iter()
            .position(|(index, _, _)| *index == request.focal_index)
            .ok_or(RecordToolCallNeighborhoodError::MissingIndex(
                request.focal_index,
            ))?;
        let start = turn_position.saturating_sub(request.radius_before);
        let end = (turn_position + request.radius_after + 1).min(turn_calls.len());

        let before = turn_calls[start..turn_position]
            .iter()
            .map(|(index, turn, call)| summarize_neighborhood_call(*index, *turn, call))
            .collect();
        let focal = summarize_neighborhood_call(
            turn_calls[turn_position].0,
            turn_calls[turn_position].1,
            &turn_calls[turn_position].2,
        );
        let after = turn_calls[turn_position + 1..end]
            .iter()
            .map(|(index, turn, call)| summarize_neighborhood_call(*index, *turn, call))
            .collect();

        Ok(trace::ToolCallNeighborhood {
            subject_id: self.subject_id.clone(),
            total_calls_in_run: indexed.len(),
            total_calls_in_turn: turn_calls.len(),
            turn: trace::TurnContext {
                turn: focal_turn,
                tool_count: turn_record.tool_calls().len(),
                failed_tool_count: failed_tool_count(turn_record),
                patch_proposed: turn_record
                    .tool_calls()
                    .iter()
                    .any(|call| call.request.tool == "non_semantic_patch"),
                patch_applied: summarize_patch_state(&turn_record.tool_calls()) == "yes",
            },
            before,
            focal,
            after,
        })
    }
}

fn build_tool_call_review_subject(
    record: &crate::record::RunRecord,
    record_path: &std::path::Path,
    index: usize,
) -> Result<trace::ToolCallNeighborhood, PrepareError> {
    let subject_id = record_path
        .parent()
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| record.manifest_id.clone());
    let adapter = RecordToolCallNeighborhoodAdapter { record, subject_id };
    adapter
        .neighborhood(&trace::NeighborhoodRequest::centered(index))
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "protocol_tool_call_review",
            detail: err.to_string(),
        })
}

fn build_segment_review_subject(
    segmented: &segment::SegmentedToolCallSequence,
    segment_index: usize,
) -> Result<review::SegmentReviewSubject, PrepareError> {
    let segment = segmented
        .segments
        .iter()
        .find(|segment| segment.segment_index == segment_index)
        .cloned()
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "protocol_tool_call_segment_review",
            detail: format!("segment index {segment_index} not found"),
        })?;

    Ok(review::SegmentReviewSubject {
        subject_id: segmented.sequence.subject_id.clone(),
        sequence: segmented.sequence.clone(),
        segment,
        coverage: segmented.coverage.clone(),
    })
}

fn summarize_neighborhood_call(
    index: usize,
    turn: u32,
    call: &crate::record::ToolExecutionRecord,
) -> trace::NeighborhoodCall {
    trace::NeighborhoodCall {
        index,
        turn,
        tool_name: call.request.tool.clone(),
        tool_kind: classify_tool_kind(&call.request.tool),
        failed: matches!(call.result, crate::record::ToolResult::Failed(_)),
        latency_ms: call.latency_ms,
        summary: tool_call_summary_line(call),
        args_preview: truncate_middle(&call.request.arguments, 96),
        result_preview: tool_result_preview(call),
        search_term: extract_argument_string(&call.request.arguments, &["search_term", "query"]),
        path_hint: extract_argument_string(
            &call.request.arguments,
            &["file", "dir", "path", "target_dir"],
        ),
    }
}

fn classify_tool_kind(tool_name: &str) -> trace::ToolKind {
    match tool_name {
        "request_code_context" | "search_code" | "search_symbols" | "query_codebase" => {
            trace::ToolKind::Search
        }
        "read_file" => trace::ToolKind::Read,
        "list_dir" => trace::ToolKind::Browse,
        "apply_code_edit" => trace::ToolKind::Edit,
        "run_command" | "shell" => trace::ToolKind::Execute,
        _ => trace::ToolKind::Other,
    }
}

fn tool_result_preview(call: &crate::record::ToolExecutionRecord) -> String {
    match &call.result {
        crate::record::ToolResult::Completed(completed) => truncate_middle(&completed.content, 96),
        crate::record::ToolResult::Failed(failed) => truncate_middle(&failed.error, 96),
    }
}

fn extract_argument_string(arguments: &str, keys: &[&str]) -> Option<String> {
    let json = serde_json::from_str::<serde_json::Value>(arguments).ok()?;
    for key in keys {
        if let Some(value) = json.get(*key).and_then(|value| value.as_str()) {
            return Some(value.to_string());
        }
    }
    None
}

fn tool_call_summary_line(call: &crate::record::ToolExecutionRecord) -> String {
    match &call.result {
        crate::record::ToolResult::Completed(completed) => format!(
            "tool={} status=completed latency_ms={} args={} result={}",
            call.request.tool,
            call.latency_ms,
            truncate_middle(&call.request.arguments, 96),
            truncate_middle(&completed.content, 96),
        ),
        crate::record::ToolResult::Failed(failed) => format!(
            "tool={} status=failed latency_ms={} args={} error={}",
            call.request.tool,
            call.latency_ms,
            truncate_middle(&call.request.arguments, 96),
            truncate_middle(&failed.error, 96),
        ),
    }
}

fn resolve_protocol_model_id(model_id: Option<String>) -> Result<ModelId, PrepareError> {
    match model_id {
        Some(model_id) => {
            model_id
                .parse()
                .map_err(|err: ploke_llm::IdError| PrepareError::DatabaseSetup {
                    phase: "protocol_model_id",
                    detail: err.to_string(),
                })
        }
        None => load_active_model().map(|selection| selection.model_id),
    }
}

fn resolve_protocol_provider_slug(
    model_id: &ModelId,
    provider: Option<String>,
) -> Result<Option<String>, PrepareError> {
    if let Some(provider) = provider {
        let parsed = ProviderKey::new(&provider).map_err(|err| PrepareError::DatabaseSetup {
            phase: "protocol_provider_slug",
            detail: err.to_string(),
        })?;
        return Ok(Some(parsed.slug.as_str().to_string()));
    }

    Ok(load_provider_for_model(model_id)?.map(|provider| provider.slug.as_str().to_string()))
}

fn print_protocol_artifact_detail(index: usize, entry: &StoredProtocolArtifactFile, full: bool) {
    println!("Protocol Artifact {}", index);
    println!("{}", "-".repeat(40));
    println!("Path: {}", entry.path.display());
    println!("Procedure: {}", entry.stored.procedure_name);
    println!("Subject: {}", entry.stored.subject_id);
    println!("Run: {}", entry.stored.run_id);
    println!("Created (ms): {}", entry.stored.created_at_ms);
    println!(
        "Model: {}",
        entry.stored.model_id.as_deref().unwrap_or("(unknown)")
    );
    println!(
        "Provider: {}",
        entry
            .stored
            .provider_slug
            .as_deref()
            .unwrap_or("auto/openrouter")
    );
    println!("Summary: {}", protocol_artifact_summary(entry));
    println!();
    if full {
        println!("Input");
        println!("{}", "-".repeat(40));
        println!(
            "{}",
            serde_json::to_string_pretty(&entry.stored.input)
                .unwrap_or_else(|_| { protocol_artifact_preview(&entry.stored.input) })
        );
        println!();
        println!("Output");
        println!("{}", "-".repeat(40));
        println!(
            "{}",
            serde_json::to_string_pretty(&entry.stored.output)
                .unwrap_or_else(|_| { protocol_artifact_preview(&entry.stored.output) })
        );
        println!();
        println!("Artifact");
        println!("{}", "-".repeat(40));
        println!(
            "{}",
            serde_json::to_string_pretty(&entry.stored.artifact)
                .unwrap_or_else(|_| { protocol_artifact_preview(&entry.stored.artifact) })
        );
    } else {
        println!("Input: {}", protocol_artifact_preview(&entry.stored.input));
        println!(
            "Output: {}",
            protocol_artifact_preview(&entry.stored.output)
        );
        println!(
            "Artifact: {}",
            protocol_artifact_preview(&entry.stored.artifact)
        );
        println!();
        println!("Tip: rerun with `--full` to print the full nested payloads.");
    }
}

fn tool_call_review_error_to_prepare(err: review::ToolCallReviewError) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "protocol_tool_call_review",
        detail: err.to_string(),
    }
}

fn tool_call_intent_segmentation_error_to_prepare(
    err: segment::IntentSegmentationError,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "protocol_tool_call_intent_segmentation",
        detail: err.to_string(),
    }
}

fn tool_call_segment_review_error_to_prepare(
    err: review::ToolCallSegmentReviewError,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "protocol_tool_call_segment_review",
        detail: err.to_string(),
    }
}

fn failed_tool_count(turn: &crate::record::TurnRecord) -> usize {
    turn.tool_calls()
        .iter()
        .filter(|call| matches!(call.result, crate::record::ToolResult::Failed(_)))
        .count()
}

fn summarize_turn_outcome(turn: &crate::record::TurnRecord) -> String {
    match &turn.outcome {
        crate::record::TurnOutcome::ToolCalls { .. } => "completed".to_string(),
        crate::record::TurnOutcome::Content => "content".to_string(),
        crate::record::TurnOutcome::Error { message } => {
            format!("error: {}", truncate_middle(message, 16))
        }
        crate::record::TurnOutcome::Timeout { elapsed_secs } => {
            format!("timeout({}s)", elapsed_secs)
        }
    }
}

fn summarize_message_content(content: &str) -> String {
    let first_line = content.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        "(empty)".to_string()
    } else {
        truncate_middle(first_line, 72)
    }
}

fn tool_call_next_step_index(tool_call_count: usize) -> Option<usize> {
    if tool_call_count > 0 { Some(0) } else { None }
}

fn join_indices(indices: &[usize]) -> String {
    indices
        .iter()
        .map(|idx| idx.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn join_turns(turns: &[u32]) -> String {
    turns
        .iter()
        .map(|turn| turn.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_segment_descriptor(
    status: segment::SegmentStatus,
    label: Option<segment::IntentLabel>,
) -> String {
    match (status, label) {
        (segment::SegmentStatus::Labeled, Some(label)) => format!("labeled:{label:?}"),
        (segment::SegmentStatus::Labeled, None) => "labeled:<missing>".to_string(),
        (segment::SegmentStatus::Ambiguous, _) => "ambiguous".to_string(),
    }
}

fn print_turn_summary(turn: &crate::record::TurnRecord) {
    let messages = turn.messages();
    let tool_calls = turn.tool_calls();
    let failed_tools = tool_calls
        .iter()
        .filter(|call| matches!(call.result, crate::record::ToolResult::Failed(_)))
        .count();
    let patch_proposed = tool_calls
        .iter()
        .any(|call| call.request.tool == "non_semantic_patch");
    let patch_applied = summarize_patch_state(&tool_calls);

    println!("Turn {}", turn.turn_number);
    println!("  tools .............. {}", tool_calls.len());
    println!("  failed tools ....... {}", failed_tools);
    println!("  messages ........... {}", messages.len());
    println!(
        "  patch proposed ..... {}",
        if patch_proposed { "yes" } else { "no" }
    );
    println!("  patch applied ...... {}", patch_applied);
}

fn assistant_message_id_for_turn(turn: &crate::record::TurnRecord) -> Option<&str> {
    turn.agent_turn_artifact
        .as_ref()
        .and_then(|artifact| artifact.terminal_record.as_ref())
        .map(|record| record.assistant_message_id.as_str())
}

fn resolve_full_response_trace_path(
    record_path: &std::path::Path,
    record: &crate::record::RunRecord,
) -> Result<PathBuf, PrepareError> {
    let run_dir = record_path
        .parent()
        .ok_or_else(|| PrepareError::MissingRunManifest(record_path.to_path_buf()))?;
    let path = run_dir.join("llm-full-responses.jsonl");
    if path.exists() {
        Ok(path)
    } else if record.metadata.run_arm.execution == "agent-single-turn" {
        Err(PrepareError::MissingRunManifest(path))
    } else {
        Err(PrepareError::DatabaseSetup {
            phase: "inspect_turn",
            detail: "raw full responses are only captured for agent-mode runs".to_string(),
        })
    }
}

fn load_full_response_records_for_turn(
    path: &std::path::Path,
    assistant_message_id: &str,
) -> Result<Vec<RawFullResponseRecord>, PrepareError> {
    let text = std::fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let mut responses = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record: RawFullResponseRecord =
            serde_json::from_str(trimmed).map_err(|source| PrepareError::ParseManifest {
                path: path.to_path_buf(),
                source,
            })?;
        if record.assistant_message_id == assistant_message_id {
            responses.push(record);
        }
    }
    responses.sort_by_key(|record| record.response_index);
    Ok(responses)
}

fn load_all_full_response_records(
    path: &std::path::Path,
) -> Result<Vec<RawFullResponseRecord>, PrepareError> {
    let text = std::fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let mut responses = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record: RawFullResponseRecord =
            serde_json::from_str(trimmed).map_err(|source| PrepareError::ParseManifest {
                path: path.to_path_buf(),
                source,
            })?;
        responses.push(record);
    }
    responses.sort_by_key(|record| record.response_index);
    Ok(responses)
}

fn aggregate_full_response_usage(
    responses: &[RawFullResponseRecord],
) -> ploke_llm::response::TokenUsage {
    // Stopgap: this sums the persisted raw-response sidecar as captured today.
    // It is useful for eval introspection, but may undercount a turn when the
    // final non-tool-call/stop response is not yet captured into the sidecar.
    let mut total = ploke_llm::response::TokenUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
    };
    for record in responses {
        if let Some(usage) = record.response.usage.as_ref() {
            total.prompt_tokens += usage.prompt_tokens;
            total.completion_tokens += usage.completion_tokens;
            total.total_tokens += usage.total_tokens;
        }
    }
    total
}

fn load_full_response_usage_totals(
    record_path: &std::path::Path,
    record: &crate::record::RunRecord,
) -> Result<Option<ploke_llm::response::TokenUsage>, PrepareError> {
    let trace_path = match resolve_full_response_trace_path(record_path, record) {
        Ok(path) => path,
        Err(PrepareError::MissingRunManifest(_)) => return Ok(None),
        Err(err) => return Err(err),
    };
    let responses = load_all_full_response_records(&trace_path)?;
    if responses.is_empty() {
        Ok(None)
    } else {
        Ok(Some(aggregate_full_response_usage(&responses)))
    }
}

fn format_usage_triplet(prompt: u32, completion: u32, total: u32) -> String {
    format!(
        "prompt:{} completion:{} total:{}",
        prompt, completion, total
    )
}

fn render_full_response_table(turn: u32, responses: &[RawFullResponseRecord]) -> String {
    let totals = aggregate_full_response_usage(responses);
    let mut out = String::new();
    out.push_str(&format!("Turn {} Raw Full Responses\n", turn));
    out.push_str(&format!("{}\n", "-".repeat(80)));
    out.push_str(&format!(
        "{:<5} {:<40} {:<14} {}\n",
        "Idx", "Response ID", "Finish", "Usage"
    ));
    out.push_str(&format!("{}\n", "-".repeat(80)));
    for record in responses {
        let response_id = truncate_for_table(&record.response.id, 38);
        let finish = record
            .response
            .choices
            .first()
            .and_then(|choice| choice.finish_reason.as_ref())
            .map(|reason| format!("{reason:?}").to_lowercase())
            .unwrap_or_else(|| "-".to_string());
        let usage = record
            .response
            .usage
            .as_ref()
            .map(|usage| {
                format!(
                    "p:{} c:{} t:{}",
                    usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
                )
            })
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "{:<5} {:<40} {:<14} {}\n",
            record.response_index, response_id, finish, usage
        ));
    }
    out.push_str(&format!("{}\n", "-".repeat(80)));
    out.push_str(&format!(
        "Totals: {}\n",
        format_usage_triplet(
            totals.prompt_tokens,
            totals.completion_tokens,
            totals.total_tokens
        )
    ));
    out.push_str(
        "Note: sidecar totals may undercount if the final stop response was not captured.\n",
    );
    out
}

fn summarize_patch_state(tool_calls: &[crate::record::ToolExecutionRecord]) -> &'static str {
    let patch_calls: Vec<_> = tool_calls
        .iter()
        .filter(|call| call.request.tool == "non_semantic_patch")
        .collect();

    if patch_calls.is_empty() {
        return "no";
    }

    if patch_calls.iter().any(|call| {
        matches!(
            &call.result,
            crate::record::ToolResult::Failed(_) | crate::record::ToolResult::Completed(_)
        )
    }) {
        let completed_calls: Vec<_> = patch_calls
            .iter()
            .filter_map(|call| match &call.result {
                crate::record::ToolResult::Completed(completed) => Some(completed),
                crate::record::ToolResult::Failed(_) => None,
            })
            .collect();

        if completed_calls.is_empty() {
            "no"
        } else if completed_calls.iter().any(|completed| {
            completed
                .ui_payload
                .as_ref()
                .and_then(|ui| {
                    ui.fields
                        .iter()
                        .find(|field| field.name.as_ref() == "applied")
                        .map(|field| field.value.as_ref())
                })
                .map(|value| value != "0")
                .unwrap_or(false)
        }) {
            "yes"
        } else {
            "partial"
        }
    } else {
        "no"
    }
}

fn print_tool_call_detail(
    turn: u32,
    index: usize,
    call: &crate::record::ToolExecutionRecord,
    full: bool,
) {
    println!("Tool Call {}", index);
    println!("{}", "-".repeat(40));
    println!("Turn: {}", turn);
    println!("Tool: {}", call.request.tool);
    println!("Status: {}", tool_status_label(&call.result));
    println!("Latency: {} ms", call.latency_ms);
    println!();
    println!("Inputs");
    println!("{}", "-".repeat(40));
    let rendered_inputs = render_tool_inputs(&call.request.arguments);
    if rendered_inputs.is_empty() {
        println!("(none)");
    } else {
        for line in rendered_inputs {
            println!("{}", line);
        }
    }
    println!();
    print_tool_result_detail(index, &call.result, full);
    if full {
        println!();
        println!("Raw Arguments");
        println!("{}", "-".repeat(40));
        println!("{}", call.request.arguments);
    }
}

fn print_tool_result_detail(index: usize, result: &crate::record::ToolResult, full: bool) {
    println!("Result");
    println!("{}", "-".repeat(40));
    match result {
        crate::record::ToolResult::Completed(completed) => {
            if let Some(ui_payload) = &completed.ui_payload {
                println!("Summary: {}", ui_payload.summary);
                for field in &ui_payload.fields {
                    println!("{}: {}", field.name, prettify_field_value(&field.value));
                }
                if let Some(details) = &ui_payload.details {
                    println!("Details:");
                    println!(
                        "{}",
                        format_payload_block(details, if full { 1200 } else { 220 })
                    );
                }
            }
            if !completed.content.is_empty() {
                println!("Output:");
                println!(
                    "{}",
                    format_payload_block(&completed.content, if full { 2400 } else { 220 })
                );
            }
        }
        crate::record::ToolResult::Failed(failed) => {
            println!("Error:");
            println!(
                "{}",
                format_payload_block(&failed.error, if full { 2400 } else { 220 })
            );
            if let Some(ui_payload) = &failed.ui_payload {
                println!("UI Summary: {}", ui_payload.summary);
                for field in &ui_payload.fields {
                    println!("{}: {}", field.name, prettify_field_value(&field.value));
                }
            }
        }
    }
    if !full {
        println!();
        println!("Tip: rerun with `--full {}` for the full payload.", index);
    }
}

fn render_tool_inputs(arguments: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(arguments) else {
        return vec![format!(
            "arguments: {}",
            format_payload_block(arguments, 220)
        )];
    };

    match value {
        serde_json::Value::Object(map) => map
            .into_iter()
            .map(|(key, value)| format!("{}: {}", key, summarize_json_value(&key, &value, true)))
            .collect(),
        other => vec![format!(
            "arguments: {}",
            summarize_json_value("arguments", &other, true)
        )],
    }
}

fn summarize_tool_inputs(arguments: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(arguments) else {
        return truncate_middle(arguments, 46);
    };

    let Some(object) = value.as_object() else {
        return truncate_middle(&value.to_string(), 46);
    };

    let preferred = [
        "file",
        "file_path",
        "dir",
        "path",
        "search_term",
        "canon",
        "symbol",
        "name",
        "query",
        "command",
        "patches",
        "confidence",
    ];

    let mut parts = Vec::new();
    for key in preferred {
        if let Some(value) = object.get(key) {
            parts.push(format!(
                "{}={}",
                key,
                summarize_json_value(key, value, false)
            ));
        }
        if parts.len() >= 3 {
            break;
        }
    }

    if parts.is_empty() {
        for (key, value) in object.iter().take(3) {
            parts.push(format!(
                "{}={}",
                key,
                summarize_json_value(key, value, false)
            ));
        }
    }

    parts.join("; ")
}

fn summarize_tool_result(result: &crate::record::ToolResult) -> String {
    match result {
        crate::record::ToolResult::Completed(completed) => {
            if let Some(ui_payload) = &completed.ui_payload {
                format!("ok: {}", truncate_middle(&ui_payload.summary, 24))
            } else {
                "ok".to_string()
            }
        }
        crate::record::ToolResult::Failed(failed) => {
            format!("failed: {}", truncate_middle(&failed.error, 24))
        }
    }
}

fn summarize_json_value(key: &str, value: &serde_json::Value, multiline: bool) -> String {
    match value {
        serde_json::Value::String(text) => {
            if looks_like_path(key, text) {
                abbreviate_path_tail(text, if multiline { 72 } else { 24 })
            } else {
                let limit = if multiline { 120 } else { 24 };
                prettify_field_value(&truncate_middle(text, limit))
            }
        }
        serde_json::Value::Array(items) => {
            if items.is_empty() {
                "[]".to_string()
            } else {
                format!(
                    "[{} item{}]",
                    items.len(),
                    if items.len() == 1 { "" } else { "s" }
                )
            }
        }
        serde_json::Value::Object(map) => format!(
            "{{{} key{}}}",
            map.len(),
            if map.len() == 1 { "" } else { "s" }
        ),
        other => other.to_string(),
    }
}

fn looks_like_path(key: &str, value: &str) -> bool {
    matches!(key, "file" | "file_path" | "dir" | "path" | "root_path")
        || value.starts_with('/')
        || value.contains(std::path::MAIN_SEPARATOR)
}

fn abbreviate_path_tail(path: &str, max_len: usize) -> String {
    if path.chars().count() <= max_len {
        return path.to_string();
    }

    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return truncate_middle(path, max_len);
    }

    let mut kept = Vec::new();
    let mut len = 3usize;
    for part in parts.iter().rev() {
        let next_len = len + part.len() + if kept.is_empty() { 0 } else { 1 };
        if next_len > max_len {
            break;
        }
        kept.push(*part);
        len = next_len;
    }
    kept.reverse();

    if kept.is_empty() {
        truncate_middle(path, max_len)
    } else {
        format!(".../{}", kept.join("/"))
    }
}

fn truncate_for_table(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        format!(
            "{}...",
            text.chars()
                .take(max_len.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn truncate_middle(text: &str, max_len: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_len {
        return text.to_string();
    }
    if max_len <= 3 {
        return ".".repeat(max_len);
    }
    let front = (max_len - 3) / 2;
    let back = max_len - 3 - front;
    format!(
        "{}...{}",
        chars[..front].iter().collect::<String>(),
        chars[chars.len() - back..].iter().collect::<String>()
    )
}

fn format_payload_block(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();
    let rendered = serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| trimmed.to_string());

    truncate_middle(&rendered, max_len)
}

fn prettify_field_value(text: &str) -> String {
    text.replace('\n', "\\n")
}

fn tool_status_label(result: &crate::record::ToolResult) -> &'static str {
    match result {
        crate::record::ToolResult::Completed(_) => "completed",
        crate::record::ToolResult::Failed(_) => "failed",
    }
}

fn tool_error_code_label(code: ploke_tui::tools::ToolErrorCode) -> &'static str {
    use ploke_tui::tools::ToolErrorCode;

    match code {
        ToolErrorCode::FieldTooLarge => "field_too_large",
        ToolErrorCode::WrongType => "wrong_type",
        ToolErrorCode::MissingField => "missing_field",
        ToolErrorCode::MalformedDiff => "malformed_diff",
        ToolErrorCode::InvalidFormat => "invalid_format",
        ToolErrorCode::Io => "io",
        ToolErrorCode::Timeout => "timeout",
        ToolErrorCode::Internal => "internal",
    }
}

fn resolve_record_path(
    record: Option<PathBuf>,
    instance: Option<String>,
) -> Result<PathBuf, PrepareError> {
    resolve_record_path_from_eval_home(record, instance, crate::layout::ploke_eval_home()?)
}

fn resolve_record_path_from_eval_home(
    record: Option<PathBuf>,
    instance: Option<String>,
    eval_home: PathBuf,
) -> Result<PathBuf, PrepareError> {
    let runs_root = eval_home.join("runs");
    match (record, instance) {
        (Some(path), None) => Ok(path),
        (None, Some(instance)) => Ok(runs_root.join(instance).join("record.json.gz")),
        (None, None) => {
            let last_run = crate::run_history::load_last_run_at(&eval_home)?;
            Ok(last_run.run_dir.join("record.json.gz"))
        }
        (Some(_), Some(_)) => Err(PrepareError::MissingRunManifest(
            runs_root.join("<instance>/record.json.gz"),
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
    use ploke_core::ArcStr;
    use ploke_tui::chat_history::{MessageKind, MessageStatus};
    use uuid::Uuid;

    fn sample_message(
        kind: MessageKind,
        content: &str,
        tool_call_id: Option<&str>,
    ) -> crate::record::ConversationMessage {
        crate::record::ConversationMessage {
            id: Uuid::nil(),
            branch_id: Uuid::nil(),
            status: MessageStatus::Completed,
            metadata: None,
            parent: None,
            children: Vec::new(),
            selected_child: None,
            content: content.to_string(),
            kind,
            tool_call_id: tool_call_id.map(ArcStr::from),
            tool_payload: None,
            context_status: Default::default(),
            last_included_turn: None,
            include_count: 0,
        }
    }

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
        assert_eq!(
            render_messages_json(messages).expect("render should succeed"),
            "[]"
        );
    }

    #[test]
    fn render_messages_table_renders_compact_blocks() {
        let messages = vec![
            sample_message(MessageKind::System, "first line\nsecond line", None),
            sample_message(MessageKind::Assistant, "assistant response", Some("call-1")),
        ];

        let rendered = render_messages_table(&messages);
        assert!(rendered.contains("Message 1"));
        assert!(rendered.contains("role .......... system"));
        assert!(rendered.contains("content ....... first line"));
        assert!(rendered.contains("Message 2"));
        assert!(rendered.contains("role .......... assistant"));
        assert!(rendered.contains("tool call id ... call-1"));
        assert!(!rendered.contains("\"kind\""));
    }

    fn sample_tool_request(tool: &str, arguments: &str) -> crate::runner::ToolRequestRecord {
        crate::runner::ToolRequestRecord {
            request_id: "req-1".to_string(),
            parent_id: "parent-1".to_string(),
            call_id: "call-1".to_string(),
            tool: tool.to_string(),
            arguments: arguments.to_string(),
        }
    }

    fn sample_tool_call_completed(
        tool: &str,
        arguments: &str,
        summary: &str,
        fields: &[(&str, &str)],
    ) -> crate::record::ToolExecutionRecord {
        let mut payload = ploke_tui::tools::ToolUiPayload::new(
            ploke_tui::tools::ToolName::RequestCodeContext,
            ArcStr::from("call-1"),
            summary,
        );
        for (name, value) in fields {
            payload = payload.with_field(*name, *value);
        }

        crate::record::ToolExecutionRecord {
            request: sample_tool_request(tool, arguments),
            result: crate::record::ToolResult::Completed(crate::runner::ToolCompletedRecord {
                request_id: "req-1".to_string(),
                parent_id: "parent-1".to_string(),
                call_id: "call-1".to_string(),
                tool: tool.to_string(),
                content: "synthetic output".to_string(),
                ui_payload: Some(payload),
                latency_ms: 17,
            }),
            latency_ms: 17,
        }
    }

    fn sample_tool_call_failed(
        tool: &str,
        arguments: &str,
        summary: &str,
        fields: &[(&str, &str)],
    ) -> crate::record::ToolExecutionRecord {
        let mut payload = ploke_tui::tools::ToolUiPayload::new(
            ploke_tui::tools::ToolName::NsPatch,
            ArcStr::from("call-2"),
            summary,
        );
        payload.error_code = Some(ploke_tui::tools::ToolErrorCode::InvalidFormat);
        for (name, value) in fields {
            payload = payload.with_field(*name, *value);
        }

        crate::record::ToolExecutionRecord {
            request: sample_tool_request(tool, arguments),
            result: crate::record::ToolResult::Failed(crate::runner::ToolFailedRecord {
                request_id: "req-2".to_string(),
                parent_id: "parent-2".to_string(),
                call_id: "call-2".to_string(),
                tool: Some(tool.to_string()),
                error: "synthetic error".to_string(),
                ui_payload: Some(payload),
                latency_ms: 31,
            }),
            latency_ms: 31,
        }
    }

    #[test]
    fn render_tool_loop_table_renders_compact_blocks() {
        let tool_calls = vec![
            sample_tool_call_completed(
                "request_code_context",
                r#"{"search_term":"Arg::with_name(\"iglob\")"}"#,
                "Context assembled",
                &[("returned", "10 snippets"), ("top score", "0.031")],
            ),
            sample_tool_call_failed(
                "non_semantic_patch",
                r#"{"patches":[{"file":"src/main.rs"}]}"#,
                "Send one patch per tool call",
                &[("field", "patches"), ("expected", "array length of 1")],
            ),
        ];

        let rendered = render_tool_loop_table(1, &tool_calls);
        assert!(rendered.contains("Turn 1"));
        assert!(rendered.contains("[0] request_code_context"));
        assert!(rendered.contains("input"));
        assert!(rendered.contains("status"));
        assert!(rendered.contains("summary"));
        assert!(rendered.contains("returned"));
        assert!(rendered.contains("top score"));
        assert!(rendered.contains("code ........ internal") || rendered.contains("code"));
        assert!(rendered.contains("field"));
        assert!(rendered.contains("expected"));
        assert!(!rendered.contains("\"arguments\""));
    }

    #[test]
    fn tool_call_next_step_index_uses_first_real_index() {
        assert_eq!(tool_call_next_step_index(0), None);
        assert_eq!(tool_call_next_step_index(3), Some(0));
    }

    #[test]
    fn inspect_tool_calls_accepts_missing_record_selector() {
        Cli::try_parse_from(["ploke-eval", "inspect", "tool-calls"])
            .expect("inspect tool-calls should default to the most recent run");
    }

    #[test]
    fn resolve_record_path_defaults_to_last_run_record() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let run_dir = eval_home.join("runs").join("demo-run");
        std::fs::create_dir_all(&run_dir).expect("run dir");
        crate::run_history::record_last_run_at(&eval_home, &run_dir).expect("record last run");

        let path = resolve_record_path_from_eval_home(None, None, eval_home)
            .expect("default record path should resolve");

        assert_eq!(path, run_dir.join("record.json.gz"));
    }

    #[test]
    fn abbreviate_path_tail_keeps_filename_suffix() {
        let path = "/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/globset/src/lib.rs";
        let abbreviated = abbreviate_path_tail(path, 32);
        assert!(abbreviated.starts_with(".../"));
        assert!(abbreviated.ends_with("globset/src/lib.rs"));
    }

    #[test]
    fn summarize_tool_inputs_prefers_parsed_fields() {
        let args = r#"{"file":"/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/globset/src/lib.rs","end_line":120}"#;
        let summary = summarize_tool_inputs(args);
        assert!(summary.contains("file=.../globset/src/lib.rs"));
        assert!(summary.contains("end_line=120"));
    }

    #[test]
    fn inspect_tool_calls_accepts_positional_index() {
        let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "tool-calls", "5"])
            .expect("tool-calls should accept a positional index");

        match parsed.command {
            Command::Inspect(InspectCommand {
                command: InspectSubcommand::ToolCalls(cmd),
            }) => assert_eq!(cmd.index, Some(5)),
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn inspect_turn_accepts_positional_turn() {
        let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "turn", "1"])
            .expect("turn should accept a positional turn number");

        match parsed.command {
            Command::Inspect(InspectCommand {
                command: InspectSubcommand::Turn(cmd),
            }) => {
                assert_eq!(cmd.turn, Some(1));
                assert_eq!(cmd.turn_flag, None);
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn inspect_turn_accepts_loop_show_option() {
        let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "turn", "1", "--show", "loop"])
            .expect("turn should accept the loop show option");

        match parsed.command {
            Command::Inspect(InspectCommand {
                command: InspectSubcommand::Turn(cmd),
            }) => assert_eq!(cmd.show, TurnShowOption::Loop),
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn inspect_turn_accepts_responses_show_option() {
        let parsed =
            Cli::try_parse_from(["ploke-eval", "inspect", "turn", "1", "--show", "responses"])
                .expect("turn should accept the responses show option");

        match parsed.command {
            Command::Inspect(InspectCommand {
                command: InspectSubcommand::Turn(cmd),
            }) => assert_eq!(cmd.show, TurnShowOption::Responses),
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn inspect_turn_still_accepts_long_turn_flag() {
        let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "turn", "--turn", "1"])
            .expect("turn should still accept the hidden --turn flag");

        match parsed.command {
            Command::Inspect(InspectCommand {
                command: InspectSubcommand::Turn(cmd),
            }) => {
                assert_eq!(cmd.turn, None);
                assert_eq!(cmd.turn_flag, Some(1));
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn inspect_conversations_accepts_turns_alias() {
        let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "turns"])
            .expect("conversations should accept the turns alias");

        match parsed.command {
            Command::Inspect(InspectCommand {
                command: InspectSubcommand::Conversations(_),
            }) => {}
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn inspect_turn_messages_accepts_role_filters() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "inspect",
            "turn",
            "1",
            "--show",
            "messages",
            "--exclude-roles",
            "system,user",
        ])
        .expect("turn messages should accept role filters");

        match parsed.command {
            Command::Inspect(InspectCommand {
                command: InspectSubcommand::Turn(cmd),
            }) => {
                assert_eq!(cmd.turn, Some(1));
                assert_eq!(
                    cmd.exclude_roles,
                    vec![InspectMessageRole::System, InspectMessageRole::User]
                );
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }
}
