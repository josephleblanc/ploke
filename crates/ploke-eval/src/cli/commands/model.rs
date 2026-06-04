use clap::{Parser, Subcommand};

use crate::cli::format;
use crate::model_registry::{
    find_models, load_active_model, load_model_registry, refresh_model_registry, save_active_model,
};
use crate::spec::PrepareError;

#[derive(Debug, Parser)]
#[command(
    about = "Manage the cached OpenRouter model registry and active model selection",
    after_help = "\
Examples:
\n  cargo run -p ploke-eval -- model refresh
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
\n  cargo run -p ploke-eval -- model provider current
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
\nIf no model id is passed, the current active eval model is used.
",
        after_help = "\
Examples:
\n  ploke-eval model providers
  ploke-eval model providers moonshotai/kimi-k2
\nThe output shows provider slug, provider name, tool support, and context length.
"
    )]
    Providers {
        /// Exact model id to inspect. Defaults to the current active model.
        model_id: Option<String>,
    },
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
                    let context = format::display_context_length(item);
                    let input = format::display_price_per_million(item.pricing.prompt);
                    let output = format::display_price_per_million(item.pricing.completion);
                    let size = format::model_size_string(item);
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
            ModelSubcommand::Providers { model_id } => super::super::print_model_providers(model_id).await,
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
                        if let Some(item) = registry.data.iter().find(|item| item.id == active.model_id)
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
            } => super::super::set_persisted_provider(model_id, provider_slug).await,
            ProviderSubcommand::Current { model_id } => {
                let (model_id, provider) = super::super::current_provider_for_model(model_id)?;
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
                let model = super::super::resolve_provider_model_id(model_id)?;
                crate::provider_prefs::clear_provider_for_model(&model)?;
                println!("{}\tauto", model);
                Ok(())
            }
        }
    }
}
