use ploke_test_utils::workspace_root;
use fmt::format::FmtSpan;

use tracing::Level;
use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;

use tracing_subscriber::filter::FilterExt;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::filter;

pub struct LoggingGuards {
    /// Guard for the main app log
    pub main: WorkerGuard,
    /// Guard for the API-only log
    pub api: WorkerGuard,
}

pub fn init_tracing() -> LoggingGuards {
    // -------- Main app log (unchanged from your version) --------
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("dbg_tools=debug,debug,ploke_db=error,cozo=error,tokenizer=error"));

    let mut log_dir = workspace_root();
    log_dir.push("crates/ploke-tui/logs");
    std::fs::create_dir_all(&log_dir).expect("Failed to create logs directory");

    let file_appender = tracing_appender::rolling::daily(&log_dir, "ploke.log");
    let (non_blocking_file, main_guard) = tracing_appender::non_blocking(file_appender);

    let common_fmt = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false);

    let main_layer = common_fmt.with_writer(non_blocking_file);

    // -------- API-only pretty JSON log --------
    // Separate rolling file just for API responses
    let api_appender = tracing_appender::rolling::daily(log_dir, "api_responses.log");
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
    let only_api_json = filter::filter_fn(|meta| meta.target() == "api_json")
        .and(LevelFilter::INFO); // optional: cap level if you want

    // Install both layers on the global registry
    let _ = tracing_subscriber::registry()
        .with(filter)                // env filter for the main layer
        .with(main_layer)            // normal app logs -> ploke.log
        .with(api_layer.with_filter(only_api_json)) // api_json events -> api_responses.log
        .try_init();

    LoggingGuards { main: main_guard, api: api_guard }
}

pub fn init_tracing_tests(level: Level) -> WorkerGuard {
    let env_filter = format!("{},ploke_db=error,cozo=error,tokenizer=error,candle_transformers=error,hyper_util=error", level);
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&env_filter)); // Default to 'info' level

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
