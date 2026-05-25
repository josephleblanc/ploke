//! Pipeline registry lookup and validation commands.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use clap::{Args, Subcommand};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::{CommandContext, XtaskError};
use crate::display_relative;

const DEFAULT_REGISTRY: &str = "docs/workflow/pipeline-registry.jsonl";
const DEFAULT_HOOK_STATE: &str = ".codex/pipeline-hook-context-state.json";
const DEFAULT_HOOK_COOLDOWN_SECS: u64 = 20 * 60;

/// Commands for the durable pipeline/function registry.
#[derive(Debug, Clone, Subcommand)]
pub enum Pipeline {
    /// List registered pipelines.
    List(PipelineList),
    /// Find registry records by query, source path, or symbol.
    Find(PipelineFind),
    /// Show one pipeline and its registered functions.
    Show(PipelineShow),
    /// Validate registry shape and referenced files.
    Check(PipelineCheck),
    /// Emit Codex hook context for registered pipeline mentions.
    #[command(name = "hook-context")]
    HookContext(PipelineHookContext),
}

impl Pipeline {
    /// Execute a pipeline registry command.
    pub fn execute(&self, ctx: &CommandContext) -> Result<PipelineOutput, XtaskError> {
        match self {
            Self::List(cmd) => list(ctx, &cmd.registry),
            Self::Find(cmd) => find(ctx, cmd),
            Self::Show(cmd) => show(ctx, cmd),
            Self::Check(cmd) => check(ctx, cmd),
            Self::HookContext(_) => Err(XtaskError::validation(
                "`pipeline hook-context` must be dispatched through Cli::execute_hook_context",
            )
            .with_recovery("Run `cargo xtask pipeline hook-context` through the xtask CLI.")),
        }
    }
}

/// Arguments for `pipeline list`.
#[derive(Debug, Clone, Args)]
pub struct PipelineList {
    /// Registry JSONL path. Defaults to docs/workflow/pipeline-registry.jsonl.
    #[arg(long, default_value = DEFAULT_REGISTRY, value_name = "PATH")]
    registry: PathBuf,
}

/// Arguments for `pipeline find`.
#[derive(Debug, Clone, Args)]
pub struct PipelineFind {
    /// Optional free-text query matched against pipeline, stage, path, symbol, role, docs, and tests.
    #[arg(value_name = "QUERY")]
    query: Option<String>,

    /// Match records for a source path. Absolute paths are normalized relative to the workspace.
    #[arg(long, value_name = "PATH")]
    path: Option<PathBuf>,

    /// Match a Rust symbol name.
    #[arg(long, value_name = "SYMBOL")]
    symbol: Option<String>,

    /// Registry JSONL path. Defaults to docs/workflow/pipeline-registry.jsonl.
    #[arg(long, default_value = DEFAULT_REGISTRY, value_name = "PATH")]
    registry: PathBuf,
}

/// Arguments for `pipeline show`.
#[derive(Debug, Clone, Args)]
pub struct PipelineShow {
    /// Pipeline id to display.
    #[arg(value_name = "PIPELINE_ID")]
    pipeline_id: String,

    /// Registry JSONL path. Defaults to docs/workflow/pipeline-registry.jsonl.
    #[arg(long, default_value = DEFAULT_REGISTRY, value_name = "PATH")]
    registry: PathBuf,
}

/// Arguments for `pipeline check`.
#[derive(Debug, Clone, Args)]
pub struct PipelineCheck {
    /// Registry JSONL path. Defaults to docs/workflow/pipeline-registry.jsonl.
    #[arg(long, default_value = DEFAULT_REGISTRY, value_name = "PATH")]
    registry: PathBuf,

    /// Return passed=false instead of exiting with a validation error.
    #[arg(long)]
    report_only: bool,
}

/// Arguments for `pipeline hook-context`.
#[derive(Debug, Clone, Args)]
pub struct PipelineHookContext {
    /// Registry JSONL path. Defaults to docs/workflow/pipeline-registry.jsonl.
    #[arg(long, default_value = DEFAULT_REGISTRY, value_name = "PATH")]
    registry: PathBuf,

    /// Hook cooldown state path. Defaults to .codex/pipeline-hook-context-state.json.
    #[arg(long, default_value = DEFAULT_HOOK_STATE, value_name = "PATH")]
    state_path: PathBuf,

    /// Suppress each matching registry row for this many seconds after emission.
    #[arg(long, default_value_t = DEFAULT_HOOK_COOLDOWN_SECS)]
    cooldown_secs: u64,

    /// Maximum matched records included in hook context.
    #[arg(long, default_value_t = 5)]
    max_matches: usize,
}

impl PipelineHookContext {
    /// Read a Codex hook event from stdin and emit raw Codex hook JSON.
    pub fn execute_raw(&self, ctx: &CommandContext) -> Result<(), XtaskError> {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        let Some(output) = self.output_for_event(ctx, &input)? else {
            return Ok(());
        };
        println!("{}", serde_json::to_string(&output)?);
        Ok(())
    }

    fn output_for_event(
        &self,
        ctx: &CommandContext,
        input: &str,
    ) -> Result<Option<CodexHookOutput>, XtaskError> {
        let event = serde_json::from_str::<serde_json::Value>(input).map_err(|err| {
            XtaskError::validation(format!("invalid Codex hook event JSON: {err}"))
                .with_recovery("Codex should pass one hook event JSON object on stdin.")
        })?;
        let Some(event_name) = event
            .get("hook_event_name")
            .and_then(serde_json::Value::as_str)
        else {
            return Ok(None);
        };
        if !matches!(event_name, "UserPromptSubmit" | "PreToolUse") {
            return Ok(None);
        }

        let loaded = load_registry(ctx, &self.registry)?;
        let haystack = hook_event_text(&event);
        if haystack.trim().is_empty() {
            return Ok(None);
        }

        let matches = hook_matches(&loaded.entries, &haystack.to_lowercase());
        let matches = self.apply_cooldown(ctx, matches)?;
        if matches.is_empty() {
            return Ok(None);
        }

        Ok(Some(CodexHookOutput {
            hook_specific_output: CodexHookSpecificOutput {
                hook_event_name: event_name.to_string(),
                additional_context: render_hook_context(&matches),
            },
        }))
    }

    fn apply_cooldown(
        &self,
        ctx: &CommandContext,
        matches: Vec<HookMatch>,
    ) -> Result<Vec<HookMatch>, XtaskError> {
        if matches.is_empty() {
            return Ok(matches);
        }
        if self.cooldown_secs == 0 {
            return Ok(matches.into_iter().take(self.max_matches).collect());
        }

        let root = ctx.workspace_root()?;
        let state_path = resolve_path(root, &self.state_path);
        let now = unix_epoch_secs();
        let mut state = HookCooldownState::load_best_effort(&state_path);
        let allowed = matches
            .into_iter()
            .filter(|hook_match| state.should_emit(&hook_match.row_key, now, self.cooldown_secs))
            .take(self.max_matches)
            .collect::<Vec<_>>();

        for hook_match in &allowed {
            state.mark_emitted(
                hook_match.row_key.clone(),
                hook_match.record.key(),
                now,
                self.cooldown_secs,
            );
        }
        if !allowed.is_empty() {
            state.store_best_effort(&state_path);
        }

        Ok(allowed)
    }
}

/// Output from pipeline registry commands.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PipelineOutput {
    /// Output for `pipeline list`.
    PipelineList {
        /// Registry path used.
        registry: String,
        /// Registered pipelines.
        pipelines: Vec<PipelineSummary>,
    },
    /// Output for `pipeline find`.
    PipelineFind {
        /// Registry path used.
        registry: String,
        /// Matching records.
        matches: Vec<RegistryRecord>,
    },
    /// Output for `pipeline show`.
    PipelineShow {
        /// Registry path used.
        registry: String,
        /// Pipeline metadata record.
        pipeline: RegistryRecord,
        /// Function records in the pipeline.
        functions: Vec<RegistryRecord>,
    },
    /// Output for `pipeline check`.
    PipelineRegistryCheck {
        /// Registry path used.
        registry: String,
        /// Whether validation passed.
        passed: bool,
        /// Number of records parsed.
        records: usize,
        /// Validation problems.
        problems: Vec<RegistryProblem>,
    },
}

/// One pipeline summary for `pipeline list`.
#[derive(Debug, Clone, Serialize)]
pub struct PipelineSummary {
    /// Pipeline id.
    pub id: String,
    /// Optional status label.
    pub status: Option<String>,
    /// Optional owner area.
    pub owner_area: Option<String>,
    /// Optional one-line summary.
    pub summary: Option<String>,
    /// Number of registered function records.
    pub functions: usize,
    /// Docs linked from the pipeline metadata.
    pub docs: Vec<String>,
}

/// One parsed JSONL registry record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryRecord {
    /// Record kind, currently `pipeline` or `function`.
    pub kind: String,
    /// Pipeline id for pipeline records.
    #[serde(default)]
    pub id: Option<String>,
    /// Owning pipeline id for function records.
    #[serde(default)]
    pub pipeline: Option<String>,
    /// Optional lifecycle status.
    #[serde(default)]
    pub status: Option<String>,
    /// Optional owner area.
    #[serde(default)]
    pub owner_area: Option<String>,
    /// Optional one-line pipeline summary.
    #[serde(default)]
    pub summary: Option<String>,
    /// Pipeline stage for function records.
    #[serde(default)]
    pub stage: Option<String>,
    /// Source path for function records.
    #[serde(default)]
    pub path: Option<String>,
    /// Source symbol for function records.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Behavioral role for function records.
    #[serde(default)]
    pub role: Option<String>,
    /// Authority docs for this record.
    #[serde(default)]
    pub docs: Vec<String>,
    /// Focused tests related to this record.
    #[serde(default)]
    pub tests: Vec<String>,
    /// Known bug reports related to this record.
    #[serde(default)]
    pub bugs: Vec<String>,
}

/// One registry validation problem.
#[derive(Debug, Clone, Serialize)]
pub struct RegistryProblem {
    /// One-indexed JSONL line number.
    pub line: usize,
    /// Optional record key.
    pub record: Option<String>,
    /// Human-readable problem.
    pub problem: String,
}

#[derive(Debug, Serialize)]
struct CodexHookOutput {
    #[serde(rename = "hookSpecificOutput")]
    hook_specific_output: CodexHookSpecificOutput,
}

#[derive(Debug, Serialize)]
struct CodexHookSpecificOutput {
    #[serde(rename = "hookEventName")]
    hook_event_name: String,
    #[serde(rename = "additionalContext")]
    additional_context: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct HookCooldownState {
    rows: BTreeMap<String, HookCooldownRow>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HookCooldownRow {
    record: Option<String>,
    last_emitted_epoch_secs: u64,
    cooldown_secs: u64,
}

fn list(ctx: &CommandContext, registry: &Path) -> Result<PipelineOutput, XtaskError> {
    let loaded = load_registry(ctx, registry)?;
    let mut functions_by_pipeline: BTreeMap<String, usize> = BTreeMap::new();
    for entry in &loaded.entries {
        if entry.record.kind == "function" {
            if let Some(pipeline) = &entry.record.pipeline {
                *functions_by_pipeline.entry(pipeline.clone()).or_default() += 1;
            }
        }
    }

    let mut pipelines = loaded
        .entries
        .iter()
        .filter(|entry| entry.record.kind == "pipeline")
        .map(|entry| {
            let id = entry.record.id.clone().unwrap_or_default();
            PipelineSummary {
                functions: functions_by_pipeline.get(&id).copied().unwrap_or_default(),
                id,
                status: entry.record.status.clone(),
                owner_area: entry.record.owner_area.clone(),
                summary: entry.record.summary.clone(),
                docs: entry.record.docs.clone(),
            }
        })
        .collect::<Vec<_>>();
    pipelines.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(PipelineOutput::PipelineList {
        registry: loaded.registry_display,
        pipelines,
    })
}

fn find(ctx: &CommandContext, cmd: &PipelineFind) -> Result<PipelineOutput, XtaskError> {
    let loaded = load_registry(ctx, &cmd.registry)?;
    let root = ctx.workspace_root()?;
    let normalized_path = cmd.path.as_ref().map(|path| {
        if path.is_absolute() {
            display_relative(path, &root)
        } else {
            path.display().to_string()
        }
    });
    let query = cmd.query.as_ref().map(|query| query.to_lowercase());
    let symbol = cmd.symbol.as_ref().map(|symbol| symbol.to_lowercase());

    let matches = loaded
        .entries
        .into_iter()
        .filter(|entry| {
            let record = &entry.record;
            if let Some(path) = &normalized_path {
                if record.path.as_deref() != Some(path.as_str()) {
                    return false;
                }
            }
            if let Some(symbol) = &symbol {
                if record.symbol.as_deref().map(str::to_lowercase).as_deref()
                    != Some(symbol.as_str())
                {
                    return false;
                }
            }
            if let Some(query) = &query {
                return record.search_text().contains(query);
            }
            true
        })
        .map(|entry| entry.record)
        .collect();

    Ok(PipelineOutput::PipelineFind {
        registry: loaded.registry_display,
        matches,
    })
}

fn show(ctx: &CommandContext, cmd: &PipelineShow) -> Result<PipelineOutput, XtaskError> {
    let loaded = load_registry(ctx, &cmd.registry)?;
    let mut pipeline = None;
    let mut functions = Vec::new();
    for entry in loaded.entries {
        let record = entry.record;
        if record.kind == "pipeline" && record.id.as_deref() == Some(cmd.pipeline_id.as_str()) {
            pipeline = Some(record);
        } else if record.kind == "function"
            && record.pipeline.as_deref() == Some(cmd.pipeline_id.as_str())
        {
            functions.push(record);
        }
    }
    functions.sort_by(|left, right| {
        left.stage
            .cmp(&right.stage)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.symbol.cmp(&right.symbol))
    });

    let pipeline = pipeline.ok_or_else(|| {
        XtaskError::validation(format!("pipeline `{}` is not registered", cmd.pipeline_id))
            .with_recovery("Run `cargo xtask pipeline list` to see registered pipeline ids.")
    })?;

    Ok(PipelineOutput::PipelineShow {
        registry: loaded.registry_display,
        pipeline,
        functions,
    })
}

fn check(ctx: &CommandContext, cmd: &PipelineCheck) -> Result<PipelineOutput, XtaskError> {
    let loaded = load_registry(ctx, &cmd.registry)?;
    let root = ctx.workspace_root()?;
    let problems = validate_registry(&loaded.entries, &root);
    let output = PipelineOutput::PipelineRegistryCheck {
        registry: loaded.registry_display,
        passed: problems.is_empty(),
        records: loaded.entries.len(),
        problems: problems.clone(),
    };

    if !problems.is_empty() && !cmd.report_only {
        let mut message = format!("pipeline registry has {} problem(s)", problems.len());
        for problem in problems.iter().take(12) {
            message.push_str(&format!("\nline {}: {}", problem.line, problem.problem));
        }
        if problems.len() > 12 {
            message.push_str(&format!(
                "\n... {} additional problem(s) omitted",
                problems.len() - 12
            ));
        }
        return Err(XtaskError::validation(message)
            .with_recovery("Fix the JSONL record or run with `--report-only` to inspect output."));
    }

    Ok(output)
}

#[derive(Debug)]
struct LoadedRegistry {
    registry_display: String,
    entries: Vec<RegistryEntry>,
}

#[derive(Debug)]
struct RegistryEntry {
    line: usize,
    record: RegistryRecord,
}

fn load_registry(ctx: &CommandContext, registry: &Path) -> Result<LoadedRegistry, XtaskError> {
    let root = ctx.workspace_root()?;
    let path = resolve_path(&root, registry);
    let content = fs::read_to_string(&path).map_err(|err| {
        XtaskError::Resource(format!(
            "failed to read pipeline registry `{}`: {err}",
            display_relative(&path, &root)
        ))
    })?;

    let mut entries = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record = serde_json::from_str::<RegistryRecord>(trimmed).map_err(|err| {
            XtaskError::validation(format!(
                "invalid JSON in pipeline registry at line {line_no}: {err}"
            ))
            .with_recovery("Each non-empty registry line must be one complete JSON object.")
        })?;
        entries.push(RegistryEntry {
            line: line_no,
            record,
        });
    }

    Ok(LoadedRegistry {
        registry_display: display_relative(&path, &root),
        entries,
    })
}

fn validate_registry(entries: &[RegistryEntry], root: &Path) -> Vec<RegistryProblem> {
    let mut problems = Vec::new();
    let mut pipeline_ids = BTreeSet::new();

    for entry in entries {
        let record = &entry.record;
        match record.kind.as_str() {
            "pipeline" => {
                let Some(id) = required(&mut problems, entry, record.id.as_deref(), "id") else {
                    continue;
                };
                if !pipeline_ids.insert(id.to_string()) {
                    problems.push(problem(entry, Some(id), "duplicate pipeline id"));
                }
                validate_paths(&mut problems, entry, root, &record.docs, "doc");
                validate_paths(&mut problems, entry, root, &record.bugs, "bug");
            }
            "function" => {
                required(&mut problems, entry, record.pipeline.as_deref(), "pipeline");
                required(&mut problems, entry, record.stage.as_deref(), "stage");
                let path = required(&mut problems, entry, record.path.as_deref(), "path");
                let symbol = required(&mut problems, entry, record.symbol.as_deref(), "symbol");
                required(&mut problems, entry, record.role.as_deref(), "role");
                validate_paths(&mut problems, entry, root, &record.docs, "doc");
                validate_paths(&mut problems, entry, root, &record.bugs, "bug");
                if let (Some(path), Some(symbol)) = (path, symbol) {
                    validate_source_symbol(&mut problems, entry, root, path, symbol);
                }
            }
            other => problems.push(problem(
                entry,
                record.key().as_deref(),
                format!("unknown record kind `{other}`"),
            )),
        }
    }

    for entry in entries {
        let record = &entry.record;
        if record.kind == "function" {
            if let Some(pipeline) = &record.pipeline {
                if !pipeline_ids.contains(pipeline) {
                    problems.push(problem(
                        entry,
                        record.key().as_deref(),
                        format!("function references unknown pipeline `{pipeline}`"),
                    ));
                }
            }
        }
    }

    problems
}

fn required<'a>(
    problems: &mut Vec<RegistryProblem>,
    entry: &RegistryEntry,
    value: Option<&'a str>,
    field: &str,
) -> Option<&'a str> {
    match value.filter(|value| !value.trim().is_empty()) {
        Some(value) => Some(value),
        None => {
            problems.push(problem(
                entry,
                entry.record.key().as_deref(),
                format!("missing required `{field}`"),
            ));
            None
        }
    }
}

fn validate_paths(
    problems: &mut Vec<RegistryProblem>,
    entry: &RegistryEntry,
    root: &Path,
    paths: &[String],
    label: &str,
) {
    for path in paths {
        let resolved = resolve_path(root, Path::new(path));
        if !resolved.exists() {
            problems.push(problem(
                entry,
                entry.record.key().as_deref(),
                format!("{label} path `{path}` does not exist"),
            ));
        }
    }
}

fn validate_source_symbol(
    problems: &mut Vec<RegistryProblem>,
    entry: &RegistryEntry,
    root: &Path,
    path: &str,
    symbol: &str,
) {
    let resolved = resolve_path(root, Path::new(path));
    let Ok(content) = fs::read_to_string(&resolved) else {
        problems.push(problem(
            entry,
            entry.record.key().as_deref(),
            format!("source path `{path}` does not exist or is not readable"),
        ));
        return;
    };
    if !content.contains(symbol) {
        problems.push(problem(
            entry,
            entry.record.key().as_deref(),
            format!("symbol `{symbol}` was not found in `{path}`"),
        ));
    }
}

#[derive(Debug)]
struct HookMatch {
    row_key: String,
    record: RegistryRecord,
    pipeline: Option<RegistryRecord>,
}

fn hook_event_text(event: &serde_json::Value) -> String {
    let prompt = event
        .get("prompt")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let tool_name = event
        .get("tool_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let tool_input = event
        .get("tool_input")
        .map(|value| serde_json::to_string(value).unwrap_or_else(|_| value.to_string()))
        .unwrap_or_default();

    [prompt, tool_name, tool_input.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .join("\n")
}

fn hook_matches(entries: &[RegistryEntry], haystack: &str) -> Vec<HookMatch> {
    let pipelines = entries
        .iter()
        .filter_map(|entry| {
            let record = &entry.record;
            if record.kind == "pipeline" {
                record.id.as_ref().map(|id| (id.as_str(), record))
            } else {
                None
            }
        })
        .collect::<BTreeMap<_, _>>();

    let mut seen = BTreeSet::new();
    entries
        .iter()
        .filter_map(|entry| {
            let record = &entry.record;
            if !matches!(record.kind.as_str(), "pipeline" | "function") {
                return None;
            }
            let matched = [
                record.id.as_deref(),
                record.pipeline.as_deref(),
                record.path.as_deref(),
                record.symbol.as_deref(),
            ]
            .into_iter()
            .flatten()
            .any(|key| haystack.contains(&key.to_lowercase()));
            if !matched {
                return None;
            }

            let pipeline_id = record.id.as_ref().or(record.pipeline.as_ref())?;
            let symbol = record.symbol.as_deref().unwrap_or_default();
            if !seen.insert((pipeline_id.clone(), symbol.to_string())) {
                return None;
            }

            Some(HookMatch {
                row_key: hook_row_key(entry),
                record: record.clone(),
                pipeline: pipelines
                    .get(pipeline_id.as_str())
                    .map(|record| (*record).clone()),
            })
        })
        .collect()
}

fn hook_row_key(entry: &RegistryEntry) -> String {
    format!("jsonl-line:{}", entry.line)
}

fn render_hook_context(matches: &[HookMatch]) -> String {
    let mut lines = vec![
        "Pipeline registry context: this prompt or tool input mentions registered workflow pipeline code. Read the linked authority docs before changing behavior.".to_string(),
    ];

    for hook_match in matches {
        let record = &hook_match.record;
        let pipeline_id = record
            .id
            .as_deref()
            .or(record.pipeline.as_deref())
            .unwrap_or("unknown");
        let subject = record
            .symbol
            .as_ref()
            .map(|symbol| format!("{pipeline_id}::{symbol}"))
            .unwrap_or_else(|| pipeline_id.to_string());
        lines.push(format!("- {subject}"));
        if let Some(path) = &record.path {
            lines.push(format!("  path: {path}"));
        }
        if let Some(stage) = &record.stage {
            lines.push(format!("  stage: {stage}"));
        }
        if let Some(role) = record.role.as_ref().or(record.summary.as_ref()) {
            lines.push(format!("  role: {role}"));
        }

        let docs = unique_strings(
            record.docs.iter().chain(
                hook_match
                    .pipeline
                    .iter()
                    .flat_map(|pipeline| pipeline.docs.iter()),
            ),
        );
        if !docs.is_empty() {
            lines.push(format!("  docs: {}", docs.into_iter().take(4).join(", ")));
        }

        let tests = unique_strings(
            record.tests.iter().chain(
                hook_match
                    .pipeline
                    .iter()
                    .flat_map(|pipeline| pipeline.tests.iter()),
            ),
        );
        if !tests.is_empty() {
            lines.push(format!("  tests: {}", tests.into_iter().take(4).join(", ")));
        }
    }

    lines.join("\n")
}

fn unique_strings<'a>(values: impl Iterator<Item = &'a String>) -> Vec<&'a str> {
    let mut seen = BTreeSet::new();
    values
        .map(String::as_str)
        .filter(|value| seen.insert(*value))
        .collect()
}

impl HookCooldownState {
    fn load_best_effort(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    fn should_emit(&self, row_key: &str, now_epoch_secs: u64, cooldown_secs: u64) -> bool {
        self.rows
            .get(row_key)
            .map(|row| now_epoch_secs.saturating_sub(row.last_emitted_epoch_secs) >= cooldown_secs)
            .unwrap_or(true)
    }

    fn mark_emitted(
        &mut self,
        row_key: String,
        record: Option<String>,
        now_epoch_secs: u64,
        cooldown_secs: u64,
    ) {
        self.rows.insert(
            row_key,
            HookCooldownRow {
                record,
                last_emitted_epoch_secs: now_epoch_secs,
                cooldown_secs,
            },
        );
    }

    fn store_best_effort(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, content);
        }
    }
}

fn unix_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn resolve_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn problem(
    entry: &RegistryEntry,
    record: Option<&str>,
    problem: impl Into<String>,
) -> RegistryProblem {
    RegistryProblem {
        line: entry.line,
        record: record.map(ToOwned::to_owned),
        problem: problem.into(),
    }
}

impl RegistryRecord {
    fn key(&self) -> Option<String> {
        match self.kind.as_str() {
            "pipeline" => self.id.clone(),
            "function" => match (&self.pipeline, &self.symbol) {
                (Some(pipeline), Some(symbol)) => Some(format!("{pipeline}:{symbol}")),
                (Some(pipeline), None) => Some(pipeline.clone()),
                _ => None,
            },
            _ => self.id.clone().or_else(|| self.pipeline.clone()),
        }
    }

    fn search_text(&self) -> String {
        [
            Some(self.kind.as_str()),
            self.id.as_deref(),
            self.pipeline.as_deref(),
            self.status.as_deref(),
            self.owner_area.as_deref(),
            self.summary.as_deref(),
            self.stage.as_deref(),
            self.path.as_deref(),
            self.symbol.as_deref(),
            self.role.as_deref(),
        ]
        .into_iter()
        .flatten()
        .chain(
            self.docs
                .iter()
                .chain(self.tests.iter())
                .chain(self.bugs.iter())
                .map(String::as_str),
        )
        .join(" ")
        .to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_check_accepts_default_registry() {
        let ctx = CommandContext::new().expect("CommandContext");
        let cmd = PipelineCheck {
            registry: PathBuf::from(DEFAULT_REGISTRY),
            report_only: false,
        };

        let output = check(&ctx, &cmd).expect("default registry should validate");
        match output {
            PipelineOutput::PipelineRegistryCheck {
                passed,
                records,
                problems,
                ..
            } => {
                assert!(passed, "problems: {problems:?}");
                assert!(records > 1, "expected seeded registry records");
                assert!(problems.is_empty());
            }
            other => panic!("expected PipelineRegistryCheck, got {other:?}"),
        }
    }

    #[test]
    fn pipeline_find_matches_registered_symbol() {
        let ctx = CommandContext::new().expect("CommandContext");
        let cmd = PipelineFind {
            query: None,
            path: None,
            symbol: Some("should_wait_for_settled_edit".to_string()),
            registry: PathBuf::from(DEFAULT_REGISTRY),
        };

        let output = find(&ctx, &cmd).expect("find should succeed");
        match output {
            PipelineOutput::PipelineFind { matches, .. } => {
                assert_eq!(matches.len(), 1, "matches: {matches:?}");
                let record = &matches[0];
                assert_eq!(
                    record.pipeline.as_deref(),
                    Some("prototype1.edit_tool_gated_refresh")
                );
                assert_eq!(
                    record.path.as_deref(),
                    Some("crates/ploke-tui/src/llm/manager/session.rs")
                );
            }
            other => panic!("expected PipelineFind, got {other:?}"),
        }
    }

    #[test]
    fn pipeline_hook_context_emits_codex_additional_context() {
        let ctx = CommandContext::new().expect("CommandContext");
        let temp = tempfile::tempdir().expect("tempdir");
        let cmd = PipelineHookContext {
            registry: PathBuf::from(DEFAULT_REGISTRY),
            state_path: temp.path().join("pipeline-hook-state.json"),
            cooldown_secs: DEFAULT_HOOK_COOLDOWN_SECS,
            max_matches: 5,
        };
        let input = serde_json::json!({
            "cwd": ctx.workspace_root().expect("root").display().to_string(),
            "hook_event_name": "UserPromptSubmit",
            "model": "gpt-5.5",
            "permission_mode": "default",
            "prompt": "I want to edit should_wait_for_settled_edit",
            "session_id": "test-session",
            "transcript_path": null,
            "turn_id": "test-turn"
        })
        .to_string();

        let output = cmd
            .output_for_event(&ctx, &input)
            .expect("hook context should execute")
            .expect("registered symbol should produce context");
        let serialized = serde_json::to_value(output).expect("serialize hook output");

        assert_eq!(
            serialized["hookSpecificOutput"]["hookEventName"],
            "UserPromptSubmit"
        );
        let context = serialized["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("additional context");
        assert!(context.contains("prototype1.edit_tool_gated_refresh"));
        assert!(context.contains("should_wait_for_settled_edit"));
        assert!(context.contains("tui-approve-deny-pipeline.md"));

        let suppressed = cmd
            .output_for_event(&ctx, &input)
            .expect("second hook context should execute");
        assert!(
            suppressed.is_none(),
            "same registry row should be suppressed within cooldown"
        );
    }
}
