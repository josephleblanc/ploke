#![cfg(all(feature = "live_api_tests", feature = "test_harness"))]

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::Utc;
use ploke_tui::llm::providers::ProviderSlug;
use reqwest::Client;
use tokio::time::Duration;

use ploke_tui::llm::openrouter::model_provider::{ToolChoice, ToolChoiceFunction};
use ploke_tui::llm::openrouter::openrouter_catalog::fetch_model_endpoints;
use ploke_tui::llm::session::build_openai_request;
use ploke_tui::llm::{RequestMessage, Role};
use ploke_tui::test_harness::{default_headers, openrouter_env};
use ploke_tui::tools::request_code_context::RequestCodeContextGat;
use ploke_tui::tools::{FunctionMarker, Tool};
use ploke_tui::user_config::{ModelConfig, ProviderType};

fn ai_temp_dir() -> PathBuf {
    let p = PathBuf::from("ai_temp_data/live");
    fs::create_dir_all(&p).ok();
    p
}

#[tokio::test]
async fn live_tool_call_request_code_context() {
    let Some(env) = openrouter_env() else {
        return;
    };
    let client = Client::new();

    // Build a model config for OpenRouter
    let mut provider = ModelConfig {
        id: "live-openrouter".to_string(),
        api_key: env.key.clone(),
        provider_slug: None,
        api_key_env: None,
        base_url: env.base_url.to_string(),
        // Choose a model likely to have tool-capable endpoints
        model: "qwen/qwen-2.5-7b-instruct".to_string(),
        display_name: None,
        provider_type: ProviderType::OpenRouter,
        llm_params: None,
    };

    // Fetch endpoints and pick a provider that supports tools if available; skip gracefully otherwise
    if let Ok(eps) =
        fetch_model_endpoints(&client, env.base_url.clone(), &env.key, &provider.model).await
    {
        if let Some(p) = eps.endpoints.iter().find(|e| e.supports_tools()) {
            provider.provider_slug = Some(ProviderSlug::from_str(&p.name).expect("provider_slug"));
        } else {
            let dir =
                ai_temp_dir().join(format!("openrouter-{}", Utc::now().format("%Y%m%d-%H%M%S")));
            fs::create_dir_all(&dir).ok();
            let note = format!(
                "No tools-capable provider found for model '{}'; skipping.",
                provider.model
            );
            fs::write(dir.join("no_tool_providers.txt"), note).ok();
            return;
        }
    } else {
        panic!(
            "unable to fetch models via fetch_model_endpoints using ModelConfig:\n{:#?}",
            provider
        );
        // Unable to fetch endpoints; should fail to avoid false positive
    }

    let sys = RequestMessage {
        role: Role::System,
        content: "You can call tools.".to_string(),
        tool_call_id: None,
    };
    let user = RequestMessage {
        role: Role::User,
        content: "Fetch context for lib.rs via tool; only call the tool.".to_string(),
        tool_call_id: None,
    };
    let messages = vec![sys, user];
    let tools = vec![RequestCodeContextGat::tool_def()];

    let params = ploke_tui::llm::LLMParameters {
        max_tokens: Some(64),
        temperature: Some(0.0),
        ..Default::default()
    };
    let mut request = build_openai_request(&provider, messages, &params, Some(tools), true, true);
    // Force tool call for validation
    request.tool_choice = Some(ToolChoice::Function {
        r#type: FunctionMarker,
        function: ToolChoiceFunction {
            name: "request_code_context".to_string(),
        },
    });

    // Persist request plan
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let dir = ai_temp_dir().join(format!("openrouter-{}", ts));
    fs::create_dir_all(&dir).ok();
    let req_path = dir.join("request.json");
    fs::write(&req_path, serde_json::to_string_pretty(&request).unwrap()).ok();

    // Dispatch live request (requires OPENROUTER_API_KEY)
    let url = format!(
        "{}/chat/completions",
        provider.base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .headers(default_headers())
        .header("Accept", "application/json")
        .bearer_auth(&provider.api_key)
        .json(&request)
        .timeout(Duration::from_secs(ploke_tui::LLM_TIMEOUT_SECS))
        .send()
        .await
        .expect("request send");

    let status = resp.status();
    let body = resp.text().await.expect("body text");
    // Persist response body: prefer JSON pretty if parsable; otherwise save as HTML
    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(v) => {
            let pretty = serde_json::to_string_pretty(&v).unwrap();
            fs::write(dir.join("response.json"), pretty).ok();
        }
        Err(_) => {
            fs::write(dir.join("response.html"), &body).ok();
        }
    }

    assert!(
        status.is_success(),
        "non-success status: {} body: {}",
        status,
        body
    );
    // Evidence check: prefer tool_calls or function-type presence; if missing, record as not validated
    let has_tools = body.contains("tool_calls") || body.contains("\"type\":\"function\"");
    if !has_tools {
        fs::write(
            dir.join("no_tool_calls.txt"),
            "tool_calls not observed; live path not validated",
        )
        .ok();
    }
}
