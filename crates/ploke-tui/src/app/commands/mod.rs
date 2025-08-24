mod exec;
mod exec_live_tests;
mod exec_real_tools_live_tests;
pub mod parser;

use crate::app::App;

/// Entry point for command handling: parse then execute.
pub fn execute_command(app: &mut App) {
    let style = app.command_style;
    let cmd = app.input_buffer.clone();
    let command = parser::parse(app, &cmd, style);
    exec::execute(app, command);
}

/// Shared help text for commands
#[doc = "User-visible help covering supported commands and keybindings."]
pub const HELP_COMMANDS: &str = r#"Available commands:
    index start [directory] - Run workspace indexing on specified directory
                              (defaults to current dir)
    index pause - Pause indexing
    index resume - Resume indexing
    index cancel - Cancel indexing
    check api - Check API key configuration

    model list - List available models
    model info - Show active model/provider settings
    model use <name> - Switch to a configured model by alias or id
    model refresh [--local] - Refresh model registry (OpenRouter) and API keys; use --local to skip network
    model load [path] - Load configuration from path (default: ~/.config/ploke/config.toml)
    model save [path] [--with-keys] - Save configuration; omit --with-keys to redact secrets
    model search <keyword> - Search OpenRouter models and open interactive browser
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

    Insert mode history:
      ↑/↓ - Navigate your previous user messages in this conversation
      PageUp/PageDown - Jump to oldest/newest user message in history
"#;
