use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::operational_metrics::OperationalRunMetrics;
use crate::protocol::protocol_aggregate::{
    ProtocolAggregate, ProtocolDerivedMetrics, ProtocolReviewSignals,
};

pub(crate) type Operational = OperationalRunMetrics;

pub(crate) trait Summary {
    fn selection_points(&self) -> i64;

    fn selection_delta_points(&self, _baseline: Option<&Self>) -> i64
    where
        Self: Sized,
    {
        self.selection_points()
    }
}

impl Summary for Operational {
    fn selection_points(&self) -> i64 {
        self.selection_quality_points()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Protocol {
    pub(crate) scanned_artifact_count: usize,
    pub(crate) artifact_counts: BTreeMap<String, usize>,
    pub(crate) total_calls_in_run: usize,
    pub(crate) total_segments_in_anchor: usize,
    pub(crate) reviewed_call_count: usize,
    pub(crate) reviewed_segment_count: usize,
    pub(crate) missing_call_count: usize,
    pub(crate) missing_segment_count: usize,
    pub(crate) skipped_segment_review_count: usize,
    pub(crate) segment_anchor_mismatch_count: usize,
    pub(crate) call_review_overall_counts: BTreeMap<String, usize>,
    pub(crate) segment_review_overall_counts: BTreeMap<String, usize>,
    pub(crate) call_review_confidence_counts: BTreeMap<String, usize>,
    pub(crate) segment_review_confidence_counts: BTreeMap<String, usize>,
    pub(crate) calls_with_segment_crosswalk: usize,
    pub(crate) calls_without_segment_crosswalk: usize,
    pub(crate) average_calls_per_anchor_segment_x1000: u64,
    pub(crate) review_signal_totals: BTreeMap<String, usize>,
}

impl From<&ProtocolAggregate> for Protocol {
    fn from(aggregate: &ProtocolAggregate) -> Self {
        Self {
            scanned_artifact_count: aggregate.coverage.scanned_artifact_count,
            artifact_counts: aggregate.coverage.artifact_counts.clone(),
            total_calls_in_run: aggregate.coverage.total_calls_in_run,
            total_segments_in_anchor: aggregate.coverage.total_segments_in_anchor,
            reviewed_call_count: aggregate.coverage.reviewed_call_count,
            reviewed_segment_count: aggregate.coverage.reviewed_segment_count,
            missing_call_count: aggregate.coverage.missing_call_indices.len(),
            missing_segment_count: aggregate.coverage.missing_segment_indices.len(),
            skipped_segment_review_count: aggregate.coverage.skipped_segment_review_count,
            segment_anchor_mismatch_count: aggregate.coverage.segment_anchor_mismatch_count,
            call_review_overall_counts: aggregate
                .derived_metrics
                .call_review_overall_counts
                .clone(),
            segment_review_overall_counts: aggregate
                .derived_metrics
                .segment_review_overall_counts
                .clone(),
            call_review_confidence_counts: aggregate
                .derived_metrics
                .call_review_confidence_counts
                .clone(),
            segment_review_confidence_counts: aggregate
                .derived_metrics
                .segment_review_confidence_counts
                .clone(),
            calls_with_segment_crosswalk: aggregate.derived_metrics.calls_with_segment_crosswalk,
            calls_without_segment_crosswalk: aggregate
                .derived_metrics
                .calls_without_segment_crosswalk,
            average_calls_per_anchor_segment_x1000: scaled_average(&aggregate.derived_metrics),
            review_signal_totals: review_signal_totals(aggregate),
        }
    }
}

impl Summary for Protocol {
    fn selection_points(&self) -> i64 {
        let mut score = 0_i64;
        score += self.reviewed_call_count as i64 * 20;
        score += self.reviewed_segment_count as i64 * 20;
        score += self.calls_with_segment_crosswalk as i64 * 15;
        score += self.count("focused_progress", &self.call_review_overall_counts) as i64 * 100;
        score += self.count("focused_progress", &self.segment_review_overall_counts) as i64 * 100;
        score -= self.missing_call_count as i64 * 25;
        score -= self.missing_segment_count as i64 * 25;
        score -= self.skipped_segment_review_count as i64 * 25;
        score -= self.segment_anchor_mismatch_count as i64 * 50;
        score -= self.calls_without_segment_crosswalk as i64 * 10;
        score -= self.count("failed_calls_in_scope", &self.review_signal_totals) as i64 * 5;
        score -= self.count("uncovered_calls_in_source", &self.review_signal_totals) as i64 * 5;
        score -= self.count("ambiguous_segments_in_source", &self.review_signal_totals) as i64 * 5;
        score -= self.count("candidate_concerns", &self.review_signal_totals) as i64 * 10;
        score
    }

    fn selection_delta_points(&self, baseline: Option<&Self>) -> i64 {
        let mut score = self.selection_points();
        if let Some(baseline) = baseline {
            score += positive_delta(baseline.reviewed_call_count, self.reviewed_call_count) * 50;
            score +=
                positive_delta(baseline.reviewed_segment_count, self.reviewed_segment_count) * 50;
            score += positive_delta(
                baseline.calls_with_segment_crosswalk,
                self.calls_with_segment_crosswalk,
            ) * 30;
            score += positive_delta(
                baseline.count("focused_progress", &baseline.call_review_overall_counts),
                self.count("focused_progress", &self.call_review_overall_counts),
            ) * 100;
            score += positive_delta(
                baseline.count("focused_progress", &baseline.segment_review_overall_counts),
                self.count("focused_progress", &self.segment_review_overall_counts),
            ) * 100;
            score += positive_delta(self.missing_call_count, baseline.missing_call_count) * 50;
            score +=
                positive_delta(self.missing_segment_count, baseline.missing_segment_count) * 50;
            score += positive_delta(
                self.skipped_segment_review_count,
                baseline.skipped_segment_review_count,
            ) * 50;
            score += positive_delta(
                self.segment_anchor_mismatch_count,
                baseline.segment_anchor_mismatch_count,
            ) * 75;
        }
        score
    }
}

impl Protocol {
    fn count(&self, key: &str, counts: &BTreeMap<String, usize>) -> usize {
        counts.get(key).copied().unwrap_or_default()
    }
}

fn scaled_average(metrics: &ProtocolDerivedMetrics) -> u64 {
    (metrics.average_calls_per_anchor_segment * 1_000.0)
        .round()
        .max(0.0) as u64
}

fn review_signal_totals(aggregate: &ProtocolAggregate) -> BTreeMap<String, usize> {
    let mut totals = BTreeMap::new();
    for signals in aggregate
        .call_reviews
        .iter()
        .map(|row| &row.signals)
        .chain(aggregate.segment_reviews.iter().map(|row| &row.signals))
    {
        add_review_signals(&mut totals, signals);
    }
    totals
}

fn add_review_signals(totals: &mut BTreeMap<String, usize>, signals: &ProtocolReviewSignals) {
    add_optional(
        totals,
        "browse_calls_in_scope",
        signals.browse_calls_in_scope,
    );
    add(
        totals,
        "candidate_concerns",
        signals.candidate_concerns.len(),
    );
    add_optional(totals, "directory_pivots", signals.directory_pivots);
    add_optional(totals, "distinct_tool_count", signals.distinct_tool_count);
    add_optional(totals, "edit_calls_in_scope", signals.edit_calls_in_scope);
    add_optional(
        totals,
        "execute_calls_in_scope",
        signals.execute_calls_in_scope,
    );
    add_optional(
        totals,
        "failed_calls_in_scope",
        signals.failed_calls_in_scope,
    );
    add_optional(
        totals,
        "labeled_segments_in_source",
        signals.labeled_segments_in_source,
    );
    add_optional(
        totals,
        "ambiguous_segments_in_source",
        signals.ambiguous_segments_in_source,
    );
    add_optional(totals, "read_calls_in_scope", signals.read_calls_in_scope);
    add_optional(
        totals,
        "repeated_tool_name_count",
        signals.repeated_tool_name_count,
    );
    add_optional(totals, "scope_turn_count", signals.scope_turn_count);
    add_optional(
        totals,
        "search_calls_in_scope",
        signals.search_calls_in_scope,
    );
    add_optional(
        totals,
        "similar_search_neighbors",
        signals.similar_search_neighbors,
    );
    add_optional(
        totals,
        "uncovered_calls_in_source",
        signals.uncovered_calls_in_source,
    );
}

fn add_optional(totals: &mut BTreeMap<String, usize>, key: &'static str, value: Option<usize>) {
    if let Some(value) = value {
        add(totals, key, value);
    }
}

fn add(totals: &mut BTreeMap<String, usize>, key: &'static str, value: usize) {
    if value > 0 {
        *totals.entry(key.to_string()).or_default() += value;
    }
}

fn positive_delta(before: usize, after: usize) -> i64 {
    after.saturating_sub(before) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operational_metrics::PatchApplyState;
    use crate::record::SubmissionArtifactState;

    #[test]
    fn operational_summary_uses_existing_quality_points() {
        let operational = OperationalRunMetrics {
            tool_calls_total: 2,
            tool_calls_failed: 0,
            patch_attempted: true,
            patch_apply_state: PatchApplyState::Applied,
            submission_artifact_state: SubmissionArtifactState::Nonempty,
            partial_patch_failures: 0,
            same_file_patch_retry_count: 0,
            same_file_patch_max_streak: 0,
            aborted: false,
            aborted_repair_loop: false,
            nonempty_valid_patch: true,
            convergence: true,
            oracle_eligible: true,
        };

        assert_eq!(
            operational.selection_points(),
            operational.selection_quality_points()
        );
    }

    #[test]
    fn protocol_summary_rewards_better_treatment() {
        let baseline = protocol(1, 1, 2, 2, 0);
        let treatment = protocol(3, 3, 0, 0, 2);

        assert!(treatment.selection_delta_points(Some(&baseline)) > treatment.selection_points());
        assert!(treatment.selection_points() > baseline.selection_points());
    }

    fn protocol(
        reviewed_call_count: usize,
        reviewed_segment_count: usize,
        missing_call_count: usize,
        missing_segment_count: usize,
        focused_progress: usize,
    ) -> Protocol {
        Protocol {
            scanned_artifact_count: 0,
            artifact_counts: BTreeMap::new(),
            total_calls_in_run: reviewed_call_count + missing_call_count,
            total_segments_in_anchor: reviewed_segment_count + missing_segment_count,
            reviewed_call_count,
            reviewed_segment_count,
            missing_call_count,
            missing_segment_count,
            skipped_segment_review_count: 0,
            segment_anchor_mismatch_count: 0,
            call_review_overall_counts: counts([("focused_progress", focused_progress)]),
            segment_review_overall_counts: counts([("focused_progress", focused_progress)]),
            call_review_confidence_counts: BTreeMap::new(),
            segment_review_confidence_counts: BTreeMap::new(),
            calls_with_segment_crosswalk: reviewed_call_count,
            calls_without_segment_crosswalk: 0,
            average_calls_per_anchor_segment_x1000: 0,
            review_signal_totals: BTreeMap::new(),
        }
    }

    fn counts(entries: impl IntoIterator<Item = (&'static str, usize)>) -> BTreeMap<String, usize> {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }
}
