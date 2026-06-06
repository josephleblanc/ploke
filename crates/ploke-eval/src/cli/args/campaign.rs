use std::path::PathBuf;

use clap::{Parser, Subcommand};

use ploke_llm::request::models::ModelRouteSource;

use super::common::{InspectOutputFormat, parse_model_route_source};

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

    /// Override the selected route source: openrouter or direct-google.
    #[arg(long, value_name = "ROUTE", value_parser = parse_model_route_source)]
    pub route_source: Option<ModelRouteSource>,

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
