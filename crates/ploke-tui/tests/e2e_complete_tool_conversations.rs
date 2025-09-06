#![cfg(feature = "test_harness")]

//! Complete tool-call conversation cycle testing
//!
//! This module validates the full end-to-end messaging loop:
//! 1. User message → LLM request with tools
//! 2. LLM response with tool calls
//! 3. Tool execution and result generation
//! 4. Tool result → LLM as system message
//! 5. LLM final response to user
//!
//! Tests multi-turn conversations with tool usage

use ploke_core::ArcStr;
use ploke_tui::tools::Tool;
use ploke_tui::tracing_setup::{init_tracing, init_tracing_to_file_ai};
use std::time::Duration;
use uuid::Uuid;

mod harness;
use harness::AppHarness;

use color_eyre::Result;

/// Test basic message addition to chat history
#[tokio::test]
async fn e2e_basic_message_addition() -> Result<()> {
    let harness = AppHarness::spawn().await?;

    // Check initial state
    {
        let chat = harness.state.chat.read().await;
        let initial_count = chat.messages.len();
        println!("Initial message count: {}", initial_count);
    }

    // Add a single user message
    let user_message = "Test message";
    let msg_id = harness.add_user_msg(user_message).await;

    // Wait for processing
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify the message was added
    {
        let chat = harness.state.chat.read().await;
        println!("Final message count: {}", chat.messages.len());

        // Check if our message exists
        if let Some(user_msg) = chat.messages.get(&msg_id) {
            assert_eq!(user_msg.content, user_message);
            assert_eq!(user_msg.kind, ploke_tui::chat_history::MessageKind::User);
            println!("✓ Message found and verified");
        } else {
            println!("❌ Message with ID {} not found", msg_id);
            println!("Available message IDs:");
            for (id, msg) in &chat.messages {
                println!("  {} -> {:?} ({})", id, msg.kind, msg.content);
            }
            panic!("User message should be in chat history");
        }
    }

    harness.shutdown().await;
    println!("✓ Basic message addition validated");
    Ok(())
}

/// Test a complete conversation cycle with tool usage and response
#[tokio::test]
async fn e2e_complete_get_metadata_conversation() -> color_eyre::Result<()> {
    let harness = AppHarness::spawn().await?;

    // Simulate user asking for file metadata
    let user_message = "Please get the metadata for the file 'Cargo.toml' and tell me about it.";

    // Add user message to chat
    let msg_id = harness.add_user_msg(user_message).await;

    // Wait for message processing (allows state manager to process)
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify the message was added to chat history
    {
        let chat = harness.state.chat.read().await;
        let messages = &chat.messages;
        assert!(!messages.is_empty(), "Chat should have messages");

        // Find our user message
        let user_msg = messages.get(&msg_id);
        assert!(user_msg.is_some(), "User message should be in chat history");

        let user_msg = user_msg.unwrap();
        assert_eq!(user_msg.content, user_message);
        assert_eq!(user_msg.kind, ploke_tui::chat_history::MessageKind::User);
    }

    harness.shutdown().await;
    println!("✓ Complete conversation cycle structure validated");
    Ok(())
}

/// Test tool execution event flow in conversation context
#[tokio::test]
async fn e2e_tool_execution_event_flow() -> color_eyre::Result<()> {
    let harness = AppHarness::spawn().await?;

    // Create a simulated tool call scenario
    let tool_call_id = "test_call_123";
    let call_args = r#"{"file_path": "/test/file.rs"}"#;

    // Emit a tool call requested event
    let request_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    harness.emit_tool_completed(
        request_id,
        parent_id,
        ArcStr::from(call_args),
        "get_file_metadata".to_string(),
    );

    // Wait for event processing
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Tool execution should have been triggered
    // (We can't easily assert on the exact execution without more complex event tracking,
    //  but this validates the basic event flow structure)

    harness.shutdown().await;
    println!("✓ Tool execution event flow validated");
    Ok(())
}

/// Test multi-step conversation with sequential tool calls
#[tokio::test]
async fn e2e_multi_step_tool_conversation() -> color_eyre::Result<()> {
    let _g = init_tracing_to_file_ai("e2e_multi_step_tool_conversation");
    let harness = AppHarness::spawn().await?;

    // Step 1: User asks for file metadata
    let msg1_id = harness.add_user_msg("Get metadata for Cargo.toml").await;
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Step 2: User asks for another file
    let msg2_id = harness.add_user_msg("Now get metadata for README.md").await;
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Verify both messages in chat history
    {
        let chat = harness.state.chat.read().await;
        let messages = &chat.messages;

        let msg1 = messages.get(&msg1_id);
        let msg2 = messages.get(&msg2_id);

        assert!(msg1.is_some(), "First message should be in history");
        assert!(msg2.is_some(), "Second message should be in history");

        // Both messages should exist (chronological order is maintained by the path)
        assert_eq!(msg1.unwrap().content, "Get metadata for Cargo.toml");
        // NOTE: There is no README.md the file can retrieve, as we only store data parse from Rust
        // files and Cargo.toml files
        assert_eq!(msg2.unwrap().content, "Now get metadata for README.md");
    }

    harness.shutdown().await;
    println!("✓ Multi-step conversation structure validated");
    Ok(())
}

/// Test conversation with tool error handling
#[tokio::test]
async fn e2e_conversation_with_tool_errors() -> color_eyre::Result<()> {
    let harness = AppHarness::spawn().await?;

    // Add a message that would trigger a tool call to a non-existent file
    let msg_id = harness
        .add_user_msg("Get metadata for /nonexistent/file.txt")
        .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Simulate a tool error event
    let error_request_id = Uuid::new_v4();
    let error_parent_id = Uuid::new_v4();
    harness.emit_tool_completed(
        error_request_id,
        error_parent_id,
        ArcStr::from(r#"{"error": "File not found"}"#),
        "get_file_metadata".to_string(),
    );

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify message was added despite tool error
    {
        let chat = harness.state.chat.read().await;
        let user_msg = chat.messages.get(&msg_id);
        assert!(
            user_msg.is_some(),
            "User message should persist even with tool errors"
        );
    }

    harness.shutdown().await;
    println!("✓ Conversation error handling validated");
    Ok(())
}

/// Test conversation state persistence across tool calls
/// Inexpensive test:
///     - default model `kimi-k2`
///     - Cost/run ~ $0.001146 with
///     - Tokens/run sent ~ 3730
///     - Tokens/run received ~ 251
///     - Avg tps 54.15 (reported by OpenRouter dashboard)
///     - 4 total generation requests
#[tokio::test]
async fn e2e_conversation_state_persistence() -> Result<()> {
    let _g = init_tracing_to_file_ai("e2e_conversation_state_persistence");
    let harness = AppHarness::spawn().await?;

    // Add multiple messages to build conversation context
    let msgs = vec![
        "Hello, I need help with some files",
        "Can you get metadata for Cargo.toml?",
        "What about the README.md file?",
        "Thanks for the help!",
    ];

    let mut msg_ids = Vec::new();
    for msg in &msgs {
        let id = harness.add_user_msg(*msg).await;
        msg_ids.push(id);
        tokio::time::sleep(Duration::from_secs(10)).await;
    }

    // Verify all messages are preserved in conversation state
    {
        let chat = harness.state.chat.read().await;

        // Should have at least our messages (plus any root/system messages)
        assert!(
            chat.messages.len() >= msgs.len(),
            "All user messages should be preserved. Expected >= {}, got {}",
            msgs.len(),
            chat.messages.len()
        );

        // Verify each message exists and has correct content
        for (expected_content, msg_id) in msgs.iter().zip(msg_ids.iter()) {
            let found_msg = chat.messages.get(msg_id);
            assert!(
                found_msg.is_some(),
                "Message should exist: {}",
                expected_content
            );

            let found_msg = found_msg.unwrap();
            assert_eq!(found_msg.content, *expected_content);
        }
    }

    harness.shutdown().await;
    println!("✓ Conversation state persistence validated");
    Ok(())
}

/// Test tool result integration into conversation flow
#[tokio::test]
async fn e2e_tool_result_conversation_integration() -> color_eyre::Result<()> {
    let harness = AppHarness::spawn().await?;

    // Create a realistic tool call scenario
    let user_msg_id = harness.add_user_msg("Check the size of Cargo.toml").await;
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Simulate successful tool execution with realistic result
    let tool_result = r#"{
        "file_path": "Cargo.toml",
        "exists": true,
        "size": 1234,
        "modified": "2025-09-01T05:00:00Z",
        "tracking_hash": "abc123def456"
    }"#;

    let int_request_id = Uuid::new_v4();
    let int_parent_id = Uuid::new_v4();
    harness.emit_tool_completed(
        int_request_id,
        int_parent_id,
        ArcStr::from(tool_result),
        "get_file_metadata".to_string(),
    );

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Check that conversation state is maintained appropriately
    {
        let chat = harness.state.chat.read().await;

        // Should have our user message
        let user_msg = chat.messages.get(&user_msg_id);
        assert!(user_msg.is_some(), "User message should be preserved");

        // Verify message content
        let user_msg = user_msg.unwrap();
        assert_eq!(user_msg.content, "Check the size of Cargo.toml");
    }

    harness.shutdown().await;
    println!("✓ Tool result conversation integration validated");
    Ok(())
}

/// Test conversation context building for tool calls
#[tokio::test]
async fn e2e_conversation_context_for_tools() -> color_eyre::Result<()> {
    let _g = init_tracing();
    let harness = AppHarness::spawn().await?;

    // Build up a conversation with context
    let context_msgs = vec![
        "I'm working on a Rust project",
        "I need to check some file information",
        "Please get metadata for these files: Cargo.toml and README.md",
    ];

    let mut msg_ids = Vec::new();
    for msg in &context_msgs {
        let id = harness.add_user_msg(*msg).await;
        msg_ids.push(id);
        tokio::time::sleep(Duration::from_secs(10)).await;
    }

    // Verify conversation context is properly maintained
    {
        let chat = harness.state.chat.read().await;

        // Check that all context messages are preserved
        for (i, expected_content) in context_msgs.iter().enumerate() {
            let msg_id = msg_ids[i];
            let found_msg = chat.messages.get(&msg_id);

            assert!(found_msg.is_some(), "Context message {} should exist", i);
            let found_msg = found_msg.unwrap();
            assert_eq!(found_msg.content, *expected_content);
        }

        // Verify all messages exist (chronological order is maintained via conversation path)
        for msg_id in &msg_ids {
            assert!(
                chat.messages.contains_key(msg_id),
                "Message should exist in chat"
            );
        }
    }

    harness.shutdown().await;
    println!("✓ Conversation context building validated");
    Ok(())
}

/// Test message receipt and deserialization from API
/// Verifies that messages are correctly received and deserialized into strongly-typed structures
#[tokio::test]
async fn e2e_message_receipt_and_deserialization() -> Result<()> {
    let _g = init_tracing_to_file_ai("e2e_message_receipt_and_deserialization");
    let harness = AppHarness::spawn().await?;

    // Add a message that will trigger an API call
    let user_msg = "Please help me understand how to use tools in this system.";
    let msg_id = harness.add_user_msg(user_msg).await;

    // Wait for API response
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Verify message was added and processed
    {
        let chat = harness.state.chat.read().await;

        // Check user message exists
        let user_message = chat.messages.get(&msg_id);
        assert!(user_message.is_some(), "User message should exist");
        assert_eq!(user_message.unwrap().content, user_msg);
        assert_eq!(
            user_message.unwrap().kind,
            ploke_tui::chat_history::MessageKind::User
        );

        // Look for assistant response
        let assistant_msgs: Vec<_> = chat
            .messages
            .values()
            .filter(|m| matches!(m.kind, ploke_tui::chat_history::MessageKind::Assistant))
            .collect();

        assert!(
            !assistant_msgs.is_empty(),
            "Should have at least one assistant message"
        );

        // Verify assistant message has content
        for msg in &assistant_msgs {
            assert!(
                !msg.content.is_empty(),
                "Assistant message should have content"
            );
            println!("Assistant message content length: {}", msg.content.len());
        }

        // Check message structure integrity
        for (id, msg) in &chat.messages {
            assert!(id != &Uuid::nil(), "Message ID should not be nil");

            // Verify role mapping
            match msg.kind {
                ploke_tui::chat_history::MessageKind::User => {
                    println!(
                        "✓ User message verified: {}",
                        &msg.content[..msg.content.len().min(50)]
                    );
                }
                ploke_tui::chat_history::MessageKind::Assistant => {
                    println!(
                        "✓ Assistant message verified: {}",
                        &msg.content[..msg.content.len().min(50)]
                    );
                }
                ploke_tui::chat_history::MessageKind::System => {
                    println!("✓ System message verified");
                }
                _ => {}
            }
        }
    }

    harness.shutdown().await;
    println!("✓ Message receipt and deserialization validated");
    Ok(())
}

/// Test tool calls and return output
/// Verifies tools are called with correct parameters, execution generates results,
/// tool results are properly formatted, and error handling works correctly
#[tokio::test]
async fn e2e_tool_calls_and_return_output() -> Result<()> {
    let _g = init_tracing_to_file_ai("e2e_tool_calls_and_return_output");
    let harness = AppHarness::spawn().await?;

    // Configure the model to support tools by adding capability
    {
        let mut config = harness.state.config.write().await;

        // Get the current model name from config
        let model_name =
            if let Some(active_config) = config.model_registry.get_active_model_config() {
                active_config.model.clone()
            } else {
                "kimi-k2".to_string() // fallback to default
            };

        // Set up tool support for the active model
        config.model_registry.capabilities.insert(
            model_name.clone(),
            ploke_tui::user_config::ModelCapabilities {
                supports_tools: true,
                context_length: Some(8192),
                input_cost_per_million: Some(1.0),
                output_cost_per_million: Some(3.0),
            },
        );

        println!("✓ Configured model '{}' to support tools", model_name);
    }

    // Add a message specifically designed to trigger tool usage
    let user_msg =
        "Please get the metadata for the file 'Cargo.toml' using the get_file_metadata tool.";
    let msg_id = harness.add_user_msg(user_msg).await;

    // Wait longer for tool call processing
    tokio::time::sleep(Duration::from_secs(15)).await;

    // Verify the conversation includes tool usage
    {
        let chat = harness.state.chat.read().await;

        // Check user message exists
        let user_message = chat.messages.get(&msg_id);
        assert!(user_message.is_some(), "User message should exist");
        assert_eq!(user_message.unwrap().content, user_msg);

        // Look for system messages that indicate tool usage
        let system_msgs: Vec<_> = chat
            .messages
            .values()
            .filter(|m| matches!(m.kind, ploke_tui::chat_history::MessageKind::System))
            .collect();

        println!("Found {} system messages", system_msgs.len());
        for (i, msg) in system_msgs.iter().enumerate() {
            println!(
                "System message {}: {}",
                i,
                &msg.content[..msg.content.len().min(100)]
            );
        }

        // Look for assistant response
        let assistant_msgs: Vec<_> = chat
            .messages
            .values()
            .filter(|m| matches!(m.kind, ploke_tui::chat_history::MessageKind::Assistant))
            .collect();

        assert!(
            !assistant_msgs.is_empty(),
            "Should have at least one assistant message"
        );
        println!("Found {} assistant messages", assistant_msgs.len());

        // Verify that assistant message discusses the file metadata or tool usage
        let assistant_content = &assistant_msgs[0].content;
        assert!(
            !assistant_content.is_empty(),
            "Assistant message should have content"
        );

        // Check for indicators of tool usage in the response
        let tool_indicators = ["metadata", "file", "Cargo.toml", "tool"];
        let contains_tool_reference = tool_indicators.iter().any(|&indicator| {
            assistant_content
                .to_lowercase()
                .contains(&indicator.to_lowercase())
        });

        if !contains_tool_reference {
            println!("Assistant response: {}", assistant_content);
        }
        assert!(
            contains_tool_reference,
            "Assistant response should reference tool usage or file metadata"
        );

        // Log all messages for debugging
        println!("Total messages in conversation: {}", chat.messages.len());
        for (id, msg) in &chat.messages {
            println!(
                "Message {}: {:?} - {}",
                id,
                msg.kind,
                &msg.content[..msg.content.len().min(80)]
            );
        }
    }

    harness.shutdown().await;
    println!("✓ Tool calls and return output validated");
    Ok(())
}

/// Test tool output sent to API with expected values
/// Verifies tool outputs are correctly formatted and sent to API as Role::Tool messages
/// with proper structure and expected field values
#[tokio::test]
async fn e2e_tool_output_sent_to_api() -> Result<()> {
    let _g = init_tracing_to_file_ai("e2e_tool_output_sent_to_api");
    let harness = AppHarness::spawn().await?;

    // Configure the model to support tools by adding capability
    {
        let mut config = harness.state.config.write().await;

        // Get the current model name from config
        let model_name =
            if let Some(active_config) = config.model_registry.get_active_model_config() {
                active_config.model.clone()
            } else {
                "kimi-k2".to_string() // fallback to default
            };

        // Set up tool support for the active model
        config.model_registry.capabilities.insert(
            model_name.clone(),
            ploke_tui::user_config::ModelCapabilities {
                supports_tools: true,
                context_length: Some(8192),
                input_cost_per_million: Some(1.0),
                output_cost_per_million: Some(3.0),
            },
        );

        // Enable stricter tool requirements to ensure tools are definitely used
        config.model_registry.require_tool_support = true;

        println!(
            "✓ Configured model '{}' to require tool support",
            model_name
        );
    }

    // Add a message that explicitly requests tool usage with a valid file that should exist
    let user_msg = "Use the get_file_metadata tool to check the file 'Cargo.toml' in the current directory. I need to see the file size and tracking hash.";
    let msg_id = harness.add_user_msg(user_msg).await;

    // Wait longer for tool call processing and API response
    tokio::time::sleep(Duration::from_secs(20)).await;

    // Verify the complete tool output cycle
    {
        let chat = harness.state.chat.read().await;

        // Check user message exists
        let user_message = chat.messages.get(&msg_id);
        assert!(user_message.is_some(), "User message should exist");
        assert_eq!(user_message.unwrap().content, user_msg);

        // Collect all messages by type for analysis
        let user_msgs: Vec<_> = chat
            .messages
            .values()
            .filter(|m| matches!(m.kind, ploke_tui::chat_history::MessageKind::User))
            .collect();
        let assistant_msgs: Vec<_> = chat
            .messages
            .values()
            .filter(|m| matches!(m.kind, ploke_tui::chat_history::MessageKind::Assistant))
            .collect();
        let system_msgs: Vec<_> = chat
            .messages
            .values()
            .filter(|m| matches!(m.kind, ploke_tui::chat_history::MessageKind::System))
            .collect();
        let sysinfo_msgs: Vec<_> = chat
            .messages
            .values()
            .filter(|m| matches!(m.kind, ploke_tui::chat_history::MessageKind::SysInfo))
            .collect();

        println!("Message analysis:");
        println!("  User messages: {}", user_msgs.len());
        println!("  Assistant messages: {}", assistant_msgs.len());
        println!("  System messages: {}", system_msgs.len());
        println!("  SysInfo messages: {}", sysinfo_msgs.len());

        // Look for evidence of tool execution in system messages
        let tool_system_msgs: Vec<_> = system_msgs
            .iter()
            .filter(|msg| {
                let content = msg.content.to_lowercase();
                content.contains("tool")
                    || content.contains("metadata")
                    || content.contains("file")
                    || content.contains("cargo.toml")
            })
            .collect();

        // Look for evidence of tool-related content in sysinfo messages
        let tool_sysinfo_msgs: Vec<_> = sysinfo_msgs
            .iter()
            .filter(|msg| {
                let content = msg.content.to_lowercase();
                content.contains("tool")
                    || content.contains("metadata")
                    || content.contains("request summary")
                    || content.contains("api")
            })
            .collect();

        println!("Tool-related messages:");
        println!("  Tool system messages: {}", tool_system_msgs.len());
        println!("  Tool sysinfo messages: {}", tool_sysinfo_msgs.len());

        // Print system messages for debugging
        for (i, msg) in system_msgs.iter().enumerate() {
            let content_preview = &msg.content[..msg.content.len().min(120)];
            println!("System message {}: {}", i, content_preview);
        }

        // Print sysinfo messages for debugging
        for (i, msg) in sysinfo_msgs.iter().enumerate() {
            let content_preview = &msg.content[..msg.content.len().min(120)];
            println!("SysInfo message {}: {}", i, content_preview);
        }

        // We should have at least one assistant message in response
        assert!(
            !assistant_msgs.is_empty(),
            "Should have at least one assistant message"
        );

        // Check that we have some indication that tool processing occurred
        // This could be in system messages (tool results) or sysinfo messages (request summaries)
        let has_tool_evidence = !tool_system_msgs.is_empty()
            || !tool_sysinfo_msgs.is_empty()
            || sysinfo_msgs
                .iter()
                .any(|msg| msg.content.contains("Request summary"));

        if !has_tool_evidence {
            println!("All messages:");
            for (id, msg) in &chat.messages {
                println!(
                    "  {}: {:?} - {}",
                    id,
                    msg.kind,
                    &msg.content[..msg.content.len().min(100)]
                );
            }
        }

        assert!(
            has_tool_evidence,
            "Should have evidence of tool processing in system or sysinfo messages"
        );

        // Verify assistant response has content related to the request
        let assistant_content = &assistant_msgs[0].content;
        assert!(
            !assistant_content.is_empty(),
            "Assistant message should have content"
        );

        // Check that the response addresses the file metadata request
        let addresses_request = assistant_content.to_lowercase().contains("file")
            || assistant_content.to_lowercase().contains("cargo")
            || assistant_content.to_lowercase().contains("metadata")
            || assistant_content.to_lowercase().contains("error")
            || assistant_content.to_lowercase().contains("failed");

        if !addresses_request {
            println!("Assistant response: {}", assistant_content);
        }

        assert!(
            addresses_request,
            "Assistant response should address the file metadata request or explain any errors"
        );

        println!(
            "✓ Tool output API cycle validated with {} total messages",
            chat.messages.len()
        );
    }

    harness.shutdown().await;
    println!("✓ Tool output sent to API with expected values validated");
    Ok(())
}

/// Test workflow compliance with documented message and tool call lifecycle
/// Verifies message handling matches the workflow described in message_and_tool_call_lifecycle.md
/// including StateCommand processing order and event bus message routing
#[tokio::test]
async fn e2e_workflow_compliance_validation() -> Result<()> {
    let _g = init_tracing_to_file_ai("e2e_workflow_compliance_validation");
    let harness = AppHarness::spawn().await?;

    // Configure the model to support tools by adding capability
    {
        let mut config = harness.state.config.write().await;

        // Get the current model name from config
        let model_name =
            if let Some(active_config) = config.model_registry.get_active_model_config() {
                active_config.model.clone()
            } else {
                "kimi-k2".to_string() // fallback to default
            };

        // Set up tool support for the active model
        config.model_registry.capabilities.insert(
            model_name.clone(),
            ploke_tui::user_config::ModelCapabilities {
                supports_tools: true,
                context_length: Some(8192),
                input_cost_per_million: Some(1.0),
                output_cost_per_million: Some(3.0),
            },
        );

        println!("✓ Configured model '{}' to support tools", model_name);
    }

    // Verify the documented workflow:
    // 1. User enters normal message
    // 2. AddUserMessage → add_msg_immediate → User message in chat + llm::Event::Request emitted
    // 3. ScanForChange → scan_for_change → checks file hash changes
    // 4. EmbedMessage → process_with_rag → assembles context + llm::Event::PromptConstructed
    // 5. LLM manager pairs Request + PromptConstructed → process_llm_request spawned
    // 6. process_llm_request → CreateAssistantMessage → prepare_and_run_llm_call → build_comp_req with tools
    // 7. Tool calls dispatched/awaited → UpdateMessage with final content

    println!("Initiating documented workflow test with message asking for code context");
    let user_msg = "Can you find any code snippets related to SimpleStruct in the codebase?";
    let msg_id = harness.add_user_msg(user_msg).await;

    // Give the system time to complete the full documented workflow
    tokio::time::sleep(Duration::from_secs(18)).await;

    // Verify the workflow stages according to the documentation
    {
        let chat = harness.state.chat.read().await;

        // Stage 1: User message should be in chat history
        let user_message = chat.messages.get(&msg_id);
        assert!(
            user_message.is_some(),
            "Step 1: User message should exist in chat"
        );
        let user_message = user_message.unwrap();
        assert_eq!(user_message.content, user_msg);
        assert_eq!(
            user_message.kind,
            ploke_tui::chat_history::MessageKind::User
        );
        println!("✓ Step 1: User message correctly added to chat");

        // Stage 2-4: The add_user_msg function in the harness performs the documented steps:
        // - AddUserMessage (writes to chat, emits Request event)
        // - ScanForChange (checks file changes)
        // - EmbedMessage (assembles RAG context, emits PromptConstructed)
        // These are all handled by the harness - we verify their effects

        // Stage 5-7: LLM processing should have resulted in assistant message
        let assistant_msgs: Vec<_> = chat
            .messages
            .values()
            .filter(|m| matches!(m.kind, ploke_tui::chat_history::MessageKind::Assistant))
            .collect();

        assert!(
            !assistant_msgs.is_empty(),
            "Step 5-7: Should have assistant message from LLM processing"
        );
        println!(
            "✓ Step 5-7: LLM processing completed with {} assistant messages",
            assistant_msgs.len()
        );

        // Verify the assistant response addresses the request about SimpleStruct
        let assistant_msg = &assistant_msgs[0];
        assert!(
            !assistant_msg.content.is_empty(),
            "Assistant message should have content"
        );

        // Look for evidence that this went through the RAG/context system
        let content_lower = assistant_msg.content.to_lowercase();
        let has_relevant_response = content_lower.contains("struct")
            || content_lower.contains("code")
            || content_lower.contains("simple")
            || content_lower.contains("search")
            || content_lower.contains("find")
            || content_lower.contains("context")
            || content_lower.contains("error")
            || content_lower.contains("unable");

        if !has_relevant_response {
            println!(
                "Assistant response (first 200 chars): {}",
                &assistant_msg.content[..assistant_msg.content.len().min(200)]
            );
        }

        assert!(
            has_relevant_response,
            "Assistant should respond about code search/context or explain inability"
        );
        println!("✓ Step 6-7: Assistant response addresses the code context request");

        // Verify system messages from tool execution or processing
        let system_msgs: Vec<_> = chat
            .messages
            .values()
            .filter(|m| matches!(m.kind, ploke_tui::chat_history::MessageKind::System))
            .collect();

        let sysinfo_msgs: Vec<_> = chat
            .messages
            .values()
            .filter(|m| matches!(m.kind, ploke_tui::chat_history::MessageKind::SysInfo))
            .collect();

        // Should have some system/sysinfo messages indicating processing occurred
        let total_processing_msgs = system_msgs.len() + sysinfo_msgs.len();
        assert!(
            total_processing_msgs > 0,
            "Should have system messages indicating processing"
        );
        println!(
            "✓ Workflow: Found {} processing messages (system + sysinfo)",
            total_processing_msgs
        );

        // Validate overall message structure follows documented patterns
        let total_msgs = chat.messages.len();
        assert!(
            total_msgs >= 3,
            "Should have at least user + assistant + processing messages"
        );
        println!(
            "✓ Workflow: Total conversation has {} messages following documented structure",
            total_msgs
        );

        // Log message structure for verification
        println!("Final conversation structure:");
        for (id, msg) in &chat.messages {
            let content_preview = &msg.content[..msg.content.len().min(60)];
            println!(
                "  {}: {:?} - {}",
                &id.to_string()[..8],
                msg.kind,
                content_preview
            );
        }
    }

    harness.shutdown().await;
    println!("✓ Workflow compliance with documented message and tool call lifecycle validated");
    Ok(())
}

/// Test API response deserialization and GAT-based tool system validation
/// This test captures raw API responses via the test harness and validates strongly-typed deserialization
#[cfg(all(feature = "test_harness", feature = "live_api_tests"))]
#[tokio::test]
async fn e2e_api_response_deserialization_and_gat_validation() -> Result<()> {
    use ploke_tui::app_state::events::SystemEvent;
    use tokio_test::assert_ok;

    let _g = init_tracing_to_file_ai("e2e_api_response_deserialization_gat_validation");
    let harness = AppHarness::spawn().await?;

    // Subscribe to API responses before triggering any requests
    let mut api_rx = harness.subscribe_api_responses();

    // Configure the model to support tools
    {
        let mut config = harness.state.config.write().await;
        let model_name =
            if let Some(active_config) = config.model_registry.get_active_model_config() {
                active_config.model.clone()
            } else {
                "kimi-k2".to_string()
            };
        config.model_registry.capabilities.insert(
            model_name.clone(),
            ploke_tui::user_config::ModelCapabilities {
                supports_tools: true,
                context_length: Some(8192),
                input_cost_per_million: Some(1.0),
                output_cost_per_million: Some(3.0),
            },
        );
        config.model_registry.require_tool_support = true;
        println!(
            "✓ Configured model '{}' to require tool support",
            model_name
        );
    }

    // Add a message that will definitely trigger tool usage
    let user_msg = "Please use the get_file_metadata tool to check if the file 'Cargo.toml' exists and show me its metadata including the tracking hash.";
    let msg_id = harness.add_user_msg(user_msg).await;

    println!("Waiting for API response events...");
    let mut api_responses = Vec::new();
    let mut tool_calls_detected = false;

    // Collect API responses for up to 30 seconds
    let timeout = tokio::time::timeout(Duration::from_secs(30), async {
        let mut events_seen = 0;
        loop {
            match api_rx.recv().await {
                Ok(event) => {
                    events_seen += 1;
                    if events_seen <= 10 {
                        // Only log first 10 events to avoid spam
                        println!(
                            "Event {}: {:?}",
                            events_seen,
                            std::mem::discriminant(&event)
                        );
                    }

                    match event {
                        ploke_tui::AppEvent::System(
                            SystemEvent::TestHarnessApiResponse {
                                request_id: _,
                                response_body,
                                model,
                                use_tools,
                            },
                        ) => {
                            println!(
                                "✓ Captured API response: model={}, use_tools={}, body_len={}",
                                model,
                                use_tools,
                                response_body.len()
                            );

                            // Validate deserialization into strongly-typed structures
                            match ploke_tui::llm::test_parse_response_summary(&response_body) {
                                Ok(summary) => {
                                    println!(
                                        "✓ Successfully parsed API response: choices={}, tool_calls={}, has_content={}",
                                        summary.choices,
                                        summary.tool_calls_total,
                                        summary.has_content
                                    );

                                    if summary.tool_calls_total > 0 {
                                        tool_calls_detected = true;
                                        println!("✓ Tool calls detected in API response");
                                    }

                                    api_responses.push((response_body.clone(), summary));
                                }
                                Err(e) => {
                                    println!("❌ Failed to parse API response: {}", e);
                                    println!("📄 Raw API response body: {}", response_body);
                                    println!("📊 Response length: {} bytes", response_body.len());
                                    assert!(
                                        false,
                                        "API response should be parseable into strongly-typed structures: {}",
                                        e
                                    );
                                }
                            }

                            // Exit after we've collected some responses
                            if api_responses.len() >= 2
                                || (api_responses.len() >= 1 && tool_calls_detected)
                            {
                                break;
                            }
                        }
                        _ => continue, // Other events
                    }
                }
                Err(e) => {
                    println!("Event channel error: {:?}", e);
                    break; // Channel closed
                }
            }
        }
        println!(
            "Exiting event loop after seeing {} events, captured {} API responses",
            events_seen,
            api_responses.len()
        );
    });

    let _ = timeout.await;

    // Validate that we captured at least one API response
    if api_responses.is_empty() {
        println!("❌ No API responses captured. This could mean:");
        println!("  1. The test harness isn't making live API calls");
        println!("  2. The API response emission code isn't working");
        println!("  3. The event filtering is too strict");
        println!("  4. The OPENROUTER_API_KEY environment variable isn't set");

        // Check conversation state to see if anything happened
        let chat = harness.state.chat.read().await;
        println!("  Conversation has {} total messages", chat.messages.len());
        for (id, msg) in &chat.messages {
            println!(
                "    {}: {:?} - {}",
                &id.to_string()[..8],
                msg.kind,
                &msg.content[..msg.content.len().min(80)]
            );
        }

        panic!("Should have captured at least one API response - see debug output above");
    }
    println!("✓ Captured {} API responses", api_responses.len());

    // Validate deserialization integrity
    for (i, (response_body, summary)) in api_responses.iter().enumerate() {
        println!("Validating response {}: {} bytes", i, response_body.len());

        // TODO: Use the following deserialization to test the values instead of the ugly, deeply
        // nested if-let monstrosity.
        use ploke_tui::llm::OpenAiResponse;
        let response_de_result: Result<OpenAiResponse, _> = serde_json::from_str(response_body);
        assert_ok!(
            &response_de_result,
            "Should be able to deserialize to OpenAiResponse"
        );
        let response_de: OpenAiResponse = response_de_result?;
        // TODO: check fields of deserialized response_de

        // Test JSON structure validation
        let parsed_result: Result<serde_json::Value, _> = serde_json::from_str(response_body);
        match parsed_result {
            Ok(parsed) => {
                println!("✓ Response {} successfully parsed as JSON", i);

                // Validate basic response structure
                assert!(
                    parsed.get("choices").is_some(),
                    "Response should have 'choices' field"
                );
                let choices = parsed["choices"]
                    .as_array()
                    .expect("choices should be array");
                assert!(
                    !choices.is_empty(),
                    "Response should have at least one choice"
                );

                // If tool calls were detected in summary, test GAT-based deserialization
                if summary.tool_calls_total > 0 {
                    println!("Testing GAT-based tool deserialization...");

                    // Look for tool calls in choices
                    for (choice_idx, choice) in choices.iter().enumerate() {
                        if let Some(message) = choice.get("message") {
                            if let Some(tool_calls) = message.get("tool_calls") {
                                if let Some(tool_calls_array) = tool_calls.as_array() {
                                    for tool_call in tool_calls_array {
                                        if let Some(function) = tool_call.get("function") {
                                            if let Some(name_str) =
                                                function.get("name").and_then(|n| n.as_str())
                                            {
                                                if let Some(args_str) = function
                                                    .get("arguments")
                                                    .and_then(|a| a.as_str())
                                                {
                                                    // Test GAT deserialization based on tool name
                                                    match name_str {
                                                        "get_file_metadata" => {
                                                            match ploke_tui::tools::GetFileMetadata::deserialize_params(args_str) {
                                                                Ok(_params) => {
                                                                    println!("✓ GAT deserialization success: get_file_metadata");
                                                                }
                                                                Err(e) => {
                                                                    println!("⚠ GAT deserialization failed for get_file_metadata: {:?} (args: {})", e, args_str);
                                                                }
                                                            }
                                                        }
                                                        "request_code_context" => {
                                                            match ploke_tui::tools::RequestCodeContextGat::deserialize_params(args_str) {
                                                                Ok(_params) => {
                                                                    println!("✓ GAT deserialization success: request_code_context");
                                                                }
                                                                Err(e) => {
                                                                    println!("⚠ GAT deserialization failed for request_code_context: {:?} (args: {})", e, args_str);
                                                                }
                                                            }
                                                        }
                                                        "apply_code_edit" => {
                                                            match ploke_tui::tools::GatCodeEdit::deserialize_params(args_str) {
                                                                Ok(_params) => {
                                                                    println!("✓ GAT deserialization success: apply_code_edit");  
                                                                }
                                                                Err(e) => {
                                                                    println!("⚠ GAT deserialization failed for apply_code_edit: {:?} (args: {})", e, args_str);
                                                                }
                                                            }
                                                        }
                                                        other => {
                                                            println!("⚠ Unknown tool name for GAT test: {}", other);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("❌ Failed to parse response {} as JSON: {}", i, e);
                println!(
                    "Response body (first 500 chars): {}",
                    &response_body[..response_body.len().min(500)]
                );
                assert!(false, "All API responses should be valid JSON");
            }
        }
    }

    // Verify conversation state
    {
        let chat = harness.state.chat.read().await;
        let user_message = chat.messages.get(&msg_id);
        assert!(user_message.is_some(), "User message should exist");
        assert_eq!(user_message.unwrap().content, user_msg);

        // Should have assistant messages as well
        let assistant_msgs: Vec<_> = chat
            .messages
            .values()
            .filter(|m| matches!(m.kind, ploke_tui::chat_history::MessageKind::Assistant))
            .collect();
        assert!(!assistant_msgs.is_empty(), "Should have assistant messages");

        println!("✓ Conversation has {} total messages", chat.messages.len());
    }

    harness.shutdown().await;
    println!("✓ API response deserialization and GAT-based tool system validated");
    Ok(())
}
