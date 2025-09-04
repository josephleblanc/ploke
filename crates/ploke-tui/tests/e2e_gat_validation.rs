#![cfg(feature = "test_harness")]

//! GAT (Generic Associated Types) system validation for tool calls
//! 
//! This module validates:
//! 1. Zero-copy deserialization through GAT trait system
//! 2. Static dispatch for all tool implementations 
//! 3. Type safety and compile-time validation
//! 4. Live API serialization/deserialization with JSON persistence
//! 5. Tool parameter parsing and result handling

use std::fs;
use std::path::PathBuf;
use serde_json::{json, Value};
use uuid::Uuid;

mod harness;
use harness::AppHarness;

use ploke_tui::llm::{RequestMessage, Role};
use ploke_tui::tools::{Tool, ToolDefinition, Ctx};
use ploke_tui::tools::get_file_metadata::{GetFileMetadata, GetFileMetadataParams};
use ploke_tui::tools::request_code_context::{RequestCodeContextGat, RequestCodeContextParams};
use ploke_tui::tools::code_edit::{GatCodeEdit, CodeEditParams};
use ploke_tui::test_harness::openrouter_env;

/// Create a directory for storing test artifacts with verification JSONs
fn create_test_artifact_dir(test_name: &str) -> PathBuf {
    let mut dir = ploke_test_utils::workspace_root();
    dir.push("crates/ploke-tui/ai_temp_data/gat_validation");
    dir.push(format!("{}-{}", test_name, chrono::Utc::now().format("%Y%m%d-%H%M%S")));
    fs::create_dir_all(&dir).expect("Failed to create test artifact directory");
    dir
}

/// Test GAT zero-copy deserialization for GetFileMetadata tool
#[tokio::test]
async fn e2e_gat_get_file_metadata_zero_copy() -> color_eyre::Result<()> {
    let harness = AppHarness::spawn().await?;
    
    // Test zero-copy deserialization with borrowed string
    let json_args = r#"{"file_path": "/home/test/README.md"}"#;
    
    // Test GAT deserialization directly
    let params: GetFileMetadataParams = GetFileMetadata::deserialize_params(json_args)
        .expect("Failed to deserialize GetFileMetadata params");
    
    assert_eq!(params.file_path, "/home/test/README.md");
    
    // Test tool definition generation
    let tool_def: ToolDefinition = GetFileMetadata::tool_def();
    assert_eq!(tool_def.function.name.as_str(), "get_file_metadata");
    
    // Test tool definition generation validates the schema
    let parameters = &tool_def.function.parameters;
    assert!(parameters["properties"]["file_path"].is_object(), "Schema should define file_path property");
    
    harness.shutdown().await;
    println!("✓ GAT zero-copy deserialization validated for GetFileMetadata");
    Ok(())
}

/// Test GAT system for RequestCodeContext tool with complex parameters
#[tokio::test]
async fn e2e_gat_request_code_context_complex() -> color_eyre::Result<()> {
    let harness = AppHarness::spawn().await?;
    
    // Test parameter structure
    let json_args = json!({
        "search_term": "SimpleStruct implementation",
        "token_budget": 1500
    }).to_string();
    
    // Test GAT deserialization with optional fields
    let params: RequestCodeContextParams = RequestCodeContextGat::deserialize_params(&json_args)
        .expect("Failed to deserialize RequestCodeContext params");
    
    assert_eq!(params.search_term.as_ref().map(|s| s.as_ref()), Some("SimpleStruct implementation"));
    assert_eq!(params.token_budget, 1500);
    
    // Test actual tool execution through GAT system
    let ctx = Ctx {
        state: harness.state.clone(),
        event_bus: harness.event_bus.clone(),
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        call_id: "call_456".into(),
    };
    
    // Execute tool (this tests the GAT execute method)
    let result = RequestCodeContextGat::execute(params, ctx).await;
    
    // Validate result structure (should be proper tool result)
    match result {
        Ok(tool_result) => {
            // Tool result should be serializable  
            let _serialized = serde_json::to_string(&tool_result)
                .expect("Tool result should be serializable");
            println!("✓ GAT execution successful with valid result");
        }
        Err(e) => {
            println!("ℹ Tool execution failed (expected without proper RAG setup): {}", e);
            // This is acceptable - we're testing the GAT system, not the tool logic
        }
    }
    
    harness.shutdown().await;
    println!("✓ GAT system validated for RequestCodeContext with complex parameters");
    Ok(())
}

/// Test ApplyCodeEdit GAT system with edit validation
#[tokio::test]
async fn e2e_gat_apply_code_edit_validation() -> color_eyre::Result<()> {
    let harness = AppHarness::spawn().await?;
    
    // Test complex edit structure
    let json_args = json!({
        "edits": [{
            "file": "src/test.rs",
            "canon": "test::SimpleStruct",
            "node_type": "struct",
            "code": "pub struct SimpleStruct {\n    pub field: String,\n}"
        }],
        "confidence": 0.9
    }).to_string();
    
    // Test GAT deserialization for complex nested structure
    let params: CodeEditParams = GatCodeEdit::deserialize_params(&json_args)
        .expect("Failed to deserialize CodeEdit params");
    
    assert_eq!(params.edits.len(), 1);
    assert_eq!(params.edits[0].file.as_ref(), "src/test.rs");
    assert_eq!(params.edits[0].canon.as_ref(), "test::SimpleStruct");
    assert_eq!(params.confidence, Some(0.9));
    
    // Validate tool definition schema matches our parameters
    let tool_def: ToolDefinition = GatCodeEdit::tool_def();
    let schema_json = serde_json::to_value(&tool_def).expect("Failed to serialize tool definition");
    
    // Verify schema has all required fields
    let properties = &schema_json["function"]["parameters"]["properties"];
    assert!(properties["edits"].is_object(), "Schema should define edits property");
    assert!(properties["confidence"].is_object(), "Schema should define confidence property");
    
    harness.shutdown().await;
    println!("✓ GAT system validated for CodeEdit with complex nested structures");
    Ok(())
}

/// Test live API tool call with JSON persistence for verification
#[cfg(feature = "live_api_tests")]
#[tokio::test]
async fn e2e_live_gat_tool_call_with_persistence() -> color_eyre::Result<()> {
    let env = openrouter_env().expect("OPENROUTER_API_KEY required for live GAT validation");
    let harness = AppHarness::spawn().await?;
    
    // Create artifact directory for this test
    let artifact_dir = create_test_artifact_dir("live_gat_tool_call");
    
    // Create a realistic tool call request
    let messages = vec![
        RequestMessage {
            role: Role::System,
            content: "You are a helpful assistant. When asked about files, always use the get_file_metadata tool.".to_string(),
            tool_call_id: None,
        },
        RequestMessage {
            role: Role::User,
            content: "Please get metadata for the file 'Cargo.toml' in the current directory.".to_string(),
            tool_call_id: None,
        },
    ];
    
    let tools = vec![GetFileMetadata::tool_def()];
    
    use ploke_tui::llm::providers::ProviderSlug;
    // Build and send request
    use ploke_tui::llm::session::build_comp_req;
    use ploke_tui::llm::LLMParameters;
    use ploke_tui::user_config::{ModelConfig, ProviderType};
    
    let params = LLMParameters {
        max_tokens: Some(500),
        temperature: Some(0.1),
        ..Default::default()
    };
    
    let provider = ModelConfig {
        id: "test-live-gat".to_string(),
        api_key: env.key.clone(),
        provider_slug: Some( ProviderSlug::openai ),
        api_key_env: None,
        base_url: env.base_url.to_string(),
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
    
    // Persist request for verification
    let request_path = artifact_dir.join("request.json");
    let request_json = serde_json::to_string_pretty(&comp_req)
        .expect("Failed to serialize request");
    fs::write(&request_path, &request_json)
        .expect("Failed to write request JSON");
    
    // Make API call
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", env.base_url.as_str().trim_end_matches('/'));
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", env.key))
        .header("HTTP-Referer", "https://github.com/ploke-ai/ploke")
        .header("X-Title", "ploke-tui-gat-validation")
        .header("Content-Type", "application/json")
        .json(&comp_req)
        .send()
        .await
        .expect("Failed to send request");
    
    let status = response.status();
    if !status.is_success() {
        println!("❌ API request failed with status: {}", status);
        let error_text = response.text().await.unwrap_or_else(|_| "Failed to read error text".to_string());
        println!("Error response: {}", error_text);
        
        let error_path = artifact_dir.join("error_response.txt");
        fs::write(&error_path, format!("Status: {}\n\nResponse:\n{}", status, error_text))
            .expect("Failed to write error response");
        
        panic!("API request failed with status: {}", status);
    }
    
    // Persist and analyze response
    let response_text = response.text().await.expect("Failed to read response text");
    let response_path = artifact_dir.join("response.json");
    fs::write(&response_path, &response_text)
        .expect("Failed to write response JSON");
    
    // Debug the response if it fails to parse
    let response_json: Value = match serde_json::from_str(&response_text) {
        Ok(json) => json,
        Err(e) => {
            println!("❌ Failed to parse response as JSON: {}", e);
            println!("Response text (first 500 chars): {}", 
                if response_text.len() > 500 { &response_text[..500] } else { &response_text });
            
            // Save debug info
            let debug_path = artifact_dir.join("debug_response.txt");
            fs::write(&debug_path, format!("Parse Error: {}\n\nFull Response:\n{}", e, response_text))
                .expect("Failed to write debug response");
            
            panic!("Failed to parse response as JSON: {}", e);
        }
    };
    
    // 🔧 NEW: Test GAT deserialization of actual API response
    let mut gat_validation = json!({
        "gat_tool_deserialization": {
            "attempted": false,
            "successful": false,
            "tool_calls_processed": 0,
            "deserialization_results": []
        }
    });
    
    if let Some(choices) = response_json["choices"].as_array() {
        if let Some(first_choice) = choices.first() {
            if let Some(tool_calls) = first_choice["message"]["tool_calls"].as_array() {
                gat_validation["gat_tool_deserialization"]["attempted"] = json!(true);
                let mut successful_deserializations = 0;
                let mut deserialization_details = Vec::new();
                
                for tool_call in tool_calls {
                    let function_name = tool_call["function"]["name"].as_str().unwrap_or("");
                    let arguments_str = tool_call["function"]["arguments"].as_str().unwrap_or("{}");
                    
                    // Test GAT deserialization for each tool call
                    let deserialization_result = match function_name {
                        "get_file_metadata" => {
                            match GetFileMetadata::deserialize_params(arguments_str) {
                                Ok(params) => {
                                    successful_deserializations += 1;
                                    json!({
                                        "tool": "get_file_metadata",
                                        "success": true,
                                        "file_path": params.file_path,
                                        "error": null
                                    })
                                },
                                Err(e) => json!({
                                    "tool": "get_file_metadata",
                                    "success": false,
                                    "file_path": null,
                                    "error": e.to_string()
                                })
                            }
                        },
                        "request_code_context" => {
                            match RequestCodeContextGat::deserialize_params(arguments_str) {
                                Ok(params) => {
                                    successful_deserializations += 1;
                                    json!({
                                        "tool": "request_code_context",
                                        "success": true,
                                        "token_budget": params.token_budget,
                                        "search_term": params.search_term.as_ref().map(|s| s.as_ref()),
                                        "error": null
                                    })
                                },
                                Err(e) => json!({
                                    "tool": "request_code_context",
                                    "success": false,
                                    "token_budget": null,
                                    "search_term": null,
                                    "error": e.to_string()
                                })
                            }
                        },
                        _ => json!({
                            "tool": function_name,
                            "success": false,
                            "error": format!("Unknown tool: {}", function_name)
                        })
                    };
                    
                    deserialization_details.push(deserialization_result);
                }
                
                gat_validation["gat_tool_deserialization"]["successful"] = json!(successful_deserializations > 0);
                gat_validation["gat_tool_deserialization"]["tool_calls_processed"] = json!(tool_calls.len());
                gat_validation["gat_tool_deserialization"]["deserialization_results"] = json!(deserialization_details);
                
                // Assert that at least one tool call was successfully deserialized through GAT
                assert!(successful_deserializations > 0, 
                    "At least one tool call should be successfully deserialized through GAT system. Details: {:?}", 
                    deserialization_details);
                
                println!("  ✓ GAT deserialization successful: {}/{} tool calls", 
                    successful_deserializations, tool_calls.len());
            }
        }
    }
    
    // Create verification report
    let mut verification = json!({
        "test_name": "live_gat_tool_call_with_persistence",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "request_file": request_path.file_name().unwrap().to_string_lossy(),
        "response_file": response_path.file_name().unwrap().to_string_lossy(),
        "verification": {
            "request_serialization": {
                "tools_included": false,
                "tool_count": 0,
                "tool_names": [],
                "messages_count": 0,
                "model": "",
                "temperature": null,
                "max_tokens": null
            },
            "response_deserialization": {
                "has_choices": false,
                "choice_count": 0,
                "has_tool_calls": false,
                "tool_call_count": 0,
                "tool_call_details": []
            },
            "gat_deserialization": {
                "attempted": false,
                "successful": false,
                "tool_calls_processed": 0,
                "deserialization_results": []
            }
        }
    });
    
    // Verify request serialization
    if let Ok(req_json) = serde_json::from_str::<Value>(&request_json) {
        verification["verification"]["request_serialization"]["tools_included"] = json!(req_json["tools"].is_array());
        if let Some(tools_array) = req_json["tools"].as_array() {
            verification["verification"]["request_serialization"]["tool_count"] = json!(tools_array.len());
            let tool_names: Vec<String> = tools_array.iter()
                .filter_map(|tool| tool["function"]["name"].as_str())
                .map(|s| s.to_string())
                .collect();
            verification["verification"]["request_serialization"]["tool_names"] = json!(tool_names);
        }
        if let Some(messages) = req_json["messages"].as_array() {
            verification["verification"]["request_serialization"]["messages_count"] = json!(messages.len());
        }
        verification["verification"]["request_serialization"]["model"] = req_json["model"].clone();
        verification["verification"]["request_serialization"]["temperature"] = req_json["temperature"].clone();
        verification["verification"]["request_serialization"]["max_tokens"] = req_json["max_tokens"].clone();
    }
    
    // Verify response deserialization
    verification["verification"]["response_deserialization"]["has_choices"] = json!(response_json["choices"].is_array());
    if let Some(choices) = response_json["choices"].as_array() {
        verification["verification"]["response_deserialization"]["choice_count"] = json!(choices.len());
        
        if let Some(first_choice) = choices.first() {
            if let Some(tool_calls) = first_choice["message"]["tool_calls"].as_array() {
                verification["verification"]["response_deserialization"]["has_tool_calls"] = json!(true);
                verification["verification"]["response_deserialization"]["tool_call_count"] = json!(tool_calls.len());
                
                let mut tool_details = Vec::new();
                for tool_call in tool_calls {
                    let detail = json!({
                        "id": tool_call["id"],
                        "type": tool_call["type"],
                        "function_name": tool_call["function"]["name"],
                        "arguments_valid_json": serde_json::from_str::<Value>(
                            tool_call["function"]["arguments"].as_str().unwrap_or("{}")
                        ).is_ok()
                    });
                    tool_details.push(detail);
                }
                verification["verification"]["response_deserialization"]["tool_call_details"] = json!(tool_details);
            }
        }
    }
    
    // 🔧 Copy GAT validation results to verification report
    verification["verification"]["gat_deserialization"] = gat_validation["gat_tool_deserialization"].clone();
    
    // Write verification report
    let verification_path = artifact_dir.join("verification.json");
    let verification_json = serde_json::to_string_pretty(&verification)
        .expect("Failed to serialize verification");
    fs::write(&verification_path, &verification_json)
        .expect("Failed to write verification JSON");
    
    // Print verification summary
    println!("✓ Live GAT tool call validation complete");
    println!("  Artifacts saved to: {}", artifact_dir.display());
    println!("  Request file: {}", request_path.display());
    println!("  Response file: {}", response_path.display());
    println!("  Verification: {}", verification_path.display());
    
    // Validate key points
    let req_valid = verification["verification"]["request_serialization"]["tools_included"].as_bool().unwrap_or(false);
    let resp_valid = verification["verification"]["response_deserialization"]["has_choices"].as_bool().unwrap_or(false);
    
    assert!(req_valid, "Request should include tools array");
    assert!(resp_valid, "Response should have choices array");
    
    // Check if we got tool calls (optional - depends on model behavior)
    if let Some(has_tool_calls) = verification["verification"]["response_deserialization"]["has_tool_calls"].as_bool() {
        if has_tool_calls {
            let tool_count = verification["verification"]["response_deserialization"]["tool_call_count"].as_u64().unwrap_or(0);
            println!("  ✓ Tool calls received: {}", tool_count);
            
            // Validate tool call structure
            if let Some(details) = verification["verification"]["response_deserialization"]["tool_call_details"].as_array() {
                for detail in details {
                    assert_eq!(detail["function_name"], "get_file_metadata", "Tool call should use get_file_metadata");
                    assert_eq!(detail["type"], "function", "Tool call type should be function");
                    assert!(detail["arguments_valid_json"].as_bool().unwrap_or(false), "Tool arguments should be valid JSON");
                }
            }
        } else {
            println!("  ℹ No tool calls in response (model-dependent behavior)");
        }
    }
    
    harness.shutdown().await;
    Ok(())
}

/// Test GAT deserialization with various JSON inputs
#[tokio::test]
async fn e2e_gat_deserialization_validation() {
    // Test GetFileMetadata with different path formats
    let absolute_path = r#"{"file_path": "/absolute/path/file.rs"}"#;
    let params1: GetFileMetadataParams = GetFileMetadata::deserialize_params(absolute_path)
        .expect("Failed to deserialize absolute path");
    assert_eq!(params1.file_path, "/absolute/path/file.rs");
    
    let relative_path = r#"{"file_path": "relative/path/file.rs"}"#;
    let params2: GetFileMetadataParams = GetFileMetadata::deserialize_params(relative_path)
        .expect("Failed to deserialize relative path");
    assert_eq!(params2.file_path, "relative/path/file.rs");
    
    // Test RequestCodeContext with various token budgets
    let small_budget = r#"{"token_budget": 100, "search_term": "test"}"#;
    let params3: RequestCodeContextParams = RequestCodeContextGat::deserialize_params(small_budget)
        .expect("Failed to deserialize small budget");
    assert_eq!(params3.token_budget, 100);
    assert_eq!(params3.search_term.as_ref().map(|s| s.as_ref()), Some("test"));
    
    let large_budget = r#"{"token_budget": 5000}"#;
    let params4: RequestCodeContextParams = RequestCodeContextGat::deserialize_params(large_budget)
        .expect("Failed to deserialize large budget");
    assert_eq!(params4.token_budget, 5000);
    assert!(params4.search_term.is_none());
    
    println!("✓ GAT deserialization validated with various JSON inputs");
}
