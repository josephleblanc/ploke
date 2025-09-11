

use std::time::Duration;

use reqwest::Client;
use color_eyre::Result;

use crate::llm2::{error::LlmError, router_only::openrouter::OpenRouter};

pub(crate) const MODELS_JSON_RAW: &str = "crates/ploke-tui/data/models/all_raw.json";
pub(crate) const MODELS_JSON_IDS: &str = "crates/ploke-tui/data/models/all_ids.txt";
pub(crate) const ENDPOINTS_JSON_DIR: &str = "crates/ploke-tui/data/endpoints/";
pub(crate) const COMPLETION_JSON_SIMPLE_DIR: &str = "crates/ploke-tui/data/chat_completions/";

use super::*;
async fn simple_query_models() -> Result<()> {
    use ploke_test_utils::workspace_root;

    let url = OpenRouter::MODELS_URL;
    let key = OpenRouter::resolve_api_key()?;

    let response = Client::new()
        .get(url)
        .bearer_auth(key)
        .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| LlmError::Request(e.to_string()))?;

    let response_json = response.text().await?;

    if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
        let mut dir = workspace_root();
        dir.push(MODELS_JSON_RAW);
        println!("Writing '/models' raw response to:\n{}", dir.display());
        std::fs::write(dir, &response_json)?;
    }
    Ok(())
}
