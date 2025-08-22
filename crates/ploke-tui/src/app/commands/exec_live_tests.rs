#![cfg(test)]

use serde_json::json;
use serde::{Serialize, Deserialize};
use tracing::{info, warn, instrument};

use reqwest::Client;
use std::time::Duration;

use crate::llm::provider_endpoints::ModelEndpointsResponse;
use crate::tracing_setup::init_tracing;
use crate::user_config::OPENROUTER_URL;

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

    // Choose a widely available tools-capable model; override via env if desired.
    let model_id = std::env::var("PLOKE_MODEL_ID")
        .unwrap_or_else(|_| "qwen/qwen-2.5-72b-instruct".to_string());

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
        error: Option<String>,
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
                            let used_tool = match serde_json::from_str::<serde_json::Value>(&body) {
                                Ok(v) => {
                                    // Non-streaming path: choices[0].message.tool_calls exists and non-empty
                                    let tc_opt = v.get("choices")
                                        .and_then(|c| c.as_array())
                                        .and_then(|arr| arr.get(0))
                                        .and_then(|c0| c0.get("message"))
                                        .and_then(|m| m.get("tool_calls"));
                                    match tc_opt {
                                        Some(serde_json::Value::Array(a)) => !a.is_empty(),
                                        _ => false,
                                    }
                                }
                                Err(_) => false,
                            };

                            let finish_reason = serde_json::from_str::<serde_json::Value>(&body)
                                .ok()
                                .and_then(|v| v.get("choices")
                                    .and_then(|c| c.as_array())
                                    .and_then(|arr| arr.get(0))
                                    .and_then(|c0| c0.get("finish_reason"))
                                    .and_then(|fr| fr.as_str().map(|s| s.to_string()))
                                );

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

                            results.push(CaseResult{
                                system: system.to_string(),
                                user: user.to_string(),
                                tool_choice: tc.to_string(),
                                provider_pref: pref.to_string(),
                                status: status.as_u16(),
                                used_tool,
                                finish_reason,
                                error: None,
                            });
                        }
                        Err(e) => {
                            results.push(CaseResult{
                                system: system.to_string(),
                                user: user.to_string(),
                                tool_choice: tc.to_string(),
                                provider_pref: pref.to_string(),
                                status: 0,
                                used_tool: false,
                                finish_reason: None,
                                error: Some(e.to_string()),
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
        panic!("No tool_calls observed in any force_search_workspace cases. This suggests the API ignored the forced tool_choice or our schema is invalid.");
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
