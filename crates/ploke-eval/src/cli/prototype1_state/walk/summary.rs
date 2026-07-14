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

use ploke_records::ids::CampaignId;
use serde::Serialize;
use serde_json::Value as JsonValue;
use toml::Value as TomlValue;

use crate::{
    campaign_manifest_path,
    cli::{
        InspectOutputFormat, Prototype1StateWalkSummaryCommand,
        prototype1_state::{
            event::RuntimeId,
            identity::{load_parent_identity, parent_identity_path},
            journal::{self, JournalEntry, prototype1_transition_journal_path},
            successor,
        },
    },
    spec::PrepareError,
};

use super::{paths, phase::WalkPhase};

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

type AttemptKey = (CampaignId, String, RuntimeId);
type ParentKey = (CampaignId, String, String, u32);
type SelectionKey = (CampaignId, String);

#[derive(Default)]
struct JournalProjection {
    spawns: BTreeMap<AttemptKey, successor::Record>,
    acknowledged: BTreeMap<AttemptKey, journal::SuccessorHandoffEntry>,
    failed: BTreeSet<AttemptKey>,
    checkouts: BTreeMap<SelectionKey, journal::ActiveCheckoutAdvancedEntry>,
    parents: BTreeMap<ParentKey, WalkPhase>,
    latest: Option<WalkPhase>,
}

impl JournalProjection {
    fn observe(&mut self, entry: &JournalEntry) {
        match entry {
            JournalEntry::ActiveCheckoutAdvanced(entry) => {
                let Some(predecessor) = entry.previous_parent_identity.as_ref() else {
                    return;
                };
                if predecessor.campaign_id() != &entry.campaign_id
                    || entry.selected_parent_identity.campaign_id() != &entry.campaign_id
                {
                    return;
                }
                self.checkouts.insert(
                    (
                        entry.campaign_id.clone(),
                        entry.selected_parent_identity.node_id().to_string(),
                    ),
                    entry.clone(),
                );
            }
            JournalEntry::Successor(record) => {
                let Some(key) = summary_attempt_key(record) else {
                    return;
                };
                match &record.state {
                    successor::State::Spawned { .. } => {
                        self.spawns.insert(key.clone(), record.clone());
                        let phase = if self.is_acknowledged(&key) {
                            WalkPhase::R13b
                        } else {
                            WalkPhase::R13c
                        };
                        self.mark_parent(&key, phase);
                    }
                    successor::State::Ready { .. } => {
                        let phase = if self.is_acknowledged(&key) {
                            WalkPhase::R13b
                        } else {
                            WalkPhase::R13c
                        };
                        self.mark_parent(&key, phase);
                    }
                    successor::State::TimedOut { .. }
                    | successor::State::ExitedBeforeReady { .. } => {
                        self.failed.insert(key.clone());
                        self.acknowledged.remove(&key);
                        self.mark_parent(&key, WalkPhase::R13c);
                    }
                    successor::State::Selected { .. }
                    | successor::State::Stopped { .. }
                    | successor::State::Checkout { .. }
                    | successor::State::Completed { .. } => {}
                }
            }
            JournalEntry::SuccessorHandoff(entry) => {
                let key = (
                    entry.campaign_id.clone(),
                    entry.node_id.clone(),
                    entry.runtime_id,
                );
                if !self.failed.contains(&key) && self.spawn_matches_handoff(&key, entry) {
                    self.acknowledged.insert(key.clone(), entry.clone());
                    self.mark_parent(&key, WalkPhase::R13b);
                } else {
                    self.acknowledged.remove(&key);
                    self.mark_parent(&key, WalkPhase::R13c);
                }
            }
            _ => {}
        }
    }

    fn is_acknowledged(&self, key: &AttemptKey) -> bool {
        !self.failed.contains(key)
            && self
                .acknowledged
                .get(key)
                .is_some_and(|entry| self.spawn_matches_handoff(key, entry))
    }

    fn spawn_matches_handoff(
        &self,
        key: &AttemptKey,
        entry: &journal::SuccessorHandoffEntry,
    ) -> bool {
        let Some(spawn) = self.spawns.get(key) else {
            return false;
        };
        let successor::State::Spawned {
            pid,
            incarnation: _,
            active_parent_root,
            binary_path,
            invocation_path,
            ready_path,
            streams,
        } = &spawn.state
        else {
            return false;
        };
        let selected = (entry.campaign_id.clone(), entry.node_id.clone());
        let Some(checkout) = self.checkouts.get(&selected) else {
            return false;
        };

        spawn.campaign_id == entry.campaign_id
            && spawn.node_id == entry.node_id
            && spawn.runtime_id == Some(entry.runtime_id)
            && key.0 == entry.campaign_id
            && key.1 == entry.node_id
            && key.2 == entry.runtime_id
            && pid == &entry.pid
            && active_parent_root == &entry.active_parent_root
            && binary_path == &entry.binary_path
            && invocation_path == &entry.invocation_path
            && ready_path == &entry.ready_path
            && entry
                .streams
                .as_ref()
                .is_none_or(|handoff_streams| handoff_streams == streams)
            && checkout.campaign_id == entry.campaign_id
            && checkout.selected_parent_identity.campaign_id() == &entry.campaign_id
            && checkout.selected_parent_identity.node_id() == entry.node_id
            && checkout.active_parent_root == entry.active_parent_root
    }

    fn mark_parent(&mut self, key: &AttemptKey, phase: WalkPhase) {
        let selected = (key.0.clone(), key.1.clone());
        let parent = self
            .checkouts
            .get(&selected)
            .and_then(|entry| entry.previous_parent_identity.as_ref())
            .map(|identity| {
                (
                    identity.campaign_id().clone(),
                    identity.parent_id().to_string(),
                    identity.node_id().to_string(),
                    identity.generation(),
                )
            });
        if let Some(parent) = parent {
            self.parents.insert(parent, phase);
        }
    }

    fn handoff_phase(&self, entry: &journal::SuccessorHandoffEntry) -> WalkPhase {
        let key = (
            entry.campaign_id.clone(),
            entry.node_id.clone(),
            entry.runtime_id,
        );
        if self.is_acknowledged(&key) {
            WalkPhase::R13b
        } else {
            WalkPhase::R13c
        }
    }

    fn parent_phase(&self, sample: &journal::resource::Sample) -> WalkPhase {
        let key = (
            sample.campaign_id.clone(),
            sample.parent_id.clone(),
            sample.node_id.clone(),
            sample.generation,
        );
        match self.parents.get(&key) {
            Some(WalkPhase::R13b | WalkPhase::R14b) => WalkPhase::R14b,
            Some(WalkPhase::R13c) => WalkPhase::R13c,
            _ => WalkPhase::R14a,
        }
    }
}

fn summary_attempt_key(record: &successor::Record) -> Option<AttemptKey> {
    record.runtime_id.map(|runtime_id| {
        (
            record.campaign_id.clone(),
            record.node_id.clone(),
            runtime_id,
        )
    })
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
        let manifest = campaign_manifest_path(&campaign_id)?;
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
    let mut projection = JournalProjection::default();
    let mut latest = None;
    let mut count = 0usize;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: JsonValue =
            serde_json::from_str(line).map_err(|source| PrepareError::ParseManifest {
                path: path.to_path_buf(),
                source,
            })?;
        let kind = string_field(&value, "kind");
        let entry = if matches!(
            kind.as_deref(),
            Some("resource.parentstart" | "resource.parentcomplete")
        ) {
            None
        } else {
            Some(serde_json::from_value(value.clone()).map_err(|source| {
                PrepareError::ParseManifest {
                    path: path.to_path_buf(),
                    source,
                }
            })?)
        };
        let cursor = cursor_from_entry(count, &value, entry.as_ref(), &mut projection);
        latest = Some(cursor);
        count += 1;
    }
    Ok(JournalSummary {
        path: path.to_path_buf(),
        entries: count,
        latest_cursor: latest,
    })
}

fn cursor_from_entry(
    index: usize,
    value: &JsonValue,
    entry: Option<&JournalEntry>,
    projection: &mut JournalProjection,
) -> JournalCursor {
    if let Some(entry) = entry {
        projection.observe(entry);
    }
    let kind = string_field(value, "kind").unwrap_or_else(|| "unknown".to_string());
    let (label, meaning, phase) = match (kind.as_str(), entry) {
        ("resource.parentstart", None) => (
            "r5 parent_start_resource".to_string(),
            "parent-start resource was recorded for reconstruction/audit evidence".to_string(),
            WalkPhase::R5,
        ),
        ("resource.parentcomplete", None) => parent_complete_cursor(WalkPhase::R14a),
        (_, Some(JournalEntry::ParentStarted(_))) => (
            "r5 parent_started".to_string(),
            "parent runtime started".to_string(),
            WalkPhase::R5,
        ),
        (_, Some(JournalEntry::Resource(sample)))
            if sample.phase == journal::resource::Phase::ParentStart =>
        {
            (
                "r5 parent_start_resource".to_string(),
                "parent-start resource was recorded for reconstruction/audit evidence".to_string(),
                WalkPhase::R5,
            )
        }
        (_, Some(JournalEntry::Resource(sample)))
            if sample.phase == journal::resource::Phase::ParentComplete =>
        {
            parent_complete_cursor(projection.parent_phase(sample))
        }
        (_, Some(JournalEntry::SuccessorHandoff(entry))) => {
            if projection.handoff_phase(entry) == WalkPhase::R13b {
                (
                    "r13b successor_handoff".to_string(),
                    "durable same-runtime successor handoff acknowledgement was recorded"
                        .to_string(),
                    WalkPhase::R13b,
                )
            } else {
                (
                    "r13c successor_handoff_mismatch".to_string(),
                    "successor handoff acknowledgement does not match the active runtime attempt"
                        .to_string(),
                    WalkPhase::R13c,
                )
            }
        }
        (_, Some(JournalEntry::Successor(record))) => successor_cursor(record, projection),
        (_, Some(JournalEntry::ActiveCheckoutAdvanced(_))) => (
            "r12 active_checkout_advanced".to_string(),
            "selected successor checkout was installed; handoff remains in progress".to_string(),
            WalkPhase::R12,
        ),
        _ => (
            kind.clone(),
            "durable transition-journal evidence".to_string(),
            projection.latest.unwrap_or(WalkPhase::Empty),
        ),
    };
    projection.latest = Some(phase);
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

fn parent_complete_cursor(phase: WalkPhase) -> (String, String, WalkPhase) {
    match phase {
        WalkPhase::R13b | WalkPhase::R14b => (
            "r14b parent_complete".to_string(),
            "handoff parent turn reached final report after durable same-runtime successor acknowledgement; campaign may still be non-terminal".to_string(),
            WalkPhase::R14b,
        ),
        WalkPhase::R13c => (
            "r13c parent_complete_unacknowledged".to_string(),
            "parent-complete evidence cannot promote an incomplete successor handoff".to_string(),
            WalkPhase::R13c,
        ),
        _ => (
            "r14a parent_complete".to_string(),
            "stopped parent turn reached final report/parent-complete evidence; campaign may still be non-terminal".to_string(),
            WalkPhase::R14a,
        ),
    }
}

fn successor_cursor(
    record: &successor::Record,
    projection: &JournalProjection,
) -> (String, String, WalkPhase) {
    let acknowledged = summary_attempt_key(record)
        .as_ref()
        .is_some_and(|key| projection.is_acknowledged(key));
    match &record.state {
        successor::State::Selected { .. } => (
            "r12 successor.selected".to_string(),
            "successor selection is recorded; checkout and handoff remain in progress".to_string(),
            WalkPhase::R12,
        ),
        successor::State::Checkout { .. } => (
            "r12 successor.checkout".to_string(),
            "selected successor checkout is being installed; handoff is not yet acknowledged"
                .to_string(),
            WalkPhase::R12,
        ),
        successor::State::Stopped { .. } => (
            "r13a successor.stopped".to_string(),
            "parent recorded a stopped continuation without spawning a successor".to_string(),
            WalkPhase::R13a,
        ),
        successor::State::Spawned { .. } | successor::State::Ready { .. } if acknowledged => (
            "r13b successor.acknowledged".to_string(),
            "successor runtime evidence is backed by a durable same-runtime handoff acknowledgement"
                .to_string(),
            WalkPhase::R13b,
        ),
        successor::State::Spawned { .. } => (
            "r13c successor.spawned".to_string(),
            "successor runtime was spawned but durable same-runtime handoff acknowledgement is missing"
                .to_string(),
            WalkPhase::R13c,
        ),
        successor::State::Ready { .. } => (
            "r13c successor.ready".to_string(),
            "successor runtime reported ready but the predecessor did not durably acknowledge the same runtime"
                .to_string(),
            WalkPhase::R13c,
        ),
        successor::State::TimedOut { .. } => (
            "r13c successor.timed_out".to_string(),
            "predecessor retired with an incomplete successor handoff after the ready wait timed out"
                .to_string(),
            WalkPhase::R13c,
        ),
        successor::State::ExitedBeforeReady { .. } => (
            "r13c successor.exited_before_ready".to_string(),
            "predecessor retired with an incomplete handoff after the successor exited before acknowledgement"
                .to_string(),
            WalkPhase::R13c,
        ),
        successor::State::Completed { .. } if acknowledged => (
            "r14b successor.completed".to_string(),
            "successor handoff parent emitted completion evidence after durable same-runtime acknowledgement"
                .to_string(),
            WalkPhase::R14b,
        ),
        successor::State::Completed { .. } => (
            "r13c successor.completed_unacknowledged".to_string(),
            "successor completion evidence lacks a durable same-runtime predecessor acknowledgement"
                .to_string(),
            WalkPhase::R13c,
        ),
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
    use crate::cli::prototype1_state::{
        identity::ParentIdentity,
        journal::{
            ActiveCheckoutAdvancedEntry, ParentStartedEntry, PrototypeJournal,
            SuccessorHandoffEntry,
        },
    };

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
        let legacy = serde_json::json!({
            "kind": "resource.parentcomplete",
            "node_id": "node-root"
        });
        let cursor = cursor_from_entry(2, &legacy, None, &mut JournalProjection::default());
        assert_eq!(cursor.label, "r14a parent_complete");
        assert!(
            cursor
                .meaning
                .contains("campaign may still be non-terminal")
        );
    }

    fn identity(node_id: &str) -> ParentIdentity {
        ParentIdentity::root_bootstrap(
            CampaignId::from("campaign"),
            node_id,
            format!("instance-{node_id}"),
            format!("branch-{node_id}"),
            Some(format!("artifact-{node_id}")),
        )
    }

    fn summary_record(runtime_id: RuntimeId, state: successor::State) -> successor::Record {
        successor::Record {
            runtime_id: Some(runtime_id),
            recorded_at: crate::cli::prototype1_state::event::RecordedAt(1),
            campaign_id: ploke_records::ids::CampaignId::from("campaign"),
            node_id: "node-successor".to_string(),
            state,
        }
    }

    fn checkout() -> JournalEntry {
        JournalEntry::ActiveCheckoutAdvanced(ActiveCheckoutAdvancedEntry {
            recorded_at: crate::cli::prototype1_state::event::RecordedAt(1),
            campaign_id: CampaignId::from("campaign"),
            previous_parent_identity: Some(identity("node-parent")),
            selected_parent_identity: identity("node-successor"),
            active_parent_root: PathBuf::from("/tmp/repo"),
            selected_branch: "artifact-successor".to_string(),
            installed_commit: "abc123".to_string(),
        })
    }

    fn spawned(runtime_id: RuntimeId) -> JournalEntry {
        JournalEntry::Successor(summary_record(
            runtime_id,
            successor::State::Spawned {
                pid: 42,
                incarnation: None,
                active_parent_root: PathBuf::from("/tmp/repo"),
                binary_path: PathBuf::from("/tmp/ploke-eval"),
                invocation_path: PathBuf::from("/tmp/invocation.json"),
                ready_path: PathBuf::from("/tmp/ready.jsonl"),
                streams: journal::Streams {
                    stdout: PathBuf::from("/tmp/stdout"),
                    stderr: PathBuf::from("/tmp/stderr"),
                },
            },
        ))
    }

    fn ready(runtime_id: RuntimeId) -> JournalEntry {
        JournalEntry::Successor(summary_record(
            runtime_id,
            successor::State::Ready {
                pid: 42,
                ready_path: PathBuf::from("/tmp/ready.jsonl"),
                controller: None,
            },
        ))
    }

    fn handoff(runtime_id: RuntimeId) -> JournalEntry {
        JournalEntry::SuccessorHandoff(SuccessorHandoffEntry {
            recorded_at: crate::cli::prototype1_state::event::RecordedAt(2),
            campaign_id: CampaignId::from("campaign"),
            node_id: "node-successor".to_string(),
            runtime_id,
            active_parent_root: PathBuf::from("/tmp/repo"),
            binary_path: PathBuf::from("/tmp/ploke-eval"),
            invocation_path: PathBuf::from("/tmp/invocation.json"),
            ready_path: PathBuf::from("/tmp/ready.jsonl"),
            streams: None,
            pid: 42,
            acceptance: None,
        })
    }

    fn parent_started(runtime_id: RuntimeId) -> JournalEntry {
        JournalEntry::ParentStarted(ParentStartedEntry {
            recorded_at: crate::cli::prototype1_state::event::RecordedAt(3),
            campaign_id: CampaignId::from("campaign"),
            parent_identity: identity("node-successor"),
            repo_root: PathBuf::from("/tmp/repo"),
            handoff_runtime_id: Some(runtime_id),
            pid: 43,
        })
    }

    fn parent_complete() -> JournalEntry {
        let parent = identity("node-parent");
        JournalEntry::Resource(journal::resource::Sample {
            recorded_at: crate::cli::prototype1_state::event::RecordedAt(4),
            campaign_id: CampaignId::from("campaign"),
            parent_id: parent.parent_id().to_string(),
            node_id: parent.node_id().to_string(),
            generation: parent.generation(),
            runtime_id: None,
            subject: journal::resource::Subject::CargoTarget,
            phase: journal::resource::Phase::ParentComplete,
            path: PathBuf::from("/tmp/repo/target"),
            status: journal::resource::Status::Measured,
            bytes: Some(1),
            error: None,
        })
    }

    fn load_persisted(entries: Vec<JournalEntry>) -> JournalSummary {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("transition-journal.jsonl");
        let mut journal = PrototypeJournal::new(path.clone());
        for entry in entries {
            journal
                .append_with_receipt(entry)
                .expect("persist journal entry");
        }
        load_journal(&path).expect("load persisted journal summary")
    }

    #[test]
    fn persisted_summary_keeps_predecessor_ack_across_successor_r5() {
        let runtime_id = RuntimeId(uuid::Uuid::from_u128(1));
        let ready_summary = load_persisted(vec![
            checkout(),
            spawned(runtime_id),
            handoff(runtime_id),
            parent_started(runtime_id),
            ready(runtime_id),
        ]);
        let cursor = ready_summary.latest_cursor.expect("latest ready cursor");
        assert_eq!(cursor.label, "r13b successor.acknowledged");

        let complete_summary = load_persisted(vec![
            checkout(),
            spawned(runtime_id),
            handoff(runtime_id),
            parent_started(runtime_id),
            ready(runtime_id),
            parent_complete(),
        ]);
        let cursor = complete_summary
            .latest_cursor
            .expect("latest complete cursor");
        assert_eq!(cursor.label, "r14b parent_complete");
    }

    #[test]
    fn persisted_summary_rejects_orphan_mismatch_and_late_ack() {
        let runtime_id = RuntimeId(uuid::Uuid::from_u128(1));
        let other_id = RuntimeId(uuid::Uuid::from_u128(2));
        let orphan = load_persisted(vec![spawned(runtime_id), handoff(runtime_id)]);
        assert_eq!(
            orphan.latest_cursor.expect("orphan cursor").label,
            "r13c successor_handoff_mismatch"
        );

        let mismatch = load_persisted(vec![checkout(), spawned(runtime_id), handoff(other_id)]);
        assert_eq!(
            mismatch.latest_cursor.expect("mismatch cursor").label,
            "r13c successor_handoff_mismatch"
        );

        let failed = load_persisted(vec![
            checkout(),
            spawned(runtime_id),
            JournalEntry::Successor(summary_record(
                runtime_id,
                successor::State::ExitedBeforeReady { exit_code: Some(1) },
            )),
            handoff(runtime_id),
            parent_complete(),
        ]);
        assert_eq!(
            failed.latest_cursor.expect("failed cursor").label,
            "r13c parent_complete_unacknowledged"
        );
    }

    #[test]
    fn persisted_summary_rejects_same_id_handoff_path_and_pid_mismatch() {
        let runtime_id = RuntimeId(uuid::Uuid::from_u128(1));
        for field in [
            "active_parent_root",
            "binary_path",
            "invocation_path",
            "ready_path",
            "pid",
        ] {
            let mut mismatched = handoff(runtime_id);
            let JournalEntry::SuccessorHandoff(entry) = &mut mismatched else {
                unreachable!("handoff fixture must be a successor handoff")
            };
            match field {
                "active_parent_root" => entry.active_parent_root = PathBuf::from("/tmp/other"),
                "binary_path" => entry.binary_path = PathBuf::from("/tmp/other-ploke-eval"),
                "invocation_path" => entry.invocation_path = PathBuf::from("/tmp/other.json"),
                "ready_path" => entry.ready_path = PathBuf::from("/tmp/other-ready.jsonl"),
                "pid" => entry.pid = 99,
                _ => unreachable!("all mismatch fields are covered"),
            }

            let summary = load_persisted(vec![checkout(), spawned(runtime_id), mismatched.clone()]);
            assert_eq!(
                summary.latest_cursor.expect("mismatch cursor").label,
                "r13c successor_handoff_mismatch",
                "same-ID {field} mismatch must fail closed"
            );

            let summary = load_persisted(vec![
                checkout(),
                spawned(runtime_id),
                handoff(runtime_id),
                mismatched,
                parent_complete(),
            ]);
            assert_eq!(
                summary.latest_cursor.expect("complete cursor").label,
                "r13c parent_complete_unacknowledged"
            );
        }
    }

    #[test]
    fn persisted_summary_rejects_checkout_root_mismatch() {
        let runtime_id = RuntimeId(uuid::Uuid::from_u128(1));
        let mut mismatched = checkout();
        let JournalEntry::ActiveCheckoutAdvanced(entry) = &mut mismatched else {
            unreachable!("checkout fixture must be an active checkout advance")
        };
        entry.active_parent_root = PathBuf::from("/tmp/other");

        let summary = load_persisted(vec![mismatched, spawned(runtime_id), handoff(runtime_id)]);
        assert_eq!(
            summary.latest_cursor.expect("mismatch cursor").label,
            "r13c successor_handoff_mismatch"
        );
    }

    #[test]
    fn persisted_summary_correlates_present_handoff_streams() {
        let runtime_id = RuntimeId(uuid::Uuid::from_u128(1));
        let streams = journal::Streams {
            stdout: PathBuf::from("/tmp/stdout"),
            stderr: PathBuf::from("/tmp/stderr"),
        };
        let mut exact = handoff(runtime_id);
        let JournalEntry::SuccessorHandoff(entry) = &mut exact else {
            unreachable!("handoff fixture must be a successor handoff")
        };
        entry.streams = Some(streams.clone());
        let summary = load_persisted(vec![checkout(), spawned(runtime_id), exact]);
        assert_eq!(
            summary.latest_cursor.expect("exact cursor").label,
            "r13b successor_handoff"
        );

        for field in ["stdout", "stderr"] {
            let mut mismatched = handoff(runtime_id);
            let JournalEntry::SuccessorHandoff(entry) = &mut mismatched else {
                unreachable!("handoff fixture must be a successor handoff")
            };
            let mut handoff_streams = streams.clone();
            match field {
                "stdout" => handoff_streams.stdout = PathBuf::from("/tmp/other-stdout"),
                "stderr" => handoff_streams.stderr = PathBuf::from("/tmp/other-stderr"),
                _ => unreachable!("all stream fields are covered"),
            }
            entry.streams = Some(handoff_streams);

            let summary = load_persisted(vec![checkout(), spawned(runtime_id), mismatched]);
            assert_eq!(
                summary.latest_cursor.expect("mismatch cursor").label,
                "r13c successor_handoff_mismatch",
                "same-ID {field} mismatch must fail closed"
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
