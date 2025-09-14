//! Online UI command snapshots
//!
//! These tests prioritize realistic, end-to-end flows that exercise the
//! command parser + executor and let background tasks perform network requests
//! (OpenRouter). We snapshot the resulting SysInfo output and also persist
//! fetched payloads into `target/online-fixtures/` for later offline reuse.
//!
//! Running:
//! - Requires `OPENROUTER_API_KEY` in env for online tests to actually run.
//! - Without the key, tests return early and do not assert snapshots.
//! - Approve snapshots: `cargo insta review -p ploke-tui`

use std::path::PathBuf;
use std::time::{Duration, Instant};

use insta::assert_snapshot;
use ploke_tui::app::commands::{exec, parser};
use ploke_tui::test_harness::{app as test_app, get_state, openrouter_env};

/// Write a small text/JSON payload into `target/online-fixtures/` for reuse.
fn write_fixture(name: &str, bytes: &[u8]) {
    let mut dir = PathBuf::from("target/online-fixtures");
    std::fs::create_dir_all(&dir).ok();
    dir.push(name);
    if let Some(parent) = dir.parent() { let _ = std::fs::create_dir_all(parent); }
    let _ = std::fs::write(dir, bytes);
}

/// Waits up to `timeout` for any new sysinfo message containing a substring.
async fn wait_for_sysinfo_contains(substr: &str, timeout: Duration) -> Option<String> {
    let state = get_state().await;
    // Snapshot current known message ids
    let mut seen = {
        let guard = state.chat.0.read().await;
        guard.iter_path().map(|m| m.id).collect::<std::collections::HashSet<_>>()
    };
    let start = Instant::now();
    loop {
        {
            let guard = state.chat.0.read().await;
            for m in guard.iter_path() {
                if !seen.contains(&m.id) {
                    // mark seen for next pass and check
                    // we only care about sysinfo-like outputs
                    if m.content.contains(substr) {
                        return Some(m.content.clone());
                    }
                }
            }
            // expand seen with current snapshot
            seen.extend(guard.iter_path().map(|m| m.id));
        }
        if start.elapsed() > timeout { return None; }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn online_help_and_model_info_list() {
    // Help/info/list do not require network; include as controls in this suite.
    let app_arc = test_app();
    let mut app = app_arc.lock().await;
    // Render /help
    let help_cmd = parser::parse(&app, "/help", ploke_tui::user_config::CommandStyle::Slash);
    exec::execute(&mut app, help_cmd);
    // Render model info
    let mi_cmd = parser::parse(&app, "/model info", ploke_tui::user_config::CommandStyle::Slash);
    exec::execute(&mut app, mi_cmd);
    // Render model list
    let ml_cmd = parser::parse(&app, "/model list", ploke_tui::user_config::CommandStyle::Slash);
    exec::execute(&mut app, ml_cmd);
    drop(app);

    // Collect the last three sysinfo messages
    let state = get_state().await;
    let guard = state.chat.0.read().await;
    let msgs: Vec<String> = guard
        .iter_path()
        .filter(|m| matches!(m.kind, ploke_tui::chat_history::MessageKind::SysInfo))
        .map(|m| m.content.clone())
        .collect();

    let window = if msgs.len() >= 3 { &msgs[msgs.len()-3..] } else { &msgs[..] };
    let joined = window.join("\n---\n");
    assert_snapshot!("online_help_modelinfo_list", joined);
}

#[tokio::test]
async fn online_model_providers_snapshot() {
    // Require key for online calls; return early if not set
    if openrouter_env().is_none() {
        eprintln!("skipping: OPENROUTER_API_KEY not set");
        return;
    }

    // Choose a broadly available model id; allow override via env for flexibility
    let model_id = std::env::var("PLOKE_ONLINE_MODEL_ID")
        .unwrap_or_else(|_| "openai/gpt-4o-mini".to_string());

    // Execute the providers command via UI parser+executor
    let app_arc = test_app();
    let mut app = app_arc.lock().await;
    let cmdline = format!("/model providers {}", model_id);
    let cmd = parser::parse(&app, &cmdline, ploke_tui::user_config::CommandStyle::Slash);
    exec::execute(&mut app, cmd);
    drop(app);

    // Wait for the network task to post its SysInfo message
    let Some(text) = wait_for_sysinfo_contains("Available endpoints for model", Duration::from_secs(10)).await else {
        panic!("Timed out waiting for endpoints sysinfo output");
    };

    // Store the raw text for offline reference
    let fixture_name = format!("model_providers_{}.txt", model_id.replace('/', "_"));
    write_fixture(&fixture_name, text.as_bytes());

    // Normalize obviously variable numbers to keep the snapshot stable enough to catch regressions
    let normalized = regex::Regex::new(r"\b\d+(?:\.\d+)?\b").unwrap().replace_all(&text, "<num>");
    assert_snapshot!("online_model_providers", normalized);
}

#[tokio::test]
async fn online_models_catalog_snapshot() {
    // Require key for online calls; return early if not set
    let Some(_env) = openrouter_env() else {
        eprintln!("skipping: OPENROUTER_API_KEY not set");
        return;
    };

    // Fetch live catalog and persist a filtered slice as fixture
    let client = reqwest::Client::new();
    let Ok(models) = ploke_tui::llm2::router_only::openrouter::OpenRouter::fetch_models(&client).await else {
        // Gracefully skip if the endpoint flakes
        eprintln!("skipping: OpenRouter models endpoint unavailable");
        return;
    };

    // Keep a small, deterministic slice by sorting and taking first N matching a keyword
    let keyword = std::env::var("PLOKE_ONLINE_MODELS_KEYWORD").unwrap_or_else(|_| "openai".to_string());
    let mut filtered: Vec<_> = models.into_iter()
        .filter(|m| m.id.to_string().to_lowercase().contains(&keyword) || m.name.as_str().to_lowercase().contains(&keyword))
        .collect();
    filtered.sort_by(|a,b| a.id.to_string().cmp(&b.id.to_string()));
    filtered.truncate(8);

    // Minimal text projection similar to overlay’s header rows
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("Models ({} results) for '{}':", filtered.len(), keyword));
    for m in &filtered {
        let id = m.id.to_string();
        let name = m.name.as_str();
        lines.push(format!("- {} — {}", id, name));
    }
    let text = lines.join("\n");
    write_fixture(&format!("models_{}.txt", keyword), text.as_bytes());
    assert_snapshot!("online_models_catalog_header_rows", text);
}

