use std::collections::BTreeMap;
use std::process::Command;

use ploke_ty_mcp::{McpConfig, McpManager, ServerId, ServerSpec};

fn e2e_enabled() -> bool {
    std::env::var("PLOKE_E2E_MCP").ok().as_deref() == Some("1")
}

fn cmd_available(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn cfg_git() -> McpConfig {
    let srv = ServerSpec {
        id: ServerId("git".to_string()),
        command: "uvx".to_string(),
        args: vec!["mcp-server-git".to_string()],
        env: BTreeMap::new(),
        autostart: true,
        restart_on_exit: false,
        default_timeout_ms: Some(10_000),
        priority: 0,
    };
    McpConfig { servers: vec![srv] }
}

fn cfg_context7() -> McpConfig {
    let srv = ServerSpec {
        id: ServerId("context7".to_string()),
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "@upstash/context7-mcp".to_string()],
        env: BTreeMap::new(),
        autostart: true,
        restart_on_exit: false,
        default_timeout_ms: Some(15_000),
        priority: 1,
    };
    McpConfig { servers: vec![srv] }
}

#[test]
fn mcp_servers_installed() {
    if !e2e_enabled() {
        eprintln!("Skipping install checks: set PLOKE_E2E_MCP=1 to enable.");
        return;
    }
    assert!(cmd_available("uvx"), "uvx not found or not executable");
    assert!(cmd_available("npx"), "npx not found or not executable");
}

#[tokio::test]
async fn git_status_e2e() {
    if !e2e_enabled() {
        eprintln!("Skipping git E2E: set PLOKE_E2E_MCP=1 to enable.");
        return;
    }
    if !cmd_available("uvx") {
        eprintln!("Skipping git E2E: 'uvx' is not available.");
        return;
    }

    let mgr = McpManager::from_config(cfg_git()).await.expect("manager");
    mgr.ensure_started(&ServerId("git".into()))
        .await
        .expect("start git");

    let git = mgr.client_git().expect("git client");
    let status = git.status(".").await.expect("git status");
    assert!(
        !status.trim().is_empty(),
        "git status should return some output"
    );

    // Best-effort cleanup
    let _ = mgr.cancel(&ServerId("git".into())).await;
}

#[tokio::test]
async fn context7_resolve_e2e() {
    if !e2e_enabled() {
        eprintln!("Skipping context7 E2E: set PLOKE_E2E_MCP=1 to enable.");
        return;
    }
    if !cmd_available("npx") {
        eprintln!("Skipping context7 E2E: 'npx' is not available.");
        return;
    }

    let mgr = McpManager::from_config(cfg_context7())
        .await
        .expect("manager");
    mgr.ensure_started(&ServerId("context7".into()))
        .await
        .expect("start context7");

    let ctx = mgr.client_context7().expect("context7 client");
    let text = ctx.resolve_library_id("bevy").await.expect("resolve");
    assert!(
        text.contains("/bevyengine/bevy"),
        "resolve_library_id output should mention '/bevyengine/bevy'"
    );

    // Best-effort cleanup
    let _ = mgr.cancel(&ServerId("context7".into())).await;
}
