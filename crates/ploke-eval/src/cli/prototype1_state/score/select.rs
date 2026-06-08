use super::*;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScoreSelectionReviewRequest {
    pub(crate) rows: usize,
    pub(crate) generation: Option<u32>,
    pub(crate) format: InspectOutputFormat,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ScoreSelectionSnapshot {
    pub(super) schema_version: String,
    pub(super) generated_at: String,
    pub(super) campaign_id: String,
    pub(super) manifest_path: PathBuf,
    pub(super) prototype_root: PathBuf,
    pub(super) review: ScoreSelectionReview,
}

impl ScoreSelectionSnapshot {
    pub(super) fn report(&self, request: &ScoreSelectionReviewRequest) -> ScoreSelectionReport {
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

        ScoreSelectionReport {
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
            missing_data: MissingDataSummary::from_rows(&rows),
            diagnostics: self.review.diagnostics.clone(),
            generation_filter: request.generation,
            rows,
        }
    }

    pub(super) fn print(&self, request: &ScoreSelectionReviewRequest) {
        let report = self.report(request);
        println!("prototype1 score-selection review");
        println!("{}", "-".repeat(40));
        println!("schema_version: {}", report.schema_version);
        println!("generated_at: {}", report.generated_at);
        println!("campaign_id: {}", report.campaign_id);
        println!("manifest: {}", report.manifest_path.display());
        println!("prototype_root: {}", report.prototype_root.display());
        println!("score_procedure_id: {}", report.score_procedure_id);
        println!("selection_procedure_id: {}", report.selection_procedure_id);
        println!("rows: {}", report.row_count);
        println!("total_rows: {}", report.total_rows);
        println!("matching_rows: {}", report.total_matching_rows);
        println!("missing_data_total: {}", report.missing_data.total);
        println!(
            "missing_data_rows: {}",
            report.missing_data.rows_with_missing
        );
        println!(
            "missing_selection_rows: {}",
            report.missing_data.missing_selection_rows
        );
        println!(
            "projection_failures: {}",
            report.missing_data.projection_failures
        );
        if let Some(generation) = report.generation_filter {
            println!("generation_filter: {generation}");
        }
        println!(
            "report_note: read-only review; no selection change, archive traversal, or History admission"
        );
        println!();

        println!("rows");
        println!("{}", "-".repeat(40));
        println!(
            "gen | node | branch | selection | individual | generation_decision | score_state | diagnostic_score | local_alpha | diagnostics"
        );
        if report.rows.is_empty() {
            println!("(none)");
        } else {
            for row in &report.rows {
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

        let diagnostics = report
            .diagnostics
            .iter()
            .map(|diagnostic| {
                format!(
                    "global {} {}: {}",
                    diagnostic.severity, diagnostic.field, diagnostic.message
                )
            })
            .chain(report.rows.iter().flat_map(|row| {
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
pub(super) struct ScoreSelectionReport {
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
    missing_data: MissingDataSummary,
    diagnostics: Vec<ScoreSelectionDiagnostic>,
    generation_filter: Option<u32>,
    rows: Vec<ScoreSelectionRow>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MissingDataSummary {
    pub(crate) total: usize,
    pub(crate) rows_with_missing: usize,
    pub(crate) missing_selection_rows: usize,
    pub(crate) projection_failures: usize,
    pub(crate) by_field: BTreeMap<String, usize>,
}

impl MissingDataSummary {
    fn from_rows(rows: &[ScoreSelectionRow]) -> Self {
        let mut by_field = BTreeMap::new();
        let mut total = 0;
        let mut rows_with_missing = 0;
        let mut missing_selection_rows = 0;
        let mut projection_failures = 0;

        for row in rows {
            if row.selection_status == ScoreSelectionStatus::MissingSelection {
                missing_selection_rows += 1;
            }
            if row.selection_status == ScoreSelectionStatus::FailedProjection {
                projection_failures += 1;
            }

            let row_missing = row
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == "missing")
                .inspect(|diagnostic| {
                    total += 1;
                    *by_field.entry(diagnostic.field.clone()).or_default() += 1;
                })
                .count();
            if row_missing > 0 {
                rows_with_missing += 1;
            }
        }

        Self {
            total,
            rows_with_missing,
            missing_selection_rows,
            projection_failures,
            by_field,
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

    pub(super) fn from_parts(scores: ScoreSet, selection: SelectionProjectionSet) -> Self {
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
    pub(super) fn from_selection(
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

    pub(super) fn from_score_only(
        coordinate: ScoreCoordinate,
        matching_scores: &[ChildScore],
    ) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::InspectOutputFormat;

    #[test]
    fn score_selection_report_summarizes_missing_runtime_data() {
        let mut missing_selection = ScoreSelectionRow::base(
            "node-a".to_string(),
            Some("branch-a".to_string()),
            Some(2),
            ScoreSelectionStatus::MissingSelection,
        );
        missing_selection
            .diagnostics
            .push(ScoreSelectionDiagnostic::missing(
                "selection_input",
                "score row has no joined selection input",
            ));

        let mut projected_without_score = ScoreSelectionRow::base(
            "node-b".to_string(),
            Some("branch-b".to_string()),
            Some(2),
            ScoreSelectionStatus::Projected,
        );
        projected_without_score
            .diagnostics
            .push(ScoreSelectionDiagnostic::missing(
                "score_row",
                "selection input has no joined child score row",
            ));
        projected_without_score
            .diagnostics
            .push(ScoreSelectionDiagnostic::invalid(
                "child_score",
                "invalid score rows are not missing data",
            ));

        let mut failed_projection = ScoreSelectionRow::base(
            "node-c".to_string(),
            None,
            None,
            ScoreSelectionStatus::FailedProjection,
        );
        failed_projection
            .diagnostics
            .push(ScoreSelectionDiagnostic {
                severity: "selection_failure".to_string(),
                field: "selection_projection".to_string(),
                message: "projection failed before a selection row was available".to_string(),
            });

        let snapshot = ScoreSelectionSnapshot {
            schema_version: "prototype1-score-selection-review.v1".to_string(),
            generated_at: "2026-06-08T00:00:00Z".to_string(),
            campaign_id: "campaign-a".to_string(),
            manifest_path: PathBuf::from("/tmp/campaign/manifest.json"),
            prototype_root: PathBuf::from("/tmp/campaign/prototype1"),
            review: ScoreSelectionReview {
                schema_version: "prototype1-score-selection-review.v1".to_string(),
                score_schema_version: "prototype1-score-projection.v1".to_string(),
                score_procedure_id: "prototype1.score.operational_metrics.v1".to_string(),
                selection_procedure_id: "prototype1.successor_selection.v1".to_string(),
                rows: vec![
                    missing_selection,
                    projected_without_score,
                    failed_projection,
                ],
                diagnostics: Vec::new(),
            },
        };

        let report = snapshot.report(&ScoreSelectionReviewRequest {
            rows: 10,
            generation: None,
            format: InspectOutputFormat::Json,
        });

        assert_eq!(report.missing_data.total, 2);
        assert_eq!(report.missing_data.rows_with_missing, 2);
        assert_eq!(report.missing_data.missing_selection_rows, 1);
        assert_eq!(report.missing_data.projection_failures, 1);
        assert_eq!(
            report.missing_data.by_field.get("selection_input"),
            Some(&1)
        );
        assert_eq!(report.missing_data.by_field.get("score_row"), Some(&1));
        assert_eq!(report.missing_data.by_field.get("child_score"), None);
    }
}
