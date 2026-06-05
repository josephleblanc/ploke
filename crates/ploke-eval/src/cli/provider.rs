use std::str::FromStr;

use ploke_llm::request::models::ModelRouteSource;
use ploke_llm::{ModelId, ProviderKey};

use crate::model_registry::{load_active_model, load_model_registry, load_parent_patcher_model};
use crate::provider_prefs::load_provider_for_model;
use crate::spec::PrepareError;

pub(crate) fn parse_provider_key(
    provider: Option<String>,
) -> Result<Option<ProviderKey>, PrepareError> {
    provider
        .map(|slug| {
            ProviderKey::new(&slug).map_err(|err| PrepareError::DatabaseSetup {
                phase: "parse_provider_key",
                detail: format!("invalid provider slug '{slug}': {err}"),
            })
        })
        .transpose()
}

pub(crate) fn resolve_provider_model_id(model_id: Option<String>) -> Result<ModelId, PrepareError> {
    match model_id {
        Some(model_id) => ModelId::from_str(&model_id).map_err(|err| PrepareError::DatabaseSetup {
            phase: "parse_model_id",
            detail: format!("invalid model id '{model_id}': {err}"),
        }),
        None => Ok(load_active_model()?.model_id),
    }
}

pub(crate) fn current_provider_for_model(
    model_id: Option<String>,
) -> Result<(ModelId, Option<ProviderKey>), PrepareError> {
    let model = resolve_provider_model_id(model_id)?;
    if registry_route_source(&model)?.is_some_and(|source| source.is_direct_google()) {
        let provider = ProviderKey::new("google").map_err(|err| PrepareError::DatabaseSetup {
            phase: "direct_google_provider_key",
            detail: err.to_string(),
        })?;
        return Ok((model, Some(provider)));
    }
    let provider = load_provider_for_model(&model)?;
    Ok((model, provider))
}

pub(crate) fn registry_route_source(
    model_id: &ModelId,
) -> Result<Option<ModelRouteSource>, PrepareError> {
    match load_model_registry() {
        Ok(registry) => Ok(registry
            .data
            .iter()
            .find(|item| item.id == *model_id)
            .map(|item| item.route_source)),
        Err(PrepareError::MissingModelRegistry(_)) => Ok(None),
        Err(err) => Err(err),
    }
}

pub(crate) fn headless_model_selection(
    model_id: ModelId,
    provider: Option<ProviderKey>,
) -> Result<crate::cli::prototype1_state::edit_surface::tui_adapter::ModelSelection, PrepareError> {
    let registry_direct_google =
        registry_route_source(&model_id)?.is_some_and(|source| source.is_direct_google());
    let requested_google = provider
        .as_ref()
        .is_some_and(|provider| provider.slug.as_str() == "google");

    if registry_direct_google || requested_google {
        if let Some(provider) = provider.as_ref()
            && provider.slug.as_str() != "google"
        {
            return Err(PrepareError::DatabaseSetup {
                phase: "headless_model_route",
                detail: format!(
                    "direct Google model '{model_id}' does not accept OpenRouter provider '{}'",
                    provider.slug.as_str()
                ),
            });
        }
        return Ok(
            crate::cli::prototype1_state::edit_surface::tui_adapter::ModelSelection::direct_google(
                model_id,
            ),
        );
    }

    Ok(
        crate::cli::prototype1_state::edit_surface::tui_adapter::ModelSelection::openrouter(
            model_id, provider,
        ),
    )
}

pub(crate) fn headless_model_selection_from_provider_preference(
    model_id: ModelId,
    provider: Option<ProviderKey>,
) -> Result<crate::cli::prototype1_state::edit_surface::tui_adapter::ModelSelection, PrepareError> {
    if registry_route_source(&model_id)?.is_some_and(|source| source.is_direct_google()) {
        return Ok(
            crate::cli::prototype1_state::edit_surface::tui_adapter::ModelSelection::direct_google(
                model_id,
            ),
        );
    }

    headless_model_selection(model_id, provider)
}

pub(crate) fn load_parent_patcher_model_selection()
-> Result<crate::cli::prototype1_state::edit_surface::tui_adapter::ModelSelection, PrepareError> {
    // Temporary split config: broad parent patch generation reads the
    // parent-patcher selection here, while eval/protocol defaults still read
    // `load_active_model()` in `resolve_protocol_model_id()` above. Collapse
    // both onto the admitted profile/campaign config once that plumbing exists.
    let selected = load_parent_patcher_model().or_else(|err| match err {
        PrepareError::MissingParentPatcherModel(_) => load_active_model(),
        other => Err(other),
    })?;
    let provider = load_provider_for_model(&selected.model_id)?;
    headless_model_selection_from_provider_preference(selected.model_id, provider)
}
