use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};
use ploke_llm::Router;
use ploke_llm::router_only::openrouter::OpenRouter;

use crate::layout::{
    active_model_file, cache_dir, datasets_dir, model_registry_file, models_dir, repos_dir,
    runs_dir, starting_db_cache_dir, workspace_root_for_key,
};
use crate::model_registry::{
    find_models, load_active_model, load_model_registry, refresh_model_registry,
    registry_has_model, save_active_model,
};
use crate::msb::PrepareMsbSingleRunRequest;
use crate::registry::{builtin_dataset_registry_entries, builtin_dataset_registry_entry};
use crate::runner::{ReplayMsbBatchRequest, RunMsbAgentSingleRequest, RunMsbSingleRequest};
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
  repo cache         ~/.ploke-eval/repos
  run artifacts      ~/.ploke-eval/runs

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

Health check

  cargo run -p ploke-eval -- doctor

Replay a specific embedding batch from a prepared run

  cargo run -p ploke-eval -- replay-msb-batch --instance BurntSushi__ripgrep-2209 --batch 6

Replay notes:
  - batch numbers are 1-based
  - the command writes `replay-batch-<nnn>.json` beside the run manifest
  - the JSON includes the full serialized `TypedEmbedData` batch
  - the command then runs only that batch through the normal embed path

Default read/write locations

  Dataset JSONL cache:
    ~/.ploke-eval/datasets/BurntSushi__ripgrep_dataset.jsonl

  Repo checkout:
    ~/.ploke-eval/repos/BurntSushi/ripgrep

  Run directory:
    ~/.ploke-eval/runs/BurntSushi__ripgrep-2209

  Key run artifacts:
    run.json
    repo-state.json
    execution-log.json
    indexing-status.json
    snapshot-status.json

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
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Normalize one evaluation instance into a run manifest.
    PrepareSingle(PrepareSingleCommand),
    /// Normalize one Multi-SWE-bench instance into a run manifest.
    PrepareMsbSingle(PrepareMsbSingleCommand),
    /// Execute one prepared run through repo reset and initial artifact generation.
    RunMsbSingle(RunMsbSingleCommand),
    /// Execute one prepared run and then run a single agentic benchmark turn.
    RunMsbAgentSingle(RunMsbAgentSingleCommand),
    /// Replay one specific batch from a prepared run and print the exact snippets for it.
    ReplayMsbBatch(ReplayMsbBatchCommand),
    /// Clone or refresh a benchmark repo into ~/.ploke-eval/repos.
    FetchMsbRepo(FetchMsbRepoCommand),
    /// List built-in dataset registry entries.
    ListMsbDatasets,
    /// Inspect the current eval setup and report likely configuration issues.
    Doctor,
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
            Command::RunMsbSingle(cmd) => match cmd.run().await {
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
}

#[derive(Debug, Parser)]
#[command(
    about = "Execute one prepared Multi-SWE-bench run and one benchmark issue turn",
    after_help = "\
This extends the normal run with a single agentic turn that:
  - submits the prepared issue prompt through the real app/state path
  - records prompt construction, tool lifecycle, message updates, and turn completion
  - writes a turn trace and summary beside the run artifacts
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
}

#[derive(Debug, Parser)]
#[command(
    about = "Manage the cached OpenRouter model registry and active model selection",
    after_help = "\
Examples:

  cargo run -p ploke-eval -- model refresh
  cargo run -p ploke-eval -- model list
  cargo run -p ploke-eval -- model find qwen
  cargo run -p ploke-eval -- model set moonshotai/kimi-k2
  cargo run -p ploke-eval -- model current
"
)]
pub struct ModelCommand {
    #[command(subcommand)]
    pub command: ModelSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ModelSubcommand {
    /// Download the latest OpenRouter model catalog into the local registry JSON.
    Refresh,
    /// List all cached models.
    List,
    /// Find models whose id, name, or canonical id matches the stem.
    Find {
        /// Stem or substring to search for.
        query: String,
    },
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
                for item in items {
                    println!("{}\t{}", item.id, item.name.as_str());
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
        }
        .run()
        .await?;
        println!("{}", artifacts.execution_log.display());
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
        }
        .run()
        .await?;
        println!("{}", artifacts.base.execution_log.display());
        println!("{}", artifacts.turn_summary.display());
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

fn run_doctor() -> Result<(), PrepareError> {
    let mut ok = 0usize;
    let mut warn = 0usize;

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

    check_dir("datasets dir", datasets_dir()?, &mut ok, &mut warn);
    check_dir("models dir", models_dir()?, &mut ok, &mut warn);
    check_dir("repo cache dir", repos_dir()?, &mut ok, &mut warn);
    check_dir("run artifacts dir", runs_dir()?, &mut ok, &mut warn);
    check_dir(
        "starting-db cache dir",
        starting_db_cache_dir()?,
        &mut ok,
        &mut warn,
    );
    check_dir("cache dir", cache_dir()?, &mut ok, &mut warn);

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
    println!("summary: {ok} ok, {warn} warnings");
    Ok(())
}

fn check_dir(label: &str, path: PathBuf, ok: &mut usize, warn: &mut usize) {
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
        *warn += 1;
        println!("[warn] {label}: missing ({})", path.display());
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
