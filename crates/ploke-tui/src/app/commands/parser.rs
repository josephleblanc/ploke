use crate::user_config::CommandStyle;
use crate::app_state::core::PreviewMode;
use uuid::Uuid;

/// Parsed command variants handled by the executor.
/// Phase 3 wires a subset as examples and falls back to Raw for others.
#[derive(Debug, Clone)]
pub enum Command {
    Help,
    ModelList,
    Update,
    EditApprove(Uuid),
    EditDeny(Uuid),
    EditSetPreviewMode(PreviewMode),
    EditSetPreviewLines(usize),
    EditSetAutoConfirm(bool),
    Raw(String),
}

/// Parse the input buffer into a Command, stripping the style prefix.
pub fn parse(input: &str, style: CommandStyle) -> Command {
    let trimmed = match style {
        CommandStyle::NeoVim => input.trim_start_matches(':').trim(),
        CommandStyle::Slash => input.trim_start_matches('/').trim(),
    };

    match trimmed {
        "help" => Command::Help,
        "model list" => Command::ModelList,
        "update" => Command::Update,
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
        other => Command::Raw(other.to_string()),
    }
}
