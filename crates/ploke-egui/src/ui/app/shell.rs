//! Stable frame rendering for the operator graph UI.

use eframe::egui;

use crate::ui::id_display;
use crate::ui::id_display::TraceId;
use crate::ui::inspector::{
    ArtifactSourceSlot, BadgeSlot, IdentitySlot, InspectorSections, MetricsSlot, PatchInspection,
    RunRecordInspection, SelectionEdge, SourceRef, UnavailableReason, find_run_forest_node,
    phase_label, result_class_label, run_forest_node_identity, surface_apply_status_label,
    surface_check_status_label,
};
use crate::ui::view::{GraphViewDiagnostics, GraphViewMode};
use ploke_tree::Graph;
use ploke_tree::graph::{AgentTurnArtifactMetadata, ParentCreateAttempt, ParentCreateLookup};
use std::path::Path;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct InspectorOpenState {
    force_open: Option<InspectorPanelSection>,
}

impl InspectorOpenState {
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    pub(crate) fn benchmark(section: Option<crate::benchmark::BenchmarkInspectorSection>) -> Self {
        Self {
            force_open: section.map(InspectorPanelSection::from_benchmark),
        }
    }

    fn open(self, section: InspectorPanelSection) -> Option<bool> {
        (self.force_open == Some(section)).then_some(true)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorPanelSection {
    RunRecords,
    GraphEdges,
    ArtifactEdges,
    PatchDebug,
    SourceRefs,
    ArtifactIds,
}

impl InspectorPanelSection {
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    fn from_benchmark(section: crate::benchmark::BenchmarkInspectorSection) -> Self {
        match section {
            crate::benchmark::BenchmarkInspectorSection::RunRecords => Self::RunRecords,
            crate::benchmark::BenchmarkInspectorSection::GraphEdges => Self::GraphEdges,
            crate::benchmark::BenchmarkInspectorSection::ArtifactEdges => Self::ArtifactEdges,
            crate::benchmark::BenchmarkInspectorSection::PatchDebug => Self::PatchDebug,
            crate::benchmark::BenchmarkInspectorSection::SourceRefs => Self::SourceRefs,
            crate::benchmark::BenchmarkInspectorSection::ArtifactIds => Self::ArtifactIds,
        }
    }
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "top_strip")
)]
pub(crate) fn render_top_strip(
    ui: &mut egui::Ui,
    mode: GraphViewMode,
    run_name: Option<&str>,
    graph_has_content: bool,
) {
    ui.horizontal(|ui| {
        ui.label("ploke-egui");
        ui.separator();
        ui.label(format!("Mode: {}", mode.as_str()));
        if let Some(run_name) = run_name {
            ui.separator();
            ui.label(format!("Run: {run_name}"));
        }
        if !graph_has_content {
            ui.separator();
            ui.label("No run loaded");
        }
    });
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "selection_inspector")
)]
pub(crate) fn render_right_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    selection_kind: Option<&str>,
    selection_label: Option<&str>,
    sections: Option<&InspectorSections>,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
    open_state: InspectorOpenState,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("Inspector");
            ui.separator();

            ui.label("Summary");
            if let (Some(kind), Some(label)) = (selection_kind, selection_label) {
                kv(ui, "kind", kind);
                kv(ui, "label", label);
            } else {
                kv(ui, "selection", "not_applicable");
            }

            ui.separator();
            ui.label("Identity");
            if let Some(sections) = sections {
                render_identity(ui, graph, sections);
            } else {
                kv(ui, "record refs", "not_applicable");
            }

            ui.separator();
            ui.label("Roles");
            if let Some(sections) = sections {
                render_roles_and_metrics(ui, graph, sections);
            } else {
                kv(ui, "roles", "not_applicable");
            }

            ui.separator();
            ui.label("Patch Generation");
            if let Some(sections) = sections {
                render_parent_create_for_inspector(ui, graph, sections);
            } else {
                kv(ui, "attempt", "not_applicable");
            }

            ui.separator();
            egui::CollapsingHeader::new("Run Records")
                .open(open_state.open(InspectorPanelSection::RunRecords))
                .show(ui, |ui| {
                    if let Some(sections) = sections {
                        render_run_records_for_inspector(ui, graph, sections);
                    } else {
                        kv(ui, "run records", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Graph edges")
                .open(open_state.open(InspectorPanelSection::GraphEdges))
                .show(ui, |ui| {
                    if let Some(sections) = sections {
                        render_graph_edges_for_inspector(ui, sections);
                    } else {
                        kv(ui, "edges", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Artifact edges")
                .open(open_state.open(InspectorPanelSection::ArtifactEdges))
                .show(ui, |ui| {
                    if let Some(sections) = sections {
                        render_artifact_edges_for_inspector(ui, sections);
                    } else {
                        kv(ui, "artifact edges", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Patch Debug")
                .open(open_state.open(InspectorPanelSection::PatchDebug))
                .show(ui, |ui| {
                    if let Some(sections) = sections {
                        render_patches_for_inspector(ui, graph, sections, diff_cache);
                    } else {
                        kv(ui, "patch", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Source refs")
                .open(open_state.open(InspectorPanelSection::SourceRefs))
                .show(ui, |ui| {
                    if let Some(sections) = sections {
                        render_source_refs_for_inspector(ui, graph, sections);
                    } else {
                        kv(ui, "record refs", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Artifact Ids")
                .open(open_state.open(InspectorPanelSection::ArtifactIds))
                .show(ui, |ui| {
                    if let Some(sections) = sections {
                        render_artifact_ids_for_inspector(ui, graph, sections);
                    } else {
                        kv(ui, "artifact ids", "not_applicable");
                    }
                });
        });
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "timeline")
)]
pub(crate) fn render_bottom_timeline(
    ui: &mut egui::Ui,
    diagnostics: Option<&GraphViewDiagnostics>,
    selection_synced: bool,
) {
    ui.horizontal(|ui| {
        ui.label("Timeline");
        ui.separator();
        if let Some(diagnostics) = diagnostics {
            ui.label(format!(
                "nodes={}, edges={}",
                diagnostics.node_count, diagnostics.edge_count
            ));
        } else {
            ui.label("spans=0");
        }
        ui.separator();
        ui.label(format!(
            "selection={}",
            if selection_synced {
                "synced"
            } else {
                "not_applicable"
            }
        ));
        ui.separator();
        ui.label("order_strength=blocked");
    });
}

fn kv(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(key);
        id_display::expandable_id(ui, ("kv", key, value), value);
    });
}

fn kv_usize(ui: &mut egui::Ui, key: &str, value: usize) {
    let mut buffer = itoa::Buffer::new();
    kv(ui, key, buffer.format(value));
}

fn kv_path(ui: &mut egui::Ui, key: &str, path: &Path) {
    kv(ui, key, path.to_str().unwrap_or("non_utf8_path"));
}

fn render_badges(ui: &mut egui::Ui, badges: &[BadgeSlot]) {
    if badges.is_empty() {
        kv(ui, "none", "not_applicable");
        return;
    }
    for slot in badges {
        let badge = slot.badge();
        ui.horizontal(|ui| {
            let badge_text = badge.to_badge_text();
            let artifact_id = badge_text.artifact_id();
            badge_text.show(ui);
            id_display::expandable_id(
                ui,
                ("role-badge", artifact_id.0.as_str()),
                artifact_id.0.as_str(),
            );
        });
    }
}

fn render_identity(ui: &mut egui::Ui, graph: &Graph, sections: &InspectorSections) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }

    match sections.identity() {
        Some(IdentitySlot::RunForestNode { node_key }) => {
            if let Some(node) = find_run_forest_node(graph, node_key) {
                render_run_forest_identity(ui, node);
            } else {
                kv(ui, "run forest node", "not_found");
            }
        }
        Some(IdentitySlot::Artifact { sources }) => render_artifact_identity(ui, graph, sources),
        None => kv(ui, "identity", "not_available"),
    }
}

fn render_run_forest_identity(ui: &mut egui::Ui, node: &ploke_tree::TreeNode) {
    let identity = run_forest_node_identity(node);
    kv(ui, "run forest node", identity.node_key);
    kv(ui, "candidate", identity.candidate_id);
    kv(ui, "source artifact", identity.source_artifact);
    if let Some(parent) = identity.parent_node {
        kv(ui, "parent run forest node", parent);
    }
    // Artifact ids are still rendered as plain expandable ids here. The intended
    // UI is a progressive-discovery "Artifact Ids" drilldown that shows compact
    // prefix + short-hash forms first, expands to the full value on click, and
    // keeps copy affordances available for debugging. See
    // docs/active/archaeology/ploke-tree-graph/artifact-identity.md before
    // refactoring this into shared interaction behavior.
    if let Some(base) = identity.base_artifact {
        kv(ui, "base artifact", base);
    }
    if let Some(derived) = identity.derived_artifact {
        kv(ui, "derived artifact", derived);
    }
    if let Some(patch) = identity.patch {
        kv(ui, "patch", patch);
    }
    kv(ui, "branch", identity.branch_id);
    kv(ui, "target", identity.target_relpath);
    kv(ui, "phase", phase_label(identity.phase));
    kv(ui, "result", result_class_label(identity.result));
}

/// archaeology:artifact-identity
/// proof:docs/active/archaeology/ploke-tree-graph/artifact-identity.md
fn render_artifact_identity(ui: &mut egui::Ui, graph: &Graph, sources: &[ArtifactSourceSlot]) {
    if let Some(artifact_id) = primary_artifact_id(graph, sources) {
        render_fixed_id_row(
            ui,
            "artifact",
            ("artifact-primary", artifact_id.0.as_str()),
            artifact_id,
        );
        return;
    }

    if let Some(artifact_ref) = primary_artifact_ref(graph, sources) {
        render_fixed_id_row(
            ui,
            "artifact",
            ("artifact-ref-primary", artifact_ref.id().0.as_str()),
            artifact_ref,
        );
        return;
    }

    kv(ui, "artifact", artifact_label(graph, sources));
}

fn render_roles_and_metrics(ui: &mut egui::Ui, graph: &Graph, sections: &InspectorSections) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }

    render_badges(ui, sections.roles());
    match sections.metrics() {
        Some(MetricsSlot::RunForestNode { node_key }) => {
            if let Some(node) = find_run_forest_node(graph, node_key) {
                render_run_forest_metrics(ui, node);
            }
        }
        Some(MetricsSlot::Artifact { sources }) => render_artifact_metrics(ui, graph, sources),
        None => {}
    }
}

fn render_run_forest_metrics(ui: &mut egui::Ui, node: &ploke_tree::TreeNode) {
    ui.horizontal(|ui| {
        ui.label("generation");
        ui.monospace(node.generation.to_string());
    });
    ui.horizontal(|ui| {
        ui.label("child run forest nodes");
        ui.monospace(node.children.len().to_string());
    });
}

fn render_artifact_metrics(ui: &mut egui::Ui, graph: &Graph, sources: &[ArtifactSourceSlot]) {
    ui.horizontal(|ui| {
        ui.label("source records");
        ui.monospace(artifact_source_count(graph, sources).to_string());
    });
    ui.horizontal(|ui| {
        ui.label("evidence refs");
        ui.monospace(artifact_evidence_count(graph, sources).to_string());
    });
}

fn render_parent_create_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    match sections.parent_create() {
        Some(slot) => render_parent_create(ui, slot.resolve(graph)),
        None => kv(ui, "attempt", "not_available"),
    }
}

/// archaeology:run-record-branch-output
/// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_run_records")
)]
fn render_run_records_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_run_records(
        ui,
        sections
            .run_records()
            .iter()
            .filter_map(|slot| slot.resolve(graph)),
    );
}

fn render_run_records<'a>(
    ui: &mut egui::Ui,
    records: impl IntoIterator<Item = RunRecordInspection<'a>>,
) {
    let mut rendered = false;
    for record in records {
        rendered = true;
        ui.separator();
        kv(ui, "arm", compared_run_arm_label(record.record_ref.arm));
        kv(ui, "instance", record.record_ref.instance_id.as_str());
        kv_path(ui, "record", record.record_ref.record_path.as_path());
        kv(ui, "manifest", record.record.manifest_id.as_str());
        if let Some(model) = record.record.metadata.agent.model_id.as_deref() {
            kv(ui, "model", model);
        }
        if let Some(provider) = record.record.metadata.agent.provider.as_deref() {
            kv(ui, "provider", provider);
        }
        kv_path(
            ui,
            "repo root",
            record.record.metadata.benchmark.repo_root.as_path(),
        );
        kv_usize(ui, "turns", record.stats.turn_count);
        kv_usize(ui, "tool calls", record.stats.tool_call_count);
        kv_usize(ui, "failed tool calls", record.stats.failed_tool_call_count);
        if let Some(packaging) = record.record.phases.packaging.as_ref() {
            kv(
                ui,
                "submission",
                submission_artifact_state_label(packaging.submission_artifact_state),
            );
            kv(
                ui,
                "patch projection",
                patch_projection_check_state_label(packaging.patch_projection_check_state),
            );
        }
    }
    if !rendered {
        kv(ui, "run records", "none");
    }
}

fn compared_run_arm_label(arm: ploke_tree::ComparedRunArm) -> &'static str {
    match arm {
        ploke_tree::ComparedRunArm::Baseline => "baseline",
        ploke_tree::ComparedRunArm::Treatment => "treatment",
    }
}

fn submission_artifact_state_label(
    state: ploke_records::run_record::SubmissionArtifactState,
) -> &'static str {
    match state {
        ploke_records::run_record::SubmissionArtifactState::NotRecorded => "not_recorded",
        ploke_records::run_record::SubmissionArtifactState::NotApplicable => "not_applicable",
        ploke_records::run_record::SubmissionArtifactState::Missing => "missing",
        ploke_records::run_record::SubmissionArtifactState::Empty => "empty",
        ploke_records::run_record::SubmissionArtifactState::Nonempty => "nonempty",
    }
}

fn patch_projection_check_state_label(
    state: ploke_records::evaluation::PatchProjectionCheckState,
) -> &'static str {
    match state {
        ploke_records::evaluation::PatchProjectionCheckState::NotRecorded => "not_recorded",
        ploke_records::evaluation::PatchProjectionCheckState::NotApplicable => "not_applicable",
        ploke_records::evaluation::PatchProjectionCheckState::Passed => "passed",
        ploke_records::evaluation::PatchProjectionCheckState::Failed => "failed",
        ploke_records::evaluation::PatchProjectionCheckState::NotRun => "not_run",
    }
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_graph_edges")
)]
fn render_graph_edges_for_inspector(ui: &mut egui::Ui, sections: &InspectorSections) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_edges(
        ui,
        "in",
        sections.graph_edges_in().iter().map(|slot| slot.edge()),
    );
    render_edges(
        ui,
        "out",
        sections.graph_edges_out().iter().map(|slot| slot.edge()),
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_artifact_edges")
)]
fn render_artifact_edges_for_inspector(ui: &mut egui::Ui, sections: &InspectorSections) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_edges(
        ui,
        "in",
        sections.artifact_edges_in().iter().map(|slot| slot.edge()),
    );
    render_edges(
        ui,
        "out",
        sections.artifact_edges_out().iter().map(|slot| slot.edge()),
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_patch_debug")
)]
fn render_patches_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_patches(
        ui,
        sections
            .patches()
            .iter()
            .filter_map(|slot| slot.resolve(graph)),
        diff_cache,
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_source_refs")
)]
fn render_source_refs_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_source_refs(
        ui,
        sections
            .source_refs()
            .iter()
            .filter_map(|slot| slot.resolve(graph)),
    );
}

/// archaeology:artifact-identity
/// proof:docs/active/archaeology/ploke-tree-graph/artifact-identity.md
#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_artifact_ids")
)]
fn render_artifact_ids_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
) {
    if let Some(reason) = sections.unavailable() {
        let _span = tracing::trace_span!(
            "ploke_egui.inspector.artifact_ids_section",
            selection_kind = "unresolved",
            state = reason.state(),
            selection_key = reason.subject()
        )
        .entered();
        render_unavailable(ui, reason);
        return;
    }

    match sections.identity() {
        Some(IdentitySlot::RunForestNode { node_key }) => {
            let _span = tracing::trace_span!(
                "ploke_egui.inspector.artifact_ids_section",
                selection_kind = "run_forest_node",
                state = "not_applicable",
                selection_key = node_key.as_str()
            )
            .entered();
            kv(ui, "artifact ids", "not_applicable");
        }
        Some(IdentitySlot::Artifact { sources }) => {
            let _span = tracing::trace_span!(
                "ploke_egui.inspector.artifact_ids_section",
                selection_kind = "artifact",
                state = "rendered",
                selection_key = artifact_label(graph, sources)
            )
            .entered();
            render_artifact_ids(ui, graph, sources);
        }
        None => kv(ui, "artifact ids", "not_available"),
    }
}

fn render_artifact_ids(ui: &mut egui::Ui, graph: &Graph, sources: &[ArtifactSourceSlot]) {
    let _span = tracing::trace_span!(
        "ploke_egui.inspector.render_artifact_ids",
        primary_artifact_id = primary_artifact_id(graph, sources)
            .map(|artifact| artifact.0.as_str())
            .unwrap_or("not_recorded")
    )
    .entered();

    let mut saw_artifact_id = false;
    if let Some(source) = first_artifact_source(graph, sources) {
        for artifact_id in source.artifact_ids() {
            saw_artifact_id = true;
            render_prefixed_id_row(ui, "artifact id", artifact_id);
        }
    }
    if !saw_artifact_id {
        kv(ui, "artifact id", "not_recorded");
    }

    let mut saw_artifact_ref = false;
    if let Some(source) = first_artifact_source(graph, sources) {
        for artifact_ref in source.artifact_refs() {
            saw_artifact_ref = true;
            render_prefixed_id_row(ui, "artifact ref", artifact_ref);
        }
    }
    if !saw_artifact_ref {
        kv(ui, "artifact ref", "not_recorded");
    }

    let mut saw_tree_key = false;
    if let Some(source) = first_artifact_source(graph, sources) {
        for tree_key in source.tree_keys() {
            saw_tree_key = true;
            render_prefixed_id_row(ui, "tree key", tree_key);
        }
    }
    if !saw_tree_key {
        kv(ui, "tree key", "not_recorded");
    }
}

fn render_unavailable(ui: &mut egui::Ui, reason: UnavailableReason) {
    kv(ui, reason.subject(), reason.state());
}

fn render_fixed_id_row(
    ui: &mut egui::Ui,
    key: &str,
    id_source: impl std::hash::Hash,
    id: &impl id_display::InteractiveId,
) {
    ui.horizontal(|ui| {
        ui.label(key);
        id.show_compact(ui, id_source);
    });
}

fn render_prefixed_id_row(
    ui: &mut egui::Ui,
    fallback_key: &str,
    id: &impl id_display::InteractiveId,
) {
    let full = id.full_id();
    let key = id.id_prefix().unwrap_or(fallback_key);
    render_fixed_id_row(
        ui,
        key,
        ("artifact-ids", key, full),
        id.trace_artifact_id_row(fallback_key, key),
    );
}

#[cfg(all(test, feature = "native-benchmark"))]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tracing::field::{Field, Visit};
    use tracing::{Event, Id, Subscriber};
    use tracing_subscriber::layer::{Context, SubscriberExt};
    use tracing_subscriber::registry::LookupSpan;
    use tracing_subscriber::{Layer, Registry};

    use crate::benchmark::{STANDARD_RUN_ROOT, StartupProfile, load_graph_with_startup_profile};
    use crate::ui::diff::PatchDiffCache;
    use crate::ui::inspector::{
        GraphRevision, InspectorCache, SelectionInspector, default_selections,
    };
    use crate::ui::view::GraphSelectionRef;
    use ploke_records::history::{ArtifactRefRecord, TreeKeyHashRecord};
    use ploke_records::ids::{ArtifactId, HistoryHash};
    use ploke_tree::graph::{
        ArtifactIdentity, ArtifactIds, ArtifactIndex, ArtifactKey, ArtifactNode,
    };
    use ploke_tree::{
        AuthorityLabel, Diagnostic, EvidenceRef, Lanes, NodeKey, NodeKind, PassiveEvidence, Phase,
        Progress, ResultClass, RunForest, Terminality, TreeNode,
    };

    #[derive(Clone, Default)]
    struct TraceLines(Arc<Mutex<Vec<String>>>);

    impl TraceLines {
        fn push(&self, line: String) {
            self.0.lock().expect("trace lock").push(line);
        }

        fn snapshot(&self) -> Vec<String> {
            self.0.lock().expect("trace lock").clone()
        }
    }

    #[derive(Default)]
    struct TraceFields {
        values: Vec<String>,
    }

    impl TraceFields {
        fn push(&mut self, field: &Field, value: impl Into<String>) {
            self.values
                .push(format!("{}={}", field.name(), value.into()));
        }

        fn finish(self) -> String {
            self.values.join(" ")
        }
    }

    impl Visit for TraceFields {
        fn record_bool(&mut self, field: &Field, value: bool) {
            self.push(field, value.to_string());
        }

        fn record_str(&mut self, field: &Field, value: &str) {
            self.push(field, value.to_string());
        }

        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.push(field, format!("{value:?}"));
        }
    }

    struct TraceLayer {
        lines: TraceLines,
    }

    impl<S> Layer<S> for TraceLayer
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        fn on_new_span(
            &self,
            attrs: &tracing::span::Attributes<'_>,
            _id: &Id,
            _ctx: Context<'_, S>,
        ) {
            let mut fields = TraceFields::default();
            attrs.record(&mut fields);
            self.lines.push(format!(
                "span:{} {}",
                attrs.metadata().name(),
                fields.finish()
            ));
        }

        fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
            let mut fields = TraceFields::default();
            event.record(&mut fields);
            self.lines.push(format!(
                "event:{} {}",
                event.metadata().target(),
                fields.finish()
            ));
        }
    }

    fn collect_traces<T>(f: impl FnOnce() -> T) -> (T, Vec<String>) {
        let lines = TraceLines::default();
        let subscriber = Registry::default().with(TraceLayer {
            lines: lines.clone(),
        });
        let output = tracing::subscriber::with_default(subscriber, f);
        (output, lines.snapshot())
    }

    fn test_run_forest_node(key: &str, source_artifact: &str) -> TreeNode {
        TreeNode {
            key: NodeKey::from(key),
            kind: NodeKind::SchedulerSearchNode,
            authority: AuthorityLabel::MutableProjection,
            parent: None,
            children: Vec::new(),
            generation: 0,
            branch_id: "branch".to_owned(),
            parent_branch_id: None,
            candidate_id: key.to_owned(),
            instance_id: "instance".to_owned(),
            source_state_id: source_artifact.to_owned(),
            target_relpath: ".ploke/prototype1/parent_identity.json".to_owned(),
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            progress: Progress {
                phase: Phase::Running,
                terminality: Terminality::NonTerminal,
                result_class: ResultClass::Unknown,
            },
            created_at: "2026-05-15T00:00:00Z".to_owned(),
            updated_at: "2026-05-15T00:00:00Z".to_owned(),
            evidence: vec![EvidenceRef {
                kind: ploke_tree::EvidenceKind::SchedulerNode,
                authority: AuthorityLabel::MutableProjection,
                node_key: Some(NodeKey::from(key)),
                runtime_id: None,
                recorded_at: Some("2026-05-15T00:00:00Z".to_owned()),
                detail: None,
            }],
            diagnostics: Vec::<Diagnostic>::new(),
        }
    }

    #[test]
    fn artifact_id_section_traces_expected_compact_rows() {
        let history_ref = ArtifactRefRecord::from_artifact_id(ArtifactId(
            "artifact:git-commit:deadbeefcafebabe".to_owned(),
        ));
        let artifact_id =
            ArtifactId("text-file-sha256:f6f73d0a2259c38d377144ed14f53be3".to_owned());
        let tree_key = TreeKeyHashRecord {
            hash: HistoryHash("tree:abcdef0123456789fedcba".to_owned()),
        };
        let node = ArtifactNode {
            key: ArtifactKey::HistoryRef {
                id: history_ref.id().0.clone(),
            },
            identity: ArtifactIdentity::HistoryRef(history_ref.clone()),
            ids: ArtifactIds {
                artifact_ids: vec![artifact_id.clone()],
                artifact_refs: vec![history_ref.clone()],
                tree_keys: vec![tree_key.clone()],
            },
            evidence: Vec::new(),
        };
        let selection = GraphSelectionRef::Artifact {
            key: node.entity_key().to_owned(),
        };
        let graph = Graph {
            artifacts: ArtifactIndex {
                artifacts: BTreeMap::from([(node.key.clone(), node)]),
            },
            ..Default::default()
        };
        let mut cache = InspectorCache::default();
        let sections = cache
            .sections(&graph, GraphRevision::default(), Some(&selection))
            .expect("artifact selection cached");
        let mut diff_cache = PatchDiffCache::default();

        let (_, traces) = collect_traces(|| {
            egui::__run_test_ui(|ui| {
                render_right_inspector(
                    ui,
                    &graph,
                    Some("artifact"),
                    Some("A1"),
                    Some(sections),
                    &mut diff_cache,
                    InspectorOpenState::default(),
                );
                render_artifact_ids_for_inspector(ui, &graph, sections);
            });
        });

        assert!(traces.iter().any(|line| {
            line.contains("span:ploke_egui.inspector.artifact_ids_section")
                && line.contains("selection_kind=artifact")
                && line.contains("state=rendered")
        }));
        assert!(traces.iter().any(|line| line.contains(
            "span:ploke_egui.inspector.render_artifact_ids primary_artifact_id=text-file-sha256:f6f73d0a2259c38d377144ed14f53be3"
        )));
        assert!(traces.iter().any(|line| {
            line.contains(
                "span:ploke_egui.inspector.render_artifact_id_row slot=artifact id label=text-file-sha256"
            ) && line.contains("compact=f6f73d0a")
                && line.contains("expandable=true")
        }));
        assert!(traces.iter().any(|line| line.contains(
            "span:ploke_egui.inspector.render_artifact_id_row slot=artifact ref label=artifact:git-commit"
        ) && line.contains("compact=deadbeef")
            && line.contains("expandable=true")));
        assert!(traces.iter().any(|line| {
            line.contains(
                "span:ploke_egui.inspector.render_artifact_id_row slot=tree key label=tree",
            ) && line.contains("compact=abcdef01")
                && line.contains("expandable=true")
        }));
        assert!(traces.iter().any(|line| line.contains(
            "span:ploke_egui.id_display.show_compact full=text-file-sha256:f6f73d0a2259c38d377144ed14f53be3 compact=f6f73d0a expandable=true expanded=false"
        )));
    }

    #[test]
    fn benchmark_inspector_open_state_forces_target_section() {
        let state = InspectorOpenState::benchmark(Some(
            crate::benchmark::BenchmarkInspectorSection::GraphEdges,
        ));

        assert_eq!(state.open(InspectorPanelSection::GraphEdges), Some(true));
        assert_eq!(state.open(InspectorPanelSection::RunRecords), None);
        assert_eq!(state.open(InspectorPanelSection::PatchDebug), None);
    }

    #[test]
    fn patch_debug_diff_scroll_areas_have_unique_ids() {
        let run_root = Path::new(STANDARD_RUN_ROOT);
        assert!(
            run_root.join("scheduler.json").is_file(),
            "standard benchmark run root missing: {}",
            run_root.display()
        );
        let (graph, startup) = load_graph_with_startup_profile(run_root, StartupProfile::default())
            .expect("load standard benchmark graph from typed records");
        assert!(
            !startup.compressed_run_records.is_empty(),
            "standard benchmark should deserialize compressed run records"
        );

        let selection = default_selections(&graph)
            .into_iter()
            .find(|selection| {
                matches!(
                    SelectionInspector::from_graph(&graph, selection),
                    SelectionInspector::Artifact(artifact) if artifact.patches.len() >= 2
                )
            })
            .expect("standard benchmark graph should contain an artifact with multiple patches");
        let mut inspector_cache = InspectorCache::default();
        let sections = inspector_cache
            .sections(&graph, GraphRevision::default(), Some(&selection.reference))
            .expect("real benchmark artifact selection has inspector sections");
        assert!(
            sections.patches().len() >= 2,
            "real benchmark selection should render multiple patch diffs"
        );
        let mut diff_cache = PatchDiffCache::default();
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::empty());

        let output = ctx.run_ui(Default::default(), |ui| {
            render_patches_for_inspector(ui, &graph, sections, &mut diff_cache);
        });

        let warning_texts: Vec<_> = clipped_shape_texts(&output.shapes)
            .into_iter()
            .filter(|text| text.contains("ScrollArea ID"))
            .collect();
        assert!(
            warning_texts.is_empty(),
            "unexpected egui ScrollArea ID clash warnings: {warning_texts:?}"
        );
    }

    #[test]
    fn artifact_id_section_traces_not_applicable_for_run_forest_selection() {
        let node = test_run_forest_node("node-f1fbab3a2bb5e7e5", "artifact:source");
        let selection = GraphSelectionRef::RunForestNode {
            key: node.key.as_str().to_owned(),
        };
        let graph = Graph {
            forest: Some(RunForest {
                campaign: ploke_tree::CampaignRef {
                    campaign_id: "campaign".to_owned(),
                    updated_at: "now".to_owned(),
                },
                roots: vec![node.key.clone()],
                nodes: vec![node],
                lanes: Lanes {
                    frontier: Vec::new(),
                    completed: Vec::new(),
                    failed: Vec::new(),
                },
                passive_evidence: PassiveEvidence::default(),
                diagnostics: Vec::new(),
            }),
            ..Default::default()
        };
        let mut cache = InspectorCache::default();
        let sections = cache
            .sections(&graph, GraphRevision::default(), Some(&selection))
            .expect("run forest selection cached");

        let (_, traces) = collect_traces(|| {
            egui::__run_test_ui(|ui| {
                render_artifact_ids_for_inspector(ui, &graph, sections);
            });
        });

        assert!(traces.iter().any(|line| line.contains(
            "span:ploke_egui.inspector.artifact_ids_section selection_kind=run_forest_node state=not_applicable selection_key=node-f1fbab3a2bb5e7e5"
        )));
    }

    fn clipped_shape_texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        let mut texts = Vec::new();
        for shape in shapes {
            collect_shape_texts(&shape.shape, &mut texts);
        }
        texts
    }

    fn collect_shape_texts(shape: &egui::epaint::Shape, texts: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(text) => texts.push(text.galley.text().to_owned()),
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_shape_texts(shape, texts);
                }
            }
            _ => {}
        }
    }
}

fn render_parent_create(ui: &mut egui::Ui, lookup: ParentCreateLookup<'_, '_>) {
    match lookup {
        ParentCreateLookup::Attempt(attempt) => render_parent_create_attempt(ui, attempt),
        ParentCreateLookup::Unavailable(reason) => {
            kv(ui, "attempt", "missing");
            render_parent_create_unavailable(ui, reason);
        }
        ParentCreateLookup::Ambiguous { count, reason } => {
            kv(ui, "attempt", "ambiguous");
            ui.horizontal(|ui| {
                ui.label("matches");
                ui.monospace(count.to_string());
            });
            render_parent_create_unavailable(ui, reason);
        }
    }
}

fn render_parent_create_attempt(ui: &mut egui::Ui, attempt: ParentCreateAttempt<'_>) {
    let child = attempt.child();
    let surface = attempt.surface();
    let summary = agent_turn_summary(attempt);
    let (surface_producer, router_model) = match attempt.surface_producer() {
        Some(ploke_records::history::SurfaceProposalProducerRecord::NonRouter) => {
            (Some("non_router"), None)
        }
        Some(ploke_records::history::SurfaceProposalProducerRecord::Router { request_policy }) => {
            (Some("router"), Some(request_policy.model.value.as_str()))
        }
        None => (None, None),
    };

    kv(ui, "attempt", "available");
    kv(
        ui,
        "target",
        child
            .request
            .target_relpath
            .to_str()
            .unwrap_or("non_utf8_path"),
    );
    if let Some(producer) = surface_producer {
        kv(ui, "surface", producer);
    }
    if let Some(touched_files) = surface.map(|surface| surface.touches.len()) {
        ui.horizontal(|ui| {
            ui.label("surface touches");
            ui.monospace(touched_files.to_string());
        });
    }
    if let Some(surface) = surface {
        kv(
            ui,
            "check/apply",
            &format!(
                "{}/{}",
                crate::ui::inspector::surface_check_status_label(surface.check_status),
                crate::ui::inspector::surface_apply_status_label(surface.apply_status)
            ),
        );
    }
    if let Some(model) = router_model {
        kv(ui, "model", model);
    } else if surface_producer == Some("non_router") {
        kv(ui, "model", "not_applicable");
    }
    ui.horizontal(|ui| {
        ui.label("tools");
        ui.monospace(format!(
            "{} requested, {} completed, {} failed",
            summary.tool_requested, summary.tool_completed, summary.tool_failed
        ));
    });
    ui.horizontal(|ui| {
        ui.label("llm proposal");
        ui.monospace(format!(
            "{} edits, {} creates, {} files",
            summary.edit_proposals, summary.create_proposals, summary.expected_file_changes
        ));
    });
    ui.horizontal(|ui| {
        ui.label("child eval");
        ui.monospace(format!(
            "{} evidence refs",
            attempt.candidate_evaluation_count()
        ));
    });

    egui::CollapsingHeader::new("LLM calls")
        .default_open(false)
        .show(ui, |ui| render_agent_turns(ui, attempt.agent_turns()));
    egui::CollapsingHeader::new("Source status")
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("child");
                id_display::expandable_id(
                    ui,
                    ("parent-create-child", child.node.node_id.as_str()),
                    child.node.node_id.as_str(),
                );
            });
            ui.horizontal(|ui| {
                ui.label("branch");
                id_display::expandable_id(
                    ui,
                    (
                        "parent-create-branch",
                        child.resolved.branch.branch_id.as_str(),
                    ),
                    child.resolved.branch.branch_id.as_str(),
                );
            });
            ui.horizontal(|ui| {
                ui.label("candidate");
                id_display::expandable_id(
                    ui,
                    (
                        "parent-create-candidate",
                        child.resolved.branch.candidate_id.as_str(),
                    ),
                    child.resolved.branch.candidate_id.as_str(),
                );
            });
            ui.horizontal(|ui| {
                ui.label("record refs");
                ui.monospace(attempt.source_ref_count().to_string());
            });
        });
}

fn render_parent_create_unavailable(
    ui: &mut egui::Ui,
    reason: ploke_tree::graph::ParentCreateUnavailable<'_>,
) {
    let (record, key, value) = match reason {
        ploke_tree::graph::ParentCreateUnavailable::MissingJoin { record, key, value }
        | ploke_tree::graph::ParentCreateUnavailable::AmbiguousJoin { record, key, value } => {
            (record, key, value)
        }
    };

    ui.horizontal(|ui| {
        ui.label(record);
        ui.monospace(key);
        id_display::expandable_id(ui, ("parent-create-unavailable", record, key, value), value);
    });
}

#[derive(Default)]
struct AgentTurnSummary {
    tool_requested: usize,
    tool_completed: usize,
    tool_failed: usize,
    edit_proposals: usize,
    create_proposals: usize,
    expected_file_changes: usize,
}

fn agent_turn_summary(attempt: ParentCreateAttempt<'_>) -> AgentTurnSummary {
    let mut summary = AgentTurnSummary::default();
    for turn in attempt.agent_turns() {
        summary.tool_requested += turn.tool_request_event_count;
        summary.tool_completed += turn.tool_completed_event_count;
        summary.tool_failed += turn.tool_failed_event_count;
        summary.edit_proposals += turn.edit_proposal_count;
        summary.create_proposals += turn.create_proposal_count;
        summary.expected_file_changes += turn.expected_file_change_count;
    }
    summary
}

fn render_agent_turns<'a>(
    ui: &mut egui::Ui,
    turns: impl IntoIterator<Item = &'a AgentTurnArtifactMetadata>,
) {
    let mut rendered = false;
    for (index, turn) in turns.into_iter().enumerate() {
        rendered = true;
        egui::CollapsingHeader::new(format!("turn {}", index + 1))
            .default_open(index == 0)
            .show(ui, |ui| {
                kv(ui, "model", turn.selected_model.as_str());
                if let Some(outcome) = turn.terminal_outcome.as_deref() {
                    kv(ui, "outcome", outcome);
                }
                ui.horizontal(|ui| {
                    ui.label("events");
                    ui.monospace(turn.event_count.to_string());
                });
                ui.horizontal(|ui| {
                    ui.label("prompt messages");
                    ui.monospace(turn.llm_prompt_message_count.to_string());
                });
                ui.horizontal(|ui| {
                    ui.label("tools");
                    ui.monospace(format!(
                        "{} requested, {} completed, {} failed",
                        turn.tool_request_event_count,
                        turn.tool_completed_event_count,
                        turn.tool_failed_event_count
                    ));
                });
                ui.horizontal(|ui| {
                    ui.label("patch");
                    ui.monospace(if turn.patch_applied {
                        "applied"
                    } else {
                        "not_applied"
                    });
                });
                id_display::expandable_id(
                    ui,
                    ("agent-turn", turn.task_id.as_str()),
                    turn.task_id.as_str(),
                );
            });
    }
    if !rendered {
        kv(ui, "llm", "not_recorded");
    }
}

fn render_edges<'a>(
    ui: &mut egui::Ui,
    direction: &str,
    edges: impl IntoIterator<Item = SelectionEdge<'a>>,
) {
    let mut rendered = false;
    for edge in edges {
        rendered = true;
        ui.horizontal(|ui| {
            ui.label(direction);
            ui.monospace(edge.relation.label());
            id_display::expandable_id(
                ui,
                ("edge-from", direction, edge.relation.label(), edge.from),
                edge.from,
            );
            ui.label("->");
            id_display::expandable_id(
                ui,
                ("edge-to", direction, edge.relation.label(), edge.to),
                edge.to,
            );
            ui.monospace(format!("({})", edge.source_count));
        });
    }
    if !rendered {
        kv(ui, direction, "none");
    }
}

fn render_source_refs<'a>(ui: &mut egui::Ui, source_refs: impl IntoIterator<Item = SourceRef<'a>>) {
    let mut total = 0;
    for source_ref in source_refs {
        if total < 8 {
            render_source_ref(ui, &source_ref);
        }
        total += 1;
    }
    if total == 0 {
        kv(ui, "record refs", "none");
        return;
    }
    if total > 8 {
        kv(ui, "more", &format!("{}", total - 8));
    }
}

fn render_source_ref(ui: &mut egui::Ui, source_ref: &SourceRef<'_>) {
    match source_ref {
        SourceRef::Evidence {
            kind,
            authority,
            recorded_at,
        } => {
            ui.horizontal(|ui| {
                ui.monospace(*kind);
                ui.monospace(*authority);
                if let Some(recorded_at) = recorded_at {
                    id_display::expandable_id(
                        ui,
                        ("source-ref", kind, authority, recorded_at),
                        recorded_at,
                    );
                }
            });
        }
        SourceRef::Diagnostic { severity, code } => {
            ui.horizontal(|ui| {
                ui.monospace("diagnostic");
                ui.monospace(*severity);
                id_display::expandable_id(ui, ("source-ref", severity, code), code);
            });
        }
        SourceRef::ArtifactHistoryRef { artifact } => {
            ui.horizontal(|ui| {
                ui.monospace("artifact_history_ref");
                id_display::expandable_id(ui, ("source-ref", artifact), artifact);
            });
        }
        SourceRef::ArtifactId { artifact } => {
            ui.horizontal(|ui| {
                ui.monospace("artifact_id");
                id_display::expandable_id(ui, ("source-ref", artifact), artifact);
            });
        }
    }
}

fn render_patches<'a>(
    ui: &mut egui::Ui,
    patches: impl IntoIterator<Item = PatchInspection<'a>>,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
) {
    let mut rendered = false;
    for patch in patches {
        rendered = true;
        render_patch(ui, patch, diff_cache);
    }
    if !rendered {
        kv(ui, "patch", "not_available");
    }
}

fn render_patch(
    ui: &mut egui::Ui,
    patch: PatchInspection<'_>,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
) {
    ui.horizontal(|ui| {
        ui.label("patch");
        id_display::expandable_id(ui, ("patch", patch.patch_id()), patch.patch_id());
    });
    kv(ui, "target", patch.target_relpath());
    kv(ui, "branch", patch.branch_id());
    kv(ui, "candidate", patch.candidate_id());
    kv(ui, "source hash", patch.source_content_hash());
    kv(ui, "proposed hash", patch.proposed_content_hash());
    if let Some(base) = patch.base_artifact() {
        kv(ui, "base artifact", base);
    }
    if let Some(derived) = patch.derived_artifact() {
        kv(ui, "derived artifact", derived);
    }
    if let Some(check) = patch.check_status() {
        kv(ui, "check", surface_check_status_label(check));
    }
    if let Some(apply) = patch.apply_status() {
        kv(ui, "apply", surface_apply_status_label(apply));
    }

    let mut touched = false;
    for touch in patch.touches() {
        touched = true;
        ui.label(format!(
            "touch {} {}:{}-{}",
            touch.index, touch.relpath, touch.start, touch.end
        ));
        ui.add(egui::Label::new(egui::RichText::new(touch.replacement).monospace()).wrap());
    }
    if !touched {
        kv(ui, "touches", "none");
    }
    ui.label("diff");
    render_diff(ui, patch, diff_cache);
}

fn render_diff(
    ui: &mut egui::Ui,
    patch: PatchInspection<'_>,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
) {
    let width = ui.available_width().max(240.0);
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .id_salt((
                    "ploke_egui.patch_debug.diff",
                    patch.child.node.node_id.as_str(),
                ))
                .auto_shrink([false, false])
                .max_height(320.0)
                .show(ui, |ui| {
                    let job = diff_cache.highlighted_patch_job(ui, patch);
                    ui.set_min_width(width);
                    ui.add(egui::Label::new(job.clone()).selectable(true));
                });
        });
}

fn first_artifact_source<'g>(
    graph: &'g Graph,
    sources: &[ArtifactSourceSlot],
) -> Option<&'g ploke_tree::graph::ArtifactNode> {
    sources.iter().find_map(|source| source.resolve(graph))
}

fn primary_artifact_id<'g>(
    graph: &'g Graph,
    sources: &[ArtifactSourceSlot],
) -> Option<&'g ploke_records::ids::ArtifactId> {
    first_artifact_source(graph, sources).and_then(|source| source.artifact_ids().first())
}

fn primary_artifact_ref<'g>(
    graph: &'g Graph,
    sources: &[ArtifactSourceSlot],
) -> Option<&'g ploke_records::history::ArtifactRefRecord> {
    first_artifact_source(graph, sources).and_then(|source| source.artifact_refs().first())
}

fn artifact_label<'g>(graph: &'g Graph, sources: &[ArtifactSourceSlot]) -> &'g str {
    first_artifact_source(graph, sources)
        .map(crate::ui::inspector::artifact_node_label_for_render)
        .unwrap_or("missing_artifact_identity")
}

fn artifact_source_count(graph: &Graph, sources: &[ArtifactSourceSlot]) -> usize {
    sources
        .iter()
        .filter(|source| source.resolve(graph).is_some())
        .count()
}

fn artifact_evidence_count(graph: &Graph, sources: &[ArtifactSourceSlot]) -> usize {
    sources
        .iter()
        .filter_map(|source| source.resolve(graph))
        .map(|source| source.evidence.len())
        .sum()
}
