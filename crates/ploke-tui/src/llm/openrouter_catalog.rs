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

use serde::Deserialize;

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
    /// Context window size if known.
    #[serde(default)]
    pub context_length: Option<u32>,
    /// Input/output pricing (USD per 1M tokens).
    #[serde(default)]
    pub pricing: Option<ModelPricing>,
    /// Raw capability flags (currently tools).
    #[serde(default)]
    pub capabilities: Option<ModelCapabilitiesRaw>,
}

#[derive(Deserialize, Debug, Clone)]
/// Pricing information for a model.
pub struct ModelPricing {
    /// USD per 1M input tokens
    #[serde(default)]
    pub input: Option<f64>,  // USD per 1M input tokens
    /// USD per 1M output tokens
    #[serde(default)]
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

/// Fetch the list of available models from OpenRouter with minimal fields needed
/// for capability and pricing awareness.
pub async fn fetch_models(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> color_eyre::Result<Vec<ModelEntry>> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
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
