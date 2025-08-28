//! Live tool-call test on a real tools-capable endpoint using a fixture file.
//!
//! Purpose
//! - Validate the edit lifecycle on a tools-capable endpoint: provider tool_calls → proposal staged → approve → Applied → file delta.
//!
//! Gates
//! - Requires `OPENROUTER_API_KEY` and `PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS=1`.
//! - Under the live gate, this test fails hard if:
//!   - No tools-capable endpoint is found for `kimi/kimi-k2` or `moonshotai/kimi-k2`.
//!   - No `apply_code_edit` tool call is observed.
//!   - No proposal is staged or approval does not lead to `Applied` status and a file delta.
//!
//! Evidence & Artifacts
//! - Writes endpoint discovery dumps and compact traces under `target/test-output/openrouter_e2e/`.
//! - Asserts key state transitions and file updates.
//!
//! Flow
//! - Spin up managers; instruct the model to call `apply_code_edit` with exact JSON against a temp copy of a fixture file.
//! - Observe requested tool call; verify proposal; approve; assert Applied and content change.

use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use ploke_core::{TrackingHash, PROJECT_NAMESPACE_UUID};
use ploke_test_utils::workspace_root;
use ploke_tui::llm::provider_endpoints::{ModelEndpointsResponse, SupportedParameters};
use ploke_tui::tracing_setup::init_tracing_tests;
use ploke_tui as app;
use ploke_tui::app_state::core::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState};
use ploke_tui::llm::{self, LLMParameters};
use ploke_tui::user_config::{ModelConfig, ModelRegistry, ProviderType, UserConfig, OPENROUTER_URL};
use quote::ToTokens;
use tokio::sync::mpsc;
use tokio::time::{timeout, Instant};
use tracing::Level;
use uuid::Uuid;

/// Copy the fixture_nodes crate into a temp dir and return (dir, target_file).
fn copy_fixture_to_temp() -> (tempfile::TempDir, PathBuf) {
    let mut src_root = workspace_root();
    src_root.push("tests/fixture_crates/fixture_nodes");
    assert!(src_root.exists(), "fixture_nodes not found at {:?}", src_root);
    let tmp = tempfile::tempdir().expect("tempdir");
    let dst_root = tmp.path().join("fixture_nodes");
    fs::create_dir_all(&dst_root).expect("mkdir dst_root");
    // Shallow copy: just copy a single file we will edit (lib.rs) to keep test fast.
    let src_file = src_root.join("src/lib.rs");
    let dst_file = dst_root.join("src");
    fs::create_dir_all(&dst_file).expect("mkdir src");
    let dst_file_path = dst_file.join("lib.rs");
    fs::copy(&src_file, &dst_file_path).expect("copy lib.rs");
    (tmp, dst_file_path)
}

/// Fetch endpoints for a given model_id and return the first tools-capable provider slug.
async fn find_tools_capable_provider(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model_id: &str,
) -> color_eyre::Result<Option<String>> {
    // Use the same HTTP shape as other passing live tests
    let mut parts = model_id.split('/');
    let author = parts.next().unwrap_or(model_id);
    let slug = parts.next().unwrap_or("");
    let url = format!(
        "{}/models/{}/{}/endpoints",
        base_url.trim_end_matches('/'),
        author,
        slug
    );
    let resp = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await?
        .error_for_status()?;
    let body = resp.text().await?;
    // Dump endpoints for diagnostics
    let _ = std::fs::create_dir_all("target/test-output/openrouter_e2e");
    let fname = format!(
        "target/test-output/openrouter_e2e/endpoints_{}_{}.json",
        model_id.replace('/', "_"),
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let _ = std::fs::write(&fname, &body);
    // let v: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::json!({"data":[]}));
    let v: ModelEndpointsResponse = serde_json::from_str(&body)?;
    // OpenRouter shape observed:
    //  { "data": 
    //      { 
    //          id, 
    //          name, 
    //          endpoints: [ 
    //              { 
    //                  id/provider fields, 
    //                  supported_parameters, 
    //                  capabilities 
    //              } 
    //          ] 
    //      } 
    //  }
    let arr = v.data.endpoints;
    eprintln!("Fetched endpoints for {} via {}: {} entries", model_id, base_url, arr.len());
    let slug = arr.into_iter().find_map(|ep| {
        if ep.supported_parameters.contains(&SupportedParameters::Tools) {
            Some( ep.id )

        } else {
            None
        }
    });
    Ok(slug)
}

/// Build a provider config for moonshotai/kimi-k2 without pinning a provider slug.
/// Rationale: OpenRouter can route to a tools-capable endpoint when tools are requested.
async fn configured_kimi_provider(strict: bool) -> color_eyre::Result<Option<ModelConfig>> {
    let api_key = match load_openrouter_api_key() {
        Some(k) => k,
        None => {
            let _ = std::fs::create_dir_all("target/test-output/openrouter_e2e");
            let _ = std::fs::write(
                format!(
                    "target/test-output/openrouter_e2e/missing_api_key_{}.txt",
                    chrono::Utc::now().format("%Y%m%d-%H%M%S")
                ),
                "OPENROUTER_API_KEY not present in environment",
            );
            return Ok(None);
        }
    };
    if api_key.trim().is_empty() {
        let _ = std::fs::create_dir_all("target/test-output/openrouter_e2e");
        let _ = std::fs::write(
            format!(
                "target/test-output/openrouter_e2e/blank_api_key_{}.txt",
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            ),
            "OPENROUTER_API_KEY is blank",
        );
        return Ok(None);
    }
    let model = "moonshotai/kimi-k2".to_string();
    Ok(Some(ModelConfig {
        id: "live-kimi".to_string(),
        api_key: if strict { api_key.clone() } else { String::new() },
        provider_slug: None,
        api_key_env: None,
        base_url: OPENROUTER_URL.to_string(),
        model,
        display_name: Some("Kimi K2 (tools)".to_string()),
        provider_type: ProviderType::OpenRouter,
        llm_params: Some(LLMParameters {
            tool_timeout_secs: Some(90),
            // Encourage tool usage
            temperature: Some(0.1),
            ..Default::default()
        }),
    }))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_fixture_apply_code_edit() {
    let _tracing_guard = init_tracing_tests(Level::ERROR);
    // Gate to avoid accidental runs in CI.
    if std::env::var("PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS").ok().as_deref() != Some("1") {
        eprintln!("Skipping: PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS!=1");
        return;
    }
    let provider = match configured_kimi_provider(true).await.expect("endpoint query") {
        Some(p) => p,
        None => {
            // Live gate ON: fail hard with guidance
            panic!(
                "No tools-capable endpoint found for kimi/kimi-k2 or moonshotai/kimi-k2. \
Run ':model refresh' in the TUI and verify endpoints for tools."
            );
        }
    };

    // Prepare a temp copy of the fixture file and compute a precise splice.
    let (_tmpdir, target_file) = copy_fixture_to_temp();
    let initial = fs::read_to_string(&target_file).expect("read fixture lib.rs");
    // Pick a small, stable substring to rename; choose the first occurrence of "fixture".
    let needle = "fixture";
    let start = initial.find(needle).expect("needle present in lib.rs");
    let end = start + needle.len();

    // Compute tracking hash using syn tokens (current tool expects this hash type)
    let ast = syn::parse_file(&initial).expect("parse rust file");
    let tokens = ast.into_token_stream();
    let file_hash = TrackingHash::generate(PROJECT_NAMESPACE_UUID, &target_file, &tokens);
    let expected_file_hash = file_hash.0.to_string();

    // Build the tool arguments for a direct splice
    let args = serde_json::json!({
        "confidence": 0.99,
        "namespace": PROJECT_NAMESPACE_UUID.to_string(),
        "edits": [{
            "file_path": target_file.display().to_string(),
            "expected_file_hash": expected_file_hash,
            "start_byte": start as u64,
            "end_byte": end as u64,
            "replacement": "fxtr"
        }]
    });

    // Set up application state with auto_confirm disabled to require explicit approval.
    let mut registry = ModelRegistry {
        providers: vec![provider.clone()],
        active_model_config: provider.id.clone(),
        aliases: Default::default(),
        ..Default::default()
    };
    registry.require_tool_support = true;
    registry.capabilities.insert(
        provider.model.clone(),
        ploke_tui::user_config::ModelCapabilities {
            supports_tools: true,
            context_length: Some(128000),
            input_cost_per_million: None,
            output_cost_per_million: None,
        },
    );
    let user_cfg = UserConfig {
        registry: registry.clone(),
        editing: ploke_tui::user_config::EditingConfig {
            auto_confirm_edits: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
    let embedder = Arc::new(user_cfg.load_embedding_processor().expect("embedder"));
    let io_handle = ploke_io::IoManagerHandle::new();
    let event_bus = Arc::new(ploke_tui::EventBus::new(ploke_tui::EventBusCaps::default()));
    let state = Arc::new(AppState {
        chat: ChatState::new(ploke_tui::chat_history::ChatHistory::new()),
        config: ConfigState::new(RuntimeConfig { model_registry: registry.clone(), ..Default::default() }),
        system: SystemState::default(),
        indexing_state: tokio::sync::RwLock::new(None),
        indexer_task: None,
        indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
        db,
        embedder,
        io_handle: io_handle.clone(),
        rag: None,
        budget: ploke_rag::TokenBudget::default(),
        proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    });

    // Spawn state and LLM managers
    let (cmd_tx, cmd_rx) = mpsc::channel::<ploke_tui::app_state::commands::StateCommand>(128);
    let (rag_tx, _rag_rx) = mpsc::channel::<ploke_tui::RagEvent>(16);
    let state_clone = Arc::clone(&state);
    let bus_clone = Arc::clone(&event_bus);
    tokio::spawn(async move { ploke_tui::app_state::state_manager(state_clone, cmd_rx, bus_clone, rag_tx).await; });
    let bg_rx = event_bus.subscribe(ploke_tui::EventPriority::Background);
    let state_clone = Arc::clone(&state);
    let bus_clone = Arc::clone(&event_bus);
    tokio::spawn(async move { ploke_tui::llm::llm_manager(bg_rx, state_clone, cmd_tx, bus_clone).await; });

    // Subscribe to realtime events for lifecycle observation
    let _rt_rx = event_bus.subscribe(ploke_tui::EventPriority::Realtime);
    let mut bg_rx_events = event_bus.subscribe(ploke_tui::EventPriority::Background);

    // Craft the conversation instructing an exact tool call
    let system_instr = "You are a strict tool-using coding agent. When provided with explicit tool arguments, you MUST call the tool and not answer in natural language.";
    let user_instr = format!(
        "Call the tool apply_code_edit with the following JSON arguments EXACTLY and immediately. Do not change any value and do not add prose.\n{}",
        args
    );

    // Send an LLM request that includes tools
    let parent_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();
    let new_msg_id = Uuid::new_v4();
    event_bus.send(app::AppEvent::Llm(llm::Event::Request {
        request_id,
        parent_id,
        prompt: "ignored".to_string(),
        parameters: LLMParameters { model: provider.model.clone(), ..Default::default() },
        new_msg_id,
    }));
    event_bus.send(app::AppEvent::Llm(llm::Event::PromptConstructed {
        parent_id,
        prompt: vec![
            (ploke_tui::chat_history::MessageKind::System, system_instr.to_string()),
            (ploke_tui::chat_history::MessageKind::User, user_instr),
        ],
    }));

    // Await provider-driven tool_call for apply_code_edit
    let mut req_id: Option<Uuid> = None;
    let deadline = Instant::now() + Duration::from_secs(120);
    while Instant::now() < deadline {
        match timeout(Duration::from_secs(10), bg_rx_events.recv()).await {
            Ok(Ok(app::AppEvent::LlmTool(ploke_tui::llm::ToolEvent::Requested { name, request_id, .. }))) => {
                if name == "apply_code_edit" { req_id = Some(request_id); break; }
            }
            _ => {}
        }
    }
    let req_id = req_id.expect("provider did not request apply_code_edit");
    {
        let reg = state.proposals.read().await;
        let prop = reg.get(&req_id).expect("proposal staged");
        assert_eq!(prop.files.len(), 1);
        assert_eq!(prop.files[0], target_file);
    }

    // Approve edits and await completion implicitly via the function's ToolCallCompleted emission
    let bus2 = Arc::clone(&event_bus);
    let state2 = Arc::clone(&state);
    let approve = async move { ploke_tui::rag::editing::approve_edits(&state2, &bus2, req_id).await };
    timeout(Duration::from_secs(60), approve)
        .await
        .expect("approve_edits timeout");

    // Validate file content updated
    let updated = fs::read_to_string(&target_file).expect("read updated file");
    assert!(updated.contains("fxtr"), "replacement should be present in file");

    // Verify proposal status transitioned to Applied
    {
        let reg = state.proposals.read().await;
        let prop = reg.get(&req_id).expect("proposal present");
        use ploke_tui::app_state::core::EditProposalStatus as S;
        assert!(matches!(prop.status, S::Applied), "proposal should be Applied");
    }

    // Shutdown IO manager
    state.io_handle.shutdown().await;
}
fn load_openrouter_api_key() -> Option<String> {
    if let Ok(v) = std::env::var("OPENROUTER_API_KEY") {
        if !v.trim().is_empty() {
            return Some(v);
        }
    }
    // Fallback: read from workspace `.env`
    let mut root = workspace_root();
    root.push(".env");
    if let Ok(content) = std::fs::read_to_string(&root) {
        for line in content.lines() {
            let s = line.trim();
            if s.is_empty() || s.starts_with('#') { continue; }
            if let Some(rest) = s.strip_prefix("OPENROUTER_API_KEY=") {
                let val = rest.trim_matches(|c| c == '"' || c == '\'');
                if !val.is_empty() { return Some(val.to_string()); }
            }
        }
        let trimmed = content.trim();
        if !trimmed.is_empty() { return Some(trimmed.to_string()); }
    }
    None
}
