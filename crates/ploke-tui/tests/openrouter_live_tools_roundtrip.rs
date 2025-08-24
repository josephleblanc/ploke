#![cfg(test)]

use itertools::Itertools;
use lazy_static::lazy_static;
use ploke_db::get_by_id::{GetNodeInfo, NodePaths};
use ploke_db::{Database, create_index_primary};
use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource};
use ploke_embed::local::{EmbeddingConfig, LocalEmbedder};
use ploke_error::Error;
use ploke_rag::{RagConfig, RagService, RrfConfig, TokenBudget};
use ploke_test_utils::workspace_root;
use ploke_tui::error::ErrorExt;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

use ploke_tui::llm::provider_endpoints::ModelEndpointsResponse;
use ploke_tui::llm::{
    apply_code_edit_tool_def, get_file_metadata_tool_def, openrouter_catalog,
    request_code_context_tool_def,
};
use ploke_tui::tracing_setup::init_tracing;
use ploke_tui::user_config::OPENROUTER_URL;

const LLM_TOKEN_BUDGET: usize = 512;

#[allow(dead_code)]
struct ContextToolResponse {
    name: String,
    path: String,
    file: String,
    snippet: String,
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
    headers.insert(x_title, HeaderValue::from_static("Ploke TUI Tests"));
    headers
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

/// Minimal price signal for an endpoint: prompt + completion (per 1M tokens)
fn endpoint_price_hint(ep: &ploke_tui::llm::provider_endpoints::ModelEndpoint) -> f64 {
    ep.pricing.prompt_or_default() + ep.pricing.completion_or_default()
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
            Ok(v) => v
                .get("data")
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
                .unwrap_or_default(),
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
        .json::<ModelEndpointsResponse>()
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
    // Keep these schemas aligned with ploke_tui::llm definitions.
    let request_code_context = serde_json::to_value(request_code_context_tool_def())
        .expect("Error with code context tool tranlsation to json");

    let get_file_metadata = serde_json::to_value(get_file_metadata_tool_def())
        .expect("Error with code context tool tranlsation to json");

    let apply_code_edit = serde_json::to_value(apply_code_edit_tool_def())
        .expect("Error with code context tool tranlsation to json");

    vec![request_code_context, get_file_metadata, apply_code_edit]
}

/// Local execution for the three tools against temporary test targets or fixtures.
fn local_get_file_metadata(file_path: &Path) -> String {
    // Compute a simple JSON result with size and sha256; include a pseudo tracking hash (UUID v5)
    let mut f = fs::File::open(file_path).expect("open temp file");
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).expect("read temp file");
    let size = buf.len() as u64;
    let mut hasher = Sha256::new();
    hasher.update(&buf);
    let hash_hex = format!("{:x}", hasher.finalize());

    // Derive a stable-ish namespace UUID from path for deterministic tests
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
    // splice [start, end)
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

fn local_request_code_context(hint: Option<&str>, token_budget: u32) -> String {
    // Perform a simple scan over fixture files to produce a snippet.
    let root = workspace_root();
    let fixture_dir = root.join("tests/fixture_crates/fixture_nodes/src");
    let mut hits: Vec<(String, String)> = Vec::new();

    if let Ok(entries) = fs::read_dir(&fixture_dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    if let Ok(body) = fs::read_to_string(entry.path()) {
                        let h = hint.unwrap_or("SimpleStruct");
                        if body.contains(h) {
                            // Return up to token_budget/4 chars around the first match
                            let idx = body.find(h).unwrap_or(0);
                            let start = idx.saturating_sub(120);
                            let end = (idx + h.len() + 120).min(body.len());
                            let snippet = body[start..end].to_string();
                            hits.push((
                                entry.file_name().to_string_lossy().to_string(),
                                snippet.chars().take(token_budget as usize / 4).collect(),
                            ));
                        }
                    }
                }
            }
        }
    }

    serde_json::to_string(&json!({
        "hint": hint.unwrap_or_default(),
        "hits": hits
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

lazy_static! {
    /// Optional shared DB restored from a backup of `fixture_nodes` (if present).
    pub static ref TEST_DB_NODES: Result<Arc< ploke_db::Database >, ploke_error::Error> = {
        let db = ploke_db::Database::init_with_schema()?;
        let mut backup = workspace_root();
        backup.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
        // Import if the backup exists; otherwise return the empty DB.
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

/// Execute one forced tool round-trip:
/// 1) Send a request forcing tool_choice to the given tool with provided args
/// 2) Assert tool_calls present; then execute locally on a temp target
/// 3) Send a follow-up with the tool role message and log the final completion
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
    db: &Database
) {
    // Prepare messages
    let mut messages = vec![
        json!({
            "role":"system",
            "content":"You are a tool-using assistant. Prefer calling a tool when one is available."}
        ),
        json!({
            "role":"user",
            "content": format!("Please call the tool '{}' with these JSON arguments, then wait for results:\n{}",
            tool_name, tool_args.to_string())
        }),
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
    info!("first leg '{}' -> {}", tool_name, status);

    let parsed = serde_json::from_str::<Value>(&body).expect("Could not parse json return value");
    let tool_calls = parsed
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c0| c0.get("message"))
        .and_then(|m| m.get("tool_calls"))
        .and_then(|a| a.as_array())
        .cloned()
        .expect("Respose malformed or no tool called");

    assert!(
        !tool_calls.is_empty(),
        "Expected tool_calls for '{}', none found. Body (first LLM_TOKEN_BUDGET): {}",
        tool_name,
        if body.is_empty() {
            "Response body empty"
        } else {
            &body
        }
    );

    // Create temp targets and execute locally
    let tool_call_id = tool_calls
        .first()
        .and_then(|x| x.get("id"))
        .and_then(|s| s.as_str())
        .unwrap_or("call_1")
        .to_string();
    let local_result = match tool_name {
        "get_file_metadata" => {
            let mut tf = NamedTempFile::new().expect("temp file");
            writeln!(tf, "Hello from Ploke tests at {}", chrono::Utc::now()).ok();
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
            let mut token_budget = TokenBudget::default();
            let token_budget_max = tool_args
                .get("token_budget")
                .and_then(|t| t.as_u64())
                .unwrap_or(LLM_TOKEN_BUDGET as u64);
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

            let maybe_node_paths: Result< Vec<NodePaths>, Error > = rag_result.parts.iter().map(|p| {
                 db.paths_from_id(p.id)
            }).map_ok(|rows| {
                let np: Result<NodePaths, Error> = rows.try_into().map_err(Error::from);
                np
                } ).try_collect().expect("Could not parse NodePaths from NamedRows");
            let _node_paths = maybe_node_paths.expect("Could not parse NodePaths from NamedRows");


            let rag_json =
                serde_json::to_value(rag_result).expect("Could not parse rag result to json Value");

            let pretty_print = serde_json::to_string_pretty(&rag_json)
                .expect("Could not format rag json to pretty");
            info!("Tool call success, returning value:\n{}", pretty_print);

            // TODO: Transition this to use the rag_json value instead once we have nailed down our
            // tool use semantics and format.
            // rag_json
            local_request_code_context(Some(hint), token_budget_max as u32)
        }
        _ => {
            warn!("unknown tool '{}'", tool_name);
            "{}".to_string()
        }
    };

    // Second leg: send tool result to model
    let assistant_msg = json!({
        "role": "assistant",
        "content": Value::Null,
        "tool_calls": [{
            "id": tool_call_id,
            "type": "function",
            "function": {
                "name": tool_name,
                "arguments": serde_json::to_string(&tool_args).unwrap_or_else(|_| "{}".to_string())
            }
        }]
    });

    let tool_msg = json!({
        "role": "tool",
        "tool_call_id": assistant_msg["tool_calls"][0]["id"].as_str().unwrap_or("call_1"),
        "content": local_result
    });

    messages.push(assistant_msg);
    messages.push(tool_msg);

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
        }
        Err(e) => {
            warn!("second leg '{}' failed: {}", tool_name, e);
        }
    }
}

/// Live end-to-end test that:
/// 1) Queries the OpenRouter /models/user for available models
/// 2) For each model, fetches its endpoints
/// 3) Selects a tools-capable provider endpoint (cheapest by prompt+completion price)
/// 4) For each of our real tools, sends a forced tool call and performs the operation locally on temporary targets
///
/// Fails if any forced tool call for a tools-capable endpoint does not produce tool_calls.
/// Requires OPENROUTER_API_KEY to be set.
// #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[tokio::test]
async fn openrouter_live_tools_roundtrip() -> Result<(), Error> {
    let _guard = init_tracing();

    // ----- setup start -----
    let db_handle = TEST_DB_NODES
        .as_ref()
        .expect("Must set up TEST_DB_NODES correctly.");

    let model = LocalEmbedder::new(EmbeddingConfig::default())?;
    let source = EmbeddingSource::Local(model);
    let proc_arc = Arc::new(EmbeddingProcessor::new(source));

    let io_handle = ploke_io::IoManagerHandle::new();

    // RAG service (optional)
    let rag = match RagService::new_full(
        db_handle.clone(),
        Arc::clone(&proc_arc),
        io_handle.clone(),
        RagConfig::default(),
    ) {
        Ok(svc) => Some(Arc::new(svc)),
        Err(e) => {
            e.emit_error();
            None
        }
    }
    .expect("Failed to create full rag service.");

    info!("Success: RAG service built with IO.");

    // Should not error
    rag.bm25_rebuild().await?;

    let Some((api_key, base_url)) = openrouter_env() else {
        eprintln!("Skipping: OPENROUTER_API_KEY not set.");
        return Ok(());
    };
    // ----- setup end -----

    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .default_headers(default_headers())
        .build()
        .expect("client");

    // Fetch user-filtered models
    let models = match openrouter_catalog::fetch_models(&client, &base_url, &api_key).await {
        Ok(m) => m,
        Err(e) => {
            panic!("Failed to fetch OpenRouter catalog: {}", e);
        }
    };
    info!("models/user returned {} entries", models.len());

    // Optional cap for run time via env; default to 10 to keep CI friendly.
    let max_models: usize = std::env::var("PLOKE_LIVE_MAX_MODELS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10);

    let mut processed = 0usize;
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
            "  chosen endpoint: provider='{}' slug_hint='{}' context_length={} price_hint={:.8}",
            endpoint.name,
            provider_slug_hint
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            endpoint.context_length,
            endpoint_price_hint(&endpoint)
        );

        // Build our tool set and targeted args per tool
        let tools = tool_defs();

        // request_code_context
        let rc_args = json!({"token_budget": LLM_TOKEN_BUDGET, "hint":"SimpleStruct"});

        // get_file_metadata | apply_code_edit prepare their own temporary targets internally
        // Use plausible arguments to encourage tool invocation; local execution ignores these values.
        let gfm_args = json!({"file_path":"/etc/hosts"});
        let ace_args = json!({"edits":[{"file_path":"/etc/hosts","expected_file_hash":"ignored","start_byte":0,"end_byte":0,"replacement":"ploke"}]});

        for (def, (name, args)) in tools.iter().zip(vec![
            ("request_code_context", rc_args),
            ("get_file_metadata", gfm_args),
            ("apply_code_edit", ace_args),
        ]) {
            run_tool_roundtrip(
                &client,
                &base_url,
                &api_key,
                &model_id,
                provider_slug_hint.as_deref(),
                def,
                name,
                args,
                &rag,
                db_handle
            )
            .await;
        }

        processed += 1;
        // polite spacing between models
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Ok(())
}
