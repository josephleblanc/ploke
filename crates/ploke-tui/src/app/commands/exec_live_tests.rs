#![cfg(test)]

use serde_json::json;
use serde::{Serialize, Deserialize};
use tracing::{info, warn, instrument};

use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::time::{Duration, Instant};

use crate::llm::provider_endpoints::ModelEndpointsResponse;
use crate::tracing_setup::init_tracing;
use crate::user_config::OPENROUTER_URL;
use crate::llm::openrouter_catalog;

// Leverage the in-crate test harness (Arc<Mutex<App>>) for constructing a realistic App.
use crate::test_harness::app;

/// Helper to resolve API key and base URL for OpenRouter.
fn openrouter_env() -> Option<(String, String)> {
    let key = std::env::var("OPENROUTER_API_KEY").ok()?;
    if key.trim().is_empty() {
        return None;
    }
    Some((key, OPENROUTER_URL.to_string()))
}

/// Default OpenRouter-recommended headers for telemetry and routing.
fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    // OpenRouter recommends sending these headers to help with routing and abuse prevention.
    // They are not strictly required, but some providers behave better with them.
    let referer = HeaderName::from_static("http-referer");
    let x_title = HeaderName::from_static("x-title");
    headers.insert(referer, HeaderValue::from_static("https://github.com/ploke-ai/ploke"));
    headers.insert(x_title, HeaderValue::from_static("Ploke TUI Tests"));
    headers
}

/// Choose a tools-capable model:
/// 1) If a preferred model is provided and advertises "tools" via the endpoints API, use it.
/// 2) Otherwise, scan /models/user and pick the first model whose supported_parameters includes "tools".
/// 3) Fallback to "google/gemini-2.0-flash-001" if nothing else is found.
async fn choose_tools_model(
    client: &Client,
    base_url: &str,
    api_key: &str,
    preferred: Option<&str>,
) -> String {

    if let Some(pref) = preferred {
        // Inline probe of endpoints to see if preferred model advertises "tools"
        let parts: Vec<&str> = pref.split('/').collect();
        if parts.len() == 2 {
            let author = parts[0];
            let slug = parts[1];
            let url = format!("{}/models/{}/{}/endpoints", base_url, author, slug);
            let supports = match client
                .get(&url)
                .bearer_auth(api_key)
                .send()
                .await
                .and_then(|r| r.error_for_status())
            {
                Ok(resp) => {
                    if let Ok(text) = resp.text().await {
                        if let Ok(parsed) = serde_json::from_str::<ModelEndpointsResponse>(&text) {
                            parsed
                                .data
                                .endpoints
                                .iter()
                                .any(|ep| ep.supported_parameters.iter().any(|p| p.eq_ignore_ascii_case("tools")))
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                Err(_) => false,
            };

            if supports {
                info!("choose_tools_model: using preferred model '{}'", pref);
                return pref.to_string();
            } else {
                warn!("choose_tools_model: preferred model '{}' did not advertise tools; probing catalog for alternatives", pref);
            }
        } else {
            warn!("choose_tools_model: preferred model '{}' invalid; expected '<author>/<slug>'", pref);
        }
    }

    // Probe catalog for a tools-capable model
    match openrouter_catalog::fetch_models(client, base_url, api_key).await {
        Ok(models) => {
            if let Some(m) = models.iter().find(|m| {
                m.supported_parameters
                    .as_ref()
                    .map(|sp| sp.iter().any(|p| p.eq_ignore_ascii_case("tools")))
                    .unwrap_or(false)
            }) {
                info!("choose_tools_model: selected tools-capable model from user catalog: {}", m.id);
                return m.id.clone();
            }
            // Try provider-level signals if model-level is missing
            if let Some(m) = models.iter().find(|m| {
                m.providers.as_ref().map(|ps| {
                    ps.iter().any(|p| {
                        p.supported_parameters
                            .as_ref()
                            .map(|sp| sp.iter().any(|x| x.eq_ignore_ascii_case("tools")))
                            .unwrap_or(false)
                    })
                }).unwrap_or(false)
            }) {
                info!("choose_tools_model: selected tools-capable provider variant: {}", m.id);
                return m.id.clone();
            }
            warn!("choose_tools_model: no tools-capable model found in user catalog; falling back");
        }
        Err(e) => {
            warn!("choose_tools_model: failed to fetch models catalog: {}", e);
        }
    }

    // Conservative fallback commonly supporting tools
    "google/gemini-2.0-flash-001".to_string()
}

/// Inspect tools support for the active model by querying the endpoints list and printing diagnostics.
/// Skips when OPENROUTER_API_KEY is unset.
#[instrument(skip_all)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_model_tools_support_check() {
    let _guard = init_tracing();

    let Some((api_key, base_url)) = openrouter_env() else {
        eprintln!("Skipping: OPENROUTER_API_KEY not set.");
        return;
    };

    let model_id = std::env::var("PLOKE_MODEL_ID")
        .unwrap_or_else(|_| "qwen/qwen-2.5-72b-instruct".to_string());
    let parts: Vec<&str> = model_id.split('/').collect();
    if parts.len() != 2 {
        warn!("Invalid model id '{}'; expected '<author>/<slug>'", model_id);
        return;
    }
    let author = parts[0];
    let slug = parts[1];

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .default_headers(default_headers())
        .build()
        .expect("client");
    let url = format!("{}/models/{}/{}/endpoints", base_url, author, slug);
    match client
        .get(&url)
        .bearer_auth(&api_key)
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let saved = save_response_body("model_tools_support_check", &body);
            info!("GET {} -> {}. Saved to {}", url, status, saved);

            match serde_json::from_str::<ModelEndpointsResponse>(&body) {
                Ok(parsed) => {
                    let total = parsed.data.endpoints.len();
                    let tools_cnt = parsed
                        .data
                        .endpoints
                        .iter()
                        .filter(|ep| ep.supported_parameters.iter().any(|p| p.eq_ignore_ascii_case("tools")))
                        .count();
                    info!("model={} endpoints total={}, tools_capable={}", model_id, total, tools_cnt);
                    for ep in parsed.data.endpoints.iter() {
                        let supports_tools = ep.supported_parameters.iter().any(|p| p.eq_ignore_ascii_case("tools"));
                        info!(
                            "  - provider='{}' slug_hint='{}' supports_tools={} context_length={}",
                            ep.name,
                            ep.name.to_lowercase().replace(' ', "-"),
                            supports_tools,
                            ep.context_length
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to parse endpoints json: {}", e);
                }
            }
        }
        Err(e) => {
            warn!("Failed to fetch endpoints: {}", e);
        }
    }
}

/// Diagnostic: send a single forced tool_choice request and log the request/response thoroughly.
/// Skips when OPENROUTER_API_KEY is unset.
#[instrument(skip_all)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_tools_forced_choice_diagnostics() {
    let _guard = init_tracing();

    let Some((api_key, base_url)) = openrouter_env() else {
        eprintln!("Skipping: OPENROUTER_API_KEY not set.");
        return;
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .default_headers(default_headers())
        .build()
        .expect("client");

    let preferred = std::env::var("PLOKE_MODEL_ID").ok();
    let model_id = choose_tools_model(&client, &base_url, &api_key, preferred.as_deref()).await;

    let payload = json!({
        "model": model_id,
        "messages": [
            {"role":"user","content":"Find code that mentions 'serde_json::from_str' and summarize the count."}
        ],
        "tools": [
            {"type":"function","function":{
                "name":"search_workspace",
                "description":"Search for code or text in the workspace",
                "parameters":{"type":"object","properties":{
                    "query":{"type":"string"},
                    "limit":{"type":"integer","minimum":1,"maximum":50}
                },"required":["query"]}}
            }
        ],
        "tool_choice": {"type":"function","function":{"name":"search_workspace"}},
        "max_tokens": 64
    });

    info!("forced_choice request payload:\n{}", serde_json::to_string_pretty(&payload).unwrap_or_default());

    let url = format!("{}/chat/completions", base_url);
    let start = Instant::now();
    match client.post(&url).bearer_auth(&api_key).json(&payload).send().await {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let saved = save_response_body("tools_forced_diag", &body);
            let elapsed_ms = start.elapsed().as_millis();
            info!("forced_choice -> status={}, elapsed_ms={}, saved={}", status, elapsed_ms, saved);

            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                let used_tool = v.get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|c0| c0.get("message"))
                    .and_then(|m| m.get("tool_calls"))
                    .and_then(|tc| tc.as_array())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false);
                let finish_reason = v.get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.get(0))
                    .and_then(|c0| c0.get("finish_reason"))
                    .and_then(|fr| fr.as_str())
                    .unwrap_or("<none>");
                info!("forced_choice -> used_tool={}, finish_reason={}", used_tool, finish_reason);
            } else {
                warn!("forced_choice -> non-JSON response; inspect {}", saved);
            }
        }
        Err(e) => {
            warn!("forced_choice -> request error: {}", e);
        }
    }
}

fn save_response_body(prefix: &str, contents: &str) -> String {
    let _ = std::fs::create_dir_all("logs");
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let path = format!("logs/{}_{}.json", prefix, ts);
    match std::fs::write(&path, contents) {
        Ok(()) => {
            info!("wrote response body to {}", path);
        }
        Err(e) => {
            warn!("failed to write {}: {}", path, e);
        }
    }

    // Also write/overwrite a stable 'latest' alias for quick inspection.
    let latest_path = format!("logs/{}_latest.json", prefix);
    match std::fs::write(&latest_path, contents) {
        Ok(()) => {
            info!("updated {}", latest_path);
        }
        Err(e) => {
            warn!("failed to write {}: {}", latest_path, e);
        }
    }

    path
}

/// Very basic check that our test App can be acquired from the harness.
#[instrument(skip_all)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn harness_smoke_app_constructs() {
    let app_arc = app();
    let app_lock = app_arc.lock().await;
    // Print a few things for visibility in logs.
    info!("Harness app constructed. show_context_preview={}", app_lock.show_context_preview);
    // Drop lock explicitly at end.
    drop(app_lock);
}

/// Live smoke-test against OpenRouter to validate the endpoints shape for a commonly available model.
#[instrument(skip_all)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_endpoints_live_smoke() {
    let _guard = init_tracing();
    let Some((api_key, base_url)) = openrouter_env() else {
        eprintln!("Skipping: OPENROUTER_API_KEY not set.");
        return;
    };

    let model_id = std::env::var("PLOKE_MODEL_ID")
        .unwrap_or_else(|_| "qwen/qwen-2.5-72b-instruct".to_string());

    let parts: Vec<&str> = model_id.split('/').collect();
    assert!(
        parts.len() == 2,
        "Expected model id '<author>/<slug>', got '{}'",
        model_id
    );
    let author = parts[0];
    let slug = parts[1];

    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .default_headers(default_headers())
        .build()
        .expect("client");

    // Warm up: fetch providers list to build a name->slug map (logged only)
    let providers_url = format!("{}/providers", base_url);
    let providers_resp = client
        .get(&providers_url)
        .bearer_auth(&api_key)
        .send()
        .await;

    match providers_resp {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            info!("GET {} -> {} ({} bytes)", providers_url, status, body.len());
        }
        Err(e) => {
            warn!("GET {} failed: {}", providers_url, e);
        }
    }

    // Fetch endpoints for the model
    let url = format!("{}/models/{}/{}/endpoints", base_url, author, slug);
    let resp = client
        .get(&url)
        .bearer_auth(&api_key)
        .send()
        .await
        .and_then(|r| r.error_for_status());

    match resp {
        Ok(resp) => {
            let status = resp.status();
            let headers = resp.headers().clone();
            info!("GET {} -> {}", url, status);
            for (k, v) in headers.iter() {
                info!("header {}: {}", k, v.to_str().unwrap_or("<binary>"));
            }
            let text = resp.text().await.unwrap_or_default();
            info!("body ({} bytes)", text.len());
            let saved = save_response_body("openrouter_endpoints", &text);
            info!("saved endpoints body to {} (and logs/openrouter_endpoints_latest.json)", saved);

            // Try to parse strongly-typed to validate our schema
            match serde_json::from_str::<ModelEndpointsResponse>(&text) {
                Ok(parsed) => {
                    info!(
                        "Parsed endpoints: {} entries",
                        parsed.data.endpoints.len()
                    );
                    // Soft assertion: Expect at least one endpoint in most cases
                    // Avoids being too brittle; we mainly validate deserialization.
                    assert!(
                        parsed.data.endpoints.len() >= 1,
                        "Endpoints vector should exist"
                    );
                }
                Err(e) => {
                    panic!("Failed to parse endpoints json: {}", e);
                }
            }
        }
        Err(e) => {
            panic!("Failed to fetch endpoints for {}: {}", model_id, e);
        }
    }
}

/// Matrix-style live test to measure tool_call frequency under varied prompts/requests.
/// Saves per-case results and an aggregate summary to logs/tools_success_matrix_<timestamp>.json.
#[instrument(skip_all)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_tools_success_matrix() {
    let _guard = init_tracing();

    let Some((api_key, base_url)) = openrouter_env() else {
        eprintln!("Skipping: OPENROUTER_API_KEY not set.");
        return;
    };

    // HTTP client used across this test
    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .default_headers(default_headers())
        .build()
        .expect("client");

    // Choose a tools-capable model (prefer env override, else auto-detect from catalog).
    let preferred = std::env::var("PLOKE_MODEL_ID").ok();
    let model_id = choose_tools_model(&client, &base_url, &api_key, preferred.as_deref()).await;

    // Pre-check: does this model expose any endpoints that advertise "tools" support?
    let parts: Vec<&str> = model_id.split('/').collect();
    let mut endpoints_tools_count: Option<usize> = None;
    if parts.len() == 2 {
        let author = parts[0];
        let slug = parts[1];
        let url = format!("{}/models/{}/{}/endpoints", base_url, author, slug);
        let client_probe = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("client");
        match client_probe
            .get(&url)
            .bearer_auth(&api_key)
            .send()
            .await
            .and_then(|r| r.error_for_status())
        {
            Ok(resp) => {
                let body = resp.text().await.unwrap_or_default();
                let _ = save_response_body("tools_matrix_endpoints_probe", &body);
                if let Ok(parsed) = serde_json::from_str::<ModelEndpointsResponse>(&body) {
                    let cnt = parsed
                        .data
                        .endpoints
                        .iter()
                        .filter(|ep| ep.supported_parameters.iter().any(|p| p.eq_ignore_ascii_case("tools")))
                        .count();
                    endpoints_tools_count = Some(cnt);
                    info!(
                        "endpoints probe: {} endpoints total, {} advertise tools",
                        parsed.data.endpoints.len(),
                        cnt
                    );
                } else {
                    warn!("endpoints probe: failed to parse response; inspect logs/tools_matrix_endpoints_probe_latest.json");
                }
            }
            Err(e) => {
                warn!("endpoints probe failed: {}", e);
            }
        }
    } else {
        warn!("model id '{}' is not '<author>/<slug>'; skipping endpoints probe", model_id);
    }

    // Define tools (placeholder until wired to our registry export)
    let tools = json!([
      {
        "type": "function",
        "function": {
          "name": "search_workspace",
          "description": "Search for code or text in the workspace",
          "parameters": {
            "type": "object",
            "properties": {
              "query": { "type": "string", "description": "Search query string" },
              "limit": { "type": "integer", "minimum": 1, "maximum": 50 }
            },
            "required": ["query"]
          }
        }
      },
      {
        "type": "function",
        "function": {
          "name": "add_numbers",
          "description": "Add two integers",
          "parameters": {
            "type": "object",
            "properties": {
              "a": {"type": "integer"},
              "b": {"type": "integer"}
            },
            "required": ["a","b"]
          }
        }
      }
    ]);

    // Axes
    let system_variants = vec![
        "",
        "You are a tool-using assistant. Prefer calling a tool when one is available.",
        "Use tools exclusively; output no natural language unless tool results are present.",
    ];

    let user_variants = vec![
        "Search the workspace for references to trait implementations of Iterator.",
        "What's the weather in Paris right now?",
        "Find code files that mention 'serde_json::from_str' and summarize.",
    ];

    let tool_choice_variants = vec![
        "auto",
        "force_search_workspace",
    ];

    let provider_prefs = vec![
        "none",
        "order_openai",
    ];

    #[derive(Debug, Serialize)]
    struct CaseResult {
        system: String,
        user: String,
        tool_choice: String,
        provider_pref: String,
        status: u16,
        used_tool: bool,
        finish_reason: Option<String>,
        native_finish_reason: Option<String>,
        response_model: Option<String>,
        error_code: Option<i64>,
        error_message: Option<String>,
        error: Option<String>,
        latency_ms: u64,
    }

    #[derive(Debug, Serialize)]
    struct Summary {
        model: String,
        total: usize,
        successes: usize,
        failures: usize,
        cases: Vec<CaseResult>,
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .default_headers(default_headers())
        .build()
        .expect("client");

    let mut results: Vec<CaseResult> = Vec::new();

    for system in &system_variants {
        for user in &user_variants {
            for tc in &tool_choice_variants {
                for pref in &provider_prefs {
                    // Build messages
                    let mut messages = vec![];
                    if !system.is_empty() {
                        messages.push(json!({"role":"system","content": system}));
                    }
                    messages.push(json!({"role":"user","content": user}));

                    // Build root payload
                    let mut root = json!({
                        "model": model_id,
                        "messages": messages,
                        "tools": tools,
                        "max_tokens": 128,
                    });

                    // tool_choice
                    match *tc {
                        "auto" => {
                            root.as_object_mut().unwrap().insert("tool_choice".to_string(), json!("auto"));
                        }
                        "force_search_workspace" => {
                            root.as_object_mut().unwrap().insert("tool_choice".to_string(), json!({"type":"function","function":{"name":"search_workspace"}}));
                        }
                        _ => {}
                    }

                    // provider preference
                    match *pref {
                        "none" => {}
                        "order_openai" => {
                            root.as_object_mut().unwrap().insert("provider".to_string(), json!({"order": ["openai"]}));
                        }
                        _ => {}
                    }

                    let url = format!("{}/chat/completions", base_url);
                    let start = Instant::now();
                    let send_res = client
                        .post(&url)
                        .bearer_auth(&api_key)
                        .json(&root)
                        .send()
                        .await;

                    match send_res {
                        Ok(resp) => {
                            let status = resp.status();
                            let body = resp.text().await.unwrap_or_default();

                            let parsed_json = serde_json::from_str::<serde_json::Value>(&body).ok();

                            let used_tool = parsed_json.as_ref().and_then(|v| {
                                v.get("choices")
                                    .and_then(|c| c.as_array())
                                    .and_then(|arr| arr.get(0))
                                    .and_then(|c0| c0.get("message"))
                                    .and_then(|m| m.get("tool_calls"))
                                    .and_then(|tc| tc.as_array())
                                    .map(|a| !a.is_empty())
                            }).unwrap_or(false);

                            let finish_reason = parsed_json.as_ref().and_then(|v| {
                                v.get("choices")
                                    .and_then(|c| c.as_array())
                                    .and_then(|arr| arr.get(0))
                                    .and_then(|c0| c0.get("finish_reason"))
                                    .and_then(|fr| fr.as_str().map(|s| s.to_string()))
                            });

                            let native_finish_reason = parsed_json.as_ref().and_then(|v| {
                                v.get("choices")
                                    .and_then(|c| c.as_array())
                                    .and_then(|arr| arr.get(0))
                                    .and_then(|c0| c0.get("native_finish_reason"))
                                    .and_then(|fr| fr.as_str().map(|s| s.to_string()))
                            });

                            let response_model = parsed_json.as_ref().and_then(|v| v.get("model")).and_then(|m| m.as_str()).map(|s| s.to_string());

                            // Error details (top-level or per-choice)
                            let (error_code, error_message) = if let Some(v) = parsed_json.as_ref() {
                                let top = v.get("error");
                                if let Some(err) = top {
                                    let code = err.get("code").and_then(|c| c.as_i64());
                                    let msg = err.get("message").and_then(|m| m.as_str()).map(|s| s.to_string());
                                    (code, msg)
                                } else {
                                    // Check first choice error if present
                                    let choice_err = v.get("choices")
                                        .and_then(|c| c.as_array())
                                        .and_then(|arr| arr.get(0))
                                        .and_then(|c0| c0.get("error"));
                                    if let Some(ej) = choice_err {
                                        let code = ej.get("code").and_then(|c| c.as_i64());
                                        let msg = ej.get("message").and_then(|m| m.as_str()).map(|s| s.to_string());
                                        (code, msg)
                                    } else {
                                        (None, None)
                                    }
                                }
                            } else {
                                (None, None)
                            };

                            let label = format!("tools_matrix_s={},u={},tc={},p={}",
                                if system.is_empty() { "none" } else { "var" },
                                match *user {
                                    "Search the workspace for references to trait implementations of Iterator." => "repo",
                                    "What's the weather in Paris right now?" => "weather",
                                    _ => "code",
                                },
                                tc,
                                pref
                            );
                            let saved = save_response_body(&label.replace([' ', '/', '\n'], "_"), &body);
                            info!("saved case body to {} (and logs/{}_latest.json)", saved, label.replace([' ', '/', '\n'], "_"));

                            let elapsed_ms = start.elapsed().as_millis() as u64;
                            info!(
                                "case {} -> status={}, used_tool={}, finish_reason={:?}, native_finish_reason={:?}, elapsed_ms={}",
                                label, status, used_tool, finish_reason, native_finish_reason, elapsed_ms
                            );

                            results.push(CaseResult{
                                system: system.to_string(),
                                user: user.to_string(),
                                tool_choice: tc.to_string(),
                                provider_pref: pref.to_string(),
                                status: status.as_u16(),
                                used_tool,
                                finish_reason,
                                native_finish_reason,
                                response_model,
                                error_code,
                                error_message,
                                error: None,
                                latency_ms: elapsed_ms,
                            });
                        }
                        Err(e) => {
                            info!(
                                "case build/send error for system='{}', user='{}', tool_choice='{}', provider_pref='{}': {}",
                                system, user, tc, pref, e
                            );
                            results.push(CaseResult{
                                system: system.to_string(),
                                user: user.to_string(),
                                tool_choice: tc.to_string(),
                                provider_pref: pref.to_string(),
                                status: 0,
                                used_tool: false,
                                finish_reason: None,
                                native_finish_reason: None,
                                response_model: None,
                                error_code: None,
                                error_message: None,
                                error: Some(e.to_string()),
                                latency_ms: 0,
                            });
                        }
                    }
                }
            }
        }
    }

    let successes = results.iter().filter(|r| r.used_tool).count();
    let failures = results.len() - successes;

    let summary = Summary{
        model: model_id,
        total: results.len(),
        successes,
        failures,
        cases: results,
    };

    let serialized = serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "{}".to_string());
    info!("tools_success_matrix summary: total={}, success={}, failure={}", summary.total, summary.successes, summary.failures);

    // Save the full summary to timestamped and 'latest' paths.
    let summary_path = save_response_body("tools_success_matrix", &serialized);
    info!("tools_success_matrix saved to {} and logs/tools_success_matrix_latest.json", summary_path);

    // Minimal, meaningful assertion: when tool_choice is forced for our declared tool,
    // we expect at least one call to register as using a tool. If not, fail the test.
    let forced_total = summary
        .cases
        .iter()
        .filter(|c| c.tool_choice == "force_search_workspace")
        .count();
    let forced_success = summary
        .cases
        .iter()
        .filter(|c| c.tool_choice == "force_search_workspace" && c.used_tool)
        .count();
    info!(
        "force_search_workspace -> used_tool {}/{}",
        forced_success, forced_total
    );
    if forced_total > 0 && forced_success == 0 {
        panic!(
            "No tool_calls observed in any force_search_workspace cases.
endpoints_tools_count(advertise 'tools')={:?}.
Summary saved to: {}.
Hints:
- Verify the selected model supports tools on at least one endpoint (see endpoints probe log above).
- Inspect {} and logs/tools_matrix_endpoints_probe_latest.json for raw responses.
- Try setting PLOKE_MODEL_ID to a known tools-capable model (e.g., qwen/qwen-2.5-72b-instruct or google/gemini-2.0-flash-001).
- Run with: RUST_LOG=info cargo test -p ploke-tui openrouter_tools_success_matrix -- --nocapture.",
            endpoints_tools_count, summary_path, summary_path
        );
    }
}

/// HYP-001: Provider preference hypotheses.
/// Sends three minimal chat/completions with different provider preference shapes and logs the results.
#[instrument(skip_all)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_provider_preference_experiment() {

    let Some((api_key, base_url)) = openrouter_env() else {
        eprintln!("Skipping: OPENROUTER_API_KEY not set.");
        return;
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .expect("client");

    // Choose a small, widely available chat-capable model
    let model_id = std::env::var("PLOKE_MODEL_ID")
        .unwrap_or_else(|_| "qwen/qwen-2.5-72b-instruct".to_string());

    let mk_payload = |provider_obj: Option<serde_json::Value>| {
        let mut root = json!({
            "model": model_id,
            "max_tokens": 16,
            "messages": [
                {"role": "user", "content": "Reply with: OK"}
            ]
        });
        if let Some(p) = provider_obj {
            root.as_object_mut().unwrap().insert("provider".to_string(), p);
        }
        root
    };

    // A: provider omitted (control)
    let a = mk_payload(None);

    // B: provider.order = ["openai"] (hypothesis: accepted and respected)
    let b = mk_payload(Some(json!({"order": ["openai"]})));

    // C: provider.allow = ["openai"] (hypothesis: might 400 on chat/completions)
    let c = mk_payload(Some(json!({"allow": ["openai"]})));

    for (label, payload) in [("A: omitted", a), ("B: order", b), ("C: allow", c)] {
        let url = format!("{}/chat/completions", base_url);
        let res = client
            .post(&url)
            .bearer_auth(&api_key)
            .json(&payload)
            .send()
            .await;

        match res {
            Ok(rsp) => {
                let status = rsp.status();
                let headers = rsp.headers().clone();
                for (k, v) in headers.iter() {
                    info!("{} header {}: {}", label, k, v.to_str().unwrap_or("<binary>"));
                }
                let body = rsp.text().await.unwrap_or_default();
                info!(
                    "{} -> Status: {}. Body (first 512): {}",
                    label,
                    status,
                    &body.chars().take(512).collect::<String>()
                );
                let saved = save_response_body(&format!("provider_pref_{}", label.replace([' ', ':'], "_")), &body);
                info!("{} -> saved body to {} (and logs/provider_pref_{}_latest.json)", label, saved, label.replace([' ', ':'], "_"));
            }
            Err(e) => {
                warn!("{} -> Request error: {}", label, e);
            panic!("Test failed")
            }
        }
    }
}

/// HYP-001: Tool support smoke. Attempts a basic "tools" call to observe whether tool_calls are returned.
#[instrument(skip_all)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_tools_smoke() {
    let _guard = init_tracing();

    let Some((api_key, base_url)) = openrouter_env() else {
        eprintln!("Skipping: OPENROUTER_API_KEY not set.");
        return;
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .expect("client");

    let model_id = std::env::var("PLOKE_MODEL_ID")
        .unwrap_or_else(|_| "qwen/qwen-2.5-72b-instruct".to_string());

    let payload = json!({
        "model": model_id,
        "max_tokens": 64,
        "messages": [
            {"role": "user", "content": "Call the tool add_numbers with a=2 and b=3, then return only the tool call."}
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "add_numbers",
                    "description": "Add two integers",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "a": {"type": "integer"},
                            "b": {"type": "integer"}
                        },
                        "required": ["a", "b"]
                    }
                }
            }
        ]
    });

    let url = format!("{}/chat/completions", base_url);
    let res = client
        .post(&url)
        .bearer_auth(&api_key)
        .json(&payload)
        .send()
        .await;

    match res {
        Ok(rsp) => {
            let status = rsp.status();
            let headers = rsp.headers().clone();
            for (k, v) in headers.iter() {
                info!("tools_smoke header {}: {}", k, v.to_str().unwrap_or("<binary>"));
            }
            let body = rsp.text().await.unwrap_or_default();
            info!("tools_smoke -> Status: {}", status);
            info!(
                "tools_smoke -> Body (first 1024): {}",
                &body.chars().take(1024).collect::<String>()
            );
            let saved = save_response_body("tools_smoke", &body);
            info!("tools_smoke -> saved body to {} (and logs/tools_smoke_latest.json)", saved);
            warn!("tools_smoke is currently using a synthetic tool schema. Integrate real tools from our registry for stronger validation.");
        }
        Err(e) => {
            warn!("tools_smoke -> Request error: {}", e);
            panic!("Test failed")
        }
    }
}

/// Quick-touchpoint test across a small set of models to see if a forced tool call is honored.
/// Does not try all permutations; intended to be fast and informative.
/// Requires OPENROUTER_API_KEY; respects PLOKE_MODEL_ID override for the first slot, then tries a few popular models.
#[instrument(skip_all)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_tools_model_touchpoints() {
    let _guard = init_tracing();

    let Some((api_key, base_url)) = openrouter_env() else {
        eprintln!("Skipping: OPENROUTER_API_KEY not set.");
        return;
    };

    // Small set of models to probe. The first entry is either explicit override or autodetected tools-capable model.
    // The rest are popular/representative models for a quick cross-section.
    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .default_headers(default_headers())
        .build()
        .expect("client");

    let preferred = std::env::var("PLOKE_MODEL_ID").ok();
    let primary = choose_tools_model(&client, &base_url, &api_key, preferred.as_deref()).await;

    let mut models: Vec<String> = vec![
        primary,
        "google/gemini-2.0-flash-001".to_string(),
        "qwen/qwen-2.5-72b-instruct".to_string(),
        "anthropic/claude-3.5-sonnet".to_string(),
    ];
    // Dedup while preserving order
    models.dedup();

    // Shared tool schema and forced tool choice. Keep it minimal and consistent with our other tests.
    let tools = json!([
      {
        "type": "function",
        "function": {
          "name": "search_workspace",
          "description": "Search for code or text in the workspace",
          "parameters": {
            "type": "object",
            "properties": {
              "query": { "type": "string", "description": "Search query string" },
              "limit": { "type": "integer", "minimum": 1, "maximum": 50 }
            },
            "required": ["query"]
          }
        }
      }
    ]);

    #[derive(Debug, Serialize)]
    struct Touchpoint {
        model: String,
        status: u16,
        used_tool: bool,
        finish_reason: Option<String>,
        native_finish_reason: Option<String>,
        error_code: Option<i64>,
        error_message: Option<String>,
        latency_ms: u64,
    }

    let mut results: Vec<Touchpoint> = Vec::new();

    for model_id in models {
        let messages = json!([
            {"role":"system","content":"You are a tool-using assistant. Prefer calling a tool when one is available."},
            {"role":"user","content":"Search the workspace for references to trait implementations of Iterator."}
        ]);

        let payload = json!({
            "model": model_id,
            "messages": messages,
            "tools": tools,
            "tool_choice": {"type":"function","function":{"name":"search_workspace"}},
            "max_tokens": 128
        });

        let url = format!("{}/chat/completions", base_url);
        let start = Instant::now();
        match client.post(&url).bearer_auth(&api_key).json(&payload).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();

                let parsed_json = serde_json::from_str::<serde_json::Value>(&body).ok();

                let used_tool = parsed_json.as_ref().and_then(|v| {
                    v.get("choices")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.get(0))
                        .and_then(|c0| c0.get("message"))
                        .and_then(|m| m.get("tool_calls"))
                        .and_then(|tc| tc.as_array())
                        .map(|a| !a.is_empty())
                }).unwrap_or(false);

                let finish_reason = parsed_json.as_ref().and_then(|v| {
                    v.get("choices")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.get(0))
                        .and_then(|c0| c0.get("finish_reason"))
                        .and_then(|fr| fr.as_str().map(|s| s.to_string()))
                });

                let native_finish_reason = parsed_json.as_ref().and_then(|v| {
                    v.get("choices")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.get(0))
                        .and_then(|c0| c0.get("native_finish_reason"))
                        .and_then(|fr| fr.as_str().map(|s| s.to_string()))
                });

                let (error_code, error_message) = if let Some(v) = parsed_json.as_ref() {
                    let top = v.get("error");
                    if let Some(err) = top {
                        let code = err.get("code").and_then(|c| c.as_i64());
                        let msg = err.get("message").and_then(|m| m.as_str()).map(|s| s.to_string());
                        (code, msg)
                    } else {
                        let choice_err = v.get("choices")
                            .and_then(|c| c.as_array())
                            .and_then(|arr| arr.get(0))
                            .and_then(|c0| c0.get("error"));
                        if let Some(ej) = choice_err {
                            let code = ej.get("code").and_then(|c| c.as_i64());
                            let msg = ej.get("message").and_then(|m| m.as_str()).map(|s| s.to_string());
                            (code, msg)
                        } else {
                            (None, None)
                        }
                    }
                } else {
                    (None, None)
                };

                let elapsed_ms = start.elapsed().as_millis() as u64;
                let label = format!("tools_touchpoint_{}", model_id.replace([' ', '/', '\n', ':'], "_"));
                let saved = save_response_body(&label, &body);
                info!(
                    "touchpoint model='{}' -> status={}, used_tool={}, finish_reason={:?}, native_finish_reason={:?}, saved={}",
                    model_id, status, used_tool, finish_reason, native_finish_reason, saved
                );

                results.push(Touchpoint{
                    model: model_id,
                    status: status.as_u16(),
                    used_tool,
                    finish_reason,
                    native_finish_reason,
                    error_code,
                    error_message,
                    latency_ms: elapsed_ms,
                });
            }
            Err(e) => {
                info!(
                    "touchpoint request error for model='{}': {}",
                    model_id, e
                );
                results.push(Touchpoint{
                    model: model_id,
                    status: 0,
                    used_tool: false,
                    finish_reason: None,
                    native_finish_reason: None,
                    error_code: None,
                    error_message: Some(e.to_string()),
                    latency_ms: 0,
                });
            }
        }
    }

    #[derive(Debug, Serialize)]
    struct TouchpointSummary {
        total: usize,
        used_tool: usize,
        cases: Vec<Touchpoint>,
    }

    let used_tool_count = results.iter().filter(|r| r.used_tool).count();
    let summary = TouchpointSummary {
        total: results.len(),
        used_tool: used_tool_count,
        cases: results,
    };

    let serialized = serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "{}".to_string());
    info!(
        "tools_model_touchpoints summary: total={}, used_tool={}",
        summary.total, summary.used_tool
    );
    let _ = save_response_body("tools_model_touchpoints", &serialized);

    // Intentionally do not fail: this is a discovery/visibility test.
    // For strict assertions, see openrouter_tools_forced_choice_diagnostics and the full matrix test.
}
