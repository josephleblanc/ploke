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
use itertools::Itertools;
use lazy_static::lazy_static;
use ploke_error::Error;
use ploke_tui::app_state::StateCommand;
use ploke_tui::llm::provider_endpoints::{ModelEndpoint, ModelEndpointsResponse, SupportedParameters};
use ploke_tui::llm;
use ploke_tui::llm::providers::ProvidersResponse;
use ploke_tui::rag::context::{PROMPT_CODE, PROMPT_HEADER};
use ploke_tui::tracing_setup::init_tracing_tests;
use ploke_tui::user_config::{OPENROUTER_URL, ModelCapabilities};
use ploke_tui::{AppEvent, EventPriority};
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use tokio::time::Duration;
use tracing::{info, warn, Level};
use uuid::Uuid;

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

/// Read OPENROUTER_API_KEY and base URL from environment.
fn openrouter_env() -> Option<(String, String)> {
    // Try current process env first; if missing, load from .env as a fallback
    let key_opt = std::env::var("OPENROUTER_API_KEY").ok();
    let key = match key_opt {
        Some(k) if !k.trim().is_empty() => k,
        _ => {
            let _ = dotenvy::dotenv();
            let k = std::env::var("OPENROUTER_API_KEY").ok()?;
            if k.trim().is_empty() { return None; }
            k
        }
    };
    Some((key, OPENROUTER_URL.to_string()))
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
fn endpoint_price_hint(ep: &ModelEndpoint) -> f64 {
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
    ModelEndpoint,
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
        .json::<ModelEndpointsResponse>()
        .await
        .inspect(|resp| tracing::trace!("url: {url}\nResponse:\n{:#?}", resp))
        .ok()?;

    let mut candidates: Vec<ModelEndpoint> = payload
        .data
        .endpoints
        .into_iter()
        .filter(|ep| {
            ep.supported_parameters
                .iter()
                .any(|p| matches!(p, SupportedParameters::Tools))
                && ep
                    .supported_parameters
                    .iter()
                    .any(|p| matches!(p, SupportedParameters::ToolChoice))
        })
        .inspect(|cand| tracing::trace!("candidate: {:#?}", cand))
        .collect();

    // Cache tools-capable endpoint names for later reference/diagnostics
    if let Ok(mut map) = TOOL_ENDPOINT_CANDIDATES.lock() {
        let names: Vec<String> = candidates.iter().map(|ep| ep.name.clone()).collect();
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
        endpoint_price_hint(a)
            .partial_cmp(&endpoint_price_hint(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let chosen = candidates.remove(0);
    let slug_hint = providers_map.get(&chosen.name).cloned().or_else(|| {
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
    if std::env::var("PLOKE_RUN_LIVE_TESTS").ok().as_deref() != Some("1") {
        eprintln!("Skipping: PLOKE_RUN_LIVE_TESTS!=1");
        return Ok(());
    }
    // Dedicated diagnostics directory (env-driven by LLM layer)
    let out_dir = std::path::PathBuf::from("target/test-output/openrouter_e2e");
    fs::create_dir_all(&out_dir).ok();
    println!("[E2E] Diagnostics directory: {}", out_dir.display());
    // Spawn headless App and subsystems via test harness
    let h = AppHarness::spawn().await;
    // Best-effort capability refresh so tools are considered for models that advertise them.
    {
        let mut cfg = h.state.config.write().await;
        cfg.model_registry.refresh_from_openrouter().await?;
    }

    let Some((api_key, base_url)) = openrouter_env() else {
        eprintln!("Skipping E2E live test: OPENROUTER_API_KEY not set.");
        return Ok(());
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .default_headers(default_headers())
        .build()
        .expect("client");

    // Fetch catalog filtered by user allowances
    let models = match ploke_tui::llm::openrouter_catalog::fetch_models(
        &client, &base_url, &api_key,
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
    let resp = client
        .get(format!("{}/providers", base_url))
        .bearer_auth(&api_key)
        .send()
        .await
        .and_then(|r| r.error_for_status())?;
    let providers_response: ProvidersResponse = resp.json().await?;
    tracing::debug!("providers_map:\n{:#?}", providers_response);
    let count = providers_response.data.iter()
        .inspect(|p| println!("{}", p.slug) )
        .count();
    println!("count: {}", count);

    panic!();

//     for m in models.into_iter().take(max_models) {
//         // if processed >= max_models {
//         //     break;
//         // }
//
//         let model_id = m.id;
//         info!("model: {}", model_id);
//
//         let chosen = choose_tools_endpoint_for_model(
//             &client,
//             &base_url,
//             &api_key,
//             &model_id,
//             &providers_map,
//         )
//         .await;
//         let Some((_author, _slug, endpoint, provider_slug_hint)) = chosen else {
//             info!("  no tools-capable endpoints; skipping {}", model_id);
//             processed += 1;
//             continue;
//         };
//
//         // Force-enable tool support in the registry for the selected model so tools are included.
//         {
//             let mut cfg = h.state.config.write().await;
//             cfg.model_registry.capabilities.insert(
//                 model_id.clone(),
//                 ModelCapabilities {
//                     supports_tools: true,
//                     context_length: Some(endpoint.context_length as u32),
//                     input_cost_per_million: None,
//                     output_cost_per_million: None,
//                 },
//             );
//         }
//
//         tracing::trace!(
//             "  chosen endpoint: provider='{}' context_length={} price_hint={:.8}",
//             endpoint.name,
//             endpoint.context_length,
//             endpoint_price_hint(&endpoint)
//         );
//
//         // Record the model/endpoint choice for summary/diagnostics
//         outcomes.push(ToolRoundtripOutcome {
//             tool_name: "endpoint_choice".to_string(),
//             model_id: model_id.clone(),
//             provider_slug: provider_slug_hint.clone(),
//             first_status: 0,
//             tool_called: false,
//             second_status: None,
//             body_excerpt_first: format!("chosen endpoint: {}", endpoint.name),
//         });
//
//         // Configure the active provider/model for this loop iteration
//         if let Some(provider_slug_hint) = provider_slug_hint.clone() {
//             let _ = h.cmd_tx
//                 .send(StateCommand::SelectModelProvider {
//                     model_id: model_id.clone(),
//                     provider_id: provider_slug_hint.clone(),
//                 })
//                 .await;
//             // Allow the dispatcher to apply the change
//             tokio::time::sleep(Duration::from_millis(100)).await;
//         }
//
//         // Submit a realistic user request to drive the full lifecycle
//         let request_id = Uuid::new_v4();
//         let system_instr = [PROMPT_HEADER, PROMPT_CODE].join("");
//         let user_instr = String::from(
// "Hello, I would like you to help me understand the difference between the SimpleStruct and the GenericStruct in my code.
//
// If tools are available, you MUST call the `request_code_context` tool with {\"token_budget\": 256} and wait for the tool result before responding.",
//         );
//         let parent_id = h.add_user_msg(user_instr.clone()).await;
//
//         // Observe LLM and Tool events with a shorter window; break early on first tool signal
//         let mut bg_rx = h.event_bus.subscribe(EventPriority::Background);
//         let mut rt_rx = h.event_bus.subscribe(EventPriority::Realtime);
//         let observe_until = std::time::Instant::now() + Duration::from_secs(10);
//         let mut saw_tool = false;
//         while std::time::Instant::now() < observe_until {
//             let next_bg = tokio::time::timeout(Duration::from_millis(1000), bg_rx.recv());
//             let next_rt = tokio::time::timeout(Duration::from_millis(1000), rt_rx.recv());
//             tokio::select! {
//                 Ok(Ok(ev)) = next_rt => match ev {
//                     AppEvent::LlmTool(llm::ToolEvent::Requested { name, call_id, .. }) => {
//                         println!("[E2E] tool_requested model={} name={} call_id={}", model_id, name, call_id);
//                         saw_tool = true; break;
//                     }
//                     AppEvent::LlmTool(llm::ToolEvent::Completed { call_id, content, .. }) => {
//                         let excerpt: String = content.chars().take(200).collect();
//                         println!("[E2E] tool_completed model={} call_id={} excerpt={}", model_id, call_id, excerpt);
//                         saw_tool = true; break;
//                     }
//                     AppEvent::LlmTool(llm::ToolEvent::Failed { call_id, error, .. }) => {
//                         println!("[E2E] tool_failed model={} call_id={} error={}", model_id, call_id, error);
//                         saw_tool = true; break;
//                     }
//                     _ => {}
//                 },
//                 Ok(Ok(ev)) = next_bg => match ev {
//                     AppEvent::Llm(llm::Event::ToolCall {
//                         name,
//                         ..
//                     }) => {
//
//                         saw_tool = true;
//                         break;
//                     }
//                     AppEvent::Llm(llm::Event::Response { content, model, .. }) => {
//                         let excerpt: String = content.chars().take(180).collect();
//                         println!("[E2E] response model={} excerpt={}", model, excerpt);
//                     }
//                     _ => {}
//                 },
//                 _ = tokio::time::sleep(Duration::from_millis(100)) => {}
//             }
//         }
//
//         println!(
//             "[E2E] summary model={} provider_hint={} saw_tool={}",
//             model_id,
//             provider_slug_hint.clone().unwrap_or_else(|| "-".into()),
//             saw_tool
//         );
//
//         outcomes.push(ToolRoundtripOutcome {
//             tool_name: "request_code_context".to_string(),
//             model_id: model_id.clone(),
//             provider_slug: provider_slug_hint.clone(),
//             first_status: 0,
//             tool_called: saw_tool,
//             second_status: None,
//             body_excerpt_first: "event observation complete".to_string(),
//         });
//
//         processed += 1;
//
//         // Stop early once we observe at least one tool call to keep run time reasonable.
//         if saw_tool {
//             break;
//         }
//     }
//
//     // Persist discovered tools-capable endpoints for diagnostics
//     if let Ok(map) = TOOL_ENDPOINT_CANDIDATES.lock() {
//         let path = out_dir.join("openrouter_tools_candidates.json");
//         let _ = fs::write(&path, serde_json::to_string_pretty(&*map).unwrap_or_default());
//         println!("[E2E] wrote tools-capable endpoint summary to {}", path.display());
//     }
//
//     // Summary of outcomes across models/tools
//     let total = outcomes.len();
//     let successes = outcomes
//         .iter()
//         .filter(|o| o.tool_called && matches!(o.second_status, Some(s) if (200..=299).contains(&s)))
//         .count();
//     let no_tool_calls = outcomes.iter().filter(|o| !o.tool_called).count();
//     let first_404 = outcomes.iter().filter(|o| o.first_status == 404).count();
//     let any_429 = outcomes
//         .iter()
//         .filter(|o| o.first_status == 429 || matches!(o.second_status, Some(429)))
//         .count();
//     info!(
//         "Summary: total_outcomes={} successes={} no_tool_calls={} http_404_first_leg={} http_429_any_leg={}",
//         total, successes, no_tool_calls, first_404, any_429
//     );
//
//     assert!(
//         outcomes.iter().any(|o| o.tool_called),
//         "No tool calls were observed across evaluated models. Ensure OPENROUTER_API_KEY is set and at least one tools-capable endpoint is selected."
//     );

    // Request a clean shutdown of the UI loop
    h.shutdown().await;

    Ok(())
}
