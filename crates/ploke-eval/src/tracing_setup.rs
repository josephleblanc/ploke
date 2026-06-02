use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt as std_fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{env, fs, str::FromStr};

use chrono::Local;
use ploke_core::EXECUTION_DEBUG_TARGET;
use ploke_tui::tracing_setup::FULL_RESPONSE_TARGET;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt as tracing_fmt, prelude::*};

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

    let file_layer = tracing_fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .without_time()
        .with_thread_ids(false)
        .with_ansi(false)
        .with_writer(non_blocking_file);

    let full_response_layer = tracing_fmt::layer()
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

    let console_layer = tracing_fmt::layer()
        .event_format(CompactConsoleFormat)
        .with_ansi(true)
        .with_writer(std::io::stderr);
    let console_filter = if cfg!(feature = "demo") {
        filter::Targets::new().with_default(filter::LevelFilter::OFF)
    } else if debug_tools {
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
                tracing_fmt::layer()
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
                            .with_target(CHAT_HTTP_TARGET, Level::TRACE)
                            .with_target("chat-loop", Level::TRACE),
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

struct CompactConsoleFormat;

impl<S, N> FormatEvent<S, N> for CompactConsoleFormat
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std_fmt::Result {
        let metadata = event.metadata();
        if metadata.target() == EXECUTION_DEBUG_TARGET {
            return format_prototype1_console_event(&mut writer, event, metadata.level());
        }
        write!(writer, "{:<5}", metadata.level())?;
        if let Some(scope) = ctx.event_scope() {
            let mut first = true;
            for span in scope.from_root() {
                if first {
                    write!(writer, " ")?;
                    first = false;
                } else {
                    write!(writer, ">")?;
                }
                write!(writer, "{}", span.metadata().name())?;
            }
            if !first {
                write!(writer, ":")?;
            }
        }
        write!(writer, " {}:", metadata.target())?;
        if let (Some(file), Some(line)) = (metadata.file(), metadata.line()) {
            write!(writer, " {file}:{line}:")?;
        }
        write!(writer, " ")?;
        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

fn format_prototype1_console_event(
    writer: &mut Writer<'_>,
    event: &Event<'_>,
    level: &Level,
) -> std_fmt::Result {
    let mut fields = EventFields::default();
    event.record(&mut fields);

    write!(writer, "{:<5} p1", level)?;
    if let Some(phase) = fields.short_phase() {
        write!(writer, " {phase}")?;
    }
    if let Some(transition) = fields.get("transition") {
        write!(writer, " {transition}")?;
    }
    if let Some(outcome) = fields.get("outcome") {
        write!(writer, " outcome={}", compact_value(outcome))?;
    }
    if let Some(disposition) = fields
        .get("disposition")
        .or_else(|| fields.get("runner_disposition"))
    {
        write!(writer, " disp={}", compact_value(disposition))?;
    }
    if let Some(generation) = fields.get("generation") {
        write!(writer, " g={generation}")?;
    }
    if let Some(node) = fields
        .get("node_id")
        .or_else(|| fields.get("candidate_node_id"))
        .or_else(|| fields.get("child_node_id"))
    {
        write!(writer, " node={}", short_id(node))?;
    }
    if let Some(branch) = fields.get("branch_id") {
        write!(writer, " branch={}", short_id(branch))?;
    }
    if let Some(candidate) = fields.get("candidate_id") {
        write!(writer, " cand={}", short_candidate(candidate))?;
    }
    if let Some(runtime) = fields
        .get("runtime_id")
        .or_else(|| fields.get("child_runtime"))
        .or_else(|| fields.get("primary_runtime_id"))
    {
        write!(writer, " rt={}", short_runtime(runtime))?;
    }
    for key in [
        "workspace_root",
        "scratch_dir",
        "channel_dir",
        "binary_path",
        "child_binary",
    ] {
        if let Some(path) = fields.get(key) {
            write!(writer, " {}={}", short_path_key(key), compact_path(path))?;
        }
    }
    if let Some(duration) = fields.get("duration_ms") {
        write!(writer, " {duration}ms")?;
    }
    if let Some(exit_code) = fields.get("exit_code") {
        write!(writer, " exit={}", compact_value(exit_code))?;
    }
    if let Some(message) = fields.message() {
        write!(writer, " {}", compact_message(compact_value(message)))?;
    }
    writeln!(writer)
}

#[derive(Default)]
struct EventFields {
    values: BTreeMap<&'static str, String>,
}

impl EventFields {
    fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    fn message(&self) -> Option<&str> {
        self.get("message")
    }

    fn short_phase(&self) -> Option<&str> {
        self.get("phase")
            .or_else(|| self.message().and_then(phase_from_message))
    }

    fn insert(&mut self, field: &Field, value: String) {
        self.values.insert(field.name(), value);
    }
}

impl Visit for EventFields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.insert(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.insert(field, value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std_fmt::Debug) {
        self.insert(field, format!("{value:?}"));
    }
}

fn phase_from_message(message: &str) -> Option<&'static str> {
    if message.contains("materialize") || message.contains("Artifact") {
        Some("materialize")
    } else if message.contains("build") || message.contains("compiled") {
        Some("build")
    } else if message.contains("spawn") || message.contains("ready acknowledgement") {
        Some("spawn")
    } else if message.contains("completion") || message.contains("terminal result") {
        Some("observe")
    } else if message.contains("selection") || message.contains("successor") {
        Some("select")
    } else {
        None
    }
}

fn compact_message(message: &str) -> &str {
    match message {
        "starting planned child state path from broadcast payload" => "start child",
        "resolved prototype1 candidate for parent turn" => "resolved",
        "recorded materialize before entry" => "before",
        "realized child workspace and persisted runner workspace root" => "workspace ready",
        "recorded materialize after entry" => "after",
        "materialized child Artifact" => "artifact ready",
        "recorded build before entry" => "before",
        "compiled child Runtime binary" => "binary ready",
        "build completed" => "done",
        "spawned child Runtime and observed ready acknowledgement" => "runtime ready",
        "spawn completed" => "done",
        "recorded completion before entry" => "before",
        "observed child terminal result through runtime channel" => "terminal",
        "child completion observed" => "done",
        "prototype1 step finished" => "step",
        "prototype1 command finished" => "command",
        other => other,
    }
}

fn compact_value(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn short_id(value: &str) -> String {
    let value = compact_value(value);
    for prefix in ["node-", "branch-", "candidate-"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            let take = rest.len().min(8);
            return format!("{prefix}{}", &rest[..take]);
        }
    }
    value.chars().take(16).collect()
}

fn short_candidate(value: &str) -> String {
    let value = compact_value(value);
    if let Some(rest) = value.strip_prefix("tui-edit-surface-") {
        return format!("tui-{rest}");
    }
    short_id(value)
}

fn short_runtime(value: &str) -> String {
    let value = compact_value(value);
    let value = value
        .strip_prefix("Some(")
        .and_then(|value| value.strip_suffix(')'))
        .map(compact_value)
        .unwrap_or(value);
    if let Some(rest) = value
        .strip_prefix("RuntimeId(")
        .and_then(|v| v.strip_suffix(')'))
    {
        return rest.chars().take(8).collect();
    }
    if value.contains('-') {
        return value.chars().take(8).collect();
    }
    short_id(value)
}

fn compact_path(path: &str) -> String {
    let path = compact_value(path);
    let components = path.split('/').collect::<Vec<_>>();
    if let Some(index) = components.iter().position(|part| *part == "nodes")
        && let Some(node) = components.get(index + 1)
    {
        let mut compact = vec![format!("nodes/{}", short_id(node))];
        compact.extend(
            components
                .iter()
                .skip(index + 2)
                .map(|part| (*part).to_string()),
        );
        return compact.join("/");
    }
    if let Some(index) = components.iter().position(|part| *part == "worktrees") {
        return components
            .iter()
            .skip(index)
            .copied()
            .collect::<Vec<_>>()
            .join("/");
    }
    if components.len() > 3 {
        return components
            .iter()
            .rev()
            .take(3)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("/");
    }
    path.to_string()
}

fn short_path_key(key: &str) -> &str {
    match key {
        "workspace_root" => "worktree",
        "scratch_dir" => "target",
        "channel_dir" => "channel",
        "binary_path" | "child_binary" => "bin",
        other => other,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_prototype1_console_path_uses_node_relative_shape() {
        let path = "/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-20260507-1/prototype1/nodes/node-2b87134077acb431/channels/6dc21cec-e34f-42c8-81e5-df04b7eeab2b";

        assert_eq!(
            compact_path(path),
            "nodes/node-2b871340/channels/6dc21cec-e34f-42c8-81e5-df04b7eeab2b"
        );
    }

    #[test]
    fn compact_prototype1_console_ids_keep_type_prefix() {
        assert_eq!(short_id("node-2b87134077acb431"), "node-2b871340");
        assert_eq!(short_id("branch-9f1ab873140c45ec"), "branch-9f1ab873");
        assert_eq!(short_candidate("tui-edit-surface-g1-01"), "tui-g1-01");
    }

    #[test]
    fn compact_prototype1_console_messages_strip_debug_quotes() {
        assert_eq!(
            compact_message(compact_value("\"recorded build before entry\"")),
            "before"
        );
    }
}
