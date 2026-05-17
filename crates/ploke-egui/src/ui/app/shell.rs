//! Stable frame rendering for the operator graph UI.

use eframe::egui;
use std::collections::BTreeMap;

use crate::ui::id_display;
use crate::ui::id_display::{InteractiveId, ShortId, TraceId};
use crate::ui::inspector::{
    ArtifactSourceSlot, IdentitySlot, InspectorSections, MetricsSlot, PatchInspection,
    RoleBadgeSet, RunRecordInspection, SelectionEdge, SourceRef, UnavailableReason,
    find_run_forest_node, phase_label, result_class_label, run_forest_node_identity,
    surface_apply_status_label, surface_check_status_label,
};
use crate::ui::view::{GraphViewDiagnostics, GraphViewMode};
use ploke_tree::Graph;
use ploke_tree::graph::{AgentTurnArtifactMetadata, ParentCreateAttempt, ParentCreateLookup};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct InspectorOpenState {
    force_open: Option<InspectorPanelSection>,
}

#[derive(Debug, Default)]
pub(crate) struct InspectorRenderCache {
    parent_create_rows: BTreeMap<ParentCreateRowsKey, ParentCreateRows>,
    parent_create_row_rebuilds: usize,
    text_galleys: Vec<CachedTextGalley>,
    id_galleys: Vec<CachedIdGalley>,
    text_galley_rebuilds: usize,
    id_galley_rebuilds: usize,
}

impl InspectorRenderCache {
    fn parent_create_rows(&mut self, key: ParentCreateRowsKey) -> ParentCreateRows {
        if !self.parent_create_rows.contains_key(&key) {
            self.parent_create_rows
                .insert(key, ParentCreateRows::from_key(key));
            self.parent_create_row_rebuilds += 1;
        }
        self.parent_create_rows
            .get(&key)
            .cloned()
            .expect("parent-create row cache populated")
    }

    #[cfg(test)]
    fn parent_create_row_rebuilds(&self) -> usize {
        self.parent_create_row_rebuilds
    }

    fn text_galley(
        &mut self,
        ui: &egui::Ui,
        text: &str,
        kind: CachedTextKind,
    ) -> Arc<egui::Galley> {
        let style_key = CachedTextStyleKey::from_ui(ui);
        if let Some(entry) = self.text_galleys.iter().find(|entry| {
            entry.kind == kind && entry.style_key == style_key && entry.text.as_ref() == text
        }) {
            return entry.galley.clone();
        }

        let galley = layout_cached_text(ui, text, kind);
        self.text_galleys.push(CachedTextGalley {
            kind,
            style_key,
            text: text.into(),
            galley: galley.clone(),
        });
        self.text_galley_rebuilds += 1;
        galley
    }

    fn id_galley(&mut self, ui: &egui::Ui, full: &str, expanded: bool) -> Arc<egui::Galley> {
        let style_key = CachedTextStyleKey::from_ui(ui);
        if let Some(entry) = self.id_galleys.iter().find(|entry| {
            entry.expanded == expanded
                && entry.style_key == style_key
                && entry.full.as_ref() == full
        }) {
            return entry.galley.clone();
        }

        let label = if expanded {
            full.to_owned()
        } else {
            ShortId::new(full)
                .map(|short| short.to_string())
                .unwrap_or_else(|| full.to_owned())
        };
        let galley = layout_cached_text(ui, label.as_str(), CachedTextKind::Monospace);
        self.id_galleys.push(CachedIdGalley {
            expanded,
            style_key,
            full: full.into(),
            galley: galley.clone(),
        });
        self.id_galley_rebuilds += 1;
        galley
    }

    #[cfg(test)]
    fn text_galley_rebuilds(&self) -> usize {
        self.text_galley_rebuilds
    }

    #[cfg(test)]
    fn id_galley_rebuilds(&self) -> usize {
        self.id_galley_rebuilds
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CachedTextKind {
    Plain,
    Monospace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CachedTextStyleKey {
    dark_mode: bool,
    pixels_per_point: u32,
}

impl CachedTextStyleKey {
    fn from_ui(ui: &egui::Ui) -> Self {
        Self {
            dark_mode: ui.visuals().dark_mode,
            pixels_per_point: ui.ctx().pixels_per_point().to_bits(),
        }
    }
}

#[derive(Debug, Clone)]
struct CachedTextGalley {
    kind: CachedTextKind,
    style_key: CachedTextStyleKey,
    text: Box<str>,
    galley: Arc<egui::Galley>,
}

#[derive(Debug, Clone)]
struct CachedIdGalley {
    expanded: bool,
    style_key: CachedTextStyleKey,
    full: Box<str>,
    galley: Arc<egui::Galley>,
}

fn layout_cached_text(ui: &egui::Ui, text: &str, kind: CachedTextKind) -> Arc<egui::Galley> {
    let style = ui.style();
    let text_style = match kind {
        CachedTextKind::Plain => egui::TextStyle::Body,
        CachedTextKind::Monospace => egui::TextStyle::Monospace,
    };
    let font_id = text_style.resolve(style);
    ui.fonts_mut(|fonts| fonts.layout_no_wrap(text.to_owned(), font_id, egui::Color32::PLACEHOLDER))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ParentCreateRowsKey {
    surface_touches: Option<usize>,
    check_status: Option<&'static str>,
    apply_status: Option<&'static str>,
    tool_requested: usize,
    tool_completed: usize,
    tool_failed: usize,
    edit_proposals: usize,
    create_proposals: usize,
    expected_file_changes: usize,
    candidate_evaluations: usize,
}

#[derive(Debug, Clone)]
struct ParentCreateRows {
    surface_touches: Option<Arc<str>>,
    check_apply: Option<Arc<str>>,
    tools: Arc<str>,
    llm_proposal: Arc<str>,
    child_eval: Arc<str>,
}

impl ParentCreateRows {
    fn from_key(key: ParentCreateRowsKey) -> Self {
        Self {
            surface_touches: key
                .surface_touches
                .map(|count| Arc::<str>::from(format!("{count}"))),
            check_apply: key
                .check_status
                .zip(key.apply_status)
                .map(|(check, apply)| Arc::<str>::from(format!("{check}/{apply}"))),
            tools: Arc::<str>::from(format!(
                "{} requested, {} completed, {} failed",
                key.tool_requested, key.tool_completed, key.tool_failed
            )),
            llm_proposal: Arc::<str>::from(format!(
                "{} edits, {} creates, {} files",
                key.edit_proposals, key.create_proposals, key.expected_file_changes
            )),
            child_eval: Arc::<str>::from(format!("{} evidence refs", key.candidate_evaluations)),
        }
    }
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
    render_cache: &mut InspectorRenderCache,
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
                cached_kv_id(ui, render_cache, "kind", kind);
                cached_kv_id(ui, render_cache, "label", label);
            } else {
                kv(ui, "selection", "not_applicable");
            }

            ui.separator();
            ui.label("Identity");
            if let Some(sections) = sections {
                render_identity(ui, graph, sections, render_cache);
            } else {
                kv(ui, "record refs", "not_applicable");
            }

            ui.separator();
            ui.label("Roles");
            if let Some(sections) = sections {
                render_roles_and_metrics(ui, graph, sections, render_cache);
            } else {
                kv(ui, "roles", "not_applicable");
            }

            ui.separator();
            ui.label("Patch Generation");
            if let Some(sections) = sections {
                render_parent_create_for_inspector(ui, graph, sections, render_cache);
            } else {
                kv(ui, "attempt", "not_applicable");
            }

            ui.separator();
            egui::CollapsingHeader::new("Run Records")
                .open(open_state.open(InspectorPanelSection::RunRecords))
                .show(ui, |ui| {
                    if let Some(sections) = sections {
                        render_run_records_for_inspector(ui, graph, sections, render_cache);
                    } else {
                        kv(ui, "run records", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Graph edges")
                .open(open_state.open(InspectorPanelSection::GraphEdges))
                .show(ui, |ui| {
                    if let Some(sections) = sections {
                        render_graph_edges_for_inspector(ui, sections, render_cache);
                    } else {
                        kv(ui, "edges", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Artifact edges")
                .open(open_state.open(InspectorPanelSection::ArtifactEdges))
                .show(ui, |ui| {
                    if let Some(sections) = sections {
                        render_artifact_edges_for_inspector(ui, sections, render_cache);
                    } else {
                        kv(ui, "artifact edges", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Patch Debug")
                .open(open_state.open(InspectorPanelSection::PatchDebug))
                .show(ui, |ui| {
                    if let Some(sections) = sections {
                        render_patches_for_inspector(ui, graph, sections, render_cache, diff_cache);
                    } else {
                        kv(ui, "patch", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Source refs")
                .open(open_state.open(InspectorPanelSection::SourceRefs))
                .show(ui, |ui| {
                    if let Some(sections) = sections {
                        render_source_refs_for_inspector(ui, graph, sections, render_cache);
                    } else {
                        kv(ui, "record refs", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Artifact Ids")
                .open(open_state.open(InspectorPanelSection::ArtifactIds))
                .show(ui, |ui| {
                    if let Some(sections) = sections {
                        render_artifact_ids_for_inspector(ui, graph, sections, render_cache);
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

fn cached_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, CachedTextKind::Plain);
    ui.add(egui::Label::new(galley))
}

fn cached_monospace_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, CachedTextKind::Monospace);
    ui.add(egui::Label::new(galley))
}

fn cached_expandable_id(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    full: &str,
) -> egui::Response {
    let Some(_) = ShortId::new(full) else {
        return cached_monospace_label(ui, render_cache, full);
    };

    let id = ui.make_persistent_id(("ploke-egui.short-id", id_source));
    let mut expanded = ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
    let galley = render_cache.id_galley(ui, full, expanded);
    let response = ui
        .add(egui::Label::new(galley).sense(egui::Sense::click()))
        .on_hover_text("Click to expand. Right click to copy the full id.");

    if response.clicked() {
        expanded = !expanded;
        ui.data_mut(|data| data.insert_temp(id, expanded));
    }

    response.context_menu(|ui| {
        if ui.button("Copy full id").clicked() {
            ui.ctx().copy_text(full.to_owned());
            ui.close();
        }
    });

    response
}

fn cached_compact_id(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    id: &impl InteractiveId,
) -> egui::Response {
    let full = id.full_id();
    let persistent_id = ui.make_persistent_id(("ploke-egui.compact-id", id_source));
    let mut expanded = ui.data(|data| data.get_temp::<bool>(persistent_id).unwrap_or(false));
    let (compact, expandable) = id.compact_label();
    let _span = tracing::trace_span!(
        "ploke_egui.id_display.show_compact",
        full = full,
        compact = compact,
        expandable = expandable,
        expanded = expanded
    )
    .entered();
    let label = if expanded { full } else { compact };
    let galley = render_cache.text_galley(ui, label, CachedTextKind::Monospace);
    let hint = if expanded {
        "Click to collapse. Right click to copy the full id."
    } else {
        "Click to expand. Right click to copy the full id."
    };
    let response = ui
        .add(egui::Label::new(galley).sense(egui::Sense::click()))
        .on_hover_ui(|ui| {
            ui.monospace(full);
            ui.label(hint);
        });

    if response.clicked() && expandable {
        expanded = !expanded;
        ui.data_mut(|data| data.insert_temp(persistent_id, expanded));
    }

    response.context_menu(|ui| {
        if ui.button("Copy full id").clicked() {
            ui.ctx().copy_text(full.to_owned());
            ui.close();
        }
    });

    response
}

fn cached_kv_id(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, key);
        cached_expandable_id(ui, render_cache, ("kv", key, value), value);
    });
}

fn cached_kv_usize(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: usize,
) {
    let mut buffer = itoa::Buffer::new();
    cached_kv_id(ui, render_cache, key, buffer.format(value));
}

fn cached_kv_text(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, key);
        cached_monospace_label(ui, render_cache, value);
    });
}

/// archaeology:runtime-role
/// proof:docs/active/archaeology/ploke-tree-graph/runtime-role.md
fn render_badges(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    badges: RoleBadgeSet<'_>,
) {
    if badges.is_empty() {
        kv(ui, "none", "not_applicable");
        return;
    }
    for badge in badges.badges() {
        ui.horizontal(|ui| {
            let badge_text = badge.to_badge_text();
            let artifact_id = badge_text.artifact_id();
            badge_text.show(ui);
            cached_expandable_id(
                ui,
                render_cache,
                ("role-badge", artifact_id.0.as_str()),
                artifact_id.0.as_str(),
            );
        });
    }
}

fn render_identity(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }

    match sections.identity() {
        Some(IdentitySlot::RunForestNode { node_key }) => {
            if let Some(node) = find_run_forest_node(graph, node_key) {
                render_run_forest_identity(ui, node, render_cache);
            } else {
                kv(ui, "run forest node", "not_found");
            }
        }
        Some(IdentitySlot::Artifact { sources }) => {
            render_artifact_identity(ui, graph, sources, render_cache)
        }
        None => kv(ui, "identity", "not_available"),
    }
}

fn render_run_forest_identity(
    ui: &mut egui::Ui,
    node: &ploke_tree::TreeNode,
    render_cache: &mut InspectorRenderCache,
) {
    let identity = run_forest_node_identity(node);
    cached_kv_id(ui, render_cache, "run forest node", identity.node_key);
    cached_kv_id(ui, render_cache, "candidate", identity.candidate_id);
    cached_kv_id(
        ui,
        render_cache,
        "source artifact",
        identity.source_artifact,
    );
    if let Some(parent) = identity.parent_node {
        cached_kv_id(ui, render_cache, "parent run forest node", parent);
    }
    // Artifact ids are still rendered as plain expandable ids here. The intended
    // UI is a progressive-discovery "Artifact Ids" drilldown that shows compact
    // prefix + short-hash forms first, expands to the full value on click, and
    // keeps copy affordances available for debugging. See
    // docs/active/archaeology/ploke-tree-graph/artifact-identity.md before
    // refactoring this into shared interaction behavior.
    if let Some(base) = identity.base_artifact {
        cached_kv_id(ui, render_cache, "base artifact", base);
    }
    if let Some(derived) = identity.derived_artifact {
        cached_kv_id(ui, render_cache, "derived artifact", derived);
    }
    if let Some(patch) = identity.patch {
        cached_kv_id(ui, render_cache, "patch", patch);
    }
    cached_kv_id(ui, render_cache, "branch", identity.branch_id);
    cached_kv_id(ui, render_cache, "target", identity.target_relpath);
    cached_kv_id(ui, render_cache, "phase", phase_label(identity.phase));
    cached_kv_id(
        ui,
        render_cache,
        "result",
        result_class_label(identity.result),
    );
}

/// archaeology:artifact-identity
/// proof:docs/active/archaeology/ploke-tree-graph/artifact-identity.md
fn render_artifact_identity(
    ui: &mut egui::Ui,
    graph: &Graph,
    sources: &[ArtifactSourceSlot],
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(artifact_id) = primary_artifact_id(graph, sources) {
        render_fixed_id_row(
            ui,
            render_cache,
            "artifact",
            ("artifact-primary", artifact_id.0.as_str()),
            artifact_id,
        );
        return;
    }

    if let Some(artifact_ref) = primary_artifact_ref(graph, sources) {
        render_fixed_id_row(
            ui,
            render_cache,
            "artifact",
            ("artifact-ref-primary", artifact_ref.id().0.as_str()),
            artifact_ref,
        );
        return;
    }

    cached_kv_id(ui, render_cache, "artifact", artifact_label(graph, sources));
}

fn render_roles_and_metrics(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }

    render_badges(ui, render_cache, sections.role_badges(graph));
    match sections.metrics() {
        Some(MetricsSlot::RunForestNode { node_key }) => {
            if let Some(node) = find_run_forest_node(graph, node_key) {
                render_run_forest_metrics(ui, node, render_cache);
            }
        }
        Some(MetricsSlot::Artifact { sources }) => {
            render_artifact_metrics(ui, graph, sources, render_cache)
        }
        None => {}
    }
}

fn render_run_forest_metrics(
    ui: &mut egui::Ui,
    node: &ploke_tree::TreeNode,
    render_cache: &mut InspectorRenderCache,
) {
    cached_kv_usize(ui, render_cache, "generation", node.generation as usize);
    cached_kv_usize(
        ui,
        render_cache,
        "child run forest nodes",
        node.children.len(),
    );
}

fn render_artifact_metrics(
    ui: &mut egui::Ui,
    graph: &Graph,
    sources: &[ArtifactSourceSlot],
    render_cache: &mut InspectorRenderCache,
) {
    cached_kv_usize(
        ui,
        render_cache,
        "source records",
        artifact_source_count(graph, sources),
    );
    cached_kv_usize(
        ui,
        render_cache,
        "evidence refs",
        artifact_evidence_count(graph, sources),
    );
}

fn render_parent_create_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    match sections.parent_create() {
        Some(slot) => render_parent_create(ui, slot.resolve(graph), render_cache),
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
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_run_records(
        ui,
        render_cache,
        sections
            .run_records()
            .iter()
            .filter_map(|slot| slot.resolve(graph)),
    );
}

fn render_run_records<'a>(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    records: impl IntoIterator<Item = RunRecordInspection<'a>>,
) {
    let mut rendered = false;
    for record in records {
        rendered = true;
        ui.separator();
        cached_kv_id(
            ui,
            render_cache,
            "arm",
            compared_run_arm_label(record.record_ref.arm),
        );
        cached_kv_id(
            ui,
            render_cache,
            "instance",
            record.record_ref.instance_id.as_str(),
        );
        cached_kv_id(
            ui,
            render_cache,
            "record",
            record
                .record_ref
                .record_path
                .to_str()
                .unwrap_or("non_utf8_path"),
        );
        cached_kv_id(
            ui,
            render_cache,
            "manifest",
            record.record.manifest_id.as_str(),
        );
        if let Some(model) = record.record.metadata.agent.model_id.as_deref() {
            cached_kv_id(ui, render_cache, "model", model);
        }
        if let Some(provider) = record.record.metadata.agent.provider.as_deref() {
            cached_kv_id(ui, render_cache, "provider", provider);
        }
        cached_kv_id(
            ui,
            render_cache,
            "repo root",
            record
                .record
                .metadata
                .benchmark
                .repo_root
                .to_str()
                .unwrap_or("non_utf8_path"),
        );
        cached_kv_usize(ui, render_cache, "turns", record.stats.turn_count);
        cached_kv_usize(ui, render_cache, "tool calls", record.stats.tool_call_count);
        cached_kv_usize(
            ui,
            render_cache,
            "failed tool calls",
            record.stats.failed_tool_call_count,
        );
        if let Some(packaging) = record.record.phases.packaging.as_ref() {
            cached_kv_id(
                ui,
                render_cache,
                "submission",
                submission_artifact_state_label(packaging.submission_artifact_state),
            );
            cached_kv_id(
                ui,
                render_cache,
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
fn render_graph_edges_for_inspector(
    ui: &mut egui::Ui,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_edges(
        ui,
        render_cache,
        "in",
        sections.graph_edges_in().iter().map(|slot| slot.edge()),
    );
    render_edges(
        ui,
        render_cache,
        "out",
        sections.graph_edges_out().iter().map(|slot| slot.edge()),
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_artifact_edges")
)]
fn render_artifact_edges_for_inspector(
    ui: &mut egui::Ui,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_edges(
        ui,
        render_cache,
        "in",
        sections.artifact_edges_in().iter().map(|slot| slot.edge()),
    );
    render_edges(
        ui,
        render_cache,
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
    render_cache: &mut InspectorRenderCache,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_patches(
        ui,
        render_cache,
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
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_source_refs(
        ui,
        render_cache,
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
    render_cache: &mut InspectorRenderCache,
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
            render_artifact_ids(ui, graph, sources, render_cache);
        }
        None => kv(ui, "artifact ids", "not_available"),
    }
}

fn render_artifact_ids(
    ui: &mut egui::Ui,
    graph: &Graph,
    sources: &[ArtifactSourceSlot],
    render_cache: &mut InspectorRenderCache,
) {
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
            render_prefixed_id_row(ui, render_cache, "artifact id", artifact_id);
        }
    }
    if !saw_artifact_id {
        kv(ui, "artifact id", "not_recorded");
    }

    let mut saw_artifact_ref = false;
    if let Some(source) = first_artifact_source(graph, sources) {
        for artifact_ref in source.artifact_refs() {
            saw_artifact_ref = true;
            render_prefixed_id_row(ui, render_cache, "artifact ref", artifact_ref);
        }
    }
    if !saw_artifact_ref {
        kv(ui, "artifact ref", "not_recorded");
    }

    let mut saw_tree_key = false;
    if let Some(source) = first_artifact_source(graph, sources) {
        for tree_key in source.tree_keys() {
            saw_tree_key = true;
            render_prefixed_id_row(ui, render_cache, "tree key", tree_key);
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
    render_cache: &mut InspectorRenderCache,
    key: &str,
    id_source: impl std::hash::Hash,
    id: &impl id_display::InteractiveId,
) {
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, key);
        cached_compact_id(ui, render_cache, id_source, id);
    });
}

fn render_prefixed_id_row(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    fallback_key: &str,
    id: &impl id_display::InteractiveId,
) {
    let full = id.full_id();
    let key = id.id_prefix().unwrap_or(fallback_key);
    render_fixed_id_row(
        ui,
        render_cache,
        key,
        ("artifact-ids", key, full),
        id.trace_artifact_id_row(fallback_key, key),
    );
}

#[cfg(test)]
mod render_cache_tests {
    use super::*;

    #[test]
    fn parent_create_render_rows_reuse_allocated_text_for_stable_key() {
        let key = ParentCreateRowsKey {
            surface_touches: Some(2),
            check_status: Some("passed"),
            apply_status: Some("applied"),
            tool_requested: 3,
            tool_completed: 2,
            tool_failed: 1,
            edit_proposals: 4,
            create_proposals: 1,
            expected_file_changes: 5,
            candidate_evaluations: 6,
        };
        let mut cache = InspectorRenderCache::default();

        let first_tools = Arc::as_ptr(&cache.parent_create_rows(key).tools);
        assert_eq!(cache.parent_create_row_rebuilds(), 1);

        let second_tools = Arc::as_ptr(&cache.parent_create_rows(key).tools);
        assert_eq!(cache.parent_create_row_rebuilds(), 1);
        assert_eq!(first_tools, second_tools);

        let mut changed = key;
        changed.tool_completed += 1;
        let third_tools = Arc::as_ptr(&cache.parent_create_rows(changed).tools);
        assert_eq!(cache.parent_create_row_rebuilds(), 2);
        assert_ne!(first_tools, third_tools);
    }

    #[test]
    fn inspector_text_galley_cache_reuses_stable_labels() {
        let mut cache = InspectorRenderCache::default();

        egui::__run_test_ui(|ui| {
            let first = cache.text_galley(ui, "artifact", CachedTextKind::Monospace);
            assert_eq!(cache.text_galley_rebuilds(), 1);

            let second = cache.text_galley(ui, "artifact", CachedTextKind::Monospace);
            assert_eq!(cache.text_galley_rebuilds(), 1);
            assert!(Arc::ptr_eq(&first, &second));

            let plain = cache.text_galley(ui, "artifact", CachedTextKind::Plain);
            assert_eq!(cache.text_galley_rebuilds(), 2);
            assert!(!Arc::ptr_eq(&first, &plain));
        });
    }

    #[test]
    fn inspector_id_galley_cache_reuses_short_id_labels() {
        let mut cache = InspectorRenderCache::default();
        let id = "artifact:git-commit:deadbeefcafebabe";

        egui::__run_test_ui(|ui| {
            let first = cache.id_galley(ui, id, false);
            assert_eq!(cache.id_galley_rebuilds(), 1);

            let second = cache.id_galley(ui, id, false);
            assert_eq!(cache.id_galley_rebuilds(), 1);
            assert!(Arc::ptr_eq(&first, &second));

            let expanded = cache.id_galley(ui, id, true);
            assert_eq!(cache.id_galley_rebuilds(), 2);
            assert!(!Arc::ptr_eq(&first, &expanded));
        });
    }
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
        let mut render_cache = InspectorRenderCache::default();
        let mut diff_cache = PatchDiffCache::default();

        let (_, traces) = collect_traces(|| {
            egui::__run_test_ui(|ui| {
                render_right_inspector(
                    ui,
                    &graph,
                    Some("artifact"),
                    Some("A1"),
                    Some(sections),
                    &mut render_cache,
                    &mut diff_cache,
                    InspectorOpenState::default(),
                );
                render_artifact_ids_for_inspector(ui, &graph, sections, &mut render_cache);
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
        let mut render_cache = InspectorRenderCache::default();
        let mut diff_cache = PatchDiffCache::default();
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::empty());

        let output = ctx.run_ui(Default::default(), |ui| {
            render_patches_for_inspector(ui, &graph, sections, &mut render_cache, &mut diff_cache);
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
                let mut render_cache = InspectorRenderCache::default();
                render_artifact_ids_for_inspector(ui, &graph, sections, &mut render_cache);
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

fn render_parent_create(
    ui: &mut egui::Ui,
    lookup: ParentCreateLookup<'_, '_>,
    render_cache: &mut InspectorRenderCache,
) {
    match lookup {
        ParentCreateLookup::Attempt(attempt) => {
            render_parent_create_attempt(ui, attempt, render_cache);
        }
        ParentCreateLookup::Unavailable(reason) => {
            cached_kv_id(ui, render_cache, "attempt", "missing");
            render_parent_create_unavailable(ui, reason, render_cache);
        }
        ParentCreateLookup::Ambiguous { count, reason } => {
            cached_kv_id(ui, render_cache, "attempt", "ambiguous");
            cached_kv_usize(ui, render_cache, "matches", count);
            render_parent_create_unavailable(ui, reason, render_cache);
        }
    }
}

fn render_parent_create_attempt(
    ui: &mut egui::Ui,
    attempt: ParentCreateAttempt<'_>,
    render_cache: &mut InspectorRenderCache,
) {
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
    let rows = render_cache.parent_create_rows(ParentCreateRowsKey {
        surface_touches: surface.map(|surface| surface.touches.len()),
        check_status: surface.map(|surface| surface_check_status_label(surface.check_status)),
        apply_status: surface.map(|surface| surface_apply_status_label(surface.apply_status)),
        tool_requested: summary.tool_requested,
        tool_completed: summary.tool_completed,
        tool_failed: summary.tool_failed,
        edit_proposals: summary.edit_proposals,
        create_proposals: summary.create_proposals,
        expected_file_changes: summary.expected_file_changes,
        candidate_evaluations: attempt.candidate_evaluation_count(),
    });

    cached_kv_id(ui, render_cache, "attempt", "available");
    cached_kv_id(
        ui,
        render_cache,
        "target",
        child
            .request
            .target_relpath
            .to_str()
            .unwrap_or("non_utf8_path"),
    );
    if let Some(producer) = surface_producer {
        cached_kv_id(ui, render_cache, "surface", producer);
    }
    if let Some(touched_files) = rows.surface_touches.as_ref() {
        cached_kv_text(ui, render_cache, "surface touches", touched_files);
    }
    if let Some(check_apply) = rows.check_apply.as_ref() {
        cached_kv_text(ui, render_cache, "check/apply", check_apply);
    }
    if let Some(model) = router_model {
        cached_kv_id(ui, render_cache, "model", model);
    } else if surface_producer == Some("non_router") {
        cached_kv_id(ui, render_cache, "model", "not_applicable");
    }
    cached_kv_text(ui, render_cache, "tools", rows.tools.as_ref());
    cached_kv_text(ui, render_cache, "llm proposal", rows.llm_proposal.as_ref());
    cached_kv_text(ui, render_cache, "child eval", rows.child_eval.as_ref());

    egui::CollapsingHeader::new("LLM calls")
        .default_open(false)
        .show(ui, |ui| {
            render_agent_turns(ui, render_cache, attempt.agent_turns())
        });
    egui::CollapsingHeader::new("Source status")
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                cached_label(ui, render_cache, "child");
                cached_expandable_id(
                    ui,
                    render_cache,
                    ("parent-create-child", child.node.node_id.as_str()),
                    child.node.node_id.as_str(),
                );
            });
            ui.horizontal(|ui| {
                cached_label(ui, render_cache, "branch");
                cached_expandable_id(
                    ui,
                    render_cache,
                    (
                        "parent-create-branch",
                        child.resolved.branch.branch_id.as_str(),
                    ),
                    child.resolved.branch.branch_id.as_str(),
                );
            });
            ui.horizontal(|ui| {
                cached_label(ui, render_cache, "candidate");
                cached_expandable_id(
                    ui,
                    render_cache,
                    (
                        "parent-create-candidate",
                        child.resolved.branch.candidate_id.as_str(),
                    ),
                    child.resolved.branch.candidate_id.as_str(),
                );
            });
            cached_kv_usize(ui, render_cache, "record refs", attempt.source_ref_count());
        });
}

fn render_parent_create_unavailable(
    ui: &mut egui::Ui,
    reason: ploke_tree::graph::ParentCreateUnavailable<'_>,
    render_cache: &mut InspectorRenderCache,
) {
    let (record, key, value) = match reason {
        ploke_tree::graph::ParentCreateUnavailable::MissingJoin { record, key, value }
        | ploke_tree::graph::ParentCreateUnavailable::AmbiguousJoin { record, key, value } => {
            (record, key, value)
        }
    };

    ui.horizontal(|ui| {
        cached_label(ui, render_cache, record);
        cached_monospace_label(ui, render_cache, key);
        cached_expandable_id(
            ui,
            render_cache,
            ("parent-create-unavailable", record, key, value),
            value,
        );
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
    render_cache: &mut InspectorRenderCache,
    turns: impl IntoIterator<Item = &'a AgentTurnArtifactMetadata>,
) {
    let mut rendered = false;
    for (index, turn) in turns.into_iter().enumerate() {
        rendered = true;
        egui::CollapsingHeader::new(format!("turn {}", index + 1))
            .default_open(index == 0)
            .show(ui, |ui| {
                cached_kv_id(ui, render_cache, "model", turn.selected_model.as_str());
                if let Some(outcome) = turn.terminal_outcome.as_deref() {
                    cached_kv_id(ui, render_cache, "outcome", outcome);
                }
                cached_kv_usize(ui, render_cache, "events", turn.event_count);
                cached_kv_usize(
                    ui,
                    render_cache,
                    "prompt messages",
                    turn.llm_prompt_message_count,
                );
                render_agent_turn_tools(ui, render_cache, turn);
                cached_kv_id(
                    ui,
                    render_cache,
                    "patch",
                    if turn.patch_applied {
                        "applied"
                    } else {
                        "not_applied"
                    },
                );
                cached_expandable_id(
                    ui,
                    render_cache,
                    ("agent-turn", turn.task_id.as_str()),
                    turn.task_id.as_str(),
                );
            });
    }
    if !rendered {
        kv(ui, "llm", "not_recorded");
    }
}

fn render_agent_turn_tools(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turn: &AgentTurnArtifactMetadata,
) {
    let mut requested = itoa::Buffer::new();
    let mut completed = itoa::Buffer::new();
    let mut failed = itoa::Buffer::new();
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, "tools");
        cached_monospace_label(
            ui,
            render_cache,
            requested.format(turn.tool_request_event_count),
        );
        cached_monospace_label(ui, render_cache, "requested,");
        cached_monospace_label(
            ui,
            render_cache,
            completed.format(turn.tool_completed_event_count),
        );
        cached_monospace_label(ui, render_cache, "completed,");
        cached_monospace_label(
            ui,
            render_cache,
            failed.format(turn.tool_failed_event_count),
        );
        cached_monospace_label(ui, render_cache, "failed");
    });
}

fn render_edges<'a>(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    direction: &str,
    edges: impl IntoIterator<Item = SelectionEdge<'a>>,
) {
    let mut rendered = false;
    for edge in edges {
        rendered = true;
        ui.horizontal(|ui| {
            cached_label(ui, render_cache, direction);
            cached_monospace_label(ui, render_cache, edge.relation.label());
            cached_expandable_id(
                ui,
                render_cache,
                ("edge-from", direction, edge.relation.label(), edge.from),
                edge.from,
            );
            cached_label(ui, render_cache, "->");
            cached_expandable_id(
                ui,
                render_cache,
                ("edge-to", direction, edge.relation.label(), edge.to),
                edge.to,
            );
            render_count_parens(ui, render_cache, edge.source_count);
        });
    }
    if !rendered {
        kv(ui, direction, "none");
    }
}

fn render_source_refs<'a>(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    source_refs: impl IntoIterator<Item = SourceRef<'a>>,
) {
    let mut total = 0;
    for source_ref in source_refs {
        if total < 8 {
            render_source_ref(ui, render_cache, &source_ref);
        }
        total += 1;
    }
    if total == 0 {
        kv(ui, "record refs", "none");
        return;
    }
    if total > 8 {
        cached_kv_usize(ui, render_cache, "more", total - 8);
    }
}

fn render_source_ref(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    source_ref: &SourceRef<'_>,
) {
    match source_ref {
        SourceRef::Evidence {
            kind,
            authority,
            recorded_at,
        } => {
            ui.horizontal(|ui| {
                cached_monospace_label(ui, render_cache, kind);
                cached_monospace_label(ui, render_cache, authority);
                if let Some(recorded_at) = recorded_at {
                    cached_expandable_id(
                        ui,
                        render_cache,
                        ("source-ref", kind, authority, recorded_at),
                        recorded_at,
                    );
                }
            });
        }
        SourceRef::Diagnostic { severity, code } => {
            ui.horizontal(|ui| {
                cached_monospace_label(ui, render_cache, "diagnostic");
                cached_monospace_label(ui, render_cache, severity);
                cached_expandable_id(ui, render_cache, ("source-ref", severity, code), code);
            });
        }
        SourceRef::ArtifactHistoryRef { artifact } => {
            ui.horizontal(|ui| {
                cached_monospace_label(ui, render_cache, "artifact_history_ref");
                cached_expandable_id(ui, render_cache, ("source-ref", artifact), artifact);
            });
        }
        SourceRef::ArtifactId { artifact } => {
            ui.horizontal(|ui| {
                cached_monospace_label(ui, render_cache, "artifact_id");
                cached_expandable_id(ui, render_cache, ("source-ref", artifact), artifact);
            });
        }
    }
}

fn render_patches<'a>(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    patches: impl IntoIterator<Item = PatchInspection<'a>>,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
) {
    let mut rendered = false;
    for patch in patches {
        rendered = true;
        render_patch(ui, render_cache, patch, diff_cache);
    }
    if !rendered {
        kv(ui, "patch", "not_available");
    }
}

fn render_patch(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    patch: PatchInspection<'_>,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
) {
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, "patch");
        cached_expandable_id(
            ui,
            render_cache,
            ("patch", patch.patch_id()),
            patch.patch_id(),
        );
    });
    cached_label(ui, render_cache, "diff");
    render_diff(ui, patch, diff_cache);

    egui::CollapsingHeader::new("Details")
        .id_salt(("patch-details", patch.patch_id()))
        .default_open(false)
        .show(ui, |ui| {
            render_patch_details(ui, render_cache, patch);
        });
}

fn render_patch_details(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    patch: PatchInspection<'_>,
) {
    cached_kv_id(ui, render_cache, "target", patch.target_relpath());
    cached_kv_id(ui, render_cache, "branch", patch.branch_id());
    cached_kv_id(ui, render_cache, "candidate", patch.candidate_id());
    cached_kv_id(ui, render_cache, "source hash", patch.source_content_hash());
    cached_kv_id(
        ui,
        render_cache,
        "proposed hash",
        patch.proposed_content_hash(),
    );
    if let Some(base) = patch.base_artifact() {
        cached_kv_id(ui, render_cache, "base artifact", base);
    }
    if let Some(derived) = patch.derived_artifact() {
        cached_kv_id(ui, render_cache, "derived artifact", derived);
    }
    if let Some(check) = patch.check_status() {
        cached_kv_id(ui, render_cache, "check", surface_check_status_label(check));
    }
    if let Some(apply) = patch.apply_status() {
        cached_kv_id(ui, render_cache, "apply", surface_apply_status_label(apply));
    }

    let mut touched = false;
    for touch in patch.touches() {
        touched = true;
        render_patch_touch_label(ui, render_cache, touch);
        ui.add(egui::Label::new(egui::RichText::new(touch.replacement).monospace()).wrap());
    }
    if !touched {
        kv(ui, "touches", "none");
    }
}

fn render_count_parens(ui: &mut egui::Ui, render_cache: &mut InspectorRenderCache, count: usize) {
    let mut buffer = itoa::Buffer::new();
    ui.horizontal(|ui| {
        cached_monospace_label(ui, render_cache, "(");
        cached_monospace_label(ui, render_cache, buffer.format(count));
        cached_monospace_label(ui, render_cache, ")");
    });
}

fn render_patch_touch_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    touch: crate::ui::inspector::PatchTouch<'_>,
) {
    let mut index = itoa::Buffer::new();
    let mut start = itoa::Buffer::new();
    let mut end = itoa::Buffer::new();
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, "touch");
        cached_monospace_label(ui, render_cache, index.format(touch.index));
        cached_monospace_label(ui, render_cache, touch.relpath);
        cached_monospace_label(ui, render_cache, start.format(touch.start));
        cached_label(ui, render_cache, "-");
        cached_monospace_label(ui, render_cache, end.format(touch.end));
    });
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
                    let job = diff_cache.highlighted_patch_galley(ui, patch);
                    ui.set_min_width(width);
                    ui.add(egui::Label::new(job).selectable(true));
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
