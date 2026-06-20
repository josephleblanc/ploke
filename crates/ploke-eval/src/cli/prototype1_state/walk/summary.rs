//! Read-only durable campaign summary for operator discovery.
//!
//! This command intentionally does not contact the walk server. It answers the
//! "what already happened here?" question from committed parent identity and
//! campaign artifacts so completed runs remain discoverable after all runtimes
//! have exited.
//!
//! Keep this as a lightweight discovery view. If a future field needs to explain
//! selection authority, child evidence, or sealed History semantics, prefer a
//! typed read model over `EvidenceStore`/`FsEvidenceStore` and `FsBlockStore`
//! rather than adding more ad-hoc JSON parsing here.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::Value as JsonValue;
use toml::Value as TomlValue;

use crate::{
    cli::prototype1_state::{
        cli_facing::campaign_manifest_path_for_id,
        identity::{load_parent_identity, parent_identity_path},
        journal::prototype1_transition_journal_path,
    },
    cli::{InspectOutputFormat, Prototype1StateWalkSummaryCommand},
    spec::PrepareError,
};

use super::paths;

const SUMMARY_SCHEMA_VERSION: &str = "prototype1.walk.summary.v1";

#[derive(Debug, Serialize)]
pub(crate) struct WalkSummary {
    schema_version: &'static str,
    repo_root: PathBuf,
    campaign_id: String,
    active: ActiveSummary,
    policy: PolicySummary,
    progress: ProgressSummary,
    completion: CompletionSummary,
    journal: JournalSummary,
    generations: Vec<GenerationSummary>,
    paths: SummaryPaths,
}

#[derive(Debug, Serialize)]
struct ActiveSummary {
    node_id: String,
    generation: u32,
    branch_id: String,
    previous_parent_id: Option<String>,
    parent_node_id: Option<String>,
}

#[derive(Debug, Default, Serialize)]
struct PolicySummary {
    max_generations: Option<u32>,
    max_total_nodes: Option<u32>,
    child_min: Option<u32>,
    child_max: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ProgressSummary {
    node_count: usize,
    state_reports: usize,
    child_plan_count: usize,
    planned_generations: Vec<u32>,
    latest_generation: u32,
    expected_nodes: Option<usize>,
}

#[derive(Debug, Serialize)]
struct CompletionSummary {
    terminal_condition: String,
    reached_max_generations: bool,
    reached_max_total_nodes: bool,
    expected_fanout_satisfied: bool,
    expected_node_count_satisfied: Option<bool>,
    expected_completion_satisfied: bool,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct JournalSummary {
    path: PathBuf,
    entries: usize,
    latest_cursor: Option<JournalCursor>,
}

#[derive(Debug, Serialize)]
struct JournalCursor {
    index: usize,
    label: String,
    node_id: Option<String>,
    generation: Option<u32>,
}

#[derive(Debug, Serialize)]
struct GenerationSummary {
    parent_node_id: String,
    child_generation: u32,
    child_count: usize,
    children: Vec<ChildSummary>,
    successor_node_id: Option<String>,
    branch_disposition: Option<String>,
    selection_outcome: Option<String>,
    handoff: Option<String>,
    selection_row_hint: Option<usize>,
}

#[derive(Debug, Serialize)]
struct ChildSummary {
    node_id: String,
    branch_id: String,
    target_relpath: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Serialize)]
struct SummaryPaths {
    parent_identity: PathBuf,
    campaign_manifest: PathBuf,
    prototype1_root: PathBuf,
    run_profile: PathBuf,
}

#[derive(Debug)]
struct ReportSummary {
    successor: Option<String>,
    branch_disposition: Option<String>,
    selection_outcome: Option<String>,
    handoff: Option<String>,
}

/// Print a read-only summary for a previously admitted Prototype 1 walk run.
pub(crate) fn run(command: Prototype1StateWalkSummaryCommand) -> Result<(), PrepareError> {
    let repo_root = paths::resolve_repo_root(command.repo_root.as_deref())?;
    let summary = WalkSummary::load(&repo_root)?;
    match command.format {
        InspectOutputFormat::Table => print_table(&summary, command.verbose),
        InspectOutputFormat::Json => print_json(&summary)?,
    }
    Ok(())
}

impl WalkSummary {
    fn load(repo_root: &Path) -> Result<Self, PrepareError> {
        let identity = load_parent_identity(repo_root)?;
        let campaign_id = identity.campaign_id().clone();
        let manifest = campaign_manifest_path_for_id(&campaign_id)?;
        let root = manifest
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("prototype1");
        let profile = root.join("run-profile.toml");
        let policy = load_policy(&profile)?;
        let reports = load_reports(&root)?;
        let generations = load_generations(&root, &reports)?;
        let journal_path = prototype1_transition_journal_path(&manifest);
        let journal = load_journal(&journal_path)?;
        let node_count = count_dirs(&root.join("nodes"))?;
        let state_reports = reports.len();
        let child_plan_count = generations.len();
        let planned_generations = generations
            .iter()
            .map(|generation| generation.child_generation)
            .collect::<Vec<_>>();
        let latest_generation = identity.generation();
        let expected_nodes = expected_nodes(&policy);
        let progress = ProgressSummary {
            node_count,
            state_reports,
            child_plan_count,
            planned_generations,
            latest_generation,
            expected_nodes,
        };
        let completion = completion(&policy, &progress, &generations);
        Ok(Self {
            schema_version: SUMMARY_SCHEMA_VERSION,
            repo_root: repo_root.to_path_buf(),
            campaign_id: campaign_id.to_string(),
            active: ActiveSummary {
                node_id: identity.node_id().to_string(),
                generation: identity.generation(),
                branch_id: identity.branch_id().to_string(),
                previous_parent_id: identity.previous_parent_id().map(str::to_string),
                parent_node_id: identity.parent_node_id().map(str::to_string),
            },
            policy,
            progress,
            completion,
            journal,
            generations,
            paths: SummaryPaths {
                parent_identity: parent_identity_path(repo_root),
                campaign_manifest: manifest,
                prototype1_root: root,
                run_profile: profile,
            },
        })
    }
}

fn print_table(summary: &WalkSummary, verbose: bool) {
    let campaign_dir = campaign_dir(summary);
    println!("walk summary");
    println!("----------------------------------------");
    println!("campaign_id: {}", summary.campaign_id);
    println!("repo_root: {}", summary.repo_root.display());
    println!("campaign_dir: {}", display_dir(&campaign_dir));
    println!("active_node: {}", summary.active.node_id);
    println!("active_generation: {}", summary.active.generation);
    println!("active_branch: {}", summary.active.branch_id);
    println!("terminal: {}", summary.completion.terminal_condition);
    println!(
        "completion: {}",
        yes(summary.completion.expected_completion_satisfied)
    );
    if !summary.completion.blockers.is_empty() {
        println!("blockers:");
        for blocker in &summary.completion.blockers {
            println!("  - {blocker}");
        }
    }
    println!("policy:");
    println!(
        "  max_generations: {}",
        display_opt(summary.policy.max_generations)
    );
    println!(
        "  max_total_nodes: {}",
        display_opt(summary.policy.max_total_nodes)
    );
    println!(
        "  child_budget: {}..{}",
        display_opt(summary.policy.child_min),
        display_opt(summary.policy.child_max)
    );
    println!("progress:");
    println!("  nodes: {}", summary.progress.node_count);
    println!("  state_reports: {}", summary.progress.state_reports);
    println!("  child_plans: {}", summary.progress.child_plan_count);
    println!(
        "  expected_nodes: {}",
        summary
            .progress
            .expected_nodes
            .map(|count| count.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    println!(
        "  expected_fanout: {}",
        yes(summary.completion.expected_fanout_satisfied)
    );
    println!("journal:");
    println!(
        "  path: {}",
        display_under(&summary.journal.path, &campaign_dir, "campaign_dir")
    );
    println!("  entries: {}", summary.journal.entries);
    if let Some(cursor) = &summary.journal.latest_cursor {
        println!("  latest_cursor: #{} {}", cursor.index, cursor.label);
    } else {
        println!("  latest_cursor: -");
    }
    println!("generations:");
    if summary.generations.is_empty() {
        println!("  (no child plans found)");
    }
    for generation in &summary.generations {
        println!(
            "  gen{} parent={} children={} successor={} branch={} decision={} handoff={}",
            generation.child_generation,
            generation.parent_node_id,
            generation.child_count,
            generation.successor_node_id.as_deref().unwrap_or("-"),
            generation.branch_disposition.as_deref().unwrap_or("-"),
            generation.selection_outcome.as_deref().unwrap_or("-"),
            generation.handoff.as_deref().unwrap_or("-"),
        );
    }
    if verbose {
        print_verbose(summary);
    }
}

fn campaign_dir(summary: &WalkSummary) -> PathBuf {
    summary
        .paths
        .campaign_manifest
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn display_dir(path: &Path) -> String {
    let mut value = path.display().to_string();
    if !value.ends_with(std::path::MAIN_SEPARATOR) {
        value.push(std::path::MAIN_SEPARATOR);
    }
    value
}

fn display_under(path: &Path, root: &Path, label: &str) -> String {
    if let Ok(stripped) = path.strip_prefix(root) {
        if stripped.as_os_str().is_empty() {
            return format!("{{{label}}}/");
        }
        return format!("{{{label}}}/{}", stripped.display());
    }
    path.display().to_string()
}

fn print_verbose(summary: &WalkSummary) {
    println!();
    println!("field guide:");
    println!("  branch: selected branch disposition from the parent final report when available.");
    println!(
        "  decision: candidate-local successor selection outcome; decision=Stop does not by itself mean the campaign stopped."
    );
    println!("  handoff: selected-successor handoff status from the parent final report.");
    println!(
        "  latest_cursor: latest durable transition-journal entry, not a per-operator replay cursor."
    );
    println!("  selection_row_hint: row index to try with the sealed History selection inspector.");
    println!();
    println!("selection context:");
    if summary.generations.is_empty() {
        println!("  (no generation selection context found)");
    }
    for generation in &summary.generations {
        println!("  gen{}:", generation.child_generation);
        println!(
            "    selected_successor: {}",
            generation.successor_node_id.as_deref().unwrap_or("-")
        );
        println!(
            "    branch: {}",
            generation.branch_disposition.as_deref().unwrap_or("-")
        );
        println!(
            "    decision: {}",
            generation.selection_outcome.as_deref().unwrap_or("-")
        );
        println!(
            "    handoff: {}",
            generation.handoff.as_deref().unwrap_or("-")
        );
        if rejected_handoff(generation) {
            println!(
                "    note: rejected selected branch still has acknowledged handoff; inspect sealed selection for the exact continuation disposition."
            );
        } else if accepted_handoff(generation) {
            println!("    note: accepted selected branch has acknowledged handoff.");
        }
        if let Some(row) = generation.selection_row_hint {
            println!("    selection_row_hint: {row}");
            println!(
                "    inspect_selection: ploke-eval history --repo-root {} selection-show --row {row} --replay",
                summary.repo_root.display()
            );
        }
        println!(
            "    review_scores: ploke-eval history --repo-root {} score-selection-review --generation {}",
            summary.repo_root.display(),
            generation.child_generation
        );
    }
    println!();
    println!("storage note:");
    println!(
        "  Detailed selection evidence lives behind the typed history/evidence inspection commands; use the inspect commands above instead of opening artifact JSON by hand."
    );
}

fn accepted_handoff(generation: &GenerationSummary) -> bool {
    generation
        .branch_disposition
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("keep"))
        && generation
            .handoff
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("acknowledged"))
}

fn rejected_handoff(generation: &GenerationSummary) -> bool {
    generation
        .branch_disposition
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("reject"))
        && generation
            .handoff
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("acknowledged"))
}

fn print_json(summary: &WalkSummary) -> Result<(), PrepareError> {
    let json = serde_json::to_string_pretty(summary).map_err(PrepareError::Serialize)?;
    println!("{json}");
    Ok(())
}

fn load_policy(path: &Path) -> Result<PolicySummary, PrepareError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(PolicySummary::default());
        }
        Err(source) => {
            return Err(PrepareError::ReadManifest {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let value: TomlValue = toml::from_str(&text).map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_walk_summary_profile_parse",
        detail: format!("failed to parse run profile '{}': {source}", path.display()),
    })?;
    let search = value.get("search");
    let children = search.and_then(|value| value.get("children"));
    Ok(PolicySummary {
        max_generations: toml_u32(search, "max_generations"),
        max_total_nodes: toml_u32(search, "max_total_nodes"),
        child_min: toml_u32(children, "min"),
        child_max: toml_u32(children, "max"),
    })
}

fn load_reports(root: &Path) -> Result<BTreeMap<String, ReportSummary>, PrepareError> {
    let mut reports = BTreeMap::new();
    let nodes = root.join("nodes");
    for entry in read_dir_optional(&nodes)? {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: nodes.clone(),
            source,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source| PrepareError::ReadManifest {
                path: entry.path(),
                source,
            })?;
        if !file_type.is_dir() {
            continue;
        }
        let parent = entry.file_name().to_string_lossy().to_string();
        let path = entry.path().join("reports").join("state-report.json");
        let Some(value) = read_json_optional(&path)? else {
            continue;
        };
        let outcome = string_field(&value, "outcome");
        let parts = outcome.as_deref().map(parse_outcome).unwrap_or_default();
        reports.insert(
            parent,
            ReportSummary {
                successor: string_field(&value, "node_id"),
                branch_disposition: report_branch_disposition(outcome.as_deref()),
                selection_outcome: parts.get("selection").cloned(),
                handoff: parts.get("successor_handoff").cloned(),
            },
        );
    }
    Ok(reports)
}

fn load_generations(
    root: &Path,
    reports: &BTreeMap<String, ReportSummary>,
) -> Result<Vec<GenerationSummary>, PrepareError> {
    let dir = root.join("messages").join("child-plan");
    let mut generations = Vec::new();
    for entry in read_dir_optional(&dir)? {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: dir.clone(),
            source,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source| PrepareError::ReadManifest {
                path: entry.path(),
                source,
            })?;
        if !file_type.is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let value = read_json(&path)?;
        let parent = string_field(&value, "parent_node_id").unwrap_or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("unknown-parent")
                .to_string()
        });
        let child_generation = u32_field(&value, "child_generation").unwrap_or(0);
        let children = value
            .get("children")
            .and_then(JsonValue::as_array)
            .map(|children| children.iter().map(child_summary).collect::<Vec<_>>())
            .unwrap_or_default();
        let report = reports.get(&parent);
        generations.push(GenerationSummary {
            parent_node_id: parent,
            child_generation,
            child_count: children.len(),
            children,
            successor_node_id: report.and_then(|report| report.successor.clone()),
            branch_disposition: report.and_then(|report| report.branch_disposition.clone()),
            selection_outcome: report.and_then(|report| report.selection_outcome.clone()),
            handoff: report.and_then(|report| report.handoff.clone()),
            selection_row_hint: None,
        });
    }
    generations.sort_by_key(|generation| generation.child_generation);
    for (index, generation) in generations.iter_mut().enumerate() {
        generation.selection_row_hint = Some(index);
    }
    Ok(generations)
}

fn child_summary(value: &JsonValue) -> ChildSummary {
    let node = value.get("node").unwrap_or(value);
    ChildSummary {
        node_id: string_field(node, "node_id").unwrap_or_else(|| "-".to_string()),
        branch_id: string_field(node, "branch_id").unwrap_or_else(|| "-".to_string()),
        target_relpath: string_field(node, "target_relpath"),
        status: string_field(node, "status"),
    }
}

fn load_journal(path: &Path) -> Result<JournalSummary, PrepareError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(JournalSummary {
                path: path.to_path_buf(),
                entries: 0,
                latest_cursor: None,
            });
        }
        Err(source) => {
            return Err(PrepareError::ReadManifest {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let mut latest = None;
    let mut count = 0usize;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: JsonValue =
            serde_json::from_str(line).map_err(|source| PrepareError::ParseManifest {
                path: path.to_path_buf(),
                source,
            })?;
        latest = Some(cursor_from_entry(count, &value));
        count += 1;
    }
    Ok(JournalSummary {
        path: path.to_path_buf(),
        entries: count,
        latest_cursor: latest,
    })
}

fn cursor_from_entry(index: usize, value: &JsonValue) -> JournalCursor {
    let kind = string_field(value, "kind").unwrap_or_else(|| "unknown".to_string());
    let state = value.get("state");
    let label = match (kind.as_str(), state) {
        ("parent_started", _) => "r5 parent_started".to_string(),
        ("resource.parentstart", _) => "r5 resource.parentstart".to_string(),
        ("resource.parentcomplete", _) => "r14a resource.parentcomplete".to_string(),
        ("successor_handoff", _) => "r13b successor_handoff".to_string(),
        ("successor", Some(state)) if state.get("selected").is_some() => {
            "r13b successor.selected".to_string()
        }
        ("successor", Some(state)) if state.get("checkout").is_some() => {
            "r13b successor.checkout".to_string()
        }
        ("successor", Some(state)) if state.get("spawned").is_some() => {
            "r13b successor.spawned".to_string()
        }
        ("successor", Some(state)) if state.get("ready").is_some() => {
            "r13b successor.ready".to_string()
        }
        ("successor", Some(state)) if state.get("completed").is_some() => {
            "r14b successor.completed".to_string()
        }
        _ => kind.clone(),
    };
    JournalCursor {
        index,
        label,
        node_id: string_field(value, "node_id")
            .or_else(|| nested_string(value, &["refs", "node_id"])),
        generation: u32_field(value, "generation")
            .or_else(|| nested_u32(value, &["refs", "generation"])),
    }
}

fn completion(
    policy: &PolicySummary,
    progress: &ProgressSummary,
    generations: &[GenerationSummary],
) -> CompletionSummary {
    let reached_max_generations = policy
        .max_generations
        .map(|max| progress.latest_generation >= max)
        .unwrap_or(false);
    let reached_max_total_nodes = policy
        .max_total_nodes
        .map(|max| progress.node_count >= max as usize)
        .unwrap_or(false);
    let fanout = expected_fanout(policy, generations);
    let node_count = progress
        .expected_nodes
        .map(|expected| progress.node_count == expected);
    let mut blockers = Vec::new();
    if !reached_max_generations && !reached_max_total_nodes {
        blockers.push("no terminal policy threshold reached".to_string());
    }
    if !fanout {
        blockers
            .push("one or more child plans do not satisfy the configured child budget".to_string());
    }
    if matches!(node_count, Some(false)) {
        blockers
            .push("durable node count does not match the strict expected node count".to_string());
    }
    if progress.state_reports < progress.child_plan_count {
        blockers.push("not every child-plan generation has a final state report".to_string());
    }
    let terminal_condition = if reached_max_generations {
        "reached max_generations".to_string()
    } else if reached_max_total_nodes {
        "reached max_total_nodes".to_string()
    } else {
        "not terminal".to_string()
    };
    let expected_completion_satisfied = blockers.is_empty();
    CompletionSummary {
        terminal_condition,
        reached_max_generations,
        reached_max_total_nodes,
        expected_fanout_satisfied: fanout,
        expected_node_count_satisfied: node_count,
        expected_completion_satisfied,
        blockers,
    }
}

fn expected_fanout(policy: &PolicySummary, generations: &[GenerationSummary]) -> bool {
    let Some(min) = policy.child_min else {
        return true;
    };
    let Some(max) = policy.child_max else {
        return true;
    };
    if let Some(expected) = policy.max_generations {
        if generations.len() < expected as usize {
            return false;
        }
    }
    generations.iter().all(|generation| {
        let count = generation.child_count as u32;
        count >= min && count <= max
    })
}

fn expected_nodes(policy: &PolicySummary) -> Option<usize> {
    let generations = policy.max_generations? as usize;
    let min = policy.child_min?;
    let max = policy.child_max?;
    if min == max {
        Some(1 + generations * max as usize)
    } else {
        None
    }
}

fn count_dirs(path: &Path) -> Result<usize, PrepareError> {
    let mut count = 0;
    for entry in read_dir_optional(path)? {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source| PrepareError::ReadManifest {
                path: entry.path(),
                source,
            })?;
        if file_type.is_dir() {
            count += 1;
        }
    }
    Ok(count)
}

fn read_dir_optional(path: &Path) -> Result<fs::ReadDir, PrepareError> {
    fs::read_dir(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn read_json_optional(path: &Path) -> Result<Option<JsonValue>, PrepareError> {
    match fs::read_to_string(path) {
        Ok(text) => {
            serde_json::from_str(&text)
                .map(Some)
                .map_err(|source| PrepareError::ParseManifest {
                    path: path.to_path_buf(),
                    source,
                })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn read_json(path: &Path) -> Result<JsonValue, PrepareError> {
    read_json_optional(path)?.ok_or_else(|| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::NotFound, "file not found"),
    })
}

fn report_branch_disposition(outcome: Option<&str>) -> Option<String> {
    let prefix = outcome?.split(';').next()?;
    let value = prefix.strip_prefix("completed:")?;
    match value {
        "Keep" => Some("keep".to_string()),
        "Reject" => Some("reject".to_string()),
        other => Some(other.to_ascii_lowercase()),
    }
}

fn parse_outcome(outcome: &str) -> BTreeMap<String, String> {
    outcome
        .split(';')
        .filter_map(|part| part.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn string_field(value: &JsonValue, field: &str) -> Option<String> {
    value.get(field)?.as_str().map(str::to_string)
}

fn u32_field(value: &JsonValue, field: &str) -> Option<u32> {
    value
        .get(field)?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
}

fn nested_string(value: &JsonValue, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str().map(str::to_string)
}

fn nested_u32(value: &JsonValue, path: &[&str]) -> Option<u32> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_u64().and_then(|value| u32::try_from(value).ok())
}

fn toml_u32(value: Option<&TomlValue>, field: &str) -> Option<u32> {
    value?
        .get(field)?
        .as_integer()
        .and_then(|value| u32::try_from(value).ok())
}

fn display_opt(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn yes(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_under_uses_campaign_dir_placeholder() {
        let campaign_dir = PathBuf::from("/tmp/eval/campaigns/run-1");
        let journal = campaign_dir.join("prototype1/transition-journal.jsonl");

        let rendered = display_under(&journal, &campaign_dir, "campaign_dir");

        assert_eq!(
            rendered,
            "{campaign_dir}/prototype1/transition-journal.jsonl"
        );
    }
}
