use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelEntry {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub context_length: Option<u32>,
    #[serde(default)]
    pub pricing: Option<ModelPricing>,
    #[serde(default)]
    pub capabilities: Option<ModelCapabilitiesRaw>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelPricing {
    #[serde(default)]
    pub input: Option<f64>,  // USD per 1M input tokens
    #[serde(default)]
    pub output: Option<f64>, // USD per 1M output tokens
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelCapabilitiesRaw {
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
