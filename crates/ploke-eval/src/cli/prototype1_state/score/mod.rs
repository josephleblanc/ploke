//! Narrow score projection over typed Prototype 1 child evidence.
//!
//! This module consumes the grouped typed evidence surface from `evidence.rs`.
//! The plain score projection does not load files directly, parse JSON, recover
//! identity from paths, call successor selection, or admit anything into
//! History. It is only a replayable projection over existing
//! `EvaluationEvidence`, `ComparedRunEvidence`, and `OperationalRunMetrics`
//! fields.
//!
//! The score-selection review projection is also read-only: it replays the
//! current successor-selection procedure beside score rows so operators can
//! compare the selector's current inputs with score evidence. That review does
//! not change live selection behavior, perform archive traversal, or admit
//! anything into History.
//!
//! Complete scores require explicit typed evaluation identity from the branch
//! evaluation report: evaluation procedure id, evaluator id/version, and an
//! eval-set identity. They also require typed baseline/treatment run
//! registration identity under each compared run. Older reports without those
//! fields remain incomplete instead of recovering identity from paths,
//! filenames, or compared-instance surrogates.

#![allow(dead_code)]

pub(crate) mod component;
pub(crate) mod operational;
pub(crate) mod profile;
pub(crate) mod protocol;
pub(crate) mod select;
pub(crate) use select::*;

pub(crate) use component::{ScoreComponent, ScoreComponentProvenance, ScoreProfileComponent};
pub(crate) use profile::ScoreProfile;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::evidence::{
    ChildEvidence, ChildEvidenceSet, ComparedRunEvidence, EvaluationEvidence, EvidenceDiagnostic,
    EvidenceSource, RunEvidence, SelectionProjectionError, SelectionProjectionSet,
};
use super::history_preview::{FsEvidenceStore, PreviewError};
use crate::cli::InspectOutputFormat;
use crate::inner::core::RegisteredRunRole;
use crate::spec::PrepareError;
use crate::successor_selection::{self, SelectionInput, SuccessorDecision};

const SCHEMA_VERSION: &str = "prototype1-score-projection.v1";
const PROCEDURE_ID: &str = "prototype1.score.operational_metrics.v1";
const OPERATIONAL_COMPONENT_ID: &str = "operational";
const PROTOCOL_COMPONENT_ID: &str = "protocol_payload";
const PROTOCOL_COMPONENT_PROCEDURE_ID: &str = "prototype1.score.protocol_payload.v1";
const OPERATIONAL_PROTOCOL_PROFILE_ID: &str = "prototype1.score.profile.operational_protocol.v1";
const REVIEW_SCHEMA_VERSION: &str = "prototype1-score-selection-review.v1";

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScoreRequest {
    pub(crate) rows: usize,
    pub(crate) generation: Option<u32>,
    pub(crate) format: InspectOutputFormat,
}

pub(crate) fn run(
    campaign_id: &str,
    manifest_path: &Path,
    request: ScoreRequest,
) -> Result<(), PrepareError> {
    let snapshot =
        build(campaign_id, manifest_path).map_err(|source| PrepareError::DatabaseSetup {
            phase: "build prototype1 score snapshot",
            detail: source.to_string(),
        })?;

    match request.format {
        InspectOutputFormat::Table => snapshot.print(&request),
        InspectOutputFormat::Json => {
            let report = snapshot.report(&request);
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
            );
        }
    }

    Ok(())
}

pub(crate) fn build(
    campaign_id: &str,
    manifest_path: &Path,
) -> Result<ScoreSnapshot, PreviewError> {
    let store = FsEvidenceStore::new(manifest_path);
    let evidence = store.child_evidence()?;
    let scores = ScoreSet::from_evidence(&evidence);
    Ok(ScoreSnapshot {
        schema_version: scores.schema_version.clone(),
        generated_at: Utc::now().to_rfc3339(),
        campaign_id: campaign_id.to_string(),
        manifest_path: manifest_path.to_path_buf(),
        prototype_root: prototype_root(manifest_path),
        scores,
    })
}

pub(crate) fn run_selection_review(
    campaign_id: &str,
    manifest_path: &Path,
    request: ScoreSelectionReviewRequest,
) -> Result<(), PrepareError> {
    let snapshot = build_selection_review(campaign_id, manifest_path).map_err(|source| {
        PrepareError::DatabaseSetup {
            phase: "build prototype1 score-selection review",
            detail: source.to_string(),
        }
    })?;

    match request.format {
        InspectOutputFormat::Table => snapshot.print(&request),
        InspectOutputFormat::Json => {
            let report = snapshot.report(&request);
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
            );
        }
    }

    Ok(())
}

pub(crate) fn build_selection_review(
    campaign_id: &str,
    manifest_path: &Path,
) -> Result<ScoreSelectionSnapshot, PreviewError> {
    let store = FsEvidenceStore::new(manifest_path);
    let evidence = store.child_evidence()?;
    let review = ScoreSelectionReview::from_evidence(&evidence);
    Ok(ScoreSelectionSnapshot {
        schema_version: review.schema_version.clone(),
        generated_at: Utc::now().to_rfc3339(),
        campaign_id: campaign_id.to_string(),
        manifest_path: manifest_path.to_path_buf(),
        prototype_root: prototype_root(manifest_path),
        review,
    })
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ScoreSnapshot {
    schema_version: String,
    generated_at: String,
    campaign_id: String,
    manifest_path: PathBuf,
    prototype_root: PathBuf,
    scores: ScoreSet,
}

impl ScoreSnapshot {
    fn report(&self, request: &ScoreRequest) -> ScoreReport {
        let children = self
            .scores
            .children
            .iter()
            .filter(|child| generation_matches(child, request.generation))
            .take(request.rows)
            .cloned()
            .collect::<Vec<_>>();
        let total_matching_children = self
            .scores
            .children
            .iter()
            .filter(|child| generation_matches(child, request.generation))
            .count();

        ScoreReport {
            schema_version: self.schema_version.clone(),
            generated_at: self.generated_at.clone(),
            campaign_id: self.campaign_id.clone(),
            manifest_path: self.manifest_path.clone(),
            prototype_root: self.prototype_root.clone(),
            procedure_id: self.scores.procedure_id.clone(),
            total_children: self.scores.children.len(),
            total_matching_children,
            row_count: children.len(),
            global_diagnostics: self.scores.diagnostics.len(),
            generation_filter: request.generation,
            score_set: ScoreSet {
                schema_version: self.scores.schema_version.clone(),
                procedure_id: self.scores.procedure_id.clone(),
                profile: self.scores.profile.clone(),
                children,
                diagnostics: self.scores.diagnostics.clone(),
            },
        }
    }

    fn print(&self, request: &ScoreRequest) {
        let report = self.report(request);
        println!("prototype1 score report");
        println!("{}", "-".repeat(40));
        println!("schema_version: {}", report.schema_version);
        println!("generated_at: {}", report.generated_at);
        println!("campaign_id: {}", report.campaign_id);
        println!("manifest: {}", report.manifest_path.display());
        println!("prototype_root: {}", report.prototype_root.display());
        println!("procedure_id: {}", report.procedure_id);
        println!("children: {}", report.total_children);
        println!("matching_children: {}", report.total_matching_children);
        println!("rows: {}", report.row_count);
        println!("global_diagnostics: {}", report.global_diagnostics);
        if let Some(generation) = report.generation_filter {
            println!("generation_filter: {generation}");
        }
        println!("report_note: read-only score state; no selection or History admission");
        println!();

        println!("children");
        println!("{}", "-".repeat(40));
        println!(
            "gen | node | branch | state | score | evaluations | comparable | missing_metrics | diagnostics | eval_set"
        );
        if report.score_set.children.is_empty() {
            println!("(none)");
        } else {
            for child in &report.score_set.children {
                println!(
                    "{} | {} | {} | {} | {} | {} | {} | {} | {} | {}",
                    opt_u32(child.generation),
                    child.node_id,
                    opt_text(child.branch_id.as_deref()),
                    child.state.as_str(),
                    opt_i64(child.score),
                    child.evaluations.len(),
                    child
                        .evaluations
                        .iter()
                        .map(|evaluation| evaluation.comparable_runs)
                        .sum::<usize>(),
                    child
                        .evaluations
                        .iter()
                        .map(|evaluation| evaluation.missing_metric_runs)
                        .sum::<usize>(),
                    child_diagnostic_count(child),
                    eval_set_summary(child),
                );
            }
        }

        let diagnostics = report
            .score_set
            .diagnostics
            .iter()
            .map(|diagnostic| format!("global {}: {}", diagnostic.field, diagnostic.message))
            .chain(report.score_set.children.iter().flat_map(child_diagnostics))
            .take(20)
            .collect::<Vec<_>>();
        if !diagnostics.is_empty() {
            println!();
            println!("diagnostics");
            println!("{}", "-".repeat(40));
            for diagnostic in diagnostics {
                println!("{diagnostic}");
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ScoreReport {
    schema_version: String,
    generated_at: String,
    campaign_id: String,
    manifest_path: PathBuf,
    prototype_root: PathBuf,
    procedure_id: String,
    total_children: usize,
    total_matching_children: usize,
    row_count: usize,
    global_diagnostics: usize,
    generation_filter: Option<u32>,
    score_set: ScoreSet,
}

fn generation_matches(child: &ChildScore, generation: Option<u32>) -> bool {
    generation.is_none_or(|generation| child.generation == Some(generation))
}

fn generation_matches_review(row: &ScoreSelectionRow, generation: Option<u32>) -> bool {
    generation.is_none_or(|generation| row.generation == Some(generation))
}

fn decision_summary(decision: Option<&SuccessorDecision>) -> String {
    decision
        .map(|decision| {
            format!(
                "{:?}:{}",
                decision.outcome,
                opt_text(decision.selected_branch_id.as_deref())
            )
        })
        .unwrap_or_else(|| "-".to_string())
}

fn child_diagnostic_count(child: &ChildScore) -> usize {
    child.diagnostics.len()
        + child
            .evaluations
            .iter()
            .map(|evaluation| {
                evaluation.diagnostics.len()
                    + evaluation
                        .runs
                        .iter()
                        .map(|run| run.diagnostics.len())
                        .sum::<usize>()
            })
            .sum::<usize>()
}

fn child_diagnostics(child: &ChildScore) -> Vec<String> {
    let mut diagnostics = Vec::new();
    for diagnostic in &child.diagnostics {
        diagnostics.push(format!(
            "node={} {}: {}",
            child.node_id, diagnostic.field, diagnostic.message
        ));
    }
    for evaluation in &child.evaluations {
        for diagnostic in &evaluation.diagnostics {
            diagnostics.push(format!(
                "node={} evaluation={} {}: {}",
                child.node_id, evaluation.branch_id, diagnostic.field, diagnostic.message
            ));
        }
        for run in &evaluation.runs {
            for diagnostic in &run.diagnostics {
                diagnostics.push(format!(
                    "node={} evaluation={} run={} {}: {}",
                    child.node_id,
                    evaluation.branch_id,
                    opt_text(run.instance_id.as_deref()),
                    diagnostic.field,
                    diagnostic.message
                ));
            }
        }
    }
    diagnostics
}

fn eval_set_summary(child: &ChildScore) -> String {
    let mut kinds = BTreeSet::new();
    let mut instance_ids = 0_usize;
    let mut missing_instance_ids = 0_usize;
    for evaluation in &child.evaluations {
        kinds.insert(evaluation.identity.eval_set.kind.as_str());
        instance_ids += evaluation.identity.eval_set.instance_ids.len();
        missing_instance_ids += evaluation.identity.eval_set.missing_instance_ids;
    }
    if kinds.is_empty() {
        return "-".to_string();
    }
    format!(
        "{} ids={} missing={}",
        kinds.into_iter().collect::<Vec<_>>().join("+"),
        instance_ids,
        missing_instance_ids
    )
}

fn prototype_root(manifest_path: &Path) -> PathBuf {
    manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
}

fn opt_text(value: Option<&str>) -> &str {
    value.filter(|text| !text.is_empty()).unwrap_or("-")
}

fn opt_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn opt_i64(value: Option<i64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScoreSet {
    pub(crate) schema_version: String,
    pub(crate) procedure_id: String,
    pub(crate) profile: ScoreProfile,
    pub(crate) children: Vec<ChildScore>,
    pub(crate) diagnostics: Vec<ScoreDiagnostic>,
}

impl ScoreSet {
    pub(crate) fn from_evidence(evidence: &ChildEvidenceSet) -> Self {
        Self::from_evidence_with_profile(evidence, &ScoreProfile::operational())
    }

    pub(crate) fn from_evidence_with_profile(
        evidence: &ChildEvidenceSet,
        profile: &ScoreProfile,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            procedure_id: profile.id.clone(),
            profile: profile.clone(),
            children: evidence
                .children
                .iter()
                .map(|child| ChildScore::from_child_with_profile(child, profile))
                .collect(),
            diagnostics: evidence
                .diagnostics
                .iter()
                .map(ScoreDiagnostic::from_evidence)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ScoreCoordinate {
    node_id: String,
    branch_id: String,
    generation: u32,
}

impl ScoreCoordinate {
    fn from_input(input: &SelectionInput) -> Self {
        Self {
            node_id: input.candidate.node_id.clone(),
            branch_id: input.candidate.branch_id.clone(),
            generation: input.candidate.generation,
        }
    }

    fn from_score(score: &ChildScore) -> Option<Self> {
        Some(Self {
            node_id: score.node_id.clone(),
            branch_id: score.branch_id.clone()?,
            generation: score.generation?,
        })
    }
}

fn generation_decisions(inputs: &[SelectionInput]) -> BTreeMap<ScoreCoordinate, SuccessorDecision> {
    let mut by_generation = BTreeMap::<u32, Vec<SelectionInput>>::new();
    for input in inputs {
        by_generation
            .entry(input.candidate.generation)
            .or_default()
            .push(input.clone());
    }

    let mut decisions = BTreeMap::new();
    for inputs in by_generation.into_values() {
        if let Some(decision) = successor_selection::decide_generation(inputs.clone()) {
            if let Some(input) = inputs.iter().find(|input| {
                input.candidate.node_id == decision.candidate_node_id
                    && decision
                        .selected_branch_id
                        .as_ref()
                        .is_none_or(|branch_id| branch_id == &input.candidate.branch_id)
            }) {
                decisions.insert(ScoreCoordinate::from_input(input), decision);
            }
        }
    }
    decisions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChildScore {
    pub(crate) node_id: String,
    pub(crate) parent_node_id: Option<String>,
    pub(crate) generation: Option<u32>,
    pub(crate) branch_id: Option<String>,
    pub(crate) state: ScoreState,
    pub(crate) score: Option<i64>,
    pub(crate) evaluations: Vec<EvaluationScore>,
    pub(crate) diagnostics: Vec<ScoreDiagnostic>,
}

impl ChildScore {
    pub(crate) fn from_child(child: &ChildEvidence) -> Self {
        Self::from_child_with_profile(child, &ScoreProfile::operational())
    }

    fn from_child_with_profile(child: &ChildEvidence, profile: &ScoreProfile) -> Self {
        let mut diagnostics = child
            .diagnostics
            .iter()
            .map(ScoreDiagnostic::from_evidence)
            .collect::<Vec<_>>();
        if child.branch_id.is_none() {
            diagnostics.push(ScoreDiagnostic::missing(
                "branch_id",
                None,
                "child evidence has no branch id for score context",
            ));
        }
        if child.evaluations.is_empty() {
            diagnostics.push(ScoreDiagnostic::missing(
                "evaluations",
                None,
                "child evidence has no evaluation reports to score",
            ));
        }

        let mut branch_counts = BTreeMap::<&str, usize>::new();
        for evaluation in &child.evaluations {
            *branch_counts
                .entry(evaluation.branch_id.as_str())
                .or_default() += 1;
        }
        for (branch_id, count) in branch_counts {
            if count > 1 {
                diagnostics.push(ScoreDiagnostic::invalid(
                    "evaluations",
                    None,
                    format!(
                        "child evidence has {count} evaluation reports for branch '{branch_id}'"
                    ),
                ));
            }
        }

        let evaluations = child
            .evaluations
            .iter()
            .map(|evaluation| {
                EvaluationScore::from_evaluation_with_profile(
                    evaluation,
                    child.branch_id.as_deref(),
                    profile,
                )
            })
            .collect::<Vec<_>>();
        let state = aggregate_state(
            diagnostics.iter().map(|diagnostic| diagnostic.state()),
            evaluations.iter().map(|evaluation| evaluation.state),
        );
        let score = if state == ScoreState::Invalid {
            None
        } else {
            sum_scores(evaluations.iter().filter_map(|evaluation| evaluation.score))
        };

        Self {
            node_id: child.node_id.clone(),
            parent_node_id: child.parent_node_id.clone(),
            generation: child.generation,
            branch_id: child.branch_id.clone(),
            state,
            score,
            evaluations,
            diagnostics,
        }
    }

    /// Returns the decision-grade child score for consumers that require a
    /// comparable value.
    ///
    /// `score` remains available as an operator-facing diagnostic projection:
    /// an incomplete child can still carry useful numeric operational deltas.
    /// Selection and archive traversal must use this gate so those diagnostic
    /// values do not become `alpha_i` inputs before the identity/procedure/run
    /// registration requirements make the child score complete.
    pub(crate) fn comparable_score(&self) -> Option<i64> {
        (self.state == ScoreState::Complete)
            .then_some(self.score)
            .flatten()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvaluationScore {
    pub(crate) branch_id: String,
    pub(crate) disposition: Option<String>,
    pub(crate) identity: EvaluationIdentity,
    pub(crate) state: ScoreState,
    pub(crate) score: Option<i64>,
    pub(crate) comparable_runs: usize,
    pub(crate) missing_metric_runs: usize,
    pub(crate) runs: Vec<RunScore>,
    pub(crate) provenance: EvaluationProvenance,
    pub(crate) diagnostics: Vec<ScoreDiagnostic>,
}

impl EvaluationScore {
    fn from_evaluation(evaluation: &EvaluationEvidence, child_branch_id: Option<&str>) -> Self {
        Self::from_evaluation_with_profile(
            evaluation,
            child_branch_id,
            &ScoreProfile::operational(),
        )
    }

    fn from_evaluation_with_profile(
        evaluation: &EvaluationEvidence,
        child_branch_id: Option<&str>,
        profile: &ScoreProfile,
    ) -> Self {
        let source_ref = Some(evaluation.source.pointer.ref_id().to_string());
        let mut diagnostics = Vec::new();
        if evaluation.overall_disposition.is_none() {
            diagnostics.push(ScoreDiagnostic::missing(
                "overall_disposition",
                source_ref.clone(),
                "evaluation evidence has no overall disposition",
            ));
        }
        if let Some(child_branch_id) = child_branch_id {
            if evaluation.branch_id != child_branch_id {
                diagnostics.push(ScoreDiagnostic::invalid(
                    "branch_id",
                    source_ref.clone(),
                    format!(
                        "evaluation branch '{}' does not match child branch '{}'",
                        evaluation.branch_id, child_branch_id
                    ),
                ));
            }
        }

        let identity = EvaluationIdentity::from_evaluation(evaluation);
        identity.push_diagnostics(source_ref.clone(), &mut diagnostics);

        let runs = evaluation
            .compared
            .iter()
            .map(|compared| RunScore::from_compared_with_profile(compared, profile))
            .collect::<Vec<_>>();
        let comparable_runs = runs
            .iter()
            .filter(|run| run.state == ScoreState::Complete && run.score.is_some())
            .count();
        let scored_runs = runs.iter().filter(|run| run.score.is_some()).count();
        let missing_metric_runs = runs.len().saturating_sub(scored_runs);
        let state = aggregate_state(
            diagnostics.iter().map(|diagnostic| diagnostic.state()),
            runs.iter().map(|run| run.state),
        );
        let score = if state == ScoreState::Invalid {
            None
        } else {
            sum_scores(runs.iter().filter_map(|run| run.score))
        };

        Self {
            branch_id: evaluation.branch_id.clone(),
            disposition: evaluation.overall_disposition.clone(),
            identity,
            state,
            score,
            comparable_runs,
            missing_metric_runs,
            runs,
            provenance: EvaluationProvenance {
                source: evaluation.source.clone(),
                evaluation_artifact_path: evaluation.evaluation_artifact_path.clone(),
            },
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvaluationIdentity {
    pub(crate) procedure_id: IdentityValue,
    pub(crate) evaluator: EvaluatorIdentity,
    pub(crate) eval_set: EvalSetIdentity,
}

impl EvaluationIdentity {
    fn from_evaluation(evaluation: &EvaluationEvidence) -> Self {
        let compared = &evaluation.compared;
        let mut instance_ids = BTreeSet::new();
        let mut missing_instance_ids = 0_usize;
        for run in compared {
            match run.instance_id.as_deref().filter(|id| !id.is_empty()) {
                Some(instance_id) => {
                    instance_ids.insert(instance_id.to_string());
                }
                None => missing_instance_ids += 1,
            }
        }

        let instance_ids = instance_ids.into_iter().collect::<Vec<_>>();
        let eval_set = evaluation
            .eval_set_identity
            .as_ref()
            .map(|identity| EvalSetIdentity {
                value: required_identity_text(identity.id.clone()),
                explicit: identity.explicit,
                kind: identity.kind.clone(),
                authority: required_identity_text(identity.authority.clone()),
                benchmark_family: Some(format!("{:?}", identity.benchmark_family)),
                dataset_source_count: identity.dataset_sources.len(),
                instance_ids: identity.instance_ids.clone(),
                missing_instance_ids: identity.missing_treatment_instance_ids.len(),
                note: identity.note.clone(),
            })
            .unwrap_or_else(|| EvalSetIdentity {
                value: None,
                explicit: false,
                kind: if instance_ids.is_empty() {
                    "missing".to_string()
                } else {
                    "compared_instance_ids".to_string()
                },
                authority: None,
                benchmark_family: None,
                dataset_source_count: 0,
                instance_ids,
                missing_instance_ids,
                note: Some(
                    "compared instance ids are an eval-set surrogate, not an explicit eval-set id"
                        .to_string(),
                ),
            });

        Self {
            procedure_id: IdentityValue::from_optional(
                evaluation.evaluation_procedure_id.clone(),
                "evaluation report does not carry an evaluation procedure id",
            ),
            evaluator: EvaluatorIdentity::from_optional(
                evaluation
                    .evaluator_identity
                    .as_ref()
                    .map(|identity| (identity.id.clone(), identity.version.clone())),
                "evaluation report does not carry evaluator identity",
            ),
            eval_set,
        }
    }

    fn push_diagnostics(&self, source_ref: Option<String>, diagnostics: &mut Vec<ScoreDiagnostic>) {
        if self.procedure_id.value.is_none() {
            diagnostics.push(ScoreDiagnostic::missing(
                "procedure_id",
                source_ref.clone(),
                self.procedure_id.note.clone(),
            ));
        }
        if self.evaluator.id.is_none() || self.evaluator.version.is_none() {
            diagnostics.push(ScoreDiagnostic::missing(
                "evaluator",
                source_ref.clone(),
                self.evaluator.note.clone(),
            ));
        }
        if self.eval_set.explicit {
            if self.eval_set.value.is_none() {
                diagnostics.push(ScoreDiagnostic::missing(
                    "eval_set",
                    source_ref.clone(),
                    "malformed eval_set identity has blank required id",
                ));
            }
            if !has_identity_text(&self.eval_set.kind) {
                diagnostics.push(ScoreDiagnostic::missing(
                    "eval_set",
                    source_ref.clone(),
                    "malformed eval_set identity has blank required kind",
                ));
            }
            if self
                .eval_set
                .authority
                .as_deref()
                .map_or(true, |authority| !has_identity_text(authority))
            {
                diagnostics.push(ScoreDiagnostic::missing(
                    "eval_set",
                    source_ref.clone(),
                    "malformed eval_set identity has blank required authority",
                ));
            }
        } else if self.eval_set.value.is_none() {
            diagnostics.push(ScoreDiagnostic::missing(
                "eval_set",
                source_ref.clone(),
                "evaluation report does not carry an explicit eval-set identity",
            ));
        } else {
            diagnostics.push(ScoreDiagnostic::missing(
                "eval_set",
                source_ref.clone(),
                self.eval_set.note.clone().unwrap_or_else(|| {
                    "evaluation report carries only a surrogate eval-set identity".to_string()
                }),
            ));
        }
        if self.eval_set.kind == "missing" {
            diagnostics.push(ScoreDiagnostic::missing(
                "eval_set",
                source_ref,
                "evaluation report has no compared instance ids for eval-set surrogate",
            ));
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IdentityValue {
    pub(crate) value: Option<String>,
    pub(crate) status: IdentityStatus,
    pub(crate) note: String,
}

impl IdentityValue {
    fn from_optional(value: Option<String>, missing_note: impl Into<String>) -> Self {
        match value.filter(|value| has_identity_text(value)) {
            Some(value) => Self {
                value: Some(value),
                status: IdentityStatus::Present,
                note: "typed evaluation report field".to_string(),
            },
            None => Self::missing(missing_note),
        }
    }

    fn missing(note: impl Into<String>) -> Self {
        Self {
            value: None,
            status: IdentityStatus::Missing,
            note: note.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IdentityStatus {
    Present,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvaluatorIdentity {
    pub(crate) id: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) status: IdentityStatus,
    pub(crate) note: String,
}

impl EvaluatorIdentity {
    fn from_optional(value: Option<(String, String)>, missing_note: impl Into<String>) -> Self {
        match value {
            Some((id, version)) if has_identity_text(&id) && has_identity_text(&version) => Self {
                id: Some(id),
                version: Some(version),
                status: IdentityStatus::Present,
                note: "typed evaluation report field".to_string(),
            },
            _ => Self {
                id: None,
                version: None,
                status: IdentityStatus::Missing,
                note: missing_note.into(),
            },
        }
    }
}

fn required_identity_text(value: String) -> Option<String> {
    has_identity_text(&value).then_some(value)
}

fn has_identity_text(value: &str) -> bool {
    !value.trim().is_empty()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvalSetIdentity {
    pub(crate) value: Option<String>,
    pub(crate) explicit: bool,
    pub(crate) kind: String,
    pub(crate) authority: Option<String>,
    pub(crate) benchmark_family: Option<String>,
    pub(crate) dataset_source_count: usize,
    pub(crate) instance_ids: Vec<String>,
    pub(crate) missing_instance_ids: usize,
    pub(crate) note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RunScore {
    pub(crate) instance_id: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) state: ScoreState,
    pub(crate) score: Option<i64>,
    pub(crate) metrics: Vec<MetricScore>,
    pub(crate) components: Vec<ScoreComponent>,
    pub(crate) provenance: RunProvenance,
    pub(crate) diagnostics: Vec<ScoreDiagnostic>,
}

impl RunScore {
    fn from_compared(compared: &ComparedRunEvidence) -> Self {
        Self::from_compared_with_profile(compared, &ScoreProfile::operational())
    }

    fn from_compared_with_profile(compared: &ComparedRunEvidence, profile: &ScoreProfile) -> Self {
        let mut diagnostics = Vec::new();
        if compared.instance_id.is_none() {
            diagnostics.push(ScoreDiagnostic::missing(
                "instance_id",
                None,
                "compared run has no instance id",
            ));
        }
        diagnostics.extend(
            compared
                .diagnostics
                .iter()
                .map(ScoreDiagnostic::from_compared),
        );

        validate_run_identity(
            "baseline",
            compared.baseline_run.as_ref(),
            RegisteredRunRole::Control,
            compared.instance_id.as_deref(),
            compared.baseline_record_path.as_deref(),
            &mut diagnostics,
        );
        validate_run_identity(
            "treatment",
            compared.treatment_run.as_ref(),
            RegisteredRunRole::Treatment,
            compared.instance_id.as_deref(),
            compared.treatment_record_path.as_deref(),
            &mut diagnostics,
        );

        let mut components = Vec::new();
        if profile.includes_operational() {
            let operational = operational::operational_component(compared);
            diagnostics.extend(operational.diagnostics.clone());
            components.push(operational);
        }
        if profile.includes_protocol() {
            components.push(protocol::protocol_component(compared));
        }

        let state = aggregate_state(
            diagnostics.iter().map(|diagnostic| diagnostic.state()),
            components.iter().map(|component| component.state),
        );
        let score = if state == ScoreState::Invalid {
            None
        } else {
            sum_scores(components.iter().filter_map(|component| component.score))
        };
        let metrics = components
            .iter()
            .find(|component| component.component_id == OPERATIONAL_COMPONENT_ID)
            .map(|component| component.metrics.clone())
            .unwrap_or_default();

        Self {
            instance_id: compared.instance_id.clone(),
            status: compared.status.clone(),
            state,
            score,
            metrics,
            components,
            provenance: RunProvenance {
                baseline_registration_path: compared.baseline_registration_path.clone(),
                treatment_registration_path: compared.treatment_registration_path.clone(),
                baseline_record_path: compared.baseline_record_path.clone(),
                treatment_record_path: compared.treatment_record_path.clone(),
                baseline_run: compared.baseline_run.clone(),
                treatment_run: compared.treatment_run.clone(),
            },
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MetricScore {
    pub(crate) field: String,
    pub(crate) baseline: String,
    pub(crate) treatment: String,
    pub(crate) direction: ScoreDirection,
    pub(crate) points: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScoreDirection {
    Improved,
    Regressed,
    Unchanged,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScoreState {
    Complete,
    Incomplete,
    Invalid,
}

impl ScoreState {
    fn as_str(self) -> &'static str {
        match self {
            ScoreState::Complete => "complete",
            ScoreState::Incomplete => "incomplete",
            ScoreState::Invalid => "invalid",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvaluationProvenance {
    pub(crate) source: EvidenceSource,
    pub(crate) evaluation_artifact_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RunProvenance {
    pub(crate) baseline_registration_path: Option<PathBuf>,
    pub(crate) treatment_registration_path: Option<PathBuf>,
    pub(crate) baseline_record_path: Option<PathBuf>,
    pub(crate) treatment_record_path: Option<PathBuf>,
    pub(crate) baseline_run: Option<RunEvidence>,
    pub(crate) treatment_run: Option<RunEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ScoreDiagnostic {
    pub(crate) severity: String,
    pub(crate) field: String,
    pub(crate) source_ref: Option<String>,
    pub(crate) message: String,
}

impl ScoreDiagnostic {
    fn missing(
        field: impl Into<String>,
        source_ref: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: "missing".to_string(),
            field: field.into(),
            source_ref,
            message: message.into(),
        }
    }

    fn warning(
        field: impl Into<String>,
        source_ref: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: "warning".to_string(),
            field: field.into(),
            source_ref,
            message: message.into(),
        }
    }

    fn invalid(
        field: impl Into<String>,
        source_ref: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: "invalid".to_string(),
            field: field.into(),
            source_ref,
            message: message.into(),
        }
    }

    fn from_evidence(diagnostic: &EvidenceDiagnostic) -> Self {
        Self {
            severity: diagnostic.severity.clone(),
            field: "child_evidence".to_string(),
            source_ref: diagnostic.source_ref.clone(),
            message: diagnostic.message.clone(),
        }
    }

    fn from_compared(diagnostic: &super::evidence::ComparedRunDiagnostic) -> Self {
        Self {
            severity: diagnostic.severity.clone(),
            field: diagnostic.field.clone(),
            source_ref: None,
            message: diagnostic.message.clone(),
        }
    }

    fn state(&self) -> ScoreState {
        match self.severity.as_str() {
            "invalid" | "conflict" | "ambiguous" => ScoreState::Invalid,
            "missing" => ScoreState::Incomplete,
            _ => ScoreState::Complete,
        }
    }
}

fn validate_run_identity(
    arm: &'static str,
    run: Option<&RunEvidence>,
    expected_role: RegisteredRunRole,
    instance_id: Option<&str>,
    record_path: Option<&Path>,
    diagnostics: &mut Vec<ScoreDiagnostic>,
) {
    let Some(run) = run else {
        diagnostics.push(ScoreDiagnostic::missing(
            format!("{arm}_run_registration"),
            None,
            format!("compared {arm} arm does not carry typed RunRegistration evidence"),
        ));
        return;
    };

    if run.run_role != expected_role {
        diagnostics.push(ScoreDiagnostic::invalid(
            format!("{arm}_run_role"),
            None,
            format!(
                "typed {arm} run '{}' has role {:?}, expected {:?}",
                run.run_id, run.run_role, expected_role
            ),
        ));
    }

    if let Some(instance_id) = instance_id {
        if run.task_id != instance_id {
            diagnostics.push(ScoreDiagnostic::invalid(
                format!("{arm}_run_task_id"),
                None,
                format!(
                    "typed {arm} run '{}' task '{}' does not match compared instance '{}'",
                    run.run_id, run.task_id, instance_id
                ),
            ));
        }
    }

    if let Some(record_path) = record_path {
        if !same_path(&run.artifacts.record_path, record_path) {
            diagnostics.push(ScoreDiagnostic::invalid(
                format!("{arm}_run_record_path"),
                None,
                format!(
                    "typed {arm} run '{}' record '{}' does not match compared record '{}'",
                    run.run_id,
                    run.artifacts.record_path.display(),
                    record_path.display()
                ),
            ));
        }
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    let normalize =
        |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    normalize(left) == normalize(right)
}

fn sum_scores(scores: impl Iterator<Item = i64>) -> Option<i64> {
    let mut count = 0_usize;
    let mut total = 0_i64;
    for score in scores {
        count += 1;
        total += score;
    }
    (count > 0).then_some(total)
}

fn aggregate_state(
    diagnostics: impl Iterator<Item = ScoreState>,
    children: impl Iterator<Item = ScoreState>,
) -> ScoreState {
    let mut state = ScoreState::Complete;
    for child in diagnostics.chain(children) {
        match child {
            ScoreState::Invalid => return ScoreState::Invalid,
            ScoreState::Incomplete => state = ScoreState::Incomplete,
            ScoreState::Complete => {}
        }
    }
    state
}

#[cfg(test)]
mod tests;
