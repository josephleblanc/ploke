//! Read-only Prototype 1 metrics projection dashboard.
//!
//! Metrics here are projections over current Prototype 1 evidence. They are
//! not sealed History entries and do not strengthen the authority of mutable
//! scheduler, registry, or node-local files. Each row keeps source refs so the
//! derived numbers can be traced back to the evidence used to compute them.
//!
//! Child/runtime/result/evaluation facts consumed here come from typed
//! `Stored<T>` records through `history_preview::FsEvidenceStore`, not loose
//! `Document` JSON, degraded value parsing, or path/filename recovery.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;

use crate::cli::{InspectOutputFormat, MetricSlice};
use crate::intervention::{
    Prototype1BranchRegistry, Prototype1NodeStatus, Prototype1RunnerDisposition,
    TreatmentBranchStatus,
};
use crate::record::SubmissionArtifactState;
use crate::spec::PrepareError;
use crate::{OperationalRunMetrics, PatchApplyState};

use super::evidence::{
    ChildEvidenceRecords, ChildEvidenceSet, ComparedRunEvidence, EvidenceSource,
};
use super::evidence_class::EvidenceClass;
use super::history_preview::{EvidenceStore, FsEvidenceStore, SelectionPreviewRecords};
use super::journal::JournalEntry;

const SCHEMA_VERSION: &str = "prototype1-metrics-projection.v2";
const DERIVATION: &str = "prototype1.metrics.projection.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Dashboard,
    Cohorts,
    Trajectory,
}

#[derive(Debug, Clone)]
pub(crate) struct MetricRequest {
    pub(crate) rows: usize,
    pub(crate) generation: Option<u32>,
    pub(crate) view: MetricSlice,
    pub(crate) format: InspectOutputFormat,
}

impl MetricRequest {
    fn mode(&self) -> Mode {
        match self.view {
            MetricSlice::Summary => Mode::Dashboard,
            MetricSlice::Cohorts => Mode::Cohorts,
            MetricSlice::Trajectory => Mode::Trajectory,
        }
    }
}

pub(crate) fn run(
    campaign_id: &str,
    manifest_path: &Path,
    request: MetricRequest,
) -> Result<(), PrepareError> {
    let metrics =
        build(campaign_id, manifest_path).map_err(|source| PrepareError::DatabaseSetup {
            phase: "build prototype1 metrics projection",
            detail: source.to_string(),
        })?;
    match request.format {
        InspectOutputFormat::Table => metrics.print(&request),
        InspectOutputFormat::Json => {
            let view = metrics.slice(&request);
            println!(
                "{}",
                serde_json::to_string_pretty(&view).map_err(PrepareError::Serialize)?
            );
        }
    }
    Ok(())
}

pub(crate) fn build(campaign_id: &str, manifest_path: &Path) -> Result<Dashboard, String> {
    let store = FsEvidenceStore::new(manifest_path);
    let child_records = store.child_records().map_err(|source| source.to_string())?;
    let child_evidence = ChildEvidenceSet::from_records(&child_records);
    let selection_records = store
        .preview_selection_records()
        .map_err(|source| source.to_string())?;

    let mut state = Assembly::default();
    state.apply_child_evidence(&child_evidence);
    state.apply_child_records(&child_records);
    for stored in &child_records.journal {
        state.apply_journal(
            stored.item(),
            SourceRef::from_pointer(EvidenceClass::TransitionJournal.as_str(), stored.pointer()),
        );
    }
    state.apply_preview_selection_records(&selection_records);

    Ok(state.finish(campaign_id, manifest_path))
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Dashboard {
    schema_version: &'static str,
    generated_at: String,
    campaign_id: String,
    manifest_path: PathBuf,
    derivation: &'static str,
    rows: Vec<Row>,
    generations: Vec<Generation>,
    cohorts: Vec<Cohort>,
    trajectory: Trajectory,
    selected_by_generation: Vec<Step>,
    child_evidence: EvidenceProjection,
    diagnostics: Vec<String>,
}

impl Dashboard {
    fn slice(&self, request: &MetricRequest) -> Slice {
        let rows = self
            .rows
            .iter()
            .filter(|row| generation_matches(row.generation, request.generation))
            .take(request.rows)
            .cloned()
            .collect();
        let generations = self
            .generations
            .iter()
            .filter(|generation| {
                Some(generation.generation) == request.generation || request.generation.is_none()
            })
            .cloned()
            .collect();
        let cohorts = self
            .cohorts
            .iter()
            .filter(|cohort| {
                Some(cohort.key.generation) == request.generation || request.generation.is_none()
            })
            .cloned()
            .collect();
        let trajectory = self.trajectory.slice(request.generation);
        let selected_by_generation = trajectory.steps.clone();
        Slice {
            schema_version: self.schema_version,
            generated_at: self.generated_at.clone(),
            campaign_id: self.campaign_id.clone(),
            derivation: self.derivation,
            row_count: self.rows.len(),
            generation_count: self.generations.len(),
            diagnostics: self.diagnostics.clone(),
            rows,
            generations,
            cohorts,
            trajectory,
            selected_by_generation,
            child_evidence: self.child_evidence.clone(),
        }
    }

    fn print(&self, request: &MetricRequest) {
        let slice = self.slice(request);
        let mode = request.mode();
        println!("prototype1 metrics projection");
        println!("{}", "-".repeat(40));
        println!("schema_version: {}", slice.schema_version);
        println!("generated_at: {}", slice.generated_at);
        println!("campaign_id: {}", slice.campaign_id);
        println!("derivation: {}", slice.derivation);
        println!("rows: {}", slice.row_count);
        println!("generations: {}", slice.generation_count);
        println!(
            "child_evidence: {} children, {} unplaced sources, {} diagnostics",
            slice.child_evidence.children,
            slice.child_evidence.unplaced_sources,
            slice.child_evidence.diagnostics
        );
        if let Some(generation) = request.generation {
            println!("generation_filter: {generation}");
        }
        println!();

        if matches!(mode, Mode::Dashboard) {
            print_generations(&slice.generations);
            println!();
        }
        if matches!(mode, Mode::Dashboard | Mode::Cohorts) {
            print_cohorts(&slice.cohorts);
            println!();
        }
        if matches!(mode, Mode::Dashboard | Mode::Trajectory) {
            print_trajectory(&slice.trajectory, matches!(mode, Mode::Trajectory));
            println!();
        }
        if matches!(mode, Mode::Dashboard) {
            print_rows(&slice.rows);
            println!();
        }
        if !matches!(mode, Mode::Trajectory) && !slice.diagnostics.is_empty() {
            println!("diagnostics");
            println!("{}", "-".repeat(40));
            for diagnostic in &slice.diagnostics {
                println!("{diagnostic}");
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Slice {
    schema_version: &'static str,
    generated_at: String,
    campaign_id: String,
    derivation: &'static str,
    row_count: usize,
    generation_count: usize,
    diagnostics: Vec<String>,
    rows: Vec<Row>,
    generations: Vec<Generation>,
    cohorts: Vec<Cohort>,
    trajectory: Trajectory,
    selected_by_generation: Vec<Step>,
    child_evidence: EvidenceProjection,
}

#[derive(Debug, Clone, Serialize)]
struct EvidenceProjection {
    join: &'static str,
    children: usize,
    unplaced_sources: usize,
    diagnostics: usize,
    document_sources: usize,
    journal_sources: usize,
    evaluation_sources: usize,
    source_treatments: Vec<SourceTreatment>,
}

impl EvidenceProjection {
    fn from_set(evidence: &ChildEvidenceSet) -> Self {
        let mut document_sources = 0;
        let mut journal_sources = 0;
        let mut evaluation_sources = 0;
        let mut treatments = BTreeMap::<(String, String), usize>::new();
        let mut counted_sources = BTreeSet::<String>::new();
        for child in &evidence.children {
            document_sources += child.documents.len();
            journal_sources += child.journal.len();
            evaluation_sources += child.evaluations.len();
            for source in child.documents.iter().chain(child.journal.iter()).chain(
                child
                    .evaluations
                    .iter()
                    .map(|evaluation| &evaluation.source),
            ) {
                record_treatment(source, &mut counted_sources, &mut treatments);
            }
        }
        for source in &evidence.unplaced {
            record_treatment(source, &mut counted_sources, &mut treatments);
        }
        Self {
            join: "history_preview.child_evidence",
            children: evidence.children.len(),
            unplaced_sources: evidence.unplaced.len(),
            diagnostics: evidence.diagnostics.len()
                + evidence
                    .children
                    .iter()
                    .map(|child| child.diagnostics.len())
                    .sum::<usize>(),
            document_sources,
            journal_sources,
            evaluation_sources,
            source_treatments: treatments
                .into_iter()
                .map(
                    |((class, preview_import_treatment), sources)| SourceTreatment {
                        class,
                        preview_import_treatment,
                        sources,
                    },
                )
                .collect(),
        }
    }

    fn empty() -> Self {
        Self {
            join: "history_preview.child_evidence",
            children: 0,
            unplaced_sources: 0,
            diagnostics: 0,
            document_sources: 0,
            journal_sources: 0,
            evaluation_sources: 0,
            source_treatments: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SourceTreatment {
    class: String,
    preview_import_treatment: String,
    sources: usize,
}

fn record_treatment(
    source: &EvidenceSource,
    counted_sources: &mut BTreeSet<String>,
    treatments: &mut BTreeMap<(String, String), usize>,
) {
    if counted_sources.insert(source.pointer.ref_id().to_string()) {
        let key = (
            source.class.as_str().to_string(),
            source.preview_import_treatment.clone(),
        );
        *treatments.entry(key).or_default() += 1;
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Row {
    generation: Option<u32>,
    node_id: String,
    parent_node_id: Option<String>,
    runtime_id: Option<String>,
    branch_id: Option<String>,
    role: String,
    status: Option<String>,
    disposition: Option<String>,
    selected: bool,
    selection_authority: Option<&'static str>,
    selection_sources: Vec<SelectionSource>,
    dashboard_rank: Option<usize>,
    dashboard_score: i64,
    dashboard_score_derivation: ScoreDerivation,
    compared_instances: usize,
    oracle_eligible_instances: usize,
    converged_instances: usize,
    nonempty_submission_instances: usize,
    applied_patch_instances: usize,
    patch_attempted_instances: usize,
    partial_patch_instances: usize,
    no_patch_instances: usize,
    nonempty_valid_patch_instances: usize,
    missing_submission_instances: usize,
    empty_submission_instances: usize,
    partial_patch_failures: usize,
    same_file_patch_retries: usize,
    same_file_patch_max_streak: usize,
    aborted_instances: usize,
    aborted_repair_loop_instances: usize,
    total_tool_calls: usize,
    failed_tool_calls: usize,
    failed_tool_call_rate: Option<f64>,
    evaluation_ref: Option<String>,
    result_ref: Option<String>,
    source_refs: Vec<SourceRef>,
}

impl Row {
    fn new(node_id: impl Into<String>) -> Self {
        Self {
            generation: None,
            node_id: node_id.into(),
            parent_node_id: None,
            runtime_id: None,
            branch_id: None,
            role: "unknown".to_string(),
            status: None,
            disposition: None,
            selected: false,
            selection_authority: None,
            selection_sources: Vec::new(),
            dashboard_rank: None,
            dashboard_score: 0,
            dashboard_score_derivation: ScoreDerivation::default(),
            compared_instances: 0,
            oracle_eligible_instances: 0,
            converged_instances: 0,
            nonempty_submission_instances: 0,
            applied_patch_instances: 0,
            patch_attempted_instances: 0,
            partial_patch_instances: 0,
            no_patch_instances: 0,
            nonempty_valid_patch_instances: 0,
            missing_submission_instances: 0,
            empty_submission_instances: 0,
            partial_patch_failures: 0,
            same_file_patch_retries: 0,
            same_file_patch_max_streak: 0,
            aborted_instances: 0,
            aborted_repair_loop_instances: 0,
            total_tool_calls: 0,
            failed_tool_calls: 0,
            failed_tool_call_rate: None,
            evaluation_ref: None,
            result_ref: None,
            source_refs: Vec::new(),
        }
    }

    fn select(&mut self, source: SelectionSource) {
        if !self
            .selection_sources
            .iter()
            .any(|existing| existing.source.ref_id == source.source.ref_id)
        {
            self.source(source.source.clone());
            self.selection_sources.push(source);
        }
        self.selected = true;
        self.selection_authority = strongest_selection_authority(&self.selection_sources);
    }

    fn source(&mut self, source: SourceRef) {
        if !self
            .source_refs
            .iter()
            .any(|existing| existing.ref_id == source.ref_id)
        {
            self.source_refs.push(source);
        }
    }

    fn apply_metrics(&mut self, metrics: &Totals) {
        self.compared_instances = metrics.compared_instances;
        self.oracle_eligible_instances = metrics.oracle_eligible_instances;
        self.converged_instances = metrics.converged_instances;
        self.nonempty_submission_instances = metrics.nonempty_submission_instances;
        self.applied_patch_instances = metrics.applied_patch_instances;
        self.patch_attempted_instances = metrics.patch_attempted_instances;
        self.partial_patch_instances = metrics.partial_patch_instances;
        self.no_patch_instances = metrics.no_patch_instances;
        self.nonempty_valid_patch_instances = metrics.nonempty_valid_patch_instances;
        self.missing_submission_instances = metrics.missing_submission_instances;
        self.empty_submission_instances = metrics.empty_submission_instances;
        self.partial_patch_failures = metrics.partial_patch_failures;
        self.same_file_patch_retries = metrics.same_file_patch_retries;
        self.same_file_patch_max_streak = metrics.same_file_patch_max_streak;
        self.aborted_instances = metrics.aborted_instances;
        self.aborted_repair_loop_instances = metrics.aborted_repair_loop_instances;
        self.total_tool_calls = metrics.total_tool_calls;
        self.failed_tool_calls = metrics.failed_tool_calls;
        self.failed_tool_call_rate = rate(metrics.failed_tool_calls, metrics.total_tool_calls);
    }

    fn refresh_dashboard_score(&mut self) {
        let derivation = ScoreDerivation::from_row(self);
        self.dashboard_score = derivation.total;
        self.dashboard_score_derivation = derivation;
    }

    fn rank_key(&self) -> RankKey {
        RankKey {
            evaluated: (self.compared_instances > 0 || self.evaluation_ref.is_some()) as u8,
            keep: matches_text(self.disposition.as_deref(), "keep") as u8,
            oracle_eligible: self.oracle_eligible_instances as u64,
            converged: self.converged_instances as u64,
            nonempty_submission: self.nonempty_submission_instances as u64,
            applied_patch: self.applied_patch_instances as u64,
            fewer_aborts: u64::MAX - self.aborted_instances as u64,
            fewer_repair_loops: u64::MAX - self.aborted_repair_loop_instances as u64,
            fewer_failures: u64::MAX - self.failed_tool_calls as u64,
            more_tool_activity: self.total_tool_calls as u64,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
struct ScoreDerivation {
    kind: &'static str,
    rank_relation: &'static str,
    keep_disposition: i64,
    oracle_eligible_instances: i64,
    converged_instances: i64,
    nonempty_submission_instances: i64,
    applied_patch_instances: i64,
    failed_tool_calls: i64,
    total: i64,
}

impl ScoreDerivation {
    fn from_row(row: &Row) -> Self {
        let keep_disposition = if matches_text(row.disposition.as_deref(), "keep") {
            10_000
        } else {
            0
        };
        let oracle_eligible_instances = row.oracle_eligible_instances as i64 * 1_000;
        let converged_instances = row.converged_instances as i64 * 500;
        let nonempty_submission_instances = row.nonempty_submission_instances as i64 * 250;
        let applied_patch_instances = row.applied_patch_instances as i64 * 100;
        let failed_tool_calls = -(row.failed_tool_calls as i64);
        let total = keep_disposition
            + oracle_eligible_instances
            + converged_instances
            + nonempty_submission_instances
            + applied_patch_instances
            + failed_tool_calls;
        Self {
            kind: "heuristic_projection",
            rank_relation: "separate_from_dashboard_rank",
            keep_disposition,
            oracle_eligible_instances,
            converged_instances,
            nonempty_submission_instances,
            applied_patch_instances,
            failed_tool_calls,
            total,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RankKey {
    evaluated: u8,
    keep: u8,
    oracle_eligible: u64,
    converged: u64,
    nonempty_submission: u64,
    applied_patch: u64,
    fewer_aborts: u64,
    fewer_repair_loops: u64,
    fewer_failures: u64,
    more_tool_activity: u64,
}

#[derive(Debug, Clone, Serialize)]
struct Generation {
    generation: u32,
    nodes: usize,
    completed: usize,
    failed: usize,
    evaluations: usize,
    selected_node_id: Option<String>,
    selected_branch_id: Option<String>,
    selected_authority: Option<&'static str>,
    selected_dashboard_rank: Option<usize>,
    top_ranked_node_id: Option<String>,
    top_ranked_branch_id: Option<String>,
    top_dashboard_score: Option<i64>,
    selected_dashboard_score: Option<i64>,
    deltas: Vec<Delta>,
    total_tool_calls: usize,
    failed_tool_calls: usize,
    failed_tool_call_rate: Option<f64>,
    keep_count: usize,
    reject_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
struct Key {
    lineage: Option<String>,
    parent_node_id: Option<String>,
    generation: u32,
}

#[derive(Debug, Clone, Serialize)]
struct Cohort {
    key: Key,
    nodes: usize,
    completed: usize,
    failed: usize,
    evaluations: usize,
    selected_count: usize,
    selected: Vec<Choice>,
    top: Option<Choice>,
    alternative: Option<Choice>,
    deltas: Vec<Delta>,
    total_tool_calls: usize,
    failed_tool_calls: usize,
    failed_tool_call_rate: Option<f64>,
    patch_attempted_instances: usize,
    applied_patch_instances: usize,
    partial_patch_instances: usize,
    aborted_instances: usize,
    aborted_repair_loop_instances: usize,
}

#[derive(Debug, Clone, Serialize)]
struct Trajectory {
    coordinate: &'static str,
    state: ProjectionState,
    decisions: Vec<Decision>,
    steps: Vec<Step>,
    diagnostics: Vec<String>,
}

impl Trajectory {
    fn from_cohorts(cohorts: &[Cohort], rows: &[Row]) -> Self {
        let rows = rows
            .iter()
            .map(|row| (row.node_id.as_str(), row))
            .collect::<BTreeMap<_, _>>();
        let decisions = cohorts.iter().map(Decision::from_cohort).collect();
        Self::from_decisions(decisions, &rows)
    }

    fn slice(&self, generation: Option<u32>) -> Self {
        let Some(generation) = generation else {
            return self.clone();
        };
        let decisions = self
            .decisions
            .iter()
            .filter(|decision| decision.key.generation == generation)
            .cloned()
            .collect::<Vec<_>>();
        let steps = self
            .steps
            .iter()
            .filter(|step| step.generation == generation)
            .cloned()
            .collect::<Vec<_>>();
        let diagnostics = self
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.contains("lineage unavailable")
                    || diagnostic.contains(&format!("generation {generation}"))
            })
            .cloned()
            .collect::<Vec<_>>();
        let state = state_from_parts(&decisions, &steps);
        Self {
            coordinate: self.coordinate,
            state,
            decisions,
            steps,
            diagnostics,
        }
    }

    fn from_decisions(decisions: Vec<Decision>, rows: &BTreeMap<&str, &Row>) -> Self {
        let mut diagnostics = vec![
            "trajectory lineage unavailable in current records: projecting selected rows over degraded parent_node_id/generation cohorts".to_string(),
        ];
        let mut by_generation = BTreeMap::<u32, Vec<&Decision>>::new();
        let mut has_ambiguity = false;
        let mut has_incomplete = false;

        for decision in &decisions {
            by_generation
                .entry(decision.key.generation)
                .or_default()
                .push(decision);
            match decision.state {
                DecisionState::None => {}
                DecisionState::One => {}
                DecisionState::Many => {
                    has_ambiguity = true;
                    diagnostics.push(format!(
                        "trajectory cohort is ambiguous at generation {} parent {}: selected nodes {}",
                        decision.key.generation,
                        display_parent(decision.key.parent_node_id.as_deref()),
                        choice_ids(&decision.selected)
                    ));
                }
            }
        }

        let mut steps = Vec::new();
        for (generation, decisions) in &by_generation {
            let selected = decisions
                .iter()
                .copied()
                .filter(|decision| !decision.selected.is_empty())
                .collect::<Vec<_>>();
            if selected.len() > 1 {
                has_ambiguity = true;
                diagnostics.push(format!(
                    "trajectory generation {generation} has selected rows in multiple parent cohorts: {}",
                    selected
                        .iter()
                        .map(|decision| format!(
                            "parent {} -> {}",
                            display_parent(decision.key.parent_node_id.as_deref()),
                            choice_ids(&decision.selected)
                        ))
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }
            for decision in selected {
                if let DecisionState::One = decision.state {
                    steps.push(Step::from_decision(decision, rows));
                }
            }
        }

        let continuity = mark_continuity(&mut steps, &mut diagnostics);
        has_ambiguity |= continuity.ambiguous;
        has_incomplete |= continuity.incomplete;
        let state = if decisions.is_empty() {
            ProjectionState::Empty
        } else if has_ambiguity {
            ProjectionState::Ambiguous
        } else if has_incomplete {
            ProjectionState::Incomplete
        } else {
            ProjectionState::Unambiguous
        };

        Self {
            coordinate: "degraded_parent_generation",
            state,
            decisions,
            steps,
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProjectionState {
    Empty,
    Unambiguous,
    Ambiguous,
    Incomplete,
}

fn state_from_parts(decisions: &[Decision], steps: &[Step]) -> ProjectionState {
    if decisions.is_empty() {
        return ProjectionState::Empty;
    }
    if decisions
        .iter()
        .any(|decision| matches!(decision.state, DecisionState::Many))
        || steps.iter().any(|step| {
            matches!(
                step.parent_continuity,
                Some("ambiguous_generation" | "ambiguous_previous" | "discontinuous")
            )
        })
    {
        return ProjectionState::Ambiguous;
    }
    if steps
        .iter()
        .any(|step| matches!(step.parent_continuity, Some("unknown_parent")))
    {
        return ProjectionState::Incomplete;
    }
    ProjectionState::Unambiguous
}

#[derive(Debug, Clone, Serialize)]
struct Decision {
    key: Key,
    state: DecisionState,
    nodes: usize,
    completed: usize,
    failed: usize,
    evaluations: usize,
    selected_count: usize,
    selected: Vec<Choice>,
    top: Option<Choice>,
    alternative: Option<Choice>,
    deltas: Vec<Delta>,
}

impl Decision {
    fn from_cohort(cohort: &Cohort) -> Self {
        let mut decision = Self {
            key: cohort.key.clone(),
            state: DecisionState::from_count(cohort.selected_count),
            nodes: cohort.nodes,
            completed: cohort.completed,
            failed: cohort.failed,
            evaluations: cohort.evaluations,
            selected_count: cohort.selected_count,
            selected: cohort.selected.clone(),
            top: cohort.top.clone(),
            alternative: cohort.alternative.clone(),
            deltas: cohort.deltas.clone(),
        };
        if matches!(decision.state, DecisionState::Many) {
            decision.deltas.push(Delta::against_alternative(
                DeltaState::Ambiguous,
                decision.alternative.as_ref(),
                decision.selected_count,
                None,
            ));
        }
        decision
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DecisionState {
    None,
    One,
    Many,
}

impl DecisionState {
    fn from_count(selected_count: usize) -> Self {
        match selected_count {
            0 => Self::None,
            1 => Self::One,
            _ => Self::Many,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Choice {
    node_id: String,
    branch_id: Option<String>,
    authority: Option<&'static str>,
    dashboard_rank: Option<usize>,
    disposition: Option<String>,
    dashboard_score: i64,
}

#[derive(Debug, Clone, Serialize)]
struct Delta {
    basis: Basis,
    state: DeltaState,
    against: Option<Point>,
    value: Option<i64>,
    count: usize,
}

impl Delta {
    fn against_top(selected: Option<&Row>, top: Option<&Row>, selected_count: usize) -> Self {
        let state = match (selected_count, selected, top) {
            (count, _, _) if count > 1 => DeltaState::Ambiguous,
            (1, Some(_), Some(_)) => DeltaState::Known,
            _ => DeltaState::Empty,
        };
        Self {
            basis: Basis::Top,
            state,
            against: top.map(Point::from_row),
            value: selected
                .zip(top)
                .filter(|_| matches!(state, DeltaState::Known))
                .map(|(selected, top)| selected.dashboard_score - top.dashboard_score),
            count: if matches!(state, DeltaState::Known) {
                1
            } else if matches!(state, DeltaState::Ambiguous) {
                selected_count
            } else {
                0
            },
        }
    }

    fn against_parent(
        selected: &Choice,
        parent_node_id: Option<&str>,
        parent: Option<&Row>,
    ) -> Self {
        let against = parent
            .map(Point::from_row)
            .or_else(|| parent_node_id.map(Point::missing));
        Self {
            basis: Basis::Parent,
            state: if parent.is_some() {
                DeltaState::Known
            } else {
                DeltaState::Missing
            },
            against,
            value: parent.map(|parent| selected.dashboard_score - parent.dashboard_score),
            count: usize::from(parent.is_some()),
        }
    }

    fn against_alternative(
        state: DeltaState,
        alternative: Option<&Choice>,
        count: usize,
        selected: Option<&Choice>,
    ) -> Self {
        let state = if matches!(state, DeltaState::Ambiguous) {
            DeltaState::Ambiguous
        } else if alternative.is_some() {
            state
        } else {
            DeltaState::Empty
        };
        Self {
            basis: Basis::Alternative,
            state,
            against: alternative.map(Point::from_choice),
            value: selected
                .zip(alternative)
                .filter(|_| matches!(state, DeltaState::Known))
                .map(|(selected, alternative)| {
                    selected.dashboard_score - alternative.dashboard_score
                }),
            count: if alternative.is_some() || matches!(state, DeltaState::Ambiguous) {
                count
            } else {
                0
            },
        }
    }

    fn against_previous(current: &Step, previous: &Step) -> Self {
        let against = previous.selected_node_id.as_ref().map(|node_id| Point {
            node_id: node_id.clone(),
            branch_id: previous.selected_branch_id.clone(),
            dashboard_rank: previous.selected_dashboard_rank,
            dashboard_score: previous.selected_dashboard_score,
        });
        let value = current
            .selected_dashboard_score
            .zip(previous.selected_dashboard_score)
            .map(|(current, previous)| current - previous);
        Self {
            basis: Basis::Previous,
            state: if value.is_some() {
                DeltaState::Known
            } else {
                DeltaState::Empty
            },
            against,
            value,
            count: usize::from(value.is_some()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Basis {
    Top,
    Parent,
    Alternative,
    Previous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DeltaState {
    Known,
    Missing,
    Empty,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize)]
struct Point {
    node_id: String,
    branch_id: Option<String>,
    dashboard_rank: Option<usize>,
    dashboard_score: Option<i64>,
}

impl Point {
    fn from_choice(choice: &Choice) -> Self {
        Self {
            node_id: choice.node_id.clone(),
            branch_id: choice.branch_id.clone(),
            dashboard_rank: choice.dashboard_rank,
            dashboard_score: Some(choice.dashboard_score),
        }
    }

    fn from_row(row: &Row) -> Self {
        Self {
            node_id: row.node_id.clone(),
            branch_id: row.branch_id.clone(),
            dashboard_rank: row.dashboard_rank,
            dashboard_score: Some(row.dashboard_score),
        }
    }

    fn missing(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            branch_id: None,
            dashboard_rank: None,
            dashboard_score: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Step {
    cohort: Key,
    generation: u32,
    selected_node_id: Option<String>,
    selected_branch_id: Option<String>,
    selected_authority: Option<&'static str>,
    selected_dashboard_rank: Option<usize>,
    selected_disposition: Option<String>,
    selected_dashboard_score: Option<i64>,
    parent_continuity: Option<&'static str>,
    deltas: Vec<Delta>,
    completed_nodes: usize,
    failed_nodes: usize,
}

impl Step {
    fn from_decision(decision: &Decision, rows: &BTreeMap<&str, &Row>) -> Self {
        let selected = decision.selected.first().expect("one selected choice");
        let parent = decision
            .key
            .parent_node_id
            .as_deref()
            .and_then(|node_id| rows.get(node_id).copied());
        Self {
            cohort: decision.key.clone(),
            generation: decision.key.generation,
            selected_node_id: Some(selected.node_id.clone()),
            selected_branch_id: selected.branch_id.clone(),
            selected_authority: selected.authority,
            selected_dashboard_rank: selected.dashboard_rank,
            selected_disposition: selected.disposition.clone(),
            selected_dashboard_score: Some(selected.dashboard_score),
            parent_continuity: None,
            deltas: vec![
                Delta::against_parent(selected, decision.key.parent_node_id.as_deref(), parent),
                Delta::against_alternative(
                    DeltaState::Known,
                    decision.alternative.as_ref(),
                    1,
                    Some(selected),
                ),
            ],
            completed_nodes: decision.completed,
            failed_nodes: decision.failed,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SelectionSource {
    authority: &'static str,
    source: SourceRef,
}

#[derive(Debug, Clone, Serialize)]
struct SourceRef {
    class: &'static str,
    ref_id: String,
    path: PathBuf,
    hash: String,
}

impl SourceRef {
    fn from_pointer(
        class: &'static str,
        pointer: &super::history_preview::EvidencePointer,
    ) -> Self {
        Self {
            class,
            ref_id: pointer.ref_id().to_string(),
            path: pointer.path().to_path_buf(),
            hash: pointer.hash().as_str().to_string(),
        }
    }

    fn from_evidence(source: &EvidenceSource) -> Self {
        Self {
            class: source.class.as_str(),
            ref_id: source.pointer.ref_id().to_string(),
            path: source.pointer.path().to_path_buf(),
            hash: source.pointer.hash().as_str().to_string(),
        }
    }
}

fn format_evidence_diagnostic(
    scope: &str,
    node_id: Option<&str>,
    severity: &str,
    source_ref: Option<&str>,
    message: &str,
) -> String {
    let node = node_id
        .map(|node_id| format!(" for {node_id}"))
        .unwrap_or_default();
    let source = source_ref
        .map(|source_ref| format!(" at {source_ref}"))
        .unwrap_or_default();
    format!("{scope} {severity}{node}{source}: {message}")
}

#[derive(Debug, Clone, Default)]
struct Totals {
    compared_instances: usize,
    oracle_eligible_instances: usize,
    converged_instances: usize,
    nonempty_submission_instances: usize,
    applied_patch_instances: usize,
    patch_attempted_instances: usize,
    partial_patch_instances: usize,
    no_patch_instances: usize,
    nonempty_valid_patch_instances: usize,
    missing_submission_instances: usize,
    empty_submission_instances: usize,
    partial_patch_failures: usize,
    same_file_patch_retries: usize,
    same_file_patch_max_streak: usize,
    aborted_instances: usize,
    aborted_repair_loop_instances: usize,
    total_tool_calls: usize,
    failed_tool_calls: usize,
}

#[derive(Debug, Default)]
struct Assembly {
    rows: BTreeMap<String, Row>,
    branch_to_node: BTreeMap<String, String>,
    ambiguous_branches: BTreeSet<String>,
    selected_branches: BTreeMap<String, Vec<SelectionSource>>,
    child_evidence: Option<EvidenceProjection>,
    diagnostics: Vec<String>,
}

impl Assembly {
    fn apply_child_evidence(&mut self, evidence: &ChildEvidenceSet) {
        self.child_evidence = Some(EvidenceProjection::from_set(evidence));
        let branch_placement = evidence.branch_placement();
        for branch_id in &branch_placement.ambiguous {
            if self.ambiguous_branches.insert(branch_id.clone()) {
                self.branch_to_node.remove(branch_id);
                self.diagnostics.push(format!(
                    "child_evidence warning: branch '{branch_id}' appears under multiple child nodes; branch-scoped evidence will not be attached"
                ));
            }
        }
        for (branch_id, node_id) in branch_placement.nodes {
            self.insert_branch_node(&branch_id, &node_id, "child_evidence");
        }
        for diagnostic in &evidence.diagnostics {
            self.diagnostics.push(format_evidence_diagnostic(
                "child_evidence",
                None,
                &diagnostic.severity,
                diagnostic.source_ref.as_deref(),
                &diagnostic.message,
            ));
        }

        for child in &evidence.children {
            let mut diagnostics = Vec::new();
            {
                let row = self.row(&child.node_id);
                row.generation = row.generation.or(child.generation);
                row.parent_node_id = row
                    .parent_node_id
                    .clone()
                    .or_else(|| child.parent_node_id.clone());
                row.branch_id = row.branch_id.clone().or_else(|| child.branch_id.clone());
                for source in child.documents.iter().chain(child.journal.iter()) {
                    row.source(SourceRef::from_evidence(source));
                }
                for runtime in &child.runtimes {
                    if row.runtime_id.is_none() {
                        row.runtime_id = Some(runtime.runtime_id.clone());
                    } else if row.runtime_id.as_deref() != Some(runtime.runtime_id.as_str()) {
                        diagnostics.push(format!(
                            "child_evidence warning: node {} has multiple runtime ids; keeping {} and citing {}",
                            child.node_id,
                            row.runtime_id.as_deref().unwrap_or("-"),
                            runtime.runtime_id
                        ));
                    }
                    for source in runtime.documents.iter().chain(runtime.journal.iter()) {
                        row.source(SourceRef::from_evidence(source));
                    }
                }
                for branch in &child.branches {
                    if row.branch_id.is_none() {
                        row.branch_id = Some(branch.branch_id.clone());
                    }
                    for source in &branch.sources {
                        row.source(SourceRef::from_evidence(source));
                    }
                }
                for evaluation in &child.evaluations {
                    let totals = totals_from_compared(&evaluation.compared);
                    row.apply_metrics(&totals);
                    row.evaluation_ref = row
                        .evaluation_ref
                        .clone()
                        .or_else(|| Some(evaluation.source.pointer.ref_id().to_string()));
                    row.disposition = row
                        .disposition
                        .clone()
                        .or_else(|| evaluation.overall_disposition.clone());
                    row.compared_instances = row.compared_instances.max(evaluation.compared.len());
                    row.source(SourceRef::from_evidence(&evaluation.source));
                }
            }

            self.index_branch(&child.node_id);
            self.diagnostics.extend(diagnostics);
            for diagnostic in &child.diagnostics {
                self.diagnostics.push(format_evidence_diagnostic(
                    "child_evidence",
                    Some(&child.node_id),
                    &diagnostic.severity,
                    diagnostic.source_ref.as_deref(),
                    &diagnostic.message,
                ));
            }
        }
        if !evidence.unplaced.is_empty() {
            self.diagnostics.push(format!(
                "child_evidence info: {} sources were not joined to child rows",
                evidence.unplaced.len()
            ));
        }
    }

    fn apply_child_records(&mut self, records: &ChildEvidenceRecords) {
        for stored in &records.nodes {
            let record = stored.item();
            let source =
                SourceRef::from_pointer(EvidenceClass::NodeRecord.as_str(), stored.pointer());
            {
                let row = self.row(&record.node_id);
                row.generation = row.generation.or(Some(record.generation));
                row.parent_node_id = row
                    .parent_node_id
                    .clone()
                    .or_else(|| record.parent_node_id.clone());
                row.branch_id = row
                    .branch_id
                    .clone()
                    .or_else(|| Some(record.branch_id.clone()));
                row.status = row
                    .status
                    .clone()
                    .or_else(|| Some(status_text(record.status).to_string()));
                row.source(source);
            }
            self.index_branch(&record.node_id);
        }

        for stored in &records.runner_requests {
            let record = stored.item();
            let source =
                SourceRef::from_pointer(EvidenceClass::RunnerRequest.as_str(), stored.pointer());
            {
                let row = self.row(&record.node_id);
                row.generation = row.generation.or(Some(record.generation));
                row.branch_id = row
                    .branch_id
                    .clone()
                    .or_else(|| Some(record.branch_id.clone()));
                row.role = "child".to_string();
                row.source(source);
            }
            self.index_branch(&record.node_id);
        }

        for stored in &records.runner_results {
            let record = stored.item();
            let source =
                SourceRef::from_pointer(EvidenceClass::RunnerResult.as_str(), stored.pointer());
            {
                let row = self.row(&record.node_id);
                row.generation = row.generation.or(Some(record.generation));
                row.branch_id = row
                    .branch_id
                    .clone()
                    .or_else(|| Some(record.branch_id.clone()));
                row.status = row
                    .status
                    .clone()
                    .or_else(|| Some(status_text(record.status).to_string()));
                row.disposition = row
                    .disposition
                    .clone()
                    .or_else(|| Some(disposition_text(record.disposition).to_string()));
                row.evaluation_ref = row.evaluation_ref.clone().or_else(|| {
                    record
                        .evaluation_artifact_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                });
                row.role = "child".to_string();
                row.source(source);
            }
            self.index_branch(&record.node_id);
        }

        for stored in &records.attempt_results {
            let record = stored.item();
            let source =
                SourceRef::from_pointer(EvidenceClass::AttemptResult.as_str(), stored.pointer());
            {
                let row = self.row(&record.node_id);
                row.generation = row.generation.or(Some(record.generation));
                row.branch_id = row
                    .branch_id
                    .clone()
                    .or_else(|| Some(record.branch_id.clone()));
                row.status = row
                    .status
                    .clone()
                    .or_else(|| Some(status_text(record.status).to_string()));
                row.disposition = row
                    .disposition
                    .clone()
                    .or_else(|| Some(disposition_text(record.disposition).to_string()));
                row.evaluation_ref = row.evaluation_ref.clone().or_else(|| {
                    record
                        .evaluation_artifact_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                });
                row.result_ref = row
                    .result_ref
                    .clone()
                    .or_else(|| Some(source.ref_id.clone()));
                row.role = "child".to_string();
                row.source(source);
            }
            self.index_branch(&record.node_id);
        }
    }

    fn apply_journal(&mut self, entry: &JournalEntry, source: SourceRef) {
        if let JournalEntry::Successor(record) = entry {
            if let super::successor::State::Selected { decision, .. } = &record.state {
                if let Some(branch_id) = decision.selected_next_branch_id.as_ref() {
                    self.record_selection(
                        branch_id,
                        SelectionSource {
                            authority: "transition_journal",
                            source: source.clone(),
                        },
                    );
                    self.diagnostics.push(format!(
                        "selection observed in transition journal at {}: branch={branch_id}",
                        source.ref_id
                    ));
                }
            }
        }
    }

    fn apply_preview_selection_records(&mut self, records: &SelectionPreviewRecords) {
        if let Some(stored) = records.scheduler.as_ref() {
            let source =
                SourceRef::from_pointer(EvidenceClass::Scheduler.as_str(), stored.pointer());
            if let Some(branch_id) = stored
                .item()
                .last_continuation_decision
                .as_ref()
                .and_then(|decision| decision.selected_next_branch_id.as_deref())
                .filter(|branch_id| !branch_id.is_empty())
            {
                self.apply_preview_selection(branch_id, source);
            }
        }

        if let Some(stored) = records.branch_registry.as_ref() {
            let source =
                SourceRef::from_pointer(EvidenceClass::BranchRegistry.as_str(), stored.pointer());
            for branch_id in selected_registry_branches(stored.item()) {
                self.apply_preview_selection(&branch_id, source.clone());
            }
        }
    }

    fn apply_preview_selection(&mut self, branch_id: &str, source: SourceRef) {
        self.record_selection(
            branch_id,
            SelectionSource {
                authority: "mutable_projection",
                source: source.clone(),
            },
        );
        if self.ambiguous_branches.contains(branch_id) {
            self.diagnostics.push(format!(
                    "preview selection source {} kept unattached: branch '{branch_id}' is ambiguous across child nodes",
                    source.ref_id
                ));
        } else if let Some(node_id) = self.branch_to_node.get(branch_id).cloned() {
            self.row(&node_id).source(source.clone());
        }
    }

    fn finish(mut self, campaign_id: &str, manifest_path: &Path) -> Dashboard {
        for (branch_id, selections) in self.selected_branches.clone() {
            if let Some(node_id) = self.branch_to_node.get(&branch_id).cloned() {
                let row = self.row(&node_id);
                for selection in selections {
                    row.select(selection);
                }
                row.source_refs
                    .sort_by(|left, right| left.ref_id.cmp(&right.ref_id));
            }
        }
        for row in self.rows.values_mut() {
            row.refresh_dashboard_score();
            row.source_refs
                .sort_by(|left, right| left.ref_id.cmp(&right.ref_id));
            row.selection_sources
                .sort_by(|left, right| left.source.ref_id.cmp(&right.source.ref_id));
        }
        assign_ranks(&mut self.rows);
        let mut rows = self.rows.into_values().collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            left.generation
                .cmp(&right.generation)
                .then_with(|| left.node_id.cmp(&right.node_id))
        });
        let generations = generations(&rows);
        let cohorts = cohorts(&rows, &mut self.diagnostics);
        let trajectory = Trajectory::from_cohorts(&cohorts, &rows);
        let selected_by_generation = trajectory.steps.clone();
        self.diagnostics.extend(trajectory.diagnostics.clone());
        Dashboard {
            schema_version: SCHEMA_VERSION,
            generated_at: Utc::now().to_rfc3339(),
            campaign_id: campaign_id.to_string(),
            manifest_path: manifest_path.to_path_buf(),
            derivation: DERIVATION,
            rows,
            generations,
            cohorts,
            trajectory,
            selected_by_generation,
            child_evidence: self
                .child_evidence
                .unwrap_or_else(EvidenceProjection::empty),
            diagnostics: self.diagnostics,
        }
    }

    fn row(&mut self, node_id: &str) -> &mut Row {
        self.rows
            .entry(node_id.to_string())
            .or_insert_with(|| Row::new(node_id))
    }

    fn index_branch(&mut self, node_id: &str) {
        let Some(branch_id) = self.rows.get(node_id).and_then(|row| row.branch_id.clone()) else {
            return;
        };
        self.insert_branch_node(&branch_id, node_id, "metrics row");
    }

    fn insert_branch_node(&mut self, branch_id: &str, node_id: &str, source: &str) {
        if self.ambiguous_branches.contains(branch_id) {
            return;
        }
        if let Some(existing) = self.branch_to_node.get(branch_id) {
            if existing != node_id {
                let existing = existing.clone();
                self.branch_to_node.remove(branch_id);
                self.ambiguous_branches.insert(branch_id.to_string());
                self.diagnostics.push(format!(
                    "{source} warning: branch '{branch_id}' maps to both '{existing}' and '{node_id}'; branch-scoped evidence will not be attached"
                ));
            }
        } else {
            self.branch_to_node
                .insert(branch_id.to_string(), node_id.to_string());
        }
    }

    fn record_selection(&mut self, branch_id: &str, selection: SelectionSource) {
        let selections = self
            .selected_branches
            .entry(branch_id.to_string())
            .or_default();
        if !selections
            .iter()
            .any(|existing| existing.source.ref_id == selection.source.ref_id)
        {
            selections.push(selection);
        }
    }
}

fn assign_ranks(rows: &mut BTreeMap<String, Row>) {
    let mut by_parent_generation = BTreeMap::<(Option<String>, u32), Vec<String>>::new();
    for row in rows.values() {
        if let Some(generation) = row.generation {
            by_parent_generation
                .entry((row.parent_node_id.clone(), generation))
                .or_default()
                .push(row.node_id.clone());
        }
    }
    for node_ids in by_parent_generation.values_mut() {
        node_ids.sort_by(|left, right| {
            let left_row = rows.get(left).expect("node id from rows");
            let right_row = rows.get(right).expect("node id from rows");
            compare_rank(left_row, right_row)
        });
        for (index, node_id) in node_ids.iter().enumerate() {
            if let Some(row) = rows.get_mut(node_id) {
                row.dashboard_rank = Some(index + 1);
            }
        }
    }
}

fn compare_rank(left: &Row, right: &Row) -> Ordering {
    right
        .rank_key()
        .cmp(&left.rank_key())
        .then_with(|| left.node_id.cmp(&right.node_id))
}

fn generations(rows: &[Row]) -> Vec<Generation> {
    let mut by_generation = BTreeMap::<u32, Vec<&Row>>::new();
    for row in rows {
        if let Some(generation) = row.generation {
            by_generation.entry(generation).or_default().push(row);
        }
    }
    by_generation
        .into_iter()
        .map(|(generation, rows)| generation_summary(generation, &rows))
        .collect()
}

fn cohorts(rows: &[Row], diagnostics: &mut Vec<String>) -> Vec<Cohort> {
    let mut by_key = BTreeMap::<Key, Vec<&Row>>::new();
    let mut degraded = false;
    for row in rows {
        if let Some(generation) = row.generation {
            let key = Key {
                lineage: None,
                parent_node_id: row.parent_node_id.clone(),
                generation,
            };
            degraded = true;
            by_key.entry(key).or_default().push(row);
        }
    }
    if degraded {
        diagnostics.push(
            "cohort lineage unavailable in current records: grouped by parent_node_id and generation"
                .to_string(),
        );
    }
    by_key
        .into_iter()
        .map(|(key, rows)| cohort(key, &rows))
        .collect()
}

fn cohort(key: Key, rows: &[&Row]) -> Cohort {
    let selected_rows = rows
        .iter()
        .copied()
        .filter(|row| row.selected)
        .collect::<Vec<_>>();
    let selected = selected_rows
        .iter()
        .copied()
        .map(choice)
        .collect::<Vec<_>>();
    let top_row = rows.iter().copied().max_by(|left, right| {
        left.rank_key()
            .cmp(&right.rank_key())
            .then_with(|| right.node_id.cmp(&left.node_id))
    });
    let top = top_row.map(choice);
    let alternative_row = rows
        .iter()
        .copied()
        .filter(|row| !row.selected)
        .max_by(|left, right| {
            left.rank_key()
                .cmp(&right.rank_key())
                .then_with(|| right.node_id.cmp(&left.node_id))
        });
    let alternative = alternative_row.map(choice);
    let total_tool_calls = rows.iter().map(|row| row.total_tool_calls).sum();
    let failed_tool_calls = rows.iter().map(|row| row.failed_tool_calls).sum();
    let selected_row = if selected_rows.len() == 1 {
        selected_rows.first().copied()
    } else {
        None
    };
    let deltas = vec![Delta::against_top(
        selected_row,
        top_row,
        selected_rows.len(),
    )];
    Cohort {
        key,
        nodes: rows.len(),
        completed: rows.iter().filter(|row| completed(row)).count(),
        failed: rows.iter().filter(|row| failed(row)).count(),
        evaluations: rows
            .iter()
            .filter(|row| row.evaluation_ref.is_some() || row.compared_instances > 0)
            .count(),
        selected_count: selected.len(),
        selected,
        top,
        alternative,
        deltas,
        total_tool_calls,
        failed_tool_calls,
        failed_tool_call_rate: rate(failed_tool_calls, total_tool_calls),
        patch_attempted_instances: rows.iter().map(|row| row.patch_attempted_instances).sum(),
        applied_patch_instances: rows.iter().map(|row| row.applied_patch_instances).sum(),
        partial_patch_instances: rows.iter().map(|row| row.partial_patch_instances).sum(),
        aborted_instances: rows.iter().map(|row| row.aborted_instances).sum(),
        aborted_repair_loop_instances: rows
            .iter()
            .map(|row| row.aborted_repair_loop_instances)
            .sum(),
    }
}

fn choice(row: &Row) -> Choice {
    Choice {
        node_id: row.node_id.clone(),
        branch_id: row.branch_id.clone(),
        authority: row.selection_authority,
        dashboard_rank: row.dashboard_rank,
        disposition: row.disposition.clone(),
        dashboard_score: row.dashboard_score,
    }
}

fn generation_summary(generation: u32, rows: &[&Row]) -> Generation {
    let top_ranked = rows.iter().copied().max_by(|left, right| {
        left.rank_key()
            .cmp(&right.rank_key())
            .then_with(|| right.node_id.cmp(&left.node_id))
    });
    let selected_rows = rows
        .iter()
        .copied()
        .filter(|row| row.selected)
        .collect::<Vec<_>>();
    let selected = selected_rows.first().copied();
    let total_tool_calls = rows.iter().map(|row| row.total_tool_calls).sum();
    let failed_tool_calls = rows.iter().map(|row| row.failed_tool_calls).sum();
    Generation {
        generation,
        nodes: rows.len(),
        completed: rows.iter().filter(|row| completed(row)).count(),
        failed: rows.iter().filter(|row| failed(row)).count(),
        evaluations: rows
            .iter()
            .filter(|row| row.evaluation_ref.is_some() || row.compared_instances > 0)
            .count(),
        selected_node_id: selected.map(|row| row.node_id.clone()),
        selected_branch_id: selected.and_then(|row| row.branch_id.clone()),
        selected_authority: selected.and_then(|row| row.selection_authority),
        selected_dashboard_rank: selected.and_then(|row| row.dashboard_rank),
        top_ranked_node_id: top_ranked.map(|row| row.node_id.clone()),
        top_ranked_branch_id: top_ranked.and_then(|row| row.branch_id.clone()),
        top_dashboard_score: top_ranked.map(|row| row.dashboard_score),
        selected_dashboard_score: selected.map(|row| row.dashboard_score),
        deltas: vec![Delta::against_top(
            selected,
            top_ranked,
            selected_rows.len(),
        )],
        total_tool_calls,
        failed_tool_calls,
        failed_tool_call_rate: rate(failed_tool_calls, total_tool_calls),
        keep_count: rows
            .iter()
            .filter(|row| matches_text(row.disposition.as_deref(), "keep"))
            .count(),
        reject_count: rows
            .iter()
            .filter(|row| matches_text(row.disposition.as_deref(), "reject"))
            .count(),
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Continuity {
    ambiguous: bool,
    incomplete: bool,
}

fn mark_continuity(steps: &mut [Step], diagnostics: &mut Vec<String>) -> Continuity {
    let mut by_generation = BTreeMap::<u32, Vec<usize>>::new();
    for (index, step) in steps.iter().enumerate() {
        by_generation
            .entry(step.generation)
            .or_default()
            .push(index);
    }

    let mut previous = None::<Vec<String>>;
    let mut previous_index = None;
    let mut continuity = Continuity::default();
    for indices in by_generation.values() {
        if indices.len() != 1 {
            continuity.ambiguous = true;
            for index in indices {
                steps[*index].parent_continuity = Some("ambiguous_generation");
            }
        } else {
            let index = indices[0];
            let current_parent = steps[index].cohort.parent_node_id.clone();
            let current_node = steps[index].selected_node_id.clone();
            if let Some(previous_nodes) = previous.as_ref() {
                if previous_nodes.len() == 1 {
                    let previous_node = previous_nodes[0].as_str();
                    match current_parent.as_deref() {
                        Some(parent_node_id) if parent_node_id == previous_node => {
                            steps[index].parent_continuity = Some("continuous");
                        }
                        Some(parent_node_id) => {
                            continuity.ambiguous = true;
                            diagnostics.push(format!(
                                "trajectory projection discontinuity at generation {}: selected node {} has parent {}, previous selected node was {}",
                                steps[index].generation,
                                steps[index].selected_node_id.as_deref().unwrap_or("-"),
                                parent_node_id,
                                previous_node
                            ));
                            steps[index].parent_continuity = Some("discontinuous");
                        }
                        None => {
                            continuity.incomplete = true;
                            diagnostics.push(format!(
                                "trajectory projection parent continuity unknown at generation {}: selected node {} has no parent_node_id",
                                steps[index].generation,
                                steps[index].selected_node_id.as_deref().unwrap_or("-")
                            ));
                            steps[index].parent_continuity = Some("unknown_parent");
                        }
                    }
                } else {
                    continuity.ambiguous = true;
                    steps[index].parent_continuity = Some("ambiguous_previous");
                }
            }
            if let Some(previous_index) = previous_index {
                let delta = Delta::against_previous(&steps[index], &steps[previous_index]);
                steps[index].deltas.push(delta);
            }
            previous_index = Some(index);
            previous = current_node.map(|node| vec![node]);
            continue;
        }

        previous = Some(
            indices
                .iter()
                .filter_map(|index| steps[*index].selected_node_id.clone())
                .collect(),
        );
        previous_index = None;
    }
    continuity
}

fn display_parent(parent_node_id: Option<&str>) -> String {
    display_id_opt(parent_node_id)
}

fn choice_ids(choices: &[Choice]) -> String {
    let ids = choices
        .iter()
        .map(|choice| choice.node_id.as_str())
        .collect::<Vec<_>>()
        .join(",");
    if ids.is_empty() { "-".to_string() } else { ids }
}

fn choice_ids_short(choices: &[Choice]) -> String {
    let ids = choices
        .iter()
        .map(|choice| short_id(&choice.node_id))
        .collect::<Vec<_>>()
        .join(",");
    if ids.is_empty() { "-".to_string() } else { ids }
}

fn delta_summary(deltas: &[Delta]) -> String {
    let parts = deltas
        .iter()
        .map(|delta| {
            let basis = match delta.basis {
                Basis::Top => "top",
                Basis::Parent => "parent",
                Basis::Alternative => "alternative",
                Basis::Previous => "previous",
            };
            let against = delta
                .against
                .as_ref()
                .map(|against| short_id(&against.node_id))
                .unwrap_or_else(|| "-".to_string());
            let value = delta
                .value
                .map(|value| {
                    if value >= 0 {
                        format!("+{value}")
                    } else {
                        value.to_string()
                    }
                })
                .unwrap_or_else(|| "-".to_string());
            format!(
                "{basis}:{:?}@{against}:{value}:n{}",
                delta.state, delta.count
            )
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(",")
    }
}

#[derive(Debug, Clone, Copy)]
enum Align {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
struct Column {
    heading: &'static str,
    max_width: usize,
    align: Align,
}

impl Column {
    const fn left(heading: &'static str, max_width: usize) -> Self {
        Self {
            heading,
            max_width,
            align: Align::Left,
        }
    }

    const fn right(heading: &'static str, max_width: usize) -> Self {
        Self {
            heading,
            max_width,
            align: Align::Right,
        }
    }
}

fn print_table(columns: &[Column], rows: Vec<Vec<String>>) {
    for line in table_lines(columns, &rows) {
        println!("{line}");
    }
}

fn table_lines(columns: &[Column], rows: &[Vec<String>]) -> Vec<String> {
    let widths = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let content_width = rows
                .iter()
                .filter_map(|row| row.get(index))
                .map(|value| value.chars().count())
                .max()
                .unwrap_or(0);
            column
                .heading
                .chars()
                .count()
                .max(content_width)
                .min(column.max_width)
        })
        .collect::<Vec<_>>();

    let mut lines = Vec::with_capacity(rows.len() + 2);
    lines.push(render_table_row(
        columns,
        &widths,
        &columns
            .iter()
            .map(|column| column.heading.to_string())
            .collect::<Vec<_>>(),
    ));
    lines.push(
        widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("-+-"),
    );
    for row in rows {
        lines.push(render_table_row(columns, &widths, row));
    }
    lines
}

fn render_table_row(columns: &[Column], widths: &[usize], row: &[String]) -> String {
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let value = row.get(index).map(String::as_str).unwrap_or("-");
            let value = fit_text(value, widths[index]);
            match column.align {
                Align::Left => format!("{value:<width$}", width = widths[index]),
                Align::Right => format!("{value:>width$}", width = widths[index]),
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn print_generations(generations: &[Generation]) {
    println!("generation summaries");
    println!("{}", "-".repeat(40));
    print_table(
        &[
            Column::right("gen", 4),
            Column::right("nodes", 5),
            Column::right("done", 4),
            Column::right("fail", 4),
            Column::right("evals", 5),
            Column::right("sel_rank", 8),
            Column::left("selected", 18),
            Column::left("authority", 18),
            Column::left("top", 18),
            Column::left("deltas", 48),
            Column::right("fail_rate", 9),
        ],
        generations
            .iter()
            .map(|row| {
                vec![
                    row.generation.to_string(),
                    row.nodes.to_string(),
                    row.completed.to_string(),
                    row.failed.to_string(),
                    row.evaluations.to_string(),
                    display_opt(row.selected_dashboard_rank),
                    display_id_opt(row.selected_node_id.as_deref()),
                    row.selected_authority.unwrap_or("-").to_string(),
                    display_id_opt(row.top_ranked_node_id.as_deref()),
                    delta_summary(&row.deltas),
                    display_rate(row.failed_tool_call_rate),
                ]
            })
            .collect(),
    );
}

fn print_cohorts(cohorts: &[Cohort]) {
    println!("cohorts");
    println!("{}", "-".repeat(40));
    println!("selection");
    print_table(
        &[
            Column::right("gen", 4),
            Column::left("parent", 18),
            Column::right("nodes", 5),
            Column::right("done", 4),
            Column::right("evals", 5),
            Column::right("sel", 3),
            Column::left("selected", 28),
            Column::left("top", 18),
            Column::left("deltas", 48),
        ],
        cohorts
            .iter()
            .map(|row| {
                vec![
                    row.key.generation.to_string(),
                    display_id_opt(row.key.parent_node_id.as_deref()),
                    row.nodes.to_string(),
                    row.completed.to_string(),
                    row.evaluations.to_string(),
                    row.selected_count.to_string(),
                    choice_ids_short(&row.selected),
                    row.top
                        .as_ref()
                        .map(|choice| short_id(&choice.node_id))
                        .unwrap_or_else(|| "-".to_string()),
                    delta_summary(&row.deltas),
                ]
            })
            .collect(),
    );
    println!();
    println!("health");
    print_table(
        &[
            Column::right("gen", 4),
            Column::left("parent", 18),
            Column::left("lineage", 18),
            Column::right("patch", 5),
            Column::right("applied", 7),
            Column::right("partial", 7),
            Column::right("aborted", 7),
            Column::right("repair", 6),
            Column::right("tools", 5),
            Column::right("failed", 6),
            Column::right("fail_rate", 9),
        ],
        cohorts
            .iter()
            .map(|row| {
                vec![
                    row.key.generation.to_string(),
                    display_id_opt(row.key.parent_node_id.as_deref()),
                    display_id_opt(row.key.lineage.as_deref()),
                    row.patch_attempted_instances.to_string(),
                    row.applied_patch_instances.to_string(),
                    row.partial_patch_instances.to_string(),
                    row.aborted_instances.to_string(),
                    row.aborted_repair_loop_instances.to_string(),
                    row.total_tool_calls.to_string(),
                    row.failed_tool_calls.to_string(),
                    display_rate(row.failed_tool_call_rate),
                ]
            })
            .collect(),
    );
}

fn print_trajectory(trajectory: &Trajectory, include_diagnostics: bool) {
    println!("trajectory projection");
    println!("{}", "-".repeat(40));
    println!("coordinate: {}", trajectory.coordinate);
    println!("state: {:?}", trajectory.state);
    println!();
    println!("cohort decisions");
    print_table(
        &[
            Column::right("gen", 4),
            Column::left("parent", 18),
            Column::right("nodes", 5),
            Column::right("done", 4),
            Column::right("evals", 5),
            Column::left("decision", 8),
            Column::right("sel", 3),
            Column::left("selected", 28),
            Column::left("top", 18),
            Column::left("deltas", 48),
        ],
        trajectory
            .decisions
            .iter()
            .map(|decision| {
                vec![
                    decision.key.generation.to_string(),
                    display_parent(decision.key.parent_node_id.as_deref()),
                    decision.nodes.to_string(),
                    decision.completed.to_string(),
                    decision.evaluations.to_string(),
                    format!("{:?}", decision.state),
                    decision.selected_count.to_string(),
                    choice_ids_short(&decision.selected),
                    decision
                        .top
                        .as_ref()
                        .map(|choice| short_id(&choice.node_id))
                        .unwrap_or_else(|| "-".to_string()),
                    delta_summary(&decision.deltas),
                ]
            })
            .collect(),
    );
    println!();
    println!("unambiguous projection steps");
    print_table(
        &[
            Column::right("gen", 4),
            Column::left("parent", 18),
            Column::left("selected", 18),
            Column::left("branch", 18),
            Column::left("authority", 18),
            Column::right("rank", 4),
            Column::left("disp", 12),
            Column::right("score", 8),
            Column::left("continuity", 20),
            Column::left("deltas", 48),
        ],
        trajectory
            .steps
            .iter()
            .map(|row| {
                vec![
                    row.generation.to_string(),
                    display_parent(row.cohort.parent_node_id.as_deref()),
                    display_id_opt(row.selected_node_id.as_deref()),
                    display_id_opt(row.selected_branch_id.as_deref()),
                    row.selected_authority.unwrap_or("-").to_string(),
                    display_opt(row.selected_dashboard_rank),
                    row.selected_disposition
                        .as_deref()
                        .unwrap_or("-")
                        .to_string(),
                    display_opt(row.selected_dashboard_score),
                    row.parent_continuity.unwrap_or("-").to_string(),
                    delta_summary(&row.deltas),
                ]
            })
            .collect(),
    );
    if include_diagnostics && !trajectory.diagnostics.is_empty() {
        println!();
        println!("trajectory diagnostics");
        for diagnostic in &trajectory.diagnostics {
            println!("{diagnostic}");
        }
    }
}

fn print_rows(rows: &[Row]) {
    println!("node rows");
    println!("{}", "-".repeat(40));
    println!("selection");
    print_table(
        &[
            Column::right("gen", 4),
            Column::left("node", 18),
            Column::left("branch", 18),
            Column::left("runtime", 18),
            Column::left("selected", 8),
            Column::left("authority", 18),
            Column::right("rank", 4),
            Column::right("score", 8),
            Column::left("disp", 16),
        ],
        rows.iter()
            .map(|row| {
                vec![
                    display_opt(row.generation),
                    short_id(&row.node_id),
                    display_id_opt(row.branch_id.as_deref()),
                    display_id_opt(row.runtime_id.as_deref()),
                    yes_no(row.selected).to_string(),
                    row.selection_authority.unwrap_or("-").to_string(),
                    display_opt(row.dashboard_rank),
                    row.dashboard_score.to_string(),
                    row.disposition.as_deref().unwrap_or("-").to_string(),
                ]
            })
            .collect(),
    );
    println!();
    println!("health");
    print_table(
        &[
            Column::right("gen", 4),
            Column::left("node", 18),
            Column::left("status", 16),
            Column::right("cmp", 3),
            Column::right("oracle", 6),
            Column::right("conv", 4),
            Column::right("patch", 5),
            Column::right("applied", 7),
            Column::right("partial", 7),
            Column::right("aborted", 7),
            Column::right("repair", 6),
            Column::right("tools", 5),
            Column::right("failed", 6),
            Column::right("fail_rate", 9),
        ],
        rows.iter()
            .map(|row| {
                vec![
                    display_opt(row.generation),
                    short_id(&row.node_id),
                    row.status.as_deref().unwrap_or("-").to_string(),
                    row.compared_instances.to_string(),
                    row.oracle_eligible_instances.to_string(),
                    row.converged_instances.to_string(),
                    row.patch_attempted_instances.to_string(),
                    row.applied_patch_instances.to_string(),
                    row.partial_patch_instances.to_string(),
                    row.aborted_instances.to_string(),
                    row.aborted_repair_loop_instances.to_string(),
                    row.total_tool_calls.to_string(),
                    row.failed_tool_calls.to_string(),
                    display_rate(row.failed_tool_call_rate),
                ]
            })
            .collect(),
    );
}

fn totals_from_compared(compared: &[ComparedRunEvidence]) -> Totals {
    let mut totals = Totals::default();
    totals.compared_instances = compared.len();
    for instance in compared {
        let Some(metrics) = instance.treatment_metrics.as_ref() else {
            continue;
        };
        add_metrics(metrics, &mut totals);
    }
    totals
}

fn add_metrics(metrics: &OperationalRunMetrics, totals: &mut Totals) {
    if metrics.oracle_eligible {
        totals.oracle_eligible_instances += 1;
    }
    if metrics.convergence {
        totals.converged_instances += 1;
    }
    if metrics.submission_artifact_state == SubmissionArtifactState::Nonempty {
        totals.nonempty_submission_instances += 1;
    }
    match metrics.patch_apply_state {
        PatchApplyState::Applied => totals.applied_patch_instances += 1,
        PatchApplyState::Partial => totals.partial_patch_instances += 1,
        PatchApplyState::No => totals.no_patch_instances += 1,
    }
    if metrics.patch_attempted {
        totals.patch_attempted_instances += 1;
    }
    if metrics.nonempty_valid_patch {
        totals.nonempty_valid_patch_instances += 1;
    }
    match metrics.submission_artifact_state {
        SubmissionArtifactState::Missing => totals.missing_submission_instances += 1,
        SubmissionArtifactState::Empty => totals.empty_submission_instances += 1,
        SubmissionArtifactState::NotRecorded
        | SubmissionArtifactState::NotApplicable
        | SubmissionArtifactState::Nonempty => {}
    }
    totals.partial_patch_failures += metrics.partial_patch_failures;
    totals.same_file_patch_retries += metrics.same_file_patch_retry_count;
    totals.same_file_patch_max_streak = totals
        .same_file_patch_max_streak
        .max(metrics.same_file_patch_max_streak);
    if metrics.aborted {
        totals.aborted_instances += 1;
    }
    if metrics.aborted_repair_loop {
        totals.aborted_repair_loop_instances += 1;
    }
    totals.total_tool_calls += metrics.tool_calls_total;
    totals.failed_tool_calls += metrics.tool_calls_failed;
}

fn selected_registry_branches(registry: &Prototype1BranchRegistry) -> BTreeSet<String> {
    let mut selected = BTreeSet::new();
    for source in &registry.source_nodes {
        if let Some(branch_id) = source
            .selected_branch_id
            .as_deref()
            .filter(|branch_id| !branch_id.is_empty())
        {
            selected.insert(branch_id.to_string());
        }
        for branch in &source.branches {
            if branch.status == TreatmentBranchStatus::Selected && !branch.branch_id.is_empty() {
                selected.insert(branch.branch_id.clone());
            }
        }
    }
    for target in &registry.active_targets {
        if let Some(branch_id) = target
            .active_branch_id
            .as_deref()
            .filter(|branch_id| !branch_id.is_empty())
        {
            selected.insert(branch_id.to_string());
        }
    }
    selected
}

fn strongest_selection_authority(sources: &[SelectionSource]) -> Option<&'static str> {
    if sources
        .iter()
        .any(|source| source.authority == "transition_journal")
    {
        Some("transition_journal")
    } else if sources
        .iter()
        .any(|source| source.authority == "mutable_projection")
    {
        Some("mutable_projection")
    } else {
        None
    }
}

fn completed(row: &Row) -> bool {
    matches_text(row.status.as_deref(), "succeeded")
        || matches_text(row.disposition.as_deref(), "succeeded")
        || row.evaluation_ref.is_some()
}

fn failed(row: &Row) -> bool {
    matches_text(row.status.as_deref(), "failed")
        || matches_text(row.disposition.as_deref(), "failed")
}

fn rate(numerator: usize, denominator: usize) -> Option<f64> {
    if denominator == 0 {
        None
    } else {
        Some(numerator as f64 / denominator as f64)
    }
}

fn generation_matches(row_generation: Option<u32>, filter: Option<u32>) -> bool {
    filter
        .map(|generation| row_generation == Some(generation))
        .unwrap_or(true)
}

fn matches_text(value: Option<&str>, expected: &str) -> bool {
    value
        .map(|value| value.eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

fn status_text(status: Prototype1NodeStatus) -> &'static str {
    match status {
        Prototype1NodeStatus::Planned => "planned",
        Prototype1NodeStatus::WorkspaceStaged => "workspace_staged",
        Prototype1NodeStatus::BinaryBuilt => "binary_built",
        Prototype1NodeStatus::Running => "running",
        Prototype1NodeStatus::Succeeded => "succeeded",
        Prototype1NodeStatus::Failed => "failed",
    }
}

fn disposition_text(disposition: Prototype1RunnerDisposition) -> &'static str {
    match disposition {
        Prototype1RunnerDisposition::Succeeded => "succeeded",
        Prototype1RunnerDisposition::CompileFailed => "compile_failed",
        Prototype1RunnerDisposition::TreatmentFailed => "treatment_failed",
    }
}

fn display_opt<T: std::fmt::Display>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn display_id_opt(value: Option<&str>) -> String {
    value.map(short_id).unwrap_or_else(|| "-".to_string())
}

fn short_id(value: &str) -> String {
    const MAX_WIDTH: usize = 18;
    fit_text(value, MAX_WIDTH)
}

fn fit_text(value: &str, max_width: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_width {
        return value.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    if max_width == 1 {
        return "~".to_string();
    }

    let prefix_width = max_width.saturating_sub(1);
    let mut output = value.chars().take(prefix_width).collect::<String>();
    output.push('~');
    output
}

fn display_rate(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_string())
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use crate::intervention::{
        PROTOTYPE1_BRANCH_REGISTRY_SCHEMA_VERSION, PROTOTYPE1_SCHEDULER_SCHEMA_VERSION,
        PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1ContinuationDecision,
        Prototype1ContinuationDisposition, RecordStore,
    };

    use super::*;
    use crate::cli::prototype1_state::journal::{
        JournalEntry, PrototypeJournal, prototype1_transition_journal_path,
    };
    use crate::cli::prototype1_state::successor;

    #[test]
    fn dashboard_ranks_selected_child_with_sources() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        let prototype = tmp.path().join("prototype1");
        let node_a = prototype.join("nodes/node-a");
        let node_b = prototype.join("nodes/node-b");
        fs::create_dir_all(node_a.join("results")).expect("node a");
        fs::create_dir_all(node_b.join("results")).expect("node b");
        fs::create_dir_all(prototype.join("evaluations")).expect("evals");
        fs::write(
            node_a.join("node.json"),
            node_record(&prototype, "node-a", None, 1, "branch-a"),
        )
        .expect("node a record");
        fs::write(
            node_b.join("node.json"),
            node_record(&prototype, "node-b", None, 1, "branch-b"),
        )
        .expect("node b record");
        fs::write(
            node_a.join("results/runtime-a.json"),
            runner_result(&prototype, "node-a", 1, "branch-a"),
        )
        .expect("result a");
        fs::write(
            node_b.join("results/runtime-b.json"),
            runner_result(&prototype, "node-b", 1, "branch-b"),
        )
        .expect("result b");
        fs::write(
            prototype.join("evaluations/branch-a.json"),
            evaluation("branch-a", "reject", 1, 0, 5, 2),
        )
        .expect("eval a");
        fs::write(
            prototype.join("evaluations/branch-b.json"),
            evaluation("branch-b", "keep", 1, 1, 4, 0),
        )
        .expect("eval b");
        fs::write(
            prototype.join("scheduler.json"),
            scheduler_with_selection(Some("branch-b")),
        )
        .expect("scheduler");

        let dashboard = build("campaign-a", &manifest).expect("dashboard");
        let summary = dashboard
            .generations
            .iter()
            .find(|generation| generation.generation == 1)
            .expect("generation summary");

        assert_eq!(summary.nodes, 2);
        assert_eq!(summary.selected_node_id.as_deref(), Some("node-b"));
        assert_eq!(summary.selected_authority, Some("mutable_projection"));
        assert_eq!(summary.selected_dashboard_rank, Some(1));
        let top_delta = delta(&summary.deltas, Basis::Top);
        assert_eq!(top_delta.state, DeltaState::Known);
        assert_eq!(top_delta.value, Some(0));
        let selected = dashboard
            .rows
            .iter()
            .find(|row| row.node_id == "node-b")
            .expect("selected row");
        assert!(selected.selected);
        assert_eq!(selected.selection_authority, Some("mutable_projection"));
        assert!(
            selected
                .selection_sources
                .iter()
                .any(|source| source.source.class == "scheduler")
        );
        assert_eq!(
            selected.dashboard_score_derivation.kind,
            "heuristic_projection"
        );
        assert_eq!(
            selected.dashboard_score_derivation.rank_relation,
            "separate_from_dashboard_rank"
        );
        assert_eq!(
            selected.dashboard_score,
            selected.dashboard_score_derivation.total
        );
    }

    #[test]
    fn dashboard_exposes_child_evidence_projection_summary() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        let prototype = tmp.path().join("prototype1");
        let node = prototype.join("nodes/node-a");
        fs::create_dir_all(&node).expect("node dir");
        fs::create_dir_all(prototype.join("evaluations")).expect("evals");
        fs::write(
            node.join("node.json"),
            node_record(&prototype, "node-a", Some("parent-a"), 2, "branch-a"),
        )
        .expect("node record");
        fs::write(
            prototype.join("evaluations/branch-a.json"),
            evaluation("branch-a", "keep", 1, 0, 0, 0),
        )
        .expect("evaluation");

        let dashboard = build("campaign-a", &manifest).expect("dashboard");
        let row = dashboard
            .rows
            .iter()
            .find(|row| row.node_id == "node-a")
            .expect("node row");

        assert_eq!(
            dashboard.child_evidence.join,
            "history_preview.child_evidence"
        );
        assert_eq!(dashboard.child_evidence.children, 1);
        assert_eq!(dashboard.child_evidence.evaluation_sources, 1);
        assert_eq!(dashboard.child_evidence.diagnostics, 0);
        assert!(
            dashboard
                .child_evidence
                .source_treatments
                .iter()
                .any(|treatment| treatment.class == "evaluation"
                    && treatment.preview_import_treatment == "admitted_preview_raw"
                    && treatment.sources == 1)
        );
        assert!(row.source_refs.iter().any(|source| {
            source.class == "evaluation" && source.ref_id.ends_with("evaluations/branch-a.json")
        }));
    }

    #[test]
    fn branch_only_evaluation_is_not_attached_to_ambiguous_child_branch() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        let prototype = tmp.path().join("prototype1");
        fs::create_dir_all(prototype.join("evaluations")).expect("evals");
        for node_id in ["node-a", "node-b"] {
            let node = prototype.join(format!("nodes/{node_id}"));
            fs::create_dir_all(&node).expect("node dir");
            fs::write(
                node.join("node.json"),
                node_record(&prototype, node_id, None, 1, "branch-shared"),
            )
            .expect("node record");
        }
        fs::write(
            prototype.join("evaluations/branch-shared.json"),
            evaluation("branch-shared", "keep", 1, 1, 3, 0),
        )
        .expect("evaluation");

        let dashboard = build("campaign-a", &manifest).expect("dashboard");
        let node_a = dashboard
            .rows
            .iter()
            .find(|row| row.node_id == "node-a")
            .expect("node-a row");
        let node_b = dashboard
            .rows
            .iter()
            .find(|row| row.node_id == "node-b")
            .expect("node-b row");

        assert_eq!(node_a.evaluation_ref, None);
        assert_eq!(node_b.evaluation_ref, None);
        assert_eq!(node_a.compared_instances, 0);
        assert_eq!(node_b.compared_instances, 0);
        assert!(dashboard.diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("branch 'branch-shared' appears under multiple child nodes")
        }));
        assert!(
            !node_a
                .source_refs
                .iter()
                .chain(node_b.source_refs.iter())
                .any(|source| source.class == "evaluation")
        );
    }

    #[test]
    fn dashboard_score_is_a_separate_projection_from_rank() {
        let mut higher_rank = Row::new("higher-rank");
        higher_rank.compared_instances = 1;
        let mut lower_rank = Row::new("lower-rank");
        lower_rank.compared_instances = 1;
        lower_rank.aborted_instances = 1;

        higher_rank.refresh_dashboard_score();
        lower_rank.refresh_dashboard_score();

        assert_eq!(higher_rank.dashboard_score, lower_rank.dashboard_score);
        assert!(higher_rank.rank_key() > lower_rank.rank_key());
        assert_eq!(
            higher_rank.dashboard_score_derivation.rank_relation,
            "separate_from_dashboard_rank"
        );
    }

    #[test]
    fn dashboard_score_refresh_recomputes_after_metrics_are_set() {
        let mut row = Row::new("node-a");
        let metrics = Totals {
            oracle_eligible_instances: 2,
            converged_instances: 1,
            nonempty_submission_instances: 1,
            applied_patch_instances: 1,
            failed_tool_calls: 3,
            ..Default::default()
        };

        row.apply_metrics(&metrics);
        row.disposition = Some("keep".to_string());
        row.refresh_dashboard_score();

        assert_eq!(row.dashboard_score_derivation.total, row.dashboard_score);
        assert_eq!(row.dashboard_score, 12_847);
    }

    #[test]
    fn journal_selection_attaches_transition_source() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        let prototype = tmp.path().join("prototype1");
        let node = prototype.join("nodes/node-b");
        fs::create_dir_all(&node).expect("node dir");
        fs::write(
            node.join("node.json"),
            node_record(&prototype, "node-b", None, 1, "branch-b"),
        )
        .expect("node record");

        let decision = Prototype1ContinuationDecision {
            disposition: Prototype1ContinuationDisposition::ContinueReady,
            selected_next_branch_id: Some("branch-b".to_string()),
            selected_branch_disposition: Some("keep".to_string()),
            next_generation: 2,
            total_nodes_after_continue: 1,
        };
        let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest));
        journal
            .append(JournalEntry::Successor(successor::Record::selected(
                "campaign-a".to_string(),
                "node-b".to_string(),
                decision,
            )))
            .expect("journal append");

        let dashboard = build("campaign-a", &manifest).expect("dashboard");
        let selected = dashboard
            .rows
            .iter()
            .find(|row| row.node_id == "node-b")
            .expect("selected row");

        assert!(selected.selected);
        assert_eq!(selected.selection_authority, Some("transition_journal"));
        assert!(
            selected
                .selection_sources
                .iter()
                .any(|source| source.authority == "transition_journal"
                    && source.source.class == "transition_journal"
                    && source.source.ref_id.ends_with("#L1"))
        );
    }

    #[test]
    fn cohorts_keep_parent_generation_structure() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        let prototype = tmp.path().join("prototype1");
        fs::create_dir_all(prototype.join("evaluations")).expect("evals");
        for (node_id, parent_node_id, branch_id) in [
            ("node-a", "parent-1", "branch-a"),
            ("node-b", "parent-1", "branch-b"),
            ("node-c", "parent-2", "branch-c"),
        ] {
            let node = prototype.join(format!("nodes/{node_id}"));
            fs::create_dir_all(node.join("results")).expect("node dir");
            fs::write(
                node.join("node.json"),
                node_record(&prototype, node_id, Some(parent_node_id), 2, branch_id),
            )
            .expect("node record");
            fs::write(
                node.join("results/runtime.json"),
                runner_result(&prototype, node_id, 2, branch_id),
            )
            .expect("result");
            fs::write(
                prototype.join(format!("evaluations/{branch_id}.json")),
                evaluation(branch_id, "keep", 1, 0, 3, 0),
            )
            .expect("evaluation");
        }
        fs::write(
            prototype.join("branches.json"),
            branch_registry_with_selections(&["branch-a", "branch-c"]),
        )
        .expect("branch registry");

        let dashboard = build("campaign-a", &manifest).expect("dashboard");
        let cohorts = dashboard
            .cohorts
            .iter()
            .filter(|cohort| cohort.key.generation == 2)
            .collect::<Vec<_>>();

        assert_eq!(cohorts.len(), 2);
        let first = cohorts
            .iter()
            .find(|cohort| cohort.key.parent_node_id.as_deref() == Some("parent-1"))
            .expect("parent-1 cohort");
        let second = cohorts
            .iter()
            .find(|cohort| cohort.key.parent_node_id.as_deref() == Some("parent-2"))
            .expect("parent-2 cohort");

        assert_eq!(first.nodes, 2);
        assert_eq!(first.selected_count, 1);
        assert_eq!(first.selected[0].node_id, "node-a");
        assert_eq!(second.nodes, 1);
        assert_eq!(second.selected_count, 1);
        assert_eq!(second.selected[0].node_id, "node-c");

        assert_eq!(dashboard.trajectory.state, ProjectionState::Ambiguous);
        assert_eq!(dashboard.trajectory.steps.len(), 2);
        assert!(dashboard.trajectory.diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("generation 2 has selected rows in multiple parent cohorts")
        }));
    }

    #[test]
    fn trajectory_records_one_selected_row_as_unambiguous_projection_step() {
        let dashboard = dashboard_from_nodes(
            &[
                ("node-a", Some("parent-1"), 1, "branch-a"),
                ("node-b", Some("parent-1"), 1, "branch-b"),
            ],
            &["branch-b"],
        );

        assert_eq!(
            dashboard.trajectory.coordinate,
            "degraded_parent_generation"
        );
        assert_eq!(dashboard.trajectory.state, ProjectionState::Unambiguous);
        assert_eq!(dashboard.trajectory.decisions.len(), 1);
        assert_eq!(dashboard.trajectory.decisions[0].state, DecisionState::One);
        assert_eq!(dashboard.trajectory.steps.len(), 1);
        assert_eq!(
            dashboard.trajectory.steps[0].selected_node_id.as_deref(),
            Some("node-b")
        );
        assert_eq!(
            dashboard.trajectory.steps[0]
                .cohort
                .parent_node_id
                .as_deref(),
            Some("parent-1")
        );
    }

    #[test]
    fn trajectory_step_records_known_parent_and_alternative_deltas() {
        let dashboard = dashboard_from_cases(
            &[
                case("parent-1", None, 0, "branch-parent", "reject", 1, 0, 4, 0),
                case(
                    "node-a",
                    Some("parent-1"),
                    1,
                    "branch-a",
                    "reject",
                    1,
                    0,
                    4,
                    1,
                ),
                case(
                    "node-b",
                    Some("parent-1"),
                    1,
                    "branch-b",
                    "keep",
                    1,
                    1,
                    4,
                    0,
                ),
            ],
            &["branch-b"],
        );
        let step = dashboard.trajectory.steps.first().expect("trajectory step");
        let selected = dashboard
            .rows
            .iter()
            .find(|row| row.node_id == "node-b")
            .expect("selected row");
        let parent = dashboard
            .rows
            .iter()
            .find(|row| row.node_id == "parent-1")
            .expect("parent row");
        let alternative = dashboard
            .rows
            .iter()
            .find(|row| row.node_id == "node-a")
            .expect("alternative row");

        let parent_delta = delta(&step.deltas, Basis::Parent);
        assert_eq!(parent_delta.state, DeltaState::Known);
        assert_eq!(
            parent_delta
                .against
                .as_ref()
                .map(|point| point.node_id.as_str()),
            Some("parent-1")
        );
        assert_eq!(
            parent_delta.value,
            Some(selected.dashboard_score - parent.dashboard_score)
        );

        let alternative_delta = delta(&step.deltas, Basis::Alternative);
        assert_eq!(alternative_delta.state, DeltaState::Known);
        assert_eq!(
            alternative_delta
                .against
                .as_ref()
                .map(|point| point.node_id.as_str()),
            Some("node-a")
        );
        assert_eq!(
            alternative_delta.value,
            Some(selected.dashboard_score - alternative.dashboard_score)
        );
    }

    #[test]
    fn trajectory_step_marks_missing_parent_row_delta() {
        let dashboard = dashboard_from_cases(
            &[case(
                "node-b",
                Some("missing-parent"),
                1,
                "branch-b",
                "keep",
                1,
                1,
                4,
                0,
            )],
            &["branch-b"],
        );
        let step = dashboard.trajectory.steps.first().expect("trajectory step");
        let parent_delta = delta(&step.deltas, Basis::Parent);

        assert_eq!(parent_delta.state, DeltaState::Missing);
        assert_eq!(parent_delta.value, None);
        assert_eq!(parent_delta.count, 0);
        assert_eq!(
            parent_delta
                .against
                .as_ref()
                .map(|point| point.node_id.as_str()),
            Some("missing-parent")
        );
    }

    #[test]
    fn trajectory_preserves_multiple_selected_rows_in_one_cohort() {
        let dashboard = dashboard_from_nodes(
            &[
                ("node-a", Some("parent-1"), 1, "branch-a"),
                ("node-b", Some("parent-1"), 1, "branch-b"),
            ],
            &["branch-a", "branch-b"],
        );

        assert_eq!(dashboard.trajectory.state, ProjectionState::Ambiguous);
        assert_eq!(dashboard.trajectory.decisions.len(), 1);
        assert_eq!(dashboard.trajectory.decisions[0].state, DecisionState::Many);
        assert_eq!(dashboard.trajectory.decisions[0].selected_count, 2);
        let alternative_delta = delta(
            &dashboard.trajectory.decisions[0].deltas,
            Basis::Alternative,
        );
        assert_eq!(alternative_delta.state, DeltaState::Ambiguous);
        assert_eq!(
            alternative_delta
                .against
                .as_ref()
                .map(|point| point.node_id.as_str()),
            None
        );
        assert_eq!(alternative_delta.count, 2);
        assert!(dashboard.trajectory.steps.is_empty());
        assert!(dashboard.trajectory.diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("cohort is ambiguous")
                && diagnostic.contains("node-a")
                && diagnostic.contains("node-b")
        }));
    }

    #[test]
    fn trajectory_decision_marks_alternative_delta_ambiguous_for_multiple_selected_rows() {
        let dashboard = dashboard_from_cases(
            &[
                case(
                    "node-a",
                    Some("parent-1"),
                    1,
                    "branch-a",
                    "keep",
                    1,
                    1,
                    4,
                    0,
                ),
                case(
                    "node-b",
                    Some("parent-1"),
                    1,
                    "branch-b",
                    "keep",
                    1,
                    1,
                    4,
                    0,
                ),
                case(
                    "node-c",
                    Some("parent-1"),
                    1,
                    "branch-c",
                    "reject",
                    1,
                    0,
                    4,
                    2,
                ),
            ],
            &["branch-a", "branch-b"],
        );
        let decision = dashboard
            .trajectory
            .decisions
            .first()
            .expect("trajectory decision");
        let alternative_delta = delta(&decision.deltas, Basis::Alternative);

        assert_eq!(decision.state, DecisionState::Many);
        assert!(dashboard.trajectory.steps.is_empty());
        assert_eq!(alternative_delta.state, DeltaState::Ambiguous);
        assert_eq!(alternative_delta.count, 2);
        assert_eq!(
            alternative_delta
                .against
                .as_ref()
                .map(|point| point.node_id.as_str()),
            Some("node-c")
        );
        assert_eq!(alternative_delta.value, None);
    }

    #[test]
    fn trajectory_preserves_selected_rows_in_multiple_generation_cohorts() {
        let dashboard = dashboard_from_nodes(
            &[
                ("node-a", Some("parent-1"), 2, "branch-a"),
                ("node-b", Some("parent-1"), 2, "branch-b"),
                ("node-c", Some("parent-2"), 2, "branch-c"),
            ],
            &["branch-a", "branch-c"],
        );

        assert_eq!(dashboard.trajectory.state, ProjectionState::Ambiguous);
        assert_eq!(dashboard.trajectory.decisions.len(), 2);
        assert_eq!(dashboard.trajectory.steps.len(), 2);
        assert!(dashboard.trajectory.diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("generation 2 has selected rows in multiple parent cohorts")
                && diagnostic.contains("parent parent-1")
                && diagnostic.contains("parent parent-2")
        }));
    }

    #[test]
    fn trajectory_keeps_unselected_cohorts_neutral() {
        let dashboard = dashboard_from_nodes(
            &[
                ("node-a", Some("parent-1"), 1, "branch-a"),
                ("node-b", Some("parent-1"), 1, "branch-b"),
            ],
            &[],
        );

        assert_eq!(dashboard.trajectory.state, ProjectionState::Unambiguous);
        assert_eq!(dashboard.trajectory.decisions.len(), 1);
        assert_eq!(dashboard.trajectory.decisions[0].state, DecisionState::None);
        assert!(dashboard.trajectory.steps.is_empty());
        assert!(!dashboard.trajectory.diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("no selected row") || diagnostic.contains("no selected")
        }));
    }

    #[test]
    fn trajectory_marks_unknown_parent_continuity_incomplete() {
        let dashboard = dashboard_from_nodes(
            &[
                ("node-a", Some("parent-1"), 1, "branch-a"),
                ("node-b", None, 2, "branch-b"),
            ],
            &["branch-a", "branch-b"],
        );

        assert_eq!(dashboard.trajectory.state, ProjectionState::Incomplete);
        let second = dashboard
            .trajectory
            .steps
            .iter()
            .find(|step| step.generation == 2)
            .expect("second generation step");
        assert_eq!(second.parent_continuity, Some("unknown_parent"));
        assert!(dashboard.trajectory.diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("parent continuity unknown at generation 2")
                && diagnostic.contains("node-b")
        }));
    }

    #[test]
    fn trajectory_generation_slice_preserves_continuity_metadata() {
        let dashboard = dashboard_from_nodes(
            &[
                ("node-a", Some("parent-1"), 1, "branch-a"),
                ("node-b", None, 2, "branch-b"),
            ],
            &["branch-a", "branch-b"],
        );

        let slice = dashboard.trajectory.slice(Some(2));

        assert_eq!(slice.state, ProjectionState::Incomplete);
        assert_eq!(slice.decisions.len(), 1);
        assert_eq!(slice.steps.len(), 1);
        assert_eq!(slice.steps[0].parent_continuity, Some("unknown_parent"));
        assert!(slice.diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("parent continuity unknown at generation 2")
        }));
    }

    #[test]
    fn cohort_top_delta_is_ambiguous_for_multiple_selected_rows() {
        let dashboard = dashboard_from_nodes(
            &[
                ("node-a", Some("parent-1"), 1, "branch-a"),
                ("node-b", Some("parent-1"), 1, "branch-b"),
            ],
            &["branch-a", "branch-b"],
        );
        let cohort = dashboard.cohorts.first().expect("cohort");
        let top_delta = delta(&cohort.deltas, Basis::Top);

        assert_eq!(cohort.selected_count, 2);
        assert_eq!(top_delta.state, DeltaState::Ambiguous);
        assert_eq!(top_delta.value, None);
    }

    #[test]
    fn rows_include_richer_operational_metrics() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        let prototype = tmp.path().join("prototype1");
        let node = prototype.join("nodes/node-a");
        fs::create_dir_all(node.join("results")).expect("node dir");
        fs::create_dir_all(prototype.join("evaluations")).expect("evals");
        fs::write(
            node.join("node.json"),
            node_record(&prototype, "node-a", None, 1, "branch-a"),
        )
        .expect("node record");
        fs::write(
            node.join("results/runtime-a.json"),
            runner_result(&prototype, "node-a", 1, "branch-a"),
        )
        .expect("result");
        fs::write(
            prototype.join("evaluations/branch-a.json"),
            evaluation_with_instances(
                &prototype,
                "branch-a",
                "reject",
                vec![serde_json::json!({
                    "instance_id": "instance-a",
                    "status": "complete",
                    "treatment_metrics": {
                        "oracle_eligible": false,
                        "convergence": false,
                        "submission_artifact_state": "missing",
                        "patch_attempted": true,
                        "patch_apply_state": "partial",
                        "partial_patch_failures": 2,
                        "same_file_patch_retry_count": 3,
                        "same_file_patch_max_streak": 3,
                        "aborted": true,
                        "aborted_repair_loop": true,
                        "nonempty_valid_patch": false,
                        "tool_calls_total": 9,
                        "tool_calls_failed": 4
                    }
                })],
            ),
        )
        .expect("evaluation");

        let dashboard = build("campaign-a", &manifest).expect("dashboard");
        let row = dashboard
            .rows
            .iter()
            .find(|row| row.node_id == "node-a")
            .expect("node row");

        assert_eq!(row.patch_attempted_instances, 1);
        assert_eq!(row.partial_patch_instances, 1);
        assert_eq!(row.applied_patch_instances, 0);
        assert_eq!(row.missing_submission_instances, 1);
        assert_eq!(row.partial_patch_failures, 2);
        assert_eq!(row.same_file_patch_retries, 3);
        assert_eq!(row.same_file_patch_max_streak, 3);
        assert_eq!(row.aborted_instances, 1);
        assert_eq!(row.aborted_repair_loop_instances, 1);
        assert_eq!(row.failed_tool_calls, 4);
        assert_eq!(row.dashboard_score, -4);
        assert_eq!(row.dashboard_score_derivation.keep_disposition, 0);
        assert_eq!(row.dashboard_score_derivation.oracle_eligible_instances, 0);
        assert_eq!(row.dashboard_score_derivation.converged_instances, 0);
        assert_eq!(
            row.dashboard_score_derivation.nonempty_submission_instances,
            0
        );
        assert_eq!(row.dashboard_score_derivation.applied_patch_instances, 0);
        assert_eq!(row.dashboard_score_derivation.failed_tool_calls, -4);
        assert_eq!(row.dashboard_score_derivation.total, row.dashboard_score);

        let value = serde_json::to_value(row).expect("serialize row");
        assert_eq!(
            value["dashboard_score_derivation"]["kind"],
            "heuristic_projection"
        );
        assert_eq!(
            value["dashboard_score_derivation"]["rank_relation"],
            "separate_from_dashboard_rank"
        );
        assert_eq!(
            value["dashboard_score_derivation"]["total"],
            row.dashboard_score
        );
    }

    struct NodeCase<'a> {
        node_id: &'a str,
        parent_node_id: Option<&'a str>,
        generation: u32,
        branch_id: &'a str,
        disposition: &'a str,
        compared: usize,
        oracle: usize,
        tool_calls: usize,
        failed: usize,
    }

    fn case<'a>(
        node_id: &'a str,
        parent_node_id: Option<&'a str>,
        generation: u32,
        branch_id: &'a str,
        disposition: &'a str,
        compared: usize,
        oracle: usize,
        tool_calls: usize,
        failed: usize,
    ) -> NodeCase<'a> {
        NodeCase {
            node_id,
            parent_node_id,
            generation,
            branch_id,
            disposition,
            compared,
            oracle,
            tool_calls,
            failed,
        }
    }

    fn delta(deltas: &[Delta], basis: Basis) -> &Delta {
        deltas
            .iter()
            .find(|delta| delta.basis == basis)
            .expect("delta")
    }

    fn evaluation(
        branch_id: &str,
        disposition: &str,
        compared: usize,
        oracle: usize,
        tool_calls: usize,
        failed: usize,
    ) -> String {
        let instances = (0..compared)
            .map(|index| {
                serde_json::json!({
                    "instance_id": format!("instance-{index}"),
                    "status": "complete",
                    "treatment_metrics": {
                        "oracle_eligible": index < oracle,
                        "convergence": true,
                        "submission_artifact_state": if index < oracle { "nonempty" } else { "empty" },
                        "patch_attempted": false,
                        "patch_apply_state": "applied",
                        "partial_patch_failures": 0,
                        "same_file_patch_retry_count": 0,
                        "same_file_patch_max_streak": 0,
                        "aborted": false,
                        "aborted_repair_loop": false,
                        "nonempty_valid_patch": false,
                        "tool_calls_total": tool_calls,
                        "tool_calls_failed": failed
                    }
                })
            })
            .collect::<Vec<_>>();
        evaluation_with_instances(
            Path::new("/tmp/prototype1"),
            branch_id,
            disposition,
            instances,
        )
    }

    fn evaluation_with_instances(
        prototype: &Path,
        branch_id: &str,
        disposition: &str,
        instances: Vec<serde_json::Value>,
    ) -> String {
        serde_json::json!({
            "baseline_campaign_id": "baseline-campaign",
            "branch_id": branch_id,
            "treatment_campaign_id": "campaign-a",
            "branch_registry_path": prototype.join("branches.json"),
            "evaluation_artifact_path": prototype.join(format!("evaluations/{branch_id}.json")),
            "treatment_campaign_manifest": prototype.parent().unwrap_or_else(|| Path::new(".")).join("campaign.json"),
            "treatment_closure_state_path": prototype.join("closure-state.json"),
            "overall_disposition": disposition,
            "reasons": ["test"],
            "compared_instances": instances
        })
        .to_string()
    }

    fn node_record(
        prototype: &Path,
        node_id: &str,
        parent_node_id: Option<&str>,
        generation: u32,
        branch_id: &str,
    ) -> String {
        let node_dir = prototype.join("nodes").join(node_id);
        let mut record = serde_json::json!({
            "schema_version": PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION,
            "node_id": node_id,
            "generation": generation,
            "instance_id": "instance-a",
            "source_state_id": "baseline-run",
            "branch_id": branch_id,
            "candidate_id": format!("candidate-{node_id}"),
            "target_relpath": "crates/ploke-core/tool_text/non_semantic_patch.md",
            "node_dir": node_dir,
            "workspace_root": prototype.join(format!("workspaces/{node_id}")),
            "binary_path": prototype.join(format!("nodes/{node_id}/bin/ploke-eval")),
            "runner_request_path": prototype.join(format!("nodes/{node_id}/runner-request.json")),
            "runner_result_path": prototype.join(format!("nodes/{node_id}/runner-result.json")),
            "status": "succeeded",
            "created_at": "2026-04-28T00:00:00Z",
            "updated_at": "2026-04-28T00:00:00Z"
        });
        if let Some(parent_node_id) = parent_node_id {
            record["parent_node_id"] = serde_json::json!(parent_node_id);
        }
        record.to_string()
    }

    fn runner_result(prototype: &Path, node_id: &str, generation: u32, branch_id: &str) -> String {
        serde_json::json!({
            "schema_version": PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION,
            "campaign_id": "campaign-a",
            "node_id": node_id,
            "generation": generation,
            "branch_id": branch_id,
            "status": "succeeded",
            "disposition": "succeeded",
            "treatment_campaign_id": "campaign-a",
            "evaluation_artifact_path": prototype.join(format!("evaluations/{branch_id}.json")),
            "recorded_at": "2026-04-28T00:00:00Z"
        })
        .to_string()
    }

    fn scheduler_with_selection(selected_branch_id: Option<&str>) -> String {
        let last_continuation_decision = selected_branch_id.map(|branch_id| {
            serde_json::json!({
                "disposition": "continue_ready",
                "selected_next_branch_id": branch_id,
                "selected_branch_disposition": "keep",
                "next_generation": 2,
                "total_nodes_after_continue": 1
            })
        });
        serde_json::json!({
            "schema_version": PROTOTYPE1_SCHEDULER_SCHEMA_VERSION,
            "campaign_id": "campaign-a",
            "updated_at": "2026-04-28T00:00:00Z",
            "frontier_node_ids": [],
            "completed_node_ids": [],
            "failed_node_ids": [],
            "last_continuation_decision": last_continuation_decision,
            "nodes": []
        })
        .to_string()
    }

    fn branch_registry_with_selections(branch_ids: &[&str]) -> String {
        let source_nodes = branch_ids
            .iter()
            .enumerate()
            .map(|(index, branch_id)| {
                serde_json::json!({
                    "source_state_id": format!("source-{index}"),
                    "instance_id": format!("instance-{index}"),
                    "target_relpath": "crates/ploke-core/tool_text/non_semantic_patch.md",
                    "source_content": "before",
                    "source_content_hash": format!("hash-{index}"),
                    "selected_branch_id": branch_id,
                    "branches": [{
                        "branch_id": branch_id,
                        "candidate_id": format!("candidate-{index}"),
                        "branch_label": format!("branch-label-{index}"),
                        "synthesized_spec_id": format!("spec-{index}"),
                        "proposed_content": "after",
                        "proposed_content_hash": format!("proposed-hash-{index}"),
                        "status": "selected"
                    }]
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "schema_version": PROTOTYPE1_BRANCH_REGISTRY_SCHEMA_VERSION,
            "campaign_id": "campaign-a",
            "updated_at": "2026-04-28T00:00:00Z",
            "source_nodes": source_nodes,
            "active_targets": []
        })
        .to_string()
    }

    fn dashboard_from_cases(cases: &[NodeCase<'_>], selected_branches: &[&str]) -> Dashboard {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        let prototype = tmp.path().join("prototype1");
        fs::create_dir_all(prototype.join("evaluations")).expect("evals");
        for node_case in cases {
            let node = prototype.join(format!("nodes/{}", node_case.node_id));
            fs::create_dir_all(node.join("results")).expect("node dir");
            fs::write(
                node.join("node.json"),
                node_record(
                    &prototype,
                    node_case.node_id,
                    node_case.parent_node_id,
                    node_case.generation,
                    node_case.branch_id,
                ),
            )
            .expect("node record");
            fs::write(
                node.join("results/runtime.json"),
                runner_result(
                    &prototype,
                    node_case.node_id,
                    node_case.generation,
                    node_case.branch_id,
                ),
            )
            .expect("result");
            fs::write(
                prototype.join(format!("evaluations/{}.json", node_case.branch_id)),
                evaluation(
                    node_case.branch_id,
                    node_case.disposition,
                    node_case.compared,
                    node_case.oracle,
                    node_case.tool_calls,
                    node_case.failed,
                ),
            )
            .expect("evaluation");
        }
        fs::write(
            prototype.join("branches.json"),
            branch_registry_with_selections(selected_branches),
        )
        .expect("branch registry");

        build("campaign-a", &manifest).expect("dashboard")
    }

    fn dashboard_from_nodes(
        nodes: &[(&str, Option<&str>, u32, &str)],
        selected_branches: &[&str],
    ) -> Dashboard {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        let prototype = tmp.path().join("prototype1");
        for (node_id, parent_node_id, generation, branch_id) in nodes {
            let node = prototype.join(format!("nodes/{node_id}"));
            fs::create_dir_all(&node).expect("node dir");
            fs::write(
                node.join("node.json"),
                node_record(&prototype, node_id, *parent_node_id, *generation, branch_id),
            )
            .expect("node record");
        }
        fs::write(
            prototype.join("branches.json"),
            branch_registry_with_selections(selected_branches),
        )
        .expect("branch registry");

        build("campaign-a", &manifest).expect("dashboard")
    }
}
