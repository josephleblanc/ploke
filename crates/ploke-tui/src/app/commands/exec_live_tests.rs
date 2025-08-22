#![cfg(test)]

use serde_json::json;
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

fn save_response_body(prefix: &str, contents: &str) {
    let _ = std::fs::create_dir_all("logs");
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let path = format!("logs/{}_{}.json", prefix, ts);
    if let Err(e) = std::fs::write(&path, contents) {
        warn!("failed to write {}: {}", path, e);
    } else {
        info!("wrote response body to {}", path);
    }
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
            save_response_body("openrouter_endpoints", &text);

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
                save_response_body(&format!("provider_pref_{}", label.replace([' ', ':'], "_")), &body);
            }
            Err(e) => {
                warn!("{} -> Request error: {}", label, e);
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
            save_response_body("tools_smoke", &body);
            warn!("tools_smoke is currently using a synthetic tool schema. Integrate real tools from our registry for stronger validation.");
        }
        Err(e) => {
            warn!("tools_smoke -> Request error: {}", e);
        }
    }
}
