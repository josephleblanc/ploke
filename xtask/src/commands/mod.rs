//! Command module for xtask utilities.
//!
//! This module provides command implementations organized by crate responsibility:
//! - `parse` - syn_parser integration (A.1)
//! - `db` - ploke_db integration (A.4)
//! - `transform` - ploke_transform integration (A.2) [M.4]
//! - `ingest` - ploke_embed integration (A.3) [M.4]
//!
//! ## Architecture
//!
//! This module integrates with the core xtask architecture from `lib.rs`:
//! - [`crate::context::CommandContext`] - Resource management
//! - [`crate::error::XtaskError`] - Error handling
//! - [`OutputFormat`] - CLI output formatting
//!
//! ## Usage
//!
//! ```rust,ignore
//! use xtask::commands::{Command, CommandContext, OutputFormat};
//!
//! fn run_command(cmd: impl Command) -> Result<(), XtaskError> {
//!     let ctx = CommandContext::new()?;
//!     cmd.execute(&ctx)?;
//!     Ok(())
//! }
//! ```

use serde::Serialize;
use serde_json::Value;
use std::path::Path;

// Re-export command modules
pub mod db;
pub mod parse;
pub mod parse_debug;

// Re-export types from core architecture
pub use crate::context::CommandContext;
pub use crate::error::XtaskError;

/// Output format for command results.
///
/// This type is used by the CLI to determine how to format command output.
/// It is separate from any executor framework to provide CLI-specific formatting options.
#[derive(Debug, Clone, Copy, Default, Serialize, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable formatted output with colors and indentation
    #[default]
    Human,
    /// JSON output for programmatic consumption
    Json,
    /// Tab-separated table output
    Table,
    /// Compact single-line output
    Compact,
}

impl OutputFormat {
    /// Format a serializable value according to this format.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn format<T: Serialize>(&self, value: &T) -> std::result::Result<String, XtaskError> {
        match self {
            OutputFormat::Human => format_human(value),
            OutputFormat::Json => {
                serde_json::to_string_pretty(value).map_err(|e| XtaskError::new(e.to_string()))
            }
            OutputFormat::Table => format_table(value),
            OutputFormat::Compact => {
                serde_json::to_string(value).map_err(|e| XtaskError::new(e.to_string()))
            }
        }
    }
}

/// Format a value for human-readable output.
fn format_human<T: Serialize>(value: &T) -> std::result::Result<String, XtaskError> {
    let json = serde_json::to_value(value).map_err(|e| XtaskError::new(e.to_string()))?;
    if let Some(rendered) = format_human_corpus(&json) {
        return Ok(rendered);
    }

    // Fallback for commands without a custom human renderer yet.
    serde_json::to_string_pretty(&json).map_err(|e| XtaskError::new(e.to_string()))
}

fn format_human_corpus(value: &Value) -> Option<String> {
    let obj = value.as_object()?;
    if obj.get("kind")?.as_str()? != "corpus" {
        return None;
    }

    let run_id = value_str(obj, "run_id");
    let checkout_root = value_str(obj, "checkout_root");
    let artifact_root = value_str(obj, "artifact_root");
    let requested_entries = value_usize(obj, "requested_entries");
    let unique_targets = value_usize(obj, "unique_targets");
    let processed_targets = value_usize(obj, "processed_targets");
    let single_crate_targets = value_usize(obj, "single_crate_targets");
    let workspace_targets = value_usize(obj, "workspace_targets");
    let reused_targets = value_usize(obj, "reused_targets");
    let cloned_targets = value_usize(obj, "cloned_targets");
    let skipped_targets = value_usize(obj, "skipped_targets");
    let clone_failures = value_usize(obj, "clone_failures");
    let discovery_failures = value_usize(obj, "discovery_failures");
    let resolve_failures = value_usize(obj, "resolve_failures");
    let merge_failures = value_usize(obj, "merge_failures");
    let panic_failures = value_usize(obj, "panic_failures");

    let mut failure_lines = Vec::new();
    let mut single_crate_rows = Vec::new();
    let mut workspace_lines = Vec::new();
    let mut other_lines = Vec::new();
    let mut classification_failures = 0u64;
    if let Some(targets) = obj.get("targets").and_then(Value::as_array) {
        for target in targets {
            if target
                .get("classification_error")
                .and_then(Value::as_str)
                .is_some()
            {
                classification_failures += 1;
            }

            match format_human_corpus_target(target, &artifact_root, &checkout_root) {
                Some(CorpusHumanTarget::Failure(line)) => failure_lines.push(line),
                Some(CorpusHumanTarget::SingleCrate(row)) => single_crate_rows.push(row),
                Some(CorpusHumanTarget::Workspace(line)) => workspace_lines.push(line),
                Some(CorpusHumanTarget::Other(line)) => other_lines.push(line),
                None => {}
            }
        }
    }

    let mut out = String::new();
    let total_failures = clone_failures
        + classification_failures
        + discovery_failures
        + resolve_failures
        + merge_failures
        + panic_failures;
    out.push_str(&format!(
        "Corpus Run: {processed_targets} targets processed, {reused_targets} reused, {cloned_targets} cloned, {skipped_targets} skipped, {total_failures} failures\n"
    ));
    out.push_str(&format!("Run ID: {run_id}\n"));
    out.push_str(&format!(
        "Target set: {requested_entries} requested, {unique_targets} unique\n"
    ));

    if workspace_targets > 0 || single_crate_targets > 0 || classification_failures > 0 {
        let mut kinds = vec![
            format!("{single_crate_targets} single-crate"),
            format!("{workspace_targets} workspace"),
        ];
        if classification_failures > 0 {
            kinds.push(format!("{classification_failures} failed classification"));
        }
        out.push_str(&format!("Kinds: {}\n", kinds.join(", ")));
    }

    if total_failures > 0 {
        out.push_str(&format!(
            "Failure breakdown: clone={clone_failures}, classification={classification_failures}, discovery={discovery_failures}, resolve={resolve_failures}, merge={merge_failures}, panic={panic_failures}\n"
        ));
    }

    if !failure_lines.is_empty() {
        out.push_str("\nFailures:\n");
        for line in failure_lines {
            out.push_str(&line);
        }
    }

    if !single_crate_rows.is_empty() {
        out.push_str(
            "\nSingle-crate targets (ms=milliseconds, f=files, n=nodes, r=relations):\n",
        );
        out.push_str(&render_single_crate_table(&single_crate_rows));
    }

    if !workspace_lines.is_empty() {
        out.push_str("\nWorkspace targets skipped by this runner:\n");
        for line in workspace_lines {
            out.push_str(&line);
        }
    }

    if !other_lines.is_empty() {
        out.push_str("\nOther targets:\n");
        for line in other_lines {
            out.push_str(&line);
        }
    }

    if let Some(list_files) = obj.get("list_files").and_then(Value::as_array) {
        let list_files: Vec<&str> = list_files.iter().filter_map(Value::as_str).collect();
        if list_files.len() > 1 {
            out.push_str("\nList files:\n");
            for path in shorten_paths_for_display(&list_files) {
                out.push_str(&format!("- {path}\n"));
            }
        }
    }

    out.push_str(&format!(
        "\nArtifacts: {artifact_root}\nCheckouts: {checkout_root}"
    ));

    Some(out.trim_end().to_string())
}

enum CorpusHumanTarget {
    Failure(String),
    SingleCrate(CorpusSingleCrateRow),
    Workspace(String),
    Other(String),
}

struct CorpusSingleCrateRow {
    repo: String,
    checkout: String,
    commit: String,
    discovery: StageCell,
    resolve: StageCell,
    merge: StageCell,
}

#[derive(Clone)]
enum StageCell {
    Missing,
    Invalid,
    Present(StageCellData),
}

#[derive(Clone)]
struct StageCellData {
    status: String,
    duration: String,
    metric_a: Option<StageMetric>,
    metric_b: Option<StageMetric>,
}

#[derive(Clone)]
struct StageMetric {
    value: String,
    suffix: char,
}

struct CorpusTableRow {
    repo: String,
    checkout: String,
    commit: String,
    discovery: String,
    resolve: String,
    merge: String,
}

#[derive(Default)]
struct CorpusTableLayout {
    discovery: StageLayout,
    resolve: StageLayout,
    merge: StageLayout,
}

#[derive(Default)]
struct StageLayout {
    status_width: usize,
    duration_width: usize,
    metric_a_width: usize,
    metric_b_width: usize,
}

fn format_human_corpus_target(
    target: &Value,
    artifact_root: &str,
    checkout_root: &str,
) -> Option<CorpusHumanTarget> {
    let obj = target.as_object()?;
    let repo = value_str(obj, "normalized_repo");
    let repository_kind = value_str(obj, "repository_kind");
    let action = obj
        .get("clone")
        .and_then(Value::as_object)
        .and_then(|clone| clone.get("action"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let commit_sha = obj
        .get("commit_sha")
        .and_then(Value::as_str)
        .map(short_sha)
        .unwrap_or_else(|| "-".to_string());

    if let Some(failure) = summarize_target_failure(obj) {
        let mut line = format!("- {repo}: {failure} [{action}, {commit_sha}]\n");
        if let Some(source) = summarize_target_failure_source(obj, checkout_root) {
            line.push_str(&source);
        }
        if let Some(emission_site) = summarize_target_failure_emission_site(obj) {
            line.push_str(&emission_site);
        }
        if let Some((label, path)) = summarize_target_failure_path(obj) {
            line.push_str(&format!(
                "  {label}: {}\n",
                shorten_path_for_display(artifact_root, &path)
            ));
        }
        return Some(CorpusHumanTarget::Failure(line));
    }

    let mut line = format!("- {repo}");

    if repository_kind == "workspace" {
        if let Some(workspace_member_count) = obj.get("workspace_member_count").and_then(Value::as_u64)
        {
            line.push_str(&format!(" ({workspace_member_count} members)"));
        }
        line.push_str(&format!(" [{action}, {commit_sha}]"));
    } else {
        let row = CorpusSingleCrateRow {
            repo,
            checkout: action.to_string(),
            commit: commit_sha,
            discovery: parse_stage_cell(obj.get("discovery")),
            resolve: parse_stage_cell(obj.get("resolve")),
            merge: parse_stage_cell(obj.get("merge")),
        };
        return Some(CorpusHumanTarget::SingleCrate(row));
    }

    if let Some(err) = obj.get("classification_error").and_then(Value::as_str) {
        line.push_str(&format!(" classification_error={err}"));
    }

    line.push('\n');
    Some(match repository_kind.as_str() {
        "single_crate" => CorpusHumanTarget::Other(line),
        "workspace" => CorpusHumanTarget::Workspace(line),
        _ => CorpusHumanTarget::Other(line),
    })
}

fn summarize_target_failure(obj: &serde_json::Map<String, Value>) -> Option<String> {
    if let Some(err) = obj
        .get("clone")
        .and_then(Value::as_object)
        .filter(|clone| !clone.get("ok").and_then(Value::as_bool).unwrap_or(false))
        .and_then(|clone| clone.get("error"))
        .and_then(Value::as_str)
    {
        return Some(format!("clone failed: {}", compact_error(err)));
    }

    if let Some(err) = obj.get("classification_error").and_then(Value::as_str) {
        return Some(format!("classification failed: {}", compact_error(err)));
    }

    for (name, stage) in [
        ("discovery", obj.get("discovery")),
        ("resolve", obj.get("resolve")),
        ("merge", obj.get("merge")),
    ] {
        let Some(stage) = stage else {
            continue;
        };
        let Some(stage_obj) = stage.as_object() else {
            continue;
        };
        if stage_obj.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }

        let duration_ms = stage_obj
            .get("duration_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let failure_kind = stage_obj
            .get("failure_kind")
            .and_then(Value::as_str)
            .unwrap_or("error");
        let error = stage_obj
            .get("error")
            .and_then(Value::as_str)
            .map(compact_error);

        let mut summary = format!("{name} {failure_kind} after {duration_ms}ms");
        if let Some(error) = error {
            summary.push_str(": ");
            summary.push_str(&error);
        }
        return Some(summary);
    }

    None
}

fn summarize_target_failure_path(
    obj: &serde_json::Map<String, Value>,
) -> Option<(&'static str, String)> {
    let fallback = || {
        obj.get("summary_path")
            .and_then(Value::as_str)
            .map(|path| ("summary", path.to_string()))
            .or_else(|| {
                obj.get("artifact_dir")
                    .and_then(Value::as_str)
                    .map(|path| ("artifacts", path.to_string()))
            })
    };

    if obj
        .get("clone")
        .and_then(Value::as_object)
        .filter(|clone| !clone.get("ok").and_then(Value::as_bool).unwrap_or(false))
        .is_some()
    {
        return fallback();
    }

    if obj.get("classification_error").and_then(Value::as_str).is_some() {
        return fallback();
    }

    for stage_name in ["discovery", "resolve", "merge"] {
        let Some(stage_obj) = obj.get(stage_name).and_then(Value::as_object) else {
            continue;
        };
        if stage_obj.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }

        if let Some(path) = stage_obj.get("failure_artifact_path").and_then(Value::as_str) {
            return Some(("failure", path.to_string()));
        }
        if let Some(path) = stage_obj.get("artifact_path").and_then(Value::as_str) {
            return Some(("artifacts", path.to_string()));
        }
        return fallback();
    }

    None
}

fn summarize_target_failure_source(
    obj: &serde_json::Map<String, Value>,
    checkout_root: &str,
) -> Option<String> {
    let diagnostic = obj
        .get("classification_diagnostic")
        .and_then(Value::as_object)?;
    let source_path = diagnostic.get("source_path").and_then(Value::as_str)?;
    let mut line = format!(
        "  source: {}",
        shorten_path_for_display(checkout_root, source_path)
    );
    if let Some(span) = diagnostic.get("source_span").and_then(Value::as_object) {
        let rendered = render_source_span(span);
        if !rendered.is_empty() {
            line.push_str(&rendered);
        }
    }
    line.push('\n');
    Some(line)
}

fn summarize_target_failure_emission_site(
    obj: &serde_json::Map<String, Value>,
) -> Option<String> {
    let diagnostic = obj
        .get("classification_diagnostic")
        .and_then(Value::as_object)?;
    let emission_site = diagnostic.get("emission_site").and_then(Value::as_object)?;
    let file = emission_site.get("file").and_then(Value::as_str)?;
    let line = emission_site.get("line").and_then(Value::as_u64)?;
    let column = emission_site.get("column").and_then(Value::as_u64)?;
    Some(format!("  emitted: {file}:{line}:{column}\n"))
}

fn render_source_span(span: &serde_json::Map<String, Value>) -> String {
    let line = span.get("line").and_then(Value::as_u64);
    let col = span.get("col").and_then(Value::as_u64);
    let start = span.get("start").and_then(Value::as_u64);
    let end = span.get("end").and_then(Value::as_u64);

    let mut rendered = String::new();
    if let (Some(line), Some(col)) = (line, col) {
        rendered.push(':');
        rendered.push_str(&line.to_string());
        rendered.push(':');
        rendered.push_str(&col.to_string());
    }

    match (start, end) {
        (Some(start), Some(end)) => rendered.push_str(&format!(" [{start}..{end}]")),
        (Some(start), None) => rendered.push_str(&format!(" [{start}]")),
        _ => {}
    }

    rendered
}

fn shorten_path_for_display(root: &str, path: &str) -> String {
    let Ok(relative) = Path::new(path).strip_prefix(Path::new(root)) else {
        return path.to_string();
    };

    let rendered = relative.display().to_string();
    if rendered.is_empty() {
        path.to_string()
    } else {
        rendered
    }
}

fn shorten_paths_for_display(paths: &[&str]) -> Vec<String> {
    let Some(common_root) = common_parent_dir(paths) else {
        return paths.iter().map(|path| (*path).to_string()).collect();
    };

    paths.iter()
        .map(|path| shorten_path_for_display(&common_root, path))
        .collect()
}

fn common_parent_dir(paths: &[&str]) -> Option<String> {
    let mut components: Vec<_> = Path::new(*paths.first()?).components().collect();
    if components.is_empty() {
        return None;
    }

    for path in &paths[1..] {
        let next_components: Vec<_> = Path::new(path).components().collect();
        let shared_len = components
            .iter()
            .zip(next_components.iter())
            .take_while(|(left, right)| left == right)
            .count();
        components.truncate(shared_len);
        if components.is_empty() {
            return None;
        }
    }

    let common = components.iter().fold(std::path::PathBuf::new(), |mut acc, component| {
        acc.push(component.as_os_str());
        acc
    });

    if common == Path::new("/") {
        return None;
    }

    Some(common.display().to_string())
}

fn parse_stage_cell(stage: Option<&Value>) -> StageCell {
    let Some(stage) = stage else {
        return StageCell::Missing;
    };
    if stage.is_null() {
        return StageCell::Missing;
    }
    let Some(obj) = stage.as_object() else {
        return StageCell::Invalid;
    };

    let ok = obj.get("ok").and_then(Value::as_bool).unwrap_or(false);
    let panic = obj.get("panic").and_then(Value::as_bool).unwrap_or(false);
    let duration_ms = obj.get("duration_ms").and_then(Value::as_u64).unwrap_or(0);
    let nodes = obj.get("nodes_parsed").and_then(Value::as_u64);
    let rels = obj.get("relations_found").and_then(Value::as_u64);
    let files = obj.get("file_count").and_then(Value::as_u64);

    StageCell::Present(StageCellData {
        status: if ok {
            "ok".to_string()
        } else if panic {
            "panic".to_string()
        } else {
            "err".to_string()
        },
        duration: format!("{duration_ms}ms"),
        metric_a: files
            .map(|value| StageMetric {
                value: value.to_string(),
                suffix: 'f',
            })
            .or_else(|| {
                nodes.map(|value| StageMetric {
                    value: value.to_string(),
                    suffix: 'n',
                })
            }),
        metric_b: rels.map(|value| StageMetric {
            value: value.to_string(),
            suffix: 'r',
        }),
    })
}

fn render_single_crate_table(rows: &[CorpusSingleCrateRow]) -> String {
    let layout = derive_corpus_table_layout(rows);
    let rendered_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            let rendered = CorpusTableRow {
                repo: row.repo.clone(),
                checkout: row.checkout.clone(),
                commit: row.commit.clone(),
                discovery: render_stage_cell(&row.discovery, &layout.discovery),
                resolve: render_stage_cell(&row.resolve, &layout.resolve),
                merge: render_stage_cell(&row.merge, &layout.merge),
            };
            vec![
                rendered.repo,
                rendered.checkout,
                rendered.commit,
                rendered.discovery,
                rendered.resolve,
                rendered.merge,
            ]
        })
        .collect();

    render_text_table(
        &["REPO", "CHECKOUT", "COMMIT", "DISCOVERY", "RESOLVE", "MERGE"],
        &rendered_rows,
    )
}

fn derive_corpus_table_layout(rows: &[CorpusSingleCrateRow]) -> CorpusTableLayout {
    let mut layout = CorpusTableLayout::default();
    for row in rows {
        update_stage_layout(&mut layout.discovery, &row.discovery);
        update_stage_layout(&mut layout.resolve, &row.resolve);
        update_stage_layout(&mut layout.merge, &row.merge);
    }
    layout
}

fn update_stage_layout(layout: &mut StageLayout, cell: &StageCell) {
    match cell {
        StageCell::Present(data) => {
            layout.status_width = layout.status_width.max(data.status.chars().count());
            layout.duration_width = layout.duration_width.max(data.duration.chars().count());
            if let Some(metric) = &data.metric_a {
                layout.metric_a_width = layout.metric_a_width.max(metric.value.chars().count());
            }
            if let Some(metric) = &data.metric_b {
                layout.metric_b_width = layout.metric_b_width.max(metric.value.chars().count());
            }
        }
        StageCell::Missing => {
            layout.status_width = layout.status_width.max(1);
        }
        StageCell::Invalid => {
            layout.status_width = layout.status_width.max("invalid".chars().count());
        }
    }
}

fn render_stage_cell(cell: &StageCell, layout: &StageLayout) -> String {
    match cell {
        StageCell::Missing => "-".to_string(),
        StageCell::Invalid => "invalid".to_string(),
        StageCell::Present(data) => {
            let mut parts = Vec::new();
            parts.push(format!("{:<width$}", data.status, width = layout.status_width));
            parts.push(format!("{:>width$}", data.duration, width = layout.duration_width));
            if let Some(metric) = &data.metric_a {
                parts.push(render_stage_metric(metric, layout.metric_a_width));
            }
            if let Some(metric) = &data.metric_b {
                parts.push(render_stage_metric(metric, layout.metric_b_width));
            }
            parts.join(" ")
        }
    }
}

fn render_stage_metric(metric: &StageMetric, width: usize) -> String {
    format!("{:>width$}{}", metric.value, metric.suffix, width = width)
}

fn render_text_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }

    let mut widths: Vec<usize> = headers.iter().map(|header| header.chars().count()).collect();
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            if idx >= widths.len() {
                widths.push(cell.chars().count());
            } else {
                widths[idx] = widths[idx].max(cell.chars().count());
            }
        }
    }

    let mut out = String::new();
    out.push_str(&render_text_table_row(
        &headers.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        &widths,
    ));
    out.push_str(&render_text_table_row(
        &widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>(),
        &widths,
    ));
    for row in rows {
        out.push_str(&render_text_table_row(row, &widths));
    }
    out
}

fn render_text_table_row(row: &[String], widths: &[usize]) -> String {
    let mut line = String::new();
    for (idx, width) in widths.iter().enumerate() {
        if idx > 0 {
            line.push_str("  ");
        }
        let cell = row.get(idx).map(String::as_str).unwrap_or("");
        if idx + 1 == widths.len() {
            line.push_str(cell);
        } else {
            line.push_str(&format!("{cell:<width$}", width = *width));
        }
    }
    line.push('\n');
    line
}

fn compact_error(error: &str) -> String {
    let normalized = error.split_whitespace().collect::<Vec<_>>().join(" ");
    const MAX_LEN: usize = 120;
    if normalized.chars().count() <= MAX_LEN {
        return normalized;
    }

    let truncated: String = normalized.chars().take(MAX_LEN - 3).collect();
    format!("{truncated}...")
}

fn value_str(obj: &serde_json::Map<String, Value>, key: &str) -> String {
    obj.get(key)
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string()
}

fn value_usize(obj: &serde_json::Map<String, Value>, key: &str) -> u64 {
    obj.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn short_sha(commit_sha: &str) -> String {
    commit_sha.chars().take(12).collect()
}

/// Format a value as a table.
fn format_table<T: Serialize>(_value: &T) -> std::result::Result<String, XtaskError> {
    // Placeholder - full implementation in M.4
    // For now, return a generic error indicating not implemented
    Err(XtaskError::new("Table formatting not yet implemented"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_default() {
        let fmt = OutputFormat::default();
        assert!(matches!(fmt, OutputFormat::Human));
    }

    #[test]
    fn test_output_format_json() {
        let data = serde_json::json!({"key": "value"});
        let formatted = OutputFormat::Json.format(&data).unwrap();
        assert!(formatted.contains("key"));
        assert!(formatted.contains("value"));
    }

    #[test]
    fn test_output_format_human() {
        let data = serde_json::json!({"key": "value"});
        let formatted = OutputFormat::Human.format(&data).unwrap();
        assert!(formatted.contains("key"));
    }

    #[test]
    fn test_output_format_human_corpus_summary() {
        let data = serde_json::json!({
            "kind": "corpus",
            "run_id": "run-123",
            "checkout_root": "/tmp/checkouts",
            "artifact_root": "/tmp/artifacts",
            "list_files": ["/tmp/list-a.txt", "/tmp/list-b.txt"],
            "requested_entries": 10,
            "unique_targets": 10,
            "processed_targets": 2,
            "single_crate_targets": 1,
            "workspace_targets": 1,
            "reused_targets": 2,
            "cloned_targets": 0,
            "skipped_targets": 0,
            "clone_failures": 0,
            "discovery_failures": 0,
            "resolve_failures": 0,
            "merge_failures": 0,
            "panic_failures": 0,
            "targets": [
                {
                    "normalized_repo": "bitflags/bitflags",
                    "repository_kind": "single_crate",
                    "workspace_member_count": null,
                    "classification_error": null,
                    "commit_sha": "88a7a18a2ec3e673ff3217da83d56cdadd9a99a4",
                    "clone": { "action": "reused" },
                    "discovery": { "ok": true, "panic": false, "duration_ms": 1, "file_count": 98, "nodes_parsed": null, "relations_found": null, "failure_kind": null, "error": null },
                    "resolve": { "ok": true, "panic": false, "duration_ms": 80, "file_count": null, "nodes_parsed": 757, "relations_found": 484, "failure_kind": null, "error": null },
                    "merge": { "ok": true, "panic": false, "duration_ms": 125, "file_count": null, "nodes_parsed": 725, "relations_found": 437, "failure_kind": null, "error": null }
                }
            ]
        });

        let formatted = OutputFormat::Human.format(&data).unwrap();
        assert!(formatted.contains("Corpus Run: 2 targets processed"));
        assert!(formatted.contains(
            "Single-crate targets (ms=milliseconds, f=files, n=nodes, r=relations):"
        ));
        assert!(formatted.contains("REPO"));
        assert!(formatted.contains("CHECKOUT"));
        assert!(formatted.contains("DISCOVERY"));
        assert!(formatted.contains("bitflags/bitflags"));
        assert!(formatted.contains("reused"));
        assert!(formatted.contains("ok 1ms 98f"));
        assert!(formatted.contains("ok 80ms 757n 484r"));
        assert!(formatted.contains("ok 125ms 725n 437r"));
        assert!(formatted.contains("List files:\n- list-a.txt\n- list-b.txt\n"));
        assert!(formatted.contains("Artifacts: /tmp/artifacts"));
    }

    #[test]
    fn test_output_format_human_corpus_failures_first() {
        let data = serde_json::json!({
            "kind": "corpus",
            "run_id": "run-456",
            "checkout_root": "/tmp/checkouts",
            "artifact_root": "/tmp/artifacts",
            "list_files": ["/tmp/list-a.txt", "/tmp/list-b.txt"],
            "requested_entries": 2,
            "unique_targets": 2,
            "processed_targets": 2,
            "single_crate_targets": 2,
            "workspace_targets": 0,
            "reused_targets": 2,
            "cloned_targets": 0,
            "skipped_targets": 0,
            "clone_failures": 0,
            "discovery_failures": 1,
            "resolve_failures": 0,
            "merge_failures": 0,
            "panic_failures": 1,
            "targets": [
                {
                    "normalized_repo": "bad/repo",
                    "repository_kind": "single_crate",
                    "workspace_member_count": null,
                    "classification_error": null,
                    "commit_sha": "1234567890abcdef1234567890abcdef12345678",
                    "summary_path": "/tmp/artifacts/bad__repo/summary.json",
                    "clone": { "ok": true, "action": "reused", "error": null },
                    "discovery": { "ok": false, "panic": true, "duration_ms": 17, "file_count": null, "nodes_parsed": null, "relations_found": null, "failure_kind": "panic", "error": "thread 'worker' panicked at parser invariant violated\nextra details", "failure_artifact_path": "/tmp/artifacts/bad__repo/discovery/failure.json" },
                    "resolve": null,
                    "merge": null
                },
                {
                    "normalized_repo": "good/repo",
                    "repository_kind": "single_crate",
                    "workspace_member_count": null,
                    "classification_error": null,
                    "commit_sha": "88a7a18a2ec3e673ff3217da83d56cdadd9a99a4",
                    "summary_path": "/tmp/artifacts/good__repo/summary.json",
                    "clone": { "ok": true, "action": "reused", "error": null },
                    "discovery": { "ok": true, "panic": false, "duration_ms": 1, "file_count": 98, "nodes_parsed": null, "relations_found": null, "failure_kind": null, "error": null },
                    "resolve": { "ok": true, "panic": false, "duration_ms": 80, "file_count": null, "nodes_parsed": 757, "relations_found": 484, "failure_kind": null, "error": null },
                    "merge": { "ok": true, "panic": false, "duration_ms": 125, "file_count": null, "nodes_parsed": 725, "relations_found": 437, "failure_kind": null, "error": null }
                }
            ]
        });

        let formatted = OutputFormat::Human.format(&data).unwrap();
        assert!(formatted.contains("Failure breakdown: clone=0, classification=0, discovery=1, resolve=0, merge=0, panic=1"));
        assert!(formatted.contains("\nFailures:\n- bad/repo: discovery panic after 17ms: thread 'worker' panicked at parser invariant violated extra details [reused, 1234567890ab]\n"));
        assert!(formatted.contains("  failure: bad__repo/discovery/failure.json\n"));
        assert!(formatted.contains(
            "\nSingle-crate targets (ms=milliseconds, f=files, n=nodes, r=relations):\nREPO"
        ));
        assert!(formatted.contains("good/repo"));
        assert!(formatted.contains("ok 1ms 98f"));
        assert!(formatted.contains("ok 80ms 757n 484r"));
        assert!(formatted.contains("ok 125ms 725n 437r"));
        assert!(!formatted.contains("bad/repo  reused"));
    }

    #[test]
    fn test_output_format_human_corpus_classification_failure_counts() {
        let data = serde_json::json!({
            "kind": "corpus",
            "run_id": "run-789",
            "checkout_root": "/tmp/checkouts",
            "artifact_root": "/tmp/artifacts",
            "list_files": ["/tmp/list-a.txt", "/tmp/list-b.txt"],
            "requested_entries": 5,
            "unique_targets": 5,
            "processed_targets": 5,
            "single_crate_targets": 2,
            "workspace_targets": 2,
            "reused_targets": 5,
            "cloned_targets": 0,
            "skipped_targets": 0,
            "clone_failures": 0,
            "discovery_failures": 0,
            "resolve_failures": 0,
            "merge_failures": 0,
            "panic_failures": 0,
            "targets": [
                {
                    "normalized_repo": "fail/repo",
                    "repository_kind": "unknown",
                    "workspace_member_count": null,
                    "classification_error": "Failed to parse workspace Cargo.toml at /tmp/checkouts/fail__repo/Cargo.toml",
                    "classification_diagnostic": {
                        "kind": "workspace_manifest_parse",
                        "summary": "Failed to parse workspace Cargo.toml at /tmp/checkouts/fail__repo/Cargo.toml",
                        "detail": "missing field `members`",
                        "source_path": "/tmp/checkouts/fail__repo/Cargo.toml",
                        "source_span": { "start": 10, "end": 19, "line": 1, "col": 11 },
                        "emission_site": { "file": "crates/ingest/syn_parser/src/discovery/workspace.rs", "line": 191, "column": 13 },
                        "backtrace": "stack backtrace:\n  0: syn_parser::discovery::workspace",
                        "context": [
                            { "key": "manifest_path", "value": "/tmp/checkouts/fail__repo/Cargo.toml" }
                        ]
                    },
                    "commit_sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "summary_path": "/tmp/artifacts/fail__repo/summary.json",
                    "clone": { "ok": true, "action": "reused", "error": null },
                    "discovery": null,
                    "resolve": null,
                    "merge": null
                },
                {
                    "normalized_repo": "good/one",
                    "repository_kind": "single_crate",
                    "workspace_member_count": null,
                    "classification_error": null,
                    "commit_sha": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "clone": { "ok": true, "action": "reused", "error": null },
                    "discovery": { "ok": true, "panic": false, "duration_ms": 1, "file_count": 10, "nodes_parsed": null, "relations_found": null, "failure_kind": null, "error": null },
                    "resolve": { "ok": true, "panic": false, "duration_ms": 20, "file_count": null, "nodes_parsed": 100, "relations_found": 50, "failure_kind": null, "error": null },
                    "merge": { "ok": true, "panic": false, "duration_ms": 30, "file_count": null, "nodes_parsed": 90, "relations_found": 45, "failure_kind": null, "error": null }
                },
                {
                    "normalized_repo": "good/two",
                    "repository_kind": "single_crate",
                    "workspace_member_count": null,
                    "classification_error": null,
                    "commit_sha": "cccccccccccccccccccccccccccccccccccccccc",
                    "clone": { "ok": true, "action": "reused", "error": null },
                    "discovery": { "ok": true, "panic": false, "duration_ms": 2, "file_count": 12, "nodes_parsed": null, "relations_found": null, "failure_kind": null, "error": null },
                    "resolve": { "ok": true, "panic": false, "duration_ms": 22, "file_count": null, "nodes_parsed": 120, "relations_found": 60, "failure_kind": null, "error": null },
                    "merge": { "ok": true, "panic": false, "duration_ms": 32, "file_count": null, "nodes_parsed": 110, "relations_found": 55, "failure_kind": null, "error": null }
                },
                {
                    "normalized_repo": "workspace/one",
                    "repository_kind": "workspace",
                    "workspace_member_count": 3,
                    "classification_error": null,
                    "commit_sha": "dddddddddddddddddddddddddddddddddddddddd",
                    "clone": { "ok": true, "action": "reused", "error": null },
                    "discovery": null,
                    "resolve": null,
                    "merge": null
                },
                {
                    "normalized_repo": "workspace/two",
                    "repository_kind": "workspace",
                    "workspace_member_count": 4,
                    "classification_error": null,
                    "commit_sha": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                    "clone": { "ok": true, "action": "reused", "error": null },
                    "discovery": null,
                    "resolve": null,
                    "merge": null
                }
            ]
        });

        let formatted = OutputFormat::Human.format(&data).unwrap();
        assert!(formatted.contains("Corpus Run: 5 targets processed, 5 reused, 0 cloned, 0 skipped, 1 failures"));
        assert!(formatted.contains("Kinds: 2 single-crate, 2 workspace, 1 failed classification"));
        assert!(formatted.contains("Failure breakdown: clone=0, classification=1, discovery=0, resolve=0, merge=0, panic=0"));
        assert!(formatted.contains("\nFailures:\n- fail/repo: classification failed: Failed to parse workspace Cargo.toml at /tmp/checkouts/fail__repo/Cargo.toml [reused, aaaaaaaaaaaa]\n"));
        assert!(formatted.contains("  source: fail__repo/Cargo.toml:1:11 [10..19]\n"));
        assert!(formatted.contains("  emitted: crates/ingest/syn_parser/src/discovery/workspace.rs:191:13\n"));
        assert!(formatted.contains("  summary: fail__repo/summary.json\n"));
    }

    #[test]
    fn test_output_format_table_not_implemented() {
        let data = serde_json::json!({"key": "value"});
        let result = OutputFormat::Table.format(&data);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not yet implemented")
        );
    }
}
