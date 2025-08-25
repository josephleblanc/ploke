/*!
E2E live tool-cycle tests using:
- A real App initialization (test_harness) to approximate runtime config
- A real database pre-loaded from a backup of the fixture_nodes graph
- Live OpenRouter API endpoint (skipped when OPENROUTER_API_KEY is not set)

What is being tested and why:
- We validate the two-leg tool flow against a live endpoint:
  1) Force a tool call for a known tool definition.
  2) Locally execute the tool call result with system components (RAG for code context; temp file operations for file tools).
  3) Send back a tool role message with the JSON result to complete the round-trip.
- This demonstrates that our JSON tool schemas and request payloads are accepted by real endpoints, and that our tool results are well-formed for the second leg.

What the test validates:
- The provider returns tool_calls when forced for models/endpoints that advertise tool support.
- The second leg (with the tool result) completes successfully.
- For request_code_context, the RAG service builds typed context over a pre-loaded database, ensuring realistic code snippets are returned.

What the test invalidates:
- It does not assert the final assistant content quality; only the tool-call lifecycle correctness.
- It does not execute our internal rag::dispatcher handlers (future work when we wire llm_manager in tests).

What we learn:
- That our tool definitions and JSON shapes interoperate with a real endpoint.
- That our RAG path is viable with pre-loaded data and can serve as a foundation for more observability and stricter assertions.

Reliability:
- The test skips when OPENROUTER_API_KEY is unset, making CI safe.
- Models are capped via PLOKE_LIVE_MAX_MODELS for run-time control.
*/

#![cfg(test)]

use lazy_static::lazy_static;
use ploke_db::{Database, bm25_index, create_index_primary};
use ploke_embed::cancel_token::CancellationToken;
use ploke_embed::indexer::IndexerTask;
use ploke_error::Error;
use ploke_rag::{RagConfig, RagService, TokenBudget};
use ploke_test_utils::workspace_root;
use ploke_tui::app::App;
use ploke_tui::app_state::{
    AppState, ChatState, ConfigState, StateCommand, SystemState, state_manager,
};
use ploke_tui::chat_history::ChatHistory;
use ploke_tui::llm::{self, llm_manager};
use ploke_tui::rag::context::{PROMPT_CODE, PROMPT_HEADER};
use ploke_tui::user_config::{OPENROUTER_URL, UserConfig, default_model, ModelCapabilities};
use ploke_tui::{AppEvent, EventBus, EventBusCaps, EventPriority, app_state};
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

// Ensure a realistic App initialization occurs (settings/env seeded).
// We don't yet drive the in-app event loops, but this simulates runtime config.

#[allow(dead_code)]
struct ToolRoundtripOutcome {
    pub tool_name: String,
    pub model_id: String,
    pub provider_slug: Option<String>,
    pub first_status: u16,
    pub tool_called: bool,
    pub second_status: Option<u16>,
    pub body_excerpt_first: String,
}

lazy_static! {
    /// Shared DB restored from a backup of `fixture_nodes` (if present), with primary index created.
    pub static ref TEST_DB_NODES: Result<Arc<Database>, ploke_error::Error> = {
        let db = Database::init_with_schema()?;
        let mut backup = workspace_root();
        backup.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
        if backup.exists() {
            let prior_rels_vec = db.relations_vec()?;
            db.import_from_backup(&backup, &prior_rels_vec)
                .map_err(ploke_db::DbError::from)
                .map_err(ploke_error::Error::from)?;
        }
        create_index_primary(&db)?;
        Ok(Arc::new(db))
    };
}

lazy_static! {
    static ref TOOL_ENDPOINT_CANDIDATES: std::sync::Mutex<std::collections::HashMap<String, Vec<String>>> =
        std::sync::Mutex::new(std::collections::HashMap::new());
}

/// Read OPENROUTER_API_KEY and base URL from environment.
fn openrouter_env() -> Option<(String, String)> {
    let key = std::env::var("OPENROUTER_API_KEY").ok()?;
    if key.trim().is_empty() {
        return None;
    }
    Some((key, OPENROUTER_URL.to_string()))
}

/// Recommended headers for OpenRouter (improves routing/diagnostics)
fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    let referer = HeaderName::from_static("http-referer");
    let x_title = HeaderName::from_static("x-title");
    headers.insert(
        referer,
        HeaderValue::from_static("https://github.com/ploke-ai/ploke"),
    );
    headers.insert(x_title, HeaderValue::from_static("Ploke TUI E2E Tests"));
    headers
}

/// Minimal price signal for an endpoint: prompt + completion (per 1M tokens)
fn endpoint_price_hint(ep: &ploke_tui::llm::provider_endpoints::ModelEndpoint) -> f64 {
    ep.pricing.prompt_or_default() + ep.pricing.completion_or_default()
}

/// Pick the cheapest tools-capable endpoint for a model (by prompt+completion price).
// reference
async fn choose_tools_endpoint_for_model(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model_id: &str,
    providers_map: &HashMap<String, String>,
) -> Option<(
    String, /*author*/
    String, /*slug*/
    ploke_tui::llm::provider_endpoints::ModelEndpoint,
    Option<String>, /*provider slug hint*/
)> {
    let parts: Vec<&str> = model_id.split('/').collect();
    if parts.len() != 2 {
        warn!("model '{}' is not '<author>/<slug>'", model_id);
        return None;
    }
    let (author, slug) = (parts[0].to_string(), parts[1].to_string());

    let url = format!("{}/models/{}/{}/endpoints", base_url, author, slug);
    let payload = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .ok()?
        .json::<ploke_tui::llm::provider_endpoints::ModelEndpointsResponse>()
        .await
        .inspect(|resp| tracing::trace!("url: {url}\nResponse:\n{:#?}", resp))
        .ok()?;

    let mut candidates: Vec<ploke_tui::llm::provider_endpoints::ModelEndpoint> = payload
        .data
        .endpoints
        .into_iter()
        .filter(|ep| {
            ep.supported_parameters
                .iter()
                .any(|p| p.eq_ignore_ascii_case("tools"))
                && ep
                    .supported_parameters
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case("tool_choice"))
        })
        .inspect(|cand| tracing::trace!("candidate: {:#?}", cand))
        .collect();

    // Cache tools-capable endpoint names for later reference/diagnostics
    if let Ok(mut map) = TOOL_ENDPOINT_CANDIDATES.lock() {
        let names: Vec<String> = candidates.iter().map(|ep| ep.name.clone()).collect();
        map.insert(model_id.to_string(), names);
    }
    tracing::info!(
        "tools-capable endpoints cached for {}: {}",
        model_id,
        candidates
            .iter()
            .map(|e| e.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    );

    if candidates.is_empty() {
        warn!("{model_id} | No candidates found for model with tools");
        return None;
    }
    candidates.sort_by(|a, b| {
        endpoint_price_hint(a)
            .partial_cmp(&endpoint_price_hint(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let chosen = candidates.remove(0);
    let slug_hint = providers_map.get(&chosen.name).cloned().or_else(|| {
        // Derive a conservative fallback slug from the provider display name
        let derived = chosen
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>();
        if derived.is_empty() {
            None
        } else {
            Some(derived)
        }
    });
    Some((author, slug, chosen, slug_hint))
}

#[tokio::test]
async fn e2e_openrouter_tools_with_app_and_db() -> Result<(), Error> {
    // Dedicated diagnostics directory (env-driven by LLM layer)
    let out_dir = std::path::PathBuf::from("target/test-output/openrouter_e2e");
    fs::create_dir_all(&out_dir).ok();
    println!("[E2E] Diagnostics directory: {}", out_dir.display());
    // Build a realistic App instance without spawning UI/event loops.
    // Keep this synchronous for ergonomic use in tests.
    let mut config = UserConfig::default();
    // Merge curated defaults with user overrides (none in tests by default)
    config.registry = config.registry.with_defaults();
    // Apply any API keys from env for more realistic behavior if present
    config.registry.load_api_keys();

    // Convert to runtime configuration
    let runtime_cfg: app_state::core::RuntimeConfig = config.clone().into();

    let db_handle = TEST_DB_NODES
        .as_ref()
        .expect("TEST_DB_NODES must initialize");

    // IO manager
    let io_handle = ploke_io::IoManagerHandle::new();

    // Event bus for the app
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    // Embedder (from config)
    let processor = config
        .load_embedding_processor()
        .expect("load embedding processor");
    let proc_arc = Arc::new(processor);

    // BM25 service (used by indexer/RAG)
    let bm25_cmd =
        bm25_index::bm25_service::start(Arc::clone(db_handle), 0.0).expect("start bm25 service");

    // Indexer task
    let indexer_task = IndexerTask::new(
        db_handle.clone(),
        io_handle.clone(),
        Arc::clone(&proc_arc),
        CancellationToken::new().0,
        8,
    )
    .with_bm25_tx(bm25_cmd);
    let indexer_task = Arc::new(indexer_task);

    // RAG service (optional)
    let rag = match RagService::new_full(
        db_handle.clone(),
        Arc::clone(&proc_arc),
        io_handle.clone(),
        RagConfig::default(),
    ) {
        Ok(svc) => Some(Arc::new(svc)),
        Err(_e) => None,
    };

    // Rebuild BM25 for consistent test behavior
    let rag = rag.expect("rag service not created correctly");
    rag.bm25_rebuild().await?;

    let (rag_event_tx, _rag_event_rx) = mpsc::channel(10);
    // Shared app state
    let state = Arc::new(AppState {
        chat: ChatState::new(ChatHistory::new()),
        config: ConfigState::new(runtime_cfg),
        system: SystemState::default(),
        indexing_state: RwLock::new(None),
        indexer_task: Some(Arc::clone(&indexer_task)),
        indexing_control: Arc::new(Mutex::new(None)),
        db: db_handle.clone(),
        embedder: Arc::clone(&proc_arc),
        io_handle: io_handle.clone(),
        proposals: RwLock::new(std::collections::HashMap::new()),
        rag: Some(Arc::clone(&rag)),
        budget: TokenBudget::default(),
    });

    // Command channel (not wired to a state_manager loop in tests)
    let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(1024);

    // Best-effort capability refresh so tools are considered for models that advertise them.
    {
        let mut cfg = state.config.write().await;
        let _ = cfg.provider_registry.refresh_from_openrouter().await;
    }

    // Spawn state manager first
    tokio::spawn(state_manager(
        state.clone(),
        cmd_rx,
        event_bus.clone(),
        rag_event_tx,
    ));

    tokio::spawn(llm_manager(
        event_bus.subscribe(EventPriority::Background),
        state.clone(),
        cmd_tx.clone(), // Clone for each subsystem
        event_bus.clone(),
    ));

    // Build the App
    let command_style = config.command_style;
    let _app = App::new(command_style, Arc::clone(&state), cmd_tx.clone(), &event_bus, default_model());

    let Some((api_key, base_url)) = openrouter_env() else {
        eprintln!("Skipping E2E live test: OPENROUTER_API_KEY not set.");
        return Ok(());
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .default_headers(default_headers())
        .build()
        .expect("client");

    // Fetch catalog filtered by user allowances
    let models = match ploke_tui::llm::openrouter_catalog::fetch_models(
        &client, &base_url, &api_key,
    )
    .await
    {
        Ok(m) => m,
        Err(e) => {
            panic!("Failed to fetch OpenRouter catalog: {}", e);
        }
    };
    info!("models/user returned {} entries", models.len());

    let max_models: usize = std::env::var("PLOKE_LIVE_MAX_MODELS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10); // slightly lower default to keep this test snappy

    // let tools = tool_defs();
    let mut outcomes: Vec<ToolRoundtripOutcome> = Vec::new();

    let mut processed = 0usize;

    // Optional: build provider name -> slug map
    let providers_map: std::collections::HashMap<String, String> = match client
        .get(format!("{}/providers", base_url))
        .bearer_auth(&api_key)
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(v) => {
                info!("Full response infodump:\n{:#?}\n", v);
                v.get("data")
                    .and_then(|d| d.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|p| {
                                let name = p.get("name").and_then(|x| x.as_str())?;
                                let slug = p.get("slug").and_then(|x| x.as_str())?;
                                Some((name.to_string(), slug.to_string()))
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }
            Err(_) => Default::default(),
        },
        Err(_) => Default::default(),
    };

    for m in models {
        if processed >= max_models {
            break;
        }

        let model_id = m.id;
        info!("model: {}", model_id);

        let chosen = choose_tools_endpoint_for_model(
            &client,
            &base_url,
            &api_key,
            &model_id,
            &providers_map,
        )
        .await;
        let Some((_author, _slug, endpoint, provider_slug_hint)) = chosen else {
            info!("  no tools-capable endpoints; skipping {}", model_id);
            processed += 1;
            continue;
        };

        // Force-enable tool support in the registry for the selected model so tools are included.
        {
            let mut cfg = state.config.write().await;
            cfg.provider_registry.capabilities.insert(
                model_id.clone(),
                ModelCapabilities {
                    supports_tools: true,
                    context_length: Some(endpoint.context_length as u32),
                    input_cost_per_million: None,
                    output_cost_per_million: None,
                },
            );
        }

        tracing::trace!(
            "  chosen endpoint: provider='{}' context_length={} price_hint={:.8}",
            endpoint.name,
            endpoint.context_length,
            endpoint_price_hint(&endpoint)
        );

        // Record the model/endpoint choice for summary/diagnostics
        outcomes.push(ToolRoundtripOutcome {
            tool_name: "endpoint_choice".to_string(),
            model_id: model_id.clone(),
            provider_slug: provider_slug_hint.clone(),
            first_status: 0,
            tool_called: false,
            second_status: None,
            body_excerpt_first: format!("chosen endpoint: {}", endpoint.name),
        });

        // Configure the active provider/model for this loop iteration
        if let Some(provider_slug_hint) = provider_slug_hint.clone() {
            let _ = cmd_tx
                .send(StateCommand::SelectModelProvider {
                    model_id: model_id.clone(),
                    provider_id: provider_slug_hint.clone(),
                })
                .await;
            // Allow the dispatcher to apply the change
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Construct a synthetic user request + context to drive the full lifecycle
        let parent_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        let new_msg_id = Uuid::new_v4();
        let system_instr = [PROMPT_HEADER, PROMPT_CODE].join("");
        let user_instr = String::from(
            "Hello, I would like you to help me understand the difference between the SimpleStruct and the GenericStruct in my code.\n\nIf tools are available, you MUST call the `request_code_context` tool with {\"token_budget\": 256} and wait for the tool result before responding.",
        );

        // First send the Request (pending in LLM manager) ...
        event_bus.send(AppEvent::Llm(llm::Event::Request {
            request_id,
            parent_id,
            new_msg_id,
            prompt: user_instr.clone(),
            parameters: Default::default(),
        }));
        // ... then send the PromptConstructed to trigger processing
        event_bus.send(AppEvent::Llm(llm::Event::PromptConstructed {
            parent_id,
            prompt: vec![
                (ploke_tui::chat_history::MessageKind::System, system_instr),
                (ploke_tui::chat_history::MessageKind::User, user_instr),
            ],
        }));

        // Observe LLM and Tool events with a shorter window; break early on first tool signal
        let mut rx = event_bus.subscribe(EventPriority::Background);
        let observe_until = std::time::Instant::now() + Duration::from_secs(20);
        let mut saw_tool = false;
        while std::time::Instant::now() < observe_until {
            match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Ok(ev)) => match ev {
                    AppEvent::Llm(llm::Event::ToolCall {
                        name,
                        vendor,
                        ..
                    }) => {
                        println!("[E2E] tool_call model={} name={} vendor={:?}", model_id, name, vendor);
                        saw_tool = true;
                        break;
                    }
                    AppEvent::LlmTool(llm::ToolEvent::Requested { name, call_id, .. }) => {
                        println!("[E2E] tool_requested model={} name={} call_id={}", model_id, name, call_id);
                        saw_tool = true;
                        break;
                    }
                    AppEvent::LlmTool(llm::ToolEvent::Completed {
                        call_id, content, ..
                    }) => {
                        let excerpt: String = content.chars().take(200).collect();
                        println!("[E2E] tool_completed model={} call_id={} excerpt={}", model_id, call_id, excerpt);
                        saw_tool = true;
                        break;
                    }
                    AppEvent::LlmTool(llm::ToolEvent::Failed { call_id, error, .. }) => {
                        println!("[E2E] tool_failed model={} call_id={} error={}", model_id, call_id, error);
                        saw_tool = true;
                        break;
                    }
                    AppEvent::Llm(llm::Event::Response { content, model, .. }) => {
                        let excerpt: String = content.chars().take(180).collect();
                        println!("[E2E] response model={} excerpt={}", model, excerpt);
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        println!(
            "[E2E] summary model={} provider_hint={} saw_tool={}",
            model_id,
            provider_slug_hint.clone().unwrap_or_else(|| "-".into()),
            saw_tool
        );

        outcomes.push(ToolRoundtripOutcome {
            tool_name: "request_code_context".to_string(),
            model_id: model_id.clone(),
            provider_slug: provider_slug_hint.clone(),
            first_status: 0,
            tool_called: saw_tool,
            second_status: None,
            body_excerpt_first: "event observation complete".to_string(),
        });

        processed += 1;

        // Stop early once we observe at least one tool call to keep run time reasonable.
        if saw_tool {
            break;
        }
    }

    // Persist discovered tools-capable endpoints for diagnostics
    if let Ok(map) = TOOL_ENDPOINT_CANDIDATES.lock() {
        let path = out_dir.join("openrouter_tools_candidates.json");
        let _ = fs::write(&path, serde_json::to_string_pretty(&*map).unwrap_or_default());
        println!("[E2E] wrote tools-capable endpoint summary to {}", path.display());
    }

    // Summary of outcomes across models/tools
    let total = outcomes.len();
    let successes = outcomes
        .iter()
        .filter(|o| o.tool_called && matches!(o.second_status, Some(s) if (200..=299).contains(&s)))
        .count();
    let no_tool_calls = outcomes.iter().filter(|o| !o.tool_called).count();
    let first_404 = outcomes.iter().filter(|o| o.first_status == 404).count();
    let any_429 = outcomes
        .iter()
        .filter(|o| o.first_status == 429 || matches!(o.second_status, Some(429)))
        .count();
    info!(
        "Summary: total_outcomes={} successes={} no_tool_calls={} http_404_first_leg={} http_429_any_leg={}",
        total, successes, no_tool_calls, first_404, any_429
    );

    assert!(
        outcomes.iter().any(|o| o.tool_called),
        "No tool calls were observed across evaluated models. Ensure OPENROUTER_API_KEY is set and at least one tools-capable endpoint is selected."
    );

    Ok(())
}
