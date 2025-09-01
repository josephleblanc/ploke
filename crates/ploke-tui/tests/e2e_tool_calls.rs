#![cfg(feature = "test_harness")]

use std::sync::Arc;
use tokio::time::{timeout, Duration};
use uuid::Uuid;
use serde_json::json;

use ploke_tui::event_bus::EventBusCaps;
use ploke_tui::{AppEvent, EventBus};
use ploke_tui::system::SystemEvent;
use ploke_tui::rag::utils::ToolCallParams;
use ploke_test_utils::workspace_root;
use ploke_tui::tools::{FunctionMarker, Tool, ToolDefinition};
use ploke_tui::tools::request_code_context::RequestCodeContextGat;

#[cfg(all(feature = "live_api_tests", feature = "test_harness"))]
use chrono::Utc;
#[cfg(all(feature = "live_api_tests", feature = "test_harness"))]
use reqwest::Client;

/// Test utilities for common patterns in e2e tests
mod test_utils {
    use super::*;
    use ploke_tui::app_state::AppState;
    #[cfg(all(feature = "live_api_tests", feature = "test_harness"))]
    use ploke_tui::user_config::ModelConfig;
    
    /// Sets up a standard test environment with state and event bus
    pub async fn setup_test_environment() -> (Arc<AppState>, Arc<EventBus>) {
        let state = ploke_tui::test_harness::get_state().await;
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        (state, event_bus)
    }
    
    /// Creates ToolCallParams with standard configuration
    pub fn create_tool_call_params(
        state: Arc<AppState>,
        event_bus: Arc<EventBus>,
        request_id: Uuid,
        name: &str,
        arguments: serde_json::Value,
        call_id: String,
    ) -> ToolCallParams {
        ToolCallParams {
            state,
            event_bus,
            request_id,
            parent_id: Uuid::new_v4(),
            name: name.to_string(),
            arguments,
            call_id,
        }
    }
    
    /// Executes a tool call and waits for completion with timeout
    pub async fn execute_tool_call_and_wait(
        params: ToolCallParams,
        timeout_secs: u64,
    ) -> Result<String, String> {
        let request_id = params.request_id;
        let call_id = params.call_id.clone();
        let event_bus = Arc::clone(&params.event_bus);
        let mut rx = event_bus.realtime_tx.subscribe();
        
        // Spawn the tool call task
        tokio::spawn(async move {
            ploke_tui::rag::dispatcher::handle_tool_call_requested(params).await;
        });
        
        // Wait for completion with timeout
        let result = timeout(Duration::from_secs(timeout_secs), async move {
            loop {
                match rx.recv().await {
                    Ok(AppEvent::System(SystemEvent::ToolCallCompleted { 
                        request_id: rid, 
                        call_id: cid, 
                        content, 
                        .. 
                    })) if rid == request_id && cid == call_id => break Ok(content),
                    Ok(AppEvent::System(SystemEvent::ToolCallFailed { 
                        request_id: rid, 
                        call_id: cid, 
                        error, 
                        .. 
                    })) if rid == request_id && cid == call_id => break Err(error),
                    Ok(_) => continue,
                    Err(e) => break Err(format!("event error: {}", e)),
                }
            }
        }).await;
        
        match result {
            Ok(Ok(content)) => Ok(content),
            Ok(Err(error)) => Err(format!("tool call failed: {}", error)),
            Err(_) => Err(format!("timeout waiting for tool call completion after {} seconds", timeout_secs)),
        }
    }

    #[cfg(all(feature = "live_api_tests", feature = "test_harness"))]
    pub mod live_api {
        use std::str::FromStr;

        use ploke_tui::llm::providers::ProviderSlug;

        use super::*;
        
        /// Finds a tool-capable model endpoint from a list of candidates
        pub async fn find_tool_capable_endpoint(
            client: &Client,
            env: &ploke_tui::test_harness::OpenRouterEnv,
        ) -> Result<ModelConfig, String> {
            let candidates = [
                "openai/gpt-4o-mini",
                "openai/gpt-4o",
                "google/gemini-2.0-flash-001",
                "qwen/qwen-2.5-72b-instruct",
            ];
            
            let mut provider = ModelConfig {
                id: "live-openrouter".to_string(),
                api_key: env.key.clone(),
                provider_slug: None,
                api_key_env: None,
                base_url: env.url.to_string(),
                model: "openai/gpt-4o-mini".to_string(),
                display_name: None,
                provider_type: ploke_tui::user_config::ProviderType::OpenRouter,
                llm_params: None,
            };
            
            for model in candidates {
                if let Ok(endpoints) = ploke_tui::llm::openrouter::openrouter_catalog::fetch_model_endpoints(
                    client,
                    env.url.clone(),
                    &provider.api_key,
                    model,
                ).await {
                    if let Some(endpoint) = endpoints.into_iter().find(|e| {
                        e.supported_parameters
                            .as_ref()
                            .map(|sp| sp.iter().any(|s| s == "tools"))
                            .unwrap_or(false)
                    }) {
                        provider.model = model.to_string();
                        provider.provider_slug = Some(ProviderSlug::from_str( &endpoint.id ).expect("endpoint.id"));
                        return Ok(provider);
                    }
                }
            }
            
            Err("No tool-capable endpoint found among candidates".to_string())
        }
        
        /// Creates and persists test artifacts directory
        pub fn create_test_artifacts_dir() -> std::path::PathBuf {
            let ts = Utc::now().format("%Y%m%d-%H%M%S");
            let dir = ploke_test_utils::workspace_root()
                .join("crates/ploke-tui/ai_temp_data/live")
                .join(format!("openrouter-{}", ts));
            std::fs::create_dir_all(&dir).ok();
            dir
        }
        
        /// Makes a live OpenRouter API request and validates response
        pub async fn make_live_request_and_validate(
            client: &Client,
            provider: &ModelConfig,
            request: &ploke_tui::llm::openrouter::model_provider::CompReq<'_>,
            artifacts_dir: &std::path::Path,
        ) -> Result<serde_json::Value, String> {
            // Persist request
            std::fs::write(
                artifacts_dir.join("request.json"),
                serde_json::to_string_pretty(request).unwrap(),
            ).ok();
            
            // Send request
            let url = format!(
                "{}/chat/completions",
                provider.base_url.trim_end_matches('/')
            );
            let resp = client
                .post(url)
                .headers(ploke_tui::test_harness::default_headers())
                .header("Accept", "application/json")
                .bearer_auth(&provider.api_key)
                .json(request)
                .timeout(std::time::Duration::from_secs(ploke_tui::LLM_TIMEOUT_SECS))
                .send()
                .await
                .map_err(|e| format!("Failed to send request: {}", e))?;
            
            let status = resp.status();
            let headers = format!("{:?}", resp.headers());
            let body = resp.text().await.map_err(|e| format!("Failed to read response body: {}", e))?;
            
            if !status.is_success() {
                return Err(format!("Non-success status: {} body: {}", status, body));
            }
            
            // Parse and validate JSON
            if body.trim().is_empty() {
                std::fs::write(
                    artifacts_dir.join("response_raw.txt"),
                    format!("status={}\nheaders={}\n<body-empty>", status, headers),
                ).ok();
                return Err(format!("Empty response body: status={} headers={}", status, headers));
            }
            
            let json_value: serde_json::Value = serde_json::from_str(&body)
                .map_err(|e| {
                    std::fs::write(
                        artifacts_dir.join("response_raw.txt"),
                        format!("status={}\nheaders={}\n{}", status, headers, body),
                    ).ok();
                    format!("Invalid JSON in response: {}", e)
                })?;
            
            // Persist response
            std::fs::write(
                artifacts_dir.join("response.json"),
                serde_json::to_string_pretty(&json_value).unwrap(),
            ).ok();
            
            Ok(json_value)
        }
    }
}

#[tokio::test]
async fn e2e_get_file_metadata_and_apply_code_edit_splice() {
    // Setup test environment
    let (state, event_bus) = test_utils::setup_test_environment().await;

    // Create a temp file and write content
    let dir = tempfile::tempdir().expect("Failed to create temp directory");
    let file_path = dir.path().join("demo.rs");
    std::fs::write(&file_path, "fn demo() { let x = 1; }\n")
        .expect("Failed to write test file");

    // 1) Call get_file_metadata via dispatcher
    let request_id_meta = Uuid::new_v4();
    let call_id_meta = Uuid::new_v4().to_string();
    let args_meta = json!({"file_path": file_path.display().to_string()});
    let params_meta = test_utils::create_tool_call_params(
        Arc::clone(&state),
        Arc::clone(&event_bus),
        request_id_meta,
        "get_file_metadata",
        args_meta,
        call_id_meta.clone(),
    );

    let content_meta = test_utils::execute_tool_call_and_wait(params_meta, 5)
        .await
        .expect("get_file_metadata tool call failed");
    

    let meta: ploke_core::rag_types::GetFileMetadataResult = serde_json::from_str(&content_meta)
        .expect("Failed to deserialize get_file_metadata result");
    assert!(meta.ok && meta.exists, "File metadata indicates file is missing or operation failed");
    assert_eq!(meta.file_path, file_path.display().to_string(), "File path mismatch in metadata");
    let on_disk = std::fs::metadata(&file_path).unwrap().len();
    assert_eq!(meta.byte_len, on_disk, "File size mismatch between metadata and disk");

    // 2) Call apply_code_edit with a splice to rename `demo` -> `demo_ok`
    let initial = std::fs::read_to_string(&file_path)
        .expect("Failed to read initial file content");
    let start = initial.find("demo")
        .expect("Could not find 'demo' in file content");
    let end = start + "demo".len();

    // Compute tracking hash compatible with IoManager (token-based)
    let ast = syn::parse_file(&initial).expect("Failed to parse rust file");
    let tokens = quote::ToTokens::to_token_stream(&ast);
    let expected = ploke_core::TrackingHash::generate(
        ploke_core::PROJECT_NAMESPACE_UUID,
        &file_path,
        &tokens,
    )
    .0
    .to_string();

    let request_id_edit = Uuid::new_v4();
    let call_id_edit = Uuid::new_v4().to_string();
    let args_edit = json!({
        "edits": [{
            "file_path": file_path.display().to_string(),
            "expected_file_hash": expected,
            "start_byte": start as u64,
            "end_byte": end as u64,
            "replacement": "demo_ok"
        }]
    });
    let params_edit = test_utils::create_tool_call_params(
        Arc::clone(&state),
        Arc::clone(&event_bus),
        request_id_edit,
        "apply_code_edit",
        args_edit,
        call_id_edit.clone(),
    );

    let content_edit = test_utils::execute_tool_call_and_wait(params_edit, 5)
        .await
        .expect("apply_code_edit tool call failed");

    let result: ploke_core::rag_types::ApplyCodeEditResult = serde_json::from_str(&content_edit)
        .expect("Failed to deserialize apply_code_edit result");
    assert!(result.ok && result.staged == 1, "Code edit failed or didn't stage exactly 1 edit");
    // Preview mode depends on config; just check it is either diff or codeblock
    assert!(
        result.preview_mode == "diff" || result.preview_mode == "codeblock",
        "Unexpected preview mode: {}",
        result.preview_mode
    );

    // Approve edits to actually apply
    ploke_tui::rag::editing::approve_edits(&state, &event_bus, request_id_edit)
        .await;
    let updated = std::fs::read_to_string(&file_path)
        .expect("Failed to read updated file content");
    assert!(updated.contains("demo_ok"), "File should contain 'demo_ok' after edit, but got: {}", updated);
}

#[tokio::test]
async fn e2e_apply_code_edit_canonical_on_fixture() {
    // Setup test environment
    let (state, event_bus) = test_utils::setup_test_environment().await;

    // Set crate_focus to the fixture root so canonical resolution uses the same root as DB
    let crate_root = workspace_root().join("tests/fixture_crates/fixture_nodes");
    {
        use ploke_tui::app_state::core::SystemStatus;
        let mut sys = state.system.write().await;
        *sys = SystemStatus::new(Some(crate_root.clone()));
    }
    let rel_file = std::path::PathBuf::from("src/imports.rs");
    let abs_file = crate_root.join(&rel_file);
    let original = std::fs::read_to_string(&abs_file)
        .expect("Failed to read fixture file");

    // Stage canonical edit: replace function body with a tiny marker
    let request_id = Uuid::new_v4();
    let call_id = Uuid::new_v4().to_string();
    let args = json!({
        "edits": [{
            "mode": "canonical",
            "file": rel_file.display().to_string(),
            "canon": "crate::imports::use_imported_items",
            "node_type": "function",
            "code": "pub fn use_imported_items() { let _e2e_marker = 7; }"
        }]
    });
    let params = test_utils::create_tool_call_params(
        Arc::clone(&state),
        Arc::clone(&event_bus),
        request_id,
        "apply_code_edit",
        args,
        call_id.clone(),
    );

    let _content = test_utils::execute_tool_call_and_wait(params, 10)
        .await
        .expect("Canonical apply_code_edit tool call failed");

    // Apply it
    ploke_tui::rag::editing::approve_edits(&state, &event_bus, request_id).await;
    let updated = std::fs::read_to_string(&abs_file)
        .expect("Failed to read updated fixture file");
    assert!(
        updated.contains("_e2e_marker"),
        "File should contain '_e2e_marker' after canonical edit, but got: {}",
        updated
    );

    // Restore original file to avoid test side effects
    std::fs::write(&abs_file, original).expect("Failed to restore original fixture file");
}

/// Live tool-calling test: ensures OpenRouter returns JSON and includes tool_calls when tools are provided
#[cfg(all(feature = "live_api_tests", feature = "test_harness"))]
#[tokio::test]
async fn e2e_live_openrouter_tool_call_json() {
    let Some(env) = ploke_tui::test_harness::openrouter_env() else { return; };
    let client = Client::new();

    // Find a tool-capable endpoint
    let provider = test_utils::live_api::find_tool_capable_endpoint(&client, &env)
        .await
        .expect("Failed to find tool-capable endpoint");

    // Build a minimal tool-call request to trigger tool_calls
    let sys = ploke_tui::llm::RequestMessage { 
        role: ploke_tui::llm::Role::System, 
        content: "You can call tools.".to_string(), 
        tool_call_id: None 
    };
    let user = ploke_tui::llm::RequestMessage { 
        role: ploke_tui::llm::Role::User, 
        content: "Call the tool to find SimpleStruct; do not answer directly.".to_string(), 
        tool_call_id: None 
    };
    let messages = vec![sys, user];
    let tools: Vec<ToolDefinition> = vec![RequestCodeContextGat::tool_def()];
    let params = ploke_tui::llm::LLMParameters { 
        max_tokens: Some(4096), 
        temperature: Some(0.0), 
        ..Default::default() 
    };
    
    // Pass require_parameters=false to avoid over-restricting routing
    let mut request = ploke_tui::llm::session::build_openai_request(
        &provider, 
        messages, 
        &params, 
        Some(tools), 
        true, 
        false
    );
    
    // Force a function tool call for validation
    use ploke_tui::llm::openrouter::model_provider::{ToolChoice, ToolChoiceFunction};
    request.tool_choice = Some(ToolChoice::Function { 
        r#type: FunctionMarker, 
        function: ToolChoiceFunction { 
            name: "request_code_context".to_string() 
        } 
    });

    // Create test artifacts directory
    let artifacts_dir = test_utils::live_api::create_test_artifacts_dir();

    // Make live request and validate response
    let response_json = test_utils::live_api::make_live_request_and_validate(
        &client,
        &provider,
        &request,
        &artifacts_dir,
    )
    .await
    .expect("Live API request failed");

    // Validate tool_calls are present in response
    let choices = response_json
        .get("choices")
        .and_then(|c| c.as_array())
        .expect("Response should contain 'choices' array");
    
    let mut saw_tool_calls = false;
    for choice in choices {
        if let Some(message) = choice.get("message") {
            if message.get("tool_calls").is_some() { 
                saw_tool_calls = true; 
                break; 
            }
        }
    }
    
    assert!(
        saw_tool_calls, 
        "Expected tool_calls in response; got: {}", 
        serde_json::to_string(&response_json).unwrap()
    );
}

/// Enhanced end-to-end test with comprehensive event tracking and validation
#[tokio::test]
async fn e2e_enhanced_tool_call_lifecycle_validation() {
    let (state, event_bus) = test_utils::setup_test_environment().await;
    
    let request_id = Uuid::new_v4();
    let mut events_collected = Vec::new();
    let mut event_rx = event_bus.realtime_tx.subscribe();

    // Create a realistic test file
    let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let test_file = temp_dir.path().join("enhanced_demo.rs");
    std::fs::write(&test_file, "fn enhanced_demo() { let x = 42; }\n")
        .expect("Failed to write test file");

    // Execute get_file_metadata with enhanced tracking
    let call_id_meta = Uuid::new_v4().to_string();
    let args_meta = json!({"file_path": test_file.display().to_string()});
    let params_meta = test_utils::create_tool_call_params(
        Arc::clone(&state),
        Arc::clone(&event_bus),
        request_id,
        "get_file_metadata",
        args_meta,
        call_id_meta.clone(),
    );

    // Start tool execution and event tracking
    let tool_task = tokio::spawn(async move {
        ploke_tui::rag::dispatcher::handle_tool_call_requested(params_meta).await;
    });

    let tracking_task = tokio::spawn(async move {
        let mut event_count = 0;
        let timeout_duration = Duration::from_secs(10);
        let mut tool_completed = false;
        
        let result = timeout(timeout_duration, async {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        events_collected.push(event.clone());
                        event_count += 1;
                        
                        // Check for tool completion
                        if let AppEvent::System(SystemEvent::ToolCallCompleted { .. }) = &event {
                            tool_completed = true;
                            break;
                        }
                        
                        if event_count > 20 {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }).await;
        
        (result, events_collected, tool_completed)
    });

    // Wait for completion
    let _ = tool_task.await;
    let (tracking_result, final_events, tool_completed) = tracking_task.await
        .expect("Event tracking task failed");
    
    assert!(tracking_result.is_ok(), "Event tracking timed out");
    assert!(tool_completed, "Tool execution did not complete");

    // Enhanced validation of events
    assert!(!final_events.is_empty(), "No events were collected");
    
    let tool_requested_events = final_events.iter()
        .filter(|e| matches!(e, AppEvent::LlmTool(ploke_tui::llm::ToolEvent::Requested { .. })))
        .count();
    
    let tool_completed_events = final_events.iter()
        .filter(|e| matches!(e, AppEvent::System(SystemEvent::ToolCallCompleted { .. })))
        .count();

    println!("Enhanced test completed:");
    println!("  Events collected: {}", final_events.len());
    println!("  Tool requested events: {}", tool_requested_events);
    println!("  Tool completed events: {}", tool_completed_events);

    // Validate event sequence
    assert!(tool_completed_events > 0, "No tool completion events found");
    
    // Find the tool completion event and validate its content
    for event in &final_events {
        if let AppEvent::System(SystemEvent::ToolCallCompleted { content, .. }) = event {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(content);
            assert!(parsed.is_ok(), "Tool result is not valid JSON: {}", content);
            
            let json_result = parsed.unwrap();
            assert!(json_result.get("ok").is_some(), "Tool result missing 'ok' field");
            
            println!("✓ Tool completed successfully with valid JSON result");
            break;
        }
    }
}

/// Tool role validation test - ensure Role::Tool works correctly
#[tokio::test]
async fn e2e_tool_role_validation() {
    // Test that our new Role::Tool functionality works correctly
    let tool_msg = ploke_tui::llm::RequestMessage::new_tool(
        json!({"ok": true, "result": "test"}).to_string(),
        "call_123".to_string()
    );
    
    // Validate the message
    assert!(tool_msg.validate().is_ok());
    assert_eq!(tool_msg.role, ploke_tui::llm::Role::Tool);
    assert_eq!(tool_msg.tool_call_id, Some("call_123".to_string()));
    
    // Test serialization
    let serialized = serde_json::to_string(&tool_msg).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(parsed["role"], "tool");
    assert_eq!(parsed["tool_call_id"], "call_123");
    
    println!("✓ Tool role validation test passed");
}

/// Simplified live API test to validate that tool calls work with OpenRouter
#[cfg(all(feature = "live_api_tests", feature = "test_harness"))]
#[tokio::test] 
async fn e2e_live_openrouter_tool_call_simple() {
    let Some(env) = ploke_tui::test_harness::openrouter_env() else { 
        println!("Skipping test: OPENROUTER_API_KEY not set");
        return; 
    };
    
    let client = Client::new();
    let artifacts_dir = test_utils::live_api::create_test_artifacts_dir();
    
    // Find tool-capable endpoint
    let provider = test_utils::live_api::find_tool_capable_endpoint(&client, &env)
        .await
        .expect("Failed to find tool-capable endpoint");

    // Create a simple request that should trigger tool calls
    let sys = ploke_tui::llm::RequestMessage::new_system(
        "You are a helpful assistant with access to code analysis tools.".to_string()
    );
    let user = ploke_tui::llm::RequestMessage::new_user(
        "Call the request_code_context tool to find SimpleStruct examples.".to_string()
    );
    let messages = vec![sys, user];
    let tools: Vec<ToolDefinition> = vec![RequestCodeContextGat::tool_def()];
    let params = ploke_tui::llm::LLMParameters { 
        max_tokens: Some(1024), 
        temperature: Some(0.0), 
        ..Default::default() 
    };
    
    let mut request = ploke_tui::llm::session::build_openai_request(
        &provider, 
        messages, 
        &params, 
        Some(tools), 
        true, 
        false
    );
    
    // Force tool use
    use ploke_tui::llm::openrouter::model_provider::{ToolChoice, ToolChoiceFunction};
    request.tool_choice = Some(ToolChoice::Function { 
        r#type: FunctionMarker, 
        function: ToolChoiceFunction { 
            name: "request_code_context".to_string() 
        } 
    });

    // Make the request and validate response contains tool calls
    let response = test_utils::live_api::make_live_request_and_validate(
        &client,
        &provider,
        &request,
        &artifacts_dir,
    )
    .await
    .expect("Live API request failed");

    // Validate tool calls are present
    let tool_calls = response
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|msg| msg.get("tool_calls"))
        .and_then(|tc| tc.as_array());

    assert!(tool_calls.is_some(), "No tool calls in response");
    assert!(!tool_calls.unwrap().is_empty(), "Empty tool calls array");

    println!("✓ Live OpenRouter tool call test passed");
    println!("  Tool calls found: {}", tool_calls.unwrap().len());
    println!("  Artifacts saved to: {:?}", artifacts_dir);
}
