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

use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Deserialize, Debug)]
/// OpenRouter `/models` response container.
pub struct ModelsResponse {
    /// List of models with minimal fields needed by the TUI.
    pub data: Vec<ModelEntry>,
}

#[derive(Deserialize, Debug, Clone)]
/// Minimal per-model record returned by OpenRouter.
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
    #[serde(default, deserialize_with = "de_optional_model_pricing")]
    pub pricing: Option<ModelPricing>,
    /// Raw capability flags (currently tools). Note: many models expose "supported_parameters" instead.
    #[serde(default)]
    pub capabilities: Option<ModelCapabilitiesRaw>,
    /// OpenRouter model-level "supported_parameters" (e.g., includes "tools" when tool-calling is supported).
    #[serde(default)]
    pub supported_parameters: Option<Vec<String>>,
    /// Provider-specific entries under this model (pricing, tools, context).
    #[serde(default)]
    pub providers: Option<Vec<ProviderEntry>>,
}

#[derive(Debug, Clone)]
/// Pricing information for a model.
pub struct ModelPricing {
    /// USD per 1M input tokens
    pub input: Option<f64>, // USD per 1M input tokens
    /// USD per 1M output tokens
    pub output: Option<f64>, // USD per 1M output tokens
}

#[derive(Deserialize, Debug, Clone)]
/// Capability flags as exposed by OpenRouter.
/// We preserve raw structure to avoid overfitting to current fields.
pub struct ModelCapabilitiesRaw {
    /// Whether the model supports tool/function calling.
    #[serde(default)]
    pub tools: Option<bool>,
}

/// Provider info commonly exposed by OpenRouter for the "top_provider" field.
#[derive(Deserialize, Debug, Clone, Copy, PartialOrd, PartialEq)]
pub struct TopProviderInfo {
    #[serde(default)]
    pub is_moderated: Option<bool>,
    #[serde(default)]
    pub context_length: Option<u32>,
    #[serde(default)]
    pub max_completion_tokens: Option<u32>,
}

/// Provider-specific entry beneath a model in the catalog.
#[derive(Deserialize, Debug, Clone)]
pub struct ProviderEntry {
    /// Provider identifier (varies by endpoint shape).
    /// Endpoints may expose "id", "provider", "slug", or a human-readable "name".
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
    #[serde(default, deserialize_with = "de_optional_model_pricing")]
    pub pricing: Option<ModelPricing>,
    /// Capability flags; many providers expose tools here
    #[serde(default)]
    pub capabilities: Option<ModelCapabilitiesRaw>,
    /// OpenRouter "supported_parameters" array for this provider (e.g., contains "tools")
    #[serde(default)]
    pub supported_parameters: Option<Vec<String>>,
}

/// Custom deserializer for optional ModelPricing supporting either
/// {input, output} or OpenRouter's {prompt, completion} string fields.
fn de_optional_model_pricing<'de, D>(deserializer: D) -> Result<Option<ModelPricing>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    if let Some(v) = value {
        if let Some(obj) = v.as_object() {
            // Attempt to read input/output directly (f64 or string)
            let mut input: Option<f64> = None;
            let mut output: Option<f64> = None;

            if let Some(iv) = obj.get("input") {
                input = iv
                    .as_f64()
                    .or_else(|| iv.as_str().and_then(|s| s.parse::<f64>().ok()));
            }
            if let Some(ov) = obj.get("output") {
                output = ov
                    .as_f64()
                    .or_else(|| ov.as_str().and_then(|s| s.parse::<f64>().ok()));
            }

            // Fallback to prompt/completion keys used by OpenRouter
            if input.is_none() {
                if let Some(pv) = obj.get("prompt") {
                    input = pv
                        .as_f64()
                        .or_else(|| pv.as_str().and_then(|s| s.parse::<f64>().ok()));
                }
            }
            if output.is_none() {
                if let Some(cv) = obj.get("completion") {
                    output = cv
                        .as_f64()
                        .or_else(|| cv.as_str().and_then(|s| s.parse::<f64>().ok()));
                }
            }

            return Ok(Some(ModelPricing { input, output }));
        }
    }
    Ok(None)
}

/// Fetch the list of available models from OpenRouter with minimal fields needed
/// for capability and pricing awareness.
pub async fn fetch_models(
    client: &reqwest::Client,
    base_url: reqwest::Url,
    api_key: &str,
) -> color_eyre::Result<Vec<ModelEntry>> {
    // Use user-filtered catalog as per product decision.
    let url = base_url.join("/models/user").expect("Malformed model/user url");
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
    let url = base_url.join("/models/user")
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
