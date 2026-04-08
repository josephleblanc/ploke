use std::path::PathBuf;
use std::{fs, str::FromStr};

use chrono::Local;
use ploke_eval::ploke_eval_home;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init_tracing(debug_tools: bool) -> Option<WorkerGuard> {
    let mut filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,embed-pipeline=trace"));
    if debug_tools
        && let Ok(directive) = tracing_subscriber::filter::Directive::from_str("dbg_tools=debug")
    {
        filter = filter.add_directive(directive);
    }

    let mut log_dir = ploke_eval_home().unwrap_or_else(|_| PathBuf::from(".ploke-eval"));
    log_dir.push("logs");
    if let Err(err) = fs::create_dir_all(&log_dir) {
        eprintln!(
            "failed to create ploke-eval log directory {}: {err}",
            log_dir.display()
        );
        return None;
    }

    let run_id = format!(
        "{}_{}",
        Local::now().format("%Y%m%d_%H%M%S"),
        std::process::id()
    );
    let log_file = log_dir.join(format!("ploke_eval_{run_id}.log"));
    let file_appender =
        tracing_appender::rolling::never(&log_dir, format!("ploke_eval_{run_id}.log"));
    let (non_blocking_file, file_guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .without_time()
        .with_thread_ids(false)
        .with_ansi(false)
        .with_writer(non_blocking_file);

    let console_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .without_time()
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true)
        .with_writer(std::io::stderr);

    if tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(console_layer)
        .try_init()
        .is_ok()
    {
        tracing::info!(
            target: "ploke_eval",
            debug_tools,
            log_file = %log_file.display(),
            "eval tracing initialized"
        );
        Some(file_guard)
    } else {
        None
    }
}
