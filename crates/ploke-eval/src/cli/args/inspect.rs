use std::path::PathBuf;

use ploke_records::ids::CampaignId;

use clap::{Parser, Subcommand};

use crate::protocol_report::ProtocolColorProfile;

use super::common::InspectOutputFormat;

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
    pub campaign: CampaignId,

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
    pub campaign: Option<CampaignId>,

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
