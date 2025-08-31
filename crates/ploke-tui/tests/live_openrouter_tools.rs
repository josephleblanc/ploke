#![cfg(all(feature = "live_api_tests", feature = "test_harness"))]

use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use tokio::time::Duration;

use ploke_tui::llm::session::build_openai_request;
use ploke_tui::llm::{RequestMessage, Role};
use ploke_tui::test_harness::{default_headers, openrouter_env};
use ploke_tui::tools::{ToolDefinition, ToolDescr, ToolFunctionDef, ToolName, FunctionMarker};
use ploke_tui::user_config::{ModelConfig, ProviderType, openrouter_url};

fn ai_temp_dir() -> PathBuf {
    let mut p = PathBuf::from("ai_temp_data/live");
    fs::create_dir_all(&p).ok();
    p
}

fn make_request_code_context_def() -> ToolDefinition {
    ToolDefinition {
        r#type: FunctionMarker,
        function: ToolFunctionDef {
            name: ToolName::RequestCodeContext,
            description: ToolDescr::RequestCodeContext,
            parameters: json!({
                "type": "object",
                "properties": {
                    "token_budget": {"type": "integer", "minimum": 1},
                    "hint": {"type": "string"}
                },
                "required": ["token_budget"],
                "additionalProperties": false
            }),
        },
    }
}

#[tokio::test]
async fn live_tool_call_request_code_context() {
    let Some(env) = openrouter_env() else { return; };
    let client = Client::new();

    // Build a model config for OpenRouter
    let provider = ModelConfig {
        id: "live-openrouter".to_string(),
        api_key: env.key.clone(),
        provider_slug: Some("openrouter".to_string()),
        api_key_env: None,
        base_url: env.url.to_string(),
        model: ploke_tui::user_config::default_model(),
        display_name: None,
        provider_type: ProviderType::OpenRouter,
        llm_params: None,
    };

    let sys = RequestMessage { role: Role::System, content: "You can call tools.".to_string(), tool_call_id: None };
    let user = RequestMessage { role: Role::User, content: "Fetch context for lib.rs via tool; only call the tool.".to_string(), tool_call_id: None };
    let messages = vec![sys, user];
    let tools = vec![make_request_code_context_def()];

    let params = ploke_tui::llm::LLMParameters { max_tokens: Some(64), temperature: Some(0.0), ..Default::default() };
    let request = build_openai_request(&provider, messages, &params, Some(tools), true, true);

    // Persist request plan
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let dir = ai_temp_dir().join(format!("openrouter-{}", ts));
    fs::create_dir_all(&dir).ok();
    let req_path = dir.join("request.json");
    fs::write(&req_path, serde_json::to_string_pretty(&request).unwrap()).ok();

    // Dispatch live request (requires OPENROUTER_API_KEY)
    let url = format!("{}/chat/completions", provider.base_url);
    let resp = client
        .post(url)
        .headers(default_headers())
        .bearer_auth(&provider.api_key)
        .json(&request)
        .timeout(Duration::from_secs(ploke_tui::LLM_TIMEOUT_SECS))
        .send()
        .await
        .expect("request send");

    let status = resp.status();
    let body = resp.text().await.expect("body text");
    fs::write(dir.join("response.json"), &body).ok();

    assert!(status.is_success(), "non-success status: {} body: {}", status, body);
    // Weak assertion: body contains either tool_calls or function object typical of tool responses
    assert!(body.contains("tool_calls") || body.contains("\"type\":\"function\""));
}
