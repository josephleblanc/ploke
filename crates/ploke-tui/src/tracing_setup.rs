use chrono::Local;
use fmt::format::FmtSpan;
use ploke_test_utils::workspace_root;

use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;

use tracing_subscriber::filter;
use tracing_subscriber::filter::FilterExt;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub const SCAN_CHANGE: &str = "scan_change";
/// A tracing target for items related to tracking messages in the conversation history, present
/// in:
///     - app_state/handlers/chat.rs
pub const CHAT_TARGET: &str = "chat_tracing_target";
/// Dedicated target for low-level message update lifecycle traces
pub const MESSAGE_UPDATE_TARGET: &str = "message-update";

pub struct LoggingGuards {
    /// Guard for the main app log
    pub main: WorkerGuard,
    /// Guard for the API-only log
    pub api: WorkerGuard,
    /// Guard for the chat-only log
    pub chat: WorkerGuard,
    /// Guard for the message-update-only log
    pub message_update: WorkerGuard,
}

pub fn init_tracing() -> LoggingGuards {
    // -------- Main app log (unchanged from your version) --------
    // Default to conservative levels to keep noise down; users can override with RUST_LOG.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("debug_dup=error,read_file=error"));

    let mut log_dir = workspace_root();
    log_dir.push("crates/ploke-tui/logs");
    std::fs::create_dir_all(&log_dir).expect("Failed to create logs directory");

    let level = Level::INFO;
    let targets = filter::Targets::new()
        .with_target("ploke", level)
        .with_target("ploke_tui", level)
        .with_target("ploke_db", level)
        .with_target("ploke_embed", level)
        .with_target("ploke_io", level)
        .with_target("ploke_transform", level)
        .with_target("ploke_rag", level)
        .with_target("chat-loop", Level::TRACE)
        .with_target(CHAT_TARGET, Level::TRACE)
        .with_target(MESSAGE_UPDATE_TARGET, Level::TRACE)
        .with_target("api_json", Level::TRACE)
        .with_target("cozo", Level::ERROR)
        .with_default(LevelFilter::WARN);

    // Use a per-run log file so each TUI session is isolated.
    let run_id = format!(
        "{}_{}",
        Local::now().format("%Y%m%d_%H%M%S"),
        std::process::id()
    );
    let file_appender = tracing_appender::rolling::never(&log_dir, format!("ploke_{run_id}.log"));
    let (non_blocking_file, main_guard) = tracing_appender::non_blocking(file_appender);

    let common_fmt = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_target(true)
        .without_time()
        .with_thread_ids(false)
        // .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false);

    let main_layer = common_fmt.with_writer(non_blocking_file);

    // -------- API-only pretty JSON log --------
    // Separate per-run file just for API responses
    let api_appender =
        tracing_appender::rolling::never(&log_dir, format!("api_responses_{run_id}.log"));
    let (api_non_blocking, api_guard) = tracing_appender::non_blocking(api_appender);

    // A super-minimal formatter so the file contains only your pretty JSON (and a trailing newline)
    let api_layer = fmt::layer()
        .with_writer(api_non_blocking)
        .with_ansi(false)
        .with_level(false)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .without_time();

    // Filter so this layer only receives events you tag with target="api_json"
    let only_api_json = filter::Targets::new().with_target("api_json", Level::TRACE);

    // -------- Chat-only log (conversational context, separate from API payloads) --------
    let chat_appender = tracing_appender::rolling::never(&log_dir, format!("chat_{run_id}.log"));
    let (chat_non_blocking, chat_guard) = tracing_appender::non_blocking(chat_appender);
    let chat_layer = fmt::layer()
        .with_writer(chat_non_blocking)
        .with_ansi(false)
        .with_level(true)
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .without_time();
    let only_chat= filter::Targets::new().with_target(CHAT_TARGET, Level::TRACE);

    // -------- Message update log (focus on message lifecycle updates) --------
    let message_update_appender =
        tracing_appender::rolling::never(&log_dir, format!("message_update_{run_id}.log"));
    let (message_update_non_blocking, message_update_guard) =
        tracing_appender::non_blocking(message_update_appender);
    let message_update_layer = fmt::layer()
        .with_writer(message_update_non_blocking)
        .with_ansi(false)
        .with_level(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true);
    let only_message_updates =
        filter::Targets::new().with_target(MESSAGE_UPDATE_TARGET, Level::TRACE);

    // Install both layers on the global registry
    let _ = tracing_subscriber::registry()
        // .with(filter) // env filter for the main layer
        .with(targets)
        .with(main_layer) // normal app logs -> ploke.log
        .with(api_layer.with_filter(only_api_json)) // api_json events -> api_responses.log
        .with(chat_layer.with_filter(only_chat)) // chat events -> chat_*.log
        .with(message_update_layer.with_filter(only_message_updates)) // message update events -> message_update_*.log
        .try_init();

    LoggingGuards {
        main: main_guard,
        api: api_guard,
        chat: chat_guard,
        message_update: message_update_guard,
    }
}

pub fn init_tracing_tests(level: Level) -> WorkerGuard {
    let env_filter = format!(
        "{},ploke_db=error,cozo=error,tokenizer=error,candle_transformers=error,hyper_util=error",
        level
    );
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&env_filter)); // Default to 'info' level

    // File appender with custom timestamp format
    let log_dir = "test-logs";
    std::fs::create_dir_all(log_dir).expect("Failed to create logs directory");
    let file_appender = tracing_appender::rolling::hourly(log_dir, "ploke.log");
    let (non_blocking_file, file_guard) = tracing_appender::non_blocking(file_appender);

    // Common log format builder
    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .with_span_events(FmtSpan::CLOSE); // Capture span durations

    let file_subscriber = fmt_layer
        .with_writer(non_blocking_file)
        .pretty()
        .with_ansi(false);

    // Also log to stderr so test failures print captured diagnostics without requiring manual file inspection.
    let console_subscriber = fmt::layer()
        .with_target(true)
        .with_level(true)
        .without_time()
        .with_line_number(true)
        .with_thread_ids(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(true);

    // Use try_init to avoid panicking if a global subscriber is already set (e.g., across tests)
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_subscriber)
        .with(console_subscriber.with_writer(std::io::stderr))
        .try_init();

    file_guard
}
