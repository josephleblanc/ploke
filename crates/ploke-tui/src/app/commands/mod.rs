pub(crate) mod exec;
#[cfg(feature = "live_api_tests")]
mod exec_real_tools_live_tests;
pub mod parser;

use crate::app::App;

// /// Entry point for command handling: parse then execute.
// pub fn execute_command(app: &mut App) {
//     let style = app.command_style;
//     let cmd = app.input_buffer.clone();
//     let command = parser::parse(app, &cmd, style);
//     exec::execute(app, command);
// }

/// Shared help text for commands
#[doc = "User-visible help covering supported commands and keybindings."]
pub const HELP_COMMANDS: &str = r#"Available commands:
    index start [directory] - Run workspace indexing on specified directory
                              (defaults to current dir)
    index pause - Pause indexing
    index resume - Resume indexing
    index cancel - Cancel indexing
    check api - Check API key configuration
    copy - Copy selected conversation message to clipboard

    model list - List available models
    model info - Show active model/provider settings
    model use <name> - Switch to a configured model by alias or id
    model refresh [--local] - Refresh model registry (OpenRouter) and API keys; use --local to skip network
    model load [path] - Load configuration from path (default: ~/.config/ploke/config.toml)
    model save [path] [--with-keys] - Save configuration; omit --with-keys to redact secrets
    model search <keyword> - Search OpenRouter models and open interactive browser
    embedding search <keyword> - Search OpenRouter embedding models and open interactive browser
    model providers <model_id> - List provider endpoints for a model and show tool support and slugs
    provider strictness <openrouter-only|allow-custom|allow-any> - Restrict selectable providers
    provider tools-only <on|off> - Enforce using only models/providers that support tool calls
    provider select <model_id> <provider_slug> - Pin a model to a specific provider endpoint
    provider pin <model_id> <provider_slug> - Alias for 'provider select'

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
    tool verbosity <minimal|normal|verbose|toggle> - Set or cycle tool output verbosity

    help - Show this help
    help <topic> - Topic-specific help, e.g. 'help model', 'help edit', 'help bm25', 'help provider', 'help index'

    Keyboard shortcuts (Normal mode):
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

#[derive(Debug, Clone, Copy)]
pub struct CommandEntry {
    pub command: &'static str,
    pub completion: &'static str,
    pub description: &'static str,
}

pub const COMMAND_ENTRIES: &[CommandEntry] = &[
    CommandEntry {
        command: "index start",
        completion: "index start [directory]",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "index pause",
        completion: "index pause",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "index resume",
        completion: "index resume",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "index cancel",
        completion: "index cancel",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "check api",
        completion: "check api",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "copy",
        completion: "copy",
        description: "Copy the selected conversation message to clipboard",
    },
    CommandEntry {
        command: "model list",
        completion: "model list",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "model info",
        completion: "model info",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "model use",
        completion: "model use <name>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "model refresh",
        completion: "model refresh [--local]",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "model load",
        completion: "model load [path]",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "model save",
        completion: "model save [path] [--with-keys]",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "model search",
        completion: "model search <model-name>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "embedding search",
        completion: "embedding search <embedding-model>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "model providers",
        completion: "model providers <model_id>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "provider strictness",
        completion: "provider strictness <openrouter-only|allow-custom|allow-any>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "provider tools-only",
        completion: "provider tools-only <on|off>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "provider select",
        completion: "provider select <model_id> <provider_slug>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "provider pin",
        completion: "provider pin <model_id> <provider_slug>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "bm25 rebuild",
        completion: "bm25 rebuild",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "bm25 status",
        completion: "bm25 status",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "bm25 save",
        completion: "bm25 save <path>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "bm25 load",
        completion: "bm25 load <path>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "bm25 search",
        completion: "bm25 search <query> [top_k]",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "hybrid",
        completion: "hybrid <query> [top_k]",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "preview",
        completion: "preview [on|off|toggle]",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "edit preview mode",
        completion: "edit preview mode <code|diff>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "edit preview lines",
        completion: "edit preview lines <N>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "edit auto",
        completion: "edit auto <on|off>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "edit approve",
        completion: "edit approve <request_id>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "edit deny",
        completion: "edit deny <request_id>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "create approve",
        completion: "create approve <request_id>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "create deny",
        completion: "create deny <request_id>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "tool verbosity",
        completion: "tool verbosity <minimal|normal|verbose|toggle>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "help",
        completion: "help [topic]",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "search",
        completion: "search <query>",
        description: "TODO: add description",
    },
    CommandEntry {
        command: "update",
        completion: "update",
        description: "TODO: add description",
    },
];
