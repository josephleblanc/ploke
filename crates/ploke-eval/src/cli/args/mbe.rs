use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(about = "Prepare and run MBE oracle evaluations over typed eval artifacts")]
pub struct MbeCommand {
    #[command(subcommand)]
    pub command: MbeSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum MbeSubcommand {
    /// Run the MBE oracle harness for one prepared Multi-SWE-bench run.
    Run(MbeRunCommand),
    /// List Prototype 1 child nodes with MBE submission artifacts.
    CampaignCandidates(MbeCampaignCandidatesCommand),
    /// Run the MBE oracle harness for one Prototype 1 child node.
    RunCampaignCandidate(MbeRunCampaignCandidateCommand),
    /// List prepared runs that can be passed to the MBE oracle.
    Runs(MbeRunsCommand),
    /// Write an MBE harness config for one prepared Multi-SWE-bench run.
    WriteConfig(MbeWriteConfigCommand),
    /// Read MBE final_report.json and print the verdict for one prepared run.
    Verdict(MbeVerdictCommand),
}

#[derive(Debug, Args)]
pub struct MbeRequestArgs {
    /// Path to the prepared run manifest. Omit when using --instance.
    #[arg(long, value_name = "PATH")]
    pub run: Option<PathBuf>,

    /// Benchmark instance id; selects the latest completed attempt with a submission.
    #[arg(long)]
    pub instance: Option<String>,

    /// Attempt number from `mbe runs --instance <id>`.
    #[arg(long)]
    pub attempt: Option<usize>,

    /// Path to multi-swe-bench-submission.jsonl. Defaults to <prepared output_dir>/multi-swe-bench-submission.jsonl.
    #[arg(long, value_name = "PATH")]
    pub submission: Option<PathBuf>,

    /// MBE output directory. Defaults to <prepared output_dir>/mbe.
    #[arg(long, value_name = "DIR")]
    pub output_dir: Option<PathBuf>,

    /// Repository cache dir containing <org>/<repo>. Defaults from the prepared repo_root.
    #[arg(long, value_name = "DIR")]
    pub repo_dir: Option<PathBuf>,

    /// Worker count for all MBE worker pools in the generated config.
    #[arg(long, default_value_t = 1)]
    pub workers: u32,
}

#[derive(Debug, Parser)]
pub struct MbeRunsCommand {
    /// Benchmark instance id.
    #[arg(long)]
    pub instance: String,
}

#[derive(Debug, Parser)]
pub struct MbeCampaignCandidatesCommand {
    /// Prototype 1 campaign id.
    #[arg(long)]
    pub campaign: String,

    /// Show only candidates whose fix_patch is non-empty.
    #[arg(long)]
    pub nonempty_only: bool,
}

#[derive(Debug, Parser)]
pub struct MbeRunCampaignCandidateCommand {
    /// Prototype 1 campaign id.
    #[arg(long)]
    pub campaign: String,

    /// Prototype 1 node id to test.
    #[arg(long)]
    pub node: String,

    /// MBE output directory. Defaults to the child treatment run's mbe directory.
    #[arg(long, value_name = "DIR")]
    pub output_dir: Option<PathBuf>,

    /// Repository cache dir containing <org>/<repo>. Defaults from the prepared repo_root.
    #[arg(long, value_name = "DIR")]
    pub repo_dir: Option<PathBuf>,

    /// Worker count for all MBE worker pools in the generated config.
    #[arg(long, default_value_t = 1)]
    pub workers: u32,

    /// Python executable used to invoke `multi_swe_bench.harness.run_evaluation`.
    #[arg(long, default_value = "python")]
    pub python: String,
}

#[derive(Debug, Parser)]
pub struct MbeRunCommand {
    #[command(flatten)]
    pub request: MbeRequestArgs,

    /// Python executable used to invoke `multi_swe_bench.harness.run_evaluation`.
    #[arg(long, default_value = "python")]
    pub python: String,
}

#[derive(Debug, Parser)]
pub struct MbeWriteConfigCommand {
    #[command(flatten)]
    pub request: MbeRequestArgs,

    /// Python executable to render in the suggested harness command.
    #[arg(long, default_value = "python")]
    pub python: String,
}

#[derive(Debug, Parser)]
pub struct MbeVerdictCommand {
    /// Path to the prepared run manifest.
    #[arg(long, value_name = "PATH")]
    pub run: PathBuf,

    /// Path to MBE final_report.json.
    #[arg(long, value_name = "PATH")]
    pub report: PathBuf,
}
