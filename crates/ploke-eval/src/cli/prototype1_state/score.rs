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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::evidence::{
    ChildEvidence, ChildEvidenceSet, ComparedRunEvidence, EvaluationEvidence, EvidenceDiagnostic,
    EvidenceSource, RunEvidence, SelectionProjectionError, SelectionProjectionSet,
};
use super::history_preview::{FsEvidenceStore, PreviewError};
use crate::OperationalRunMetrics;
use crate::cli::InspectOutputFormat;
use crate::inner::core::RegisteredRunRole;
use crate::spec::PrepareError;
use crate::successor_selection::{self, SelectionInput, SuccessorDecision};

const SCHEMA_VERSION: &str = "prototype1-score-projection.v1";
const PROCEDURE_ID: &str = "prototype1.score.operational_metrics.v1";
const REVIEW_SCHEMA_VERSION: &str = "prototype1-score-selection-review.v1";

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScoreRequest {
    pub(crate) rows: usize,
    pub(crate) generation: Option<u32>,
    pub(crate) format: InspectOutputFormat,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScoreSelectionReviewRequest {
    pub(crate) rows: usize,
    pub(crate) generation: Option<u32>,
    pub(crate) format: InspectOutputFormat,
}

pub(crate) fn run(
    campaign_id: &str,
    manifest_path: &Path,
    request: ScoreRequest,
) -> Result<(), PrepareError> {
    let projection =
        build(campaign_id, manifest_path).map_err(|source| PrepareError::DatabaseSetup {
            phase: "build prototype1 score projection",
            detail: source.to_string(),
        })?;

    match request.format {
        InspectOutputFormat::Table => projection.print(&request),
        InspectOutputFormat::Json => {
            let slice = projection.slice(&request);
            println!(
                "{}",
                serde_json::to_string_pretty(&slice).map_err(PrepareError::Serialize)?
            );
        }
    }

    Ok(())
}

pub(crate) fn build(
    campaign_id: &str,
    manifest_path: &Path,
) -> Result<ScoreProjection, PreviewError> {
    let store = FsEvidenceStore::new(manifest_path);
    let evidence = store.child_evidence()?;
    let scores = ScoreSet::from_evidence(&evidence);
    Ok(ScoreProjection {
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
    let projection = build_selection_review(campaign_id, manifest_path).map_err(|source| {
        PrepareError::DatabaseSetup {
            phase: "build prototype1 score-selection review",
            detail: source.to_string(),
        }
    })?;

    match request.format {
        InspectOutputFormat::Table => projection.print(&request),
        InspectOutputFormat::Json => {
            let slice = projection.slice(&request);
            println!(
                "{}",
                serde_json::to_string_pretty(&slice).map_err(PrepareError::Serialize)?
            );
        }
    }

    Ok(())
}

pub(crate) fn build_selection_review(
    campaign_id: &str,
    manifest_path: &Path,
) -> Result<ScoreSelectionProjection, PreviewError> {
    let store = FsEvidenceStore::new(manifest_path);
    let evidence = store.child_evidence()?;
    let review = ScoreSelectionReview::from_evidence(&evidence);
    Ok(ScoreSelectionProjection {
        schema_version: review.schema_version.clone(),
        generated_at: Utc::now().to_rfc3339(),
        campaign_id: campaign_id.to_string(),
        manifest_path: manifest_path.to_path_buf(),
        prototype_root: prototype_root(manifest_path),
        review,
    })
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ScoreProjection {
    schema_version: String,
    generated_at: String,
    campaign_id: String,
    manifest_path: PathBuf,
    prototype_root: PathBuf,
    scores: ScoreSet,
}

impl ScoreProjection {
    fn slice(&self, request: &ScoreRequest) -> ScoreSlice {
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

        ScoreSlice {
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
                children,
                diagnostics: self.scores.diagnostics.clone(),
            },
        }
    }

    fn print(&self, request: &ScoreRequest) {
        let slice = self.slice(request);
        println!("prototype1 score projection");
        println!("{}", "-".repeat(40));
        println!("schema_version: {}", slice.schema_version);
        println!("generated_at: {}", slice.generated_at);
        println!("campaign_id: {}", slice.campaign_id);
        println!("manifest: {}", slice.manifest_path.display());
        println!("prototype_root: {}", slice.prototype_root.display());
        println!("procedure_id: {}", slice.procedure_id);
        println!("children: {}", slice.total_children);
        println!("matching_children: {}", slice.total_matching_children);
        println!("rows: {}", slice.row_count);
        println!("global_diagnostics: {}", slice.global_diagnostics);
        if let Some(generation) = slice.generation_filter {
            println!("generation_filter: {generation}");
        }
        println!("projection_note: read-only score state; no selection or History admission");
        println!();

        println!("children");
        println!("{}", "-".repeat(40));
        println!(
            "gen | node | branch | state | score | evaluations | comparable | missing_metrics | diagnostics | eval_set"
        );
        if slice.score_set.children.is_empty() {
            println!("(none)");
        } else {
            for child in &slice.score_set.children {
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

        let diagnostics = slice
            .score_set
            .diagnostics
            .iter()
            .map(|diagnostic| format!("global {}: {}", diagnostic.field, diagnostic.message))
            .chain(slice.score_set.children.iter().flat_map(child_diagnostics))
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
struct ScoreSlice {
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

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ScoreSelectionProjection {
    schema_version: String,
    generated_at: String,
    campaign_id: String,
    manifest_path: PathBuf,
    prototype_root: PathBuf,
    review: ScoreSelectionReview,
}

impl ScoreSelectionProjection {
    fn slice(&self, request: &ScoreSelectionReviewRequest) -> ScoreSelectionSlice {
        let rows = self
            .review
            .rows
            .iter()
            .filter(|row| generation_matches_review(row, request.generation))
            .take(request.rows)
            .cloned()
            .collect::<Vec<_>>();
        let total_matching_rows = self
            .review
            .rows
            .iter()
            .filter(|row| generation_matches_review(row, request.generation))
            .count();

        ScoreSelectionSlice {
            schema_version: self.schema_version.clone(),
            generated_at: self.generated_at.clone(),
            campaign_id: self.campaign_id.clone(),
            manifest_path: self.manifest_path.clone(),
            prototype_root: self.prototype_root.clone(),
            score_schema_version: self.review.score_schema_version.clone(),
            score_procedure_id: self.review.score_procedure_id.clone(),
            selection_procedure_id: self.review.selection_procedure_id.clone(),
            total_rows: self.review.rows.len(),
            total_matching_rows,
            row_count: rows.len(),
            diagnostics: self.review.diagnostics.clone(),
            generation_filter: request.generation,
            rows,
        }
    }

    fn print(&self, request: &ScoreSelectionReviewRequest) {
        let slice = self.slice(request);
        println!("prototype1 score-selection review");
        println!("{}", "-".repeat(40));
        println!("schema_version: {}", slice.schema_version);
        println!("generated_at: {}", slice.generated_at);
        println!("campaign_id: {}", slice.campaign_id);
        println!("manifest: {}", slice.manifest_path.display());
        println!("prototype_root: {}", slice.prototype_root.display());
        println!("score_procedure_id: {}", slice.score_procedure_id);
        println!("selection_procedure_id: {}", slice.selection_procedure_id);
        println!("rows: {}", slice.row_count);
        println!("total_rows: {}", slice.total_rows);
        println!("matching_rows: {}", slice.total_matching_rows);
        if let Some(generation) = slice.generation_filter {
            println!("generation_filter: {generation}");
        }
        println!(
            "projection_note: read-only review; no selection change, archive traversal, or History admission"
        );
        println!();

        println!("rows");
        println!("{}", "-".repeat(40));
        println!(
            "gen | node | branch | selection | individual | generation_decision | score_state | diagnostic_score | local_alpha | diagnostics"
        );
        if slice.rows.is_empty() {
            println!("(none)");
        } else {
            for row in &slice.rows {
                println!(
                    "{} | {} | {} | {} | {} | {} | {} | {} | {} | {}",
                    opt_u32(row.generation),
                    row.node_id,
                    opt_text(row.branch_id.as_deref()),
                    row.selection_status.as_str(),
                    decision_summary(row.individual_decision.as_ref()),
                    decision_summary(row.generation_decision.as_ref()),
                    row.child_score_state.map(ScoreState::as_str).unwrap_or("-"),
                    opt_i64(row.diagnostic_score),
                    opt_i64(row.local_alpha_candidate),
                    row.diagnostics.len(),
                );
            }
        }

        let diagnostics = slice
            .diagnostics
            .iter()
            .map(|diagnostic| {
                format!(
                    "global {} {}: {}",
                    diagnostic.severity, diagnostic.field, diagnostic.message
                )
            })
            .chain(slice.rows.iter().flat_map(|row| {
                row.diagnostics.iter().map(|diagnostic| {
                    format!(
                        "node={} branch={} {} {}: {}",
                        row.node_id,
                        opt_text(row.branch_id.as_deref()),
                        diagnostic.severity,
                        diagnostic.field,
                        diagnostic.message
                    )
                })
            }))
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
struct ScoreSelectionSlice {
    schema_version: String,
    generated_at: String,
    campaign_id: String,
    manifest_path: PathBuf,
    prototype_root: PathBuf,
    score_schema_version: String,
    score_procedure_id: String,
    selection_procedure_id: String,
    total_rows: usize,
    total_matching_rows: usize,
    row_count: usize,
    diagnostics: Vec<ScoreSelectionDiagnostic>,
    generation_filter: Option<u32>,
    rows: Vec<ScoreSelectionRow>,
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
    pub(crate) children: Vec<ChildScore>,
    pub(crate) diagnostics: Vec<ScoreDiagnostic>,
}

impl ScoreSet {
    pub(crate) fn from_evidence(evidence: &ChildEvidenceSet) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            procedure_id: PROCEDURE_ID.to_string(),
            children: evidence
                .children
                .iter()
                .map(ChildScore::from_child)
                .collect(),
            diagnostics: evidence
                .diagnostics
                .iter()
                .map(ScoreDiagnostic::from_evidence)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScoreSelectionReview {
    pub(crate) schema_version: String,
    pub(crate) score_schema_version: String,
    pub(crate) score_procedure_id: String,
    pub(crate) selection_procedure_id: String,
    pub(crate) rows: Vec<ScoreSelectionRow>,
    pub(crate) diagnostics: Vec<ScoreSelectionDiagnostic>,
}

impl ScoreSelectionReview {
    pub(crate) fn from_evidence(evidence: &ChildEvidenceSet) -> Self {
        let scores = ScoreSet::from_evidence(evidence);
        let selection = evidence.selection_inputs();
        Self::from_parts(scores, selection)
    }

    fn from_parts(scores: ScoreSet, selection: SelectionProjectionSet) -> Self {
        let generation_decisions = generation_decisions(&selection.inputs);
        let mut score_rows = BTreeMap::<ScoreCoordinate, Vec<ChildScore>>::new();
        let mut unjoined_scores = Vec::new();
        for score in scores.children {
            match ScoreCoordinate::from_score(&score) {
                Some(coordinate) => score_rows.entry(coordinate).or_default().push(score),
                None => unjoined_scores.push(score),
            }
        }

        let mut rows = Vec::new();
        let mut seen_selection = BTreeSet::<ScoreCoordinate>::new();
        for input in selection.inputs {
            let coordinate = ScoreCoordinate::from_input(&input);
            let matching_scores = score_rows
                .get(&coordinate)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            seen_selection.insert(coordinate.clone());
            rows.push(ScoreSelectionRow::from_selection(
                input,
                matching_scores,
                generation_decisions.get(&coordinate).cloned(),
            ));
        }

        for (coordinate, matching_scores) in score_rows {
            if !seen_selection.contains(&coordinate) {
                rows.push(ScoreSelectionRow::from_score_only(
                    coordinate,
                    matching_scores.as_slice(),
                ));
            }
        }

        for score in unjoined_scores {
            rows.push(ScoreSelectionRow::from_unjoined_score(score));
        }

        for failure in selection.failures {
            rows.push(ScoreSelectionRow::from_selection_failure(failure));
        }

        rows.sort_by(|left, right| {
            (
                left.generation,
                left.node_id.as_str(),
                left.branch_id.as_deref(),
            )
                .cmp(&(
                    right.generation,
                    right.node_id.as_str(),
                    right.branch_id.as_deref(),
                ))
        });

        Self {
            schema_version: REVIEW_SCHEMA_VERSION.to_string(),
            score_schema_version: scores.schema_version,
            score_procedure_id: scores.procedure_id,
            selection_procedure_id: successor_selection::PROCEDURE_ID.to_string(),
            rows,
            diagnostics: scores
                .diagnostics
                .into_iter()
                .map(ScoreSelectionDiagnostic::from_score)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScoreSelectionRow {
    pub(crate) node_id: String,
    pub(crate) branch_id: Option<String>,
    pub(crate) generation: Option<u32>,
    pub(crate) selection_status: ScoreSelectionStatus,
    pub(crate) individual_decision: Option<SuccessorDecision>,
    pub(crate) generation_decision: Option<SuccessorDecision>,
    pub(crate) child_score_state: Option<ScoreState>,
    pub(crate) diagnostic_score: Option<i64>,
    pub(crate) local_alpha_candidate: Option<i64>,
    pub(crate) diagnostics: Vec<ScoreSelectionDiagnostic>,
}

impl ScoreSelectionRow {
    fn from_selection(
        input: SelectionInput,
        matching_scores: &[ChildScore],
        generation_decision: Option<SuccessorDecision>,
    ) -> Self {
        let coordinate = ScoreCoordinate::from_input(&input);
        let individual_decision = Some(successor_selection::decide(input));
        let mut row = Self::base(
            coordinate.node_id,
            Some(coordinate.branch_id),
            Some(coordinate.generation),
            ScoreSelectionStatus::Projected,
        );
        row.individual_decision = individual_decision;
        row.generation_decision = generation_decision;
        row.attach_scores(matching_scores, false);
        if matching_scores.is_empty() {
            row.diagnostics.push(ScoreSelectionDiagnostic::missing(
                "score_row",
                "selection input has no joined child score row",
            ));
        }
        row
    }

    fn from_score_only(coordinate: ScoreCoordinate, matching_scores: &[ChildScore]) -> Self {
        let mut row = Self::base(
            coordinate.node_id,
            Some(coordinate.branch_id),
            Some(coordinate.generation),
            ScoreSelectionStatus::MissingSelection,
        );
        row.attach_scores(matching_scores, true);
        row.diagnostics.push(ScoreSelectionDiagnostic::missing(
            "selection_input",
            "score row has no joined selection input",
        ));
        row
    }

    fn from_unjoined_score(score: ChildScore) -> Self {
        let mut row = Self::base(
            score.node_id.clone(),
            score.branch_id.clone(),
            score.generation,
            ScoreSelectionStatus::MissingSelection,
        );
        row.attach_score(&score, true);
        row.diagnostics.push(ScoreSelectionDiagnostic::missing(
            "join_coordinate",
            "score row lacks branch_id or generation and cannot be joined to selection input",
        ));
        row
    }

    fn from_selection_failure(failure: SelectionProjectionError) -> Self {
        let mut row = Self::base(
            failure.node_id.unwrap_or_else(|| "-".to_string()),
            None,
            None,
            ScoreSelectionStatus::FailedProjection,
        );
        row.diagnostics
            .extend(
                failure
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| ScoreSelectionDiagnostic {
                        severity: "selection_failure".to_string(),
                        field: format!("{:?}", diagnostic.field),
                        message: diagnostic.message,
                    }),
            );
        row
    }

    fn base(
        node_id: String,
        branch_id: Option<String>,
        generation: Option<u32>,
        selection_status: ScoreSelectionStatus,
    ) -> Self {
        Self {
            node_id,
            branch_id,
            generation,
            selection_status,
            individual_decision: None,
            generation_decision: None,
            child_score_state: None,
            diagnostic_score: None,
            local_alpha_candidate: None,
            diagnostics: Vec::new(),
        }
    }

    fn attach_scores(&mut self, scores: &[ChildScore], suppress_alpha: bool) {
        match scores {
            [] => {}
            [score] => self.attach_score(score, suppress_alpha),
            [first, ..] => {
                self.attach_score(first, true);
                self.diagnostics.push(ScoreSelectionDiagnostic::invalid(
                    "score_row",
                    format!(
                        "candidate coordinate has {} score rows; local alpha suppressed",
                        scores.len()
                    ),
                ));
            }
        }
    }

    fn attach_score(&mut self, score: &ChildScore, suppress_alpha: bool) {
        self.child_score_state = Some(score.state);
        self.diagnostic_score = score.score;
        self.local_alpha_candidate = if suppress_alpha {
            None
        } else {
            score.comparable_score()
        };
        match score.state {
            ScoreState::Complete => {}
            ScoreState::Incomplete => self.diagnostics.push(ScoreSelectionDiagnostic::missing(
                "score_identity",
                "child score is incomplete; evaluator/procedure/eval-set identity or typed baseline/treatment run registration identity is not decision-grade",
            )),
            ScoreState::Invalid => self.diagnostics.push(ScoreSelectionDiagnostic::invalid(
                "child_score",
                "child score is invalid and cannot provide local alpha",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScoreSelectionStatus {
    Projected,
    FailedProjection,
    MissingSelection,
}

impl ScoreSelectionStatus {
    fn as_str(self) -> &'static str {
        match self {
            ScoreSelectionStatus::Projected => "projected",
            ScoreSelectionStatus::FailedProjection => "failed_projection",
            ScoreSelectionStatus::MissingSelection => "missing_selection",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ScoreSelectionDiagnostic {
    pub(crate) severity: String,
    pub(crate) field: String,
    pub(crate) message: String,
}

impl ScoreSelectionDiagnostic {
    fn missing(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "missing".to_string(),
            field: field.into(),
            message: message.into(),
        }
    }

    fn invalid(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "invalid".to_string(),
            field: field.into(),
            message: message.into(),
        }
    }

    fn from_score(diagnostic: ScoreDiagnostic) -> Self {
        Self {
            severity: diagnostic.severity,
            field: diagnostic.field,
            message: diagnostic.message,
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
                EvaluationScore::from_evaluation(evaluation, child.branch_id.as_deref())
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
            .map(RunScore::from_compared)
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
    pub(crate) provenance: RunProvenance,
    pub(crate) diagnostics: Vec<ScoreDiagnostic>,
}

impl RunScore {
    fn from_compared(compared: &ComparedRunEvidence) -> Self {
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

        let metrics = match (
            compared.baseline_metrics.as_ref(),
            compared.treatment_metrics.as_ref(),
        ) {
            (Some(baseline), Some(treatment)) => compare_operational_metrics(baseline, treatment),
            (None, Some(_)) => {
                diagnostics.push(ScoreDiagnostic::missing(
                    "baseline_metrics",
                    None,
                    "compared run has treatment metrics but no baseline metrics",
                ));
                Vec::new()
            }
            (Some(_), None) => {
                diagnostics.push(ScoreDiagnostic::missing(
                    "treatment_metrics",
                    None,
                    "compared run has baseline metrics but no treatment metrics",
                ));
                Vec::new()
            }
            (None, None) => {
                diagnostics.push(ScoreDiagnostic::missing(
                    "operational_metrics",
                    None,
                    "compared run has no baseline or treatment metrics",
                ));
                Vec::new()
            }
        };
        let score = sum_scores(metrics.iter().map(|metric| metric.points));
        let state = aggregate_state(
            diagnostics.iter().map(|diagnostic| diagnostic.state()),
            std::iter::empty(),
        );

        Self {
            instance_id: compared.instance_id.clone(),
            status: compared.status.clone(),
            state,
            score,
            metrics,
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

fn compare_operational_metrics(
    baseline: &OperationalRunMetrics,
    treatment: &OperationalRunMetrics,
) -> Vec<MetricScore> {
    let mut scores = Vec::new();
    lower_is_better(
        &mut scores,
        "tool_calls_failed",
        baseline.tool_calls_failed,
        treatment.tool_calls_failed,
    );
    lower_is_better(
        &mut scores,
        "partial_patch_failures",
        baseline.partial_patch_failures,
        treatment.partial_patch_failures,
    );
    lower_is_better(
        &mut scores,
        "same_file_patch_retry_count",
        baseline.same_file_patch_retry_count,
        treatment.same_file_patch_retry_count,
    );
    lower_is_better(
        &mut scores,
        "same_file_patch_max_streak",
        baseline.same_file_patch_max_streak,
        treatment.same_file_patch_max_streak,
    );
    prefer_false(&mut scores, "aborted", baseline.aborted, treatment.aborted);
    prefer_false(
        &mut scores,
        "aborted_repair_loop",
        baseline.aborted_repair_loop,
        treatment.aborted_repair_loop,
    );
    prefer_true(
        &mut scores,
        "nonempty_valid_patch",
        baseline.nonempty_valid_patch,
        treatment.nonempty_valid_patch,
    );
    prefer_true(
        &mut scores,
        "convergence",
        baseline.convergence,
        treatment.convergence,
    );
    prefer_true(
        &mut scores,
        "oracle_eligible",
        baseline.oracle_eligible,
        treatment.oracle_eligible,
    );
    scores
}

fn lower_is_better(scores: &mut Vec<MetricScore>, field: &str, baseline: usize, treatment: usize) {
    let direction = if treatment < baseline {
        ScoreDirection::Improved
    } else if treatment > baseline {
        ScoreDirection::Regressed
    } else {
        ScoreDirection::Unchanged
    };
    scores.push(metric_score(field, baseline, treatment, direction));
}

fn prefer_false(scores: &mut Vec<MetricScore>, field: &str, baseline: bool, treatment: bool) {
    let direction = match (baseline, treatment) {
        (true, false) => ScoreDirection::Improved,
        (false, true) => ScoreDirection::Regressed,
        _ => ScoreDirection::Unchanged,
    };
    scores.push(metric_score(field, baseline, treatment, direction));
}

fn prefer_true(scores: &mut Vec<MetricScore>, field: &str, baseline: bool, treatment: bool) {
    let direction = match (baseline, treatment) {
        (false, true) => ScoreDirection::Improved,
        (true, false) => ScoreDirection::Regressed,
        _ => ScoreDirection::Unchanged,
    };
    scores.push(metric_score(field, baseline, treatment, direction));
}

fn metric_score(
    field: &str,
    baseline: impl ToString,
    treatment: impl ToString,
    direction: ScoreDirection,
) -> MetricScore {
    MetricScore {
        field: field.to_string(),
        baseline: baseline.to_string(),
        treatment: treatment.to_string(),
        direction,
        points: match direction {
            ScoreDirection::Improved => 1,
            ScoreDirection::Regressed => -1,
            ScoreDirection::Unchanged => 0,
        },
    }
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
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{
        ChildScore, ScoreDirection, ScoreSelectionReview, ScoreSelectionStatus, ScoreSet,
        ScoreState,
    };
    use crate::BranchDisposition;
    use crate::cli::prototype1_state::history_preview::FsEvidenceStore;
    use crate::inner::core::{RegisteredRunRole, RunIntent, RunStorageRoots};
    use crate::inner::registry::RunRegistration;
    use crate::record::SubmissionArtifactState;
    use crate::spec::EvalBudget;
    use crate::successor_selection::{CandidateRef, RunComparison, SelectionInput};
    use crate::{OperationalRunMetrics, PatchApplyState};

    #[test]
    fn scores_typed_evaluation_metrics_with_provenance_and_identity() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        let parent_metrics = metrics(false, false, 3);
        let child_metrics = metrics(true, true, 1);
        write_node(tmp.path(), "node-a", "branch-a");
        write_evaluation(
            tmp.path(),
            "branch-a",
            "keep",
            parent_metrics,
            child_metrics,
        );

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("child evidence");
        let scores = ScoreSet::from_evidence(&evidence);

        assert_eq!(scores.schema_version, "prototype1-score-projection.v1");
        assert_eq!(scores.children.len(), 1);
        let child = &scores.children[0];
        assert_eq!(child.node_id, "node-a");
        assert_eq!(child.state, ScoreState::Complete);
        assert_eq!(child.score, Some(4));
        assert_eq!(child.comparable_score(), Some(4));

        let evaluation = &child.evaluations[0];
        assert_eq!(evaluation.branch_id, "branch-a");
        assert_eq!(evaluation.state, ScoreState::Complete);
        assert_eq!(evaluation.disposition.as_deref(), Some("keep"));
        assert_eq!(evaluation.comparable_runs, 1);
        assert_eq!(evaluation.missing_metric_runs, 0);
        assert_eq!(evaluation.score, Some(4));
        assert_eq!(
            evaluation.identity.procedure_id.value.as_deref(),
            Some("prototype1.branch_evaluation.operational_metrics.v1")
        );
        assert_eq!(
            evaluation.identity.evaluator.id.as_deref(),
            Some("prototype1.branch_evaluation.mechanized")
        );
        assert_eq!(evaluation.identity.evaluator.version.as_deref(), Some("v1"));
        assert_eq!(evaluation.identity.eval_set.kind, "closure_instance_slice");
        assert_eq!(evaluation.identity.eval_set.explicit, true);
        assert_eq!(
            evaluation.identity.eval_set.instance_ids,
            vec!["instance-a"]
        );
        assert_eq!(
            evaluation
                .provenance
                .source
                .pointer
                .ref_id()
                .ends_with("prototype1/evaluations/branch-a.json"),
            true
        );
        assert!(!diagnostic_field(&evaluation.diagnostics, "procedure_id"));
        assert!(!diagnostic_field(&evaluation.diagnostics, "evaluator"));
        assert!(!diagnostic_field(&evaluation.diagnostics, "eval_set"));

        let run = &evaluation.runs[0];
        assert_eq!(run.state, ScoreState::Complete);
        assert_eq!(
            run.provenance.baseline_record_path.as_deref(),
            run.provenance
                .baseline_run
                .as_ref()
                .map(|run| run.artifacts.record_path.as_path())
        );
        assert_eq!(
            run.provenance.treatment_record_path.as_deref(),
            run.provenance
                .treatment_run
                .as_ref()
                .map(|run| run.artifacts.record_path.as_path())
        );
        assert_eq!(
            run.provenance
                .baseline_run
                .as_ref()
                .map(|run| run.run_id.as_str()),
            Some("run-baseline")
        );
        assert_eq!(
            run.provenance
                .treatment_run
                .as_ref()
                .map(|run| run.run_id.as_str()),
            Some("run-treatment")
        );
        assert!(run.metrics.iter().any(|metric| {
            metric.field == "tool_calls_failed"
                && metric.direction == ScoreDirection::Improved
                && metric.points == 1
        }));
        assert!(run.metrics.iter().any(|metric| {
            metric.field == "oracle_eligible"
                && metric.direction == ScoreDirection::Improved
                && metric.points == 1
        }));
    }

    #[test]
    fn malformed_typed_eval_set_identity_keeps_score_incomplete() {
        let cases = [
            (
                "blank-id",
                serde_json::json!({
                    "id": "",
                    "kind": "closure_instance_slice",
                    "authority": "typed_closure_context",
                    "explicit": true,
                    "benchmark_family": "multi_swe_bench_rust",
                    "dataset_sources": [{
                        "path": "dataset.jsonl",
                        "label": "sample"
                    }],
                    "eval_policy": {},
                    "instance_ids": ["instance-a"]
                }),
                "blank required id",
            ),
            (
                "blank-kind",
                serde_json::json!({
                    "id": "prototype1.eval_set.closure_instance_slice.v1:test",
                    "kind": "   ",
                    "authority": "typed_closure_context",
                    "explicit": true,
                    "benchmark_family": "multi_swe_bench_rust",
                    "dataset_sources": [{
                        "path": "dataset.jsonl",
                        "label": "sample"
                    }],
                    "eval_policy": {},
                    "instance_ids": ["instance-a"]
                }),
                "blank required kind",
            ),
            (
                "blank-authority",
                serde_json::json!({
                    "id": "prototype1.eval_set.closure_instance_slice.v1:test",
                    "kind": "closure_instance_slice",
                    "authority": "\t",
                    "explicit": true,
                    "benchmark_family": "multi_swe_bench_rust",
                    "dataset_sources": [{
                        "path": "dataset.jsonl",
                        "label": "sample"
                    }],
                    "eval_policy": {},
                    "instance_ids": ["instance-a"]
                }),
                "blank required authority",
            ),
        ];

        for (case, eval_set_identity, expected_message) in cases {
            let tmp = tempfile::tempdir().expect("tmp");
            let manifest = campaign_manifest(tmp.path());
            write_node(tmp.path(), "node-a", "branch-a");
            write_evaluation_with_eval_set_identity(
                tmp.path(),
                "branch-a",
                "keep",
                metrics(false, false, 3),
                metrics(true, true, 1),
                eval_set_identity,
            );

            let evidence = FsEvidenceStore::new(&manifest)
                .child_evidence()
                .expect("child evidence");
            let scores = ScoreSet::from_evidence(&evidence);
            let child = &scores.children[0];
            let evaluation = &child.evaluations[0];

            assert_eq!(child.state, ScoreState::Incomplete, "{case}");
            assert_eq!(child.comparable_score(), None, "{case}");
            assert_eq!(evaluation.state, ScoreState::Incomplete, "{case}");
            assert_eq!(evaluation.score, Some(4), "{case}");
            assert!(diagnostic_message(
                &evaluation.diagnostics,
                "eval_set",
                expected_message
            ));
        }
    }

    #[test]
    fn keeps_missing_metrics_diagnostic_instead_of_scoring_paths() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        write_node(tmp.path(), "node-a", "branch-a");
        write_evaluation_without_metrics(tmp.path(), "branch-a");

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("child evidence");
        let scores = ScoreSet::from_evidence(&evidence);
        let evaluation = &scores.children[0].evaluations[0];

        assert_eq!(scores.children[0].state, ScoreState::Incomplete);
        assert_eq!(scores.children[0].score, None);
        assert_eq!(evaluation.state, ScoreState::Incomplete);
        assert_eq!(evaluation.score, None);
        assert_eq!(evaluation.comparable_runs, 0);
        assert_eq!(evaluation.missing_metric_runs, 1);
        assert_eq!(evaluation.runs[0].score, None);
        assert!(diagnostic_field(
            &evaluation.runs[0].diagnostics,
            "operational_metrics"
        ));
    }

    #[test]
    fn missing_run_registration_keeps_score_incomplete_and_non_comparable() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        write_node(tmp.path(), "node-a", "branch-a");
        write_evaluation_named_inner(
            tmp.path(),
            "branch-a",
            "branch-a",
            "keep",
            metrics(false, false, 3),
            metrics(true, true, 1),
            true,
            false,
        );

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("child evidence");
        let scores = ScoreSet::from_evidence(&evidence);
        let child = &scores.children[0];
        let evaluation = &child.evaluations[0];
        let run = &evaluation.runs[0];

        assert_eq!(child.state, ScoreState::Incomplete);
        assert_eq!(child.score, Some(4));
        assert_eq!(child.comparable_score(), None);
        assert_eq!(evaluation.state, ScoreState::Incomplete);
        assert_eq!(evaluation.score, Some(4));
        assert_eq!(evaluation.comparable_runs, 0);
        assert_eq!(evaluation.missing_metric_runs, 0);
        assert_eq!(run.state, ScoreState::Incomplete);
        assert_eq!(run.score, Some(4));
        assert!(diagnostic_field(
            &run.diagnostics,
            "baseline_run_registration"
        ));
        assert!(diagnostic_field(
            &run.diagnostics,
            "treatment_run_registration"
        ));
    }

    #[test]
    fn attaches_protocol_artifact_envelopes_to_run_provenance() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        write_node(tmp.path(), "node-a", "branch-a");
        let mut baseline_registration = write_run_registration(
            tmp.path(),
            "instance-a",
            "run-baseline",
            RegisteredRunRole::Control,
        );
        let baseline_anchor = write_protocol_artifact_envelope(
            &baseline_registration,
            "tool_call_intent_segmentation",
            1_000,
            Some("protocol-model"),
            Some("protocol-provider"),
        );
        baseline_registration.update_protocol_anchor(Some(baseline_anchor.clone()));
        baseline_registration.persist().expect("persist baseline");
        let treatment_registration = write_run_registration(
            tmp.path(),
            "instance-a",
            "run-treatment",
            RegisteredRunRole::Treatment,
        );
        write_evaluation_for_registrations(
            tmp.path(),
            "branch-a",
            metrics(false, false, 3),
            metrics(true, true, 1),
            &baseline_registration,
            &treatment_registration,
        );

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("child evidence");
        let scores = ScoreSet::from_evidence(&evidence);
        let run = &scores.children[0].evaluations[0].runs[0];
        let baseline = run
            .provenance
            .baseline_run
            .as_ref()
            .expect("baseline run evidence");

        assert_eq!(run.state, ScoreState::Complete);
        assert_eq!(run.score, Some(4));
        assert_eq!(
            baseline.protocol.anchor_path.as_deref(),
            Some(baseline_anchor.as_path())
        );
        assert_eq!(
            baseline.protocol.artifacts_dir,
            baseline_registration.artifacts.protocol_artifacts_dir
        );
        assert_eq!(baseline.protocol.artifacts.len(), 1);
        let artifact = &baseline.protocol.artifacts[0];
        assert_eq!(artifact.path, baseline_anchor);
        assert_eq!(artifact.procedure_name, "tool_call_intent_segmentation");
        assert_eq!(artifact.run_id, "run-baseline");
        assert_eq!(artifact.subject_id, "instance-a");
        assert_eq!(artifact.model_id.as_deref(), Some("protocol-model"));
        assert_eq!(artifact.provider_slug.as_deref(), Some("protocol-provider"));
        assert!(baseline.protocol.diagnostics.is_empty());
    }

    #[test]
    fn missing_protocol_artifact_refs_do_not_break_complete_operational_score() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        write_node(tmp.path(), "node-a", "branch-a");
        let mut baseline_registration = write_run_registration(
            tmp.path(),
            "instance-a",
            "run-baseline",
            RegisteredRunRole::Control,
        );
        let missing_anchor = baseline_registration
            .artifacts
            .protocol_artifacts_dir
            .join("missing-anchor.json");
        baseline_registration.update_protocol_anchor(Some(missing_anchor));
        baseline_registration.persist().expect("persist baseline");
        let treatment_registration = write_run_registration(
            tmp.path(),
            "instance-a",
            "run-treatment",
            RegisteredRunRole::Treatment,
        );
        write_evaluation_for_registrations(
            tmp.path(),
            "branch-a",
            metrics(false, false, 3),
            metrics(true, true, 1),
            &baseline_registration,
            &treatment_registration,
        );

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("child evidence");
        let scores = ScoreSet::from_evidence(&evidence);
        let child = &scores.children[0];
        let run = &child.evaluations[0].runs[0];
        let baseline = run
            .provenance
            .baseline_run
            .as_ref()
            .expect("baseline run evidence");

        assert_eq!(child.state, ScoreState::Complete);
        assert_eq!(child.comparable_score(), Some(4));
        assert_eq!(run.state, ScoreState::Complete);
        assert_eq!(baseline.protocol.artifacts.len(), 0);
        assert!(
            baseline
                .protocol
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.field == "protocol_artifacts_dir")
        );
        assert!(!diagnostic_field(
            &run.diagnostics,
            "protocol_artifacts_dir"
        ));
    }

    #[test]
    fn rejects_protocol_artifact_envelopes_with_wrong_identity() {
        let cases = [
            (
                "wrong schema version",
                "protocol_artifact.schema_version",
                Some("protocol-artifact.v0"),
                None,
                None,
            ),
            (
                "wrong run id",
                "protocol_artifact.run_id",
                None,
                Some("run-other"),
                None,
            ),
            (
                "wrong subject id",
                "protocol_artifact.subject_id",
                None,
                None,
                Some("instance-other"),
            ),
        ];

        for (case, expected_field, schema_version, run_id, subject_id) in cases {
            let tmp = tempfile::tempdir().expect("tmp");
            let manifest = campaign_manifest(tmp.path());
            write_node(tmp.path(), "node-a", "branch-a");
            let baseline_registration = write_run_registration(
                tmp.path(),
                "instance-a",
                "run-baseline",
                RegisteredRunRole::Control,
            );
            write_protocol_artifact_envelope_with_payload(
                &baseline_registration,
                "tool_call_intent_segmentation",
                1_000,
                ProtocolEnvelopeOverrides {
                    schema_version,
                    run_id,
                    subject_id,
                    ..ProtocolEnvelopeOverrides::default()
                },
                serde_json::json!({"fixture": "ignored"}),
                serde_json::json!({"fixture": "ignored"}),
                serde_json::json!({"fixture": "ignored"}),
            );
            let treatment_registration = write_run_registration(
                tmp.path(),
                "instance-a",
                "run-treatment",
                RegisteredRunRole::Treatment,
            );
            write_evaluation_for_registrations(
                tmp.path(),
                "branch-a",
                metrics(false, false, 3),
                metrics(true, true, 1),
                &baseline_registration,
                &treatment_registration,
            );

            let evidence = FsEvidenceStore::new(&manifest)
                .child_evidence()
                .expect("child evidence");
            let scores = ScoreSet::from_evidence(&evidence);
            let child = &scores.children[0];
            let run = &child.evaluations[0].runs[0];
            let baseline = run
                .provenance
                .baseline_run
                .as_ref()
                .expect("baseline run evidence");

            assert_eq!(child.state, ScoreState::Complete, "{case}");
            assert_eq!(child.comparable_score(), Some(4), "{case}");
            assert_eq!(run.state, ScoreState::Complete, "{case}");
            assert_eq!(baseline.protocol.artifacts.len(), 0, "{case}");
            assert!(
                baseline
                    .protocol
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.field == expected_field),
                "{case}"
            );
            assert!(
                !diagnostic_field(&run.diagnostics, expected_field),
                "{case}"
            );
        }
    }

    #[test]
    fn protocol_artifact_attachment_ignores_payload_identity_fields() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        write_node(tmp.path(), "node-a", "branch-a");
        let baseline_registration = write_run_registration(
            tmp.path(),
            "instance-a",
            "run-baseline",
            RegisteredRunRole::Control,
        );
        let artifact_path = write_protocol_artifact_envelope_with_payload(
            &baseline_registration,
            "tool_call_intent_segmentation",
            1_000,
            ProtocolEnvelopeOverrides {
                model_id: Some("protocol-model"),
                provider_slug: Some("protocol-provider"),
                ..ProtocolEnvelopeOverrides::default()
            },
            serde_json::json!({
                "schema_version": "payload-schema",
                "run_id": "payload-run",
                "subject_id": "payload-subject",
                "protocol_anchor": "/tmp/payload/anchor.json",
                "baseline_record_path": "/tmp/payload/baseline.json.gz"
            }),
            serde_json::json!({
                "run_id": "payload-output-run",
                "subject_id": "payload-output-subject",
                "protocol_artifacts_dir": "/tmp/payload/protocol-artifacts"
            }),
            serde_json::json!({
                "procedure_name": "payload_procedure",
                "run_id": "payload-artifact-run",
                "subject_id": "payload-artifact-subject",
                "path": "/tmp/payload/path-shaped-ref.json"
            }),
        );
        let treatment_registration = write_run_registration(
            tmp.path(),
            "instance-a",
            "run-treatment",
            RegisteredRunRole::Treatment,
        );
        write_evaluation_for_registrations(
            tmp.path(),
            "branch-a",
            metrics(false, false, 3),
            metrics(true, true, 1),
            &baseline_registration,
            &treatment_registration,
        );

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("child evidence");
        let scores = ScoreSet::from_evidence(&evidence);
        let run = &scores.children[0].evaluations[0].runs[0];
        let baseline = run
            .provenance
            .baseline_run
            .as_ref()
            .expect("baseline run evidence");

        assert_eq!(run.state, ScoreState::Complete);
        assert_eq!(run.score, Some(4));
        assert_eq!(baseline.protocol.artifacts.len(), 1);
        assert!(baseline.protocol.diagnostics.is_empty());

        let artifact = &baseline.protocol.artifacts[0];
        assert_eq!(artifact.path, artifact_path);
        assert_eq!(artifact.schema_version, "protocol-artifact.v1");
        assert_eq!(artifact.procedure_name, "tool_call_intent_segmentation");
        assert_eq!(artifact.run_id, "run-baseline");
        assert_eq!(artifact.subject_id, "instance-a");
        assert_eq!(artifact.model_id.as_deref(), Some("protocol-model"));
        assert_eq!(artifact.provider_slug.as_deref(), Some("protocol-provider"));
    }

    #[test]
    fn branch_mismatch_invalidates_evaluation_score() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        write_node(tmp.path(), "node-a", "branch-a");
        write_evaluation_without_identity(
            tmp.path(),
            "branch-a",
            "keep",
            metrics(false, false, 3),
            metrics(true, true, 1),
        );

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("child evidence");
        let mut child_evidence = evidence.children[0].clone();
        child_evidence.evaluations[0].branch_id = "branch-b".to_string();
        let child = ChildScore::from_child(&child_evidence);
        let evaluation = &child.evaluations[0];

        assert_eq!(child.state, ScoreState::Invalid);
        assert_eq!(child.score, None);
        assert_eq!(child.comparable_score(), None);
        assert_eq!(evaluation.state, ScoreState::Invalid);
        assert_eq!(evaluation.score, None);
        assert!(diagnostic_field(&evaluation.diagnostics, "branch_id"));
    }

    #[test]
    fn complete_child_score_is_comparable_only_through_gate() {
        let child = ChildScore {
            node_id: "node-a".to_string(),
            parent_node_id: Some("node-parent".to_string()),
            generation: Some(2),
            branch_id: Some("branch-a".to_string()),
            state: ScoreState::Complete,
            score: Some(7),
            evaluations: Vec::new(),
            diagnostics: Vec::new(),
        };

        assert_eq!(child.score, Some(7));
        assert_eq!(child.comparable_score(), Some(7));
    }

    #[test]
    fn score_selection_review_keeps_incomplete_score_out_of_local_alpha() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        write_node(tmp.path(), "node-a", "branch-a");
        write_evaluation_without_identity(
            tmp.path(),
            "branch-a",
            "keep",
            metrics(false, false, 3),
            metrics(true, true, 1),
        );

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("child evidence");
        let selection = evidence.selection_inputs();
        let expected_individual = crate::successor_selection::decide(selection.inputs[0].clone());
        let expected_generation =
            crate::successor_selection::decide_generation(selection.inputs.clone());
        let review = ScoreSelectionReview::from_evidence(&evidence);

        let row = review
            .rows
            .iter()
            .find(|row| row.node_id == "node-a" && row.branch_id.as_deref() == Some("branch-a"))
            .expect("review row");

        assert_eq!(row.selection_status, ScoreSelectionStatus::Projected);
        assert_eq!(row.child_score_state, Some(ScoreState::Incomplete));
        assert_eq!(row.diagnostic_score, Some(4));
        assert_eq!(row.local_alpha_candidate, None);
        assert_eq!(row.individual_decision.as_ref(), Some(&expected_individual));
        assert_eq!(row.generation_decision, expected_generation);
        assert!(review_diagnostic_field(row, "score_identity"));
    }

    #[test]
    fn score_selection_review_invalid_score_has_no_local_alpha() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        write_node(tmp.path(), "node-a", "branch-a");
        write_evaluation_named(
            tmp.path(),
            "branch-a-first",
            "branch-a",
            "keep",
            metrics(false, false, 3),
            metrics(true, true, 1),
        );
        write_evaluation_named(
            tmp.path(),
            "branch-a-second",
            "branch-a",
            "keep",
            metrics(false, false, 4),
            metrics(true, true, 1),
        );

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("child evidence");
        let review = ScoreSelectionReview::from_evidence(&evidence);

        let row = review
            .rows
            .iter()
            .find(|row| row.node_id == "node-a" && row.branch_id.as_deref() == Some("branch-a"))
            .expect("score row");

        assert_eq!(row.selection_status, ScoreSelectionStatus::MissingSelection);
        assert_eq!(row.child_score_state, Some(ScoreState::Invalid));
        assert_eq!(row.diagnostic_score, None);
        assert_eq!(row.local_alpha_candidate, None);
        assert!(review_diagnostic_field(row, "child_score"));
        assert!(review.rows.iter().any(|row| {
            row.node_id == "node-a"
                && row.selection_status == ScoreSelectionStatus::FailedProjection
        }));
    }

    #[test]
    fn score_selection_review_reports_selection_input_without_score_row() {
        let review = ScoreSelectionReview::from_parts(
            empty_scores(),
            super::SelectionProjectionSet {
                inputs: vec![selection_input(
                    "node-a",
                    "branch-a",
                    2,
                    BranchDisposition::Keep,
                )],
                failures: Vec::new(),
            },
        );

        let row = &review.rows[0];
        assert_eq!(row.selection_status, ScoreSelectionStatus::Projected);
        assert_eq!(row.child_score_state, None);
        assert_eq!(row.local_alpha_candidate, None);
        assert!(review_diagnostic_field(row, "score_row"));
    }

    #[test]
    fn score_selection_review_reports_score_row_without_selection_input() {
        let review = ScoreSelectionReview::from_parts(
            ScoreSet {
                schema_version: super::SCHEMA_VERSION.to_string(),
                procedure_id: super::PROCEDURE_ID.to_string(),
                children: vec![complete_score("node-a", "branch-a", 2, 7)],
                diagnostics: Vec::new(),
            },
            super::SelectionProjectionSet {
                inputs: Vec::new(),
                failures: Vec::new(),
            },
        );

        let row = &review.rows[0];
        assert_eq!(row.selection_status, ScoreSelectionStatus::MissingSelection);
        assert_eq!(row.child_score_state, Some(ScoreState::Complete));
        assert_eq!(row.diagnostic_score, Some(7));
        assert_eq!(row.local_alpha_candidate, None);
        assert!(review_diagnostic_field(row, "selection_input"));
    }

    #[test]
    fn duplicate_branch_evaluations_invalidate_child_score() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        write_node(tmp.path(), "node-a", "branch-a");
        write_evaluation_named(
            tmp.path(),
            "branch-a-first",
            "branch-a",
            "keep",
            metrics(false, false, 3),
            metrics(true, true, 1),
        );
        write_evaluation_named(
            tmp.path(),
            "branch-a-second",
            "branch-a",
            "keep",
            metrics(false, false, 4),
            metrics(true, true, 1),
        );

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("child evidence");
        let scores = ScoreSet::from_evidence(&evidence);
        let child = &scores.children[0];

        assert_eq!(child.evaluations.len(), 2);
        assert_eq!(child.state, ScoreState::Invalid);
        assert_eq!(child.score, None);
        assert_eq!(child.comparable_score(), None);
        assert!(diagnostic_field(&child.diagnostics, "evaluations"));
    }

    #[test]
    fn score_build_aborts_on_malformed_declared_typed_record() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        let node_dir = tmp.path().join("prototype1/nodes/node-a");
        fs::create_dir_all(&node_dir).expect("node dir");
        fs::write(node_dir.join("node.json"), "{ malformed").expect("node");

        let err = super::build("campaign-a", &manifest).expect_err("malformed record aborts");

        match err {
            super::PreviewError::ParseRecord { path, record, .. } => {
                assert_eq!(path, node_dir.join("node.json"));
                assert_eq!(record, "Prototype1NodeRecord");
            }
            other => panic!("expected ParseRecord, got {other:?}"),
        }
    }

    fn campaign_manifest(root: &Path) -> std::path::PathBuf {
        let manifest = root.join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        manifest
    }

    fn write_node(root: &Path, node_id: &str, branch_id: &str) {
        let node_dir = root.join("prototype1/nodes").join(node_id);
        fs::create_dir_all(&node_dir).expect("node dir");
        fs::write(
            node_dir.join("node.json"),
            serde_json::json!({
                "schema_version": "prototype1-treatment-node.v1",
                "node_id": node_id,
                "parent_node_id": "node-parent",
                "generation": 2,
                "instance_id": "instance-a",
                "source_state_id": "source-a",
                "branch_id": branch_id,
                "candidate_id": "candidate-a",
                "target_relpath": "crates/ploke-core/tool_text/non_semantic_patch.md",
                "node_dir": node_dir,
                "workspace_root": root.join("workspace"),
                "binary_path": root.join("target/debug/ploke-eval"),
                "runner_request_path": node_dir.join("runner-request.json"),
                "runner_result_path": node_dir.join("runner-result.json"),
                "status": "planned",
                "created_at": "2026-05-06T00:00:00Z",
                "updated_at": "2026-05-06T00:00:00Z"
            })
            .to_string(),
        )
        .expect("node");
    }

    fn write_evaluation(
        root: &Path,
        branch_id: &str,
        disposition: &str,
        parent_metrics: OperationalRunMetrics,
        child_metrics: OperationalRunMetrics,
    ) {
        write_evaluation_named(
            root,
            branch_id,
            branch_id,
            disposition,
            parent_metrics,
            child_metrics,
        );
    }

    fn write_evaluation_without_identity(
        root: &Path,
        branch_id: &str,
        disposition: &str,
        parent_metrics: OperationalRunMetrics,
        child_metrics: OperationalRunMetrics,
    ) {
        write_evaluation_named_inner(
            root,
            branch_id,
            branch_id,
            disposition,
            parent_metrics,
            child_metrics,
            false,
            false,
        );
    }

    fn write_evaluation_with_eval_set_identity(
        root: &Path,
        branch_id: &str,
        disposition: &str,
        parent_metrics: OperationalRunMetrics,
        child_metrics: OperationalRunMetrics,
        eval_set_identity: serde_json::Value,
    ) {
        write_evaluation_named_inner(
            root,
            branch_id,
            branch_id,
            disposition,
            parent_metrics,
            child_metrics,
            true,
            true,
        );
        let path = root
            .join("prototype1/evaluations")
            .join(format!("{branch_id}.json"));
        let mut report: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("evaluation"))
                .expect("evaluation json");
        report["eval_set_identity"] = eval_set_identity;
        fs::write(path, report.to_string()).expect("evaluation");
    }

    fn write_evaluation_named(
        root: &Path,
        file_stem: &str,
        branch_id: &str,
        disposition: &str,
        parent_metrics: OperationalRunMetrics,
        child_metrics: OperationalRunMetrics,
    ) {
        write_evaluation_named_inner(
            root,
            file_stem,
            branch_id,
            disposition,
            parent_metrics,
            child_metrics,
            true,
            true,
        );
    }

    fn write_evaluation_named_inner(
        root: &Path,
        file_stem: &str,
        branch_id: &str,
        disposition: &str,
        parent_metrics: OperationalRunMetrics,
        child_metrics: OperationalRunMetrics,
        include_identity: bool,
        include_registration: bool,
    ) {
        let eval_dir = root.join("prototype1/evaluations");
        fs::create_dir_all(&eval_dir).expect("eval dir");
        let (baseline_registration, treatment_registration) = if include_registration {
            (
                Some(write_run_registration(
                    root,
                    "instance-a",
                    "run-baseline",
                    RegisteredRunRole::Control,
                )),
                Some(write_run_registration(
                    root,
                    "instance-a",
                    "run-treatment",
                    RegisteredRunRole::Treatment,
                )),
            )
        } else {
            (None, None)
        };
        let baseline_record_path = baseline_registration
            .as_ref()
            .map(|registration| registration.artifacts.record_path.clone())
            .unwrap_or_else(|| Path::new("/tmp/baseline/record.json.gz").to_path_buf());
        let treatment_record_path = treatment_registration
            .as_ref()
            .map(|registration| registration.artifacts.record_path.clone())
            .unwrap_or_else(|| Path::new("/tmp/treatment/record.json.gz").to_path_buf());
        let mut report = serde_json::json!({
            "baseline_campaign_id": "baseline-campaign",
            "branch_id": branch_id,
            "treatment_campaign_id": "treatment-campaign",
            "branch_registry_path": root.join("prototype1/branches.json"),
            "evaluation_artifact_path": eval_dir.join(format!("{branch_id}.json")),
            "treatment_campaign_manifest": root.join("campaign.json"),
            "treatment_closure_state_path": root.join("closure-state.json"),
            "overall_disposition": disposition,
            "reasons": ["test"],
            "compared_instances": [{
                "instance_id": "instance-a",
                "baseline_record_path": baseline_record_path,
                "treatment_record_path": treatment_record_path,
                "baseline_metrics": parent_metrics,
                "treatment_metrics": child_metrics,
                "evaluation": null,
                "status": "compared"
            }]
        });
        if let Some(registration) = baseline_registration.as_ref() {
            report["compared_instances"][0]["baseline_registration_path"] =
                serde_json::json!(registration.registry_path());
        }
        if let Some(registration) = treatment_registration.as_ref() {
            report["compared_instances"][0]["treatment_registration_path"] =
                serde_json::json!(registration.registry_path());
        }
        if include_identity {
            report["evaluation_procedure_id"] =
                serde_json::json!("prototype1.branch_evaluation.operational_metrics.v1");
            report["evaluator_identity"] = serde_json::json!({
                "id": "prototype1.branch_evaluation.mechanized",
                "version": "v1"
            });
            report["eval_set_identity"] = serde_json::json!({
                "id": "prototype1.eval_set.closure_instance_slice.v1:test",
                "kind": "closure_instance_slice",
                "authority": "typed_closure_context",
                "explicit": true,
                "benchmark_family": "multi_swe_bench_rust",
                "dataset_sources": [{
                    "path": root.join("dataset.jsonl"),
                    "label": "sample"
                }],
                "eval_policy": {},
                "instance_ids": ["instance-a"]
            });
        }
        fs::write(
            eval_dir.join(format!("{file_stem}.json")),
            report.to_string(),
        )
        .expect("evaluation");
    }

    fn write_evaluation_for_registrations(
        root: &Path,
        branch_id: &str,
        parent_metrics: OperationalRunMetrics,
        child_metrics: OperationalRunMetrics,
        baseline_registration: &RunRegistration,
        treatment_registration: &RunRegistration,
    ) {
        let eval_dir = root.join("prototype1/evaluations");
        fs::create_dir_all(&eval_dir).expect("eval dir");
        fs::write(
            eval_dir.join(format!("{branch_id}.json")),
            serde_json::json!({
                "baseline_campaign_id": "baseline-campaign",
                "branch_id": branch_id,
                "treatment_campaign_id": "treatment-campaign",
                "branch_registry_path": root.join("prototype1/branches.json"),
                "evaluation_artifact_path": eval_dir.join(format!("{branch_id}.json")),
                "treatment_campaign_manifest": root.join("campaign.json"),
                "treatment_closure_state_path": root.join("closure-state.json"),
                "overall_disposition": "keep",
                "reasons": ["test"],
                "evaluation_procedure_id": "prototype1.branch_evaluation.operational_metrics.v1",
                "evaluator_identity": {
                    "id": "prototype1.branch_evaluation.mechanized",
                    "version": "v1"
                },
                "eval_set_identity": {
                    "id": "prototype1.eval_set.closure_instance_slice.v1:test",
                    "kind": "closure_instance_slice",
                    "authority": "typed_closure_context",
                    "explicit": true,
                    "benchmark_family": "multi_swe_bench_rust",
                    "dataset_sources": [{
                        "path": root.join("dataset.jsonl"),
                        "label": "sample"
                    }],
                    "eval_policy": {},
                    "instance_ids": ["instance-a"]
                },
                "compared_instances": [{
                    "instance_id": "instance-a",
                    "baseline_registration_path": baseline_registration.registry_path(),
                    "treatment_registration_path": treatment_registration.registry_path(),
                    "baseline_record_path": baseline_registration.artifacts.record_path,
                    "treatment_record_path": treatment_registration.artifacts.record_path,
                    "baseline_metrics": parent_metrics,
                    "treatment_metrics": child_metrics,
                    "evaluation": null,
                    "status": "compared"
                }]
            })
            .to_string(),
        )
        .expect("evaluation");
    }

    fn write_run_registration(
        root: &Path,
        instance_id: &str,
        run_id: &str,
        run_role: RegisteredRunRole,
    ) -> RunRegistration {
        let intent = RunIntent {
            task_id: instance_id.to_string(),
            repo_root: root.join("repo"),
            storage_roots: RunStorageRoots::new(root.join("registries"), root.join("runs")),
            base_sha: Some("deadbeef".to_string()),
            budget: EvalBudget::default(),
            model_id: Some("model".to_string()),
            provider_slug: Some("provider".to_string()),
            campaign_id: Some("campaign-a".to_string()),
            batch_id: Some("batch-a".to_string()),
            run_arm_id: format!("{run_id}-arm"),
            run_role,
        };
        let registration =
            RunRegistration::register_with_run_id(intent, run_id).expect("registration");
        registration.persist().expect("persist registration");
        registration
    }

    fn write_protocol_artifact_envelope(
        registration: &RunRegistration,
        procedure_name: &str,
        created_at_ms: u64,
        model_id: Option<&str>,
        provider_slug: Option<&str>,
    ) -> std::path::PathBuf {
        write_protocol_artifact_envelope_with_payload(
            registration,
            procedure_name,
            created_at_ms,
            ProtocolEnvelopeOverrides {
                model_id,
                provider_slug,
                ..ProtocolEnvelopeOverrides::default()
            },
            serde_json::json!({"fixture": "ignored"}),
            serde_json::json!({"fixture": "ignored"}),
            serde_json::json!({"fixture": "ignored"}),
        )
    }

    #[derive(Default)]
    struct ProtocolEnvelopeOverrides<'a> {
        schema_version: Option<&'a str>,
        run_id: Option<&'a str>,
        subject_id: Option<&'a str>,
        model_id: Option<&'a str>,
        provider_slug: Option<&'a str>,
    }

    fn write_protocol_artifact_envelope_with_payload(
        registration: &RunRegistration,
        procedure_name: &str,
        created_at_ms: u64,
        overrides: ProtocolEnvelopeOverrides<'_>,
        input: serde_json::Value,
        output: serde_json::Value,
        artifact: serde_json::Value,
    ) -> std::path::PathBuf {
        fs::create_dir_all(&registration.artifacts.protocol_artifacts_dir)
            .expect("protocol artifacts dir");
        let path = registration.artifacts.protocol_artifacts_dir.join(format!(
            "{created_at_ms}_{procedure_name}_{}.json",
            registration.frozen_spec.task_id
        ));
        let mut stored = serde_json::json!({
            "schema_version": overrides.schema_version.unwrap_or("protocol-artifact.v1"),
            "procedure_name": procedure_name,
            "subject_id": overrides.subject_id.unwrap_or(&registration.frozen_spec.task_id),
            "run_id": overrides.run_id.unwrap_or(&registration.run_id),
            "created_at_ms": created_at_ms,
            "input": input,
            "output": output,
            "artifact": artifact
        });
        if let Some(model_id) = overrides.model_id {
            stored["model_id"] = serde_json::json!(model_id);
        }
        if let Some(provider_slug) = overrides.provider_slug {
            stored["provider_slug"] = serde_json::json!(provider_slug);
        }
        fs::write(&path, stored.to_string()).expect("protocol artifact");
        path
    }

    fn write_evaluation_without_metrics(root: &Path, branch_id: &str) {
        let eval_dir = root.join("prototype1/evaluations");
        fs::create_dir_all(&eval_dir).expect("eval dir");
        fs::write(
            eval_dir.join(format!("{branch_id}.json")),
            serde_json::json!({
                "baseline_campaign_id": "baseline-campaign",
                "branch_id": branch_id,
                "treatment_campaign_id": "treatment-campaign",
                "branch_registry_path": root.join("prototype1/branches.json"),
                "evaluation_artifact_path": eval_dir.join(format!("{branch_id}.json")),
                "treatment_campaign_manifest": root.join("campaign.json"),
                "treatment_closure_state_path": root.join("closure-state.json"),
                "overall_disposition": "keep",
                "reasons": ["test"],
                "compared_instances": [{
                    "instance_id": "instance-a",
                    "baseline_record_path": "/tmp/baseline/record.json.gz",
                    "treatment_record_path": "/tmp/treatment/record.json.gz",
                    "baseline_metrics": null,
                    "treatment_metrics": null,
                    "evaluation": null,
                    "status": "missing"
                }]
            })
            .to_string(),
        )
        .expect("evaluation");
    }

    fn metrics(
        oracle_eligible: bool,
        convergence: bool,
        failed_tool_calls: usize,
    ) -> OperationalRunMetrics {
        OperationalRunMetrics {
            tool_calls_total: 5,
            tool_calls_failed: failed_tool_calls,
            patch_attempted: true,
            patch_apply_state: if convergence {
                PatchApplyState::Applied
            } else {
                PatchApplyState::No
            },
            submission_artifact_state: if oracle_eligible {
                SubmissionArtifactState::Nonempty
            } else {
                SubmissionArtifactState::Missing
            },
            partial_patch_failures: 0,
            same_file_patch_retry_count: 0,
            same_file_patch_max_streak: 0,
            aborted: false,
            aborted_repair_loop: false,
            nonempty_valid_patch: convergence,
            convergence,
            oracle_eligible,
        }
    }

    fn empty_scores() -> ScoreSet {
        ScoreSet {
            schema_version: super::SCHEMA_VERSION.to_string(),
            procedure_id: super::PROCEDURE_ID.to_string(),
            children: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn complete_score(node_id: &str, branch_id: &str, generation: u32, score: i64) -> ChildScore {
        ChildScore {
            node_id: node_id.to_string(),
            parent_node_id: Some("node-parent".to_string()),
            generation: Some(generation),
            branch_id: Some(branch_id.to_string()),
            state: ScoreState::Complete,
            score: Some(score),
            evaluations: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn selection_input(
        node_id: &str,
        branch_id: &str,
        generation: u32,
        disposition: BranchDisposition,
    ) -> SelectionInput {
        SelectionInput::new(
            CandidateRef {
                node_id: node_id.to_string(),
                branch_id: branch_id.to_string(),
                generation,
            },
            disposition,
            Path::new("evaluations/branch-a.json").to_path_buf(),
            vec![RunComparison {
                instance_id: "instance-a".to_string(),
                parent_metrics: Some(metrics(false, false, 3)),
                child_metrics: Some(metrics(true, true, 1)),
                status: "compared".to_string(),
            }],
        )
    }

    fn diagnostic_field(diagnostics: &[super::ScoreDiagnostic], field: &str) -> bool {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field == field)
    }

    fn diagnostic_message(
        diagnostics: &[super::ScoreDiagnostic],
        field: &str,
        message: &str,
    ) -> bool {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field == field && diagnostic.message.contains(message))
    }

    fn review_diagnostic_field(row: &super::ScoreSelectionRow, field: &str) -> bool {
        row.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field == field)
    }
}
