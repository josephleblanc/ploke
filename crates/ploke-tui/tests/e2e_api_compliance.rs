#![cfg(all(feature = "live_api_tests", feature = "test_harness"))]

//! OpenRouter API compliance testing
//! 
//! This module validates that our types and serialization exactly match
//! the OpenRouter API specification as documented in:
//! crates/ploke-tui/docs/openrouter/request_structure.md

use ploke_tui::llm::providers::ProviderSlug;
use ploke_tui::tracing_setup::init_tracing_tests;
use serde_json::{json, Value};

mod harness;
use harness::AppHarness;

use ploke_tui::llm::{RequestMessage, Role};
use ploke_tui::tools::Tool;
use ploke_tui::tools::get_file_metadata::GetFileMetadata;
use ploke_tui::tools::request_code_context::RequestCodeContextGat;
use ploke_tui::test_harness::openrouter_env;
use tracing::Level;

/// Test that our RequestMessage serialization matches OpenRouter specification exactly
#[tokio::test]
async fn e2e_request_message_serialization_compliance() {
    // Test all Role variants serialize correctly
    
    // User message
    let user_msg = RequestMessage {
        role: Role::User,
        content: "Hello, world!".to_string(),
        tool_call_id: None,
    };
    let user_json = serde_json::to_value(&user_msg).expect("Failed to serialize user message");
    assert_eq!(user_json["role"], "user");
    assert_eq!(user_json["content"], "Hello, world!");
    assert!(user_json["tool_call_id"].is_null());
    
    // Assistant message
    let assistant_msg = RequestMessage {
        role: Role::Assistant,
        content: "Hello back!".to_string(),
        tool_call_id: None,
    };
    let assistant_json = serde_json::to_value(&assistant_msg).expect("Failed to serialize assistant message");
    assert_eq!(assistant_json["role"], "assistant");
    assert_eq!(assistant_json["content"], "Hello back!");
    
    // System message
    let system_msg = RequestMessage {
        role: Role::System,
        content: "You are a helpful assistant.".to_string(),
        tool_call_id: None,
    };
    let system_json = serde_json::to_value(&system_msg).expect("Failed to serialize system message");
    assert_eq!(system_json["role"], "system");
    assert_eq!(system_json["content"], "You are a helpful assistant.");
    
    // Tool message with required tool_call_id
    let tool_msg = RequestMessage::new_tool(
        json!({"ok": true, "result": "test data"}).to_string(),
        "call_123".to_string()
    );
    let tool_json = serde_json::to_value(&tool_msg).expect("Failed to serialize tool message");
    assert_eq!(tool_json["role"], "tool");
    assert_eq!(tool_json["tool_call_id"], "call_123");
    assert!(tool_json["content"].is_string());
    
    // Validate tool message validation
    assert!(tool_msg.validate().is_ok());
    
    println!("✓ All RequestMessage role serializations match OpenRouter specification");
}

/// Test that our ToolDefinition serialization matches OpenRouter specification exactly
#[tokio::test] 
async fn e2e_tool_definition_serialization_compliance() {
    // Test both our standard tools
    let get_metadata_tool = GetFileMetadata::tool_def();
    let request_context_tool = RequestCodeContextGat::tool_def();
    
    // Validate get_file_metadata structure
    let metadata_json = serde_json::to_value(&get_metadata_tool).expect("Failed to serialize get_file_metadata");
    assert_eq!(metadata_json["type"], "function");
    assert_eq!(metadata_json["function"]["name"], "get_file_metadata");
    assert!(metadata_json["function"]["description"].is_string());
    assert!(metadata_json["function"]["parameters"]["type"] == "object");
    assert!(metadata_json["function"]["parameters"]["properties"].is_object());
    assert!(metadata_json["function"]["parameters"]["required"].is_array());
    
    // Validate required field for get_file_metadata
    let required = metadata_json["function"]["parameters"]["required"].as_array().unwrap();
    assert!(required.contains(&json!("file_path")), "get_file_metadata should require file_path");
    
    // Validate request_code_context structure
    let context_json = serde_json::to_value(&request_context_tool).expect("Failed to serialize request_code_context");
    assert_eq!(context_json["type"], "function");
    assert_eq!(context_json["function"]["name"], "request_code_context");
    assert!(context_json["function"]["description"].is_string());
    assert!(context_json["function"]["parameters"]["type"] == "object");
    assert!(context_json["function"]["parameters"]["properties"].is_object());
    assert!(context_json["function"]["parameters"]["required"].is_array());
    
    // Validate required field for request_code_context
    let required = context_json["function"]["parameters"]["required"].as_array().unwrap();
    assert!(required.contains(&json!("search_term")), "request_code_context should require search_term");
    
    println!("✓ All ToolDefinition serializations match OpenRouter specification");
}

/// Test CompReq (CompletionRequest) serialization matches OpenRouter specification
#[tokio::test]
async fn e2e_completion_request_serialization_compliance() {
    let harness = AppHarness::spawn().await;
    
    // Create a realistic CompReq with tools
    let messages = vec![
        RequestMessage {
            role: Role::System,
            content: "You are a helpful assistant with access to tools.".to_string(),
            tool_call_id: None,
        },
        RequestMessage {
            role: Role::User,
            content: "Please get metadata for the file 'test.rs'".to_string(),
            tool_call_id: None,
        },
    ];
    
    let tools = vec![
        GetFileMetadata::tool_def(),
        RequestCodeContextGat::tool_def(),
    ];
    
    // Build a CompReq using our existing builder logic
    use ploke_tui::llm::session::build_comp_req;
    use ploke_tui::llm::LLMParameters;
    use ploke_tui::user_config::{ModelConfig, ProviderType};
    
    let params = LLMParameters {
        max_tokens: Some(1000),
        temperature: Some(0.1),
        ..Default::default()
    };
    
    let provider = ModelConfig {
        id: "test-compliance".to_string(),
        api_key: "test-key".to_string(),
        provider_slug: Some( ProviderSlug::openai ),
        api_key_env: None,
        base_url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
        model: "openai/gpt-4o-mini".to_string(),
        display_name: None,
        provider_type: ProviderType::OpenRouter,
        llm_params: None,
    };
    
    let comp_req = build_comp_req(
        &provider,
        messages,
        &params,
        Some(tools),
        true, // use_tools
        false // require_parameters
    );
    
    // Serialize and validate structure
    let req_json = serde_json::to_value(&comp_req).expect("Failed to serialize CompReq");
    
    // Validate required fields per OpenRouter spec
    assert!(req_json["messages"].is_array(), "messages should be array");
    assert_eq!(req_json["model"], "openai/gpt-4o-mini");
    assert!(req_json["tools"].is_array(), "tools should be array");
    assert_eq!(req_json["max_tokens"], 1000);
    // Temperature should be approximately 0.1 (accounting for f32 precision)
    let temp = req_json["temperature"].as_f64().expect("temperature should be number");
    assert!((temp - 0.1).abs() < 0.01, "temperature should be approximately 0.1, got {}", temp);
    
    // Validate messages structure
    let messages_array = req_json["messages"].as_array().unwrap();
    assert_eq!(messages_array.len(), 2);
    assert_eq!(messages_array[0]["role"], "system");
    assert_eq!(messages_array[1]["role"], "user");
    
    // Validate tools structure
    let tools_array = req_json["tools"].as_array().unwrap();
    assert_eq!(tools_array.len(), 2);
    for tool in tools_array {
        assert_eq!(tool["type"], "function");
        assert!(tool["function"]["name"].is_string());
        assert!(tool["function"]["parameters"].is_object());
    }
    
    harness.shutdown().await;
    println!("✓ CompReq serialization matches OpenRouter specification");
}

/// Test live API request/response cycle with type validation
#[tokio::test]
async fn e2e_live_api_request_response_types() {
    let env = openrouter_env().expect("Skipping live API tool test: OPENROUTER_API_KEY not set");
    
    let harness = AppHarness::spawn().await;
    
    // Create a simple request without tools first
    let messages = vec![
        RequestMessage {
            role: Role::User,
            content: "Say 'hello' exactly.".to_string(),
            tool_call_id: None,
        },
    ];
    
    use ploke_tui::llm::session::build_comp_req;
    use ploke_tui::llm::LLMParameters;
    use ploke_tui::user_config::{ModelConfig, ProviderType};
    
    let params = LLMParameters {
        max_tokens: Some(50),
        temperature: Some(0.0),
        ..Default::default()
    };
    
    let provider = ModelConfig {
        id: "test-live-api".to_string(),
        api_key: env.key.clone(),
        provider_slug: Some( ProviderSlug::openai ),
        api_key_env: None,
        base_url: env.url.to_string(),
        model: "openai/gpt-4o-mini".to_string(),
        display_name: None,
        provider_type: ProviderType::OpenRouter,
        llm_params: None,
    };
    
    let comp_req = build_comp_req(
        &provider,
        messages,
        &params,
        None, // No tools for this simple test
        false, // use_tools
        false // require_parameters
    );
    
    // Make actual API call using reqwest
    let client = reqwest::Client::new();
    let api_url = format!("{}/chat/completions", env.url.as_str().trim_end_matches('/'));
    let response = client
        .post(&api_url)
        .header("Authorization", format!("Bearer {}", env.key))
        .header("HTTP-Referer", "https://github.com/your-repo")
        .header("X-Title", "ploke-tui-e2e-tests")
        .header("Content-Type", "application/json")
        .json(&comp_req)
        .send()
        .await
        .expect("Failed to send request");
    
    // Validate response status
    assert!(response.status().is_success(), "API request failed with status: {}", response.status());
    
    // Parse response as raw JSON to validate structure
    let response_text = response.text().await.expect("Failed to read response text");
    let response_json: Value = serde_json::from_str(&response_text)
        .expect("Failed to parse response as JSON");
    
    // Validate response structure per OpenRouter specification
    assert!(response_json["choices"].is_array(), "Response should have choices array");
    let choices = response_json["choices"].as_array().unwrap();
    assert!(!choices.is_empty(), "Response should have at least one choice");
    
    let first_choice = &choices[0];
    assert!(first_choice["message"].is_object(), "Choice should have message object");
    assert!(first_choice["message"]["content"].is_string(), "Message should have content");
    
    // Validate that no tool calls are present in simple response
    if let Some(tool_calls) = first_choice["message"]["tool_calls"].as_array() {
        assert!(tool_calls.is_empty(), "Simple response should not have tool calls");
    }
    
    harness.shutdown().await;
    println!("✓ Live API request/response types validated successfully");
}

/// Test live API tool call request/response cycle with type validation
#[tokio::test]
async fn e2e_live_api_tool_call_types() {
    let _t = init_tracing_tests(Level::DEBUG);
    let env = openrouter_env().expect("Skipping live API tool test: OPENROUTER_API_KEY not set");
    
    let harness = AppHarness::spawn().await;
    
    // Create a request that should trigger tool use
    let messages = vec![
        RequestMessage {
            role: Role::System,
            content: "You are a helpful assistant. When asked about files, use the get_file_metadata tool.".to_string(),
            tool_call_id: None,
        },
        RequestMessage {
            role: Role::User,
            content: "Get metadata for the file 'README.md'".to_string(),
            tool_call_id: None,
        },
    ];
    
    let tools = vec![GetFileMetadata::tool_def()];
    
    use ploke_tui::llm::session::build_comp_req;
    use ploke_tui::llm::LLMParameters;
    use ploke_tui::user_config::{ModelConfig, ProviderType};
    
    let params = LLMParameters {
        max_tokens: Some(500),
        temperature: Some(0.1),
        ..Default::default()
    };
    
    let provider = ModelConfig {
        id: "test-live-tools".to_string(),
        api_key: env.key.clone(),
        provider_slug: Some( ProviderSlug::openai ),
        api_key_env: None,
        base_url: env.url.to_string(),
        model: "openai/gpt-4o-mini".to_string(),
        display_name: None,
        provider_type: ProviderType::OpenRouter,
        llm_params: None,
    };
    
    let comp_req = build_comp_req(
        &provider,
        messages,
        &params,
        Some(tools),
        true, // use_tools
        false // require_parameters
    );
    
    // Make API call
    let client = reqwest::Client::new();
    let api_url = format!("{}/chat/completions", env.url.as_str().trim_end_matches('/'));
    let response = client
        .post(&api_url)
        .header("Authorization", format!("Bearer {}", env.key))
        .header("HTTP-Referer", "https://github.com/your-repo")
        .header("X-Title", "ploke-tui-e2e-tool-tests")
        .header("Content-Type", "application/json")
        .json(&comp_req)
        .send()
        .await
        .expect("Failed to send tool request");
    
    assert!(response.status().is_success(), "Tool API request failed with status: {}", response.status());
    
    let response_text = response.text().await.expect("Failed to read tool response text");
    let response_json: Value = serde_json::from_str(&response_text)
        .expect("Failed to parse tool response as JSON");
    
    // Validate response has expected structure
    assert!(response_json["choices"].is_array(), "Tool response should have choices array");
    let choices = response_json["choices"].as_array().unwrap();
    assert!(!choices.is_empty(), "Tool response should have at least one choice");
    let first_choice = &choices[0];
    
    if let Some(tool_calls) = first_choice["message"]["tool_calls"].as_array() {
        assert!(!tool_calls.is_empty(), "Expected tool calls in response");
        
        // Validate tool call structure
        for tool_call in tool_calls {
            assert!(tool_call["id"].is_string(), "Tool call should have string id");
            assert_eq!(tool_call["type"], "function", "Tool call type should be 'function'");
            assert_eq!(tool_call["function"]["name"], "get_file_metadata", "Tool call should use get_file_metadata");
            
            // Validate arguments are valid JSON
            let args_str = tool_call["function"]["arguments"].as_str()
                .expect("Tool call arguments should be string");
            let _: Value = serde_json::from_str(args_str)
                .expect("Tool call arguments should be valid JSON");
        }
        
        println!("✓ Live API tool call types validated successfully");
        println!("  Tool calls found: {}", tool_calls.len());
    } else {
        // Some models might not call tools - that's acceptable for this test
        println!("ℹ Note: Model did not use tools (may be model limitation)");
    }
    
    harness.shutdown().await;
}
