#![cfg(all(feature = "live_api_tests", feature = "test_harness"))]

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::Duration;

use ploke_tui::llm::session::build_openai_request;
use ploke_tui::llm::openrouter::openrouter_catalog::fetch_model_endpoints;
use ploke_tui::llm::{RequestMessage, Role};
use ploke_tui::test_harness::{default_headers, openrouter_env};
use ploke_tui::tools::{FunctionMarker, GatTool, ToolDefinition};
use ploke_tui::tools::request_code_context::RequestCodeContextGat;
use ploke_tui::user_config::{ModelConfig, ProviderType};

fn out_dir() -> PathBuf {
    let root = PathBuf::from("target/test-output/openrouter_matrix");
    fs::create_dir_all(&root).ok();
    let dir = root.join(format!("run-{}", Utc::now().format("%Y%m%d-%H%M%S")));
    fs::create_dir_all(&dir).ok();
    dir
}

fn request_code_context_def() -> ToolDefinition {
    RequestCodeContextGat::tool_def()
}

#[derive(Serialize, Deserialize, Debug)]
struct MetricRow {
    ts: String,
    model: String,
    provider_slug: Option<String>,
    prompt_id: String,
    tool_choice: String,
    status: u16,
    duration_ms: u128,
    body_len: usize,
    saw_tool_calls: bool,
    choices: usize,
}

async fn choose_tools_provider_slug(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model_id: &str,
) -> Option<String> {
    if let Ok(eps) = fetch_model_endpoints(client, base_url.parse().ok()?, api_key, model_id).await {
        for e in eps {
            let supports_tools = e.supported_parameters.as_ref().map(|sp| sp.iter().any(|s| s == "tools")).unwrap_or(false);
            if supports_tools {
                return Some(e.id);
            }
        }
    }
    None
}

fn default_models() -> Vec<String> {
    // Keep this small by default; can be extended via env later.
    vec![
        "qwen/qwen-2.5-7b-instruct".to_string(),
        "deepseek/deepseek-chat-v3.1".to_string(),
    ]
}

fn default_prompts() -> Vec<(String, String)> {
    // Allow env override: PLOKE_LIVE_PROMPTS="id1::prompt text||id2::prompt text"
    if let Ok(spec) = std::env::var("PLOKE_LIVE_PROMPTS") {
        let mut v = Vec::new();
        for part in spec.split("||").map(|s| s.trim()).filter(|s| !s.is_empty()) {
            if let Some((id, prompt)) = part.split_once("::") {
                v.push((id.to_string(), prompt.to_string()));
            }
        }
        if !v.is_empty() { return v; }
    }
    vec![
        (
            "force_tool_simple".to_string(),
            "You can call tools. Only call request_code_context to fetch context about lib.rs.".to_string(),
        ),
        (
            "implicit_tool_hint".to_string(),
            "Find relevant code context for implementing a function. Use a tool call if helpful.".to_string(),
        ),
    ]
}

#[tokio::test]
async fn live_openrouter_tool_matrix() {
    let Some(env) = openrouter_env() else { return; };
    let client = Client::builder()
        .timeout(Duration::from_secs(ploke_tui::LLM_TIMEOUT_SECS))
        .default_headers(default_headers())
        .build()
        .expect("client");

    let dir = out_dir();
    let mut metrics_path = dir.clone();
    metrics_path.push("metrics.jsonl");
    let mut metrics_file = fs::File::create(&metrics_path).expect("create metrics.jsonl");

    let mut models = default_models();
    if let Ok(extra) = std::env::var("PLOKE_LIVE_MODELS") {
        for s in extra.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()) {
            models.push(s.to_string());
        }
        models.sort();
        models.dedup();
    }
    let prompts = default_prompts();
    let tools = vec![request_code_context_def()];

    // Cap combinations
    let max_combos: usize = std::env::var("PLOKE_LIVE_MAX_COMBOS").ok().and_then(|s| s.parse().ok()).unwrap_or(6);

    let mut attempted = 0usize;
    let mut observed_tool_calls = 0usize;
    use std::collections::HashMap;
    #[derive(Serialize, Default)]
    struct Agg { attempts: usize, ok: usize, tool_calls: usize, avg_ms: f64 }
    let mut agg: HashMap<(String, String, String), Agg> = HashMap::new();

    'MODEL: for model in models {
        if attempted >= max_combos { break; }

        let provider_slug = choose_tools_provider_slug(&client, &env.url.to_string(), &env.key, &model).await;
        let mut provider = ModelConfig {
            id: format!("matrix-{}", model.replace('/', "-")),
            api_key: env.key.clone(),
            provider_slug: provider_slug.clone(),
            api_key_env: None,
            base_url: env.url.to_string(),
            model: model.clone(),
            display_name: None,
            provider_type: ProviderType::OpenRouter,
            llm_params: None,
        };

        for (prompt_id, prompt) in &prompts {
            if attempted >= max_combos { break 'MODEL; }
            for force_function in [false, true] {
                if attempted >= max_combos { break 'MODEL; }

                let sys = RequestMessage { role: Role::System, content: "You can call tools.".to_string(), tool_call_id: None };
                let user = RequestMessage { role: Role::User, content: prompt.clone(), tool_call_id: None };
                let messages = vec![sys, user];
                let params = ploke_tui::llm::LLMParameters { max_tokens: Some(64), temperature: Some(0.0), ..Default::default() };
                let mut req = build_openai_request(&provider, messages, &params, Some(tools.clone()), true, true);

                let tool_choice = if force_function {
                    req.tool_choice = Some(ploke_tui::llm::openrouter::model_provider::ToolChoice::Function {
                        r#type: FunctionMarker,
                        function: ploke_tui::llm::openrouter::model_provider::ToolChoiceFunction { name: "request_code_context".to_string() },
                    });
                    "function"
                } else {
                    req.tool_choice = Some(ploke_tui::llm::openrouter::model_provider::ToolChoice::Auto);
                    "auto"
                };

                // Persist request snapshot
                let snap_dir = dir.join(format!("{}-{}-{}", model.replace('/', "_"), prompt_id, tool_choice));
                fs::create_dir_all(&snap_dir).ok();
                let _ = fs::write(snap_dir.join("request.json"), serde_json::to_string_pretty(&req).unwrap());

                let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
                let start = Instant::now();
                let resp = client
                    .post(&url)
                    .bearer_auth(&provider.api_key)
                    .header("Accept", "application/json")
                    .json(&req)
                    .send()
                    .await
                    .expect("send");
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_else(|_| "<no-body>".to_string());
                let duration_ms = start.elapsed().as_millis();
                let body_len = body.len();

                let (saw_tool_calls, choices) = match serde_json::from_str::<serde_json::Value>(&body) {
                    Ok(v) => {
                        let _ = fs::write(snap_dir.join("response.json"), serde_json::to_string_pretty(&v).unwrap());
                        let choices = v.get("choices").and_then(|c| c.as_array()).map(|a| a.len()).unwrap_or(0);
                        let saw = v
                            .get("choices")
                            .and_then(|c| c.as_array())
                            .map(|arr| arr.iter().any(|ch| ch.get("message").and_then(|m| m.get("tool_calls")).is_some()))
                            .unwrap_or(false);
                        (saw, choices)
                    }
                    Err(_) => {
                        let _ = fs::write(snap_dir.join("response.txt"), &body);
                        (false, 0)
                    }
                };

                if saw_tool_calls { observed_tool_calls += 1; }

                // Update summary aggregation
                let key = (model.clone(), prompt_id.clone(), tool_choice.to_string());
                let entry = agg.entry(key).or_default();
                entry.attempts += 1;
                if status == 200 { entry.ok += 1; }
                if saw_tool_calls { entry.tool_calls += 1; }
                // online mean
                let n = entry.attempts as f64;
                entry.avg_ms = entry.avg_ms + ((duration_ms as f64) - entry.avg_ms) / n;

                let row = MetricRow {
                    ts: Utc::now().to_rfc3339(),
                    model: model.clone(),
                    provider_slug: provider.provider_slug.clone(),
                    prompt_id: prompt_id.clone(),
                    tool_choice: tool_choice.to_string(),
                    status,
                    duration_ms,
                    body_len,
                    saw_tool_calls,
                    choices,
                };
                let line = serde_json::to_string(&row).unwrap();
                use std::io::Write;
                writeln!(&mut metrics_file, "{}", line).ok();

                attempted += 1;
            }
        }
    }

    // Persist summary
    let mut summary_path = dir.clone();
    summary_path.push("summary.json");
    let summary: Vec<_> = agg.into_iter().map(|((m,p,t), a)| json!({
        "model": m, "prompt_id": p, "tool_choice": t,
        "attempts": a.attempts, "ok": a.ok, "tool_calls": a.tool_calls,
        "avg_ms": a.avg_ms
    })).collect();
    std::fs::write(&summary_path, serde_json::to_string_pretty(&summary).unwrap()).ok();

    // At least attempt something
    assert!(attempted > 0, "no live matrix combinations attempted");
    // Prefer at least one tool_calls observed; if not, leave artifacts for investigation but do not panic
    if observed_tool_calls == 0 {
        let _ = fs::write(dir.join("not_validated.txt"), b"No tool_calls observed in matrix run");
    }
}
