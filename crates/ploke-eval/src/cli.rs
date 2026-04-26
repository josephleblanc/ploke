use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use chrono::Utc;
use clap::{ArgAction, Parser, Subcommand};
use ploke_llm::Router;
use ploke_llm::request::endpoint::Endpoint;
use ploke_llm::router_only::HasEndpoint;
use ploke_llm::router_only::openrouter::{OpenRouter, OpenRouterModelId};
use ploke_llm::{ModelId, ProviderKey};
use ploke_protocol::procedure::{
    ProcedureDebugEvent, ProcedureDebugEventKind, ProcedureDebugSink, set_procedure_debug_sink,
};
use ploke_protocol::tool_calls::trace::NeighborhoodSource;
use ploke_protocol::tool_calls::{review, segment, trace};
use ploke_protocol::{JsonAdjudicator, JsonLlmConfig, Procedure};
use regex::Regex;
use serde::Serialize;
use tokio::task::JoinSet;

mod prototype1_process;
/// Prototype 1 typed state model and persisted artifact map.
///
/// See `prototype1_state::mod` for the implementation split and the on-disk
/// campaign layout under `~/.ploke-eval/campaigns/<campaign-id>/prototype1/`.
mod prototype1_state;

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
use crate::inner::registry::RunRegistration;
use crate::intervention::{
    INTERVENTION_APPLY_PROCEDURE, INTERVENTION_ISSUE_DETECTION_PROCEDURE,
    INTERVENTION_SYNTHESIS_PROCEDURE, InterventionApplyInput, InterventionApplyOutput,
    InterventionSynthesisInput, IssueCase, IssueDetectionInput, IssueDetectionOutput,
    detect_issue_cases, execute_intervention_apply, issue_detection_artifact_input,
    operation_target_artifact_id, select_primary_issue, synthesize_intervention_with_llm,
};
use crate::intervention_issue_aggregate::{
    IssueDetectionAggregate, IssueDetectionAggregateError, load_issue_detection_aggregate,
};
use crate::layout::{
    active_model_file, batches_dir, cache_dir, datasets_dir, instances_dir, model_registry_file,
    models_dir, repos_dir, starting_db_cache_dir, workspace_root_for_key,
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
use crate::run_history::{
    RunDirPreference, list_finished_record_paths_in_instances_root, preferred_run_dir_for_instance,
    print_assistant_messages_from_record_path,
};
use crate::run_registry::list_registrations_for_instance;
use crate::runner::{
    BatchRunArtifactPaths, BatchRunSummary, MultiSweBenchSubmissionRecord, ReplayMsbBatchRequest,
    RunMsbAgentBatchRequest, RunMsbAgentSingleRequest, RunMsbBatchRequest, RunMsbSingleRequest,
    resolve_provider_for_model,
};
use crate::selection::{
    ActiveSelection, ActiveSelectionSlot, clear_active_selection, load_active_selection,
    load_active_selection_at, render_selection_warnings, save_active_selection,
    unset_active_selection_slot,
};
use crate::spec::{
    EvalBudget, IssueInput, OutputMode, PrepareError, PrepareSingleRunRequest, PrepareWrite,
    PreparedCampaignContext,
};
use crate::target_registry::{
    BenchmarkFamily, RegistryEntry, RegistryRecomputeRequest, TargetRegistry, load_target_registry,
    recompute_target_registry, render_target_registry_status, target_registry_path,
};

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
#[command(about = "Run higher-level loop wrappers over eval, protocol, and intervention stages")]
pub struct LoopCommand {
    #[command(subcommand)]
    pub command: LoopSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum LoopSubcommand {
    /// Run Prototype 1 through eval configuration, baseline arm, synthesis, treatment, and compare.
    Prototype1(Prototype1LoopCommand),
    /// Drive the new typed Prototype 1 runtime path for one staged node.
    #[command(hide = true)]
    Prototype1State(Prototype1StateCommand),
    /// Inspect and manipulate Prototype 1 treatment-branch state.
    Prototype1Branch(Prototype1BranchCommand),
    /// Inspect one staged Prototype 1 runner node for trampoline execution.
    #[command(hide = true)]
    Prototype1Runner(Prototype1RunnerCommand),
    /// Observe Prototype 1 live-loop files while trampolined parents run.
    Prototype1Monitor(Prototype1MonitorCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1StateStopAfter {
    Materialize,
    Build,
    Spawn,
    Complete,
}

#[derive(Debug, Parser)]
#[command(about = "Run the new typed Prototype 1 state transitions for one staged node")]
pub struct Prototype1StateCommand {
    #[arg(long)]
    pub campaign: String,

    #[arg(long)]
    pub node_id: Option<String>,

    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Bootstrap the active checkout by writing and committing parent identity.
    #[arg(long)]
    pub init_parent_identity: bool,

    /// Branch to create or switch to before writing initial parent identity.
    #[arg(long, value_name = "BRANCH", requires = "init_parent_identity")]
    pub identity_branch: Option<String>,

    /// Successor handoff token written by the previous parent runtime.
    #[arg(long, value_name = "PATH")]
    pub handoff_invocation: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = Prototype1StateStopAfter::Complete)]
    pub stop_after: Prototype1StateStopAfter,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
#[command(about = "List, peek, or watch Prototype 1 live-loop output locations")]
pub struct Prototype1MonitorCommand {
    #[arg(long)]
    pub campaign: String,

    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Prototype1MonitorSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum Prototype1MonitorSubcommand {
    /// Print expected output locations and volatility notes.
    List,
    /// Print short excerpts from existing expected output files.
    Peek(Prototype1MonitorPeekCommand),
    /// Poll expected output locations and print file changes.
    Watch(Prototype1MonitorWatchCommand),
}

#[derive(Debug, Parser)]
pub struct Prototype1MonitorPeekCommand {
    /// Maximum trailing lines to show per text file.
    #[arg(long, default_value_t = 20)]
    pub lines: usize,

    /// Maximum bytes to read per file.
    #[arg(long, default_value_t = 8192)]
    pub bytes: usize,
}

#[derive(Debug, Parser)]
pub struct Prototype1MonitorWatchCommand {
    /// Polling interval in milliseconds.
    #[arg(long, default_value_t = 50)]
    pub interval_ms: u64,

    /// Include a one-time initial snapshot of existing files.
    #[arg(long)]
    pub print_initial: bool,
}

#[derive(Debug, Parser)]
#[command(about = "Inspect and manipulate Prototype 1 treatment-branch state")]
pub struct Prototype1BranchCommand {
    #[command(subcommand)]
    pub command: Prototype1BranchSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum Prototype1BranchSubcommand {
    Status(Prototype1BranchStatusCommand),
    Show(Prototype1BranchShowCommand),
    Apply(Prototype1BranchApplyCommand),
    Evaluate(Prototype1BranchEvaluateCommand),
    Select(Prototype1BranchSelectCommand),
    Restore(Prototype1BranchRestoreCommand),
}

#[derive(Debug, Parser)]
#[command(about = "Show the current Prototype 1 branch registry for a loop campaign")]
pub struct Prototype1BranchStatusCommand {
    #[arg(long)]
    pub campaign: String,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
#[command(about = "Show the stored content and metadata for a synthesized treatment branch")]
pub struct Prototype1BranchShowCommand {
    #[arg(long)]
    pub campaign: String,

    #[arg(long)]
    pub branch_id: String,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
#[command(about = "Apply a synthesized treatment branch to a repo root")]
pub struct Prototype1BranchApplyCommand {
    #[arg(long)]
    pub campaign: String,

    #[arg(long)]
    pub branch_id: String,

    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
#[command(
    about = "Run the treatment arm for one branch on the same slice and compare mechanized metrics against baseline"
)]
pub struct Prototype1BranchEvaluateCommand {
    #[arg(long)]
    pub campaign: String,

    #[arg(long)]
    pub branch_id: String,

    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    #[arg(long)]
    pub stop_on_error: bool,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
#[command(about = "Select a synthesized treatment branch as the active branch for its target")]
pub struct Prototype1BranchSelectCommand {
    #[arg(long)]
    pub campaign: String,

    #[arg(long)]
    pub branch_id: String,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
#[command(
    about = "Restore the source content for a treatment branch and clear it from active state"
)]
pub struct Prototype1BranchRestoreCommand {
    #[arg(long)]
    pub campaign: String,

    #[arg(long)]
    pub branch_id: String,

    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
#[command(about = "Inspect one Prototype 1 runner node or execute one persisted invocation")]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1LoopStopAfter {
    BaselineEval,
    BaselineProtocol,
    TargetSelection,
    InterventionApply,
    Compare,
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

    /// Stop search continuation once a keep-worthy branch is found.
    #[arg(long)]
    pub stop_on_first_keep: bool,

    /// Require the selected next branch to have overall disposition=keep before continuation.
    #[arg(long, action = ArgAction::Set, default_value_t = true)]
    pub require_keep_for_continuation: bool,

    /// Stop the wrapper after the selected implemented stage.
    #[arg(long, value_enum, default_value_t = Prototype1LoopStopAfter::Compare)]
    pub stop_after: Prototype1LoopStopAfter,

    /// Synthesize/select the intervention candidate, but do not overwrite the target file.
    #[arg(long)]
    pub dry_run: bool,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
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
            Command::Run(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::Just(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::Doctor => match run_doctor() {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
            Command::Transcript(cmd) => match cmd.run().await {
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
            Command::Select(cmd) => match cmd.run().await {
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
            Command::Loop(cmd) => match cmd.run().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            },
        }
    }
}

impl RunCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunSubcommand::Repo(cmd) => cmd.run().await,
            RunSubcommand::Datasets(cmd) => cmd.run().await,
            RunSubcommand::Prepare(cmd) => cmd.run().await,
            RunSubcommand::List(cmd) => cmd.run().await,
            RunSubcommand::Single(cmd) => cmd.run().await,
            RunSubcommand::Batch(cmd) => cmd.run().await,
            RunSubcommand::Replay(cmd) => cmd.run().await,
        }
    }
}

impl RunRepoCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunRepoSubcommand::Fetch(cmd) => cmd.run(),
        }
    }
}

impl RunDatasetsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunDatasetsSubcommand::List => {
                print_builtin_dataset_entries();
                Ok(())
            }
        }
    }
}

impl RunPrepareCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunPrepareSubcommand::Custom(cmd) => cmd.run(),
            RunPrepareSubcommand::Instance(cmd) => cmd.run(),
            RunPrepareSubcommand::Batch(cmd) => cmd.run(),
        }
    }
}

impl SelectCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            SelectSubcommand::Status(cmd) => cmd.run(),
            SelectSubcommand::Campaign(cmd) => cmd.run(),
            SelectSubcommand::Batch(cmd) => cmd.run(),
            SelectSubcommand::Instance(cmd) => cmd.run(),
            SelectSubcommand::Attempt(cmd) => cmd.run(),
            SelectSubcommand::Unset(cmd) => cmd.run(),
            SelectSubcommand::Clear(cmd) => cmd.run(),
        }
    }
}

impl RunSingleWorkflowCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunSingleWorkflowSubcommand::Setup(cmd) => cmd.run().await,
            RunSingleWorkflowSubcommand::Agent(cmd) => cmd.run().await,
        }
    }
}

impl RunBatchWorkflowCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunBatchWorkflowSubcommand::Setup(cmd) => cmd.run().await,
            RunBatchWorkflowSubcommand::Agent(cmd) => cmd.run().await,
        }
    }
}

impl RunReplayCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RunReplaySubcommand::Batch(cmd) => cmd.run().await,
        }
    }
}

impl JustCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            JustSubcommand::FetchRepo(cmd) => cmd.run(),
            JustSubcommand::ListDatasets => {
                print_builtin_dataset_entries();
                Ok(())
            }
            JustSubcommand::PrepareCustom(cmd) => cmd.run(),
            JustSubcommand::PrepareInstance(cmd) => cmd.run(),
            JustSubcommand::PrepareBatch(cmd) => cmd.run(),
            JustSubcommand::SingleSetup(cmd) => cmd.run().await,
            JustSubcommand::Single(cmd) => cmd.run().await,
            JustSubcommand::BatchSetup(cmd) => cmd.run().await,
            JustSubcommand::Batch(cmd) => cmd.run().await,
            JustSubcommand::ReplayBatch(cmd) => cmd.run().await,
        }
    }
}

impl LoopCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            LoopSubcommand::Prototype1(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1State(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1Branch(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1Runner(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1Monitor(cmd) => cmd.run(),
        }
    }
}

impl Prototype1BranchCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            Prototype1BranchSubcommand::Status(cmd) => cmd.run(),
            Prototype1BranchSubcommand::Show(cmd) => cmd.run(),
            Prototype1BranchSubcommand::Apply(cmd) => cmd.run(),
            Prototype1BranchSubcommand::Evaluate(cmd) => cmd.run().await,
            Prototype1BranchSubcommand::Select(cmd) => cmd.run(),
            Prototype1BranchSubcommand::Restore(cmd) => cmd.run(),
        }
    }
}

struct TimingTrace;

struct TimingScope {
    label: String,
    started_at: Instant,
}

impl TimingTrace {
    fn mark(label: &str) {
        eprintln!("{} {}", Utc::now().format("%H:%M:%S"), label);
    }

    fn scope(label: impl Into<String>) -> TimingScope {
        let label = label.into();
        Self::mark(&format!("{label}.start"));
        TimingScope {
            label,
            started_at: Instant::now(),
        }
    }
}

impl Drop for TimingScope {
    fn drop(&mut self) {
        eprintln!(
            "{} {}.end +{:.3}s",
            Utc::now().format("%H:%M:%S"),
            self.label,
            self.started_at.elapsed().as_secs_f64()
        );
    }
}

fn print_builtin_dataset_entries() {
    for entry in builtin_dataset_registry_entries() {
        println!("{}\t{}\t{}", entry.key, entry.language, entry.url);
    }
}

async fn execute_batch_eval_for_manifest(
    batch_manifest: PathBuf,
    index_debug_snapshots: bool,
    use_default_model: bool,
    model_id: Option<String>,
    provider: Option<ProviderKey>,
    stop_on_error: bool,
) -> Result<BatchRunArtifactPaths, PrepareError> {
    RunMsbAgentBatchRequest {
        batch_manifest,
        index_debug_snapshots,
        use_default_model,
        model_id,
        provider,
        stop_on_error,
    }
    .run()
    .await
}

#[derive(Debug)]
struct ProtocolBatchExecution {
    executions: Vec<ProtocolRunExecution>,
    failures: Vec<String>,
}

async fn execute_protocol_run_tasks(
    tasks: Vec<ProtocolRunTask>,
    model_id: String,
    provider_slug: Option<String>,
    max_concurrency: usize,
    stop_on_error: bool,
) -> Result<ProtocolBatchExecution, PrepareError> {
    let mut executions = Vec::new();
    let mut failures = Vec::new();
    let max_concurrency = max_concurrency.max(1);
    let mut pending = tasks.into_iter().collect::<VecDeque<_>>();
    let mut join_set = JoinSet::new();

    while join_set.len() < max_concurrency {
        let Some(task) = pending.pop_front() else {
            break;
        };
        spawn_protocol_run_task(&mut join_set, task, model_id.clone(), provider_slug.clone());
    }

    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok(Ok(execution)) => {
                executions.push(execution);
                if let Some(task) = pending.pop_front() {
                    spawn_protocol_run_task(
                        &mut join_set,
                        task,
                        model_id.clone(),
                        provider_slug.clone(),
                    );
                }
            }
            Ok(Err(err)) => {
                failures.push(err.to_string());
                if stop_on_error {
                    join_set.abort_all();
                    return Err(err);
                }
                if let Some(task) = pending.pop_front() {
                    spawn_protocol_run_task(
                        &mut join_set,
                        task,
                        model_id.clone(),
                        provider_slug.clone(),
                    );
                }
            }
            Err(err) => {
                let detail = PrepareError::DatabaseSetup {
                    phase: "protocol_run_tasks",
                    detail: format!("protocol worker task failed: {err}"),
                };
                failures.push(detail.to_string());
                if stop_on_error {
                    join_set.abort_all();
                    return Err(detail);
                }
                if let Some(task) = pending.pop_front() {
                    spawn_protocol_run_task(
                        &mut join_set,
                        task,
                        model_id.clone(),
                        provider_slug.clone(),
                    );
                }
            }
        }
    }

    Ok(ProtocolBatchExecution {
        executions,
        failures,
    })
}

fn persist_issue_detection_for_record(
    record_path: &Path,
) -> Result<IssueDetectionOutput, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let protocol_aggregate = load_protocol_aggregate(record_path).ok();
    let detection_input = IssueDetectionInput::from_record(record, protocol_aggregate);
    let persisted_input = issue_detection_artifact_input(&detection_input);
    let output = detect_issue_cases(&detection_input);
    let artifact = build_issue_detection_artifact(&output);
    write_protocol_artifact(
        record_path,
        INTERVENTION_ISSUE_DETECTION_PROCEDURE,
        &subject_id,
        None,
        None,
        &persisted_input,
        &output,
        &artifact,
    )?;
    Ok(output)
}

async fn persist_intervention_synthesis_for_record(
    record_path: &Path,
    issue: IssueCase,
    source_state_id: String,
    model_id: Option<String>,
    provider: Option<String>,
) -> Result<crate::intervention::InterventionSynthesisOutput, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let target_relpath = PathBuf::from(issue.target_tool.description_artifact_relpath());
    let source_content =
        fs::read_to_string(&target_relpath).map_err(|source| PrepareError::ReadManifest {
            path: target_relpath.clone(),
            source,
        })?;
    let input = InterventionSynthesisInput {
        issue,
        source_state_id,
        source_content,
        // The generic CLI path does not yet know the durable Artifact target
        // for this record. Downstream layers will preserve a fallback text-file
        // surface id, but future backend-aware callers should pass an
        // OperationTarget here instead of relying on that fallback.
        operation_target: None,
    };
    let model_id = resolve_protocol_model_id(model_id)?;
    let provider_slug = resolve_protocol_provider_slug(&model_id, provider)?;
    let cfg = JsonLlmConfig {
        model_id: model_id.to_string(),
        provider_slug,
        timeout_secs: 45,
        max_tokens: 3200,
    };
    let run = synthesize_intervention_with_llm(input.clone(), cfg.clone())
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "intervention_synthesis",
            detail: err.to_string(),
        })?;
    write_protocol_artifact(
        record_path,
        INTERVENTION_SYNTHESIS_PROCEDURE,
        &subject_id,
        Some(cfg.model_id.as_str()),
        cfg.provider_slug.as_deref(),
        &input,
        &run.output,
        &run.artifact,
    )?;
    Ok(run.output)
}

fn persist_intervention_apply_for_record(
    record_path: &Path,
    synthesis: &crate::intervention::InterventionSynthesisOutput,
    candidate_id: &str,
    repo_root: &Path,
) -> Result<InterventionApplyOutput, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let candidate = synthesis
        .candidate_set
        .candidates
        .iter()
        .find(|candidate| candidate.candidate_id == candidate_id)
        .cloned()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "intervention apply candidate '{}' not found for subject '{}'",
                candidate_id, subject_id
            ),
        })?;
    let base_artifact_id = synthesis
        .candidate_set
        .operation_target
        .as_ref()
        .and_then(operation_target_artifact_id)
        .cloned();
    let patch_id = candidate.patch_id.clone();
    let input = InterventionApplyInput {
        source_state_id: synthesis.candidate_set.source_state_id.clone(),
        candidate,
        target_relpath: synthesis.candidate_set.target_relpath.clone(),
        expected_source_content: synthesis.candidate_set.source_content.clone(),
        repo_root: repo_root.to_path_buf(),
        base_artifact_id,
        patch_id,
    };
    let output =
        execute_intervention_apply(&input).map_err(|source| PrepareError::DatabaseSetup {
            phase: "intervention_apply",
            detail: source.to_string(),
        })?;
    write_protocol_artifact(
        record_path,
        INTERVENTION_APPLY_PROCEDURE,
        &subject_id,
        None,
        None,
        &input,
        &output,
        &output,
    )?;
    Ok(output)
}

fn pending_prototype1_stages(stage_reached: Prototype1LoopStopAfter) -> Vec<&'static str> {
    match stage_reached {
        Prototype1LoopStopAfter::BaselineEval => {
            vec![
                "baseline protocol",
                "target selection",
                "intervention apply",
                "treatment arm",
                "compare",
            ]
        }
        Prototype1LoopStopAfter::BaselineProtocol => {
            vec![
                "target selection",
                "intervention apply",
                "treatment arm",
                "compare",
            ]
        }
        Prototype1LoopStopAfter::TargetSelection => {
            vec!["intervention apply", "treatment arm", "compare"]
        }
        Prototype1LoopStopAfter::InterventionApply => vec!["treatment arm", "compare"],
        Prototype1LoopStopAfter::Compare => Vec::new(),
    }
}

fn write_json_file_pretty<T: Serialize>(path: &Path, value: &T) -> Result<(), PrepareError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(PrepareError::Serialize)?;
    fs::write(path, bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
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

impl PrepareMsbSingleCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let prepared = PrepareMsbSingleRunRequest {
            dataset_file: self.dataset,
            dataset_key: self.dataset_key,
            instance_id: self.instance,
            repo_cache: self.repo_cache.unwrap_or(repos_dir()?),
            instances_root: self.instances_root.unwrap_or(instances_dir()?),
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
            instances_root: self.instances_root.unwrap_or(instances_dir()?),
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

#[derive(Debug, Parser)]
#[command(
    about = "Manage campaign manifests, validation, and campaign-scoped submission export",
    after_help = "\
Campaigns are the stateful operator layer for measured work.

Default files:
  manifest: ~/.ploke-eval/campaigns/<campaign>/campaign.json
  closure:  ~/.ploke-eval/campaigns/<campaign>/closure-state.json

Typical flow:
  campaign list
  campaign init --campaign <campaign> --from-registry
  campaign show --campaign <campaign>
  campaign validate --campaign <campaign>
  closure status --campaign <campaign>
  closure advance eval --campaign <campaign>
  campaign export-submissions --campaign <campaign>
"
)]
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
    /// Export Multi-SWE-bench submission JSONL from completed runs in the campaign closure state.
    ExportSubmissions(CampaignExportSubmissionsCommand),
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

    /// Override the instances root.
    #[arg(long = "instances-root", alias = "runs-root", value_name = "PATH")]
    pub instances_root: Option<PathBuf>,

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

#[derive(Debug, Parser)]
#[command(after_help = "\
Default output path when --output is omitted:
  ~/.ploke-eval/campaigns/<campaign>/multi-swe-bench-submission.jsonl

If --nonempty-only is set, the default becomes:
  ~/.ploke-eval/campaigns/<campaign>/multi-swe-bench-submission.nonempty.jsonl

Source of truth:
  This command exports from completed runs in closure state and reads per-run
  submission artifacts. It is a stronger export surface than the raw batch
  aggregate JSONL.

Selection behavior:
  The export writes one record per completed closure row.
  If multiple completed runs exist for one instance, the command selects the
  preferred run that has a per-run submission artifact.
")]
pub struct CampaignExportSubmissionsCommand {
    /// Stable campaign identifier, used under ~/.ploke-eval/campaigns/<campaign>.
    #[arg(long)]
    pub campaign: String,

    /// Write only records whose fix_patch is non-empty.
    #[arg(long)]
    pub nonempty_only: bool,

    /// Output path for the exported JSONL.
    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,

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

#[derive(Debug, Clone, Serialize)]
struct RegistryDatasetView<'a> {
    dataset: &'a str,
    state_path: PathBuf,
    source_paths: Vec<PathBuf>,
    entries: Vec<&'a RegistryEntry>,
}

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
    /// Aggregate campaign-scoped tool usage and failure patterns.
    ToolOverview(InspectToolOverviewCommand),
    /// List DB snapshots at each turn boundary (run.db_snapshots())
    DbSnapshots(InspectDbSnapshotsCommand),
    /// List turns with error outcomes (run.failures())
    Failures(InspectFailuresCommand),
    /// Show run configuration (run.config())
    Config(InspectConfigCommand),
    /// Show compact mechanized operational metrics for one run.
    #[command(alias = "metrics")]
    Operational(InspectOperationalCommand),
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
    /// Show the latest persisted intervention issue-detection artifact for one run.
    #[command(alias = "issues")]
    IssueOverview(InspectIssueOverviewCommand),
}

#[derive(Debug, Parser)]
#[command(after_help = "\
WARNING:
  `inspect conversations --format json` can emit a very large payload and overwhelm
  interactive terminals, logs, or agent context windows.

  Use `--format json --full` only when you intend to filter the output immediately.

Recommended patterns:
  ploke-eval inspect conversations --instance <id>
  ploke-eval inspect conversations --instance <id> --format json --full | jq '.[0].patch_artifact'
  ploke-eval inspect conversations --instance <id> --format json --full | rg '\"error_id\"|\"call_id\"'
")]
pub struct InspectConversationsCommand {
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
    #[arg(long, conflicts_with = "record")]
    pub instance: Option<String>,

    /// Acknowledge that full JSON output may be very large. Required with `--format json`.
    #[arg(long)]
    pub full: bool,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct InspectToolCallsCommand {
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
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
pub struct InspectToolOverviewCommand {
    /// Campaign id whose eval-complete runs should be scanned.
    #[arg(long)]
    pub campaign: String,

    /// Restrict the report to one tool name, e.g. `apply_code_edit`.
    #[arg(long)]
    pub tool: Option<String>,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Maximum number of rows to show in ranked sections.
    #[arg(long, default_value_t = 8)]
    pub limit: usize,
}

#[derive(Debug, Parser)]
pub struct InspectDbSnapshotsCommand {
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
pub struct InspectFailuresCommand {
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
pub struct InspectConfigCommand {
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
pub struct InspectOperationalCommand {
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
pub struct InspectTurnCommand {
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
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
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
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
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["instance", "all_runs", "campaign"])]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
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

#[derive(Debug, Parser)]
pub struct InspectIssueOverviewCommand {
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
    /// Path to a run record file (record.json.gz). Defaults to the latest registered attempt's record.json.gz.
    #[arg(long, value_name = "PATH", conflicts_with = "instance")]
    pub record: Option<PathBuf>,

    /// Benchmark instance id, used to resolve the latest registered attempt's
    /// ~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz.
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
            (None, Some(instance)) => instances_dir()?.join(instance).join("run.json"),
            _ => {
                return Err(PrepareError::MissingRunManifest(
                    instances_dir()?.join("<instance>/run.json"),
                ));
            }
        };

        let artifacts = RunMsbSingleRequest {
            run_manifest,
            batch_id: None,
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
            (None, Some(instance)) => instances_dir()?.join(instance).join("run.json"),
            _ => {
                return Err(PrepareError::MissingRunManifest(
                    instances_dir()?.join("<instance>/run.json"),
                ));
            }
        };

        let artifacts = RunMsbAgentSingleRequest {
            run_manifest,
            batch_id: None,
            index_debug_snapshots: self.index_debug_snapshots,
            use_default_model: self.use_default_model,
            model_id: self.model_id,
            provider: parse_provider_key(self.provider)?,
            embedding_model_id: self.embedding_model_id,
            embedding_provider: parse_provider_key(self.embedding_provider)?,
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
            (None, Some(instance)) => instances_dir()?.join(instance).join("run.json"),
            _ => {
                return Err(PrepareError::MissingRunManifest(
                    instances_dir()?.join("<instance>/run.json"),
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

impl TranscriptCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(None, self.instance, None)?;
        print_assistant_messages_from_record_path(&resolution.record_path).await?;
        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl RunListCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let selection = load_active_selection()?;
        let instance = self
            .instance
            .clone()
            .or_else(|| selection.instance.clone())
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "run_list",
                detail:
                    "instance is required (pass --instance or `ploke-eval select instance <id>`)"
                        .to_string(),
            })?;
        let registrations = list_attempt_registrations(&instances_dir()?, &instance)?;
        let mut warnings = render_selection_warnings(&selection_with_resolution(
            &selection,
            Some(&instance),
            None,
        ));
        if !registrations.is_empty()
            && has_legacy_instance_root_artifacts(&instances_dir()?, &instance)
        {
            warnings.push(legacy_instance_root_warning(&instance));
        }
        let rows: Vec<_> = registrations
            .iter()
            .enumerate()
            .map(|(index, registration)| RunListRow {
                attempt: (index + 1) as u32,
                latest: index + 1 == registrations.len(),
                execution_status: registration.lifecycle.execution_status,
                submission_status: registration.lifecycle.submission_status,
                run_arm_id: registration.frozen_spec.run_arm_id.clone(),
                model_id: registration.frozen_spec.model_id.clone(),
                provider_slug: registration.frozen_spec.provider_slug.clone(),
                started_at: registration.lifecycle.started_at.clone(),
                finished_at: registration.lifecycle.finished_at.clone(),
                run_root: registration.artifacts.run_root.clone(),
            })
            .collect();

        match self.format {
            InspectOutputFormat::Table => {
                if rows.is_empty() {
                    println!("instance {} has no registered attempts.", instance);
                } else {
                    println!(
                        "{:<7} {:<6} {:<11} {:<13} {:<28} {:<12} {:<10} {}",
                        "Attempt",
                        "Latest",
                        "Execution",
                        "Submission",
                        "Arm",
                        "Provider",
                        "Model",
                        "Finished"
                    );
                    println!("{}", "-".repeat(120));
                    for row in &rows {
                        println!(
                            "{:<7} {:<6} {:<11} {:<13} {:<28} {:<12} {:<10} {}",
                            row.attempt,
                            if row.latest { "yes" } else { "" },
                            execution_status_label(row.execution_status),
                            submission_status_label(row.submission_status),
                            truncate_for_table(&row.run_arm_id, 26),
                            truncate_for_table(row.provider_slug.as_deref().unwrap_or("-"), 10),
                            truncate_for_table(row.model_id.as_deref().unwrap_or("-"), 8),
                            row.finished_at.as_deref().unwrap_or("-"),
                        );
                    }
                    println!("\ninstance: {}", instance);
                    println!(
                        "latest attempt: {}",
                        rows.last().map(|row| row.attempt).unwrap_or(0)
                    );
                }
                for warning in warnings {
                    println!("warning: {warning}");
                }
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "instance": instance,
                    "selection": selection,
                    "warnings": warnings,
                    "runs": rows,
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

impl ConversationsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();

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

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            InspectSubcommand::Conversations(cmd) => cmd.run().await,
            InspectSubcommand::ToolCalls(cmd) => cmd.run().await,
            InspectSubcommand::ToolOverview(cmd) => cmd.run().await,
            InspectSubcommand::DbSnapshots(cmd) => cmd.run().await,
            InspectSubcommand::Failures(cmd) => cmd.run().await,
            InspectSubcommand::Config(cmd) => cmd.run().await,
            InspectSubcommand::Operational(cmd) => cmd.run().await,
            InspectSubcommand::Turn(cmd) => cmd.run().await,
            InspectSubcommand::Query(cmd) => cmd.run().await,
            InspectSubcommand::ProtocolArtifacts(cmd) => cmd.run().await,
            InspectSubcommand::ProtocolOverview(cmd) => cmd.run().await,
            InspectSubcommand::IssueOverview(cmd) => cmd.run().await,
        }
    }
}

impl ProtocolCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            ProtocolSubcommand::Status(cmd) => cmd.run().await,
            ProtocolSubcommand::Run(cmd) => cmd.run().await,
            ProtocolSubcommand::IssueDetection(cmd) => cmd.run().await,
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
            CampaignSubcommand::ExportSubmissions(cmd) => cmd.run().await,
        }
    }
}

impl RegistryCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RegistrySubcommand::Recompute(cmd) => cmd.run().await,
            RegistrySubcommand::Status(cmd) => cmd.run().await,
            RegistrySubcommand::Show(cmd) => cmd.run().await,
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

impl SelectStatusCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let selection = load_active_selection()?;
        let warnings = render_selection_warnings(&selection);
        match self.format {
            InspectOutputFormat::Table => {
                println!("active selection");
                println!("{}", "-".repeat(40));
                println!(
                    "campaign: {}",
                    selection.campaign.as_deref().unwrap_or("(none)")
                );
                println!("batch: {}", selection.batch.as_deref().unwrap_or("(none)"));
                println!(
                    "instance: {}",
                    selection.instance.as_deref().unwrap_or("(none)")
                );
                println!(
                    "attempt: {}",
                    selection
                        .attempt
                        .map(|attempt| attempt.to_string())
                        .unwrap_or_else(|| "(latest)".to_string())
                );
                for warning in warnings {
                    println!("warning: {warning}");
                }
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "selection": selection,
                    "warnings": warnings,
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

impl SelectCampaignCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let mut selection = load_active_selection()?;
        selection.campaign = Some(self.campaign);
        save_active_selection(&selection)?;
        print_selection_update(&selection);
        Ok(())
    }
}

impl SelectBatchCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let mut selection = load_active_selection()?;
        selection.batch = Some(self.batch);
        save_active_selection(&selection)?;
        print_selection_update(&selection);
        Ok(())
    }
}

impl SelectInstanceCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let mut selection = load_active_selection()?;
        selection.instance = Some(self.instance);
        selection.attempt = None;
        save_active_selection(&selection)?;
        print_selection_update(&selection);
        Ok(())
    }
}

impl SelectAttemptCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        if self.attempt == 0 {
            return Err(PrepareError::DatabaseSetup {
                phase: "select_attempt",
                detail: "attempt numbers are 1-based".to_string(),
            });
        }
        let mut selection = load_active_selection()?;
        if let Some(instance) = self.instance {
            selection.instance = Some(instance);
        }
        if selection.instance.is_none() {
            return Err(PrepareError::DatabaseSetup {
                phase: "select_attempt",
                detail:
                    "attempt selection requires an active instance (set one first or pass --instance)"
                        .to_string(),
            });
        }
        selection.attempt = Some(self.attempt);
        save_active_selection(&selection)?;
        print_selection_update(&selection);
        Ok(())
    }
}

impl SelectUnsetCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        unset_active_selection_slot(self.scope)?;
        let selection = load_active_selection()?;
        print_selection_update(&selection);
        Ok(())
    }
}

impl SelectClearCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        clear_active_selection()?;
        let selection = load_active_selection()?;
        print_selection_update(&selection);
        Ok(())
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
            instances_root: self.instances_root,
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

impl CampaignExportSubmissionsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let state = load_closure_state(&self.campaign)?;
        let records = collect_campaign_submission_records(&state, self.nonempty_only)?;
        let complete_eval_rows = state
            .instances
            .iter()
            .filter(|row| row.eval_status == ClosureClass::Complete)
            .count();
        let empty_patch_rows = count_campaign_empty_patch_rows(&state)?;
        let output_path = self
            .output
            .unwrap_or(default_campaign_submission_export_path(
                &self.campaign,
                self.nonempty_only,
            )?);

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let mut jsonl = String::new();
        for record in &records {
            let line = serde_json::to_string(record).map_err(PrepareError::Serialize)?;
            jsonl.push_str(&line);
            jsonl.push('\n');
        }
        fs::write(&output_path, jsonl).map_err(|source| PrepareError::WriteManifest {
            path: output_path.clone(),
            source,
        })?;

        let summary = CampaignSubmissionExportSummary {
            campaign_id: self.campaign,
            closure_state_path: closure_state_path(&state.campaign_id)?,
            output_path,
            exported_records: records.len(),
            nonempty_only: self.nonempty_only,
            complete_eval_rows,
            empty_patch_rows_skipped: if self.nonempty_only {
                empty_patch_rows
            } else {
                0
            },
            failed_eval_rows: state
                .instances
                .iter()
                .filter(|row| row.eval_status == ClosureClass::Failed)
                .count(),
        };

        match self.format {
            InspectOutputFormat::Table => {
                println!(
                    "campaign {} | exported {} submission records{}",
                    summary.campaign_id,
                    summary.exported_records,
                    if summary.nonempty_only {
                        " (non-empty only)"
                    } else {
                        ""
                    }
                );
                println!("closure: {}", summary.closure_state_path.display());
                println!("output: {}", summary.output_path.display());
                println!("complete eval rows: {}", summary.complete_eval_rows);
                println!(
                    "empty patch rows skipped: {}",
                    summary.empty_patch_rows_skipped
                );
                println!("failed eval rows: {}", summary.failed_eval_rows);
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).map_err(PrepareError::Serialize)?
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

impl RegistryShowCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let registry = load_target_registry(BenchmarkFamily::MultiSweBenchRust)?;
        let view = registry_dataset_view(&registry, &self.dataset)?;

        match self.format {
            InspectOutputFormat::Table => print_registry_dataset_view(&view),
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&view).map_err(PrepareError::Serialize)?
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
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let subject = build_tool_call_review_subject(&record, self.index)?;
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

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ProtocolStatusCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;
        let instance_id = record.metadata.benchmark.instance_id.clone();
        let mut state = protocol_state_for_run(&instance_id, &record_path)?;
        state.next_command =
            protocol_next_command(&instance_id, &record_path, &state.next_step, true, false);

        match self.format {
            InspectOutputFormat::Table => {
                print_protocol_state_table(&state);
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ProtocolRunCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;
        let instance_id = record.metadata.benchmark.instance_id.clone();
        let mut before = protocol_state_for_run(&instance_id, &record_path)?;
        before.next_command =
            protocol_next_command(&instance_id, &record_path, &before.next_step, true, false);

        let progress_guard = match self.format {
            InspectOutputFormat::Table => Some(install_protocol_progress_printer()),
            InspectOutputFormat::Json => None,
        };

        let executed = match before.next_step.clone() {
            ProtocolNextStep::Ineligible => None,
            ProtocolNextStep::IntentSegmentation => {
                execute_protocol_intent_segments_quiet(
                    &record_path,
                    self.model_id.clone(),
                    self.provider.clone(),
                )
                .await?;
                Some("tool_call_intent_segmentation".to_string())
            }
            ProtocolNextStep::ToolCallReview { index } => {
                execute_protocol_tool_call_review_quiet(
                    &record_path,
                    self.model_id.clone(),
                    self.provider.clone(),
                    index,
                )
                .await?;
                Some(format!("tool_call_review[{index}]"))
            }
            ProtocolNextStep::ToolCallSegmentReview { segment_index } => {
                execute_protocol_tool_call_segment_review_quiet(
                    &record_path,
                    self.model_id.clone(),
                    self.provider.clone(),
                    segment_index,
                )
                .await?;
                Some(format!("tool_call_segment_review[{segment_index}]"))
            }
            ProtocolNextStep::Complete | ProtocolNextStep::Blocked => None,
        };

        let mut after = protocol_state_for_run(&instance_id, &record_path)?;
        after.next_command =
            protocol_next_command(&instance_id, &record_path, &after.next_step, true, false);

        drop(progress_guard);

        match self.format {
            InspectOutputFormat::Table => {
                if let Some(executed) = &executed {
                    println!("protocol run");
                    println!("{}", "-".repeat(40));
                    println!("executed: {}", executed);
                    println!();
                } else {
                    println!("protocol run");
                    println!("{}", "-".repeat(40));
                    println!("executed: (none)");
                    println!();
                }
                print_protocol_state_table(&after);
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "executed": executed,
                    "before": before,
                    "after": after,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ProtocolIssueDetectionCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let subject_id = record.metadata.benchmark.instance_id.clone();
        let protocol_aggregate = load_protocol_aggregate(&record_path).ok();
        let detection_input = IssueDetectionInput::from_record(record, protocol_aggregate);
        let persisted_input = issue_detection_artifact_input(&detection_input);
        let output = detect_issue_cases(&detection_input);
        let artifact = build_issue_detection_artifact(&output);
        let persisted_path = write_protocol_artifact(
            &record_path,
            INTERVENTION_ISSUE_DETECTION_PROCEDURE,
            &subject_id,
            None,
            None,
            &persisted_input,
            &output,
            &artifact,
        )?;

        match self.format {
            InspectOutputFormat::Table => {
                println!("Protocol: {}", INTERVENTION_ISSUE_DETECTION_PROCEDURE);
                println!("{}", "-".repeat(40));
                println!("Artifact: {}", persisted_path.display());
                println!("Cases: {}", output.cases.len());
                if let Some(primary) = select_primary_issue(&output) {
                    print_issue_case_block("Primary issue", &primary);
                } else {
                    println!("Primary issue: (none)");
                }
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "procedure": INTERVENTION_ISSUE_DETECTION_PROCEDURE,
                    "persisted_artifact_path": persisted_path,
                    "output": output,
                    "artifact": artifact,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

struct ProtocolProgressGuard;

impl Drop for ProtocolProgressGuard {
    fn drop(&mut self) {
        set_procedure_debug_sink(None);
    }
}

fn install_protocol_progress_printer() -> ProtocolProgressGuard {
    let sink: ProcedureDebugSink = Arc::new(|event: &ProcedureDebugEvent| match event.event {
        ProcedureDebugEventKind::ProcedureStarted => {
            if event.request_label.is_none() {
                eprintln!("protocol progress: {} started", event.procedure_name);
            }
        }
        ProcedureDebugEventKind::ProcedureFinished => {
            if event.request_label.is_none() {
                if let Some(elapsed_ms) = event.elapsed_ms {
                    eprintln!(
                        "protocol progress: {} finished ({} ms)",
                        event.procedure_name, elapsed_ms
                    );
                } else {
                    eprintln!("protocol progress: {} finished", event.procedure_name);
                }
            }
        }
        ProcedureDebugEventKind::ProcedureFailed => {
            let detail = event.detail.as_deref().unwrap_or("unknown error");
            if let Some(elapsed_ms) = event.elapsed_ms {
                eprintln!(
                    "protocol progress: {} failed after {} ms: {}",
                    event.procedure_name, elapsed_ms, detail
                );
            } else {
                eprintln!(
                    "protocol progress: {} failed: {}",
                    event.procedure_name, detail
                );
            }
        }
        ProcedureDebugEventKind::SubrequestStarted => {
            let label = event.request_label.as_deref().unwrap_or("model_request");
            match (event.request_index, event.request_total) {
                (Some(index), Some(total)) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) sent",
                        index, total, label
                    );
                }
                _ => eprintln!("protocol progress: model request ({}) sent", label),
            }
        }
        ProcedureDebugEventKind::SubrequestFinished => {
            let label = event.request_label.as_deref().unwrap_or("model_request");
            match (event.request_index, event.request_total, event.elapsed_ms) {
                (Some(index), Some(total), Some(elapsed_ms)) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) received ({} ms)",
                        index, total, label, elapsed_ms
                    );
                }
                (Some(index), Some(total), None) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) received",
                        index, total, label
                    );
                }
                _ => eprintln!("protocol progress: model request ({}) received", label),
            }
        }
        ProcedureDebugEventKind::SubrequestFailed => {
            let label = event.request_label.as_deref().unwrap_or("model_request");
            let detail = event.detail.as_deref().unwrap_or("unknown error");
            match (event.request_index, event.request_total, event.elapsed_ms) {
                (Some(index), Some(total), Some(elapsed_ms)) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) failed after {} ms: {}",
                        index, total, label, elapsed_ms, detail
                    );
                }
                (Some(index), Some(total), None) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) failed: {}",
                        index, total, label, detail
                    );
                }
                _ => eprintln!(
                    "protocol progress: model request ({}) failed: {}",
                    label, detail
                ),
            }
        }
    });

    set_procedure_debug_sink(Some(sink));
    ProtocolProgressGuard
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
struct CampaignSubmissionExportSummary {
    campaign_id: String,
    closure_state_path: PathBuf,
    output_path: PathBuf,
    exported_records: usize,
    complete_eval_rows: usize,
    empty_patch_rows_skipped: usize,
    failed_eval_rows: usize,
    nonempty_only: bool,
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

#[derive(Debug, Clone, Serialize)]
struct ProtocolRunState {
    instance_id: String,
    tool_calls_total: usize,
    protocol_eligible: bool,
    artifact_count: usize,
    segmentation_present: bool,
    call_review_count: usize,
    segment_review_count: usize,
    aggregate_available: bool,
    missing_call_indices: Vec<usize>,
    missing_segment_indices: Vec<usize>,
    next_step: ProtocolNextStep,
    next_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aggregate_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ProtocolNextStep {
    Ineligible,
    IntentSegmentation,
    ToolCallReview { index: usize },
    ToolCallSegmentReview { segment_index: usize },
    Complete,
    Blocked,
}

#[derive(Debug, Clone, Serialize)]
struct ProtocolRunTask {
    instance_id: String,
    record_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
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
        instances_root: Some(config.instances_root.clone()),
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

fn default_campaign_submission_export_path(
    campaign_id: &str,
    nonempty_only: bool,
) -> Result<PathBuf, PrepareError> {
    let file_name = if nonempty_only {
        "multi-swe-bench-submission.nonempty.jsonl"
    } else {
        "multi-swe-bench-submission.jsonl"
    };
    Ok(crate::layout::campaigns_dir()?
        .join(campaign_id)
        .join(file_name))
}

fn collect_campaign_submission_records(
    state: &crate::closure::ClosureState,
    nonempty_only: bool,
) -> Result<Vec<MultiSweBenchSubmissionRecord>, PrepareError> {
    let mut records = Vec::new();
    for row in &state.instances {
        if row.eval_status != ClosureClass::Complete {
            continue;
        }
        let record = load_submission_record_for_row(state, row)?;
        if nonempty_only && record.fix_patch.trim().is_empty() {
            continue;
        }
        records.push(record);
    }
    Ok(records)
}

fn count_campaign_empty_patch_rows(
    state: &crate::closure::ClosureState,
) -> Result<usize, PrepareError> {
    let mut count = 0;
    for row in &state.instances {
        if row.eval_status != ClosureClass::Complete {
            continue;
        }
        let record = load_submission_record_for_row(state, row)?;
        if record.fix_patch.trim().is_empty() {
            count += 1;
        }
    }
    Ok(count)
}

fn load_submission_record_for_row(
    state: &crate::closure::ClosureState,
    row: &crate::closure::ClosureInstanceRow,
) -> Result<MultiSweBenchSubmissionRecord, PrepareError> {
    let path = if let Some(path) = row.artifacts.msb_submission.as_ref() {
        path.clone()
    } else {
        let instance_root = state.config.instances_root.join(&row.instance_id);
        let run_dir = preferred_run_dir_for_instance(
            &state.config.instances_root,
            &row.instance_id,
            RunDirPreference::PreferTreatmentWithSubmission,
        )?
        .unwrap_or(instance_root);
        run_dir.join("multi-swe-bench-submission.jsonl")
    };
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(text.trim()).map_err(|source| PrepareError::ParseManifest { path, source })
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
                instances_root: config.instances_root.clone(),
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

            let artifacts = execute_batch_eval_for_manifest(
                prepared.batch.manifest_path(),
                true,
                false,
                Some(config.model_id.clone()),
                provider.clone(),
                policy.stop_on_error,
            )
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
        let execution = execute_protocol_run_tasks(
            tasks,
            config.model_id.clone(),
            config.provider_slug.clone(),
            policy.max_concurrency,
            policy.stop_on_error,
        )
        .await?;
        failures = execution.failures;
        for run in execution.executions {
            executed_runs += 1;
            segmentations_created += run.segmentations_created;
            call_reviews_created += run.call_reviews_created;
            segment_reviews_created += run.segment_reviews_created;
            plans_by_instance.insert(run.instance_id.clone(), run.plan);
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

fn protocol_state_for_run(
    instance_id: &str,
    record_path: &Path,
) -> Result<ProtocolRunState, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let tool_calls_total = record.tool_calls().len();
    let protocol_eligible = tool_calls_total > 0;
    let artifacts = list_protocol_artifacts(record_path)?;
    let artifact_count = artifacts.len();
    let mut call_review_count = 0usize;
    let mut segment_review_count = 0usize;
    let mut segmentation_present = false;
    for artifact in &artifacts {
        match artifact.stored.procedure_name.as_str() {
            "tool_call_intent_segmentation" => segmentation_present = true,
            "tool_call_review" => call_review_count += 1,
            "tool_call_segment_review" => segment_review_count += 1,
            _ => {}
        }
    }

    if !protocol_eligible {
        return Ok(ProtocolRunState {
            instance_id: instance_id.to_string(),
            tool_calls_total,
            protocol_eligible,
            artifact_count,
            segmentation_present,
            call_review_count,
            segment_review_count,
            aggregate_available: false,
            missing_call_indices: Vec::new(),
            missing_segment_indices: Vec::new(),
            next_step: ProtocolNextStep::Ineligible,
            next_command: None,
            aggregate_error: None,
        });
    }

    match load_protocol_aggregate(record_path) {
        Ok(aggregate) => {
            let next_step = if let Some(index) = aggregate.coverage.missing_call_indices.first() {
                ProtocolNextStep::ToolCallReview { index: *index }
            } else if let Some(segment_index) = aggregate.coverage.missing_segment_indices.first() {
                ProtocolNextStep::ToolCallSegmentReview {
                    segment_index: *segment_index,
                }
            } else {
                ProtocolNextStep::Complete
            };
            let next_command =
                protocol_next_command(instance_id, record_path, &next_step, false, false);
            Ok(ProtocolRunState {
                instance_id: instance_id.to_string(),
                tool_calls_total,
                protocol_eligible,
                artifact_count,
                segmentation_present: true,
                call_review_count,
                segment_review_count,
                aggregate_available: true,
                missing_call_indices: aggregate.coverage.missing_call_indices,
                missing_segment_indices: aggregate.coverage.missing_segment_indices,
                next_step,
                next_command,
                aggregate_error: None,
            })
        }
        Err(ProtocolAggregateError::MissingAnchor { .. }) => {
            let next_step = ProtocolNextStep::IntentSegmentation;
            let next_command =
                protocol_next_command(instance_id, record_path, &next_step, false, false);
            Ok(ProtocolRunState {
                instance_id: instance_id.to_string(),
                tool_calls_total,
                protocol_eligible,
                artifact_count,
                segmentation_present,
                call_review_count,
                segment_review_count,
                aggregate_available: false,
                missing_call_indices: Vec::new(),
                missing_segment_indices: Vec::new(),
                next_step,
                next_command,
                aggregate_error: None,
            })
        }
        Err(err) => {
            let next_step = ProtocolNextStep::Blocked;
            let next_command =
                protocol_next_command(instance_id, record_path, &next_step, false, false);
            Ok(ProtocolRunState {
                instance_id: instance_id.to_string(),
                tool_calls_total,
                protocol_eligible,
                artifact_count,
                segmentation_present,
                call_review_count,
                segment_review_count,
                aggregate_available: false,
                missing_call_indices: Vec::new(),
                missing_segment_indices: Vec::new(),
                next_step,
                next_command,
                aggregate_error: Some(err.to_string()),
            })
        }
    }
}

fn protocol_next_command(
    _instance_id: &str,
    _record_path: &Path,
    next_step: &ProtocolNextStep,
    prefer_run_wrapper: bool,
    for_expert_command: bool,
) -> Option<String> {
    match next_step {
        ProtocolNextStep::Ineligible => None,
        ProtocolNextStep::IntentSegmentation => {
            if prefer_run_wrapper {
                Some("ploke-eval protocol run".to_string())
            } else {
                Some("ploke-eval protocol tool-call-intent-segments".to_string())
            }
        }
        ProtocolNextStep::ToolCallReview { index } => {
            if prefer_run_wrapper {
                Some("ploke-eval protocol run".to_string())
            } else {
                Some(format!("ploke-eval protocol tool-call-review {}", index))
            }
        }
        ProtocolNextStep::ToolCallSegmentReview { segment_index } => {
            if prefer_run_wrapper {
                Some("ploke-eval protocol run".to_string())
            } else {
                Some(format!(
                    "ploke-eval protocol tool-call-segment-review {}",
                    segment_index
                ))
            }
        }
        ProtocolNextStep::Complete => None,
        ProtocolNextStep::Blocked => {
            if for_expert_command {
                Some("ploke-eval inspect protocol-artifacts --full".to_string())
            } else {
                None
            }
        }
    }
}

fn print_protocol_state_table(state: &ProtocolRunState) {
    println!("protocol state");
    println!("{}", "-".repeat(40));
    println!("instance: {}", state.instance_id);
    println!(
        "eligible: {}",
        if state.protocol_eligible { "yes" } else { "no" }
    );
    println!("tool calls: {}", state.tool_calls_total);
    println!("protocol artifacts: {}", state.artifact_count);
    println!(
        "segmentation: {}",
        if state.segmentation_present {
            "present"
        } else {
            "(none found)"
        }
    );
    println!("review_calls: {}", state.call_review_count);
    println!("review_segments: {}", state.segment_review_count);
    println!(
        "aggregate overview: {}",
        if state.aggregate_available {
            "available"
        } else {
            "unavailable"
        }
    );
    if !state.missing_call_indices.is_empty() {
        println!(
            "missing call reviews: {}",
            join_indices(&state.missing_call_indices)
        );
    }
    if !state.missing_segment_indices.is_empty() {
        println!(
            "missing segment reviews: {}",
            join_indices(&state.missing_segment_indices)
        );
    }
    if let Some(error) = &state.aggregate_error {
        println!("aggregate error: {}", error);
    }
    if let Some(next_command) = &state.next_command {
        println!();
        println!("next command to advance:");
        println!("  {next_command}");
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
    const MAX_SEGMENTATION_ATTEMPTS: usize = 3;

    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject = build_tool_call_sequence_subject(&record)?;
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
    let segmented = 'retry: loop {
        for attempt in 1..=MAX_SEGMENTATION_ATTEMPTS {
            let protocol = segment::ToolCallIntentSegmentation::new(JsonAdjudicator::new(
                client.clone(),
                cfg.clone(),
            ));
            match protocol.run(subject.clone()).await {
                Ok(segmented) => break 'retry segmented,
                Err(err)
                    if is_retryable_intent_segmentation_error(&err)
                        && attempt < MAX_SEGMENTATION_ATTEMPTS =>
                {
                    eprintln!(
                        "protocol progress: retrying tool_call_intent_segmentation attempt {}/{} after invalid adjudicated segmentation: {}",
                        attempt + 1,
                        MAX_SEGMENTATION_ATTEMPTS,
                        err
                    );
                }
                Err(err) => {
                    return Err(tool_call_intent_segmentation_error_to_prepare(err));
                }
            }
        }
        unreachable!();
    };
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
    let subject = build_tool_call_review_subject(&record, index)?;
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

fn build_issue_detection_artifact(output: &IssueDetectionOutput) -> serde_json::Value {
    serde_json::json!({
        "case_count": output.cases.len(),
        "primary_issue": select_primary_issue(output),
    })
}

fn issue_aggregate_error_to_prepare(err: IssueDetectionAggregateError) -> PrepareError {
    match err {
        IssueDetectionAggregateError::Source(source) => source,
        IssueDetectionAggregateError::MissingArtifact { record_path } => {
            PrepareError::DatabaseSetup {
                phase: "inspect_issue_overview",
                detail: format!(
                    "no persisted issue-detection artifact found for '{}'; run `ploke-eval protocol issue-detection --record {}` first",
                    record_path.display(),
                    record_path.display()
                ),
            }
        }
        IssueDetectionAggregateError::DeserializeOutput { path, detail } => {
            PrepareError::DatabaseSetup {
                phase: "inspect_issue_overview",
                detail: format!(
                    "failed to deserialize issue-detection output from '{}': {}",
                    path.display(),
                    detail
                ),
            }
        }
        IssueDetectionAggregateError::DeserializeInput { path, detail } => {
            PrepareError::DatabaseSetup {
                phase: "inspect_issue_overview",
                detail: format!(
                    "failed to deserialize issue-detection input from '{}': {}",
                    path.display(),
                    detail
                ),
            }
        }
    }
}

fn serde_name<T>(value: &T) -> String
where
    T: Serialize,
{
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "<unknown>".to_string())
}

fn print_issue_case_block(label: &str, issue: &IssueCase) {
    println!("{}:", label);
    println!("  selection_basis: {}", serde_name(&issue.selection_basis));
    println!("  target_tool: {}", issue.target_tool.as_str());
    println!(
        "  target_file: {}",
        issue.target_tool.description_artifact_relpath()
    );
    println!(
        "  evidence: reviewed_calls={} reviewed_issue_calls={}",
        issue.evidence.reviewed_call_count, issue.evidence.reviewed_issue_call_count
    );
    let protocol = &issue.evidence.protocol;
    if !protocol.reviewed_call_indices.is_empty() {
        println!("  reviewed_calls: {:?}", protocol.reviewed_call_indices);
    }
    if !protocol.reviewed_segment_indices.is_empty() {
        println!(
            "  reviewed_segments: {:?}",
            protocol.reviewed_segment_indices
        );
    }
    if !protocol.nearby_segment_labels.is_empty() {
        println!(
            "  nearby_segment_labels: {:?}",
            protocol.nearby_segment_labels
        );
    }
    if !protocol.candidate_concerns.is_empty() {
        println!("  candidate_concerns:");
        for concern in &protocol.candidate_concerns {
            println!("    - {}", concern);
        }
    }
}

fn print_issue_detection_aggregate(aggregate: &IssueDetectionAggregate) {
    println!("issue overview");
    println!("{}", "-".repeat(40));
    println!("Procedure: {}", aggregate.artifact.procedure_name);
    println!("Artifact: {}", aggregate.artifact.path.display());
    println!("Run: {}", aggregate.run.run_id);
    println!("Subject: {}", aggregate.run.subject_id);
    println!("Cases: {}", aggregate.output.cases.len());
    println!(
        "Protocol coverage: total_calls={} anchor_segments={} reviewed_calls={} reviewed_segments={} scanned_artifacts={}",
        aggregate.input.total_calls_in_run,
        aggregate.input.anchor_segment_count,
        aggregate.input.protocol_reviewed_call_count,
        aggregate.input.protocol_reviewed_segment_count,
        aggregate.input.protocol_artifact_count
    );

    if let Some(primary) = &aggregate.primary_issue {
        print_issue_case_block("Primary issue", primary);
    } else {
        println!("Primary issue: (none)");
    }

    if aggregate.output.cases.len() > 1 {
        println!("Other cases:");
        for issue in aggregate.output.cases.iter().skip(1) {
            println!(
                "  - {} ({})",
                issue.target_tool.as_str(),
                serde_name(&issue.selection_basis)
            );
        }
    }
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
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let sequence_subject = build_tool_call_sequence_subject(&record)?;
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

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ProtocolToolCallIntentSegmentsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let subject = build_tool_call_sequence_subject(&record)?;
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

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectConversationsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        if matches!(self.format, InspectOutputFormat::Json) && !self.full {
            return Err(PrepareError::DatabaseSetup {
                phase: "inspect_conversations",
                detail: "refusing to emit full conversation JSON without --full; this output can be very large. Re-run with `--format json --full` and pipe to `jq` or `rg`.".to_string(),
            });
        }

        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
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

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectToolCallsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
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
            print_record_resolution_footer(&resolution);
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

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectToolOverviewCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let report = collect_tool_campaign_overview(&self)?;

        match self.format {
            InspectOutputFormat::Table => print_tool_campaign_overview(&report, self.limit.max(1)),
            InspectOutputFormat::Json => println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
            ),
        }

        Ok(())
    }
}

impl InspectDbSnapshotsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
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

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectFailuresCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
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

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectConfigCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
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

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectOperationalCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;
        let metrics = record.operational_metrics();

        match self.format {
            InspectOutputFormat::Table => {
                println!("Operational Metrics");
                println!("{}", "-".repeat(40));
                println!("Instance ID: {}", record.metadata.benchmark.instance_id);
                println!("Run arm: {}", record.metadata.run_arm.id);
                println!(
                    "Role: {}",
                    format!("{:?}", record.metadata.run_arm.role).to_lowercase()
                );
                println!("Patch apply state: {}", metrics.patch_apply_state.as_str());
                println!(
                    "Submission artifact: {}",
                    metrics.submission_artifact_state.as_str()
                );
                println!("Patch attempted: {}", yes_no(metrics.patch_attempted));
                println!("Aborted: {}", yes_no(metrics.aborted));
                println!(
                    "Aborted repair loop: {}",
                    yes_no(metrics.aborted_repair_loop)
                );
                println!(
                    "Nonempty valid patch: {}",
                    yes_no(metrics.nonempty_valid_patch)
                );
                println!("Convergence: {}", yes_no(metrics.convergence));
                println!("Oracle eligible: {}", yes_no(metrics.oracle_eligible));
                println!();
                println!("Counts");
                println!("{}", "-".repeat(40));
                println!("Tool calls total: {}", metrics.tool_calls_total);
                println!("Tool calls failed: {}", metrics.tool_calls_failed);
                println!("Partial patch failures: {}", metrics.partial_patch_failures);
                println!(
                    "Same-file patch retries: {}",
                    metrics.same_file_patch_retry_count
                );
                println!(
                    "Same-file max streak: {}",
                    metrics.same_file_patch_max_streak
                );
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "instance_id": record.metadata.benchmark.instance_id,
                    "run_arm": {
                        "id": record.metadata.run_arm.id,
                        "role": record.metadata.run_arm.role,
                        "command": record.metadata.run_arm.command,
                        "execution": record.metadata.run_arm.execution,
                    },
                    "metrics": metrics,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectTurnCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
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

        print_record_resolution_footer(&resolution);
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
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
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

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectProtocolArtifactsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
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
            print_record_resolution_footer(&resolution);
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

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl InspectIssueOverviewCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let aggregate = load_issue_detection_aggregate(&record_path)
            .map_err(issue_aggregate_error_to_prepare)?;

        match self.format {
            InspectOutputFormat::Table => {
                print_issue_detection_aggregate(&aggregate);
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&aggregate).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
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

        let resolution = resolve_record_path(self.record.clone(), self.instance.clone(), None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;
        let instance_id = record.metadata.benchmark.instance_id.clone();
        let aggregate = match load_protocol_aggregate(&record_path) {
            Ok(aggregate) => aggregate,
            Err(ProtocolAggregateError::MissingAnchor { .. }) => {
                let mut state = protocol_state_for_run(&instance_id, &record_path)?;
                state.next_command = protocol_next_command(
                    &instance_id,
                    &record_path,
                    &state.next_step,
                    true,
                    false,
                );
                match self.format {
                    InspectOutputFormat::Table => {
                        print_protocol_state_table(&state);
                    }
                    InspectOutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&state)
                                .map_err(PrepareError::Serialize)?
                        );
                    }
                }
                print_record_resolution_footer(&resolution);
                return Ok(());
            }
            Err(err) => {
                let mut state = protocol_state_for_run(&instance_id, &record_path)?;
                state.next_command = protocol_next_command(
                    &instance_id,
                    &record_path,
                    &state.next_step,
                    true,
                    false,
                );
                state.aggregate_error = Some(err.to_string());
                match self.format {
                    InspectOutputFormat::Table => {
                        print_protocol_state_table(&state);
                    }
                    InspectOutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&state)
                                .map_err(PrepareError::Serialize)?
                        );
                    }
                }
                print_record_resolution_footer(&resolution);
                return Ok(());
            }
        };
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

        print_record_resolution_footer(&resolution);
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

fn collect_tool_campaign_overview(
    command: &InspectToolOverviewCommand,
) -> Result<ToolCampaignOverviewReport, PrepareError> {
    let state = load_closure_state(&command.campaign)?;
    let tool_filter = command.tool.as_deref().map(normalize_filter_value);
    let mut report = ToolCampaignOverviewReport {
        campaign_id: command.campaign.clone(),
        tool_filter: command.tool.clone(),
        scanned_complete_runs: 0,
        runs_with_tool: 0,
        runs_with_failed_calls: 0,
        repeated_failure_runs: 0,
        mixed_outcome_runs: 0,
        total_calls: 0,
        completed_calls: 0,
        failed_calls: 0,
        failure_codes: Vec::new(),
        failure_reasons: Vec::new(),
        exemplar_runs: Vec::new(),
        next_steps: Vec::new(),
    };
    let mut failure_code_counts = BTreeMap::<String, (usize, BTreeSet<String>)>::new();
    let mut failure_reason_counts = BTreeMap::<String, (usize, BTreeSet<String>)>::new();
    let mut run_rows = Vec::new();

    for row in state
        .instances
        .iter()
        .filter(|row| row.eval_status == ClosureClass::Complete)
    {
        report.scanned_complete_runs += 1;
        let Some(record_path) = row.artifacts.record_path.as_ref() else {
            continue;
        };
        if !record_path.exists() {
            continue;
        }
        let record =
            read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;
        let mut run = ToolRunAccumulator::default();

        for call in record.tool_calls().into_iter().filter(|call| {
            tool_filter.as_deref().map_or(true, |tool| {
                normalize_filter_value(&call.request.tool) == tool
            })
        }) {
            run.total_calls += 1;
            report.total_calls += 1;
            match call.result {
                crate::record::ToolResult::Completed(_) => {
                    run.completed_calls += 1;
                    report.completed_calls += 1;
                }
                crate::record::ToolResult::Failed(failed) => {
                    run.failed_calls += 1;
                    report.failed_calls += 1;

                    let reason = summarize_failure_reason(&failed.error);
                    *run.failure_reasons.entry(reason.clone()).or_insert(0) += 1;
                    let (count, runs) = failure_reason_counts
                        .entry(reason)
                        .or_insert_with(|| (0usize, BTreeSet::new()));
                    *count += 1;
                    runs.insert(row.instance_id.clone());

                    if let Some(code) = tool_failure_code(&failed) {
                        *run.failure_codes.entry(code.clone()).or_insert(0) += 1;
                        let (count, runs) = failure_code_counts
                            .entry(code)
                            .or_insert_with(|| (0usize, BTreeSet::new()));
                        *count += 1;
                        runs.insert(row.instance_id.clone());
                    }
                }
            }
        }

        if run.total_calls == 0 {
            continue;
        }

        report.runs_with_tool += 1;
        if run.failed_calls > 0 {
            report.runs_with_failed_calls += 1;
        }
        if run.failed_calls >= 2 {
            report.repeated_failure_runs += 1;
        }
        if run.failed_calls > 0 && run.completed_calls > 0 {
            report.mixed_outcome_runs += 1;
        }

        run_rows.push(ToolOverviewRunRow {
            run_id: row.instance_id.clone(),
            total_calls: run.total_calls,
            completed_calls: run.completed_calls,
            failed_calls: run.failed_calls,
            top_failure_code: top_failure_label(&run.failure_codes),
            top_failure_reason: top_failure_label(&run.failure_reasons),
        });
    }

    report.failure_codes = count_rows_from_map(failure_code_counts)
        .into_iter()
        .map(|row| ToolOverviewCountRow {
            label: row.label,
            count: row.count,
            affected_runs: row.affected_runs,
        })
        .collect();
    report.failure_reasons = count_rows_from_map(failure_reason_counts)
        .into_iter()
        .map(|row| ToolOverviewCountRow {
            label: row.label,
            count: row.count,
            affected_runs: row.affected_runs,
        })
        .collect();
    run_rows.sort_by(|left, right| {
        right
            .failed_calls
            .cmp(&left.failed_calls)
            .then_with(|| right.total_calls.cmp(&left.total_calls))
            .then_with(|| left.run_id.cmp(&right.run_id))
    });
    run_rows.truncate(command.limit.max(3));
    report.exemplar_runs = run_rows;
    report.failure_codes.truncate(command.limit.max(3));
    report.failure_reasons.truncate(command.limit.max(3));
    report.next_steps = build_tool_overview_next_steps(&report);

    Ok(report)
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

fn build_tool_overview_next_steps(report: &ToolCampaignOverviewReport) -> Vec<String> {
    let tool = report
        .tool_filter
        .clone()
        .unwrap_or_else(|| "apply_code_edit".to_string());
    let mut steps = Vec::new();
    if let Some(run) = report.exemplar_runs.first() {
        steps.push(format!(
            "inspect the worst exemplar with `ploke-eval inspect tool-calls --instance {}`",
            run.run_id
        ));
    }
    steps.push(format!(
        "compare protocol issue context with `ploke-eval inspect protocol-overview --campaign {} --tool {}`",
        report.campaign_id, tool
    ));
    steps
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

#[derive(Debug, Clone, Serialize)]
struct ToolCampaignOverviewReport {
    campaign_id: String,
    tool_filter: Option<String>,
    scanned_complete_runs: usize,
    runs_with_tool: usize,
    runs_with_failed_calls: usize,
    repeated_failure_runs: usize,
    mixed_outcome_runs: usize,
    total_calls: usize,
    completed_calls: usize,
    failed_calls: usize,
    failure_codes: Vec<ToolOverviewCountRow>,
    failure_reasons: Vec<ToolOverviewCountRow>,
    exemplar_runs: Vec<ToolOverviewRunRow>,
    next_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolOverviewCountRow {
    label: String,
    count: usize,
    affected_runs: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ToolOverviewRunRow {
    run_id: String,
    total_calls: usize,
    completed_calls: usize,
    failed_calls: usize,
    top_failure_code: Option<String>,
    top_failure_reason: Option<String>,
}

#[derive(Debug, Default)]
struct ToolRunAccumulator {
    total_calls: usize,
    completed_calls: usize,
    failed_calls: usize,
    failure_codes: BTreeMap<String, usize>,
    failure_reasons: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
struct SegmentEvidenceCounts {
    usable: usize,
    mismatched: usize,
    missing: usize,
    missing_indices: Vec<usize>,
}

fn collect_protocol_run_summaries() -> Result<Vec<ProtocolRunSummaryRow>, PrepareError> {
    let total_start = Instant::now();
    let mut summaries = Vec::new();
    for record_path in collect_finished_record_paths()? {
        let run_start = Instant::now();
        let summary = collect_protocol_run_summary_record(&record_path)?.summary;
        let elapsed = run_start.elapsed();
        if elapsed.as_millis() >= 200 {
            eprintln!(
                "protocol-overview: slow run {} took {} ms",
                summary.run_id,
                elapsed.as_millis()
            );
        }
        summaries.push(summary);
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
    eprintln!(
        "protocol-overview: scanned {} runs in {:.2}s",
        summaries.len(),
        total_start.elapsed().as_secs_f32()
    );
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
    let root = instances_dir()?;
    list_finished_record_paths_in_instances_root(&root)
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
                label: basis.label.map(intent_label_name),
                call_span: Some(format!("{}..{}", basis.start_index, basis.end_index)),
                status: Some(
                    segment_review
                        .map(|review| review.overall.clone())
                        .unwrap_or_else(|| segment_status_name(basis.status).to_string()),
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
                    .or_else(|| basis.confidence.map(confidence_fraction_typed)),
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
        "Protocol status: full=all expected protocol reviews present; partial=review coverage missing; missing=no segmentation anchor; error=artifact/schema failure; ineligible=zero tool calls"
    );
    println!(
        "{:<28} {:<10} {:<12} {:<18} {:<8} Summary",
        "Run", "Call revs", "Protocol", "Segment evidence", "Issues"
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

fn print_tool_campaign_overview(report: &ToolCampaignOverviewReport, limit: usize) {
    println!("Campaign: {}", report.campaign_id);
    println!(
        "Tool: {}",
        report.tool_filter.as_deref().unwrap_or("<all tools>")
    );
    println!(
        "Runs: {} complete scanned | {} with tool",
        report.scanned_complete_runs, report.runs_with_tool
    );
    println!(
        "Calls: {} total | {} completed | {} failed",
        report.total_calls, report.completed_calls, report.failed_calls
    );
    println!(
        "Run outcomes: {} with failures | {} repeated-failure runs | {} mixed-outcome runs",
        report.runs_with_failed_calls, report.repeated_failure_runs, report.mixed_outcome_runs
    );

    if !report.failure_codes.is_empty() {
        println!("\nTop failure codes");
        for row in report.failure_codes.iter().take(limit) {
            println!(
                "  - {}: {} calls across {} runs",
                row.label, row.count, row.affected_runs
            );
        }
    }

    if !report.failure_reasons.is_empty() {
        println!("\nTop failure reasons");
        for row in report.failure_reasons.iter().take(limit) {
            println!(
                "  - {}: {} calls across {} runs",
                row.label, row.count, row.affected_runs
            );
        }
    }

    if !report.exemplar_runs.is_empty() {
        println!("\nExemplar runs");
        for row in report.exemplar_runs.iter().take(limit) {
            let code = row.top_failure_code.as_deref().unwrap_or("-");
            let reason = row.top_failure_reason.as_deref().unwrap_or("-");
            println!(
                "  - {}: {} calls | {} completed | {} failed | top code {} | {}",
                row.run_id, row.total_calls, row.completed_calls, row.failed_calls, code, reason
            );
        }
    }

    if !report.next_steps.is_empty() {
        println!("\nNext");
        for step in report.next_steps.iter().take(4) {
            println!("  - {}", step);
        }
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

fn confidence_fraction_typed(value: ploke_protocol::Confidence) -> f32 {
    match value {
        ploke_protocol::Confidence::High => 0.9,
        ploke_protocol::Confidence::Medium => 0.6,
        ploke_protocol::Confidence::Low => 0.3,
    }
}

fn intent_label_name(label: ploke_protocol::IntentLabel) -> String {
    match label {
        ploke_protocol::IntentLabel::LocateTarget => "locate_target".to_string(),
        ploke_protocol::IntentLabel::InspectCandidate => "inspect_candidate".to_string(),
        ploke_protocol::IntentLabel::RefineSearch => "refine_search".to_string(),
        ploke_protocol::IntentLabel::ValidateHypothesis => "validate_hypothesis".to_string(),
        ploke_protocol::IntentLabel::EditAttempt => "edit_attempt".to_string(),
        ploke_protocol::IntentLabel::Recovery => "recovery".to_string(),
        ploke_protocol::IntentLabel::Other => "other".to_string(),
    }
}

fn segment_status_name(status: ploke_protocol::SegmentStatus) -> &'static str {
    match status {
        ploke_protocol::SegmentStatus::Labeled => "labeled",
        ploke_protocol::SegmentStatus::Ambiguous => "ambiguous",
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
) -> Result<trace::ToolCallSequence, PrepareError> {
    let subject_id = record.metadata.benchmark.instance_id.clone();
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
    index: usize,
) -> Result<trace::ToolCallNeighborhood, PrepareError> {
    let subject_id = record.metadata.benchmark.instance_id.clone();
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

fn is_retryable_intent_segmentation_error(err: &segment::IntentSegmentationError) -> bool {
    matches!(
        err,
        segment::IntentSegmentationError::Second(ploke_protocol::MergeError::Join(
            segment::NormalizeSegmentsError::Overlap { .. }
                | segment::NormalizeSegmentsError::InvalidRange { .. }
                | segment::NormalizeSegmentsError::MissingLabel { .. }
                | segment::NormalizeSegmentsError::AmbiguousWithLabel { .. }
        ))
    )
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
    println!("Parsed Inputs (convenience view from stored raw arguments)");
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
        println!("Stored Raw Arguments");
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
                println!("UI Summary (convenience only): {}", ui_payload.summary);
                for field in &ui_payload.fields {
                    println!("ui.{}: {}", field.name, prettify_field_value(&field.value));
                }
                if let Some(details) = &ui_payload.details {
                    let rendered = render_payload_block(details, if full { 1200 } else { 220 });
                    println!("UI Details (convenience only):");
                    println!("{}", rendered.text);
                    if let Some(note) = rendered.inspector_truncation_note() {
                        println!("{}", note);
                    }
                }
                println!();
            }
            println!("Stored Raw Output:");
            if completed.content.is_empty() {
                println!("(empty)");
            } else {
                let rendered =
                    render_payload_block(&completed.content, if full { 2400 } else { 220 });
                println!("{}", rendered.text);
                if let Some(note) = summarize_tool_native_truncation(&completed.content) {
                    println!("{}", note);
                }
                if let Some(note) = rendered.inspector_truncation_note() {
                    println!("{}", note);
                }
                println!("Stored raw bytes: {}", rendered.raw_bytes);
            }
        }
        crate::record::ToolResult::Failed(failed) => {
            if let Some(ui_payload) = &failed.ui_payload {
                println!("UI Summary (convenience only): {}", ui_payload.summary);
                for field in &ui_payload.fields {
                    println!("ui.{}: {}", field.name, prettify_field_value(&field.value));
                }
                println!();
            }
            println!("Stored Raw Error:");
            if failed.error.is_empty() {
                println!("(empty)");
            } else {
                let rendered = render_payload_block(&failed.error, if full { 2400 } else { 220 });
                println!("{}", rendered.text);
                if let Some(note) = rendered.inspector_truncation_note() {
                    println!("{}", note);
                }
                println!("Stored raw bytes: {}", rendered.raw_bytes);
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
            render_payload_block(arguments, 220).text
        )];
    };

    match value {
        serde_json::Value::Object(map) => render_tool_input_map(&map),
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
    let mut used = BTreeSet::new();
    if let Some(line_window) = summarize_line_window(object) {
        parts.push(format!("lines={line_window}"));
        used.insert("start_line");
        used.insert("end_line");
    }
    for key in preferred {
        if used.contains(key) {
            continue;
        }
        if let Some(value) = object.get(key) {
            parts.push(format!(
                "{}={}",
                key,
                summarize_json_value(key, value, false)
            ));
            used.insert(key);
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

fn render_tool_input_map(map: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let mut lines = Vec::new();
    let mut rendered = BTreeSet::new();

    if let Some(line_window) = summarize_line_window(map) {
        lines.push(format!(
            "lines (derived from start_line/end_line): {}",
            line_window
        ));
        rendered.insert("start_line");
        rendered.insert("end_line");
    }

    for key in [
        "file",
        "file_path",
        "dir",
        "path",
        "start_line",
        "end_line",
        "search_term",
        "canon",
        "symbol",
        "name",
        "query",
        "command",
        "patches",
        "confidence",
    ] {
        if rendered.contains(key) {
            continue;
        }
        if let Some(value) = map.get(key) {
            lines.push(format!(
                "{}: {}",
                key,
                summarize_json_value(key, value, true)
            ));
            rendered.insert(key);
        }
    }

    for (key, value) in map {
        if rendered.contains(key.as_str()) {
            continue;
        }
        lines.push(format!(
            "{}: {}",
            key,
            summarize_json_value(key, value, true)
        ));
    }

    lines
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

fn summarize_failure_reason(error: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(error) {
        for key in ["user", "summary", "message", "error"] {
            if let Some(text) = value.get(key).and_then(|value| value.as_str()) {
                let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !normalized.is_empty() {
                    return truncate_middle(&normalized, 96);
                }
            }
        }
    }
    let normalized = error
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| "-".to_string());
    truncate_middle(&normalized, 96)
}

fn tool_failure_code(failed: &crate::runner::ToolFailedRecord) -> Option<String> {
    failed
        .ui_payload
        .as_ref()
        .and_then(|payload| payload.error_code)
        .map(tool_error_code_label)
        .map(str::to_string)
}

fn top_failure_label(counts: &BTreeMap<String, usize>) -> Option<String> {
    counts
        .iter()
        .max_by(|(left_label, left_count), (right_label, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| right_label.cmp(left_label))
        })
        .map(|(label, _)| label.clone())
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

fn summarize_line_window(object: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let start = object
        .get("start_line")
        .map(compact_json_scalar)
        .filter(|value| !value.is_empty());
    let end = object
        .get("end_line")
        .map(compact_json_scalar)
        .filter(|value| !value.is_empty());

    match (start, end) {
        (Some(start), Some(end)) => Some(format!("{start}-{end}")),
        (Some(start), None) => Some(format!("{start}-EOF")),
        (None, Some(end)) => Some(format!("1-{end}")),
        (None, None) => None,
    }
}

fn compact_json_scalar(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedPayloadBlock {
    text: String,
    raw_bytes: usize,
    normalized_chars: usize,
    shown_source_chars: usize,
    inspector_truncated: bool,
}

impl RenderedPayloadBlock {
    fn inspector_truncation_note(&self) -> Option<String> {
        if !self.inspector_truncated {
            return None;
        }

        let omitted = self
            .normalized_chars
            .saturating_sub(self.shown_source_chars);
        Some(format!(
            "Inspector display truncated the normalized payload: {}/{} source chars shown (+ ellipsis), {} elided; stored raw payload is {} bytes.",
            self.shown_source_chars, self.normalized_chars, omitted, self.raw_bytes
        ))
    }
}

fn summarize_tool_native_truncation(content: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(content).ok()?;
    let object = parsed.as_object()?;
    let truncated = object.get("truncated")?.as_bool()?;
    if !truncated {
        return None;
    }

    let source_bytes = object.get("byte_len")?.as_u64()?;
    let retained_bytes = object
        .get("content")
        .and_then(|value| value.as_str())
        .map(|text| text.len() as u64)
        .unwrap_or(0);
    let omitted_bytes = source_bytes.saturating_sub(retained_bytes);

    Some(format!(
        "Tool-reported truncation: {} / {} source bytes retained, {} omitted before inspector display.",
        retained_bytes, source_bytes, omitted_bytes
    ))
}

fn render_payload_block(text: &str, max_len: usize) -> RenderedPayloadBlock {
    let trimmed = text.trim();
    let rendered = serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| trimmed.to_string());

    let normalized_chars = rendered.chars().count();
    let inspector_truncated = normalized_chars > max_len;
    let shown_source_chars = if inspector_truncated {
        max_len.saturating_sub(3).min(normalized_chars)
    } else {
        normalized_chars
    };

    RenderedPayloadBlock {
        text: truncate_middle(&rendered, max_len),
        raw_bytes: trimmed.len(),
        normalized_chars,
        shown_source_chars,
        inspector_truncated,
    }
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

#[derive(Debug, Clone)]
struct RecordResolution {
    record_path: PathBuf,
    footer: Option<String>,
    warnings: Vec<String>,
}

fn has_legacy_instance_root_artifacts(instances_root: &Path, instance_id: &str) -> bool {
    let instance_root = instances_root.join(instance_id);
    [
        "record.json.gz",
        "execution-log.json",
        "repo-state.json",
        "indexing-status.json",
        "snapshot-status.json",
        "final-snapshot.db",
        "agent-turn-summary.json",
        "agent-turn-trace.json",
        "llm-full-responses.jsonl",
        "multi-swe-bench-submission.jsonl",
        "protocol-artifacts",
    ]
    .iter()
    .any(|name| instance_root.join(name).exists())
}

fn legacy_instance_root_warning(instance_id: &str) -> String {
    format!(
        "instance {instance_id} still has legacy top-level run artifacts under ~/.ploke-eval/instances/{instance_id}; authoritative attempt data lives under runs/run-* and is selected from registrations"
    )
}

#[derive(Debug, Clone, Serialize)]
struct RunListRow {
    attempt: u32,
    latest: bool,
    execution_status: crate::run_registry::RunExecutionStatus,
    submission_status: crate::run_registry::RunSubmissionStatus,
    run_arm_id: String,
    model_id: Option<String>,
    provider_slug: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    run_root: PathBuf,
}

fn resolve_record_path(
    record: Option<PathBuf>,
    instance: Option<String>,
    attempt: Option<u32>,
) -> Result<RecordResolution, PrepareError> {
    resolve_record_path_from_eval_home(record, instance, attempt, crate::layout::ploke_eval_home()?)
}

fn resolve_record_path_from_eval_home(
    record: Option<PathBuf>,
    instance: Option<String>,
    attempt: Option<u32>,
    eval_home: PathBuf,
) -> Result<RecordResolution, PrepareError> {
    let instances_root = crate::layout::instances_dir()?;
    let selection = load_active_selection_at(&eval_home)?;
    match (record, instance) {
        (Some(path), None) => Ok(RecordResolution {
            record_path: path,
            footer: None,
            warnings: Vec::new(),
        }),
        (None, explicit_instance) => {
            let resolved_instance = explicit_instance.or_else(|| selection.instance.clone());
            let (resolved_attempt, attempt_warning) =
                resolve_attempt_override(&selection, resolved_instance.as_deref(), attempt);
            if let Some(instance_id) = resolved_instance {
                let mut warnings = render_selection_warnings(&selection_with_resolution(
                    &selection,
                    Some(&instance_id),
                    resolved_attempt,
                ));
                if let Some(warning) = attempt_warning {
                    warnings.push(warning);
                }
                return resolve_instance_record_path(
                    &instances_root,
                    &instance_id,
                    resolved_attempt,
                    warnings,
                );
            }
            if resolved_attempt.is_some() {
                return Err(PrepareError::DatabaseSetup {
                    phase: "resolve_record_path",
                    detail: "an active attempt selection requires an active or explicit instance"
                        .to_string(),
                });
            }
            let last_run = crate::run_history::load_last_run_at(&eval_home)?;
            let record_path = last_run.run_dir.join("record.json.gz");
            let footer =
                match crate::run_registry::load_registration_for_run_dir(&last_run.run_dir)? {
                    Some(registration) => {
                        let attempt = attempt_number_for_registration(
                            &instances_root,
                            &registration.frozen_spec.task_id,
                            &registration.run_id,
                        )?;
                        Some(format!(
                            "resolved run: {} attempt {} (latest)",
                            registration.frozen_spec.task_id, attempt
                        ))
                    }
                    None => Some("resolved run: most recent completed run".to_string()),
                };
            Ok(RecordResolution {
                record_path,
                footer,
                warnings: render_selection_warnings(&selection),
            })
        }
        (Some(_), Some(_)) => Err(PrepareError::MissingRunManifest(
            instances_root.join("<instance>/runs/run-*/record.json.gz"),
        )),
    }
}

fn resolve_attempt_override(
    selection: &ActiveSelection,
    resolved_instance: Option<&str>,
    explicit_attempt: Option<u32>,
) -> (Option<u32>, Option<String>) {
    if explicit_attempt.is_some() {
        return (explicit_attempt, None);
    }
    let Some(selected_attempt) = selection.attempt else {
        return (None, None);
    };
    let Some(selected_instance) = selection.instance.as_deref() else {
        return (
            None,
            Some("selected attempt was ignored because no selected instance is active".to_string()),
        );
    };
    match resolved_instance {
        Some(instance) if instance == selected_instance => (Some(selected_attempt), None),
        Some(instance) => (
            None,
            Some(format!(
                "selected attempt {} for instance {} was ignored because instance {} was requested",
                selected_attempt, selected_instance, instance
            )),
        ),
        None => (None, None),
    }
}

fn selection_with_resolution(
    selection: &ActiveSelection,
    instance: Option<&str>,
    attempt: Option<u32>,
) -> ActiveSelection {
    let mut resolved = selection.clone();
    if let Some(instance) = instance {
        resolved.instance = Some(instance.to_string());
        resolved.attempt = attempt;
    }
    resolved
}

fn resolve_instance_record_path(
    instances_root: &Path,
    instance_id: &str,
    attempt: Option<u32>,
    mut warnings: Vec<String>,
) -> Result<RecordResolution, PrepareError> {
    let registrations = list_attempt_registrations(instances_root, instance_id)?;
    if !registrations.is_empty() {
        if has_legacy_instance_root_artifacts(instances_root, instance_id) {
            warnings.push(legacy_instance_root_warning(instance_id));
        }
        let selected_index = match attempt {
            Some(number) if number > 0 => {
                let index = (number - 1) as usize;
                if index >= registrations.len() {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "resolve_record_path",
                        detail: format!(
                            "instance {instance_id} has {} attempt(s); attempt {} is out of range",
                            registrations.len(),
                            number
                        ),
                    });
                }
                index
            }
            Some(_) => {
                return Err(PrepareError::DatabaseSetup {
                    phase: "resolve_record_path",
                    detail: "attempt numbers are 1-based".to_string(),
                });
            }
            None => registrations.len() - 1,
        };
        let selected = &registrations[selected_index];
        return Ok(RecordResolution {
            record_path: selected.artifacts.record_path.clone(),
            footer: attempt.is_none().then(|| {
                format!(
                    "resolved run: {} attempt {} (latest)",
                    instance_id,
                    selected_index + 1
                )
            }),
            warnings,
        });
    }

    if attempt.is_some() {
        return Err(PrepareError::DatabaseSetup {
            phase: "resolve_record_path",
            detail: format!(
                "instance {instance_id} has no registered attempts; cannot resolve a numbered attempt"
            ),
        });
    }

    Err(PrepareError::DatabaseSetup {
        phase: "resolve_record_path",
        detail: format!(
            "instance {instance_id} has no registered attempts; legacy instance-root artifacts are no longer used"
        ),
    })
}

fn list_attempt_registrations(
    instances_root: &Path,
    instance_id: &str,
) -> Result<Vec<RunRegistration>, PrepareError> {
    let mut registrations = list_registrations_for_instance(instances_root, instance_id)?;
    registrations.sort_by(|left, right| {
        run_registration_sort_key(left).cmp(&run_registration_sort_key(right))
    });
    Ok(registrations)
}

fn attempt_number_for_registration(
    instances_root: &Path,
    instance_id: &str,
    run_id: &str,
) -> Result<usize, PrepareError> {
    let registrations = list_attempt_registrations(instances_root, instance_id)?;
    registrations
        .iter()
        .position(|registration| registration.run_id == run_id)
        .map(|index| index + 1)
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "resolve_record_path",
            detail: format!("run {run_id} is not registered under instance {instance_id}"),
        })
}

fn run_registration_sort_key(registration: &RunRegistration) -> (String, String) {
    (
        registration
            .lifecycle
            .finished_at
            .clone()
            .unwrap_or_else(|| registration.lifecycle.updated_at.clone()),
        registration.run_id.clone(),
    )
}

fn print_record_resolution_footer(resolution: &RecordResolution) {
    let _ = std::io::stdout().flush();
    for warning in &resolution.warnings {
        eprintln!("warning: {warning}");
    }
    if let Some(footer) = &resolution.footer {
        eprintln!("{footer}");
    }
}

fn print_selection_update(selection: &ActiveSelection) {
    println!(
        "campaign: {}",
        selection.campaign.as_deref().unwrap_or("(none)")
    );
    println!("batch: {}", selection.batch.as_deref().unwrap_or("(none)"));
    println!(
        "instance: {}",
        selection.instance.as_deref().unwrap_or("(none)")
    );
    println!(
        "attempt: {}",
        selection
            .attempt
            .map(|attempt| attempt.to_string())
            .unwrap_or_else(|| "(latest)".to_string())
    );
    for warning in render_selection_warnings(selection) {
        println!("warning: {warning}");
    }
}

fn execution_status_label(status: crate::run_registry::RunExecutionStatus) -> &'static str {
    match status {
        crate::run_registry::RunExecutionStatus::Registered => "registered",
        crate::run_registry::RunExecutionStatus::Running => "running",
        crate::run_registry::RunExecutionStatus::Completed => "completed",
        crate::run_registry::RunExecutionStatus::Failed => "failed",
    }
}

fn submission_status_label(status: crate::run_registry::RunSubmissionStatus) -> &'static str {
    match status {
        crate::run_registry::RunSubmissionStatus::Missing => "missing",
        crate::run_registry::RunSubmissionStatus::EmptyPatch => "empty_patch",
        crate::run_registry::RunSubmissionStatus::NonemptyPatch => "nonempty_patch",
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
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
        instances_dir()?,
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

fn registry_dataset_view<'a>(
    registry: &'a TargetRegistry,
    dataset: &'a str,
) -> Result<RegistryDatasetView<'a>, PrepareError> {
    let entries: Vec<_> = registry
        .entries
        .iter()
        .filter(|entry| entry.dataset_label == dataset)
        .collect();
    if entries.is_empty() {
        let available = registry
            .entries
            .iter()
            .map(|entry| entry.dataset_label.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "registry dataset '{}' not found; available datasets: {}",
                dataset, available
            ),
        });
    }

    let source_paths = registry
        .dataset_sources
        .iter()
        .filter(|source| source.label == dataset || source.key.as_deref() == Some(dataset))
        .map(|source| source.path.clone())
        .collect();

    Ok(RegistryDatasetView {
        dataset,
        state_path: target_registry_path(registry.benchmark_family)?,
        source_paths,
        entries,
    })
}

fn print_registry_dataset_view(view: &RegistryDatasetView<'_>) {
    println!("dataset: {}", view.dataset);
    println!("instances: {}", view.entries.len());
    println!("registry: {}", view.state_path.display());
    if !view.source_paths.is_empty() {
        println!("sources:");
        for path in &view.source_paths {
            println!("  {}", path.display());
        }
    }
    println!();
    println!("instance ids:");
    for entry in &view.entries {
        println!("  {}", entry.instance_id);
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
    use crate::inner::core::{RegisteredRunRole, RunIntent, RunStorageRoots};
    use crate::inner::registry::RunRegistration;
    use crate::record::read_compressed_record;
    use crate::run_registry::RunExecutionStatus;
    use ploke_core::ArcStr;
    use ploke_tui::chat_history::{MessageKind, MessageStatus};
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;
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

    fn write_test_run_record(path: &Path, run_arm: crate::runner::RunArm) {
        let prepared = crate::spec::PreparedSingleRun {
            task_id: "org__repo-1".to_string(),
            repo_root: PathBuf::from("/tmp/repo"),
            output_dir: PathBuf::from("/tmp/output"),
            issue: crate::spec::IssueInput {
                title: None,
                body: None,
                body_path: None,
            },
            base_sha: None,
            head_sha: None,
            budget: crate::spec::EvalBudget::default(),
            source: None,
            campaign: None,
        };
        let record = crate::record::RunRecord::new(&prepared, run_arm);
        crate::record::write_compressed_record(path, &record).expect("write record");
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn hold_env_lock() -> std::sync::MutexGuard<'static, ()> {
        env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn sample_run_intent(base: &Path, instances_root: &Path) -> RunIntent {
        RunIntent {
            task_id: "org__repo-1".to_string(),
            repo_root: base.join("repo"),
            storage_roots: RunStorageRoots::new(
                base.join("registries"),
                instances_root.join("org__repo-1").join("runs"),
            ),
            base_sha: Some("deadbeef".to_string()),
            budget: crate::spec::EvalBudget::default(),
            model_id: Some("model".to_string()),
            provider_slug: Some("provider".to_string()),
            campaign_id: None,
            batch_id: None,
            run_arm_id: "structured-current-policy".to_string(),
            run_role: RegisteredRunRole::Treatment,
        }
    }

    fn register_attempt(
        eval_home: &Path,
        run_id: &str,
        run_arm: crate::runner::RunArm,
        finished_at: &str,
    ) -> RunRegistration {
        let instances_root = eval_home.join("instances");
        let mut intent = sample_run_intent(eval_home, &instances_root);
        intent.run_arm_id = run_arm.id.clone();
        intent.run_role = match run_arm.role {
            crate::runner::RunArmRole::Control => RegisteredRunRole::Control,
            crate::runner::RunArmRole::Treatment => RegisteredRunRole::Treatment,
        };
        let mut registration =
            RunRegistration::register_with_run_id(intent, run_id).expect("registration");
        registration.lifecycle.execution_status = RunExecutionStatus::Completed;
        registration.lifecycle.finished_at = Some(finished_at.to_string());
        std::fs::create_dir_all(&registration.artifacts.run_root).expect("run root");
        write_test_run_record(&registration.artifacts.record_path, run_arm);
        registration.persist().expect("persist registration");
        registration
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

    #[test]
    #[ignore = "diagnostic prompt dump for tool-call intent segmentation context"]
    fn diagnostic_dump_tool_call_segmentation_prompt() {
        let record_path = PathBuf::from(
            "/home/brasides/.ploke-eval/instances/prototype1/prototype1-typed-bridge-test-1777120923237/treatments/branch-c766961d14708d45/instances/clap-rs__clap-3670/runs/run-1777136748312-structured-current-policy-a9cd20b3/record.json.gz",
        );
        let record = read_compressed_record(&record_path).expect("read diagnostic record");
        let sequence =
            build_tool_call_sequence_subject(&record).expect("build tool-call sequence subject");
        let context = ploke_protocol::SequenceReviewContext {
            sequence,
            signals: ploke_protocol::tool_calls::segment::derive_sequence_signals_for_diagnostics(
                &build_tool_call_sequence_subject(&record)
                    .expect("rebuild tool-call sequence subject"),
            ),
        };
        let rendered =
            ploke_protocol::tool_calls::segment::render_sequence_context_for_diagnostics(&context);

        eprintln!("SEGMENT_DIAG record_path={}", record_path.display());
        eprintln!("SEGMENT_DIAG rendered_chars={}", rendered.len());
        eprintln!("SEGMENT_DIAG total_calls={}", context.sequence.calls.len());
        for call in &context.sequence.calls {
            eprintln!(
                "SEGMENT_DIAG call={} tool={} kind={:?} failed={} summary_len={} args_len={} result_len={} search_term_len={} path_hint_len={}",
                call.index,
                call.tool_name,
                call.tool_kind,
                call.failed,
                call.summary.len(),
                call.args_preview.len(),
                call.result_preview.len(),
                call.search_term
                    .as_ref()
                    .map(|value| value.len())
                    .unwrap_or(0),
                call.path_hint
                    .as_ref()
                    .map(|value| value.len())
                    .unwrap_or(0),
            );
            eprintln!("SEGMENT_DIAG summary[{}]={}", call.index, call.summary);
            eprintln!("SEGMENT_DIAG args[{}]={}", call.index, call.args_preview);
            eprintln!(
                "SEGMENT_DIAG result[{}]={}",
                call.index, call.result_preview
            );
        }
        eprintln!("SEGMENT_DIAG rendered_begin");
        eprintln!("{rendered}");
        eprintln!("SEGMENT_DIAG rendered_end");
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
    fn run_single_agent_path_parses_under_run_tree() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "run",
            "single",
            "agent",
            "--instance",
            "BurntSushi__ripgrep-2209",
        ])
        .expect("run single agent should parse");

        match parsed.command {
            Command::Run(RunCommand {
                command:
                    RunSubcommand::Single(RunSingleWorkflowCommand {
                        command: RunSingleWorkflowSubcommand::Agent(cmd),
                    }),
            }) => assert_eq!(cmd.instance.as_deref(), Some("BurntSushi__ripgrep-2209")),
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn just_single_shortcut_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "just",
            "single",
            "--instance",
            "BurntSushi__ripgrep-2209",
        ])
        .expect("just single should parse");

        match parsed.command {
            Command::Just(JustCommand {
                command: JustSubcommand::Single(cmd),
            }) => assert_eq!(cmd.instance.as_deref(), Some("BurntSushi__ripgrep-2209")),
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn just_old_hyphenated_alias_still_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "just",
            "run-msb-agent-single",
            "--instance",
            "BurntSushi__ripgrep-2209",
        ])
        .expect("just should keep the old hyphenated alias as a migration path");

        match parsed.command {
            Command::Just(JustCommand {
                command: JustSubcommand::Single(cmd),
            }) => assert_eq!(cmd.instance.as_deref(), Some("BurntSushi__ripgrep-2209")),
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn run_list_parses_instance_selector() {
        let parsed =
            Cli::try_parse_from(["ploke-eval", "run", "list", "--instance", "sharkdp__fd-658"])
                .expect("run list should parse");

        match parsed.command {
            Command::Run(RunCommand {
                command: RunSubcommand::List(cmd),
            }) => assert_eq!(cmd.instance.as_deref(), Some("sharkdp__fd-658")),
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn select_attempt_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "select",
            "attempt",
            "3",
            "--instance",
            "sharkdp__fd-658",
        ])
        .expect("select attempt should parse");

        match parsed.command {
            Command::Select(SelectCommand {
                command: SelectSubcommand::Attempt(cmd),
            }) => {
                assert_eq!(cmd.attempt, 3);
                assert_eq!(cmd.instance.as_deref(), Some("sharkdp__fd-658"));
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn registry_show_parses_dataset_selector() {
        let parsed =
            Cli::try_parse_from(["ploke-eval", "registry", "show", "--dataset", "sharkdp__fd"])
                .expect("registry show should parse");

        match parsed.command {
            Command::Registry(RegistryCommand {
                command: RegistrySubcommand::Show(cmd),
            }) => assert_eq!(cmd.dataset, "sharkdp__fd"),
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    fn sample_target_registry() -> TargetRegistry {
        TargetRegistry {
            schema_version: crate::target_registry::TARGET_REGISTRY_SCHEMA_VERSION.to_string(),
            benchmark_family: BenchmarkFamily::MultiSweBenchRust,
            updated_at: "2026-04-21T00:00:00Z".to_string(),
            dataset_sources: vec![
                crate::target_registry::RegistryDatasetSource {
                    key: Some("fd".to_string()),
                    path: PathBuf::from("/tmp/sharkdp__fd_dataset.jsonl"),
                    label: "sharkdp__fd".to_string(),
                    url: Some("https://example.invalid/fd".to_string()),
                },
                crate::target_registry::RegistryDatasetSource {
                    key: Some("ripgrep".to_string()),
                    path: PathBuf::from("/tmp/BurntSushi__ripgrep_dataset.jsonl"),
                    label: "BurntSushi__ripgrep".to_string(),
                    url: Some("https://example.invalid/ripgrep".to_string()),
                },
            ],
            entries: vec![
                RegistryEntry {
                    instance_id: "sharkdp__fd-497".to_string(),
                    dataset_label: "sharkdp__fd".to_string(),
                    repo_family: "sharkdp__fd".to_string(),
                    source: crate::target_registry::RegistrySource {
                        dataset_path: PathBuf::from("/tmp/sharkdp__fd_dataset.jsonl"),
                        org: "sharkdp".to_string(),
                        repo: "fd".to_string(),
                        number: 497,
                        base_sha: "abc123".to_string(),
                    },
                    state: crate::target_registry::RegistryEntryState::Active,
                },
                RegistryEntry {
                    instance_id: "sharkdp__fd-658".to_string(),
                    dataset_label: "sharkdp__fd".to_string(),
                    repo_family: "sharkdp__fd".to_string(),
                    source: crate::target_registry::RegistrySource {
                        dataset_path: PathBuf::from("/tmp/sharkdp__fd_dataset.jsonl"),
                        org: "sharkdp".to_string(),
                        repo: "fd".to_string(),
                        number: 658,
                        base_sha: "def456".to_string(),
                    },
                    state: crate::target_registry::RegistryEntryState::Active,
                },
                RegistryEntry {
                    instance_id: "BurntSushi__ripgrep-2209".to_string(),
                    dataset_label: "BurntSushi__ripgrep".to_string(),
                    repo_family: "BurntSushi__ripgrep".to_string(),
                    source: crate::target_registry::RegistrySource {
                        dataset_path: PathBuf::from("/tmp/BurntSushi__ripgrep_dataset.jsonl"),
                        org: "BurntSushi".to_string(),
                        repo: "ripgrep".to_string(),
                        number: 2209,
                        base_sha: "fedcba".to_string(),
                    },
                    state: crate::target_registry::RegistryEntryState::Active,
                },
            ],
        }
    }

    #[test]
    fn registry_dataset_view_filters_to_one_dataset_family() {
        let registry = sample_target_registry();
        let view = registry_dataset_view(&registry, "sharkdp__fd").expect("view should resolve");

        assert_eq!(view.dataset, "sharkdp__fd");
        assert_eq!(view.entries.len(), 2);
        assert_eq!(view.entries[0].instance_id, "sharkdp__fd-497");
        assert_eq!(view.entries[1].instance_id, "sharkdp__fd-658");
        assert_eq!(
            view.source_paths,
            vec![PathBuf::from("/tmp/sharkdp__fd_dataset.jsonl")]
        );
    }

    #[test]
    fn registry_dataset_view_reports_available_datasets_on_miss() {
        let registry = sample_target_registry();
        let err = registry_dataset_view(&registry, "tokio-rs__tokio").expect_err("missing dataset");

        match err {
            PrepareError::InvalidBatchSelection { detail } => {
                assert!(detail.contains("tokio-rs__tokio"));
                assert!(detail.contains("sharkdp__fd"));
                assert!(detail.contains("BurntSushi__ripgrep"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn inspect_tool_calls_accepts_missing_record_selector() {
        Cli::try_parse_from(["ploke-eval", "inspect", "tool-calls"])
            .expect("inspect tool-calls should default to the most recent run");
    }

    #[test]
    fn protocol_issue_detection_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "protocol",
            "issue-detection",
            "--instance",
            "tokio-rs__bytes-543",
        ])
        .expect("protocol issue-detection should parse");

        match parsed.command {
            Command::Protocol(ProtocolCommand {
                command: ProtocolSubcommand::IssueDetection(cmd),
            }) => assert_eq!(cmd.instance.as_deref(), Some("tokio-rs__bytes-543")),
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn inspect_issue_overview_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "inspect",
            "issue-overview",
            "--instance",
            "tokio-rs__bytes-543",
        ])
        .expect("inspect issue-overview should parse");

        match parsed.command {
            Command::Inspect(InspectCommand {
                command: InspectSubcommand::IssueOverview(cmd),
            }) => assert_eq!(cmd.instance.as_deref(), Some("tokio-rs__bytes-543")),
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1",
            "--dataset-key",
            "clap-rs__clap",
            "--instance",
            "clap-rs__clap-3670",
            "--stop-after",
            "intervention-apply",
            "--dry-run",
        ])
        .expect("loop prototype1 should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command: LoopSubcommand::Prototype1(cmd),
            }) => {
                assert_eq!(cmd.dataset_key.as_deref(), Some("clap-rs__clap"));
                assert_eq!(cmd.instance, vec!["clap-rs__clap-3670".to_string()]);
                assert_eq!(cmd.max_generations, 1);
                assert_eq!(cmd.max_total_nodes, 32);
                assert!(cmd.require_keep_for_continuation);
                assert_eq!(cmd.stop_after, Prototype1LoopStopAfter::InterventionApply);
                assert!(cmd.dry_run);
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_command_parses_continued_branch_source() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1",
            "--dataset-key",
            "clap-rs__clap",
            "--instance",
            "clap-rs__clap-3670",
            "--source-campaign",
            "prototype1-campaign",
            "--source-branch-id",
            "branch-123",
            "--stop-after",
            "compare",
        ])
        .expect("loop prototype1 continued-source form should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command: LoopSubcommand::Prototype1(cmd),
            }) => {
                assert_eq!(cmd.source_campaign.as_deref(), Some("prototype1-campaign"));
                assert_eq!(cmd.source_branch_id.as_deref(), Some("branch-123"));
                assert_eq!(cmd.stop_after, Prototype1LoopStopAfter::Compare);
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_state_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1-state",
            "--campaign",
            "prototype1-campaign",
            "--node-id",
            "branch-abc-g1",
            "--handoff-invocation",
            "/tmp/prototype1-successor.json",
            "--stop-after",
            "build",
        ])
        .expect("loop prototype1-state should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command: LoopSubcommand::Prototype1State(cmd),
            }) => {
                assert_eq!(cmd.campaign, "prototype1-campaign");
                assert_eq!(cmd.node_id.as_deref(), Some("branch-abc-g1"));
                assert_eq!(
                    cmd.handoff_invocation.as_deref(),
                    Some(std::path::Path::new("/tmp/prototype1-successor.json"))
                );
                assert_eq!(cmd.stop_after, Prototype1StateStopAfter::Build);
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_state_identity_init_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1-state",
            "--campaign",
            "prototype1-campaign",
            "--node-id",
            "node-gen0",
            "--repo-root",
            "/tmp/repo",
            "--init-parent-identity",
            "--identity-branch",
            "prototype1-parent-gen0",
        ])
        .expect("loop prototype1-state identity init should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command: LoopSubcommand::Prototype1State(cmd),
            }) => {
                assert_eq!(cmd.campaign, "prototype1-campaign");
                assert_eq!(cmd.node_id.as_deref(), Some("node-gen0"));
                assert!(cmd.init_parent_identity);
                assert_eq!(
                    cmd.identity_branch.as_deref(),
                    Some("prototype1-parent-gen0")
                );
                assert_eq!(
                    cmd.repo_root.as_deref(),
                    Some(std::path::Path::new("/tmp/repo"))
                );
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_monitor_watch_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1-monitor",
            "--campaign",
            "prototype1-campaign",
            "--repo-root",
            "/tmp/repo",
            "watch",
            "--interval-ms",
            "75",
            "--print-initial",
        ])
        .expect("loop prototype1-monitor watch should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command: LoopSubcommand::Prototype1Monitor(cmd),
            }) => {
                assert_eq!(cmd.campaign, "prototype1-campaign");
                assert_eq!(
                    cmd.repo_root.as_deref(),
                    Some(std::path::Path::new("/tmp/repo"))
                );
                match cmd.command {
                    Prototype1MonitorSubcommand::Watch(watch) => {
                        assert_eq!(watch.interval_ms, 75);
                        assert!(watch.print_initial);
                    }
                    other => panic!("unexpected monitor subcommand: {:?}", other),
                }
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_monitor_peek_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1-monitor",
            "--campaign",
            "prototype1-campaign",
            "peek",
            "--lines",
            "5",
            "--bytes",
            "2048",
        ])
        .expect("loop prototype1-monitor peek should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command: LoopSubcommand::Prototype1Monitor(cmd),
            }) => match cmd.command {
                Prototype1MonitorSubcommand::Peek(peek) => {
                    assert_eq!(peek.lines, 5);
                    assert_eq!(peek.bytes, 2048);
                }
                other => panic!("unexpected monitor subcommand: {:?}", other),
            },
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_branch_select_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1-branch",
            "select",
            "--campaign",
            "prototype1-campaign",
            "--branch-id",
            "branch-123",
        ])
        .expect("loop prototype1-branch select should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command:
                    LoopSubcommand::Prototype1Branch(Prototype1BranchCommand {
                        command: Prototype1BranchSubcommand::Select(cmd),
                    }),
            }) => {
                assert_eq!(cmd.campaign, "prototype1-campaign");
                assert_eq!(cmd.branch_id, "branch-123");
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_branch_show_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1-branch",
            "show",
            "--campaign",
            "prototype1-campaign",
            "--branch-id",
            "branch-123",
        ])
        .expect("loop prototype1-branch show should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command:
                    LoopSubcommand::Prototype1Branch(Prototype1BranchCommand {
                        command: Prototype1BranchSubcommand::Show(cmd),
                    }),
            }) => {
                assert_eq!(cmd.campaign, "prototype1-campaign");
                assert_eq!(cmd.branch_id, "branch-123");
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_branch_apply_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1-branch",
            "apply",
            "--campaign",
            "prototype1-campaign",
            "--branch-id",
            "branch-123",
            "--repo-root",
            "/tmp/scratch",
        ])
        .expect("loop prototype1-branch apply should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command:
                    LoopSubcommand::Prototype1Branch(Prototype1BranchCommand {
                        command: Prototype1BranchSubcommand::Apply(cmd),
                    }),
            }) => {
                assert_eq!(cmd.campaign, "prototype1-campaign");
                assert_eq!(cmd.branch_id, "branch-123");
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp/scratch")));
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_branch_evaluate_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1-branch",
            "evaluate",
            "--campaign",
            "prototype1-campaign",
            "--branch-id",
            "branch-123",
            "--repo-root",
            "/tmp/scratch",
        ])
        .expect("loop prototype1-branch evaluate should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command:
                    LoopSubcommand::Prototype1Branch(Prototype1BranchCommand {
                        command: Prototype1BranchSubcommand::Evaluate(cmd),
                    }),
            }) => {
                assert_eq!(cmd.campaign, "prototype1-campaign");
                assert_eq!(cmd.branch_id, "branch-123");
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp/scratch")));
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_runner_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1-runner",
            "--campaign",
            "prototype1-campaign",
            "--node-id",
            "node-123",
            "--format",
            "json",
        ])
        .expect("loop prototype1-runner should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command: LoopSubcommand::Prototype1Runner(cmd),
            }) => {
                assert_eq!(cmd.campaign.as_deref(), Some("prototype1-campaign"));
                assert_eq!(cmd.node_id.as_deref(), Some("node-123"));
                assert!(cmd.invocation.is_none());
                assert!(!cmd.execute);
                assert!(!cmd.stop_on_error);
                assert_eq!(cmd.format, InspectOutputFormat::Json);
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_runner_invocation_command_parses() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1-runner",
            "--invocation",
            "/tmp/runtime.json",
            "--execute",
        ])
        .expect("loop prototype1-runner --invocation should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command: LoopSubcommand::Prototype1Runner(cmd),
            }) => {
                assert!(cmd.campaign.is_none());
                assert!(cmd.node_id.is_none());
                assert_eq!(cmd.invocation, Some(PathBuf::from("/tmp/runtime.json")));
                assert!(cmd.execute);
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn loop_prototype1_command_parses_search_policy_flags() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "loop",
            "prototype1",
            "--dataset-key",
            "clap-rs__clap",
            "--instance",
            "clap-rs__clap-3670",
            "--max-generations",
            "5",
            "--max-total-nodes",
            "99",
            "--stop-on-first-keep",
            "--require-keep-for-continuation",
            "false",
        ])
        .expect("loop prototype1 search-policy form should parse");

        match parsed.command {
            Command::Loop(LoopCommand {
                command: LoopSubcommand::Prototype1(cmd),
            }) => {
                assert_eq!(cmd.max_generations, 5);
                assert_eq!(cmd.max_total_nodes, 99);
                assert!(cmd.stop_on_first_keep);
                assert!(!cmd.require_keep_for_continuation);
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn resolve_record_path_defaults_to_last_run_record() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let run_dir = eval_home.join("instances").join("demo-run");
        std::fs::create_dir_all(&run_dir).expect("run dir");
        crate::run_history::record_last_run_at(&eval_home, &run_dir).expect("record last run");

        let resolution = resolve_record_path_from_eval_home(None, None, None, eval_home)
            .expect("default record path should resolve");

        assert_eq!(resolution.record_path, run_dir.join("record.json.gz"));
    }

    #[test]
    fn resolve_record_path_prefers_latest_registered_attempt_for_instance() {
        let _env_lock = hold_env_lock();
        let tmp = tempfile::tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        unsafe {
            std::env::set_var("PLOKE_EVAL_HOME", &eval_home);
        }
        let older = register_attempt(
            &eval_home,
            "run-older",
            crate::runner::RunArm::structured_current_policy_treatment(),
            "2026-04-23T10:00:00Z",
        );
        let newer = register_attempt(
            &eval_home,
            "run-newer",
            crate::runner::RunArm::structured_current_policy_treatment(),
            "2026-04-23T10:05:00Z",
        );

        let resolution = resolve_record_path_from_eval_home(
            None,
            Some("org__repo-1".to_string()),
            None,
            eval_home,
        )
        .expect("instance record path should resolve");

        assert_eq!(resolution.record_path, newer.artifacts.record_path);
        assert_ne!(resolution.record_path, older.artifacts.record_path);
    }

    #[test]
    fn resolve_record_path_defaults_to_latest_registered_attempt_even_if_control_is_newer() {
        let _env_lock = hold_env_lock();
        let tmp = tempfile::tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        unsafe {
            std::env::set_var("PLOKE_EVAL_HOME", &eval_home);
        }
        let _treatment = register_attempt(
            &eval_home,
            "run-treatment",
            crate::runner::RunArm::structured_current_policy_treatment(),
            "2026-04-23T10:00:00Z",
        );
        let control = register_attempt(
            &eval_home,
            "run-control",
            crate::runner::RunArm::shell_only_control(),
            "2026-04-23T10:05:00Z",
        );

        let resolution = resolve_record_path_from_eval_home(
            None,
            Some("org__repo-1".to_string()),
            None,
            eval_home,
        )
        .expect("instance record path should resolve");

        assert_eq!(resolution.record_path, control.artifacts.record_path);
    }

    #[test]
    fn resolve_record_path_rejects_legacy_instance_root_without_registration() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let instance_root = eval_home.join("instances").join("org__repo-1");
        std::fs::create_dir_all(&instance_root).expect("instance root");
        write_test_run_record(
            &instance_root.join("record.json.gz"),
            crate::runner::RunArm::structured_current_policy_treatment(),
        );

        let err = resolve_record_path_from_eval_home(
            None,
            Some("org__repo-1".to_string()),
            None,
            eval_home,
        )
        .expect_err("legacy instance-root record should not resolve");

        let detail = err.to_string();
        assert!(detail.contains("has no registered attempts"));
        assert!(detail.contains("legacy instance-root artifacts are no longer used"));
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
        let args = r#"{"file":"/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/globset/src/lib.rs","start_line":80,"end_line":120}"#;
        let summary = summarize_tool_inputs(args);
        assert!(summary.contains("lines=80-120"));
        assert!(summary.contains("file=.../globset/src/lib.rs"));
    }

    #[test]
    fn render_tool_inputs_surfaces_derived_line_window() {
        let lines = render_tool_inputs(
            r#"{"file":"/tmp/demo.rs","start_line":10,"end_line":24,"symbol":"demo"}"#,
        );
        assert_eq!(lines[0], "lines (derived from start_line/end_line): 10-24");
        assert!(lines.iter().any(|line| line == "file: /tmp/demo.rs"));
        assert!(lines.iter().any(|line| line == "symbol: demo"));
    }

    #[test]
    fn render_payload_block_reports_exact_inspector_truncation_metadata() {
        let rendered = render_payload_block("abcdefghijklmnopqrstuvwxyz", 10);
        assert_eq!(rendered.text, "abc...wxyz");
        assert_eq!(rendered.raw_bytes, 26);
        assert_eq!(rendered.normalized_chars, 26);
        assert_eq!(rendered.shown_source_chars, 7);
        assert!(rendered.inspector_truncated);
        assert_eq!(
            rendered.inspector_truncation_note().as_deref(),
            Some(
                "Inspector display truncated the normalized payload: 7/26 source chars shown (+ ellipsis), 19 elided; stored raw payload is 26 bytes."
            )
        );
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
    fn inspect_tool_overview_accepts_campaign_and_tool() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "inspect",
            "tool-overview",
            "--campaign",
            "rust-baseline-grok4-xai",
            "--tool",
            "apply_code_edit",
        ])
        .expect("tool-overview should parse campaign and tool");

        match parsed.command {
            Command::Inspect(InspectCommand {
                command: InspectSubcommand::ToolOverview(cmd),
            }) => {
                assert_eq!(cmd.campaign, "rust-baseline-grok4-xai");
                assert_eq!(cmd.tool.as_deref(), Some("apply_code_edit"));
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn inspect_operational_accepts_metrics_alias() {
        let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "metrics"])
            .expect("operational metrics should accept the metrics alias");

        match parsed.command {
            Command::Inspect(InspectCommand {
                command: InspectSubcommand::Operational(_),
            }) => {}
            other => panic!("unexpected command shape: {:?}", other),
        }
    }

    #[test]
    fn summarize_failure_reason_uses_first_non_empty_line() {
        let summary = summarize_failure_reason(
            "\napply_code_edit: No matching node found (strict+fallback)\nerror code: invalid_format\n",
        );
        assert_eq!(
            summary,
            "apply_code_edit: No matching node found (strict+fallback)"
        );
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

    fn sample_closure_state_for_submission_export(
        instances_root: PathBuf,
    ) -> crate::closure::ClosureState {
        crate::closure::ClosureState {
            schema_version: crate::closure::CLOSURE_STATE_SCHEMA_VERSION.to_string(),
            campaign_id: "campaign-1".to_string(),
            updated_at: "2026-04-17T00:00:00Z".to_string(),
            config: crate::closure::ClosureConfig {
                benchmark_family: BenchmarkFamily::MultiSweBenchRust,
                model_id: Some("x-ai/grok-4-fast".to_string()),
                provider_slug: Some("xai".to_string()),
                registry_path: None,
                dataset_sources: Vec::new(),
                required_procedures: Vec::new(),
                instances_root,
                batches_root: PathBuf::from("/tmp/batches"),
                framework: crate::spec::FrameworkConfig::default(),
            },
            registry: crate::closure::RegistryClosureSummary {
                expected_total: 3,
                mapped_total: 3,
                missing_total: 0,
                ambiguous_total: 0,
                status: ClosureClass::Complete,
            },
            eval: crate::closure::EvalClosureSummary {
                expected_total: 3,
                complete_total: 2,
                failed_total: 1,
                missing_total: 0,
                partial_total: 0,
                in_progress_total: 0,
                status: ClosureClass::Partial,
                last_transition_at: None,
            },
            protocol: crate::closure::ProtocolClosureSummary {
                expected_total: 0,
                full_total: 0,
                partial_total: 0,
                failed_total: 0,
                missing_total: 0,
                incompatible_total: 0,
                ineligible_total: 0,
                in_progress_total: 0,
                status: ClosureClass::Complete,
                required_procedures: Vec::new(),
                status_by_procedure: BTreeMap::new(),
                last_transition_at: None,
            },
            instances: vec![
                crate::closure::ClosureInstanceRow {
                    instance_id: "org__repo-1".to_string(),
                    dataset_label: "org__repo".to_string(),
                    repo_family: "org__repo".to_string(),
                    registry_status: crate::closure::RegistryInstanceStatus::Mapped,
                    eval_status: ClosureClass::Complete,
                    protocol_status: ClosureClass::Complete,
                    eval_failure: None,
                    protocol_failure: None,
                    artifacts: crate::closure::ClosureArtifactRefs::default(),
                    protocol_procedures: BTreeMap::new(),
                    protocol_counts: None,
                    last_event_at: None,
                },
                crate::closure::ClosureInstanceRow {
                    instance_id: "org__repo-2".to_string(),
                    dataset_label: "org__repo".to_string(),
                    repo_family: "org__repo".to_string(),
                    registry_status: crate::closure::RegistryInstanceStatus::Mapped,
                    eval_status: ClosureClass::Complete,
                    protocol_status: ClosureClass::Complete,
                    eval_failure: None,
                    protocol_failure: None,
                    artifacts: crate::closure::ClosureArtifactRefs::default(),
                    protocol_procedures: BTreeMap::new(),
                    protocol_counts: None,
                    last_event_at: None,
                },
                crate::closure::ClosureInstanceRow {
                    instance_id: "org__repo-3".to_string(),
                    dataset_label: "org__repo".to_string(),
                    repo_family: "org__repo".to_string(),
                    registry_status: crate::closure::RegistryInstanceStatus::Mapped,
                    eval_status: ClosureClass::Failed,
                    protocol_status: ClosureClass::Ineligible,
                    eval_failure: Some("failed".to_string()),
                    protocol_failure: None,
                    artifacts: crate::closure::ClosureArtifactRefs::default(),
                    protocol_procedures: BTreeMap::new(),
                    protocol_counts: None,
                    last_event_at: None,
                },
            ],
        }
    }

    #[test]
    fn collect_campaign_submission_records_filters_empty_patches_only_when_requested() {
        let tmp = tempdir().expect("tempdir");
        let instances_root = tmp.path().join("instances");
        fs::create_dir_all(
            instances_root
                .join("org__repo-1")
                .join("runs")
                .join("run-a"),
        )
        .expect("run dir 1");
        fs::create_dir_all(
            instances_root
                .join("org__repo-2")
                .join("runs")
                .join("run-b"),
        )
        .expect("run dir 2");
        fs::write(
            instances_root
                .join("org__repo-1")
                .join("runs")
                .join("run-a")
                .join("record.json.gz"),
            "record",
        )
        .expect("write record 1");
        fs::write(
            instances_root
                .join("org__repo-2")
                .join("runs")
                .join("run-b")
                .join("record.json.gz"),
            "record",
        )
        .expect("write record 2");
        fs::write(
            instances_root
                .join("org__repo-1")
                .join("runs")
                .join("run-a")
                .join("multi-swe-bench-submission.jsonl"),
            serde_json::to_string(&MultiSweBenchSubmissionRecord {
                org: "org".to_string(),
                repo: "repo".to_string(),
                number: 1,
                fix_patch: "diff --git a/src/lib.rs b/src/lib.rs\n".to_string(),
            })
            .expect("json"),
        )
        .expect("write submission 1");
        fs::write(
            instances_root
                .join("org__repo-2")
                .join("runs")
                .join("run-b")
                .join("multi-swe-bench-submission.jsonl"),
            serde_json::to_string(&MultiSweBenchSubmissionRecord {
                org: "org".to_string(),
                repo: "repo".to_string(),
                number: 2,
                fix_patch: String::new(),
            })
            .expect("json"),
        )
        .expect("write submission 2");

        let state = sample_closure_state_for_submission_export(instances_root);
        let all_records =
            collect_campaign_submission_records(&state, false).expect("all records export");
        let nonempty_records =
            collect_campaign_submission_records(&state, true).expect("nonempty export");

        assert_eq!(all_records.len(), 2);
        assert_eq!(nonempty_records.len(), 1);
        assert_eq!(nonempty_records[0].number, 1);
        assert_eq!(
            count_campaign_empty_patch_rows(&state).expect("empty rows"),
            1
        );
    }

    #[test]
    fn collect_campaign_submission_records_prefers_treatment_submission_over_newer_control_run() {
        let tmp = tempdir().expect("tempdir");
        let instances_root = tmp.path().join("instances");
        let treatment_run = instances_root
            .join("org__repo-1")
            .join("runs")
            .join("run-treatment");
        let control_run = instances_root
            .join("org__repo-1")
            .join("runs")
            .join("run-control");
        fs::create_dir_all(&treatment_run).expect("treatment run dir");
        fs::create_dir_all(&control_run).expect("control run dir");
        write_test_run_record(
            &treatment_run.join("record.json.gz"),
            crate::runner::RunArm::structured_current_policy_treatment(),
        );
        fs::write(
            treatment_run.join("multi-swe-bench-submission.jsonl"),
            serde_json::to_string(&MultiSweBenchSubmissionRecord {
                org: "org".to_string(),
                repo: "repo".to_string(),
                number: 1,
                fix_patch: "diff --git a/src/lib.rs b/src/lib.rs\n".to_string(),
            })
            .expect("json"),
        )
        .expect("write treatment submission");
        std::thread::sleep(std::time::Duration::from_millis(10));
        write_test_run_record(
            &control_run.join("record.json.gz"),
            crate::runner::RunArm::shell_only_control(),
        );

        let mut state = sample_closure_state_for_submission_export(instances_root);
        state.instances.truncate(1);
        let records =
            collect_campaign_submission_records(&state, false).expect("all records export");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].number, 1);
        assert_eq!(
            records[0].fix_patch,
            "diff --git a/src/lib.rs b/src/lib.rs\n"
        );
    }

    #[test]
    fn default_campaign_submission_export_path_uses_campaign_directory() {
        let path =
            default_campaign_submission_export_path("campaign-1", false).expect("default path");
        assert!(path.ends_with("campaign-1/multi-swe-bench-submission.jsonl"));

        let nonempty_path =
            default_campaign_submission_export_path("campaign-1", true).expect("nonempty path");
        assert!(nonempty_path.ends_with("campaign-1/multi-swe-bench-submission.nonempty.jsonl"));
    }

    #[test]
    fn campaign_export_submissions_parses_nonempty_flag() {
        let parsed = Cli::try_parse_from([
            "ploke-eval",
            "campaign",
            "export-submissions",
            "--campaign",
            "campaign-1",
            "--nonempty-only",
        ])
        .expect("campaign export-submissions should parse");

        match parsed.command {
            Command::Campaign(CampaignCommand {
                command: CampaignSubcommand::ExportSubmissions(cmd),
            }) => {
                assert_eq!(cmd.campaign, "campaign-1");
                assert!(cmd.nonempty_only);
            }
            other => panic!("unexpected command shape: {:?}", other),
        }
    }
}
