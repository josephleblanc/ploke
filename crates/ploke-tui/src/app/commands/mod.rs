pub(crate) mod exec;
#[cfg(feature = "live_api_tests")]
mod exec_real_tools_live_tests;
pub mod parser;
#[cfg(test)]
mod unit_tests;

use crate::app::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandTopic {
    General,
    Indexing,
    Workspace,
    Model,
    Provider,
    Bm25,
    Editing,
    Create,
}

impl CommandTopic {
    fn heading(self) -> &'static str {
        match self {
            CommandTopic::General => "General commands",
            CommandTopic::Indexing => "Indexing commands",
            CommandTopic::Workspace => "Workspace commands",
            CommandTopic::Model => "Model commands",
            CommandTopic::Provider => "Provider commands",
            CommandTopic::Bm25 => "BM25 commands",
            CommandTopic::Editing => "Editing commands",
            CommandTopic::Create => "Create commands",
        }
    }

    fn from_topic_prefix(prefix: &str) -> Option<Self> {
        let p = prefix.trim().to_lowercase();
        if p.starts_with("index") {
            Some(CommandTopic::Indexing)
        } else if p.starts_with("load") || p.starts_with("save") || p.starts_with("workspace") {
            Some(CommandTopic::Workspace)
        } else if p.starts_with("model") || p.starts_with("embedding") {
            Some(CommandTopic::Model)
        } else if p.starts_with("provider") {
            Some(CommandTopic::Provider)
        } else if p.starts_with("bm25") || p.starts_with("hybrid") {
            Some(CommandTopic::Bm25)
        } else if p.starts_with("edit")
            || p.starts_with("preview")
            || p.starts_with("tool verbosity")
            || p.starts_with("verbosity profile")
        {
            Some(CommandTopic::Editing)
        } else if p.starts_with("create") {
            Some(CommandTopic::Create)
        } else if p.starts_with("check")
            || p.starts_with("copy")
            || p.starts_with("query")
            || p.starts_with("batch")
            || p.starts_with("search")
            || p.starts_with("context")
            || p.starts_with("quit")
            || p.starts_with("help")
        {
            Some(CommandTopic::General)
        } else {
            None
        }
    }
}

/// Shared help text for non-command sections.
const HELP_TAIL: &str = r#"    Keyboard shortcuts (Normal mode):
    q - Quit
    i - Enter insert mode
    : - Enter command mode (vim-style)
    m - Quick model selection
    ? - Show this help
    / - Quick hybrid search prompt
    P - Toggle context preview
    v - Cycle tool verbosity (minimal -> normal -> verbose)
    y - Copy selected message to clipboard
    j/↓ - Navigate down (selection)
    k/↑ - Navigate up (selection)
    J - Page down (scroll)
    K - Page up (scroll)
    G - Go to bottom (scroll)
    gg - Go to top (scroll)
    h/← - Navigate branch previous
    l/→ - Navigate branch next
    Del - Delete selected conversation item
    Ctrl+n - Scroll down one line
    Ctrl+p - Scroll up one line

    Model Browser (opened via 'model search <keyword>'):
      ↑/↓ or j/k - Navigate
      Enter/Space - Expand/collapse details
      s - Select and set active model
      q/Esc - Close

    Embedding Browser (opened via 'embedding search <keyword>'):
      ↑/↓ or j/k - Navigate
      Enter/Space - Expand/collapse details
      s - Select embedding model (records selection in UI log)
      q/Esc - Close

    Insert mode history:
      ↑/↓ - Navigate your previous user messages in this conversation
    PageUp/PageDown - Jump to oldest/newest user message in history
"#;

fn render_command_group(topic: CommandTopic) -> Option<String> {
    let entries: Vec<&CommandEntry> = COMMAND_ENTRIES
        .iter()
        .filter(|entry| entry.topic == topic)
        .collect();
    if entries.is_empty() {
        return None;
    }

    let mut lines = vec![format!("{}:", topic.heading())];
    for entry in entries {
        lines.push(format!(
            "  {} - {}",
            escape_help_markup(entry.completion),
            escape_help_markup(entry.description)
        ));
    }
    Some(lines.join("\n"))
}

fn escape_help_markup(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn help_commands_markdown() -> String {
    let mut sections = vec!["Available commands:".to_string()];
    for topic in [
        CommandTopic::Indexing,
        CommandTopic::Workspace,
        CommandTopic::General,
        CommandTopic::Model,
        CommandTopic::Provider,
        CommandTopic::Bm25,
        CommandTopic::Editing,
        CommandTopic::Create,
    ] {
        if let Some(section) = render_command_group(topic) {
            sections.push(section);
        }
    }
    let help_tail = HELP_TAIL
        .lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    sections.push(help_tail);
    sections.join("\n\n")
}

pub fn help_topic_markdown(topic_prefix: &str) -> Option<String> {
    let topic = CommandTopic::from_topic_prefix(topic_prefix)?;
    render_command_group(topic).map(|section| section)
}

#[derive(Debug, Clone, Copy)]
pub struct CommandEntry {
    pub topic: CommandTopic,
    pub command: &'static str,
    pub completion: &'static str,
    pub description: &'static str,
}

macro_rules! command_entry {
    ($topic:ident, $command:expr, $completion:expr, $description:expr) => {
        CommandEntry {
            topic: CommandTopic::$topic,
            command: $command,
            completion: $completion,
            description: $description,
        }
    };
}

pub const COMMAND_ENTRIES: &[CommandEntry] = &[
    command_entry!(
        Indexing,
        "index start",
        "index start [path]",
        "Run indexing for the most specific Cargo target; use the crate if the path is a crate root, otherwise the nearest ancestor workspace if one is found"
    ),
    command_entry!(Indexing, "index pause", "index pause", "Pause indexing"),
    command_entry!(Indexing, "index resume", "index resume", "Resume indexing"),
    command_entry!(Indexing, "index cancel", "index cancel", "Cancel indexing"),
    command_entry!(
        General,
        "check api",
        "check api",
        "Check API key configuration"
    ),
    command_entry!(
        General,
        "copy",
        "copy",
        "Copy the selected conversation message to clipboard"
    ),
    command_entry!(Model, "model list", "model list", "List available models"),
    command_entry!(
        Model,
        "model info",
        "model info",
        "Show active model and provider settings"
    ),
    command_entry!(
        Model,
        "model use",
        "model use <name>",
        "Switch to a configured model by alias or id"
    ),
    command_entry!(
        Model,
        "model refresh",
        "model refresh [--local]",
        "Refresh the model registry and API keys; use --local to skip network"
    ),
    command_entry!(
        Model,
        "model load",
        "model load [path]",
        "Load configuration from a path (default: ~/.config/ploke/config.toml)"
    ),
    command_entry!(
        Model,
        "model save",
        "model save [path] [--with-keys]",
        "Save configuration; omit --with-keys to redact secrets"
    ),
    command_entry!(
        Model,
        "model search",
        "model search <model-name>",
        "Search OpenRouter models and open the interactive browser"
    ),
    command_entry!(
        Model,
        "embedding search",
        "embedding search <embedding-model>",
        "Search OpenRouter embedding models and open the interactive browser"
    ),
    command_entry!(
        Model,
        "model providers",
        "model providers <model_id>",
        "List provider endpoints for a model and show tool support and slugs"
    ),
    command_entry!(
        Provider,
        "provider strictness",
        "provider strictness <openrouter-only|allow-custom|allow-any>",
        "Restrict selectable providers"
    ),
    command_entry!(
        Provider,
        "provider tools-only",
        "provider tools-only <on|off>",
        "Enforce using only models and providers that support tool calls"
    ),
    command_entry!(
        Provider,
        "provider select",
        "provider select <model_id> <provider_slug>",
        "Pin a model to a specific provider endpoint"
    ),
    command_entry!(
        Provider,
        "provider pin",
        "provider pin <model_id> <provider_slug>",
        "Alias for provider select"
    ),
    command_entry!(
        Bm25,
        "bm25 rebuild",
        "bm25 rebuild",
        "Rebuild the sparse BM25 index"
    ),
    command_entry!(
        Bm25,
        "bm25 status",
        "bm25 status",
        "Show sparse BM25 index status"
    ),
    command_entry!(
        Bm25,
        "bm25 save",
        "bm25 save <path>",
        "Save the sparse index sidecar to a file"
    ),
    command_entry!(
        Bm25,
        "bm25 load",
        "bm25 load <path>",
        "Load the sparse index sidecar from a file"
    ),
    command_entry!(
        Bm25,
        "bm25 search",
        "bm25 search <query> [top_k]",
        "Search with BM25"
    ),
    command_entry!(
        Bm25,
        "hybrid",
        "hybrid <query> [top_k]",
        "Hybrid search (BM25 + dense)"
    ),
    command_entry!(
        Editing,
        "preview",
        "preview [on|off|toggle]",
        "Toggle the context preview panel"
    ),
    command_entry!(
        Editing,
        "edit preview mode",
        "edit preview mode <code|diff>",
        "Set the edit preview mode for proposals"
    ),
    command_entry!(
        Editing,
        "edit preview lines",
        "edit preview lines <N>",
        "Set the maximum preview lines per section"
    ),
    command_entry!(
        Editing,
        "edit auto",
        "edit auto <on|off>",
        "Toggle auto-approval of staged edits"
    ),
    command_entry!(
        Editing,
        "edit approve",
        "edit approve <request_id>",
        "Apply staged code edits with this request ID"
    ),
    command_entry!(
        Editing,
        "edit deny",
        "edit deny <request_id>",
        "Deny and discard staged code edits"
    ),
    command_entry!(
        Create,
        "create approve",
        "create approve <request_id>",
        "Apply staged file creations with this request ID"
    ),
    command_entry!(
        Create,
        "create deny",
        "create deny <request_id>",
        "Deny and discard staged file creations"
    ),
    command_entry!(
        Editing,
        "tool verbosity",
        "tool verbosity <minimal|normal|verbose|toggle>",
        "Set or cycle tool output verbosity"
    ),
    command_entry!(
        Editing,
        "verbosity profile",
        "verbosity profile <minimal|normal|verbose|custom>",
        "Set the conversation message verbosity profile"
    ),
    command_entry!(General, "quit", "quit", "Quit the application"),
    command_entry!(General, "help", "help [topic]", "Show this help"),
    command_entry!(
        General,
        "search",
        "search <query>",
        "Search indexed code context and open the context browser"
    ),
    command_entry!(
        Workspace,
        "load workspace",
        "load workspace <name-or-id>",
        "Restore a saved workspace snapshot and replace the current in-memory state"
    ),
    command_entry!(
        Workspace,
        "load crate",
        "load crate <name-or-id>",
        "Restore a saved standalone/workspace snapshot and replace the current in-memory state"
    ),
    command_entry!(
        Workspace,
        "load crates",
        "load crates <workspace-name-or-id> <crate-name-or-exact-root>",
        "Add one crate from a saved workspace snapshot into the current loaded workspace"
    ),
    command_entry!(
        Workspace,
        "save db",
        "save db | sd",
        "Save the active workspace snapshot and registry entry"
    ),
    command_entry!(
        Workspace,
        "workspace rm",
        "workspace rm <crate-name-or-exact-root>",
        "Remove one loaded crate namespace from the current workspace"
    ),
    command_entry!(
        General,
        "query load",
        "query load | ql",
        "Load the default query from default.dl"
    ),
    command_entry!(
        General,
        "query load",
        "query load <query_name> <file_name>",
        "Load a named query from a file"
    ),
    command_entry!(
        General,
        "batch",
        "batch [prompt_file] [out_file] [max_hits] [threshold]",
        "Run batch prompt search"
    ),
    command_entry!(
        General,
        "context plan",
        "context plan | contextplan",
        "Open the context plan overlay"
    ),
    command_entry!(
        General,
        "update",
        "update",
        "Refresh the loaded graph against current source state"
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_commands_includes_registry_sections_and_footer() {
        let help = help_commands_markdown();
        assert!(help.contains("Indexing commands:"));
        assert!(help.contains("Workspace commands:"));
        assert!(help.contains("Keyboard shortcuts (Normal mode):"));
        assert!(help.contains("Model Browser (opened via 'model search <keyword>'):"));
    }

    #[test]
    fn topic_help_is_generated_from_registry() {
        let index_help = help_topic_markdown("index").expect("index help");
        assert!(index_help.contains("index pause"));
        assert!(!index_help.contains("model list"));

        let create_help = help_topic_markdown("create").expect("create help");
        assert!(create_help.contains("create approve"));
        assert!(!create_help.contains("edit approve"));
    }
}
