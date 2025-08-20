use super::parser::Command;
use crate::app::App;
use crate::{app_state::StateCommand, chat_history::MessageKind};
use itertools::Itertools;
use std::path::PathBuf;
use tokio::sync::oneshot;
use uuid::Uuid;
use crate::user_config::{ProviderRegistryStrictness, UserConfig};

const DATA_DIR: &str = "crates/ploke-tui/data";
const TEST_QUERY_FILE: &str = "queries.json";
const TEST_QUERY_RESULTS: &str = "results.json";

/// Execute a parsed command. Falls back to legacy handler for commands
/// not yet migrated to structured parsing.
pub fn execute(app: &mut App, command: Command) {
    match command {
        Command::Help => show_command_help(app),
        Command::ModelList => list_models_async(app),
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

fn show_command_help(app: &App) {
    app.send_cmd(StateCommand::AddMessageImmediate {
        msg: HELP_COMMANDS.to_string(),
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

/// Shared help text for commands
pub const HELP_COMMANDS: &str = r#"Available commands:
    index start [directory] - Run workspace indexing on specified directory
                              (defaults to current dir)
    index pause - Pause indexing
    index resume - Resume indexing
    index cancel - Cancel indexing
    check api - Check API key configuration
    model list - List available models
    model <name> - Switch model
    bm25 rebuild - Rebuild sparse BM25 index
    bm25 status - Show sparse BM25 index status
    bm25 save <path> - Save sparse index sidecar to file
    bm25 load <path> - Load sparse index sidecar from file
    bm25 search <query> [top_k] - Search with BM25
    hybrid <query> [top_k] - Hybrid (BM25 + dense) search
    preview [on|off|toggle] - Toggle context preview panel
    edit preview mode <code|diff> - Set edit preview mode for proposals
    edit preview lines <N> - Set max preview lines per section
    edit auto <on|off> - Toggle auto-approval of staged edits
    edit approve <request_id> - Apply staged code edits with this request ID
    edit deny <request_id> - Deny and discard staged code edits
    help - Show this help

    Keyboard shortcuts (Normal mode):
    q - Quit
    i - Enter insert mode
    : - Enter command mode (vim-style)
    m - Quick model selection
    ? - Show this help
    / - Quick hybrid search prompt
    P - Toggle context preview
    j/↓ - Navigate down (selection)
    k/↑ - Navigate up (selection)
    J - Page down (scroll)
    K - Page up (scroll)
    G - Go to bottom (scroll)
    gg - Go to top (scroll)
    h/← - Navigate branch previous
    l/→ - Navigate branch next
    Ctrl+n - Scroll down one line
    Ctrl+p - Scroll up one line"#;
