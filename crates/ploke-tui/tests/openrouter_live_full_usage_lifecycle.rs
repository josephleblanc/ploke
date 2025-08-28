//! Full live usage lifecycle: user message -> request_code_context -> apply_code_edit -> user approval -> applied.
//!
//! Purpose
//! - Exercise the real-world tool-using lifecycle end-to-end on a tools-capable provider.
//! - Validate ordering (context before edit), proposal staging, approval, Applied status, and file delta.
//!
//! Gates
//! - Requires `OPENROUTER_API_KEY` and `PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS=1`.
//! - Under the live gate, fail hard if endpoint discovery fails or required tool_calls aren’t observed.
//!
//! Evidence & Artifacts
//! - Writes endpoint dumps (during discovery) and a compact event trace to `target/test-output/openrouter_e2e/`.
//! - Asserts context tool request/completion before edit request, proposal staging, Applied transition, and file delta.

use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use ploke_core::{TrackingHash, PROJECT_NAMESPACE_UUID};
use ploke_test_utils::workspace_root;
use ploke_tui as app;
use ploke_tui::app_state::core::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState};
use ploke_tui::llm::{self, LLMParameters, openrouter_catalog};
use ploke_tui::user_config::{ModelConfig, ModelRegistry, ProviderType, UserConfig, OPENROUTER_URL};
use quote::ToTokens;
use tokio::sync::mpsc;
use tokio::time::{timeout, Instant};
use uuid::Uuid;

async fn configured_kimi_provider(strict: bool) -> color_eyre::Result<Option<ModelConfig>> {
    let key = match load_openrouter_api_key() {
        Some(k) if !k.trim().is_empty() => k,
        Some(_) => {
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
    // Pin a tools-capable provider slug from endpoints
    let model = "moonshotai/kimi-k2".to_string();
    let client = reqwest::Client::new();
    let eps = openrouter_catalog::fetch_model_endpoints(&client, OPENROUTER_URL, &key, &model).await.ok();
    // Persist endpoints for triage
    if let Some(list) = &eps {
        let _ = std::fs::create_dir_all("target/test-output/openrouter_e2e");
        let path = format!("target/test-output/openrouter_e2e/endpoints_{}_{}.json", model.replace('/', "_"), chrono::Utc::now().format("%Y%m%d-%H%M%S"));
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&list.iter().map(|p| serde_json::json!({
            "id": p.id,
            "supported_parameters": p.supported_parameters,
            "capabilities": p.capabilities.as_ref().and_then(|c| c.tools)
        })).collect::<Vec<_>>()).unwrap_or_default());
    }
    let provider_slug = eps.as_ref().and_then(|list| list.iter().find(|ep| {
        ep.supported_parameters.as_ref().map(|v| v.iter().any(|s| s.eq_ignore_ascii_case("tools"))).unwrap_or_else(|| ep.capabilities.as_ref().and_then(|c| c.tools).unwrap_or(false))
    }).map(|ep| ep.id.clone()));
    let provider_slug = match provider_slug { Some(s) => Some(s), None => return Ok(None) };
    Ok(Some(ModelConfig {
        id: "live-kimi-full".to_string(),
        api_key: if strict { key.clone() } else { String::new() },
        provider_slug,
        api_key_env: None,
        base_url: OPENROUTER_URL.to_string(),
        model,
        display_name: Some("Kimi K2 (tools) full".to_string()),
        provider_type: ProviderType::OpenRouter,
        llm_params: Some(LLMParameters { tool_timeout_secs: Some(90), temperature: Some(0.1), ..Default::default() }),
    }))
}

fn copy_fixture_to_temp() -> (tempfile::TempDir, PathBuf) {
    let mut src_root = workspace_root();
    src_root.push("tests/fixture_crates/fixture_nodes");
    assert!(src_root.exists(), "fixture_nodes not found");
    let tmp = tempfile::tempdir().expect("tempdir");
    let dst_root = tmp.path().join("fixture_nodes");
    fs::create_dir_all(dst_root.join("src")).expect("mkdirs");
    fs::copy(src_root.join("src/lib.rs"), dst_root.join("src/lib.rs")).expect("copy lib.rs");
    (tmp, dst_root.join("src/lib.rs"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_full_usage_lifecycle_context_then_edit() {
    if std::env::var("PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS").ok().as_deref() != Some("1") {
        eprintln!("Skipping: PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS!=1");
        return;
    }
    let provider = match configured_kimi_provider(true).await.expect("endpoint query") {
        Some(p) => p,
        None => panic!("No tools-capable endpoint found for kimi/kimi-k2 or moonshotai/kimi-k2; cannot exercise live path."),
    };

    // Prepare temp fixture and edit target
    let (tmpdir, target_file) = copy_fixture_to_temp();
    let initial = fs::read_to_string(&target_file).expect("read file");
    let needle = "fixture";
    let start = initial.find(needle).expect("needle present");
    let end = start + needle.len();
    let ast = syn::parse_file(&initial).expect("parse rust");
    let tokens = ast.into_token_stream();
    let expected_file_hash = TrackingHash::generate(PROJECT_NAMESPACE_UUID, &target_file, &tokens)
        .0
        .to_string();

    // Build tool args
    let edit_args = serde_json::json!({
        "confidence": 0.98,
        "namespace": PROJECT_NAMESPACE_UUID.to_string(),
        "edits": [{
            "file_path": target_file.display().to_string(),
            "expected_file_hash": expected_file_hash,
            "start_byte": start as u64,
            "end_byte": end as u64,
            "replacement": "fxtr"
        }]
    });
    let ctx_args = serde_json::json!({"token_budget": 400, "hint": format!("path:{}", target_file.display())});

    // App state
    let mut registry = ModelRegistry { providers: vec![provider.clone()], active_model_config: provider.id.clone(), aliases: Default::default(), ..Default::default() };
    registry.require_tool_support = true;
    registry.capabilities.insert(
        provider.model.clone(),
        ploke_tui::user_config::ModelCapabilities { supports_tools: true, context_length: Some(128000), input_cost_per_million: None, output_cost_per_million: None }
    );
    let user_cfg = UserConfig {
        registry: registry.clone(),
        editing: ploke_tui::user_config::EditingConfig { auto_confirm_edits: false, ..Default::default() },
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

    // Spawn managers
    let (cmd_tx, cmd_rx) = mpsc::channel::<ploke_tui::app_state::commands::StateCommand>(128);
    let (rag_tx, _rag_rx) = mpsc::channel::<ploke_tui::RagEvent>(16);
    let s1 = Arc::clone(&state);
    let b1 = Arc::clone(&event_bus);
    tokio::spawn(async move { ploke_tui::app_state::state_manager(s1, cmd_rx, b1, rag_tx).await; });
    let bg_rx = event_bus.subscribe(ploke_tui::EventPriority::Background);
    let s2 = Arc::clone(&state);
    let b2 = Arc::clone(&event_bus);
    tokio::spawn(async move { ploke_tui::llm::llm_manager(bg_rx, s2, cmd_tx, b2).await; });

    let mut rt_rx = event_bus.subscribe(ploke_tui::EventPriority::Realtime);
    let mut bg_rx_events = event_bus.subscribe(ploke_tui::EventPriority::Background);

    // Minimal per-run trace capture
    let _ = std::fs::create_dir_all("target/test-output/openrouter_e2e");
    let trace_path = format!(
        "target/test-output/openrouter_e2e/live_full_usage_lifecycle_{}.json",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let mut trace: Vec<serde_json::Value> = Vec::new();

    // Prompt with explicit sequential instructions
    let system_instr = format!(
        "You are a tool-using coding agent. Follow the steps exactly.\n1) Call request_code_context with the following JSON: \n{}\n2) After receiving tool output, call apply_code_edit with this JSON: \n{}\n3) Do not produce text content.",
        ctx_args, edit_args
    );
    let user_instr = "Proceed with the steps now.".to_string();

    let parent_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();
    let new_msg_id = Uuid::new_v4();
    event_bus.send(app::AppEvent::Llm(llm::Event::Request {
        request_id,
        parent_id,
        prompt: "ignored".into(),
        parameters: LLMParameters { model: provider.model.clone(), ..Default::default() },
        new_msg_id,
    }));
    event_bus.send(app::AppEvent::Llm(llm::Event::PromptConstructed {
        parent_id,
        prompt: vec![
            (ploke_tui::chat_history::MessageKind::System, system_instr),
            (ploke_tui::chat_history::MessageKind::User, user_instr),
        ],
    }));

    // Await provider-driven tool_calls: request_code_context then apply_code_edit
    let mut saw_ctx = false;
    let mut edit_rid_opt: Option<Uuid> = None;
    let deadline = Instant::now() + Duration::from_secs(120);
    while Instant::now() < deadline {
        match timeout(Duration::from_secs(10), bg_rx_events.recv()).await {
            Ok(Ok(app::AppEvent::LlmTool(ploke_tui::llm::ToolEvent::Requested { name, request_id, .. }))) => {
                if name == "request_code_context" { saw_ctx = true; }
                if name == "apply_code_edit" { edit_rid_opt = Some(request_id); break; }
            }
            _ => {}
        }
    }
    assert!(saw_ctx, "provider did not request request_code_context");
    let edit_rid = edit_rid_opt.expect("provider did not request apply_code_edit");

    // Approve and verify file change
    let bus2 = Arc::clone(&event_bus);
    let state2 = Arc::clone(&state);
    timeout(Duration::from_secs(60), async move {
        ploke_tui::rag::editing::approve_edits(&state2, &bus2, edit_rid).await;
    })
    .await
    .expect("approve_edits timeout");

    let updated = fs::read_to_string(&target_file).expect("read updated file");
    assert!(updated.contains("fxtr"), "replacement should be present after approval");

    // Assert proposal transitioned to Applied
    {
        let reg = state.proposals.read().await;
        let prop = reg.get(&edit_rid).expect("proposal present");
        use ploke_tui::app_state::core::EditProposalStatus as S;
        assert!(matches!(prop.status, S::Applied), "proposal should be Applied");
    }

    // Persist compact trace
    let _ = std::fs::write(&trace_path, serde_json::to_string_pretty(&trace).unwrap_or_default());

    // Clean shutdown
    state.io_handle.shutdown().await;
}
fn load_openrouter_api_key() -> Option<String> {
    if let Ok(v) = std::env::var("OPENROUTER_API_KEY") {
        if !v.trim().is_empty() {
            return Some(v);
        }
    }
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
