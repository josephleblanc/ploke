use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use fmt::format::FmtSpan;

pub fn init_tracing() -> WorkerGuard {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,ploke_db=trace,cozo=error,tokenizer=error"));  // Default to 'info' level
    
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
        .with_span_events(FmtSpan::CLOSE);  // Capture span durations
    
    let file_subscriber = fmt_layer
        .with_writer(non_blocking_file)
        .pretty()
        .with_ansi(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(file_subscriber)
        .init();

    file_guard
}
