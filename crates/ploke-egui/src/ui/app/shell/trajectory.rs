//! Campaign trajectory landing table and selection drilldown.

use std::sync::Arc;

use eframe::egui;
use ploke_records::selection::{Outcome as SelectionOutcome, ScoreProfile};
use ploke_tree::Graph;
use ploke_tree::graph::{CandidateSource, SelectionMetricWitnessRef, TrajectoryGenerationRow};

use crate::allocation::scope;
use crate::ui::id_display::ShortId;
use crate::ui::inspector::GraphRevision;
use crate::ui::view::{GraphSelectionDetail, GraphSelectionRef, GraphView};

use super::fields::{
    cached_kv_i64, cached_kv_id, cached_kv_text, cached_kv_u64, cached_kv_usize, cached_label,
    cached_monospace_label,
};
use super::{InspectorRenderCache, render_imp_at_k_counts};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrajectoryTrend {
    Improving,
    Flat,
    Declining,
    Insufficient,
}

impl TrajectoryTrend {
    fn label(self) -> &'static str {
        match self {
            Self::Improving => "improving",
            Self::Flat => "flat",
            Self::Declining => "declining",
            Self::Insufficient => "insufficient data",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TrajectoryTableSnapshot {
    campaign_lineage: Arc<str>,
    policy_id: Arc<str>,
    evaluator: Arc<str>,
    rows: Arc<[TrajectoryDisplayRow]>,
    trend: TrajectoryTrend,
    trend_line: Arc<str>,
}

#[derive(Debug, Clone)]
struct TrajectoryDisplayRow {
    generation: u32,
    generation_label: Arc<str>,
    selection_entry_id: ploke_records::ids::EntryId,
    artifact_label: Arc<str>,
    score_label: Arc<str>,
    trend_label: Arc<str>,
    outcome_label: Arc<str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrajectoryCacheKey {
    revision: GraphRevision,
    row_count: usize,
}

#[derive(Debug, Default)]
struct TrajectoryCache {
    key: Option<TrajectoryCacheKey>,
    snapshot: Option<TrajectoryTableSnapshot>,
    rebuilds: usize,
}

impl TrajectoryCache {
    fn snapshot(&mut self, graph: &Graph, revision: GraphRevision) -> &TrajectoryTableSnapshot {
        let row_count = graph.trajectory_generations().len();
        let key = TrajectoryCacheKey {
            revision,
            row_count,
        };
        if self.key != Some(key) {
            self.snapshot = Some(build_trajectory_table(graph));
            self.key = Some(key);
            self.rebuilds += 1;
        }
        self.snapshot
            .as_ref()
            .expect("trajectory snapshot populated")
    }
}

#[derive(Debug, Default)]
pub(crate) struct TrajectoryRenderCache {
    table: TrajectoryCache,
}

impl TrajectoryRenderCache {
    fn table(&mut self, graph: &Graph, revision: GraphRevision) -> &TrajectoryTableSnapshot {
        self.table.snapshot(graph, revision)
    }

    #[cfg(test)]
    fn table_rebuilds(&self) -> usize {
        self.table.rebuilds
    }
}

impl InspectorRenderCache {
    pub(super) fn trajectory_table(
        &mut self,
        graph: &Graph,
        revision: GraphRevision,
    ) -> &TrajectoryTableSnapshot {
        self.trajectory.table(graph, revision)
    }

    #[cfg(test)]
    pub(super) fn trajectory_table_rebuilds(&self) -> usize {
        self.trajectory.table_rebuilds()
    }
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::TRAJECTORY_PANE)]
pub(crate) fn render_trajectory_pane(
    ui: &mut egui::Ui,
    graph: &Graph,
    view: &mut GraphView,
    revision: GraphRevision,
    render_cache: &mut InspectorRenderCache,
) {
    let snapshot = render_cache.trajectory_table(graph, revision).clone();
    ui.heading("Trajectory");
    ui.separator();

    cached_kv_text(
        ui,
        render_cache,
        "campaign / lineage",
        snapshot.campaign_lineage.as_ref(),
    );
    cached_kv_text(ui, render_cache, "policy id", snapshot.policy_id.as_ref());
    cached_kv_text(ui, render_cache, "evaluator", snapshot.evaluator.as_ref());

    ui.separator();
    ui.label(egui::RichText::new(format!(
        "Trajectory: {}",
        snapshot.trend.label()
    )));
    ui.label(snapshot.trend_line.as_ref());

    if snapshot.rows.is_empty() {
        ui.label(
            "No selection generations recorded. Load a multi-generation fixture for trend view.",
        );
        return;
    }

    ui.separator();
    ui.label(egui::RichText::new("Generations").strong());

    egui::Grid::new("trajectory-generation-table")
        .striped(true)
        .num_columns(5)
        .show(ui, |ui| {
            cached_label(ui, render_cache, "gen");
            cached_label(ui, render_cache, "selected artifact");
            cached_label(ui, render_cache, "score_child_prop");
            cached_label(ui, render_cache, "trend");
            cached_label(ui, render_cache, "outcome");
            ui.end_row();

            for row in snapshot.rows.iter() {
                let selected = view
                    .selected_reference(graph)
                    .is_some_and(|reference| reference == &selection_ref(&row.selection_entry_id));
                let response = ui.selectable_label(selected, row.generation_label.as_ref());
                cached_monospace_label(ui, render_cache, row.artifact_label.as_ref());
                cached_monospace_label(ui, render_cache, row.score_label.as_ref());
                cached_monospace_label(ui, render_cache, row.trend_label.as_ref());
                cached_monospace_label(ui, render_cache, row.outcome_label.as_ref());
                ui.end_row();

                if response.clicked() {
                    view.select_reference(graph, &selection_ref(&row.selection_entry_id));
                }
            }
        });
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::TRAJECTORY_SELECTION_DRILLDOWN)]
pub(crate) fn render_selection_drilldown_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    entry_id: &str,
    render_cache: &mut InspectorRenderCache,
) {
    let entry = ploke_records::ids::EntryId(entry_id.to_owned());
    let Some(selection) = graph.selections.selections.get(&entry) else {
        cached_kv_text(ui, render_cache, "selection", "not_found");
        return;
    };

    cached_kv_id(
        ui,
        render_cache,
        "selection entry",
        selection.entry_id.0.as_str(),
    );
    cached_kv_text(
        ui,
        render_cache,
        "procedure / policy",
        selection.procedure_or_policy.value.as_str(),
    );
    cached_kv_usize(
        ui,
        render_cache,
        "candidates considered",
        selection.considered_count,
    );
    cached_kv_usize(
        ui,
        render_cache,
        "projection failures",
        selection.projection_failure_count,
    );
    cached_kv_text(
        ui,
        render_cache,
        "decision outcome",
        outcome_label(selection.decision_outcome),
    );

    if let Some(traversal) = selection.traversal.as_ref() {
        cached_kv_u64(ui, render_cache, "traversal seed", traversal.seed);
        cached_kv_text(
            ui,
            render_cache,
            "traversal strategy",
            &format!("{:?}", traversal.strategy),
        );
        if let Some(source) = traversal.selected_source {
            cached_kv_text(
                ui,
                render_cache,
                "selected source",
                candidate_source_label(source),
            );
        }
    }

    if let Some(generation) = selection.generation_label.as_deref() {
        cached_kv_text(ui, render_cache, "generation label", generation);
    }

    let witness = selected_witness(graph, selection);
    ui.separator();
    ui.label(egui::RichText::new("score_child_prop witness").strong());
    if let Some(witness) = witness {
        render_witness_breakdown(ui, witness, render_cache);
    } else {
        cached_kv_text(ui, render_cache, "witness", "not_available");
    }
}

fn render_witness_breakdown(
    ui: &mut egui::Ui,
    witness: SelectionMetricWitnessRef<'_>,
    render_cache: &mut InspectorRenderCache,
) {
    cached_kv_usize(
        ui,
        render_cache,
        "payload index",
        witness.metric_candidate.payload_index,
    );
    cached_kv_id(
        ui,
        render_cache,
        "branch id",
        witness.key.branch_id.as_str(),
    );
    cached_kv_id(
        ui,
        render_cache,
        "candidate",
        witness.metric_candidate.candidate.as_str(),
    );

    if let Some(row) = witness.score_child_prop_row {
        cached_kv_text(
            ui,
            render_cache,
            "selected row",
            if row.selected { "yes" } else { "no" },
        );
        if let Some(total) = witness.score_child_prop_total() {
            cached_kv_i64(ui, render_cache, "total points", total);
        }
        if let Some(performance) = row.performance {
            cached_kv_i64(ui, render_cache, "performance", performance);
        }
        cached_kv_i64(ui, render_cache, "outcome points", row.outcome_points);
        cached_kv_i64(
            ui,
            render_cache,
            "operational points",
            row.operational_points,
        );
        cached_kv_i64(ui, render_cache, "protocol points", row.protocol_points);
        if let Some(delta) = row.imp_at_k_delta {
            cached_kv_i64(ui, render_cache, "imp@k delta", delta);
        }
    }

    render_imp_at_k_counts(ui, render_cache, Some(witness.metric_candidate));
}

fn build_trajectory_table(graph: &Graph) -> TrajectoryTableSnapshot {
    let _span = tracing::trace_span!(scope::TRAJECTORY_ROWS_BUILD).entered();
    let source_rows = graph.trajectory_generations();
    let header = trajectory_header(graph, &source_rows);
    let mut display_rows = Vec::with_capacity(source_rows.len());
    let mut previous_score = None;

    for row in &source_rows {
        let trend_label = trend_label(previous_score, row.score_child_prop_total);
        if row.score_child_prop_total.is_some() {
            previous_score = row.score_child_prop_total;
        }
        display_rows.push(TrajectoryDisplayRow {
            generation: row.generation,
            generation_label: Arc::from(itoa::Buffer::new().format(row.generation)),
            selection_entry_id: row.selection_entry_id.clone(),
            artifact_label: compact_artifact_label(row),
            score_label: score_label(row.score_child_prop_total),
            trend_label: Arc::from(trend_label),
            outcome_label: Arc::from(outcome_label(row.decision_outcome)),
        });
    }

    let trend = trajectory_trend(&source_rows);
    TrajectoryTableSnapshot {
        campaign_lineage: header.campaign_lineage,
        policy_id: header.policy_id,
        evaluator: header.evaluator,
        rows: display_rows.into(),
        trend,
        trend_line: Arc::from(trend_summary(trend, source_rows.len())),
    }
}

struct TrajectoryHeader {
    campaign_lineage: Arc<str>,
    policy_id: Arc<str>,
    evaluator: Arc<str>,
}

fn trajectory_header(graph: &Graph, rows: &[TrajectoryGenerationRow]) -> TrajectoryHeader {
    let first_selection = rows
        .first()
        .and_then(|row| graph.selections.selections.get(&row.selection_entry_id));

    let campaign_lineage = graph
        .forest
        .as_ref()
        .map(|forest| forest.campaign.campaign_id.as_str())
        .or_else(|| {
            graph
                .history
                .lineages
                .keys()
                .next()
                .map(|lineage| lineage.0.as_str())
        })
        .unwrap_or("not_recorded");
    let policy_id = first_selection
        .map(|selection| selection.procedure_or_policy.value.as_str())
        .unwrap_or("not_recorded");
    let evaluator = first_selection
        .and_then(|selection| graph.metrics.sets.get(&selection.metric_set_id))
        .map(|set| score_profile_label(set.policy.score_profile))
        .unwrap_or("not_recorded");

    TrajectoryHeader {
        campaign_lineage: Arc::from(campaign_lineage),
        policy_id: Arc::from(policy_id),
        evaluator: Arc::from(evaluator),
    }
}

fn score_profile_label(profile: ScoreProfile) -> &'static str {
    match profile {
        ScoreProfile::OperationalQualityV1 => "operational_quality_v1",
    }
}

fn trajectory_trend(rows: &[TrajectoryGenerationRow]) -> TrajectoryTrend {
    let scores: Vec<i64> = rows
        .iter()
        .filter_map(|row| row.score_child_prop_total)
        .collect();
    if scores.len() < 2 {
        return TrajectoryTrend::Insufficient;
    }
    let first = scores.first().copied().unwrap_or(0);
    let last = scores.last().copied().unwrap_or(0);
    if last > first {
        TrajectoryTrend::Improving
    } else if last < first {
        TrajectoryTrend::Declining
    } else {
        TrajectoryTrend::Flat
    }
}

fn trend_summary(trend: TrajectoryTrend, generation_count: usize) -> String {
    if generation_count <= 1 {
        return "Single generation recorded; load trajectory-multi-gen.json for full trend."
            .to_owned();
    }
    format!(
        "{generation_count} generations; headline score trend is {}",
        trend.label()
    )
}

fn trend_label(previous: Option<i64>, current: Option<i64>) -> &'static str {
    match (previous, current) {
        (Some(previous), Some(current)) if current > previous => "↑",
        (Some(previous), Some(current)) if current < previous => "↓",
        (Some(_), Some(_)) => "—",
        _ => "—",
    }
}

fn score_label(score: Option<i64>) -> Arc<str> {
    score
        .map(|value| {
            let mut buffer = itoa::Buffer::new();
            Arc::from(buffer.format(value))
        })
        .unwrap_or_else(|| Arc::from("—"))
}

fn compact_artifact_label(row: &TrajectoryGenerationRow) -> Arc<str> {
    if let Some(artifact_id) = row.artifact_id.as_ref() {
        if let Some(short) = ShortId::new(artifact_id.0.as_str()) {
            return Arc::from(short.to_string());
        }
        return Arc::from(artifact_id.0.as_str());
    }
    row.branch_id
        .as_deref()
        .map(Arc::from)
        .unwrap_or_else(|| Arc::from("—"))
}

fn outcome_label(outcome: SelectionOutcome) -> &'static str {
    match outcome {
        SelectionOutcome::Accepted => "accepted",
        SelectionOutcome::ExploreFrom => "explore_from",
        SelectionOutcome::Stop => "stop",
    }
}

fn selection_ref(entry_id: &ploke_records::ids::EntryId) -> GraphSelectionRef {
    GraphSelectionRef::Selection {
        entry_id: entry_id.0.clone(),
    }
}

fn selected_witness<'g>(
    graph: &'g Graph,
    selection: &ploke_tree::graph::SelectionNode,
) -> Option<SelectionMetricWitnessRef<'g>> {
    graph
        .selections
        .metric_witnesses
        .values()
        .find(|witness| witness.selection_entry_id == selection.entry_id)
        .and_then(|witness| {
            witness
                .score_child_prop_row
                .as_ref()
                .filter(|row| row.selected)
                .map(|_| witness.key.clone())
        })
        .or_else(|| {
            graph
                .selections
                .metric_witnesses
                .values()
                .find(|witness| witness.selection_entry_id == selection.entry_id)
                .map(|witness| witness.key.clone())
        })
        .and_then(|key| graph.selection_metric_witness(&key))
}

fn candidate_source_label(source: CandidateSource) -> &'static str {
    match source {
        CandidateSource::History => "history",
        CandidateSource::CurrentGeneration => "current_generation",
    }
}

pub(crate) fn selection_detail_for_reference(
    graph: &Graph,
    reference: &GraphSelectionRef,
) -> Option<GraphSelectionDetail> {
    let GraphSelectionRef::Selection { entry_id } = reference else {
        return None;
    };
    let entry = ploke_records::ids::EntryId(entry_id.clone());
    let selection = graph.selections.selections.get(&entry)?;
    let generation = graph
        .trajectory_generations()
        .into_iter()
        .find(|row| row.selection_entry_id.0 == *entry_id)
        .map(|row| row.generation)
        .unwrap_or(0);
    Some(GraphSelectionDetail {
        kind: "selection".to_owned(),
        label: format!("gen {generation}"),
        detail: selection.procedure_or_policy.value.clone(),
        reference: reference.clone(),
    })
}
