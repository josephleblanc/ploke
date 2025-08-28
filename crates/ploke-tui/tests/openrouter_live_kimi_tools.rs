//! Live tools-capable endpoint smoke test for kimi/kimi-k2 (OpenRouter).
//!
//! Gate with: OPENROUTER_API_KEY and PLOKE_RUN_LIVE_TESTS=1
//! This test selects a tools-capable endpoint for kimi/kimi-k2 and verifies
//! at least one tools-capable endpoint exists and can be listed.

use std::collections::HashSet;

use ploke_tui::{
    llm::provider_endpoints::{ModelEndpointsResponse, SupportedParameters},
    tracing_setup::init_tracing_tests,
};
use reqwest::Client;
use tracing::Level;

#[tokio::test]
async fn live_list_tools_endpoints_kimi_k2() {
    let _g = init_tracing_tests(Level::ERROR);
    if std::env::var("PLOKE_RUN_LIVE_TESTS").ok().as_deref() != Some("1") {
        eprintln!("Skipping: PLOKE_RUN_LIVE_TESTS!=1");
        return;
    }
    // Load API key with .env fallback
    let api_key = match std::env::var("OPENROUTER_API_KEY").ok().filter(|v| !v.trim().is_empty()) {
        Some(v) => v,
        None => {
            let _ = dotenvy::dotenv();
            match std::env::var("OPENROUTER_API_KEY").ok().filter(|v| !v.trim().is_empty()) {
                Some(v) => v,
                None => {
                    eprintln!("Skipping: missing OPENROUTER_API_KEY");
                    return;
                }
            }
        }
    };
    let base = ploke_tui::user_config::OPENROUTER_URL;
    let client = Client::new();
    // List endpoints for kimi/kimi-k2 and ensure at least one supports tools.
    let url = format!("{}/models/{}/endpoints", base, "moonshotai/kimi-k2");
    let resp = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .expect("request endpoints")
        .error_for_status()
        .expect("status ok");
    // Strongly-typed parse and assertions
    let parsed: ModelEndpointsResponse = resp.json().await.expect("typed json endpoints");
    let endpoints = parsed.data.endpoints;
    // Aggregate supported parameters for diagnostics (iterator style, non-consuming)
    let supp_params: HashSet<SupportedParameters> = endpoints
        .iter()
        .flat_map(|ep| ep.supported_parameters.iter().copied())
        .collect();
    tracing::info!("supported_parameters(all endpoints) = {:?}", supp_params);
    let tools_capable = endpoints
        .iter()
        .any(|ep| ep.supported_parameters.iter().any(|p| matches!(p, SupportedParameters::Tools)));
    assert!(tools_capable, "kimi/kimi-k2 should have at least one tools-capable endpoint");
}
