#![allow(missing_docs)]
//! Curated catalog of sensible default OpenRouter model configurations.
//!
//! Dataflow:
//! - Loaded once into `DEFAULT_MODELS` and merged into `ProviderRegistry` via
//!   `ProviderRegistry::with_defaults`, allowing users to override or disable
//!   entries declaratively in `config.toml`.

use once_cell::sync::Lazy;
use std::collections::HashMap;

use crate::user_config::{ProviderConfig, ProviderType};

/// Curated “starter pack” of popular OpenRouter models.
///
/// These defaults are loaded **once** and then merged with whatever the user
/// supplies in `config.toml`.  Any entry the user defines with the same `id`
/// will *overwrite* the corresponding default, so power-users can tweak or
/// disable individual models without touching code.
#[doc = "Default provider configurations keyed by short id (alias)."]
pub static DEFAULT_MODELS: Lazy<HashMap<String, ProviderConfig>> = Lazy::new(|| {
    let mut m = HashMap::new();

    insert_openrouter(&mut m, "gpt-4o", "openai/gpt-4o", None);
    insert_openrouter(
        &mut m,
        "claude-3-5-sonnet",
        "anthropic/claude-3.5-sonnet",
        None,
    );
    insert_openrouter(
        &mut m,
        "mistral-7b-instruct",
        "mistralai/mistral-7b-instruct",
        None,
    );
    insert_openrouter(&mut m, "qwq-32b-free", "qwen/qwq-32b:free", None);
    insert_openrouter(&mut m, "deepseek-chat", "deepseek/deepseek-chat", None);
    insert_openrouter(
        &mut m,
        "deepseek-chat-v3-0324:free",
        "deepseek/deepseek-chat-v3-0324:free",
        None,
    );
    insert_openrouter(
        &mut m,
        "deepseek-r1-0528:free",
        "deepseek/deepseek-r1-0528:free",
        None,
    );
    insert_openrouter(&mut m, "kimi-k2:free", "moonshotai/kimi-k2:free", None);
    insert_openrouter(&mut m, "kimi-k2", "moonshotai/kimi-k2", None);
    insert_openrouter(&mut m, "gemini-flash", "google/gemini-flash-1.5", None);
    insert_openrouter(
        &mut m,
        "kimi-dev-72b:free",
        "moonshotai/kimi-dev-72b:free",
        None,
    );

    m
});
/// Helper to reduce boilerplate when adding new OpenRouter defaults.
#[inline]
#[doc = "Insert a standard OpenRouter provider configuration into the defaults map."]
fn insert_openrouter(
    map: &mut HashMap<String, ProviderConfig>,
    id: &str,
    model: &str,
    temperature: Option<f32>,
) {
    map.insert(
        id.to_string(),
        ProviderConfig {
            id: id.to_string(),
            api_key: String::new(), // will be filled from env or user config
            provider_slug: None,
            api_key_env: Some("OPENROUTER_API_KEY".to_string()),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            model: model.to_string(),
            display_name: Some(model.to_string()),
            provider_type: ProviderType::OpenRouter,
            llm_params: Some(crate::llm::LLMParameters {
                temperature,
                model: model.to_string(),
                ..Default::default()
            }),
        },
    );
}
