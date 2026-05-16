//! Stable frame rendering for the operator graph UI.

use eframe::egui;

use crate::ui::diff;
use crate::ui::id_display;
use crate::ui::id_display::TraceId;
use crate::ui::inspector::{
    ArtifactInspection, PatchInspection, RunForestNodeInspection, RunRecordInspection,
    SelectionEdge, SelectionInspector, SourceRef, UnavailableReason, artifact_edges,
    artifact_metrics, artifact_source_refs, phase_label, result_class_label,
    run_forest_artifact_edges, run_forest_incoming_edges, run_forest_node_identity,
    run_forest_outgoing_edges, run_forest_source_refs, surface_apply_status_label,
    surface_check_status_label,
};
use crate::ui::text::decor::Badge;
use crate::ui::view::{GraphSelectionDetail, GraphViewDiagnostics, GraphViewMode};
use ploke_tree::graph::{AgentTurnArtifactMetadata, ParentCreateAttempt, ParentCreateLookup};
use std::path::Path;

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

pub(crate) fn render_right_inspector(
    ui: &mut egui::Ui,
    selection: Option<&GraphSelectionDetail>,
    inspector: Option<&SelectionInspector<'_>>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("Inspector");
            ui.separator();

            ui.label("Summary");
            if let Some(selection) = selection {
                kv(ui, "kind", selection.kind.as_str());
                kv(ui, "label", selection.label.as_str());
            } else {
                kv(ui, "selection", "not_applicable");
            }

            ui.separator();
            ui.label("Identity");
            if let Some(inspector) = inspector {
                render_identity(ui, inspector);
            } else {
                kv(ui, "record refs", "not_applicable");
            }

            ui.separator();
            ui.label("Roles");
            if let Some(inspector) = inspector {
                render_roles_and_metrics(ui, inspector);
            } else {
                kv(ui, "roles", "not_applicable");
            }

            ui.separator();
            ui.label("Patch Generation");
            if let Some(inspector) = inspector {
                render_parent_create_for_inspector(ui, inspector);
            } else {
                kv(ui, "attempt", "not_applicable");
            }

            ui.separator();
            egui::CollapsingHeader::new("Run Records")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_run_records_for_inspector(ui, inspector);
                    } else {
                        kv(ui, "run records", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Graph edges")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_graph_edges_for_inspector(ui, inspector);
                    } else {
                        kv(ui, "edges", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Artifact edges")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_artifact_edges_for_inspector(ui, inspector);
                    } else {
                        kv(ui, "artifact edges", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Patch Debug")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_patches_for_inspector(ui, inspector);
                    } else {
                        kv(ui, "patch", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Source refs")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_source_refs_for_inspector(ui, inspector);
                    } else {
                        kv(ui, "record refs", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Artifact Ids")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_artifact_ids_for_inspector(ui, inspector);
                    } else {
                        kv(ui, "artifact ids", "not_applicable");
                    }
                });
        });
}

pub(crate) fn render_bottom_timeline(
    ui: &mut egui::Ui,
    diagnostics: Option<&GraphViewDiagnostics>,
    selection: Option<&GraphSelectionDetail>,
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
            if selection.is_some() {
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

fn render_badges(ui: &mut egui::Ui, badges: &[Badge<'_>]) {
    if badges.is_empty() {
        kv(ui, "none", "not_applicable");
        return;
    }
    for badge in badges {
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

fn render_identity(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => render_run_forest_identity(ui, run),
        SelectionInspector::Artifact(artifact) => render_artifact_identity(ui, artifact),
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

fn render_run_forest_identity(ui: &mut egui::Ui, run: &RunForestNodeInspection<'_>) {
    let identity = run_forest_node_identity(run.node);
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
fn render_artifact_identity(ui: &mut egui::Ui, artifact: &ArtifactInspection<'_>) {
    let identity = artifact.identity();
    if let Some(artifact_id) = identity.primary_artifact_id() {
        render_fixed_id_row(
            ui,
            "artifact",
            ("artifact-primary", artifact_id.0.as_str()),
            artifact_id,
        );
        return;
    }

    if let Some(artifact_ref) = identity.artifact_refs().next() {
        render_fixed_id_row(
            ui,
            "artifact",
            ("artifact-ref-primary", artifact_ref.id().0.as_str()),
            artifact_ref,
        );
        return;
    }

    kv(ui, "artifact", identity.artifact());
}

fn render_roles_and_metrics(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => {
            render_badges(ui, &run.role_badges);
            render_run_forest_metrics(ui, run);
        }
        SelectionInspector::Artifact(artifact) => {
            render_badges(ui, &artifact.role_badges);
            render_artifact_metrics(ui, artifact);
        }
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

fn render_run_forest_metrics(ui: &mut egui::Ui, run: &RunForestNodeInspection<'_>) {
    ui.horizontal(|ui| {
        ui.label("generation");
        ui.monospace(run.node.generation.to_string());
    });
    ui.horizontal(|ui| {
        ui.label("child run forest nodes");
        ui.monospace(run.children.len().to_string());
    });
}

fn render_artifact_metrics(ui: &mut egui::Ui, artifact: &ArtifactInspection<'_>) {
    let metrics = artifact_metrics(&artifact.sources);
    ui.horizontal(|ui| {
        ui.label("source records");
        ui.monospace(metrics.source_records.to_string());
    });
    ui.horizontal(|ui| {
        ui.label("evidence refs");
        ui.monospace(metrics.evidence_refs.to_string());
    });
}

fn render_parent_create_for_inspector(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => render_parent_create(ui, run.parent_create),
        SelectionInspector::Artifact(artifact) => render_parent_create(ui, artifact.parent_create),
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

/// archaeology:run-record-branch-output
/// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
fn render_run_records_for_inspector(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => render_run_records(ui, run.run_records.iter()),
        SelectionInspector::Artifact(_) => kv(ui, "run records", "not_applicable"),
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
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

fn render_graph_edges_for_inspector(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => {
            render_edges(ui, "in", run_forest_incoming_edges(run));
            render_edges(ui, "out", run_forest_outgoing_edges(run));
        }
        SelectionInspector::Artifact(artifact) => {
            render_edges(ui, "in", artifact_edges(&artifact.incoming));
            render_edges(ui, "out", artifact_edges(&artifact.outgoing));
        }
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

fn render_artifact_edges_for_inspector(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => {
            render_edges(ui, "in", std::iter::empty());
            render_edges(ui, "out", run_forest_artifact_edges(run.node));
        }
        SelectionInspector::Artifact(artifact) => {
            render_edges(ui, "in", artifact_edges(&artifact.incoming));
            render_edges(ui, "out", artifact_edges(&artifact.outgoing));
        }
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

fn render_patches_for_inspector(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => render_patches(ui, run.patch.iter().copied()),
        SelectionInspector::Artifact(artifact) => {
            render_patches(ui, artifact.patches.iter().copied());
        }
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

fn render_source_refs_for_inspector(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => {
            render_source_refs(ui, run_forest_source_refs(run.node));
        }
        SelectionInspector::Artifact(artifact) => {
            render_source_refs(ui, artifact_source_refs(&artifact.sources));
        }
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

/// archaeology:artifact-identity
/// proof:docs/active/archaeology/ploke-tree-graph/artifact-identity.md
fn render_artifact_ids_for_inspector(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => {
            let _span = tracing::trace_span!(
                "ploke_egui.inspector.artifact_ids_section",
                selection_kind = "run_forest_node",
                state = "not_applicable",
                selection_key = run.node.key.as_str()
            )
            .entered();
            kv(ui, "artifact ids", "not_applicable");
        }
        SelectionInspector::Artifact(artifact) => {
            let _span = tracing::trace_span!(
                "ploke_egui.inspector.artifact_ids_section",
                selection_kind = "artifact",
                state = "rendered",
                selection_key = artifact.identity().artifact()
            )
            .entered();
            render_artifact_ids(ui, artifact);
        }
        SelectionInspector::Unresolved(reason) => {
            let _span = tracing::trace_span!(
                "ploke_egui.inspector.artifact_ids_section",
                selection_kind = "unresolved",
                state = reason.state(),
                selection_key = reason.subject()
            )
            .entered();
            render_unavailable(ui, *reason);
        }
    }
}

fn render_artifact_ids(ui: &mut egui::Ui, artifact: &ArtifactInspection<'_>) {
    let identity = artifact.identity();
    let _span = tracing::trace_span!(
        "ploke_egui.inspector.render_artifact_ids",
        primary_artifact_id = identity
            .primary_artifact_id()
            .map(|artifact| artifact.0.as_str())
            .unwrap_or("not_recorded")
    )
    .entered();

    let mut saw_artifact_id = false;
    for artifact_id in identity.artifact_ids() {
        saw_artifact_id = true;
        render_prefixed_id_row(ui, "artifact id", artifact_id);
    }
    if !saw_artifact_id {
        kv(ui, "artifact id", "not_recorded");
    }

    let mut saw_artifact_ref = false;
    for artifact_ref in identity.artifact_refs() {
        saw_artifact_ref = true;
        render_prefixed_id_row(ui, "artifact ref", artifact_ref);
    }
    if !saw_artifact_ref {
        kv(ui, "artifact ref", "not_recorded");
    }

    let mut saw_tree_key = false;
    for tree_key in identity.tree_keys() {
        saw_tree_key = true;
        render_prefixed_id_row(ui, "tree key", tree_key);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tracing::field::{Field, Visit};
    use tracing::{Event, Id, Subscriber};
    use tracing_subscriber::layer::{Context, SubscriberExt};
    use tracing_subscriber::registry::LookupSpan;
    use tracing_subscriber::{Layer, Registry};

    use crate::ui::inspector::{ArtifactInspection, SelectionInspector};
    use crate::ui::view::{GraphSelectionDetail, GraphSelectionRef};
    use ploke_records::history::{ArtifactRefRecord, TreeKeyHashRecord};
    use ploke_records::ids::{ArtifactId, HistoryHash};
    use ploke_tree::graph::{
        ArtifactIdentity, ArtifactIds, ArtifactKey, ArtifactNode, ParentCreateLookup,
        ParentCreateUnavailable,
    };
    use ploke_tree::{
        AuthorityLabel, Diagnostic, EvidenceRef, NodeKey, NodeKind, Phase, Progress, ResultClass,
        Terminality, TreeNode,
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
        let artifact = ArtifactInspection {
            sources: vec![&node],
            incoming: Vec::new(),
            outgoing: Vec::new(),
            patches: Vec::new(),
            parent_create: ParentCreateLookup::Unavailable(ParentCreateUnavailable::MissingJoin {
                record: "child_plan",
                key: "derived_artifact",
                value: node.entity_key(),
            }),
            role_badges: Vec::new(),
        };
        let selection = GraphSelectionDetail {
            kind: "artifact".to_owned(),
            label: "A1".to_owned(),
            detail: String::new(),
            reference: GraphSelectionRef::Artifact {
                key: node.entity_key().to_owned(),
            },
        };
        let inspector = SelectionInspector::Artifact(artifact);

        let (_, traces) = collect_traces(|| {
            egui::__run_test_ui(|ui| {
                render_right_inspector(ui, Some(&selection), Some(&inspector));
                render_artifact_ids_for_inspector(ui, &inspector);
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
    fn artifact_id_section_traces_not_applicable_for_run_forest_selection() {
        let node = test_run_forest_node("node-f1fbab3a2bb5e7e5", "artifact:source");
        let inspector = SelectionInspector::RunForestNode(RunForestNodeInspection {
            node: &node,
            parent: None,
            children: &node.children,
            patch: None,
            parent_create: ParentCreateLookup::Unavailable(ParentCreateUnavailable::MissingJoin {
                record: "child_plan",
                key: "node_id",
                value: node.key.as_str(),
            }),
            role_badges: Vec::new(),
            run_records: crate::ui::inspector::RunRecordBranchInspection::empty(),
        });

        let (_, traces) = collect_traces(|| {
            egui::__run_test_ui(|ui| {
                render_artifact_ids_for_inspector(ui, &inspector);
            });
        });

        assert!(traces.iter().any(|line| line.contains(
            "span:ploke_egui.inspector.artifact_ids_section selection_kind=run_forest_node state=not_applicable selection_key=node-f1fbab3a2bb5e7e5"
        )));
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

fn render_patches<'a>(ui: &mut egui::Ui, patches: impl IntoIterator<Item = PatchInspection<'a>>) {
    let mut rendered = false;
    for patch in patches {
        rendered = true;
        render_patch(ui, patch);
    }
    if !rendered {
        kv(ui, "patch", "not_available");
    }
}

fn render_patch(ui: &mut egui::Ui, patch: PatchInspection<'_>) {
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
    let diff = patch.unified_diff();
    if diff.is_empty() {
        kv(ui, "diff", "not_available");
    } else {
        ui.label("diff");
        render_diff(ui, diff.as_str());
    }
}

fn render_diff(ui: &mut egui::Ui, diff: &str) {
    let width = ui.available_width().max(240.0);
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .max_height(320.0)
                .show(ui, |ui| {
                    let job = diff::highlighted_diff_job(ui, diff, f32::INFINITY);
                    ui.set_min_width(width);
                    ui.add(egui::Label::new(job).selectable(true));
                });
        });
}
