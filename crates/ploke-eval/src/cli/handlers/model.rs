use std::path::PathBuf;
use std::str::FromStr;

use ploke_llm::request::endpoint::Endpoint;
use ploke_llm::router_only::HasEndpoint;
use ploke_llm::router_only::openrouter::{OpenRouter, OpenRouterModelId};
use ploke_llm::{ModelId, ProviderKey, SupportsTools};

use crate::cli::format::{display_context_length, display_price_per_million, model_size_string};
use crate::cli::provider::{current_provider_for_model, resolve_provider_model_id};
use crate::cli::{
    ModelCommand, ModelSubcommand, ParentPatcherCommand, ParentPatcherSubcommand, ProviderCommand,
    ProviderSubcommand,
};
use crate::model_registry::{
    find_models, load_active_model, load_model_registry, load_parent_patcher_model,
    refresh_model_registry, save_active_model, save_parent_patcher_model,
};
use crate::provider_prefs::{
    clear_provider_for_model, load_provider_for_model, set_provider_for_model,
};
use crate::runner::resolve_route_for_model;
use crate::spec::PrepareError;

impl ModelCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            ModelSubcommand::Refresh => {
                let registry = refresh_model_registry().await?;
                let direct_google = registry
                    .data
                    .iter()
                    .filter(|item| item.route_source.is_direct_google())
                    .count();
                println!(
                    "refreshed {} models ({} direct Google)",
                    registry.data.len(),
                    direct_google
                );
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
                    "{:<id_width$}  {:>14}  {:>10}  {:>10}  {:>13}  {}",
                    "model_id",
                    "context_length",
                    "in($/M)",
                    "out($/M)",
                    "route_source",
                    "size",
                    id_width = id_width
                );
                for item in items {
                    let context = display_context_length(item);
                    let input = display_price_per_million(item.pricing.prompt);
                    let output = display_price_per_million(item.pricing.completion);
                    let size = model_size_string(item);
                    let route_source = if item.route_source.is_direct_google() {
                        "direct_google"
                    } else {
                        "openrouter"
                    };
                    println!(
                        "{:<id_width$}  {:>14}  {:>10}  {:>10}  {:>13}  {}",
                        item.id,
                        context,
                        input,
                        output,
                        route_source,
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
            ModelSubcommand::ParentPatcher(cmd) => cmd.run().await,
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

impl ParentPatcherCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            ParentPatcherSubcommand::Set { model_id } => {
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
                save_parent_patcher_model(&selected.id)?;
                println!("{}", selected.id);
                Ok(())
            }
            ParentPatcherSubcommand::Current => {
                let selected = load_parent_patcher_model()?;
                match load_model_registry() {
                    Ok(registry) => {
                        if let Some(item) = registry
                            .data
                            .iter()
                            .find(|item| item.id == selected.model_id)
                        {
                            println!("{}\t{}", item.id, item.name.as_str());
                        } else {
                            println!("{}", selected.model_id);
                        }
                    }
                    Err(PrepareError::MissingModelRegistry(_)) => println!("{}", selected.model_id),
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

// ANCHOR: model_providers_route_behavior
async fn print_model_providers(model_id: Option<String>) -> Result<(), PrepareError> {
    let model_id = match model_id {
        Some(model_id) => model_id,
        None => load_active_model()?.model_id.to_string(),
    };

    let model = ModelId::from_str(&model_id).map_err(|err| PrepareError::DatabaseSetup {
        phase: "parse_model_id",
        detail: format!("invalid model id '{model_id}': {err}"),
    })?;
    if let Ok(registry) = load_model_registry() {
        if let Some(item) = registry.data.iter().find(|item| item.id == model) {
            if item.route_source.is_direct_google() {
                println!("Direct Google route for model '{}':", model);
                println!(
                    "  {:<14}  {:<14}  {:<5}  {:<8}  {}",
                    "provider_slug", "provider_name", "tools", "selected", "context"
                );
                println!(
                    "  {:<14}  {:<14}  {:<5}  {:<8}  {}",
                    "google",
                    "Google",
                    if item.supports_tools() { "yes" } else { "no" },
                    "yes",
                    item.context_length
                        .or(item.top_provider.context_length)
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                return Ok(());
            }
        }
    }
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
// ANCHOR_END: model_providers_route_behavior

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
    let route = resolve_route_for_model(&selected, Some(&provider_key)).await?;
    let provider = route
        .provider_key()
        .cloned()
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "set_provider_for_model",
            detail: format!(
                "model '{}' uses a direct route and does not have provider endpoints to persist",
                selected.id
            ),
        })?;
    set_provider_for_model(&selected.id, provider.clone())?;
    println!("{}\t{}", selected.id, provider.slug.as_str());
    Ok(())
}
