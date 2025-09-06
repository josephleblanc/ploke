#![allow(missing_docs)]
//! Minimal OpenRouter model catalog client.
//!
//! Dataflow:
//! - At startup and on `/model refresh`, we call `fetch_models` to pull the
//!   OpenRouter `/models` list and cache essential capability/pricing data in
//!   `ModelRegistry.capabilities`.
//! - The UI surfaces this cache via `model info` and uses it for validation/routing hints.
//!
//! This module intentionally fetches only the minimal subset of fields we need to
//! keep the TUI responsive and reduce payload/deserialize costs.

use crate::utils::se_de::string_or_f64;
use crate::utils::se_de::string_or_f64_opt;
use crate::utils::se_de::string_to_f64_opt_zero;
use ploke_core::ArcStr;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::model_provider as or_ep;
use super::model_provider::Endpoint;
use super::model_provider::EndpointData;
use super::provider_endpoints::ModelsEndpoint;
use super::provider_endpoints::ModelsEndpointsData;
use super::provider_endpoints::SupportedParameters;
use super::provider_endpoints::SupportsTools;
use super::providers::Provider;
use super::providers::ProviderName;

#[derive(Deserialize, Debug)]
/// OpenRouter `https://openrouter.ai/api/v1/models` response container.
pub struct ModelsResponse {
    /// List of models with minimal fields needed by the TUI.
    pub data: Vec<ModelEntry>,
}

#[derive(Deserialize, Debug, Clone, PartialOrd, PartialEq, Serialize)]
/// Represents a single model entry.
///
pub struct ModelEntry {
    /// Canonical model identifier, e.g. "openai/gpt-4o".
    pub id: String,
    /// Optional human-friendly display name.
    #[serde(default)]
    pub name: Option<String>,
    /// Context window size if known (model-level).
    #[serde(default)]
    pub context_length: Option<u32>,
    /// Top provider info (often carries context length when model-level is missing).
    #[serde(default)]
    pub top_provider: Option<TopProviderInfo>,
    /// Input/output pricing; maps from OpenRouter's prompt/completion when present.
    #[serde(default)]
    pub pricing: Option<ModelPricing>,
    /// OpenRouter model-level "supported_parameters" (e.g., includes "tools" when tool-calling is supported).
    /// e.g. frequency_penalty, logit_bias, max_tokens, min_p, presence_penalty,
    /// repetition_penalty, stop, temperature, tool_choice, tools, top_k, top_p
    #[serde(default)]
    pub supported_parameters: Option<Vec<SupportedParameters>>,
    /// Provider-specific entries under this model (pricing, tools, context).
    #[serde(default)]
    pub providers: Option<Vec<Provider>>,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Serialize, Deserialize)]
/// Pricing information for a model.
/// USD per token
pub struct ModelPricing {
    // Price per token in USD for system(?) prompt
    // All models at https://openrouter.ai/api/v1/models have this (323/323 tested)
    #[serde(default, deserialize_with = "string_or_f64")]
    pub prompt: f64,
    // Price per token in USD for generated tokens
    // All models at https://openrouter.ai/api/v1/models have this (323/323 tested)
    #[serde(default, deserialize_with = "string_or_f64")]
    pub completion: f64,
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub audio: Option<f64>,
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub image: Option<f64>,
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub input_cache_read: Option<f64>,
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub input_cache_write: Option<f64>,
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub internal_reasoning: Option<f64>,
    // Price per token in USD for system(?) prompt
    // Most have this, 322/323 have it, so all but one
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub request: Option<f64>,
    // Again all but one have this
    #[serde(
        default,
        deserialize_with = "string_or_f64_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub web_search: Option<f64>,
    #[serde(
        default,
        deserialize_with = "string_to_f64_opt_zero",
        skip_serializing_if = "Option::is_none"
    )]
    pub discount: Option<f64>,
}

impl Default for ModelPricing {
    fn default() -> Self {
        Self {
            prompt: 0.0,
            completion: 0.0,
            audio: None,
            image: None,
            input_cache_read: None,
            input_cache_write: None,
            internal_reasoning: None,
            request: None,
            web_search: None,
            discount: None,
        }
    }
}

impl ModelPricing {
    pub fn prompt_or_default(&self) -> f64 {
        self.prompt
    }
    pub fn completion_or_default(&self) -> f64 {
        self.completion
    }
}

#[derive(Deserialize, Debug, Clone, PartialOrd, PartialEq, Serialize)]
/// Capability flags as exposed by OpenRouter.
/// We preserve raw structure to avoid overfitting to current fields.
pub struct ModelCapabilitiesRaw {
    /// Whether the model supports tool/function calling.
    #[serde(default)]
    pub tools: Option<bool>,
}

/// Provider info commonly exposed by OpenRouter for the "top_provider" field.
#[derive(Deserialize, Debug, Clone, Copy, PartialOrd, PartialEq, Serialize)]
pub struct TopProviderInfo {
    #[serde(default)]
    pub is_moderated: Option<bool>,
    #[serde(default)]
    pub context_length: Option<u32>,
    #[serde(default)]
    pub max_completion_tokens: Option<u32>,
}

/// Provider-specific entry beneath a model in the catalog.
/// While not `Copy`, this is cheap to clone due to `ArcStr` being a convenience struct for
/// `Arc<str>`, making this an inexpensive way to show the summary of model endpoint info before
/// fetching more details (either locally or from the API) if needed.
#[derive(Deserialize, Debug, Clone, PartialOrd, PartialEq, Serialize)]
pub struct ProviderSummary {
    /// Human-friendly name in the form "Provider | model id"
    /// e.g. "name": "DeepInfra | deepseek/deepseek-chat-v3.1",
    pub ep_name: ArcStr,
    /// Context length for this provider if known
    #[serde(default)]
    pub ep_context_length: u32,
    /// Pricing for this provider/model combo. Accepts {input,output} or {prompt,completion}.
    #[serde(default)]
    pub ep_pricing_prompt: f64,
    /// Pricing for this provider/model combo. Accepts {input,output} or {prompt,completion}.
    #[serde(default)]
    pub ep_pricing_completion: f64,
    /// Capability flags; many providers expose tools here
    #[serde(default)]
    pub tool_use: bool,
}

impl ProviderSummary {
    pub fn from_endpoint(ep: &Endpoint) -> Self {
        let Endpoint {
            name,
            context_length,
            pricing,
            supported_parameters,
            ..
        } = ep;
        Self {
            ep_name: name.clone(),
            ep_context_length: *context_length as u32,
            ep_pricing_prompt: pricing.prompt,
            ep_pricing_completion: pricing.completion,
            tool_use: supported_parameters.supports_tools(),
        }
    }
}

/// Fetch the list of available models from OpenRouter with minimal fields needed
/// for capability and pricing awareness.
pub async fn fetch_models(
    client: &reqwest::Client,
    base_url: reqwest::Url,
    api_key: &str,
) -> color_eyre::Result<Vec<ModelsEndpoint>> {
    // Use public models endpoint (same as working tests)
    let url = base_url
        .join("models") // must be "models", not "/models"
        .expect("Malformed models url");
    let resp = client
        .get(url)
        .bearer_auth(api_key)
        .send()
        .await?
        .error_for_status()?;

    let body = resp.text().await?;
    let parsed: ModelsEndpointsData= serde_json::from_str(&body)?;
    Ok(parsed.data)
}

/// Some catalog IDs include a variant suffix (e.g., ":free") that is not accepted by the
/// `/models/:author/:slug/endpoints` path. Strip any suffix after ':'.
fn canonicalize_model_id_for_endpoints(model_id: &str) -> String {
    model_id.split(':').next().unwrap_or(model_id).to_string()
}

/// Fetch provider endpoints for a specific model (author/slug).
pub async fn fetch_model_endpoints(
    client: &reqwest::Client,
    base_url: reqwest::Url,
    api_key: &str,
    model_id: &str,
) -> color_eyre::Result<EndpointData> {
    let model_path = canonicalize_model_id_for_endpoints(model_id);
    let (author, slug) = model_path
        .split_once('/')
        .ok_or_else(|| color_eyre::eyre::eyre!("invalid model id, expected 'author/slug'"))?;

    let url = base_url
        .join(&format!("models/{}/{}/endpoints", author, slug))
        .map_err(|e| color_eyre::eyre::eyre!("Malformed url: {}", e))?;

    let resp = client
        .get(url)
        .bearer_auth(api_key)
        .header("Accept", "application/json")
        .header("HTTP-Referer", "https://github.com/ploke-ai/ploke")
        .header("X-Title", "Ploke TUI")
        .send()
        .await?
        .error_for_status()?;

    // let body = resp.json().await?;
    // Parse with stronger typed endpoint shape first
    let parsed: or_ep::EndpointsResponse = serde_json::from_value(resp.json().await?)?;


    Ok(parsed.data)
}
