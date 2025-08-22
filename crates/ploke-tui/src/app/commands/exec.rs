#![allow(missing_docs)]
//! Command executor for the TUI.
//!
//! Dataflow:
//! - Receives structured `Command` variants from the parser.
//! - Performs async side-effects (config save/load, registry refresh, etc.),
//!   then informs the app via `StateCommand::AddMessageImmediate` or domain-specific
//!   commands. Avoids blocking the UI thread.
//!
//! Critical interactions:
//! - ProviderRegistry updates (load_api_keys, refresh_from_openrouter, strictness).
//! - Model switching delegated to `StateCommand::SwitchModel` which should broadcast
//!   `SystemEvent::ModelSwitched` for UI updates.

use super::parser::Command;
use super::HELP_COMMANDS;
use crate::app::App;
use crate::llm::provider_endpoints::{ModelEndpointsResponse, Pricing};
use crate::{app_state::StateCommand, chat_history::MessageKind, emit_app_event, AppEvent};
use itertools::Itertools;
use std::path::PathBuf;
use tokio::sync::oneshot;
use uuid::Uuid;
use crate::user_config::{ProviderRegistryStrictness, ProviderType, UserConfig, OPENROUTER_URL};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::llm::openrouter_catalog::ModelEntry;
use tracing::{debug, info_span, warn, instrument};

const DATA_DIR: &str = "crates/ploke-tui/data";
const TEST_QUERY_FILE: &str = "queries.json";
const TEST_QUERY_RESULTS: &str = "results.json";


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEndpoint {
    #[serde(default)]
    provider_name: String,
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    supported_parameters: Vec<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    max_completion_tokens: Option<u64>,
    #[serde(default)]
    max_prompt_tokens: Option<u64>,
    #[serde(default)]
    pricing: Pricing,
    #[serde(default)]
    quantization: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    uptime_last_30m: f32,
}

/// Execute a parsed command. Falls back to legacy handler for commands
/// not yet migrated to structured parsing.
pub fn execute(app: &mut App, command: Command) {
    match command {
        Command::Help => show_command_help(app),
        Command::HelpTopic(topic) => show_topic_help(app, &topic),
        Command::ModelList => list_models_async(app),
        Command::ModelInfo => show_model_info_async(app),
        Command::ModelSearch(keyword) => {
            // Open the overlay immediately to avoid perceived delay
            app.open_model_browser(keyword.clone(), Vec::new());
            // Fetch results asynchronously and publish to the UI via AppEvent
            open_model_search(app, &keyword);
        }
        Command::ModelSearchHelp => {
            show_model_search_help(app);
        }
        Command::ModelProviders(model_id) => {
            list_model_providers_async(app, &model_id);
        }
        Command::ModelUse(alias) => {
            // Delegate to existing state manager path to broadcast and apply
            app.send_cmd(StateCommand::SwitchModel {
                alias_or_id: alias,
            });
        }
        Command::ModelRefresh { remote } => {
            let state = app.state.clone();
            let cmd_tx = app.cmd_tx.clone();
            tokio::spawn(async move {
                {
                    let mut cfg = state.config.write().await;
                    cfg.provider_registry.load_api_keys();
                    let _ = cmd_tx
                        .send(StateCommand::AddMessageImmediate {
                            msg: "Reloaded API keys from environment.".to_string(),
                            kind: MessageKind::SysInfo,
                            new_msg_id: Uuid::new_v4(),
                        })
                        .await;
                    if remote {
                        match cfg.provider_registry.refresh_from_openrouter().await {
                            Ok(_) => {
                                let _ = cmd_tx
                                    .send(StateCommand::AddMessageImmediate {
                                        msg: "Refreshed model capabilities from OpenRouter.".to_string(),
                                        kind: MessageKind::SysInfo,
                                        new_msg_id: Uuid::new_v4(),
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = cmd_tx
                                    .send(StateCommand::AddMessageImmediate {
                                        msg: format!("Failed to refresh OpenRouter model registry: {}", e),
                                        kind: MessageKind::SysInfo,
                                        new_msg_id: Uuid::new_v4(),
                                    })
                                    .await;
                            }
                        }
                    }
                }
            });
        }
        Command::ModelLoad(path_opt) => {
            let state = app.state.clone();
            let cmd_tx = app.cmd_tx.clone();
            tokio::spawn(async move {
                let default_path = UserConfig::default_config_path();
                let path_str =
                    path_opt.unwrap_or_else(|| default_path.to_string_lossy().to_string());
                match UserConfig::load_from_path(std::path::Path::new(&path_str)) {
                    Ok(mut new_cfg) => {
                        // Merge curated defaults, reload keys, refresh capabilities if possible
                        new_cfg.registry = new_cfg.registry.with_defaults();
                        new_cfg.registry.load_api_keys();
                        if std::env::var("OPENROUTER_API_KEY")
                            .ok()
                            .map(|s| !s.is_empty())
                            .unwrap_or(false)
                        {
                            let _ = new_cfg.registry.refresh_from_openrouter().await;
                        }

                        // Detect if embedding backend changed (we cannot hot-swap embedder safely yet)
                        let embedding_changed = {
                            let current = state.config.read().await;
                            current.embedding != new_cfg.embedding
                        };

                        // Convert to runtime config and apply
                        let runtime_cfg: crate::app_state::core::RuntimeConfig =
                            new_cfg.clone().into();
                        {
                            let mut guard = state.config.write().await;
                            *guard = runtime_cfg;
                        }

                        // Inform user about effects
                        let mut msg = format!("Loaded configuration from {}", path_str);
                        if embedding_changed {
                            msg.push_str(
                                "\nNote: Embedding backend changed; restart recommended for changes to take effect.",
                            );
                        }

                        let _ = cmd_tx
                            .send(StateCommand::AddMessageImmediate {
                                msg,
                                kind: MessageKind::SysInfo,
                                new_msg_id: Uuid::new_v4(),
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = cmd_tx
                            .send(StateCommand::AddMessageImmediate {
                                msg: format!(
                                    "Failed to load configuration from {}: {}",
                                    path_str, e
                                ),
                                kind: MessageKind::SysInfo,
                                new_msg_id: Uuid::new_v4(),
                            })
                            .await;
                    }
                }
            });
        }
        Command::ModelSave { path, with_keys } => {
            let state = app.state.clone();
            let cmd_tx = app.cmd_tx.clone();
            tokio::spawn(async move {
                let cfg = state.config.read().await;
                let default_path = UserConfig::default_config_path();
                let path_buf = path.map(PathBuf::from).unwrap_or(default_path);
                let redact = !with_keys;

                let uc = cfg.to_user_config();
                match uc.save_to_path(&path_buf, redact) {
                    Ok(_) => {
                        let _ = cmd_tx
                            .send(StateCommand::AddMessageImmediate {
                                msg: format!(
                                    "Saved configuration to {}{}",
                                    path_buf.display(),
                                    if redact { " (keys redacted)" } else { "" }
                                ),
                                kind: MessageKind::SysInfo,
                                new_msg_id: Uuid::new_v4(),
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = cmd_tx
                            .send(StateCommand::AddMessageImmediate {
                                msg: format!("Failed to save configuration: {}", e),
                                kind: MessageKind::SysInfo,
                                new_msg_id: Uuid::new_v4(),
                            })
                            .await;
                    }
                }
            });
        }
        Command::ProviderStrictness(mode) => {
            let state = app.state.clone();
            let cmd_tx = app.cmd_tx.clone();
            tokio::spawn(async move {
                {
                    let mut cfg = state.config.write().await;
                    cfg.provider_registry.strictness = mode.clone();
                }
                let _ = cmd_tx
                    .send(StateCommand::AddMessageImmediate {
                        msg: format!("Provider strictness set to {:?}", mode),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    })
                    .await;
            });
        }
        Command::ProviderToolsOnly(enabled) => {
            let state = app.state.clone();
            let cmd_tx = app.cmd_tx.clone();
            tokio::spawn(async move {
                {
                    let mut cfg = state.config.write().await;
                    cfg.provider_registry.require_tool_support = enabled;
                }
                let _ = cmd_tx
                    .send(StateCommand::AddMessageImmediate {
                        msg: if enabled {
                            "Provider tools-only enforcement enabled: model calls will be blocked unless the active model is marked as tool-capable. Use ':provider tools-only off' to disable.".to_string()
                        } else {
                            "Provider tools-only enforcement disabled.".to_string()
                        },
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    })
                    .await;
            });
        }
        Command::ProviderSelect { model_id, provider_slug } => {
            // Delegate to state layer to pin a specific provider endpoint for a model
            app.send_cmd(StateCommand::SelectModelProvider { model_id, provider_id: provider_slug });
        }
        Command::Update => spawn_update(app),
        Command::EditApprove(id) => {
            app.send_cmd(StateCommand::ApproveEdits { request_id: id });
        }
        Command::EditDeny(id) => {
            app.send_cmd(StateCommand::DenyEdits { request_id: id });
        }
        Command::EditSetPreviewMode(mode) => {
            app.send_cmd(StateCommand::SetEditingPreviewMode { mode });
        }
        Command::EditSetPreviewLines(lines) => {
            app.send_cmd(StateCommand::SetEditingMaxPreviewLines { lines });
        }
        Command::EditSetAutoConfirm(enabled) => {
            app.send_cmd(StateCommand::SetEditingAutoConfirm { enabled });
        }
        Command::Raw(cmd) => execute_legacy(app, &cmd),
    }
}

fn spawn_update(app: &App) {
    let cmd_tx = app.cmd_tx.clone();
    tokio::task::spawn(async move {
        let (scan_tx, scan_rx) = oneshot::channel();
        let _ = cmd_tx.send(StateCommand::ScanForChange { scan_tx }).await;

        let files = match scan_rx.await {
            Ok(files) => {
                files.map(|v| v.into_iter().map(|f| format!("{}", f.display())).join("\n"))
            }
            Err(_) => None,
        };
        let mut msg = String::from("Updating database with files:\n  ");
        msg.push_str(&files.unwrap_or_else(|| "No updates needed".to_string()));
        let _ = cmd_tx
            .send(StateCommand::AddMessageImmediate {
                msg,
                kind: MessageKind::SysInfo,
                new_msg_id: Uuid::new_v4(),
            })
            .await;
    });
}

fn show_model_info_async(app: &App) {
    let state = app.state.clone();
    let cmd_tx = app.cmd_tx.clone();
    tokio::spawn(async move {
        let cfg = state.config.read().await;
        let reg = &cfg.provider_registry;
        let active_id = reg.active_provider.clone();

        if let Some(p) = reg.get_active_provider() {
            let params = p.llm_params.clone().unwrap_or_default();
            let caps = reg.capabilities.get(&p.model);

            let fmt_opt_f32 = |o: Option<f32>| o.map(|v| format!("{:.3}", v)).unwrap_or_else(|| "-".to_string());
            let fmt_opt_u32 = |o: Option<u32>| o.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string());
            let fmt_opt_usize = |o: Option<usize>| o.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string());
            let fmt_opt_u64 = |o: Option<u64>| o.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string());

            let mut lines = vec![
                "Current model settings:".to_string(),
                format!("  Active provider id: {}", active_id),
                format!("  Model: {}", p.model),
                format!("  Base URL: {}", p.base_url),
                format!("  Provider type: {:?}", p.provider_type),
                format!("  Provider slug: {}", p.provider_slug.as_deref().unwrap_or("-")),
                "".to_string(),
                "  LLM parameters:".to_string(),
                format!("    temperature: {}", fmt_opt_f32(params.temperature)),
                format!("    top_p: {}", fmt_opt_f32(params.top_p)),
                format!("    max_tokens: {}", fmt_opt_u32(params.max_tokens)),
                format!("    presence_penalty: {}", fmt_opt_f32(params.presence_penalty)),
                format!("    frequency_penalty: {}", fmt_opt_f32(params.frequency_penalty)),
                format!("    stop_sequences: [{}]", params.stop_sequences.join(", ")),
                format!("    parallel_tool_calls: {}", params.parallel_tool_calls),
                format!("    response_format: {:?}", params.response_format),
                format!("    tool_max_retries: {}", fmt_opt_u32(params.tool_max_retries)),
                format!("    tool_token_limit: {}", fmt_opt_u32(params.tool_token_limit)),
                format!("    tool_timeout_secs: {}", fmt_opt_u64(params.tool_timeout_secs)),
                format!("    history_char_budget: {}", fmt_opt_usize(params.history_char_budget)),
            ];

            lines.push("".to_string());
            lines.push(format!("  Tool policy: require_tool_support: {}", reg.require_tool_support));
            if let Some(c) = caps {
                lines.push("".to_string());
                lines.push("  Capabilities (cache):".to_string());
                lines.push(format!("    supports_tools: {}", c.supports_tools));
                lines.push(format!(
                    "    context_length: {}",
                    c.context_length.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
                ));
                lines.push(format!(
                    "    input_cost_per_million: {}",
                    c.input_cost_per_million
                        .map(|v| format!("{:.4}", v))
                        .unwrap_or_else(|| "-".to_string())
                ));
                lines.push(format!(
                    "    output_cost_per_million: {}",
                    c.output_cost_per_million
                        .map(|v| format!("{:.4}", v))
                        .unwrap_or_else(|| "-".to_string())
                ));
            }

            let _ = cmd_tx
                .send(StateCommand::AddMessageImmediate {
                    msg: lines.join("\n"),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                })
                .await;
        } else {
            let _ = cmd_tx
                .send(StateCommand::AddMessageImmediate {
                    msg: "No active provider configured.".to_string(),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                })
                .await;
        }
    });
}

fn show_command_help(app: &App) {
    app.send_cmd(StateCommand::AddMessageImmediate {
        msg: HELP_COMMANDS.to_string(),
        kind: MessageKind::SysInfo,
        new_msg_id: Uuid::new_v4(),
    });
}

/// Show targeted help for a topic prefix (e.g., "model", "edit", "bm25", "provider", "index").
fn show_topic_help(app: &App, topic_prefix: &str) {
    let t = topic_prefix.to_lowercase();
    let msg = if t.starts_with("model") {
        r#"Model commands:
  model list                         - List available models
  model info                         - Show active model/provider settings
  model use <name>                   - Switch to a configured model by alias or id
  model refresh [--local]            - Refresh model registry (OpenRouter) and API keys; use --local to skip network
  model load [path]                  - Load configuration from path (default: ~/.config/ploke/config.toml)
  model save [path] [--with-keys]    - Save configuration; omit --with-keys to redact secrets
  model search <keyword>             - Search OpenRouter models; interactive browser opens:
                                       ↑/↓ or j/k to navigate, Enter/Space to expand, s to select, q/Esc to close
"#
        .to_string()
    } else if t.starts_with("edit") {
        r#"Edit commands:
  edit preview mode <code|diff>      - Set edit preview mode for proposals
  edit preview lines <N>             - Set max preview lines per section
  edit auto <on|off>                 - Toggle auto-approval of staged edits
  edit approve <request_id>          - Apply staged code edits with this request ID
  edit deny <request_id>             - Deny and discard staged code edits
"#
        .to_string()
    } else if t.starts_with("bm25") {
        r#"BM25 commands:
  bm25 rebuild                       - Rebuild sparse BM25 index
  bm25 status                        - Show sparse BM25 index status
  bm25 save <path>                   - Save sparse index sidecar to file
  bm25 load <path>                   - Load sparse index sidecar from file
  bm25 search <query> [top_k]        - Search with BM25
  hybrid <query> [top_k]             - Hybrid (BM25 + dense) search
"#
        .to_string()
    } else if t.starts_with("provider") {
        r#"Provider commands:
  provider strictness <openrouter-only|allow-custom|allow-any>
                                     - Restrict selectable providers
  provider tools-only <on|off>       - Enforce using only models/providers that support tool calls
  model providers <model_id>         - List provider endpoints for a model and show tool support and slugs
  provider select <model_id> <provider_slug>
                                     - Pin a model to a specific provider endpoint
  provider pin <model_id> <provider_slug>
                                     - Alias for 'provider select'
"#
        .to_string()
    } else if t.starts_with("index") {
        r#"Indexing commands:
  index start [directory]            - Run workspace indexing (defaults to current dir)
  index pause/resume/cancel          - Pause, resume, or cancel indexing
"#
        .to_string()
    } else {
        format!("Unknown help topic '{}'. Try 'help model', 'help edit', 'help bm25', 'help provider', or 'help index'.", topic_prefix)
    };

    app.send_cmd(StateCommand::AddMessageImmediate {
        msg,
        kind: MessageKind::SysInfo,
        new_msg_id: Uuid::new_v4(),
    });
}

fn list_models_async(app: &App) {
    let state = app.state.clone();
    let cmd_tx = app.cmd_tx.clone();
    tokio::spawn(async move {
        let cfg = state.config.read().await;

        let active = cfg.provider_registry.active_provider.clone();
        let caps_count = cfg.provider_registry.capabilities.len();
        let mut lines = vec![format!(
            "Available models (cached capabilities: {}):",
            caps_count
        )];

        for pc in &cfg.provider_registry.providers {
            let display = pc.display_name.as_ref().unwrap_or(&pc.model);
            let marker = if pc.id == active { "*" } else { " " };
            lines.push(format!("{} {:<28}  {}", marker, pc.id, display));
        }

        let _ = cmd_tx
            .send(StateCommand::AddMessageImmediate {
                msg: lines.join("\n"),
                kind: MessageKind::SysInfo,
                new_msg_id: Uuid::new_v4(),
            })
            .await;
    });
}

fn check_api_keys(app: &App) {
    let help_msg = r#"API Key Configuration Check:

 To use LLM features, you need to set your API keys:
 - For OpenRouter models: export OPENROUTER_API_KEY="your-key-here"
 - For OpenAI models: export OPENAI_API_KEY="your-key-here"
 - For Anthropic models: export ANTHROPIC_API_KEY="your-key-here"

 After setting the environment variable, restart the application.
 Use 'model list' to see available models."#;

    app.send_cmd(StateCommand::AddMessageImmediate {
        msg: help_msg.to_string(),
        kind: MessageKind::SysInfo,
        new_msg_id: Uuid::new_v4(),
    });
}

fn show_model_search_help(app: &App) {
    let msg = "Usage: model search <keyword>\n\
Examples:\n  model search gemini\n  model search claude\n  model search qwen\n\
This opens an interactive model browser:\n  ↑/↓ or j/k to navigate, Enter/Space to expand, s to select, q/Esc to close.";
    app.send_cmd(StateCommand::AddMessageImmediate {
        msg: msg.to_string(),
        kind: MessageKind::SysInfo,
        new_msg_id: Uuid::new_v4(),
    });
}

/// List available provider endpoints for a model and highlight tool support.
/// Example: :model providers qwen/qwen-2.5-72b-instruct
#[instrument(skip(app, model_id))]
fn list_model_providers_async(app: &App, model_id: &str) {
    let state = app.state.clone();
    let cmd_tx = app.cmd_tx.clone();
    let model_id = model_id.to_string();
    tokio::spawn(async move {
        let span = info_span!("list_model_providers", model_id = model_id.as_str());
        let _guard = span.enter();
        // Resolve API key
        let (api_key, base_url) = {
            let cfg = state.config.read().await;
            let key = cfg
                .provider_registry
                .providers
                .iter()
                .find(|p| matches!(p.provider_type, ProviderType::OpenRouter))
                .map(|p| p.resolve_api_key())
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                .unwrap_or_default();
            (key, OPENROUTER_URL.to_string())
        };

        if api_key.is_empty() {
            let _ = cmd_tx
                .send(StateCommand::AddMessageImmediate {
                    msg: "Missing OPENROUTER_API_KEY. Set it and try again (e.g., export OPENROUTER_API_KEY=...)".to_string(),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                })
                .await;
            return;
        }

        // Parse model id into author/slug
        let parts: Vec<&str> = model_id.split('/').collect();
        if parts.len() != 2 {
            let _ = cmd_tx
                .send(StateCommand::AddMessageImmediate {
                    msg: format!("Invalid model id '{}'. Expected format '<author>/<slug>'.", model_id),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                })
                .await;
            return;
        }
        let author = parts[0];
        let slug = parts[1];

        let client = Client::new();

        // Build a provider name -> slug map from /providers
        let providers_map: std::collections::HashMap<String, String> = match client
            .get(format!("{}/providers", base_url))
            .bearer_auth(&api_key)
            .send()
            .await
            .and_then(|r| r.error_for_status())
        {
            Ok(resp) => match resp.json::<Value>().await {
                Ok(v) => {
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

        debug!("providers_map loaded: {} entries", providers_map.len());
        // Fetch endpoints for this model
        let url = format!("{}/models/{}/{}/endpoints", base_url, author, slug);
        debug!("fetching model endpoints: {}", url);
        match client
            .get(url)
            .bearer_auth(&api_key)
            .send()
            .await
            .and_then(|r| r.error_for_status())
        {
            Ok(resp) => match resp.json::<ModelEndpointsResponse>().await {
                Ok(payload) => {
                    debug!("endpoints response parsed: {} endpoints", payload.data.endpoints.len());
                    let mut lines = vec![
                        format!("Available endpoints for model '{}':", model_id),
                        "  (Providers marked [tools] advertise tool support)".to_string(),
                    ];

                    if !payload.data.endpoints.is_empty() {
                        for ep in payload.data.endpoints {
                            let supports_tools = ep
                                .supported_parameters
                                .iter()
                                .any(|t| t.eq_ignore_ascii_case("tools"));
                            let slug = providers_map
                                .get(&ep.name)
                                .cloned()
                                .unwrap_or_else(|| ep.name.to_lowercase().replace(' ', "-"));
                            lines.push(format!(
                                "  - {} (slug: {}) {}{}",
                                &ep.name,
                                slug,
                                if supports_tools { "[tools]" } else { "" },
                                if ep.context_length == 0 {
                                    "".to_string()
                                } else {
                                    format!("context length = {}", ep.context_length)
                                }
                            ));
                        }
                    } else {
                        lines.push("  No endpoints returned for this model.".to_string());
                    }

                    lines.push("".to_string());
                    lines.push("To pin a provider endpoint:".to_string());
                    lines.push(format!("  :provider pin {} <provider_slug>", model_id));
                    lines.push("To enforce tool-capable routing:".to_string());
                    lines.push("  :provider tools-only on".to_string());

                    let _ = cmd_tx
                        .send(StateCommand::AddMessageImmediate {
                            msg: lines.join("\n"),
                            kind: MessageKind::SysInfo,
                            new_msg_id: Uuid::new_v4(),
                        })
                        .await;
                }
                Err(e) => {
                    warn!("Failed to parse endpoints response: {}", e);
                    let _ = cmd_tx
                        .send(StateCommand::AddMessageImmediate {
                            msg: format!("Failed to parse endpoints response: {}", e),
                            kind: MessageKind::SysInfo,
                            new_msg_id: Uuid::new_v4(),
                        })
                        .await;
                }
            },
            Err(e) => {
                warn!("Failed to query OpenRouter models: {}", e);
                warn!("Failed to fetch endpoints for {}: {}", model_id, e);
                let _ = cmd_tx
                    .send(StateCommand::AddMessageImmediate {
                        msg: format!("Failed to fetch endpoints for {}: {}", model_id, e),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    })
                    .await;
            }
        }
    });
}

fn open_model_search(app: &mut App, keyword: &str) {
    // Open overlay already done by caller; now spawn a non-blocking fetch and emit results.
    let state = app.state.clone();
    let cmd_tx = app.cmd_tx.clone();
    let keyword_str = keyword.to_string();

    tokio::spawn(async move {
        let span = info_span!("open_model_search", keyword = keyword_str.as_str());
        let _guard = span.enter();
        // Resolve API key from configured OpenRouter provider or env
        let (api_key, base_url) = {
            let cfg = state.config.read().await;
            let key = cfg
                .provider_registry
                .providers
                .iter()
                .find(|p| matches!(p.provider_type, crate::user_config::ProviderType::OpenRouter))
                .map(|p| p.resolve_api_key())
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                .unwrap_or_default();
            (key, OPENROUTER_URL.to_string())
        };

        if api_key.is_empty() {
            let _ = cmd_tx
                .send(StateCommand::AddMessageImmediate {
                    msg: "Missing OPENROUTER_API_KEY. Set it and try again (e.g., export OPENROUTER_API_KEY=...)".to_string(),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                })
                .await;
            return;
        }

        let client = Client::new();
        match crate::llm::openrouter_catalog::fetch_models(&client, &base_url, &api_key).await {
            Ok(models) => {
                let kw_lower = keyword_str.to_lowercase();
                let mut filtered: Vec<ModelEntry> = models
                    .into_iter()
                    .filter(|m| {
                        let id_match = m.id.to_lowercase().contains(&kw_lower);
                        let name_match = m
                            .name
                            .as_ref()
                            .map(|n| n.to_lowercase().contains(&kw_lower))
                            .unwrap_or(false);
                        id_match || name_match
                    })
                    .collect();
                filtered.sort_by(|a, b| a.id.cmp(&b.id));
                debug!("model search filtered {} results for keyword '{}'", filtered.len(), keyword_str);
                // Always emit results; UI will show "0 results" if none
                emit_app_event(AppEvent::ModelSearchResults {
                    keyword: keyword_str,
                    items: filtered,
                }).await;
            }
            Err(e) => {
                let _ = cmd_tx
                    .send(StateCommand::AddMessageImmediate {
                        msg: format!("Failed to query OpenRouter models: {}", e),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    })
                    .await;
            }
        }
    });
}

/// Legacy command executor (Phase 3): handles all existing commands using
/// the original string matching. Newer commands are gradually migrated to
/// structured Command variants.
fn execute_legacy(app: &mut App, cmd_str: &str) {
    // `cmd_str` is already trimmed of style prefixes by the parser.
    let cmd = cmd_str.trim();

    match cmd {
        "help" => show_command_help(app),
        cmd if cmd.starts_with("index start") => {
            let parts: Vec<&str> = cmd.splitn(3, ' ').collect();
            let workspace = if parts.len() >= 3 {
                parts[2].to_string()
            } else {
                ".".to_string()
            };

            match std::fs::metadata(&workspace) {
                Ok(metadata) if metadata.is_dir() => {
                    app.send_cmd(StateCommand::IndexWorkspace {
                        workspace,
                        needs_parse: true,
                    });
                }
                Ok(_) => {
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: format!("Error: '{}' is not a directory", workspace),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                }
                Err(e) => {
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: format!("Error accessing directory '{}': {}", workspace, e),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                }
            }
        }
        "index pause" => app.send_cmd(StateCommand::PauseIndexing),
        "index resume" => app.send_cmd(StateCommand::ResumeIndexing),
        "index cancel" => app.send_cmd(StateCommand::CancelIndexing),
        "check api" => {
            check_api_keys(app);
        }
        "model list" => list_models_async(app),
        cmd if cmd.starts_with("model ") => {
            let alias = cmd.trim_start_matches("model ").trim();
            tracing::debug!("StateCommand::SwitchModel {}", alias);
            if !alias.is_empty() {
                app.send_cmd(StateCommand::SwitchModel {
                    alias_or_id: alias.to_string(),
                });
            }
        }
        "save history" => {
            app.send_cmd(StateCommand::AddMessageImmediate {
                msg: "Saving conversation history...".to_string(),
                kind: MessageKind::SysInfo,
                new_msg_id: Uuid::new_v4(),
            });
            app.send_cmd(StateCommand::SaveState);
        }
        cmd if cmd.starts_with("load crate") => match cmd.trim_start_matches("load crate").trim() {
            crate_name if !crate_name.contains(' ') => {
                app.send_cmd(StateCommand::AddMessageImmediate {
                    msg: format!("Attempting to load code graph for {crate_name}..."),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                });
                app.send_cmd(StateCommand::LoadDb {
                    crate_name: crate_name.to_string(),
                });
            }
            _ => {
                app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: "Please enter the name of the crate you wish to load.\nThe crates with db backups are located in your default config directory.".to_string(),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
            }
        },
        "query load" | "ql" => {
            app.send_cmd(StateCommand::ReadQuery {
                query_name: "default".to_string(),
                file_name: "default.dl".to_string(),
            });
        }
        "save db" | "sd" => {
            app.send_cmd(StateCommand::SaveDb);
        }
        "update" => {
            // De-blocked: already implemented in spawn_update
            spawn_update(app);
        }
        cmd if cmd.starts_with("query load ") => {
            if let Some((query_name, file_name)) =
                cmd.trim_start_matches("query load ").trim().split_once(' ')
            {
                tracing::debug!("Reading Query {} from file {}", query_name, file_name);
                app.send_cmd(StateCommand::ReadQuery {
                    query_name: query_name.to_string(),
                    file_name: file_name.to_string(),
                });
            }
        }
        cmd if cmd.starts_with("batch") => {
            let mut parts = cmd.split_whitespace();
            parts.next(); // skip "batch"
            let prompt_file = parts.next().unwrap_or(TEST_QUERY_FILE);
            let out_file = parts.next().unwrap_or(TEST_QUERY_RESULTS);
            let max_hits = parts.next().and_then(|item| item.parse::<usize>().ok());
            let threshold = parts.next().and_then(|item| item.parse::<f32>().ok());

            let default_prompt_file = format!("{}/{}", DATA_DIR, TEST_QUERY_FILE);
            let default_out_file = format!("{}/{}", DATA_DIR, TEST_QUERY_RESULTS);

            let prompt_file_path = if prompt_file == TEST_QUERY_FILE {
                &default_prompt_file
            } else {
                prompt_file
            };
            let out_file_path = if out_file == TEST_QUERY_RESULTS {
                &default_out_file
            } else {
                out_file
            };

            if prompt_file == TEST_QUERY_FILE && out_file == TEST_QUERY_RESULTS {
                app.send_cmd(StateCommand::AddMessageImmediate {
                    msg: format!("Running batch search with defaults:\n  Input: {DATA_DIR}/{TEST_QUERY_FILE}\n  Output: {DATA_DIR}/{TEST_QUERY_RESULTS}"),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                });
            }

            app.send_cmd(StateCommand::BatchPromptSearch {
                prompt_file: prompt_file_path.to_string(),
                out_file: out_file_path.to_string(),
                max_hits,
                threshold,
            });
        }
        "bm25 status" => {
            app.send_cmd(StateCommand::RagBm25Status);
        }
        cmd if cmd.starts_with("bm25 save ") => {
            let path_str = cmd.trim_start_matches("bm25 save ").trim();
            if path_str.is_empty() {
                app.send_cmd(StateCommand::AddMessageImmediate {
                    msg: "Usage: bm25 save <path>".to_string(),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                });
            } else {
                app.send_cmd(StateCommand::AddMessageImmediate {
                    msg: format!("Saving BM25 index sidecar to {}", path_str),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                });
                app.send_cmd(StateCommand::RagBm25Save {
                    path: PathBuf::from(path_str),
                });
            }
        }
        cmd if cmd.starts_with("bm25 load ") => {
            let path_str = cmd.trim_start_matches("bm25 load ").trim();
            if path_str.is_empty() {
                app.send_cmd(StateCommand::AddMessageImmediate {
                    msg: "Usage: bm25 load <path>".to_string(),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                });
            } else {
                app.send_cmd(StateCommand::AddMessageImmediate {
                    msg: format!("Loading BM25 index sidecar from {}", path_str),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                });
                app.send_cmd(StateCommand::RagBm25Load {
                    path: PathBuf::from(path_str),
                });
            }
        }
        cmd if cmd.starts_with("bm25 rebuild") => {
            app.send_cmd(StateCommand::AddMessageImmediate {
                msg: "Requested BM25 rebuild. This will rebuild the sparse index from currently active primary nodes.".to_string(),
                kind: MessageKind::SysInfo,
                new_msg_id: Uuid::new_v4(),
            });
            app.send_cmd(StateCommand::Bm25Rebuild);
        }
        cmd if cmd.starts_with("bm25 search ") || cmd.starts_with("hybrid ") => {
            let tail = cmd.strip_prefix("bm25 search ").unwrap_or("hybrid ");
            let mut parts = tail.split_whitespace();
            let (top_k, query) = if let Some(first) = parts.next() {
                if let Ok(n) = first.parse::<usize>() {
                    let q = parts.collect::<Vec<_>>().join(" ");
                    (n, q)
                } else {
                    let mut v = Vec::with_capacity(1);
                    v.push(first);
                    v.extend(parts);
                    (10usize, v.join(" "))
                }
            } else {
                (10usize, String::new())
            };

            if query.is_empty() {
                app.send_cmd(StateCommand::AddMessageImmediate {
                    msg: "Usage: 'bm25 search <query> [top_k]' or 'hybrid <query> [top_k]'"
                        .to_string(),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                });
            } else {
                let is_bm25 = cmd.starts_with("bm25 search ");
                if is_bm25 {
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: format!("Searching (BM25) for \"{}\" with top_k={}...", query, top_k),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                    app.send_cmd(StateCommand::Bm25Search { query, top_k });
                } else {
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: format!(
                            "Searching (hybrid) for \"{}\" with top_k={}...",
                            query, top_k
                        ),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                    app.send_cmd(StateCommand::HybridSearch { query, top_k });
                }
            }
        }
        "preview" => {
            app.show_context_preview = !app.show_context_preview;
            app.needs_redraw = true;
        }
        cmd if cmd.starts_with("preview ") => {
            let arg = cmd.trim_start_matches("preview ").trim();
            match arg {
                "on" => app.show_context_preview = true,
                "off" => app.show_context_preview = false,
                "toggle" => app.show_context_preview = !app.show_context_preview,
                _ => {
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: "Usage: preview [on|off|toggle]".to_string(),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                }
            }
            app.needs_redraw = true;
        }
        cmd => {
            show_command_help(app);
            tracing::warn!("Unknown command: {}", cmd);
        }
    }
}


#[cfg(test)]
mod typed_response_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserialize_endpoints_basic() {
        let payload = json!({
            "data": {
                "endpoints": [
                    {
                        "provider_name": "Foo Provider",
                        "context_length": 8192,
                        "supported_parameters": ["tools", "json_output"],
                        "name": "foo/bar",
                        "max_completion_tokens": 4096,
                        "max_prompt_tokens": 8192
                    },
                    {
                        "provider_name": "Bar Provider",
                        "supported_parameters": []
                    }
                ]
            }
        });

        let parsed: ModelEndpointsResponse = serde_json::from_value(payload).expect("valid response");
        assert_eq!(parsed.data.endpoints.len(), 2);
        assert_eq!(parsed.data.endpoints[0].name, "foo/bar");
        assert_eq!(parsed.data.endpoints[0].context_length, 8192);
        assert!(parsed.data.endpoints[0].supported_parameters.iter().any(|t| t.eq_ignore_ascii_case("tools")));
        assert_eq!(parsed.data.endpoints[1].name, "");
        assert!(parsed.data.endpoints[1].context_length == 0);
    }

    #[test]
    fn default_fields_do_not_panic() {
        let minimal = serde_json::json!({"data": {"endpoints": []}});
        let parsed: ModelEndpointsResponse = serde_json::from_value(minimal).unwrap();
        assert!(parsed.data.endpoints.is_empty());
    }
}
