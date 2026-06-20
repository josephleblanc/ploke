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
    collections::{BTreeMap, BTreeSet},
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
    #[serde(skip)]
    parent_nodes: usize,
    state_reports: usize,
    child_plan_count: usize,
    #[serde(skip)]
    rejected_attempts: usize,
    planned_generations: Vec<u32>,
    latest_generation: u32,
    #[serde(skip)]
    expected_children: Option<usize>,
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
    meaning: String,
    node_id: Option<String>,
    generation: Option<u32>,
}

#[derive(Debug, Serialize)]
struct GenerationSummary {
    parent_node_id: String,
    child_generation: u32,
    child_count: usize,
    #[serde(skip)]
    rejected_attempts: usize,
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
        let parent_nodes = parent_node_count(&reports, &generations);
        let rejected_attempts = total_rejections(&generations);
        let planned_generations = generations
            .iter()
            .map(|generation| generation.child_generation)
            .collect::<Vec<_>>();
        let latest_generation = identity.generation();
        let expected_children = expected_children(&policy);
        let expected_nodes = expected_nodes(&policy);
        let progress = ProgressSummary {
            node_count,
            parent_nodes,
            state_reports,
            child_plan_count,
            rejected_attempts,
            planned_generations,
            latest_generation,
            expected_children,
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
    println!("outcome:");
    println!(
        "  campaign_terminal: {} ({})",
        yes(summary.completion.reached_max_generations
            || summary.completion.reached_max_total_nodes),
        summary.completion.terminal_condition
    );
    println!("  parent_turn: {}", parent_turn_status(summary));
    println!(
        "  strict_completion: {}",
        yes(summary.completion.expected_completion_satisfied)
    );
    if !summary.completion.blockers.is_empty() {
        println!("strict_completion_blockers:");
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
    println!("  expected:");
    println!(
        "    children: {}",
        display_count(summary.progress.expected_children)
    );
    println!(
        "    total_nodes: {}",
        display_count(summary.progress.expected_nodes)
    );
    println!("  actual:");
    println!("    parent_nodes: {}", summary.progress.parent_nodes);
    println!(
        "    admitted_children: {}",
        total_admitted_children(summary)
    );
    println!("    total_nodes: {}", summary.progress.node_count);
    println!("  admission:");
    println!("    records: {}", summary.progress.child_plan_count);
    println!("    admitted: {}", total_admitted_children(summary));
    println!(
        "    rejected_attempts: {}",
        summary.progress.rejected_attempts
    );
    println!(
        "    budget_satisfied: {} ({})",
        yes(summary.completion.expected_fanout_satisfied),
        child_budget_expectation(summary)
    );
    print_progress_mismatch(summary);
    println!("journal:");
    println!(
        "  path: {}",
        display_under(&summary.journal.path, &campaign_dir, "campaign_dir")
    );
    println!("  entries: {}", summary.journal.entries);
    if let Some(cursor) = &summary.journal.latest_cursor {
        println!("  latest_entry: #{} {}", cursor.index, cursor.label);
        println!("  latest_meaning: {}", cursor.meaning);
    } else {
        println!("  latest_entry: -");
    }
    println!("generations:");
    if summary.generations.is_empty() {
        println!("  (no child plans found)");
    }
    for generation in &summary.generations {
        println!(
            "  gen{} parent={} admitted_children={} rejected_admission_attempts={} child_budget={} selected_successor={} branch={} decision={} handoff={}",
            generation.child_generation,
            generation.parent_node_id,
            generation.child_count,
            generation.rejected_attempts,
            child_budget_status(summary, generation),
            selected_successor(generation),
            generation.branch_disposition.as_deref().unwrap_or("-"),
            generation.selection_outcome.as_deref().unwrap_or("-"),
            generation.handoff.as_deref().unwrap_or("-"),
        );
    }
    if verbose {
        print_verbose(summary);
    }
}

fn print_progress_mismatch(summary: &WalkSummary) {
    let admitted = total_admitted_children(summary);
    let mut lines = Vec::new();
    if let Some(expected) = summary.progress.expected_children {
        if admitted != expected {
            lines.push(format!("expected {expected} child, admitted {admitted}"));
        }
    }
    if let Some(expected) = summary.progress.expected_nodes {
        let found = summary.progress.node_count;
        if found != expected {
            lines.push(format!(
                "expected {expected} persisted nodes, found {found}"
            ));
        }
    }

    println!("  mismatch:");
    if lines.is_empty() {
        println!("    - none");
    } else {
        for line in lines {
            println!("    - {line}");
        }
    }
}

fn parent_turn_status(summary: &WalkSummary) -> String {
    match summary.journal.latest_cursor.as_ref() {
        Some(cursor) if cursor.label.starts_with("r14a") || cursor.label.starts_with("r14b") => {
            format!("complete ({})", cursor.label)
        }
        Some(cursor) => format!("not complete in latest journal entry ({})", cursor.label),
        None => "unknown (journal empty)".to_string(),
    }
}

fn total_admitted_children(summary: &WalkSummary) -> usize {
    total_children(&summary.generations)
}

fn total_children(generations: &[GenerationSummary]) -> usize {
    generations
        .iter()
        .map(|generation| generation.child_count)
        .sum()
}

fn total_rejections(generations: &[GenerationSummary]) -> usize {
    generations
        .iter()
        .map(|generation| generation.rejected_attempts)
        .sum()
}

fn parent_node_count(
    reports: &BTreeMap<String, ReportSummary>,
    generations: &[GenerationSummary],
) -> usize {
    let mut parents = reports.keys().cloned().collect::<BTreeSet<_>>();
    parents.extend(
        generations
            .iter()
            .map(|generation| generation.parent_node_id.clone()),
    );
    parents.len()
}

fn child_budget_expectation(summary: &WalkSummary) -> String {
    match (summary.policy.child_min, summary.policy.child_max) {
        (Some(min), Some(max)) => {
            format!("expected {min}..{max} admitted children per child admission record")
        }
        _ => "no child budget configured".to_string(),
    }
}

fn child_budget_status(summary: &WalkSummary, generation: &GenerationSummary) -> &'static str {
    match (summary.policy.child_min, summary.policy.child_max) {
        (Some(min), Some(max)) => {
            let count = generation.child_count as u32;
            if count >= min && count <= max {
                "ok"
            } else {
                "unmet"
            }
        }
        _ => "unknown",
    }
}

fn selected_successor(generation: &GenerationSummary) -> &str {
    if generation
        .selection_outcome
        .as_deref()
        .is_none_or(|outcome| outcome.eq_ignore_ascii_case("none"))
        && generation.branch_disposition.is_none()
        && generation.handoff.is_none()
    {
        return "-";
    }
    generation.successor_node_id.as_deref().unwrap_or("-")
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
        "  latest_entry: latest durable transition-journal entry; replay cursor is separate and operator-local."
    );
    println!(
        "  progress.expected: policy-derived child/node counts if the configured child budget is met."
    );
    println!(
        "  progress.actual: persisted parent/admitted-child node counts found in campaign artifacts."
    );
    println!(
        "  progress.admission.records: persisted child admission records, one per parent child-plan file."
    );
    println!(
        "  progress.admission.rejected_attempts: rejected edit/admission attempts stored in child admission records."
    );
    println!(
        "  strict_completion: whether durable artifacts satisfy policy/budget/count expectations, not whether the parent turn emitted a final report."
    );
    println!("  selection_row_hint: row index to try with the sealed History selection inspector.");
    println!();
    println!("selection context:");
    if summary.generations.is_empty() {
        println!("  (no generation selection context found)");
    }
    for generation in &summary.generations {
        println!("  gen{}:", generation.child_generation);
        println!("    selected_successor: {}", selected_successor(generation));
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
        let rejections = value
            .get("rejected_surface_attempts")
            .and_then(JsonValue::as_array)
            .map_or(0, Vec::len);
        let report = reports.get(&parent);
        generations.push(GenerationSummary {
            parent_node_id: parent,
            child_generation,
            child_count: children.len(),
            rejected_attempts: rejections,
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
    let phase = string_field(value, "phase");
    let state = value.get("state");
    let (label, meaning) = match (kind.as_str(), phase.as_deref(), state) {
        ("parent_started", _, _) => (
            "r5 parent_started".to_string(),
            "parent runtime started; this is startup evidence, not child fanout or final completion".to_string(),
        ),
        ("resource", Some("parent_start"), _) | ("resource.parentstart", _, _) => (
            "r5 parent_start_resource".to_string(),
            "parent-start resource was recorded for reconstruction/audit evidence".to_string(),
        ),
        ("resource", Some("parent_complete"), _) | ("resource.parentcomplete", _, _) => (
            "r14a parent_complete".to_string(),
            "parent turn reached final report/parent-complete evidence; campaign may still be non-terminal".to_string(),
        ),
        ("successor_handoff", _, _) => (
            "r13b successor_handoff".to_string(),
            "successor handoff was recorded".to_string(),
        ),
        ("successor", _, Some(state)) if state.get("selected").is_some() => (
            "r13b successor.selected".to_string(),
            "successor selection was recorded".to_string(),
        ),
        ("successor", _, Some(state)) if state.get("checkout").is_some() => (
            "r13b successor.checkout".to_string(),
            "selected successor checkout was installed".to_string(),
        ),
        ("successor", _, Some(state)) if state.get("spawned").is_some() => (
            "r13b successor.spawned".to_string(),
            "successor runtime was spawned".to_string(),
        ),
        ("successor", _, Some(state)) if state.get("ready").is_some() => (
            "r13b successor.ready".to_string(),
            "successor runtime reported ready".to_string(),
        ),
        ("successor", _, Some(state)) if state.get("completed").is_some() => (
            "r14b successor.completed".to_string(),
            "successor handoff parent emitted final completion evidence".to_string(),
        ),
        _ => (kind.clone(), "durable transition-journal evidence".to_string()),
    };
    JournalCursor {
        index,
        label,
        meaning,
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
        blockers.push(
            "one or more child admission records do not satisfy the configured child budget"
                .to_string(),
        );
    }
    if matches!(node_count, Some(false)) {
        if let Some(expected) = progress.expected_nodes {
            blockers.push(format!(
                "expected {expected} persisted nodes if the child budget was met; found {}",
                progress.node_count
            ));
        }
    }
    if progress.state_reports < progress.child_plan_count {
        blockers.push("not every child admission record has a parent turn report".to_string());
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

fn expected_children(policy: &PolicySummary) -> Option<usize> {
    let generations = policy.max_generations? as usize;
    let min = policy.child_min?;
    let max = policy.child_max?;
    if min == max {
        Some(generations * max as usize)
    } else {
        None
    }
}

fn expected_nodes(policy: &PolicySummary) -> Option<usize> {
    expected_children(policy).map(|children| 1 + children)
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

fn display_count(value: Option<usize>) -> String {
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

    #[test]
    fn parent_complete_cursor_explains_non_terminal_campaign() {
        for value in [
            serde_json::json!({
                "kind": "resource.parentcomplete",
                "node_id": "node-root"
            }),
            serde_json::json!({
                "kind": "resource",
                "phase": "parent_complete",
                "node_id": "node-root"
            }),
        ] {
            let cursor = cursor_from_entry(2, &value);

            assert_eq!(cursor.label, "r14a parent_complete");
            assert!(
                cursor
                    .meaning
                    .contains("campaign may still be non-terminal")
            );
        }
    }

    #[test]
    fn no_selection_generation_does_not_display_parent_as_successor() {
        let generation = GenerationSummary {
            parent_node_id: "node-root".to_string(),
            child_generation: 1,
            child_count: 0,
            rejected_attempts: 0,
            children: Vec::new(),
            successor_node_id: Some("node-root".to_string()),
            branch_disposition: None,
            selection_outcome: Some("none".to_string()),
            handoff: None,
            selection_row_hint: None,
        };

        assert_eq!(selected_successor(&generation), "-");
    }
}
