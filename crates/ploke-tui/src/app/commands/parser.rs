use crate::user_config::CommandStyle;

/// Parsed command variants handled by the executor.
/// Phase 3 wires a subset as examples and falls back to Raw for others.
#[derive(Debug, Clone)]
pub enum Command {
    Help,
    ModelList,
    Update,
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
        other => Command::Raw(other.to_string()),
    }
}
