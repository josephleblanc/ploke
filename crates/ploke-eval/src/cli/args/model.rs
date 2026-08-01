use clap::{Parser, Subcommand};

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
    about = "Manage OpenRouter provider preferences and inspect effective model providers",
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

#[derive(Debug, Parser)]
#[command(
    about = "Manage the persisted default model used for parent broad-harness patch generation",
    after_help = "\
Examples:

  cargo run -p ploke-eval -- model parent-patcher set minimax/minimax-m2.5
  cargo run -p ploke-eval -- model parent-patcher current
"
)]
pub struct ParentPatcherCommand {
    #[command(subcommand)]
    pub command: ParentPatcherSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ProviderSubcommand {
    /// Persist the OpenRouter default provider for the current or specified model.
    Set {
        /// Provider slug to remember for the model.
        provider_slug: String,

        /// Model id to update. Defaults to the current active model.
        #[arg(long)]
        model_id: Option<String>,
    },
    /// Show the effective provider for the current or specified model.
    Current {
        /// Model id to inspect. Defaults to the current active model.
        #[arg(long)]
        model_id: Option<String>,
    },
    /// Clear the persisted OpenRouter default provider for the current or specified model.
    Clear {
        /// Model id to update. Defaults to the current active model.
        #[arg(long)]
        model_id: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ParentPatcherSubcommand {
    /// Persist the model used for parent broad-harness patch generation.
    Set {
        /// Exact model id to mark as the parent patcher.
        model_id: String,
    },
    /// Show the current parent patcher model selection.
    Current,
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
    /// Persist or inspect the model used for parent broad-harness patch generation.
    #[command(name = "parent-patcher")]
    ParentPatcher(ParentPatcherCommand),
    /// Manage OpenRouter provider preferences and inspect effective providers.
    Provider(ProviderCommand),
    /// Persist the active model selection.
    Set {
        /// Exact model id to mark active.
        model_id: String,
    },
    /// Show the current active model selection.
    Current,
}
