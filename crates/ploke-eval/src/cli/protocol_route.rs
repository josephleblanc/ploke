use ploke_llm::request::models::ModelRouteSource;
use ploke_llm::{ModelId, ProviderKey};
use ploke_protocol::tool_calls::{review, segment};

use crate::model_registry::load_active_model;
use crate::provider_prefs::load_provider_for_model;
use crate::spec::PrepareError;

use super::provider::registry_route_source;

pub(crate) fn resolve_protocol_model_id(model_id: Option<String>) -> Result<ModelId, PrepareError> {
    match model_id {
        Some(model_id) => {
            model_id
                .parse()
                .map_err(|err: ploke_llm::IdError| PrepareError::DatabaseSetup {
                    phase: "protocol_model_id",
                    detail: err.to_string(),
                })
        }
        // Temporary split config: eval/protocol defaults still read the active
        // model selection here, while broad parent patching reads
        // `load_parent_patcher_model_selection()` below. Collapse both onto the
        // admitted profile/campaign config once that plumbing exists.
        None => load_active_model().map(|selection| selection.model_id),
    }
}

pub(crate) fn resolve_protocol_provider_slug(
    model_id: &ModelId,
    route_source: Option<ModelRouteSource>,
    provider: Option<String>,
) -> Result<Option<String>, PrepareError> {
    resolve_protocol_route(model_id, route_source, provider).map(|(_, provider_slug)| provider_slug)
}

pub(crate) fn resolve_protocol_route(
    model_id: &ModelId,
    route_source: Option<ModelRouteSource>,
    provider: Option<String>,
) -> Result<(ModelRouteSource, Option<String>), PrepareError> {
    if let Some(route_source) = route_source {
        return match route_source {
            ModelRouteSource::DirectGoogle => {
                if let Some(provider) = provider.as_deref()
                    && provider != "google"
                {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "protocol_route",
                        detail: format!(
                            "direct Google route does not accept OpenRouter provider '{provider}'"
                        ),
                    });
                }
                Ok((ModelRouteSource::DirectGoogle, None))
            }
            ModelRouteSource::OpenRouter => {
                let provider_slug = provider
                    .map(|provider| {
                        ProviderKey::new(&provider).map_err(|err| PrepareError::DatabaseSetup {
                            phase: "protocol_provider_slug",
                            detail: err.to_string(),
                        })
                    })
                    .transpose()?
                    .map(|provider| provider.slug.as_str().to_string());
                Ok((ModelRouteSource::OpenRouter, provider_slug))
            }
        };
    }

    if let Some(provider) = provider {
        let parsed = ProviderKey::new(&provider).map_err(|err| PrepareError::DatabaseSetup {
            phase: "protocol_provider_slug",
            detail: err.to_string(),
        })?;
        if registry_route_source(model_id)?.is_some_and(|source| source.is_direct_google()) {
            if parsed.slug.as_str() == "google" {
                return Ok((ModelRouteSource::DirectGoogle, None));
            }
            return Err(PrepareError::DatabaseSetup {
                phase: "protocol_route",
                detail: format!(
                    "direct Google model '{model_id}' does not accept OpenRouter provider '{}'",
                    parsed.slug.as_str()
                ),
            });
        }
        return Ok((
            ModelRouteSource::OpenRouter,
            Some(parsed.slug.as_str().to_string()),
        ));
    }

    if registry_route_source(model_id)?.is_some_and(|source| source.is_direct_google()) {
        return Ok((ModelRouteSource::DirectGoogle, None));
    }

    let provider = load_provider_for_model(model_id)?;
    Ok((
        ModelRouteSource::OpenRouter,
        provider.map(|provider| provider.slug.as_str().to_string()),
    ))
}

pub(crate) fn tool_call_review_error_to_prepare(err: review::ToolCallReviewError) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "protocol_tool_call_review",
        detail: err.to_string(),
    }
}

pub(crate) fn tool_call_intent_segmentation_error_to_prepare(
    err: segment::IntentSegmentationError,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "protocol_tool_call_intent_segmentation",
        detail: err.to_string(),
    }
}

pub(crate) fn is_retryable_intent_segmentation_error(
    err: &segment::IntentSegmentationError,
) -> bool {
    match err {
        segment::IntentSegmentationError::Second(ploke_protocol::MergeError::Branches(
            ploke_protocol::FanOutError::Right(llm_error),
        )) if llm_error.is_truncated_json_parse() => true,
        segment::IntentSegmentationError::Second(ploke_protocol::MergeError::Join(
            segment::NormalizeSegmentsError::Overlap { .. }
            | segment::NormalizeSegmentsError::InvalidRange { .. }
            | segment::NormalizeSegmentsError::MissingLabel { .. }
            | segment::NormalizeSegmentsError::AmbiguousWithLabel { .. },
        )) => true,
        _ => false,
    }
}

pub(crate) fn tool_call_segment_review_error_to_prepare(
    err: review::ToolCallSegmentReviewError,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "protocol_tool_call_segment_review",
        detail: err.to_string(),
    }
}

pub(crate) fn truncate_for_table(text: &str, max_len: usize) -> String {
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

pub(crate) fn truncate_middle(text: &str, max_len: usize) -> String {
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
