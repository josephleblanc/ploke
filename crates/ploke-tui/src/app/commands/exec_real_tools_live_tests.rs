#![cfg(test)]

use lazy_static::lazy_static;
use ploke_test_utils::workspace_root;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

use ploke_db::Database;
use ploke_error::Error;

use crate::llm::openrouter_catalog;
use crate::llm::provider_endpoints::ModelEndpointsResponse;
use crate::tracing_setup::init_tracing;
use crate::user_config::OPENROUTER_URL;

/// OPENROUTER_API_KEY + base URL
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
    // Unreachable: loop always returns by the last iteration
    unreachable!("post_with_retries exhausted attempts unexpectedly")
}

/// Minimal price signal for an endpoint: prompt + completion (per 1M tokens)
fn endpoint_price_hint(ep: &crate::llm::provider_endpoints::ModelEndpoint) -> f64 {
    let p = ep.pricing.prompt_or_default();
    let c = ep.pricing.completion_or_default();
    p + c
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
    crate::llm::provider_endpoints::ModelEndpoint,
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

    let mut candidates: Vec<crate::llm::provider_endpoints::ModelEndpoint> = payload
        .data
        .endpoints
        .into_iter()
        .filter(|ep| {
            ep.supported_parameters
                .iter()
                // Update to use new enum-based approach AI!
                .any(|p| p.eq_ignore_ascii_case("tools"))
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }
    candidates.sort_by(|a, b| {
        endpoint_price_hint(a)
            .partial_cmp(&endpoint_price_hint(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let chosen = candidates.remove(0);
    let slug_hint = providers_map.get(&chosen.name).cloned();
    Some((author, slug, chosen, slug_hint))
}

/// Build the three real tool definitions we expose to models.
fn tool_defs() -> Vec<Value> {
    // Keep these schemas aligned with crate::llm definitions.
    let request_code_context = json!({
        "type": "function",
        "function": {
            "name": "request_code_context",
            "description": "Request additional code context from the repository up to a token budget.",
            "parameters": {
                "type": "object",
                "properties": {
                    "token_budget": { "type": "integer", "minimum": 1, "description": "Maximum tokens of code context to return." },
                    "hint": { "type": "string", "description": "Optional hint to guide which code to retrieve." }
                },
                "required": ["token_budget"],
                "additionalProperties": false
            }
        }
    });

    let get_file_metadata = json!({
        "type": "function",
        "function": {
            "name": "get_file_metadata",
            "description": "Fetch current file metadata to obtain the expected_file_hash (tracking hash UUID) for safe edits.",
            "parameters": {
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Absolute path to the target file." }
                },
                "required": ["file_path"],
                "additionalProperties": false
            }
        }
    });

    let apply_code_edit = json!({
        "type": "function",
        "function": {
            "name": "apply_code_edit",
            "description": "Apply one or more code edits atomically (tempfile + fsync + rename) using ploke-io. Each edit splices bytes [start_byte, end_byte) with replacement.",
            "parameters": {
                "type": "object",
                "properties": {
                    "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                    "namespace": { "type": "string" },
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" },
                                "expected_file_hash": { "type": "string" },
                                "start_byte": { "type": "integer", "minimum": 0 },
                                "end_byte": { "type": "integer", "minimum": 0 },
                                "replacement": { "type": "string" }
                            },
                            "required": ["file_path", "expected_file_hash", "start_byte", "end_byte", "replacement"],
                            "additionalProperties": false
                        },
                        "minItems": 1
                    }
                },
                "required": ["edits"],
                "additionalProperties": false
            }
        }
    });

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
    // Instead of BM25, perform a simple grep over the fixture to produce a small snippet.
    // This keeps the test self-contained. If a richer DB is available, we could swap it in.
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
                            // Return up to token_budget/4 chars from the first match vicinity
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
    /// Wrapped in Arc<Mutex<...>> to protect from concurrent test mutation.
    pub static ref TEST_DB_NODES: Result<Arc<tokio::sync::Mutex<Database>>, Error> = {
        let db = Database::init_with_schema()?;
        let mut backup = workspace_root();
        backup.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
        // Import if the backup exists; otherwise return the empty DB.
        if backup.exists() {
            let prior_rels_vec = db.relations_vec()?;
            db.import_from_backup(&backup, &prior_rels_vec)
                .map_err(ploke_db::DbError::from)
                .map_err(ploke_error::Error::from)?;
        }
        Ok(Arc::new(tokio::sync::Mutex::new(db)))
    };
}

/// Execute one forced tool round-trip:
/// 1) Send a request forcing tool_choice to the given tool with provided args
/// 2) If tool_calls returned, execute locally on a temp target
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
) {
    // Prepare messages
    let mut messages = vec![
        json!({"role":"system","content":"You are a tool-using assistant. Prefer calling a tool when one is available."}),
        json!({"role":"user","content": format!("Please call the tool '{}' with these JSON arguments, then wait for results:\n{}", tool_name, tool_args.to_string())}),
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
        warn!(
            "first leg request failed for tool '{}': {}",
            tool_name,
            first.err().unwrap()
        );
        return;
    };
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    info!("first leg '{}' -> {}", tool_name, status);

    let parsed = serde_json::from_str::<Value>(&body).unwrap_or(json!({}));
    let tool_calls = parsed
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.get(0))
        .and_then(|c0| c0.get("message"))
        .and_then(|m| m.get("tool_calls"))
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();

    if tool_calls.is_empty() {
        warn!(
            "no tool_calls produced for '{}'; body (first 512): {}",
            tool_name,
            &body.chars().take(512).collect::<String>()
        );
        return;
    }

    // Create temp targets and execute locally
    let tool_call_id = tool_calls
        .get(0)
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
            // If the model asked for arguments, we ignore them and perform a safe local edit:
            // Replace "world" with "ploke".
            let content = fs::read_to_string(tf.path()).unwrap_or_default();
            let pos = content.find("world").unwrap_or(0);
            local_apply_code_edit(tf.path(), pos, pos + 5, "ploke")
        }
        "request_code_context" => {
            // Try to leverage the fixture content; token_budget approx
            let hint = tool_args
                .get("hint")
                .and_then(|h| h.as_str())
                .unwrap_or("SimpleStruct");
            let token_budget = tool_args
                .get("token_budget")
                .and_then(|t| t.as_u64())
                .unwrap_or(512) as u32;
            // Optional: If we ever wire RagService, we'd call bm25_rebuild() here.
            local_request_code_context(Some(hint), token_budget)
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
        "max_tokens": 256
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
                        .and_then(|arr| arr.get(0))
                        .and_then(|c0| c0.get("message"))
                        .and_then(|m| m.get("content"))
                        .and_then(|s| s.as_str().map(|s| s.to_string()))
                })
                .unwrap_or_default();
            info!(
                "second leg '{}' -> {}. content (first 160): {}",
                tool_name,
                status,
                content.chars().take(160).collect::<String>()
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
/// Notes:
/// - This test requires OPENROUTER_API_KEY and is ignored by default due to network and cost.
/// - It performs local tool execution; tool schemas are aligned with our LLM plumbing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_real_tools_roundtrip_smoke() {
    if std::env::var("PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS").ok().as_deref() != Some("1") {
        eprintln!("Skipping: PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS!=1");
        return;
    }
    let _guard = init_tracing();

    let Some((api_key, base_url)) = openrouter_env() else {
        eprintln!("Skipping: OPENROUTER_API_KEY not set.");
        return;
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .default_headers(default_headers())
        .build()
        .expect("client");

    // Fetch user-filtered models
    let models = match openrouter_catalog::fetch_models(&client, &base_url, &api_key).await {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to fetch OpenRouter catalog: {}", e);
            return;
        }
    };
    info!("models/user returned {} entries", models.len());

    // Cap iteration for safety (can be increased locally)
    for (processed, m) in models.into_iter().enumerate() {
        if processed >= 5 {
            break;
        }

        let model_id = m.id;
        info!("model: {}", model_id);

        let chosen = choose_tools_endpoint_for_model(&client, &base_url, &api_key, &model_id).await;
        let Some((author, slug, endpoint, provider_slug_hint)) = chosen else {
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
        let rc_args = json!({"token_budget": 512, "hint":"SimpleStruct"});

        // get_file_metadata | apply_code_edit prepare their own temporary targets internally
        let gfm_args = json!({"file_path":"will_be_overridden_by_local_execution"});
        let ace_args = json!({"edits":[{"file_path":"will_be_overridden_by_local_execution","expected_file_hash":"<unused>","start_byte":0,"end_byte":0,"replacement":"ploke"}]});

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
            )
            .await;
        }

        // Be polite between models
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
