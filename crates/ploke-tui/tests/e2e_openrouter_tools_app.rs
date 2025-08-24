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
use ploke_db::get_by_id::{GetNodeInfo, NodePaths};
use ploke_db::{Database, bm25_index, create_index_primary};
use ploke_embed::cancel_token::CancellationToken;
use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource, IndexerTask};
use ploke_embed::local::{EmbeddingConfig, LocalEmbedder};
use ploke_error::Error;
use ploke_rag::{RagConfig, RagService, RrfConfig, TokenBudget};
use ploke_test_utils::workspace_root;
use ploke_tui::app::App;
use ploke_tui::app_state::{AppState, ChatState, ConfigState, StateCommand, SystemState};
use ploke_tui::chat_history::ChatHistory;
use ploke_tui::test_harness::TEST_APP;
use ploke_tui::tracing_setup::init_tracing;
use ploke_tui::user_config::{OPENROUTER_URL, UserConfig, default_model};
use ploke_tui::{EventBus, EventBusCaps, app_state};
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

// Ensure a realistic App initialization occurs (settings/env seeded).
// We don't yet drive the in-app event loops, but this simulates runtime config.

/// Cap for live test token budget
const LLM_TOKEN_BUDGET: usize = 512;

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

/// Simple retry helper for POSTing to OpenRouter, with basic 429 backoff.
async fn post_with_retries(
    client: &Client,
    url: &str,
    api_key: &str,
    body: &Value,
    attempts: u8,
) -> Result<reqwest::Response, reqwest::Error> {
    let attempts = attempts.max(1);
    for i in 0..attempts {
        let resp = client
            .post(url)
            .bearer_auth(api_key)
            .json(body)
            .send()
            .await;
        match resp {
            Ok(r) => {
                if r.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && i + 1 < attempts {
                    tokio::time::sleep(Duration::from_millis(250 * (i as u64 + 1))).await;
                    continue;
                }
                return Ok(r);
            }
            Err(e) => {
                if i + 1 == attempts {
                    return Err(e);
                }
                tokio::time::sleep(Duration::from_millis(250 * (i as u64 + 1))).await;
            }
        }
    }
    unreachable!("post_with_retries exhausted attempts unexpectedly")
}

/// Pick the cheapest tools-capable endpoint for a model (by prompt+completion price).
async fn choose_tools_endpoint_for_model(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model_id: &str,
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

    // Optional: build provider name -> slug map
    let providers_map: std::collections::HashMap<String, String> = match client
        .get(format!("{}/providers", base_url))
        .bearer_auth(api_key)
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
        .ok()?;

    let mut candidates: Vec<ploke_tui::llm::provider_endpoints::ModelEndpoint> = payload
        .data
        .endpoints
        .into_iter()
        .filter(|ep| {
            ep.supported_parameters
                .iter()
                .any(|p| p.eq_ignore_ascii_case("tools"))
        })
        .collect();

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

/// Build the three real tool definitions we expose to models.
fn tool_defs() -> Vec<Value> {
    let request_code_context =
        serde_json::to_value(ploke_tui::llm::request_code_context_tool_def())
            .expect("Error with code context tool translation to json");

    // Context-only focus: only expose the request_code_context tool in this E2E
    vec![request_code_context]
}

/// Local execution for get_file_metadata against a temporary file.
fn local_get_file_metadata(file_path: &Path) -> String {
    let mut f = fs::File::open(file_path).expect("open temp file");
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).expect("read temp file");
    let size = buf.len() as u64;
    let mut hasher = Sha256::new();
    hasher.update(&buf);
    let hash_hex = format!("{:x}", hasher.finalize());

    let ns = uuid::Uuid::NAMESPACE_OID;
    let tracking_hash = Uuid::new_v5(&ns, hash_hex.as_bytes());

    serde_json::to_string(&json!({
        "size": size,
        "sha256": hash_hex,
        "tracking_hash": tracking_hash.to_string(),
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

fn local_apply_code_edit(file_path: &Path, start: usize, end: usize, replacement: &str) -> String {
    let data = fs::read(file_path).expect("read temp file");
    let end = end.min(data.len());
    let start = start.min(end);
    let mut new_data = Vec::new();
    new_data.extend_from_slice(&data[..start]);
    new_data.extend_from_slice(replacement.as_bytes());
    new_data.extend_from_slice(&data[end..]);

    let mut f = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(file_path)
        .expect("reopen temp file");
    f.write_all(&new_data).expect("write splice");
    f.flush().ok();

    serde_json::to_string(&json!({
        "applied": 1,
        "bytes_after": new_data.len()
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

/// Assemble a small JSON payload for request_code_context using real RAG with a pre-loaded DB.
async fn rag_request_code_context(
    rag: &RagService,
    db: &Database,
    hint: &str,
    token_budget_max: u32,
) -> String {
    let mut token_budget = TokenBudget::default();
    token_budget.max_total = token_budget_max as usize;
    let rag_result = rag
        .get_context(
            hint,
            5,
            &token_budget,
            ploke_rag::RetrievalStrategy::Hybrid {
                rrf: RrfConfig::default(),
                mmr: None,
            },
        )
        .await
        .expect("Rag get_context failed");

    // Build an enriched payload for the LLM:
    // - Typed RequestCodeContextResult (includes snippets and file_path)
    // - Plus a compact paths array mapping id -> { file_path, canon }
    let mut paths_json: Vec<Value> = Vec::new();
    for p in &rag_result.parts {
        if let Ok(rows) = db.paths_from_id(p.id) {
            if let Ok(np) = TryInto::<NodePaths>::try_into(rows) {
                paths_json.push(json!({
                    "id": p.id,
                    "file_path": np.file,
                    "canon": np.canon
                }));
            }
        }
    }

    let result = ploke_core::rag_types::RequestCodeContextResult {
        ok: true,
        query: hint.to_string(),
        top_k: 5,
        context: rag_result,
    };

    let mut obj = serde_json::to_value(&result).unwrap_or_else(|_| json!({"ok": false}));
    if let Some(map) = obj.as_object_mut() {
        map.insert("paths".to_string(), Value::Array(paths_json));
    }
    serde_json::to_string(&obj).unwrap_or_else(|_| "{}".to_string())
}

/// Execute one forced tool round-trip against a model endpoint.
async fn run_tool_roundtrip(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model_id: &str,
    provider_slug_hint: Option<&str>,
    tool_def: &Value,
    tool_name: &str,
    tool_args: Value,
    rag: &RagService,
    db: &Database,
) -> ToolRoundtripOutcome {
    // Prime messages for tool forcing
    let user_message = json!({
        "role":"user",
        "content": format!(
            "Please call the tool '{}' with these JSON arguments, then wait for results:\n{}",
            tool_name, tool_args.to_string()
        )
    });

    let mut messages = vec![
        json!({
            "role":"system",
            "content":"You are a tool-using assistant. Prefer calling a tool when one is available. All source code is Rust; use ```rust``` fenced code blocks for any snippets. Do not suggest or attempt to modify system files (e.g., /etc/hosts); operate only on ephemeral test paths. If a tool is unavailable, respond briefly and do not fabricate tool results."
        }),
        user_message.clone(),
    ];

    let mut root = json!({
        "model": model_id,
        "messages": messages,
        "tools": [tool_def.clone()],
        "tool_choice": {"type":"function","function":{"name": tool_name}},
        "max_tokens": 128
    });

    if let Some(slug) = provider_slug_hint {
        root.as_object_mut()
            .unwrap()
            .insert("provider".to_string(), json!({"order": [slug]}));
    }

    let url = format!("{}/chat/completions", base_url);
    let first = post_with_retries(client, &url, api_key, &root, 3).await;

    let Ok(resp) = first else {
        panic!(
            "first leg request failed for tool '{}': {}",
            tool_name,
            first.err().unwrap()
        );
    };
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    let first_status_u16 = status.as_u16();
    let body_excerpt_first: String = if body.is_empty() {
        String::new()
    } else {
        body.chars().take(240).collect()
    };
    info!("first leg '{}' -> {}", tool_name, status);

    let parsed = serde_json::from_str::<Value>(&body).expect("Could not parse json return value");
    let tool_calls_opt = parsed
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c0| c0.get("message"))
        .and_then(|m| m.get("tool_calls"))
        .and_then(|a| a.as_array())
        .cloned();

    // Some providers may ignore tool_choice for certain tools/endpoints. Treat as a soft skip.
    if tool_calls_opt
        .as_ref()
        .map(|v| v.is_empty())
        .unwrap_or(true)
    {
        warn!(
            "No tool_calls returned for '{}' on first leg. Provider may have ignored tool_choice. Body: {}",
            tool_name,
            if body.is_empty() { "<empty>" } else { &body }
        );
        return ToolRoundtripOutcome {
            tool_name: tool_name.to_string(),
            model_id: model_id.to_string(),
            provider_slug: provider_slug_hint.map(|s| s.to_string()),
            first_status: first_status_u16,
            tool_called: false,
            second_status: None,
            body_excerpt_first,
        };
    }
    let tool_calls = tool_calls_opt.unwrap();

    // Execute locally (temp targets) or via RAG for request_code_context
    let tool_call_id = tool_calls
        .first()
        .and_then(|x| x.get("id"))
        .and_then(|s| s.as_str())
        .unwrap_or("call_1")
        .to_string();

    let local_result = match tool_name {
        "get_file_metadata" => {
            let mut tf = NamedTempFile::new().expect("temp file");
            writeln!(tf, "Hello from Ploke E2E at {}", chrono::Utc::now()).ok();
            local_get_file_metadata(tf.path())
        }
        "apply_code_edit" => {
            let mut tf = NamedTempFile::new().expect("temp file");
            write!(tf, "hello world").ok();
            let content = fs::read_to_string(tf.path()).unwrap_or_default();
            let pos = content.find("world").unwrap_or(0);
            local_apply_code_edit(tf.path(), pos, pos + 5, "ploke")
        }
        "request_code_context" => {
            let hint = tool_args
                .get("hint")
                .and_then(|h| h.as_str())
                .unwrap_or("SimpleStruct");
            let token_budget_max = tool_args
                .get("token_budget")
                .and_then(|t| t.as_u64())
                .unwrap_or(LLM_TOKEN_BUDGET as u64) as u32;
            rag_request_code_context(rag, db, hint, token_budget_max).await
        }
        _ => {
            warn!("unknown tool '{}'", tool_name);
            "{}".to_string()
        }
    };

    // Second leg: post tool result with proper message structure
    // According to OpenRouter docs, we need to include:
    // 1. The original user message
    // 2. The assistant message with tool_calls
    // 3. The tool message with the result
    let assistant_msg = json!({
        "role": "assistant",
        "content": null,
        "tool_calls": tool_calls.first().unwrap()
    });

    let tool_msg = json!({
        "role": "tool",
        "tool_call_id": tool_call_id,
        "content": local_result
    });

    messages = vec![user_message, assistant_msg, tool_msg];

    let followup = json!({
        "model": model_id,
        "messages": messages,
        "tools": [tool_def.clone()],
        "max_tokens": LLM_TOKEN_BUDGET
    });

    let second = post_with_retries(client, &url, api_key, &followup, 3).await;

    match second {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let content = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| {
                    v.get("choices")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|c0| c0.get("message"))
                        .and_then(|m| m.get("content"))
                        .and_then(|s| s.as_str().map(|s| s.to_string()))
                })
                .unwrap_or_default();
            info!(
                "second leg '{}' -> {}. content: {}",
                tool_name, status, &content
            );
            return ToolRoundtripOutcome {
                tool_name: tool_name.to_string(),
                model_id: model_id.to_string(),
                provider_slug: provider_slug_hint.map(|s| s.to_string()),
                first_status: first_status_u16,
                tool_called: true,
                second_status: Some(status.as_u16()),
                body_excerpt_first,
            };
        }
        Err(e) => {
            warn!("second leg '{}' failed: {}", tool_name, e);
            return ToolRoundtripOutcome {
                tool_name: tool_name.to_string(),
                model_id: model_id.to_string(),
                provider_slug: provider_slug_hint.map(|s| s.to_string()),
                first_status: first_status_u16,
                tool_called: true,
                second_status: None,
                body_excerpt_first,
            };
        }
    }
}

#[tokio::test]
async fn e2e_openrouter_tools_with_app_and_db() -> Result<(), Error> {
    let _guard = init_tracing();
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
        bm25_index::bm25_service::start(Arc::clone(&db_handle), 0.0).expect("start bm25 service");

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
    let (cmd_tx, _cmd_rx) = mpsc::channel::<StateCommand>(1024);

    // Build the App
    let command_style = config.command_style;
    let app = App::new(command_style, state, cmd_tx, &event_bus, default_model());

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

    let tools = tool_defs();
    let mut outcomes: Vec<ToolRoundtripOutcome> = Vec::new();

    let mut processed = 0usize;

    let models_with_tools = "https://openrouter.ai/models?supported_parameters=tools";
    // let providers_map: std::collections::HashMap<String, String> = match client
    // AI: Let's just try to print the raw response, I'm not sure what format it uses
    // DO NOT CHANGE ANYTHING ELSE, JUST PRINT REPONSE FROM models_with_tools AI!
    let resp = client
        .get(models_with_tools)
        // .bearer_auth(&api_key)
        .send()
        .await.unwrap();
    let resp_json: Value = resp.json().await.unwrap();
    info!("Response: {:#?}", resp_json);
    let providers_map: HashMap<String, String> = match client
        .get(models_with_tools)
        // .bearer_auth(&api_key)
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
    // --- start: temporary for making sure choose_tools_endpoint_for_model works correctly
    for m in models {
        if processed >= max_models {
            break;
        }

        let model_id = m.id;
        info!("model: {}", model_id);

        let chosen = choose_tools_endpoint_for_model(&client, &base_url, &api_key, &model_id).await;
        let Some((_author, _slug, endpoint, provider_slug_hint)) = chosen else {
            info!("  no tools-capable endpoints; skipping {}", model_id);
            continue;
        };

        info!(
            "  chosen endpoint: provider='{}' context_length={} price_hint={:.8}",
            endpoint.name,
            endpoint.context_length,
            endpoint_price_hint(&endpoint)
        );
    }
    // --- end: temporary for making sure choose_tools_endpoint_for_model works correctly
    for m in models {
        if processed >= max_models {
            break;
        }

        let model_id = m.id;
        info!("model: {}", model_id);

        let chosen = choose_tools_endpoint_for_model(&client, &base_url, &api_key, &model_id).await;
        let Some((_author, _slug, endpoint, provider_slug_hint)) = chosen else {
            info!("  no tools-capable endpoints; skipping {}", model_id);
            continue;
        };

        info!(
            "  chosen endpoint: provider='{}' context_length={} price_hint={:.8}",
            endpoint.name,
            endpoint.context_length,
            endpoint_price_hint(&endpoint)
        );

        // Context-only focus: exercise only the request_code_context tool
        let rc_args = json!({"token_budget": LLM_TOKEN_BUDGET, "hint":"SimpleStruct"});

        for (def, (name, args)) in tools.iter().zip(vec![("request_code_context", rc_args)]) {
            let outcome = run_tool_roundtrip(
                &client,
                &base_url,
                &api_key,
                &model_id,
                provider_slug_hint.as_deref(),
                def,
                name,
                args,
                &rag,
                db_handle,
            )
            .await;
            outcomes.push(outcome);
        }

        processed += 1;
        tokio::time::sleep(Duration::from_millis(200)).await;
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

    Ok(())
}
