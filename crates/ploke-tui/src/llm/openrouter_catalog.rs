#![allow(missing_docs)]
//! Minimal OpenRouter model catalog client.
//!
//! Dataflow:
//! - At startup and on `/model refresh`, we call `fetch_models` to pull the
//!   OpenRouter `/models` list and cache essential capability/pricing data in
//!   `ProviderRegistry.capabilities`.
//! - The UI surfaces this cache via `model info` and uses it for validation/routing hints.
//!
//! This module intentionally fetches only the minimal subset of fields we need to
//! keep the TUI responsive and reduce payload/deserialize costs.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Deserialize, Debug)]
/// OpenRouter `/models` response container.
struct ModelsResponse {
    /// List of models with minimal fields needed by the TUI.
    data: Vec<ModelEntry>,
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
#[derive(Deserialize, Debug, Clone)]
pub struct TopProviderInfo {
    #[serde(default)]
    pub context_length: Option<u32>,
    #[serde(default)]
    pub max_completion_tokens: Option<u32>,
}

/// Provider-specific entry beneath a model in the catalog.
#[derive(Deserialize, Debug, Clone)]
pub struct ProviderEntry {
    /// Provider identifier (e.g., "openrouter", "openai", or a specific upstream)
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
    base_url: &str,
    api_key: &str,
) -> color_eyre::Result<Vec<ModelEntry>> {
    // Use user-filtered catalog as per product decision.
    let url = format!("{}/models/user", base_url.trim_end_matches('/'));
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
