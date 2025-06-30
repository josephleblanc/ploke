use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use fmt::format::FmtSpan;
use std::io;

pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));  // Default to 'info' level
    
    // File appender with custom timestamp format
    let log_dir = "logs";
    std::fs::create_dir_all(log_dir).expect("Failed to create logs directory");
    let file_appender = tracing_appender::rolling::daily(log_dir, "ploke.log");
    let (non_blocking_file, _file_guard) = tracing_appender::non_blocking(file_appender);
    
    // Common log format builder
    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .with_span_events(FmtSpan::CLOSE);  // Capture span durations
    
    // Only use ANSI formatting in TTY environment
    let is_tty = io::stdout().is_terminal();
    let stdout_subscriber = fmt_layer
        .clone()
        .with_writer(io::stdout)
        .with_ansi(is_tty);

    let file_subscriber = fmt_layer
        .with_writer(non_blocking_file)
        .with_ansi(false)
        .json();  // Structured JSON logging for files

    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_subscriber)
        .with(file_subscriber)
        .init();
}
