use crate::allocation::scope;
use crate::ui::id_display::{self, TraceId};
use crate::ui::inspector::{
    ArtifactSourceSlot, IdentitySlot, InspectorSections, MetricsSlot, RoleBadgeSet, SourceRef,
    find_run_forest_node, phase_label, result_class_label, run_forest_node_identity,
};
use eframe::egui;
use ploke_tree::Graph;

use super::fields::*;
use super::{InspectorRenderCache, render_unavailable};

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
            let badge_text = badge.to_badge_text(crate::ui::theme::tokens_from_ui(ui));
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

pub(crate) fn render_identity(
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
    cached_kv_path(ui, render_cache, "target", identity.target_relpath);
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

pub(crate) fn render_roles_and_metrics(
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

/// archaeology:lineage-authority
/// proof:docs/active/archaeology/ploke-tree-graph/lineage-authority.md
#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_lineage_authority")
)]
pub(crate) fn render_lineage_authority_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    let Some(slot) = sections.lineage_authority() else {
        kv(ui, "lineage authority", "not_available");
        return;
    };

    let mut rendered = false;
    for block in slot.blocks(graph) {
        rendered = true;
        ui.separator();
        cached_kv_id(ui, render_cache, "block hash", block.block_hash.0.as_str());
        cached_kv_u64(ui, render_cache, "height", block.block_height);
        cached_kv_id(ui, render_cache, "lineage", block.lineage_id.0.as_str());
        cached_kv_id(
            ui,
            render_cache,
            "active artifact",
            block.active_artifact.as_str(),
        );
        cached_kv_id(
            ui,
            render_cache,
            "successor artifact",
            block.selected_successor.artifact.as_str(),
        );
        cached_kv_id(ui, render_cache, "policy", block.policy_ref.value.as_str());
        cached_kv_id(
            ui,
            render_cache,
            "opening authority",
            opening_authority_label(&block.opening_authority),
        );
        cached_kv_id(
            ui,
            render_cache,
            "immutable surface",
            block.surface.immutable.root.hash.0.as_str(),
        );
        cached_kv_id(
            ui,
            render_cache,
            "mutated surface",
            block.surface.mutated.after.root.hash.0.as_str(),
        );
        cached_kv_id(
            ui,
            render_cache,
            "ambient surface",
            block.surface.ambient.after.root.hash.0.as_str(),
        );
        cached_kv_usize(ui, render_cache, "entries", block.entry_count);
    }
    if !rendered {
        kv(ui, "lineage authority", "not_available");
    }
}

fn opening_authority_label(authority: &ploke_tree::graph::OpeningAuthorityNode) -> &'static str {
    match authority {
        ploke_tree::graph::OpeningAuthorityNode::Genesis { .. } => "genesis",
        ploke_tree::graph::OpeningAuthorityNode::Predecessor { .. } => "predecessor",
    }
}

fn render_source_refs<'a>(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    source_refs: impl IntoIterator<Item = SourceRef<'a>>,
) {
    let _span = tracing::trace_span!(scope::INSPECTOR_SOURCE_REFS_ITER).entered();
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
    let _span = tracing::trace_span!(scope::INSPECTOR_SOURCE_REFS_ROW).entered();
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

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_source_refs")
)]
pub(crate) fn render_source_refs_for_inspector(
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
pub(crate) fn render_artifact_ids_for_inspector(
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
