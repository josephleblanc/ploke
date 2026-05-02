use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{env, fs, str::FromStr};

use chrono::Local;
use ploke_core::EXECUTION_DEBUG_TARGET;
use ploke_tui::tracing_setup::FULL_RESPONSE_TARGET;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::layout::ploke_eval_home;

const CHAT_HTTP_TARGET: &str = "chat_http";

#[allow(dead_code)]
pub struct LoggingGuards {
    pub main: WorkerGuard,
    pub full_response: WorkerGuard,
    pub prototype1_observation: Option<WorkerGuard>,
}

static FULL_RESPONSE_LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static PROTOTYPE1_OBSERVATION_LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn current_full_response_log_path() -> Option<&'static Path> {
    FULL_RESPONSE_LOG_PATH.get().map(PathBuf::as_path)
}

pub fn current_prototype1_observation_log_path() -> Option<&'static Path> {
    PROTOTYPE1_OBSERVATION_LOG_PATH.get().map(PathBuf::as_path)
}

pub fn init_tracing(debug_tools: bool) -> Option<LoggingGuards> {
    let mut filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,embed-pipeline=trace"));
    if debug_tools
        && let Ok(directive) = tracing_subscriber::filter::Directive::from_str(&format!(
            "{EXECUTION_DEBUG_TARGET}=debug"
        ))
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
    let (non_blocking_file, main_guard) = tracing_appender::non_blocking(file_appender);
    let full_response_log_file = log_dir.join(format!("llm_full_response_{run_id}.log"));
    let _ = FULL_RESPONSE_LOG_PATH.set(full_response_log_file.clone());
    let full_response_appender =
        tracing_appender::rolling::never(&log_dir, format!("llm_full_response_{run_id}.log"));
    let (full_response_non_blocking, full_response_guard) =
        tracing_appender::non_blocking(full_response_appender);
    let prototype1_observation = prototype1_observation_jsonl(&log_dir, &run_id);

    let file_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .without_time()
        .with_thread_ids(false)
        .with_ansi(false)
        .with_writer(non_blocking_file);

    let full_response_layer = fmt::layer()
        .with_writer(full_response_non_blocking)
        .with_ansi(false)
        .with_level(false)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .without_time();
    let only_full_response = filter::Targets::new().with_target(FULL_RESPONSE_TARGET, Level::TRACE);

    let console_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .without_time()
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true)
        .with_writer(std::io::stderr);
    let console_filter = if debug_tools {
        filter::Targets::new()
            .with_default(filter::LevelFilter::WARN)
            .with_target(EXECUTION_DEBUG_TARGET, Level::DEBUG)
    } else {
        filter::Targets::new().with_default(filter::LevelFilter::WARN)
    };

    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(full_response_layer.with_filter(only_full_response))
        .with(console_layer.with_filter(console_filter));

    let mut prototype1_log_file = None;
    let mut prototype1_guard = None;
    let initialized = if let Some(observation) = prototype1_observation {
        let (log_file, guard, writer) = observation;
        prototype1_log_file = Some(log_file);
        prototype1_guard = Some(guard);
        registry
            .with(
                fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_current_span(true)
                    .with_span_list(true)
                    .with_target(true)
                    .with_level(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_writer(writer)
                    .with_filter(
                        filter::Targets::new()
                            .with_target(EXECUTION_DEBUG_TARGET, Level::TRACE)
                            .with_target(CHAT_HTTP_TARGET, Level::TRACE),
                    ),
            )
            .try_init()
            .is_ok()
    } else {
        registry.try_init().is_ok()
    };

    if initialized {
        tracing::info!(
            target: "ploke_eval",
            debug_tools,
            log_file = %log_file.display(),
            full_response_log_file = %full_response_log_file.display(),
            prototype1_observation_log_file = ?prototype1_log_file.as_ref().map(|path| path.display().to_string()),
            "eval tracing initialized"
        );
        if let Some(path) = prototype1_log_file.as_ref() {
            tracing::info!(
                target: EXECUTION_DEBUG_TARGET,
                operation = "Observation",
                phase = "Init",
                prototype1_observation_log_file = %path.display(),
                "prototype1 observation tracing initialized"
            );
        }
        Some(LoggingGuards {
            main: main_guard,
            full_response: full_response_guard,
            prototype1_observation: prototype1_guard,
        })
    } else {
        None
    }
}

fn prototype1_observation_jsonl(
    log_dir: &Path,
    run_id: &str,
) -> Option<(
    PathBuf,
    WorkerGuard,
    tracing_appender::non_blocking::NonBlocking,
)> {
    let log_file = prototype1_observation_path(log_dir, run_id)?;
    let parent = log_file.parent().unwrap_or(log_dir);
    if let Err(err) = fs::create_dir_all(parent) {
        eprintln!(
            "failed to create Prototype 1 observation log directory {}: {err}",
            parent.display()
        );
        return None;
    }
    let file_name = log_file
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| OsStr::new("prototype1_observation.jsonl"));
    let appender = tracing_appender::rolling::never(parent, file_name);
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);
    let _ = PROTOTYPE1_OBSERVATION_LOG_PATH.set(log_file.clone());
    Some((log_file, guard, non_blocking))
}

fn prototype1_observation_path(log_dir: &Path, run_id: &str) -> Option<PathBuf> {
    let value = env::var_os("PLOKE_PROTOTYPE1_TRACE_JSONL")?;
    if value.is_empty() {
        return None;
    }
    let value_path = PathBuf::from(&value);
    let normalized = value.to_string_lossy();
    if matches!(
        normalized.as_ref(),
        "1" | "true" | "TRUE" | "auto" | "AUTO" | "default" | "DEFAULT"
    ) {
        Some(log_dir.join(format!("prototype1_observation_{run_id}.jsonl")))
    } else {
        Some(value_path)
    }
}
