use crate::user_config::CommandStyle;
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
