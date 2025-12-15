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
//! - ModelRegistry updates (load_api_keys, refresh_from_openrouter, strictness).
//! - Model switching delegated to `StateCommand::SwitchModel` which should broadcast
//!   `SystemEvent::ModelSwitched` for UI updates.

use super::HELP_COMMANDS;
use super::parser::Command;
use crate::app::App;
use crate::llm::request::endpoint::EndpointsResponse;
use crate::llm::router_only::openrouter::{OpenRouter, OpenRouterModelId};
use crate::llm::router_only::{HasEndpoint, HasModels};
use crate::llm::{self, LlmEvent, ProviderKey};
use crate::user_config::{ModelRegistryStrictness, OPENROUTER_URL, UserConfig, openrouter_url};
use crate::{AppEvent, app_state::StateCommand, chat_history::MessageKind, emit_app_event};
use itertools::Itertools;
use ploke_core::ArcStr;
use ploke_llm::embeddings::{EmbClientConfig, HasEmbeddingModels};
use ploke_llm::manager::events::{embedding_models, models};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::{debug, debug_span, info_span, instrument, warn};
use uuid::Uuid;

const DATA_DIR: &str = "crates/ploke-tui/data";
const TEST_QUERY_FILE: &str = "queries.json";
const TEST_QUERY_RESULTS: &str = "results.json";

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
        Command::EmbeddingSearch(keyword) => {
            app.open_embedding_browser(keyword.clone(), Vec::new());
            open_embedding_search(app, &keyword);
        }
        Command::EmbeddingSearchHelp => {
            show_embedding_search_help(app);
        }
        Command::ModelProviders(model_id) => {
            list_model_providers_async(app, &model_id);
        }
        Command::ModelUse(alias) => {
            // Delegate to existing state manager path to broadcast and apply
            app.send_cmd(StateCommand::SwitchModel { alias_or_id: alias });
        }
        Command::ModelRefresh { remote } => {
            let state = app.state.clone();
            let cmd_tx = app.cmd_tx.clone();
            tokio::spawn(async move {
                let _ = cmd_tx
                    .send(StateCommand::AddMessageImmediate {
                        msg: if remote {
                            "Refreshed model list from router (env-based)".to_string()
                        } else {
                            "Reloaded API keys from environment (env-only)".to_string()
                        },
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    })
                    .await;
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
                    Ok(new_cfg) => {
                        // llm: registry prefs used as-is; env-based key resolution in router

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
                    cfg.model_registry.strictness = mode.clone();
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
        Command::ProviderSelect {
            model_id,
            provider_slug,
        } => {
            // Delegate to state layer to pin a specific provider endpoint for a model
            let provider_key = ProviderKey::new(&provider_slug)
                .expect("valid provider slug for endpoint selection");
            app.send_cmd(StateCommand::SelectModelProvider {
                model_id_string: model_id,
                provider_key: Some(provider_key),
            });
        }
        Command::Update => spawn_update(app),
        Command::EditApprove(id) => {
            app.send_cmd(StateCommand::ApproveEdits { request_id: id });
        }
        Command::EditDeny(id) => {
            app.send_cmd(StateCommand::DenyEdits { request_id: id });
        }
        Command::CreateApprove(id) => {
            app.send_cmd(StateCommand::ApproveCreations { request_id: id });
        }
        Command::CreateDeny(id) => {
            app.send_cmd(StateCommand::DenyCreations { request_id: id });
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
        Command::SearchContext(search_term) => {
            tracing::debug!(
                "Command::SearchContext received with search term: {}",
                search_term
            );
            // Open the overlay immediately to avoid perceived delay
            app.open_context_browser(search_term.clone(), Vec::new());
            // Fetch results asynchronously and publish to the UI via AppEvent
            app.dispatch_context_search(&search_term);
            tracing::debug!(
                "Command::SearchContext dispatched context search with search term: {}",
                search_term
            );
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
        use crate::llm::ProviderSlug as _;
        let active = cfg.active_model.to_string();
        let params = cfg.llm_params.clone();
        let endpoints = cfg
            .model_registry
            .models
            .get(&cfg.active_model.key)
            .map(|mp| mp.selected_endpoints.clone())
            .unwrap_or_default();
        let fmt_opt_f32 = |o: Option<f32>| {
            o.map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "-".to_string())
        };
        let fmt_opt_u32 =
            |o: Option<u32>| o.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string());
        // legacy helpers removed (no longer needed): fmt_opt_usize, fmt_opt_u64

        let mut lines = vec![
            "Current model settings:".to_string(),
            format!("  Active model: {}", active),
            "".to_string(),
            "  LLM parameters:".to_string(),
            format!("    temperature: {}", fmt_opt_f32(params.temperature)),
            format!("    top_p: {}", fmt_opt_f32(params.top_p)),
            format!("    top_k: {}", fmt_opt_f32(params.top_k)),
            format!("    max_tokens: {}", fmt_opt_u32(params.max_tokens)),
            format!(
                "    seed: {}",
                params
                    .seed
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string())
            ),
            format!(
                "    presence_penalty: {}",
                fmt_opt_f32(params.presence_penalty)
            ),
            format!(
                "    frequency_penalty: {}",
                fmt_opt_f32(params.frequency_penalty)
            ),
            format!(
                "    repetition_penalty: {}",
                fmt_opt_f32(params.repetition_penalty)
            ),
            format!(
                "    top_logprobs: {}",
                params
                    .top_logprobs
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string())
            ),
            format!("    min_p: {}", fmt_opt_f32(params.min_p)),
            format!("    top_a: {}", fmt_opt_f32(params.top_a)),
        ];

        lines.push("".to_string());
        if endpoints.is_empty() {
            lines.push("  No pinned provider endpoints (router default in use).".to_string());
        } else {
            lines.push("  Pinned provider endpoints:".to_string());
            for ek in endpoints.iter() {
                lines.push(format!("    - {}", ek.provider.slug.as_str()));
            }
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
    } else if t.starts_with("create") {
        r#"Create commands:
  create approve <request_id>        - Apply staged file creations with this request ID
  create deny <request_id>           - Deny and discard staged file creations
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
        format!(
            "Unknown help topic '{}'. Try 'help model', 'help edit', 'help bm25', 'help provider', or 'help index'.",
            topic_prefix
        )
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
        use crate::llm::ProviderSlug as _;
        let active = cfg.active_model.to_string();
        let eps = cfg
            .model_registry
            .models
            .get(&cfg.active_model.key)
            .map(|mp| mp.selected_endpoints.clone())
            .unwrap_or_default();
        let mut lines = vec![format!("Active model: {}", active)];
        if eps.is_empty() {
            lines.push("No pinned provider endpoints; router default will be used.".to_string());
        } else {
            lines.push("Pinned provider endpoints (in selection order):".to_string());
            for ek in eps {
                lines.push(format!("  - {} via {}", active, ek.provider.slug.as_str()));
            }
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

fn show_embedding_search_help(app: &App) {
    let msg = "Usage: embedding search <keyword>\n\
Examples:\n  embedding search text\n  embedding search jina\n\
Opens the embedding model browser:\n  ↑/↓ or j/k to navigate, Enter/Space to expand, s to select, q/Esc to close.";
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
        // Resolve API key and base URL
        let (api_key, base_url) = (
            std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            openrouter_url(),
        );

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
        use std::str::FromStr;
        let typed_model = match crate::llm::ModelId::from_str(&model_id) {
            Ok(m) => OpenRouterModelId::from(m),
            Err(_) => {
                let _ = cmd_tx
                    .send(StateCommand::AddMessageImmediate {
                        msg: format!("Invalid model id: {}", model_id),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    })
                    .await;
                return;
            }
        };
        match OpenRouter::fetch_model_endpoints(&client, typed_model).await {
            // First get the endpoints from openrouter, using
            // typed endpoint entry from `/models/:author/:slug/endpoints`,
            // e.g. for model_id = deepseek-chat-v3.1
            // `https://openrouter.ai/api/v1/models/deepseek/deepseek-chat-v3.1/endpoints`
            Ok(endpoint_data) => {
                let endpoints = endpoint_data.data.endpoints;
                let mut lines = vec![
                    format!("Available endpoints for model '{}':", model_id),
                    "  (Providers marked [tools] advertise tool support)".to_string(),
                ];
                if endpoints.is_empty() {
                    lines.push("  No endpoints returned for this model.".to_string());
                } else {
                    for ep in endpoints {
                        let supports_tools = if ep.supports_tools() { "[tools]" } else { "" };
                        lines.push(format!(
                            "  - {} {}{}",
                            ep.name.as_ref(),
                            supports_tools,
                            format_args!(" context length = {:.0}", ep.context_length)
                        ));
                    }
                }
                lines.push("".to_string());
                lines.push("To pin a provider endpoint:".to_string());
                lines.push(format!("  :provider pin {} <provider_slug>", model_id));
                lines.push("To enforce tool-capable routing:".to_string());
                lines.push("  :provider tools-only on".to_string());

                let _ = cmd_tx
                    .send(StateCommand::AddMessageImmediate {
                        msg: lines.join(
                            "
",
                        ),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    })
                    .await;
            }
            Err(e) => {
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
        let span = debug_span!("open_model_search", keyword = keyword_str.as_str());
        let _guard = span.enter();
        // Resolve API key from configured OpenRouter provider or env
        let (api_key, base_url) = (
            std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            openrouter_url(),
        );

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
        match OpenRouter::fetch_models(&client).await {
            Ok(models_resp) => {
                let total_models = models_resp.data.len();
                let models_arc = Arc::new(models_resp);
                let search_kw = ArcStr::from(keyword_str);
                emit_app_event(AppEvent::Llm(LlmEvent::Models(models::Event::Response {
                    models: Some(models_arc),
                    // search_kw is ArcStr, which uses Arc::clone under the hood
                    search_keyword: Some(search_kw.clone()),
                })))
                .await;
                debug!(
                    ?search_kw,
                    total = total_models,
                    "model search results enqueued on event bus"
                );
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

fn open_embedding_search(app: &mut App, keyword: &str) {
    let cmd_tx = app.cmd_tx.clone();
    let keyword_str = keyword.to_string();

    tokio::spawn(async move {
        let span = debug_span!("open_embedding_search", keyword = keyword_str.as_str());
        let _guard = span.enter();

        let api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
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
        match <OpenRouter as HasEmbeddingModels>::fetch_embedding_models(
            &client,
            EmbClientConfig::default(),
        )
        .await
        {
            Ok(models_resp) => {
                let total_models = models_resp.data.len();
                let models_arc = Arc::new(models_resp);
                let search_kw = ArcStr::from(keyword_str);
                emit_app_event(AppEvent::Llm(LlmEvent::EmbeddingModels(
                    embedding_models::Event::Response {
                        models: Some(models_arc),
                        search_keyword: Some(search_kw.clone()),
                    },
                )))
                .await;
                debug!(
                    ?search_kw,
                    total = total_models,
                    "embedding model search results enqueued on event bus"
                );
            }
            Err(e) => {
                let _ = cmd_tx
                    .send(StateCommand::AddMessageImmediate {
                        msg: format!("Failed to query OpenRouter embedding models: {}", e),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    })
                    .await;
            }
        }
    });
}

#[instrument(skip(app), level = "debug")]
pub(crate) fn open_context_search(app: &mut App, query_id: u64, search_term: &str) {
    // Open overlay already done by caller; now spawn a non-blocking fetch and emit results.
    let state = app.state.clone();
    let cmd_tx = app.cmd_tx.clone();
    let keyword_str = search_term.to_string();

    tokio::spawn(async move {
        let span = debug_span!(
            "open_context_search",
            keyword = keyword_str.as_str(),
            query_id
        );
        let _guard = span.enter();

        let budget = &state.budget;
        let top_k = crate::TOP_K;
        let retrieval_strategy = &crate::RETRIEVAL_STRATEGY;
        if let Some(rag_service) = &state.rag {
            match rag_service
                .get_context(&keyword_str, top_k, budget, retrieval_strategy)
                .await
            {
                Ok(ctx_returned) => {
                    emit_app_event(AppEvent::ContextSearch(crate::SearchEvent::SearchResults {
                        query_id,
                        context: ctx_returned,
                    }))
                    .await;
                }
                Err(e) => {
                    let _ = cmd_tx
                        .send(StateCommand::AddMessageImmediate {
                            msg: format!("Failed to retrieve code snippets with RAG {}", e),
                            kind: MessageKind::SysInfo,
                            new_msg_id: Uuid::new_v4(),
                        })
                        .await;
                }
            }
        } else {
            let _ = cmd_tx
                .send(StateCommand::AddMessageImmediate {
                    msg: "No RAG service detected in open_context_search".to_string(),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                })
                .await;
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
