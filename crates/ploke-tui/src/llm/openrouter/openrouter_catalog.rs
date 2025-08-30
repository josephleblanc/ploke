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

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use crate::utils::de::string_or_f64_opt;
use crate::utils::de::string_or_f64;

use super::provider_endpoints::SupportedParameters;

#[derive(Deserialize, Debug)]
/// OpenRouter `/models` response container.
pub struct ModelsResponse {
    /// List of models with minimal fields needed by the TUI.
    pub data: Vec<ModelEntry>,
}

#[derive(Deserialize, Debug, Clone, PartialOrd, PartialEq, Serialize)]
/// Represents a single model endpoint from OpenRouter's API.
///
/// After doing some analysis on the data on Aug 29, 2025, the following fields have some nuance:
///     - hugging_face_id: missing for 43/323 models
///     - top_provider.max_completion_tokens: missing ~half the time, 151/323
///     - architecture.instruct_type: missing for most (~65%), 208/323
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
    /// Raw capability flags (currently tools). Note: many models expose "supported_parameters" instead.
    #[serde(default)]
    pub capabilities: Option<ModelCapabilitiesRaw>,
    /// OpenRouter model-level "supported_parameters" (e.g., includes "tools" when tool-calling is supported).
    /// e.g. frequency_penalty, logit_bias, max_tokens, min_p, presence_penalty,
    /// repetition_penalty, stop, temperature, tool_choice, tools, top_k, top_p
    #[serde(default)]
    pub supported_parameters: Option<Vec<SupportedParameters>>,
    /// Provider-specific entries under this model (pricing, tools, context).
    #[serde(default)]
    pub providers: Option<Vec<ProviderEntry>>,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Serialize, Deserialize)]
/// Pricing information for a model.
/// USD per token
pub struct ModelPricing {
    #[serde(deserialize_with = "string_or_f64_opt", skip_serializing_if = "Option::is_none")]
    pub audio: Option<f64>,
    // Price per token in USD for generated tokens
    // All models at https://openrouter.ai/api/v1/models have this (323/323 tested)
    #[serde(deserialize_with = "string_or_f64")]
    pub completion: f64,
    #[serde(deserialize_with = "string_or_f64_opt", skip_serializing_if = "Option::is_none")]
    pub image: Option<f64>,
    #[serde(deserialize_with = "string_or_f64_opt", skip_serializing_if = "Option::is_none")]
    pub input_cache_read: Option<f64>,
    #[serde(deserialize_with = "string_or_f64_opt", skip_serializing_if = "Option::is_none")]
    pub input_cache_write: Option<f64>,
    #[serde(deserialize_with = "string_or_f64_opt", skip_serializing_if = "Option::is_none")]
    pub internal_reasoning: Option<f64>,
    // Price per token in USD for system(?) prompt
    // All models at https://openrouter.ai/api/v1/models have this (323/323 tested)
    #[serde(deserialize_with = "string_or_f64")]
    pub prompt: f64,
    // Price per token in USD for system(?) prompt
    // Most have this, 322/323 have it, so all but one
    #[serde(deserialize_with = "string_or_f64_opt", skip_serializing_if = "Option::is_none")]
    pub request: Option<f64>,
    // Again all but one have this
    #[serde(deserialize_with = "string_or_f64_opt", skip_serializing_if = "Option::is_none")]
    pub web_search: Option<f64>,
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
#[derive(Deserialize, Debug, Clone, PartialOrd, PartialEq, Serialize)]
pub struct ProviderEntry {
    /// Provider identifier (varies by endpoint shape).
    /// Endpoints may expose "id", "provider", "slug", or a human-readable "name".
    // WARN: Not sure about the above claim, try deleting this and test again.
    #[serde(
        default,
        alias = "provider",
        alias = "name",
        alias = "slug",
        alias = "provider_slug"
    )]
    pub id: String,
    /// Context length for this provider if known
    #[serde(default)]
    pub context_length: Option<u32>,
    /// Pricing for this provider/model combo. Accepts {input,output} or {prompt,completion}.
    #[serde(default)]
    pub pricing: Option<ModelPricing>,
    /// Capability flags; many providers expose tools here
    #[serde(default)]
    pub capabilities: Option<ModelCapabilitiesRaw>,
    /// OpenRouter "supported_parameters" array for this provider (e.g., contains "tools")
    #[serde(default)]
    pub supported_parameters: Option<Vec<String>>,
}

/// Fetch the list of available models from OpenRouter with minimal fields needed
/// for capability and pricing awareness.
pub async fn fetch_models(
    client: &reqwest::Client,
    base_url: reqwest::Url,
    api_key: &str,
) -> color_eyre::Result<Vec<ModelEntry>> {
    // Use user-filtered catalog as per product decision.
    let url = base_url
        .join("/models/user")
        .expect("Malformed model/user url");
    let resp = client
        .get(url)
        .bearer_auth(api_key)
        .send()
        .await?
        .error_for_status()?;

    let body = resp.text().await?;
    let parsed: ModelsResponse = serde_json::from_str(&body)?;
    Ok(parsed.data)
}

/// OpenRouter `/models/:author/:slug/endpoints` for provider-level details.
#[derive(Deserialize, Debug)]
struct EndpointsResponse {
    data: Vec<ProviderEntry>,
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
) -> color_eyre::Result<Vec<ProviderEntry>> {
    let model_path = canonicalize_model_id_for_endpoints(model_id);
    let url = base_url
        .join("/models/user")
        .and_then(|u| u.join(model_id))
        .and_then(|u| u.join("endpoints"))
        .expect("Malformed url");
    let resp = client
        .get(url)
        .bearer_auth(api_key)
        .header("Accept", "application/json")
        .header("HTTP-Referer", "https://github.com/ploke-ai/ploke")
        .header("X-Title", "Ploke TUI")
        .send()
        .await?
        .error_for_status()?;

    let body = resp.text().await?;
    let parsed: EndpointsResponse = serde_json::from_str(&body)?;
    Ok(parsed.data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    #[cfg(feature = "xxx")]
    fn test_pricing_deser_from_strings() {
        let body = r#"{
            "data": [
                {
                    "id": "openai",
                    "context_length": 128000,
                    "pricing": { "prompt": "0.000001", "completion": "0.000002" },
                    "supported_parameters": ["tools"],
                    "capabilities": { "tools": true }
                }
            ]
        }"#;
        let parsed: EndpointsResponse = serde_json::from_str(body).expect("parse endpoints");
        assert_eq!(parsed.data.len(), 1);
        let p = &parsed.data[0];
        assert_eq!(p.id, "openai");
        let pr = p.pricing.as_ref().expect("pricing exists");
        assert!((pr.input.unwrap() - 0.000001).abs() < 1e-12);
        assert!((pr.output.unwrap() - 0.000002).abs() < 1e-12);
        assert_eq!(p.supported_parameters.as_ref().unwrap()[0], "tools");
        assert_eq!(p.capabilities.as_ref().unwrap().tools, Some(true));
    }

    #[test]
    #[cfg(feature = "xxx")]
    fn test_pricing_deser_from_numbers() {
        let body = r#"{
            "data": [
                {
                    "id": "novita",
                    "context_length": 64000,
                    "pricing": { "input": 0.000003, "output": 0.000004 }
                }
            ]
        }"#;
        let parsed: EndpointsResponse = serde_json::from_str(body).expect("parse endpoints");
        let p = &parsed.data[0];
        let pr = p.pricing.as_ref().unwrap();
        assert!((pr.input.unwrap() - 0.000003).abs() < 1e-12);
        assert!((pr.output.unwrap() - 0.000004).abs() < 1e-12);
    }

    #[test]
    fn test_alias_id_from_name() {
        // Some endpoints respond with "name" (human-readable) instead of "id"/"provider"
        let body = r#"{
            "data": [
                {
                    "name": "OpenAI",
                    "context_length": 128000,
                    "pricing": { "prompt": "0.000001", "completion": "0.000002" }
                }
            ]
        }"#;
        let parsed: EndpointsResponse = serde_json::from_str(body).expect("parse endpoints");
        assert_eq!(parsed.data[0].id, "OpenAI");
    }

    #[test]
    fn test_canonicalize_model_id_for_endpoints() {
        assert_eq!(
            canonicalize_model_id_for_endpoints("deepseek/deepseek-r1-0528-qwen3-8b:free"),
            "deepseek/deepseek-r1-0528-qwen3-8b"
        );
        assert_eq!(
            canonicalize_model_id_for_endpoints("qwen/qwen-2.5-72b-instruct"),
            "qwen/qwen-2.5-72b-instruct"
        );
    }
}
