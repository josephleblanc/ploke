use crate::{llm::router_only::openrouter::OpenRouter, tools::Tool};
use std::time::Duration;

use crate::{llm::ModelId, llm::error::LlmError};
use std::{path::PathBuf, str::FromStr as _};

mod builder_tests;

use super::*;
#[test]
fn show_openrouter_json2() {
    let req = ChatCompRequest::<OpenRouter> {
        router: openrouter::ChatCompFields::default()
            .with_route(FallbackMarker)
            .with_transforms(Transform::MiddleOut([MiddleOutMarker])),
        ..Default::default()
    };
    let j = serde_json::to_string_pretty(&req).unwrap();
    println!("{j}");
}

fn parse_with_env(response_json: &str) -> Result<()> {
    use ploke_test_utils::workspace_root;

    use crate::llm::router_only::cli::{
        MODELS_JSON_ARCH, MODELS_JSON_RAW, MODELS_JSON_RAW_PRETTY, MODELS_JSON_SUPPORTED,
        MODELS_JSON_TOP, MODELS_TXT_CANON, MODELS_TXT_IDS,
    };

    let mut dir = workspace_root();
    let parsed: models::Response = serde_json::from_str(response_json)?;

    let env_string = std::env::var("WRITE_MODE").unwrap_or_default();
    if ["raw", "all"].contains(&env_string.as_str()) {
        dir.push(MODELS_JSON_RAW);
        println!("Writing '/models' raw response to:\n{}", dir.display());
        std::fs::write(dir, response_json)?;
    }
    if ["raw_pretty", "all"].contains(&env_string.as_str()) {
        let mut dir = workspace_root();
        let raw_pretty = serde_json::Value::from_str(response_json)?;
        let pretty = serde_json::to_string_pretty(&raw_pretty)?;
        dir.push(MODELS_JSON_RAW_PRETTY);
        println!("Writing '/models' raw response to:\n{}", dir.display());
        std::fs::write(dir, &pretty)?;
    }

    write_response(&env_string, &parsed)?;

    if env_string == "all" {
        for op in ["id", "arch", "top", "pricing"] {
            write_response(op, &parsed)?;
        }
    }

    fn write_response(env_str: &str, parsed: &models::Response) -> Result<()> {
        let mut dir = workspace_root();
        match env_str {
            "id" => {
                let names = parsed.data.iter().map(|r| r.id.to_string()).join("\n");
                dir.push(MODELS_TXT_IDS);
                println!(
                    "Writing '/models' id fields response to:\n{}",
                    dir.display()
                );
                std::fs::write(dir, &names)?;
            }
            "arch" => {
                let architecture = parsed
                    .data
                    .iter()
                    .map(|r| r.architecture.clone())
                    .collect_vec();
                let pretty_arch = serde_json::to_string_pretty(&architecture)?;
                dir.push(MODELS_JSON_ARCH);
                println!(
                    "Writing '/models' architecture fields response to:\n{}",
                    dir.display()
                );
                std::fs::write(dir, &pretty_arch)?;
            }
            "top" => {
                let top_provider = parsed
                    .data
                    .iter()
                    .map(|r| r.top_provider.clone())
                    .collect_vec();
                let pretty_arch = serde_json::to_string_pretty(&top_provider)?;
                dir.push(MODELS_JSON_TOP);
                println!(
                    "Writing '/models' top_provider fields response to:\n{}",
                    dir.display()
                );
                std::fs::write(dir, &pretty_arch)?;
            }
            "pricing" => {
                let pricing = parsed.data.iter().map(|r| r.pricing).collect_vec();
                let pretty_pricing = serde_json::to_string_pretty(&pricing)?;
                dir.push(MODELS_JSON_PRICING);
                println!(
                    "Writing '/models' pricing fields response to:\n{}",
                    dir.display()
                );
                std::fs::write(dir, &pretty_pricing)?;
            }
            "supported" => {
                let supported = parsed
                    .data
                    .iter()
                    .map(|r| r.supported_parameters.clone())
                    .collect_vec();
                let pretty_supported = serde_json::to_string_pretty(&supported)?;
                dir.push(MODELS_JSON_SUPPORTED);
                println!(
                    "Writing '/models' supported fields response to:\n{}",
                    dir.display()
                );
                std::fs::write(dir, &pretty_supported)?;
            }
            "canon" => {
                let canon = parsed
                    .data
                    .iter()
                    .filter_map(|r| r.canonical.as_ref().map(|c| c.to_string()))
                    .join("\n");
                dir.push(MODELS_TXT_CANON);
                println!(
                    "Writing '/models' canon fields response to:\n{}",
                    dir.display()
                );
                std::fs::write(dir, &canon)?;
            }
            "all" => { /* handled above, just avoiding print below */ }
            "raw" => { /* handled above, just avoiding print below */ }
            "raw_pretty" => { /* handled above, just avoiding print below */ }
            s => {
                println!(
                    "
Unkown command: {s}\nvalid choices:\n\traw\n\tall\n\tid\n\tarch\n\ttop
\tpricing\n\tcanon\n"
                );
            }
        }
        Ok(())
    }
    Ok(())
}

use color_eyre::Result;
use ploke_test_utils::workspace_root;
use reqwest::Client;
#[tokio::test]
#[cfg(feature = "live_api_tests")]
async fn test_simple_query_models() -> Result<()> {
    let url = OpenRouter::MODELS_URL;
    // let key = OpenRouter::resolve_api_key()?;

    let response = Client::new()
        .get(url)
        // auth not required for this request
        // .bearer_auth(key)
        .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| LlmError::Request(e.to_string()))?;

    let response_json = response.text().await?;

    parse_with_env(&response_json)?;

    Ok(())
}

#[test]
fn test_names_vs_ids() -> Result<()> {
    let mut in_file = workspace_root();
    in_file.push(MODELS_JSON_RAW);
    let mut out_file = workspace_root();
    out_file.push(MODELS_JSON_ID_NOT_NAME);

    let s = std::fs::read_to_string(in_file)?;

    let mr: models::Response = serde_json::from_str(&s)?;

    let not_equal = mr
        .into_iter()
        .map(|i| (i.id.key.to_string(), i.name.as_str().to_string()))
        .filter(|(k, n)| k != n)
        .collect_vec();
    let pretty = serde_json::to_string_pretty(&not_equal)?;

    std::fs::write(out_file, pretty)?;
    Ok(())
}

#[tokio::test]
#[cfg(feature = "live_api_tests")]
async fn test_default_query_endpoints() -> Result<()> {
    use std::{path::PathBuf, str::FromStr as _};

    use ploke_test_utils::workspace_root;

    // TODO: we need to handle more items like the below, which the `ModelKey` doesn't
    // currently handle:
    // - nousresearch/deephermes-3-llama-3-8b-preview:free
    // - Should turn into a raw curl request like:
    //  https://openrouter.ai/api/v1/models/nousresearch/deephermes-3-llama-3-8b-preview%3Afree/endpoints

    let model_key = OpenRouterModelId::from_str("qwen/qwen3-30b-a3b")?;
    let url = OpenRouter::endpoints_url(model_key);
    eprintln!(
        "Constructed url to query `/:author/:model/endpoints at\n{}",
        url
    );
    assert_eq!(
        "https://openrouter.ai/api/v1/models/qwen/qwen3-30b-a3b/endpoints",
        url
    );
    let key = OpenRouter::resolve_api_key()?;
    let mut dir = workspace_root();
    dir.push(cli::ENDPOINTS_JSON_DIR);

    let response = Client::new()
        .get(url)
        .bearer_auth(key)
        .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| LlmError::Request(e.to_string()))?;

    let is_success = response.status().is_success();
    eprintln!("is_success: {}", is_success);
    eprintln!("status: {}", response.status());
    let response_text = response.text().await?;

    let response_value: serde_json::Value = serde_json::from_str(&response_text)?;

    if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
        let response_raw_pretty = serde_json::to_string_pretty(&response_value)?;
        std::fs::create_dir_all(&dir)?;
        dir.push("endpoints.json");
        println!("Writing raw json reponse to: {}", dir.display());
        std::fs::write(dir, response_raw_pretty)?;
    }
    assert!(is_success);

    Ok(())
}

#[tokio::test]
#[cfg(feature = "live_api_tests")]
async fn test_free_query_endpoints() -> Result<()> {
    use ploke_test_utils::workspace_root;

    let model_key =
        OpenRouterModelId::from_str("nousresearch/deephermes-3-llama-3-8b-preview:free")?;
    let url = OpenRouter::endpoints_url(model_key);
    eprintln!(
        "Constructed url to query `/:author/:model/endpoints at\n{}",
        url
    );
    assert_eq!(
        "https://openrouter.ai/api/v1/models/nousresearch/deephermes-3-llama-3-8b-preview:free/endpoints",
        url
    );
    let key = OpenRouter::resolve_api_key()?;
    let mut dir = workspace_root();
    dir.push(cli::ENDPOINTS_JSON_DIR);

    let response = Client::new()
        .get(url)
        .bearer_auth(key)
        .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| LlmError::Request(e.to_string()))?;

    let is_success = response.status().is_success();
    eprintln!("is_success: {}", is_success);
    eprintln!("status: {}", response.status());
    let response_text = response.text().await?;

    let response_value: serde_json::Value = serde_json::from_str(&response_text)?;

    if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
        let response_raw_pretty = serde_json::to_string_pretty(&response_value)?;
        std::fs::create_dir_all(&dir)?;
        dir.push("free_model.json");
        println!("Writing raw json reponse to: {}", dir.display());
        std::fs::write(dir, response_raw_pretty)?;
    }
    assert!(is_success);

    Ok(())
}

#[tokio::test]
#[cfg(feature = "live_api_tests")]
async fn test_default_post_completions() -> Result<()> {
    use crate::llm::{ModelId, router_only::cli::COMPLETION_JSON_SIMPLE_DIR};
    use openrouter::OpenRouterModelId;
    use std::path::PathBuf;

    use ploke_test_utils::workspace_root;

    let model_id = OpenRouterModelId::from_str("qwen/qwen3-30b-a3b-thinking-2507")?;
    let key = OpenRouter::resolve_api_key()?;
    let url = OpenRouter::COMPLETION_URL;
    let mut dir = workspace_root();
    dir.push(COMPLETION_JSON_SIMPLE_DIR);

    let content = String::from("Hello, can you tell me about lifetimes in Rust?");
    let msg = RequestMessage {
        role: Role::User,
        content,
        tool_call_id: None,
    };

    let req = ChatCompRequest::<OpenRouter> {
        router: openrouter::ChatCompFields::default(),
        core: ChatCompReqCore {
            messages: vec![msg],
            ..Default::default()
        },
        ..Default::default()
    };

    if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
        let pretty = serde_json::to_string_pretty(&req)?;
        dir.push("request_se.json");
        println!("Writing serialized request to: {}", dir.display());
        std::fs::write(&dir, pretty)?;
    }

    let response = Client::new()
        .post(url)
        .bearer_auth(key)
        .json(&req)
        .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| LlmError::Request(e.to_string()))?;
    let is_success = response.status().is_success();
    eprintln!("is_success: {}", is_success);
    eprintln!("status: {}", response.status());

    // let response_text = response.json().await?;

    // let response_value: serde_json::Value = serde_json::from_str(&response_text)?;
    let response_value: serde_json::Value = response.json().await?;

    if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
        let response_raw_pretty = serde_json::to_string_pretty(&response_value)?;
        dir.pop();
        dir.push("response_raw.json");
        println!("Writing raw json reponse to: {}", dir.display());
        std::fs::write(dir, response_raw_pretty)?;
    }
    assert!(is_success);

    Ok(())
}

#[test]
fn test_chat_comp_request_serialization_minimal() {
    use crate::llm::manager::RequestMessage;
    use crate::llm::request::ChatCompReqCore;
    use crate::llm::request::endpoint::ToolChoice;
    use crate::llm::router_only::default_model;
    use crate::tools::GetFileMetadata;

    let messages = vec![
        RequestMessage::new_system("sys".to_string()),
        RequestMessage::new_user("hello".to_string()),
    ];

    let default_model = default_model();
    let req = ChatCompRequest::<OpenRouter>::default()
        .with_core_bundle(ChatCompReqCore::default())
        .with_model_str(&default_model)
        .unwrap()
        .with_messages(messages)
        .with_temperature(0.0)
        .with_max_tokens(128);
    // let req = openrouter::ChatCompFields::default()
    //     .completion_core(ChatCompReqCore::default())
    //     .with_model_str(&default_model)
    //     .map(|r| r.with_messages(messages))
    //     .unwrap()
    //     .with_temperature(0.0)
    //     .with_max_tokens(128);
    let mut req = req;
    req.tools = Some(vec![GetFileMetadata::tool_def()]);
    req.tool_choice = Some(ToolChoice::Auto);

    let v = serde_json::to_value(&req).expect("serialize ChatCompRequest");
    // Top-level fields present
    assert_eq!(v.get("tool_choice").and_then(|t| t.as_str()), Some("auto"));
    assert_eq!(
        v.get("model").and_then(|m| m.as_str()),
        Some(default_model.as_str())
    );
    // Messages array content
    let msgs = v
        .get("messages")
        .and_then(|m| m.as_array())
        .expect("messages");
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].get("role").and_then(|r| r.as_str()), Some("system"));
    assert_eq!(msgs[1].get("role").and_then(|r| r.as_str()), Some("user"));
    // Tools
    let tools = v.get("tools").and_then(|t| t.as_array()).expect("tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(|s| s.as_str()),
        Some("function")
    );
}
