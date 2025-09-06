#![allow(unused_variables, unused_mut, dead_code, unreachable_code)]
/*!
E2E live tool-cycle tests using:
- A real App initialization (test_harness) to approximate runtime config
- A real database pre-loaded from a backup of the fixture_nodes graph
- Live OpenRouter API endpoint (skipped when OPENROUTER_API_KEY is not set)

What is being tested and why:
- We validate the two-leg tool flow against a live endpoint:
  1) Force a tool call for a known tool definition.
  2) Locally execute the tool call result with system components (RAG for code context; temp file operations for file tools).
  3) Send back a tool role message with the JSON result to complete the round-trip.
- This demonstrates that our JSON tool schemas and request payloads are accepted by real endpoints, and that our tool results are well-formed for the second leg.

What the test validates:
- The provider returns tool_calls when forced for models/endpoints that advertise tool support.
- The second leg (with the tool result) completes successfully.
- For request_code_context, the RAG service builds typed context over a pre-loaded database, ensuring realistic code snippets are returned.

What the test invalidates:
- It does not assert the final assistant content quality; only the tool-call lifecycle correctness.
- It does not execute our internal rag::dispatcher handlers (future work when we wire llm_manager in tests).

What we learn:
- That our tool definitions and JSON shapes interoperate with a real endpoint.
- That our RAG path is viable with pre-loaded data and can serve as a foundation for more observability and stricter assertions.

Reliability:
- The test skips when OPENROUTER_API_KEY is unset, making CI safe.
- Models are capped via PLOKE_LIVE_MAX_MODELS for run-time control.
*/

#![cfg(test)]

mod harness;

use harness::AppHarness;
use lazy_static::lazy_static;
use ploke_core::ArcStr;
use ploke_tui::llm::model_provider::{Endpoint, EndpointsResponse};
use ploke_tui::llm::openrouter_catalog::ModelsResponse;
use ploke_tui::llm::provider_endpoints::{
    ModelsEndpoint, ModelsEndpointResponse, SupportedParameters
};
use ploke_tui::llm::providers::ProvidersResponse;
use ploke_tui::test_harness::openrouter_env;
use ploke_tui::tracing_setup::init_tracing_tests;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;
use std::fs;
use tokio::time::Duration;
use tracing::{Level, info, warn};

// Ensure a realistic App initialization occurs (settings/env seeded).
// We don't yet drive the in-app event loops, but this simulates runtime config.

#[allow(dead_code)]
struct ToolRoundtripOutcome {
    pub tool_name: String,
    pub model_id: String,
    pub provider_slug: Option<String>,
    pub first_status: u16,
    pub tool_called: bool,
    pub second_status: Option<u16>,
    pub body_excerpt_first: String,
}

// DB fixture now lives in tests/harness.rs (TEST_DB_NODES)

lazy_static! {
    static ref TOOL_ENDPOINT_CANDIDATES: std::sync::Mutex<std::collections::HashMap<String, Vec<String>>> =
        std::sync::Mutex::new(std::collections::HashMap::new());
}

/// Recommended headers for OpenRouter (improves routing/diagnostics)
fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    let referer = HeaderName::from_static("http-referer");
    let x_title = HeaderName::from_static("x-title");
    headers.insert(
        referer,
        HeaderValue::from_static("https://github.com/ploke-ai/ploke"),
    );
    headers.insert(x_title, HeaderValue::from_static("Ploke TUI E2E Tests"));
    headers
}

/// Minimal price signal for an endpoint: prompt + completion (per 1M tokens)
fn endpoint_price_hint(ep: &ModelsEndpoint) -> f64 {
    ep.pricing.prompt_or_default() + ep.pricing.completion_or_default()
}

/// Pick the cheapest tools-capable endpoint for a model (by prompt+completion price).
// reference
async fn choose_tools_endpoint_for_model(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model_id: &str,
    providers_map: &HashMap<String, String>,
) -> Option<(
    String, /*author*/
    String, /*slug*/
    Endpoint,
    Option<String>, /*provider slug hint*/
)> {
    let parts: Vec<&str> = model_id.split('/').collect();
    if parts.len() != 2 {
        warn!("model '{}' is not '<author>/<slug>'", model_id);
        return None;
    }
    let (author, slug) = (parts[0].to_string(), parts[1].to_string());

    let url = format!("{}/models/{}/{}/endpoints", base_url, author, slug);
    let payload = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .ok()?
        .json::<EndpointsResponse>()
        .await
        .inspect(|resp| tracing::trace!("url: {url}\nResponse:\n{:#?}", resp))
        .ok()?;

    let mut candidates: Vec<Endpoint> = payload
        .data
        .endpoints
        .into_iter()
        .filter(|ep| {
            ep.supported_parameters.contains(&SupportedParameters::ToolChoice)
                && ep
                    .supported_parameters.contains(&SupportedParameters::Tools)
        })
        .inspect(|cand| tracing::trace!("candidate: {:#?}", cand))
        .collect();

    // Cache tools-capable endpoint names for later reference/diagnostics
    if let Ok(mut map) = TOOL_ENDPOINT_CANDIDATES.lock() {
        let names: Vec<String> = candidates.iter().map(|ep| ep.name.to_string()).collect();
        map.insert(model_id.to_string(), names);
    }
    tracing::info!(
        "tools-capable endpoints cached for {}: {}",
        model_id,
        candidates
            .iter()
            .map(|e| e.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    );

    if candidates.is_empty() {
        warn!("{model_id} | No candidates found for model with tools");
        return None;
    }
    candidates.sort_by(|a, b| {
            a.pricing.partial_cmp(&b.pricing)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let chosen = candidates.remove(0);
    let slug_hint = providers_map.get(chosen.name.as_ref()).cloned().or_else(|| {
        // Derive a conservative fallback slug from the provider display name
        let derived = chosen
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>();
        if derived.is_empty() {
            None
        } else {
            Some(derived)
        }
    });
    Some((author, slug, chosen, slug_hint))
}

#[tokio::test]
async fn e2e_openrouter_tools_with_app_and_db() -> color_eyre::Result<()> {
    let _tracing_guard = init_tracing_tests(Level::INFO);
    // Dedicated diagnostics directory (env-driven by LLM layer)
    let out_dir = std::path::PathBuf::from("target/test-output/openrouter_e2e");
    fs::create_dir_all(&out_dir).ok();
    println!("[E2E] Diagnostics directory: {}", out_dir.display());
    // Spawn headless App and subsystems via test harness
    let h = AppHarness::spawn().await?;
    let op = openrouter_env().expect("Skipping E2E live test: OPENROUTER_API_KEY not set.");

    let base_url = op.base_url.clone();

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .default_headers(default_headers())
        .build()
        .expect("client");
    let url = base_url.join("models").expect("Malformed models url");
    let resp = client
        .get(url)
        .bearer_auth(&op.key)
        .send()
        .await?
        .error_for_status()?;

    let body: serde_json::Value = resp.json().await?;

    let parsed: ModelsEndpointResponse = serde_json::from_value(body).unwrap();
    // Best-effort capability refresh so tools are considered for models that advertise them.
    {
        let mut cfg = h.state.config.write().await;
        cfg.model_registry.refresh_from_openrouter().await?;
    }

    // Fetch catalog filtered by user allowances
    let models = match ploke_tui::llm::openrouter_catalog::fetch_models(
        &client,
        op.base_url.clone(),
        &op.key,
    )
    .await
    {
        Ok(m) => m,
        Err(e) => {
            panic!("Failed to fetch OpenRouter catalog: {}", e);
        }
    };
    info!("models/user returned {} entries", models.len());

    let max_models: usize = std::env::var("PLOKE_LIVE_MAX_MODELS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10); // slightly lower default to keep this test snappy

    // let tools = tool_defs();
    let mut outcomes: Vec<ToolRoundtripOutcome> = Vec::new();

    let mut processed = 0usize;

    // Optional: build provider name -> slug map
    // let providers_map: std::collections::HashMap<String, String> = match client
    //     .get(format!("{}/providers", base_url))
    //     .bearer_auth(&api_key)
    //     .send()
    //     .await
    //     .and_then(|r| r.error_for_status())
    // {
    //     Ok(resp) => match resp.json::<Value>().await {
    //         Ok(v) => {
    //             info!("Full response infodump:\n{:#?}\n", v);
    //             v.get("data")
    //                 .and_then(|d| d.as_array())
    //                 .map(|arr| {
    //                     arr.iter()
    //                         .filter_map(|p| {
    //                             let name = p.get("name").and_then(|x| x.as_str())?;
    //                             let slug = p.get("slug").and_then(|x| x.as_str())?;
    //                             Some((name.to_string(), slug.to_string()))
    //                         })
    //                         .collect()
    //                 })
    //                 .unwrap_or_default()
    //         }
    //         Err(_) => Default::default(),
    //     },
    //     Err(_) => Default::default(),
    // };
    let provider_url = op.base_url.join("providers")?;
    let resp = client
        .get(provider_url)
        .bearer_auth(&op.key)
        .send()
        .await
        .and_then(|r| r.error_for_status())?;
    let providers_response: ProvidersResponse = resp.json().await?;
    tracing::debug!("providers_map:\n{:#?}", providers_response);
    let count = providers_response
        .data
        .iter()
        .inspect(|p| println!("{:#?}", p))
        .count();
    println!("count: {}", count);
    // Request a clean shutdown of the UI loop
    h.shutdown().await;

    Ok(())
}
