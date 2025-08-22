use fmt::format::FmtSpan;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init_tracing() -> WorkerGuard {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,ploke_db=error,cozo=error,tokenizer=error")); // Default to 'info' level

    // File appender with custom timestamp format
    let log_dir = "logs";
    std::fs::create_dir_all(log_dir).expect("Failed to create logs directory");
    let file_appender = tracing_appender::rolling::daily(log_dir, "ploke.log");
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
