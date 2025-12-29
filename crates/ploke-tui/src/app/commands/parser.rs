#![allow(missing_docs)]
//! Command parser for TUI input.
//!
//! Dataflow:
//! - Raw input is normalized by command style (Slash/NeoVim) and mapped to a
//!   structured `Command` variant.
//! - The executor consumes these variants and dispatches `StateCommand`s,
//!   keeping the UI thread non-blocking.

// TODO: Add defaults
// - `/model providers` to use currently selected model by default
// - `/model providers <model_id>` should also work for aliases
use crate::app::App;
use crate::app_state::core::PreviewMode;
use crate::tools::ToolVerbosity;
use crate::user_config::{CommandStyle, ModelRegistryStrictness};
use uuid::Uuid;

/// Parsed command variants handled by the executor.
/// Phase 3 wires a subset as examples and falls back to Raw for others.
#[derive(Debug, Clone)]
/// High-level parsed command variants handled by the executor.
pub enum Command {
    Help,
    HelpTopic(String),
    ModelList,
    ModelInfo,
    ModelUse(String),
    ModelRefresh {
        remote: bool,
    },
    ModelLoad(Option<String>),
    ModelSave {
        path: Option<String>,
        with_keys: bool,
    },
    ModelSearch(String),
    ModelSearchHelp,
    EmbeddingSearch(String),
    EmbeddingSearchHelp,
    ModelProviders(String),
    ProviderStrictness(ModelRegistryStrictness),
    ProviderToolsOnly(bool),
    ProviderSelect {
        model_id: String,
        provider_slug: String,
    },
    Update,
    EditApprove(Uuid),
    EditDeny(Uuid),
    CreateApprove(Uuid),
    CreateDeny(Uuid),
    EditSetPreviewMode(PreviewMode),
    EditSetPreviewLines(usize),
    EditSetAutoConfirm(bool),
    ToolVerbositySet(ToolVerbosity),
    ToolVerbosityToggle,
    ToolVerbosityShow,
    CopySelection,
    Raw(String),
    SearchContext(String),
}

/// Parse the input buffer into a Command, stripping the style prefix.
pub fn parse(app: &App, input: &str, style: CommandStyle) -> Command {
    let trimmed = match style {
        CommandStyle::NeoVim => input.trim_start_matches(':').trim(),
        CommandStyle::Slash => input.trim_start_matches('/').trim(),
    };

    match trimmed {
        "help" => Command::Help,
        s if s.starts_with("help ") => {
            let topic = s.trim_start_matches("help ").trim().to_string();
            if topic.is_empty() {
                Command::Help
            } else {
                Command::HelpTopic(topic)
            }
        }
        "model list" => Command::ModelList,
        "model info" => Command::ModelInfo,
        s if s.starts_with("model use ") => {
            let alias = s.trim_start_matches("model use ").trim().to_string();
            if alias.is_empty() {
                Command::Raw(trimmed.to_string())
            } else {
                Command::ModelUse(alias)
            }
        }
        s if s.starts_with("model refresh") => {
            // Default to remote refresh unless explicitly disabled by flag
            let remote = !s.contains("--local");
            Command::ModelRefresh { remote }
        }
        s if s.starts_with("model load") => {
            let rest = s.trim_start_matches("model load").trim();
            let path = if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            };
            Command::ModelLoad(path)
        }
        s if s.starts_with("model save") => {
            let rest = s.trim_start_matches("model save").trim();
            let mut path: Option<String> = None;
            let mut with_keys = false;
            if !rest.is_empty() {
                for token in rest.split_whitespace() {
                    if token == "--with-keys" {
                        with_keys = true;
                    } else if path.is_none() {
                        path = Some(token.to_string());
                    }
                }
            }
            Command::ModelSave { path, with_keys }
        }
        "model search" => Command::ModelSearchHelp,
        s if s.starts_with("model search") => {
            let kw = s.trim_start_matches("model search").trim().to_string();
            if kw.is_empty() {
                Command::ModelSearchHelp
            } else {
                Command::ModelSearch(kw)
            }
        }
        "embedding search" => Command::EmbeddingSearchHelp,
        s if s.starts_with("embedding search") => {
            let kw = s.trim_start_matches("embedding search").trim().to_string();
            if kw.is_empty() {
                Command::EmbeddingSearchHelp
            } else {
                Command::EmbeddingSearch(kw)
            }
        }
        s if s.starts_with("model providers") => {
            let id = s.trim_start_matches("model providers ").trim().to_string();
            if id.is_empty() {
                Command::ModelProviders(app.active_model_id.clone())
            } else {
                Command::ModelProviders(id)
            }
        }
        s if s.starts_with("provider strictness ") => {
            let mode = s
                .trim_start_matches("provider strictness ")
                .trim()
                .to_lowercase();
            let strictness = match mode.as_str() {
                "openrouter-only" | "openrouter_only" | "openrouteronly" => {
                    ModelRegistryStrictness::OpenRouterOnly
                }
                "allow-custom" | "allow_custom" | "allowcustom" => {
                    ModelRegistryStrictness::AllowCustom
                }
                "allow-any" | "allow_any" | "allowany" => ModelRegistryStrictness::AllowAny,
                _ => return Command::Raw(trimmed.to_string()),
            };
            Command::ProviderStrictness(strictness)
        }
        s if s.starts_with("provider tools-only ") => {
            let t = s
                .trim_start_matches("provider tools-only ")
                .trim()
                .to_lowercase();
            let enabled = matches!(t.as_str(), "on" | "true" | "1" | "enabled" | "enable");
            Command::ProviderToolsOnly(enabled)
        }
        s if s.starts_with("provider select ") => {
            // provider select <model_id> <provider_slug>
            let rest = s.trim_start_matches("provider select ").trim();
            let mut parts = rest.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some(provider_slug), None) => Command::ProviderSelect {
                    model_id: app.active_model_id.clone(),
                    provider_slug: provider_slug.to_string(),
                },
                (Some(model_id), Some(provider_slug)) => Command::ProviderSelect {
                    model_id: model_id.to_string(),
                    provider_slug: provider_slug.to_string(),
                },
                _ => Command::Raw(trimmed.to_string()),
            }
        }
        s if s.starts_with("provider pin ") => {
            // provider pin <model_id> <provider_slug> (alias of provider select)
            let rest = s.trim_start_matches("provider pin ").trim();
            let mut parts = rest.split_whitespace();
            if let (Some(model_id), Some(provider_slug)) = (parts.next(), parts.next()) {
                Command::ProviderSelect {
                    model_id: model_id.to_string(),
                    provider_slug: provider_slug.to_string(),
                }
            } else {
                Command::Raw(trimmed.to_string())
            }
        }
        "update" => Command::Update,
        "copy" => Command::CopySelection,
        s if s.starts_with("edit preview mode ") => {
            let m = s
                .trim_start_matches("edit preview mode ")
                .trim()
                .to_lowercase();
            let mode = match m.as_str() {
                "diff" => PreviewMode::Diff,
                "code" | "codeblock" | "code-block" => PreviewMode::CodeBlock,
                _ => return Command::Raw(trimmed.to_string()),
            };
            Command::EditSetPreviewMode(mode)
        }
        s if s.starts_with("edit preview lines ") => {
            let n_str = s.trim_start_matches("edit preview lines ").trim();
            match n_str.parse::<usize>() {
                Ok(n) => Command::EditSetPreviewLines(n),
                Err(_) => Command::Raw(trimmed.to_string()),
            }
        }
        s if s.starts_with("edit auto ") => {
            let t = s.trim_start_matches("edit auto ").trim().to_lowercase();
            if matches!(t.as_str(), "on" | "true" | "1" | "enabled" | "enable") {
                Command::EditSetAutoConfirm(true)
            } else if matches!(t.as_str(), "off" | "false" | "0" | "disabled" | "disable") {
                Command::EditSetAutoConfirm(false)
            } else {
                Command::Raw(trimmed.to_string())
            }
        }
        "tool verbosity" => Command::ToolVerbosityShow,
        s if s.starts_with("tool verbosity ") => {
            let t = s
                .trim_start_matches("tool verbosity ")
                .trim()
                .to_lowercase();
            match t.as_str() {
                "minimal" => Command::ToolVerbositySet(ToolVerbosity::Minimal),
                "normal" => Command::ToolVerbositySet(ToolVerbosity::Normal),
                "verbose" => Command::ToolVerbositySet(ToolVerbosity::Verbose),
                "toggle" => Command::ToolVerbosityToggle,
                _ => Command::Raw(trimmed.to_string()),
            }
        }
        s if s.starts_with("edit approve ") => {
            let id_str = s.trim_start_matches("edit approve ").trim();
            match Uuid::parse_str(id_str) {
                Ok(id) => Command::EditApprove(id),
                Err(_) => Command::Raw(trimmed.to_string()),
            }
        }
        s if s.starts_with("edit deny ") => {
            let id_str = s.trim_start_matches("edit deny ").trim();
            match Uuid::parse_str(id_str) {
                Ok(id) => Command::EditDeny(id),
                Err(_) => Command::Raw(trimmed.to_string()),
            }
        }
        s if s.starts_with("create approve ") => {
            let id_str = s.trim_start_matches("create approve ").trim();
            match Uuid::parse_str(id_str) {
                Ok(id) => Command::CreateApprove(id),
                Err(_) => Command::Raw(trimmed.to_string()),
            }
        }
        s if s.starts_with("create deny ") => {
            let id_str = s.trim_start_matches("create deny ").trim();
            match Uuid::parse_str(id_str) {
                Ok(id) => Command::CreateDeny(id),
                Err(_) => Command::Raw(trimmed.to_string()),
            }
        }
        s if s.starts_with("search ") => {
            let search_term = s.trim_start_matches("search ").trim();
            Command::SearchContext(search_term.to_string())
        }
        other => Command::Raw(other.to_string()),
    }
}
