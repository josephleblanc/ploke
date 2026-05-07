//! Read-only Prototype 1 campaign report.
//!
//! This module intentionally produces a provisional aggregate, not a sealed
//! [`super::history`] value. The goal is to expose the fields that History will
//! eventually admit while keeping the current provenance honest: these rows are
//! read from existing records whose authority, custody, and sealing model still
//! predate the `History` typestate scaffold.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::Serialize;

use crate::{
    cli::{
        InspectOutputFormat, Prototype1MonitorReportCommand,
        prototype1_state::journal::{
            JournalEntry, PrototypeJournal, prototype1_transition_journal_path,
        },
    },
    intervention::{
        Prototype1BranchRegistry, load_or_default_branch_registry, prototype1_branch_registry_path,
        prototype1_scheduler_path,
    },
    operational_metrics::PatchApplyState,
    projection::OperatorProjectionRead,
    record::SubmissionArtifactState,
    spec::PrepareError,
};

use super::cli_facing::Prototype1BranchEvaluationReport;

pub(crate) fn run(
    campaign_id: &str,
    manifest_path: &Path,
    command: &Prototype1MonitorReportCommand,
) -> Result<(), PrepareError> {
    let report = Report::load(campaign_id, manifest_path)?;
    match command.format {
        InspectOutputFormat::Table => report.print(),
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
            );
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct Report {
    schema_version: &'static str,
    generated_at: String,
    campaign_id: String,
    manifest_path: PathBuf,
    prototype_root: PathBuf,
    scheduler_path: PathBuf,
    branch_registry_path: PathBuf,
    transition_journal_path: PathBuf,
    evaluation_dir: PathBuf,
    branch_registry: RegistryView,
    journal: JournalView,
    evaluations: EvaluationView,
    deduped_fields: DedupedFields,
    missing_or_weak_fields: Vec<&'static str>,
}

impl Report {
    fn load(campaign_id: &str, manifest_path: &Path) -> Result<Self, PrepareError> {
        let prototype_root = prototype_root(manifest_path);
        let scheduler_path = prototype1_scheduler_path(manifest_path);
        let branch_registry_path = prototype1_branch_registry_path(manifest_path);
        let transition_journal_path = prototype1_transition_journal_path(manifest_path);
        let evaluation_dir = prototype_root.join("evaluations");

        let branch_registry = load_or_default_branch_registry(
            campaign_id,
            manifest_path,
            OperatorProjectionRead::cli_operator(),
        )?;
        let journal_entries = load_journal(&transition_journal_path)?;
        let evaluations = load_evaluations(&evaluation_dir)?;

        Ok(Self {
            schema_version: "prototype1-report.v1",
            generated_at: Utc::now().to_rfc3339(),
            campaign_id: campaign_id.to_string(),
            manifest_path: manifest_path.to_path_buf(),
            prototype_root,
            scheduler_path,
            branch_registry_path,
            transition_journal_path,
            evaluation_dir,
            branch_registry: RegistryView::from_registry(&branch_registry),
            journal: JournalView::from_entries(journal_entries.clone()),
            evaluations: EvaluationView::from_reports(evaluations),
            deduped_fields: DedupedFields::from_sources(&branch_registry, &journal_entries),
            missing_or_weak_fields: vec![
                "sealed_by / real Crown<Locked> authority is not present in current records",
                "predecessor block verification is not a typed carrier in current records",
                "branch merge semantics are represented as ids, not a merge protocol",
                "tool/model/prompt/full-response evidence is distributed across run artifacts",
                "metrics derivation version and source digests are not consistently recorded",
            ],
        })
    }

    fn print(&self) {
        println!("prototype1 report");
        println!("{}", "-".repeat(40));
        println!("schema_version: {}", self.schema_version);
        println!("generated_at: {}", self.generated_at);
        println!("campaign_id: {}", self.campaign_id);
        println!("manifest: {}", self.manifest_path.display());
        println!("prototype_root: {}", self.prototype_root.display());
        println!("scheduler: {}", self.scheduler_path.display());
        println!("branch_registry: {}", self.branch_registry_path.display());
        println!(
            "transition_journal: {}",
            self.transition_journal_path.display()
        );
        println!("evaluations: {}", self.evaluation_dir.display());
        println!();

        self.branch_registry.print();
        self.journal.print();
        self.evaluations.print();
        self.deduped_fields.print();

        println!("missing or weak fields");
        println!("{}", "-".repeat(40));
        for field in &self.missing_or_weak_fields {
            println!("- {field}");
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct RegistryView {
    schema_version: String,
    updated_at: String,
    source_nodes: usize,
    active_targets: usize,
    branches_total: usize,
    selected_branches: usize,
    branch_status_counts: BTreeMap<String, usize>,
}

impl RegistryView {
    fn from_registry(registry: &Prototype1BranchRegistry) -> Self {
        let mut branch_status_counts = BTreeMap::new();
        let mut branches_total = 0;
        let mut selected = 0;
        for source in &registry.source_nodes {
            if source.selected_branch_id.is_some() {
                selected += 1;
            }
            for branch in &source.branches {
                branches_total += 1;
                *branch_status_counts
                    .entry(serde_name(&branch.status).to_string())
                    .or_insert(0) += 1;
            }
        }

        Self {
            schema_version: registry.schema_version.clone(),
            updated_at: registry.updated_at.clone(),
            source_nodes: registry.source_nodes.len(),
            active_targets: registry.active_targets.len(),
            branches_total,
            selected_branches: selected,
            branch_status_counts,
        }
    }

    fn print(&self) {
        println!("branch registry");
        println!("{}", "-".repeat(40));
        println!("schema_version: {}", self.schema_version);
        println!("updated_at: {}", self.updated_at);
        println!("source_nodes: {}", self.source_nodes);
        println!("active_targets: {}", self.active_targets);
        println!("branches_total: {}", self.branches_total);
        println!("selected_branches: {}", self.selected_branches);
        println!("branch_status_counts:");
        for (status, count) in &self.branch_status_counts {
            println!("  {status}: {count}");
        }
        println!();
    }
}

#[derive(Debug, Clone, Serialize)]
struct JournalView {
    entries_total: usize,
    kind_counts: BTreeMap<&'static str, usize>,
    parent_start_count: usize,
    artifact_commit_count: usize,
    successor_handoff_count: usize,
    imported_as_legacy_evidence: bool,
}

impl JournalView {
    fn from_entries(entries: Vec<JournalEntry>) -> Self {
        let mut kind_counts = BTreeMap::new();
        let mut parent_start_count = 0;
        let mut artifact_commit_count = 0;
        let mut successor_handoff_count = 0;
        for entry in &entries {
            match entry {
                JournalEntry::ParentStarted(_) => parent_start_count += 1,
                JournalEntry::ChildArtifactCommitted(_) => artifact_commit_count += 1,
                JournalEntry::SuccessorHandoff(_) => successor_handoff_count += 1,
                _ => {}
            }
            *kind_counts.entry(journal_kind(entry)).or_insert(0) += 1;
        }

        Self {
            entries_total: entries.len(),
            kind_counts,
            parent_start_count,
            artifact_commit_count,
            successor_handoff_count,
            imported_as_legacy_evidence: true,
        }
    }

    fn print(&self) {
        println!("transition journal");
        println!("{}", "-".repeat(40));
        println!("entries_total: {}", self.entries_total);
        println!("legacy_evidence_projection: yes");
        println!("kind_counts:");
        for (kind, count) in &self.kind_counts {
            println!("  {kind}: {count}");
        }
        println!(
            "parent_start/artifact_commit/successor_handoff: {}/{}/{}",
            self.parent_start_count, self.artifact_commit_count, self.successor_handoff_count
        );
        println!();
    }
}

#[derive(Debug, Clone, Serialize)]
struct EvaluationView {
    reports_total: usize,
    branch_ids: Vec<String>,
    disposition_counts: BTreeMap<String, usize>,
    compared_instances: usize,
    oracle_eligible_instances: usize,
    converged_instances: usize,
    nonempty_submission_instances: usize,
    applied_patch_instances: usize,
    total_tool_calls: usize,
    failed_tool_calls: usize,
    records: Vec<EvaluationRow>,
}

impl EvaluationView {
    fn from_reports(reports: Vec<Prototype1BranchEvaluationReport>) -> Self {
        let mut disposition_counts = BTreeMap::new();
        let mut branch_ids = Vec::new();
        let mut compared_instances = 0;
        let mut oracle_eligible_instances = 0;
        let mut converged_instances = 0;
        let mut nonempty_submission_instances = 0;
        let mut applied_patch_instances = 0;
        let mut total_tool_calls = 0;
        let mut failed_tool_calls = 0;
        let mut records = Vec::new();

        for report in reports {
            branch_ids.push(report.branch_id.clone());
            *disposition_counts
                .entry(serde_name(&report.overall_disposition).to_string())
                .or_insert(0) += 1;
            let metrics = summarize_compared_instances(&report);
            compared_instances += report.compared_instances.len();
            oracle_eligible_instances += metrics.oracle_eligible_instances;
            converged_instances += metrics.converged_instances;
            nonempty_submission_instances += metrics.nonempty_submission_instances;
            applied_patch_instances += metrics.applied_patch_instances;
            total_tool_calls += metrics.total_tool_calls;
            failed_tool_calls += metrics.failed_tool_calls;
            records.push(EvaluationRow {
                branch_id: report.branch_id,
                treatment_campaign_id: report.treatment_campaign_id,
                overall_disposition: serde_name(&report.overall_disposition).to_string(),
                evaluation_artifact_path: report.evaluation_artifact_path,
                compared_instances: report.compared_instances.len(),
                oracle_eligible_instances: metrics.oracle_eligible_instances,
                converged_instances: metrics.converged_instances,
                nonempty_submission_instances: metrics.nonempty_submission_instances,
                applied_patch_instances: metrics.applied_patch_instances,
                total_tool_calls: metrics.total_tool_calls,
                failed_tool_calls: metrics.failed_tool_calls,
            });
        }

        branch_ids.sort();
        records.sort_by(|left, right| left.branch_id.cmp(&right.branch_id));

        Self {
            reports_total: records.len(),
            branch_ids,
            disposition_counts,
            compared_instances,
            oracle_eligible_instances,
            converged_instances,
            nonempty_submission_instances,
            applied_patch_instances,
            total_tool_calls,
            failed_tool_calls,
            records,
        }
    }

    fn print(&self) {
        println!("branch evaluations");
        println!("{}", "-".repeat(40));
        println!("reports_total: {}", self.reports_total);
        println!("compared_instances: {}", self.compared_instances);
        println!(
            "oracle/converged/nonempty/applied: {}/{}/{}/{}",
            self.oracle_eligible_instances,
            self.converged_instances,
            self.nonempty_submission_instances,
            self.applied_patch_instances
        );
        println!(
            "tool_calls_total/failed: {}/{}",
            self.total_tool_calls, self.failed_tool_calls
        );
        println!("disposition_counts:");
        for (disposition, count) in &self.disposition_counts {
            println!("  {disposition}: {count}");
        }
        println!("records:");
        if self.records.is_empty() {
            println!("  (none)");
        } else {
            for row in &self.records {
                println!(
                    "  branch={} disposition={} oracle/converged/nonempty/applied={}/{}/{}/{} tools={}/{}",
                    row.branch_id,
                    row.overall_disposition,
                    row.oracle_eligible_instances,
                    row.converged_instances,
                    row.nonempty_submission_instances,
                    row.applied_patch_instances,
                    row.total_tool_calls,
                    row.failed_tool_calls
                );
            }
        }
        println!();
    }
}

#[derive(Debug, Clone, Default, Serialize)]
struct EvaluationMetrics {
    oracle_eligible_instances: usize,
    converged_instances: usize,
    nonempty_submission_instances: usize,
    applied_patch_instances: usize,
    total_tool_calls: usize,
    failed_tool_calls: usize,
}

#[derive(Debug, Clone, Serialize)]
struct EvaluationRow {
    branch_id: String,
    treatment_campaign_id: String,
    overall_disposition: String,
    evaluation_artifact_path: PathBuf,
    compared_instances: usize,
    oracle_eligible_instances: usize,
    converged_instances: usize,
    nonempty_submission_instances: usize,
    applied_patch_instances: usize,
    total_tool_calls: usize,
    failed_tool_calls: usize,
}

#[derive(Debug, Clone, Serialize)]
struct DedupedFields {
    campaign_ids: Vec<String>,
    node_ids: Vec<String>,
    generations: Vec<u32>,
    runtime_ids: Vec<String>,
    branch_ids: Vec<String>,
    candidate_ids: Vec<String>,
    source_state_ids: Vec<String>,
    target_relpaths: Vec<PathBuf>,
}

impl DedupedFields {
    fn from_sources(registry: &Prototype1BranchRegistry, journal_entries: &[JournalEntry]) -> Self {
        let mut campaign_ids = BTreeSet::new();
        let node_ids = BTreeSet::new();
        let generations = BTreeSet::new();
        let mut runtime_ids = BTreeSet::new();
        let mut branch_ids = BTreeSet::new();
        let mut candidate_ids = BTreeSet::new();
        let mut source_state_ids = BTreeSet::new();
        let mut target_relpaths = BTreeSet::new();

        campaign_ids.insert(registry.campaign_id.clone());
        for source in &registry.source_nodes {
            source_state_ids.insert(source.source_state_id.clone());
            target_relpaths.insert(source.target_relpath.clone());
            if let Some(branch_id) = source.parent_branch_id.as_ref() {
                branch_ids.insert(branch_id.clone());
            }
            if let Some(branch_id) = source.selected_branch_id.as_ref() {
                branch_ids.insert(branch_id.clone());
            }
            for branch in &source.branches {
                branch_ids.insert(branch.branch_id.clone());
                candidate_ids.insert(branch.candidate_id.clone());
            }
        }
        for entry in journal_entries {
            match entry {
                JournalEntry::Successor(entry) => {
                    if let Some(runtime_id) = entry.runtime_id {
                        runtime_ids.insert(runtime_id.to_string());
                    }
                }
                JournalEntry::SuccessorHandoff(entry) => {
                    runtime_ids.insert(entry.runtime_id.to_string());
                }
                JournalEntry::SpawnChild(entry) => {
                    runtime_ids.insert(entry.runtime_id.to_string());
                }
                JournalEntry::Child(entry) => {
                    runtime_ids.insert(entry.runtime_id().to_string());
                }
                JournalEntry::ChildReady(entry) => {
                    runtime_ids.insert(entry.runtime_id.to_string());
                }
                JournalEntry::ObserveChild(entry) => {
                    runtime_ids.insert(entry.runtime_id.to_string());
                }
                JournalEntry::ParentStarted(_)
                | JournalEntry::Resource(_)
                | JournalEntry::ChildArtifactCommitted(_)
                | JournalEntry::ActiveCheckoutAdvanced(_)
                | JournalEntry::MaterializeBranch(_)
                | JournalEntry::BuildChild(_) => {}
            }
        }

        Self {
            campaign_ids: campaign_ids.into_iter().collect(),
            node_ids: node_ids.into_iter().collect(),
            generations: generations.into_iter().collect(),
            runtime_ids: runtime_ids.into_iter().collect(),
            branch_ids: branch_ids.into_iter().collect(),
            candidate_ids: candidate_ids.into_iter().collect(),
            source_state_ids: source_state_ids.into_iter().collect(),
            target_relpaths: target_relpaths.into_iter().collect(),
        }
    }

    fn print(&self) {
        println!("deduped fields");
        println!("{}", "-".repeat(40));
        print_list("campaign_ids", &self.campaign_ids);
        print_list("node_ids", &self.node_ids);
        print_list("generations", &self.generations);
        print_list("runtime_ids", &self.runtime_ids);
        print_list("branch_ids", &self.branch_ids);
        print_list("candidate_ids", &self.candidate_ids);
        print_list("source_state_ids", &self.source_state_ids);
        print_path_list("target_relpaths", &self.target_relpaths);
        println!();
    }
}

fn prototype_root(manifest_path: &Path) -> PathBuf {
    manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
}

fn load_journal(path: &Path) -> Result<Vec<JournalEntry>, PrepareError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    PrototypeJournal::new(path)
        .load_entries()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "load prototype1 transition journal",
            detail: source.to_string(),
        })
}

fn load_evaluations(dir: &Path) -> Result<Vec<Prototype1BranchEvaluationReport>, PrepareError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut reports = Vec::new();
    for entry in fs::read_dir(dir).map_err(|source| PrepareError::ReadManifest {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
            path: path.clone(),
            source,
        })?;
        reports.push(serde_json::from_str(&text).map_err(|source| {
            PrepareError::ParseManifest {
                path: path.clone(),
                source,
            }
        })?);
    }
    Ok(reports)
}

fn summarize_compared_instances(report: &Prototype1BranchEvaluationReport) -> EvaluationMetrics {
    let mut metrics = EvaluationMetrics::default();
    for compared in &report.compared_instances {
        if let Some(treatment) = compared.treatment_metrics.as_ref() {
            if treatment.oracle_eligible {
                metrics.oracle_eligible_instances += 1;
            }
            if treatment.convergence {
                metrics.converged_instances += 1;
            }
            if treatment.submission_artifact_state == SubmissionArtifactState::Nonempty {
                metrics.nonempty_submission_instances += 1;
            }
            if treatment.patch_apply_state == PatchApplyState::Applied {
                metrics.applied_patch_instances += 1;
            }
            metrics.total_tool_calls += treatment.tool_calls_total;
            metrics.failed_tool_calls += treatment.tool_calls_failed;
        }
    }
    metrics
}

fn journal_kind(entry: &JournalEntry) -> &'static str {
    match entry {
        JournalEntry::ParentStarted(_) => "parent_started",
        JournalEntry::Resource(_) => "resource",
        JournalEntry::ChildArtifactCommitted(_) => "artifact:committed",
        JournalEntry::ActiveCheckoutAdvanced(_) => "checkout:advanced",
        JournalEntry::SuccessorHandoff(_) => "successor:handoff",
        JournalEntry::Successor(entry) => entry.entry_kind(),
        JournalEntry::MaterializeBranch(entry) => match entry.phase {
            crate::intervention::CommitPhase::Before => "materialize:before",
            crate::intervention::CommitPhase::After => "materialize:after",
        },
        JournalEntry::BuildChild(entry) => match entry.phase {
            crate::intervention::CommitPhase::Before => "build:before",
            crate::intervention::CommitPhase::After => "build:after",
        },
        JournalEntry::SpawnChild(entry) => match entry.phase {
            super::journal::SpawnPhase::Starting => "child:starting",
            super::journal::SpawnPhase::Spawned => "child:spawned",
            super::journal::SpawnPhase::Observed => "child:observed",
        },
        JournalEntry::Child(entry) => entry.entry_kind(),
        JournalEntry::ChildReady(_) => "child:ready",
        JournalEntry::ObserveChild(entry) => match entry.phase {
            crate::intervention::CommitPhase::Before => "observe:before",
            crate::intervention::CommitPhase::After => "observe:after",
        },
    }
}

fn serde_name<T: Serialize>(value: &T) -> String {
    crate::cli::serde_name(value)
}

fn print_list<T>(label: &str, items: &[T])
where
    T: std::fmt::Display,
{
    println!("{label} ({}):", items.len());
    if items.is_empty() {
        println!("  (none)");
    } else {
        for item in items {
            println!("  {item}");
        }
    }
}

fn print_path_list(label: &str, items: &[PathBuf]) {
    println!("{label} ({}):", items.len());
    if items.is_empty() {
        println!("  (none)");
    } else {
        for item in items {
            println!("  {}", item.display());
        }
    }
}
