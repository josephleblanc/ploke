use std::time::Duration;

use color_eyre::Result;
use reqwest::Client;

use crate::{error::LlmError, router_only::openrouter::OpenRouter};

// TODO:ploke-llm
// These are these same files as in the original module for `llm`, and not oriented around
// `ploke-llm`, which will require either adding some helper test functions to populate them,
// adding xtask methods to run the fixtures, or something else. It would be bad practice to leave
// these as is, since the generation of test fixtures would benefit from being self-contained
// within ploke-llm.
pub(crate) const MODELS_JSON_RAW: &str = "crates/ploke-tui/data/models/all_raw.json";
pub(crate) const MODELS_JSON_RAW_PRETTY: &str = "crates/ploke-tui/data/models/all_raw_pretty.json";
pub(crate) const MODELS_JSON_PARSED: &str = "crates/ploke-tui/data/models/all_parsed.json";
pub(crate) const MODELS_TXT_IDS: &str = "crates/ploke-tui/data/models/all_ids_parsed.txt";
pub(crate) const MODELS_TXT_CANON: &str = "crates/ploke-tui/data/models/all_canon_parsed.txt";
pub(crate) const MODELS_JSON_ARCH: &str = "crates/ploke-tui/data/models/all_arch_parsed.json";
pub(crate) const MODELS_JSON_TOP: &str = "crates/ploke-tui/data/models/all_top_parsed.json";
pub(crate) const MODELS_JSON_PRICING: &str = "crates/ploke-tui/data/models/all_pricing_parsed.json";
pub(crate) const MODELS_JSON_SUPPORTED: &str =
    "crates/ploke-tui/data/models/all_supported_parsed.json";
pub(crate) const MODELS_JSON_ID_NOT_NAME: &str = "crates/ploke-tui/data/models/id_not_name.json";
pub(crate) const ENDPOINTS_JSON_DIR: &str = "crates/ploke-tui/data/endpoints/";
pub(crate) const COMPLETION_JSON_SIMPLE_DIR: &str = "crates/ploke-tui/data/chat_completions/";

use super::*;

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/ploke-llm -> crates -> workspace root
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or(manifest_dir)
}

async fn simple_query_models() -> Result<()> {
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

#[cfg(test)]
pub(crate) mod test_data {
    use once_cell::sync::Lazy;

    use super::workspace_root;

    use super::{MODELS_JSON_ARCH, MODELS_JSON_PRICING, MODELS_TXT_IDS};

    pub(crate) static MODELS_IDS_TEXT_LAZY: Lazy<String> = Lazy::new(|| {
        let mut p = workspace_root();
        p.push(MODELS_TXT_IDS);
        std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", p.display(), e))
    });

    pub(crate) static MODELS_ARCH_JSON_LAZY: Lazy<String> = Lazy::new(|| {
        let mut p = workspace_root();
        p.push(MODELS_JSON_ARCH);
        std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", p.display(), e))
    });

    pub(crate) static MODELS_PRICING_JSON_LAZY: Lazy<String> = Lazy::new(|| {
        let mut p = workspace_root();
        p.push(MODELS_JSON_PRICING);
        std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", p.display(), e))
    });
}
