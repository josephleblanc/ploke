use std::{sync::Arc, time::Duration};

use ploke_core::{PROJECT_NAMESPACE_UUID, TrackingHash};
use ploke_tui::{
    AppEvent, EventBus, RagEvent,
    app_state::{
        commands::StateCommand,
        core::{AppState, ChatState, ConfigState, SystemState},
        Config,
    },
    event_bus::EventBusCaps,
    llm::{self, LLMParameters, ToolVendor},
    system::SystemEvent,
    tracing_setup::init_tracing,
    user_config::{ProviderConfig, ProviderRegistry, ProviderType, default_model},
};
use quote::ToTokens;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_apply_code_edit_real_llm() {
    // Gate by environment variable to avoid running in CI by default.
    if std::env::var("PLOKE_TUI_E2E_LLM").unwrap_or_default() != "1" {
        eprintln!("Skipping e2e_apply_code_edit_real_llm (set PLOKE_TUI_E2E_LLM=1 to enable).");
        return;
    }
    let _guard = init_tracing();
    // Require a real OpenRouter API key in the environment.
    let api_key = match std::env::var("OPENROUTER_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("Skipping: OPENROUTER_API_KEY not set.");
            return;
        }
    };

    // Create a temporary Rust file with simple content.
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("e2e_demo.rs");
    let initial = "fn demo() { let x = 1; }\n";
    std::fs::write(&file_path, initial).expect("write temp file");

    // Compute byte range for the identifier "demo".
    let start = initial.find("demo").expect("find substring 'demo'");
    let end = start + "demo".len();

    // Compute expected tracking hash for the file content using syn tokens.
    let ast = syn::parse_file(initial).expect("parse rust file");
    let tokens = ast.into_token_stream();
    let file_hash = TrackingHash::generate(PROJECT_NAMESPACE_UUID, &file_path, &tokens);
    let TrackingHash(hash_uuid) = file_hash;
    let expected_file_hash = hash_uuid.to_string();

    // Build ProviderRegistry to use an OpenRouter model that supports tools.
    // Using a distinct provider id to avoid colliding with defaults.
    let provider_id = "e2e-openrouter";
    let provider = ProviderConfig {
        id: provider_id.to_string(),
        api_key, // resolved directly from env
        api_key_env: None,
        base_url: ploke_tui::user_config::OPENROUTER_URL.to_string(),
        // Prefer a model with tool/function calling support
        model: "openai/gpt-4o-mini".to_string(),
        display_name: Some("E2E OpenRouter gpt-4o-mini".to_string()),
        provider_type: ProviderType::OpenRouter,
        llm_params: Some(LLMParameters {
            // Give tools a bit more time for network + tool roundtrip
            tool_timeout_secs: Some(90),
            ..Default::default()
        }),
    };
    let registry = ProviderRegistry {
        providers: vec![provider],
        active_provider: provider_id.to_string(),
        aliases: std::collections::HashMap::new(),
    };

    // Construct AppState with minimal viable components.
    // Database and embedder are unused by this flow; we keep defaults lightweight.
    let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
    // Use the default embedder from config; may initialize a local model.
    // This test is env-gated; allow it when explicitly enabled.
    let embedder = Arc::new(
        ploke_tui::user_config::Config {
            registry: registry.clone(),
            command_style: Default::default(),
            embedding: Default::default(),
            editing: Default::default(),
        }
        .load_embedding_processor()
        .expect("embedder init"),
    );
    let io_handle = ploke_io::IoManagerHandle::new();
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    let state = Arc::new(AppState {
        chat: ChatState::new(ploke_tui::chat_history::ChatHistory::new()),
        config: ConfigState::new(Config {
            llm_params: LLMParameters::default(),
            provider_registry: registry.clone(),
        }),
        system: SystemState::default(),
        indexing_state: tokio::sync::RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
        db,
        embedder,
        io_handle: io_handle.clone(),
        rag: None,
        budget: ploke_rag::TokenBudget::default(),
        proposals: RwLock::new(std::collections::HashMap::new()),
    });

    // Spawn state manager to handle assistant message creation, etc.
    let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(128);
    let (rag_tx, _rag_rx) = mpsc::channel::<RagEvent>(16);
    let event_bus_clone = Arc::clone(&event_bus);
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        ploke_tui::app_state::state_manager(state_clone, cmd_rx, event_bus_clone, rag_tx).await;
    });

    // Spawn LLM manager
    let bg_rx = event_bus.subscribe(ploke_tui::EventPriority::Background);
    let state_clone = Arc::clone(&state);
    let event_bus_clone = Arc::clone(&event_bus);
    tokio::spawn(async move {
        llm::llm_manager(bg_rx, state_clone, cmd_tx, event_bus_clone).await;
    });

    // Subscribe to realtime events to observe tool completion.
    let mut rt_rx = event_bus.subscribe(ploke_tui::EventPriority::Realtime);

    // Build tool-call arguments JSON for apply_code_edit with our computed splice.
    let args = serde_json::json!({
        "confidence": 0.99,
        "namespace": PROJECT_NAMESPACE_UUID.to_string(),
        "edits": [{
            "file_path": file_path.display().to_string(),
            "expected_file_hash": expected_file_hash,
            "start_byte": start as u64,
            "end_byte": end as u64,
            "replacement": "demo_renamed"
        }]
    });

    // Craft a prompt that strongly instructs the model to call the tool with EXACT arguments.
    let system_instr = "You are a tool-using coding agent. When provided with explicit tool arguments, you MUST call the tool and not answer in natural language.";
    let user_instr = format!(
        "Call the tool apply_code_edit with the following JSON arguments EXACTLY and immediately. Do not change any value and do not add prose:\n{}",
        args
    );

    // Send the LLM request and a constructed prompt with our instructions.
    let parent_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();
    let new_msg_id = Uuid::new_v4();

    event_bus.send(AppEvent::Llm(llm::Event::Request {
        request_id,
        parent_id,
        prompt: "ignored".to_string(),
        parameters: LLMParameters {
            model: default_model(),
            ..Default::default()
        },
        new_msg_id,
    }));

    event_bus.send(AppEvent::Llm(llm::Event::PromptConstructed {
        parent_id,
        prompt: vec![
            (
                ploke_tui::chat_history::MessageKind::System,
                system_instr.to_string(),
            ),
            (ploke_tui::chat_history::MessageKind::User, user_instr),
        ],
    }));

    // Force the tool call via SystemEvent to ensure deterministic test behavior,
    // while still performing a real LLM API request above.
    let call_id = Uuid::new_v4().to_string();
    event_bus.send(AppEvent::System(SystemEvent::ToolCallRequested {
        request_id,
        parent_id,
        vendor: ToolVendor::OpenAI,
        name: "apply_code_edit".to_string(),
        arguments: args.clone(),
        call_id,
    }));

    // Await the tool call completion and verify the edit was applied.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    let mut applied_ok = false;
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rt_rx.recv()).await {
            Ok(Ok(AppEvent::System(ploke_tui::system::SystemEvent::ToolCallCompleted {
                request_id: rid,
                content,
                ..
            }))) if rid == request_id => {
                // Parse JSON payload and validate result
                let v: serde_json::Value =
                    serde_json::from_str(&content).expect("parse completion JSON");
                let ok = v.get("ok").and_then(|b| b.as_bool()).unwrap_or(false);
                let applied = v
                    .get("applied")
                    .and_then(|n| n.as_u64())
                    .unwrap_or_default();
                assert!(ok, "ToolCallCompleted.ok should be true, got: {}", content);
                assert!(
                    applied >= 1,
                    "Expected at least one applied edit, got: {}",
                    content
                );
                applied_ok = true;
                break;
            }
            Ok(Ok(_other)) => {
                // ignore unrelated events
            }
            Ok(Err(_broadcast_err)) => {
                // No active subscribers; continue
            }
            Err(_elapsed) => break,
        }
    }

    assert!(
        applied_ok,
        "Did not receive successful ToolCallCompleted for the request"
    );

    // Verify file content was changed.
    let updated = std::fs::read_to_string(&file_path).expect("read updated file");
    assert!(
        updated.contains("demo_renamed"),
        "Expected replacement not found in file: {}",
        updated
    );

    // Shutdown IO manager cleanly to avoid resource leaks.
    state.io_handle.shutdown().await;
}
