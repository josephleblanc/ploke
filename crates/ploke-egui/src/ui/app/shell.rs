//! Stable frame rendering for the operator graph UI.

use crate::ui::render::text::*;
use eframe::egui;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::allocation::scope;
use crate::ui::app::layout::INSPECTOR_MARGIN_INNER;
use crate::ui::eval_protocol::{
    EvalProtocolDashboard, EvalProtocolVisualSummary, EvidenceState, PatchProjectionCounts,
};
use crate::ui::id_display;
use crate::ui::id_display::{
    CopyableArtifactFile, CopyableExpandable, CopyableId, CopyablePath, CopyableRunName,
    CopyableText, InteractiveId, ShortId, TraceId,
};
use crate::ui::inspector::{
    ArtifactSourceSlot, IdentitySlot, InspectorSections, MetricsSlot, PatchInspection,
    RoleBadgeSet, RunRecordInspection, RunRecordSlot, RunRecordTurnInspection, SelectionEdge,
    SourceRef, UnavailableReason, find_run_forest_node, phase_label, response_finish_reason_label,
    result_class_label, run_forest_node_identity, surface_apply_status_label,
    surface_check_status_label, tool_execution_name, tool_execution_status_label,
    tool_execution_summary, turn_outcome_elapsed_secs, turn_outcome_error, turn_outcome_label,
    turn_outcome_tool_count,
};
use crate::ui::text::style as text_style;
use ploke_records::protocol::ArtifactBody;
#[cfg(not(target_arch = "wasm32"))]
use ploke_records::tool_contracts::{
    PersistedToolCallArguments, PersistedToolResultContent, ToolCallArguments, ToolResultContent,
};
use ploke_tree::Graph;
use ploke_tree::graph::{AgentTurnArtifactMetadata, ParentCreateAttempt, ParentCreateLookup};
use std::sync::Arc;

mod cache;
mod chrome;
mod inspector;
pub(crate) use cache::InspectorRenderCache;
use cache::ParentCreateRowsKey;
pub(crate) use chrome::{
    add_inspector_scroll_end_padding, render_bottom_timeline, render_top_strip,
};
pub(crate) use inspector::{EvalProtocolRenderMode, InspectorOpenState, InspectorPanelSection};
#[cfg(test)]
mod render_cache_tests;

const CALL_REVIEW_SCAN_HOVER_PREVIEW_BYTES: usize = 220;

fn show_inspector_section_header(
    ui: &mut egui::Ui,
    title: &str,
    section: InspectorPanelSection,
    selection_ref: Option<&crate::ui::view::GraphSelectionRef>,
    actions: &mut Option<&mut Vec<crate::ui::dashboard::tiles::TreeAction>>,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).strong());
        show_section_popout_button(ui, section, selection_ref, actions);
    });
}

fn show_section_popout_button(
    ui: &mut egui::Ui,
    section: InspectorPanelSection,
    selection_ref: Option<&crate::ui::view::GraphSelectionRef>,
    actions: &mut Option<&mut Vec<crate::ui::dashboard::tiles::TreeAction>>,
) {
    if actions.is_none() {
        return;
    }

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let Some(selection) = selection_ref else {
            ui.add_enabled(false, egui::Button::new("↗"))
                .on_hover_text("Select a graph item to pop out this section");
            return;
        };

        if ui
            .button("↗")
            .on_hover_text("Pop out to new pane")
            .clicked()
        {
            if let Some(actions) = actions.as_deref_mut() {
                actions.push(crate::ui::dashboard::tiles::TreeAction::PinSection(
                    selection.clone(),
                    section,
                ));
            }
        }
    });
}

fn show_inspector_section_collapsing<R>(
    ui: &mut egui::Ui,
    title: &str,
    section: InspectorPanelSection,
    open: Option<bool>,
    selection_ref: Option<&crate::ui::view::GraphSelectionRef>,
    actions: &mut Option<&mut Vec<crate::ui::dashboard::tiles::TreeAction>>,
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) {
    ui.separator();
    let _span = tracing::trace_span!(scope::INSPECTOR_COLLAPSING_HEADER_LAYOUT).entered();
    let id = ui.make_persistent_id(("inspector-section", title));
    let mut state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    if let Some(open) = open {
        if state.is_open() != open {
            state.toggle(ui);
        }
    }

    let header_response = ui.horizontal(|ui| {
        let prev_item_spacing = ui.spacing_mut().item_spacing;
        ui.spacing_mut().item_spacing.x = 0.0;
        state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
        ui.spacing_mut().item_spacing = prev_item_spacing;

        let title_response = ui.add(
            egui::Label::new(egui::RichText::new(title).heading()).sense(egui::Sense::click()),
        );
        if title_response.clicked() {
            state.toggle(ui);
        }
        show_section_popout_button(ui, section, selection_ref, actions);
    });

    state.show_body_indented(&header_response.response, ui, |ui| add_body(ui));
}

fn show_inspector_collapsing<R>(
    ui: &mut egui::Ui,
    header: egui::CollapsingHeader,
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) {
    let _span = tracing::trace_span!(scope::INSPECTOR_COLLAPSING_HEADER_LAYOUT).entered();
    header.show(ui, add_body);
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "selection_inspector")
)]
pub(crate) fn render_right_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    selection_ref: Option<&crate::ui::view::GraphSelectionRef>,
    selection_kind: Option<&str>,
    selection_label: Option<&str>,
    sections: Option<&InspectorSections>,
    render_cache: &mut InspectorRenderCache,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
    open_state: InspectorOpenState,
    mut actions: Option<&mut Vec<crate::ui::dashboard::tiles::TreeAction>>,
) {
    let viewport_height = ui.available_height();
    let selected_call_review = if selection_ref.is_none() {
        selected_eval_protocol_call_review_key(ui)
    } else {
        None
    };
    {
        let _span = tracing::trace_span!(scope::INSPECTOR_SCROLL_AREA_LAYOUT).entered();
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(INSPECTOR_MARGIN_INNER)
                    .show(ui, |ui| {
                        ui.heading("Inspector");
                        ui.separator();

                        ui.label("Summary");
                        if let (Some(kind), Some(label)) = (selection_kind, selection_label) {
                            cached_kv_id(ui, render_cache, "kind", kind);
                            cached_kv_id(ui, render_cache, "label", label);
                        } else if let Some(artifact_key) = selected_call_review.as_deref() {
                            cached_kv_id(
                                ui,
                                render_cache,
                                "selection",
                                "eval_protocol_call_review",
                            );
                            cached_kv_artifact_file(ui, render_cache, "artifact", artifact_key);
                        } else {
                            kv(ui, "selection", "not_applicable");
                        }

                        if let Some(artifact_key) = selected_call_review.as_deref() {
                            ui.label(egui::RichText::new("Selected Eval & Protocol").strong());
                            let dashboard = EvalProtocolDashboard::from_graph(graph);
                            if let Some(protocol_artifacts) = dashboard.protocol_artifacts() {
                                render_selected_eval_protocol_call_review(
                                    ui,
                                    render_cache,
                                    artifact_key,
                                    protocol_artifacts,
                                );
                            } else {
                                cached_kv_id(ui, render_cache, "call review", "not_available");
                                cached_kv_artifact_file(ui, render_cache, "artifact", artifact_key);
                            }
                            ui.separator();
                        }

                        show_inspector_section_collapsing(
                            ui,
                            "Eval & Protocol",
                            InspectorPanelSection::EvalProtocol,
                            open_state.open(InspectorPanelSection::EvalProtocol),
                            selection_ref,
                            &mut actions,
                            |ui| render_eval_protocol_for_graph(ui, graph, render_cache),
                        );

                        ui.separator();
                        show_inspector_section_header(
                            ui,
                            "Identity",
                            InspectorPanelSection::Identity,
                            selection_ref,
                            &mut actions,
                        );
                        if let Some(sections) = sections {
                            render_identity(ui, graph, sections, render_cache);
                        } else {
                            kv(ui, "record refs", "not_applicable");
                        }

                        ui.separator();
                        show_inspector_section_header(
                            ui,
                            "Roles",
                            InspectorPanelSection::Roles,
                            selection_ref,
                            &mut actions,
                        );
                        if let Some(sections) = sections {
                            render_roles_and_metrics(ui, graph, sections, render_cache);
                        } else {
                            kv(ui, "roles", "not_applicable");
                        }

                        show_inspector_section_collapsing(
                            ui,
                            "Agent Trace",
                            InspectorPanelSection::LlmCalls,
                            open_state.open(InspectorPanelSection::LlmCalls),
                            selection_ref,
                            &mut actions,
                            |ui| {
                                if let Some(sections) = sections {
                                    render_parent_create_for_inspector(
                                        ui,
                                        graph,
                                        sections,
                                        render_cache,
                                        open_state,
                                    );
                                    ui.separator();
                                }
                                render_run_level_llm_trace_for_graph(ui, graph, render_cache);
                            },
                        );

                        show_inspector_section_collapsing(
                            ui,
                            "Patches",
                            InspectorPanelSection::Patches,
                            open_state.open(InspectorPanelSection::Patches),
                            selection_ref,
                            &mut actions,
                            |ui| {
                                if let Some(sections) = sections {
                                    render_patches_for_inspector(
                                        ui,
                                        graph,
                                        sections,
                                        render_cache,
                                        diff_cache,
                                    );
                                    ui.separator();
                                } else {
                                    cached_kv_id(
                                        ui,
                                        render_cache,
                                        "selection patch",
                                        "not_applicable",
                                    );
                                }
                                render_run_level_patch_generation_for_graph(
                                    ui,
                                    graph,
                                    render_cache,
                                );
                            },
                        );

                        // ui.separator();
                        // show_inspector_section_header(
                        //     ui,
                        //     "Patch Generation",
                        //     InspectorPanelSection::PatchGeneration,
                        //     selection_ref,
                        //     &mut actions,
                        // );
                        // if let Some(sections) = sections {
                        //     render_parent_create_for_inspector(
                        //         ui,
                        //         graph,
                        //         sections,
                        //         render_cache,
                        //         open_state,
                        //     );
                        // } else {
                        //     kv(ui, "attempt", "not_applicable");
                        // }

                        show_inspector_section_collapsing(
                            ui,
                            "Candidate Comparison",
                            InspectorPanelSection::CandidateComparison,
                            open_state.open(InspectorPanelSection::CandidateComparison),
                            selection_ref,
                            &mut actions,
                            |ui| {
                                if let Some(sections) = sections {
                                    render_candidate_comparison_for_inspector(
                                        ui,
                                        graph,
                                        sections,
                                        render_cache,
                                    );
                                } else {
                                    kv(ui, "candidate comparison", "not_applicable");
                                }
                            },
                        );

                        show_inspector_section_collapsing(
                            ui,
                            "Lineage Authority",
                            InspectorPanelSection::LineageAuthority,
                            open_state.open(InspectorPanelSection::LineageAuthority),
                            selection_ref,
                            &mut actions,
                            |ui| {
                                if let Some(sections) = sections {
                                    render_lineage_authority_for_inspector(
                                        ui,
                                        graph,
                                        sections,
                                        render_cache,
                                    );
                                } else {
                                    kv(ui, "lineage authority", "not_applicable");
                                }
                            },
                        );

                        show_inspector_section_collapsing(
                            ui,
                            "Technical",
                            InspectorPanelSection::Technical,
                            open_state.open(InspectorPanelSection::Technical),
                            selection_ref,
                            &mut actions,
                            |ui| {
                                if let Some(sections) = sections {
                                    ui.label(egui::RichText::new("Run Records").strong());
                                    render_run_records_for_inspector(
                                        ui,
                                        graph,
                                        sections,
                                        render_cache,
                                    );

                                    ui.separator();
                                    ui.label(egui::RichText::new("Graph edges").strong());
                                    render_graph_edges_for_inspector(ui, sections, render_cache);

                                    ui.separator();
                                    ui.label(egui::RichText::new("Artifact edges").strong());
                                    render_artifact_edges_for_inspector(ui, sections, render_cache);

                                    ui.separator();
                                    ui.label(egui::RichText::new("Source refs").strong());
                                    render_source_refs_for_inspector(
                                        ui,
                                        graph,
                                        sections,
                                        render_cache,
                                    );
                                } else {
                                    kv(ui, "technical", "not_applicable");
                                }
                            },
                        );

                        show_inspector_section_collapsing(
                            ui,
                            "Artifact Ids",
                            InspectorPanelSection::ArtifactIds,
                            open_state.open(InspectorPanelSection::ArtifactIds),
                            selection_ref,
                            &mut actions,
                            |ui| {
                                if let Some(sections) = sections {
                                    render_artifact_ids_for_inspector(
                                        ui,
                                        graph,
                                        sections,
                                        render_cache,
                                    );
                                } else {
                                    kv(ui, "artifact ids", "not_applicable");
                                }
                            },
                        );

                        add_inspector_scroll_end_padding(ui, viewport_height);
                    });
            });
    }
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

    let value = CopyableId::new(full);
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        true,
        |cache, ui, expanded| cache.id_galley(ui, full, expanded),
    )
}

fn cached_path_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    full: &str,
) -> egui::Response {
    let value = CopyablePath::new(full);
    let expandable = value.is_expandable();
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        expandable,
        |cache, ui, expanded| {
            let label = if expanded { full } else { value.tail() };
            cache.text_galley(ui, label, CachedTextKind::Monospace)
        },
    )
}

fn cached_artifact_file_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    full: &str,
) -> egui::Response {
    let value = CopyableArtifactFile::new(full);
    let expandable = value.is_expandable();
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        expandable,
        |cache, ui, expanded| {
            let label = if expanded { full } else { value.display_tail() };
            cache.text_galley(ui, label, CachedTextKind::Monospace)
        },
    )
}

fn cached_run_name_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    full: &str,
) -> egui::Response {
    let value = CopyableRunName::new(full);
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        false,
        |cache, ui, _expanded| cache.text_galley(ui, full, CachedTextKind::Monospace),
    )
}

fn cached_copyable_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    value: &impl CopyableExpandable,
    expandable: bool,
    label_galley: impl FnOnce(&mut InspectorRenderCache, &egui::Ui, bool) -> Arc<egui::Galley>,
) -> egui::Response {
    let id = ui.make_persistent_id((
        "ploke-egui.copyable-value",
        value.copy_menu_label(),
        id_source,
    ));
    let mut expanded = ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
    if !expandable {
        expanded = false;
    }
    let galley = label_galley(render_cache, ui, expanded);
    let response = ui
        .add(egui::Label::new(galley).sense(egui::Sense::click()))
        .on_hover_ui(|ui| {
            ui.monospace(value.full_text());
            ui.label(value.hover_text(expanded, expandable));
        });

    if response.clicked() && expandable {
        expanded = !expanded;
        ui.data_mut(|data| data.insert_temp(id, expanded));
    }

    id_display::attach_copy_context_menu(&response, value);
    id_display::copy_button(ui, value);

    response
}

fn cached_compact_id(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    id: &impl InteractiveId,
) -> egui::Response {
    let full = id.full_id();
    let (compact, expandable) = id.compact_label();
    let _span = tracing::trace_span!(
        "ploke_egui.id_display.show_compact",
        full = full,
        compact = compact,
        expandable = expandable
    )
    .entered();
    let value = CopyableId::new(full);
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        expandable,
        |cache, ui, expanded| {
            let label = if expanded { full } else { compact };
            cache.text_galley(ui, label, CachedTextKind::Monospace)
        },
    )
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

fn cached_kv_path(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, key);
        cached_path_value(ui, render_cache, ("path", key, value), value);
    });
}

fn cached_kv_artifact_file(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, key);
        cached_artifact_file_value(ui, render_cache, ("artifact-file", key, value), value);
    });
}

fn cached_kv_run_name(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, key);
        cached_run_name_value(ui, render_cache, ("run-name", key, value), value);
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

fn cached_kv_u32(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: u32,
) {
    let mut buffer = itoa::Buffer::new();
    cached_kv_id(ui, render_cache, key, buffer.format(value));
}

fn cached_kv_u64(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: u64,
) {
    let mut buffer = itoa::Buffer::new();
    cached_kv_id(ui, render_cache, key, buffer.format(value));
}

fn cached_kv_i64(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: i64,
) {
    let mut buffer = itoa::Buffer::new();
    cached_kv_id(ui, render_cache, key, buffer.format(value));
}

fn cached_kv_bool(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: bool,
) {
    cached_kv_id(ui, render_cache, key, bool_label(value));
}

fn cached_kv_debug(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: impl std::fmt::Debug,
) {
    cached_kv_id(ui, render_cache, key, format!("{value:?}").as_str());
}

fn cached_kv_f64(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: f64,
) {
    cached_kv_text(ui, render_cache, key, format_f64(value).as_str());
}

fn cached_kv_optional_f64(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<f64>,
) {
    if let Some(value) = value {
        cached_kv_f64(ui, render_cache, key, value);
    } else {
        cached_kv_text(ui, render_cache, key, "not_recorded");
    }
}

fn cached_kv_optional_usize(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<usize>,
) {
    if let Some(value) = value {
        cached_kv_usize(ui, render_cache, key, value);
    } else {
        cached_kv_text(ui, render_cache, key, "not_recorded");
    }
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

fn render_copyable_text_preview(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    label: &str,
    text: &str,
) {
    let id = ui.make_persistent_id(("ploke-egui.copyable-text-preview", label, id_source));
    let mut state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    let value = CopyableText::new(text);

    let header_response = ui.horizontal(|ui| {
        let prev_item_spacing = ui.spacing_mut().item_spacing;
        ui.spacing_mut().item_spacing.x = 0.0;
        state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
        ui.spacing_mut().item_spacing = prev_item_spacing;

        let title_response = ui
            .add(egui::Label::new(label).sense(egui::Sense::click()))
            .on_hover_text(value.hover_text(state.is_open(), true));
        if title_response.clicked() {
            state.toggle(ui);
        }
        id_display::attach_copy_context_menu(&title_response, &value);
        id_display::copy_button(ui, &value);
    });

    state.show_body_indented(&header_response.response, ui, |ui| {
        cached_wrapped_monospace_label(ui, render_cache, text);
    });
}

pub(crate) fn render_eval_protocol_for_graph(
    ui: &mut egui::Ui,
    graph: &Graph,
    render_cache: &mut InspectorRenderCache,
) {
    let dashboard = EvalProtocolDashboard::from_graph(graph);
    if !dashboard.is_available() {
        cached_kv_text(ui, render_cache, "eval protocol", "not_available");
        return;
    }

    let availability = dashboard.availability();
    cached_kv_text(
        ui,
        render_cache,
        "closure evidence",
        evidence_state_label(availability.closure),
    );
    cached_kv_text(
        ui,
        render_cache,
        "run record evidence",
        evidence_state_label(availability.run_records),
    );
    cached_kv_text(
        ui,
        render_cache,
        "protocol evidence",
        evidence_state_label(availability.protocol_artifacts),
    );

    if let Some(closure) = dashboard.closure() {
        cached_kv_path(
            ui,
            render_cache,
            "closure state",
            closure.source_path.to_str().unwrap_or("non_utf8_path"),
        );
        cached_kv_id(
            ui,
            render_cache,
            "campaign",
            closure.state.campaign_id.as_str(),
        );
        cached_kv_text(
            ui,
            render_cache,
            "registry",
            closure_status_label(&closure.state.registry.status).as_str(),
        );
        cached_kv_text(
            ui,
            render_cache,
            "eval",
            closure_status_label(&closure.state.eval.status).as_str(),
        );
        cached_kv_text(
            ui,
            render_cache,
            "protocol",
            closure_status_label(&closure.state.protocol.status).as_str(),
        );
        if let Some(model) = closure.state.config.model_id.as_deref() {
            cached_kv_id(ui, render_cache, "model", model);
        }
        if let Some(provider) = closure.state.config.provider_slug.as_deref() {
            cached_kv_id(ui, render_cache, "provider", provider);
        }
        cached_kv_usize(ui, render_cache, "instances", closure.state.instances.len());

        for instance in closure.state.instances.iter().take(3) {
            ui.separator();
            cached_kv_id(ui, render_cache, "instance", instance.instance_id.as_str());
            cached_kv_text(
                ui,
                render_cache,
                "instance eval",
                closure_status_label(&instance.eval_status).as_str(),
            );
            cached_kv_text(
                ui,
                render_cache,
                "instance protocol",
                closure_status_label(&instance.protocol_status).as_str(),
            );
            if let Some(counts) = instance.protocol_counts.as_ref() {
                cached_kv_usize(ui, render_cache, "reviewed calls", counts.reviewed_calls);
                cached_kv_usize(ui, render_cache, "total calls", counts.total_calls);
                cached_kv_usize(ui, render_cache, "usable segments", counts.usable_segments);
                cached_kv_usize(ui, render_cache, "total segments", counts.total_segments);
            }
        }
        if closure.state.instances.len() > 3 {
            cached_kv_usize(
                ui,
                render_cache,
                "more instances",
                closure.state.instances.len() - 3,
            );
        }
    }

    if let Some(run_records) = dashboard.run_records() {
        ui.separator();
        ui.label(egui::RichText::new("Eval Run Records").strong());
        cached_kv_optional_usize(
            ui,
            render_cache,
            "record files",
            dashboard.run_records_file_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "records",
            dashboard.run_records_parsed_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "branch refs",
            dashboard.run_records_branch_ref_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "turns",
            dashboard.run_records_total_turn_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "tool calls",
            dashboard.run_records_total_tool_call_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "failed tool calls",
            dashboard.run_records_failed_tool_call_count(),
        );

        let patch = dashboard.eval_patch_counts();
        cached_kv_usize(ui, render_cache, "patch phases", patch.patch_phase_count);
        cached_kv_usize(
            ui,
            render_cache,
            "empty submissions",
            patch.empty_submission_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "nonempty submissions",
            patch.nonempty_submission_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "edit proposals",
            patch.edit_proposal_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "create proposals",
            patch.create_proposal_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "expected file changes",
            patch.expected_file_change_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "applied patch artifacts",
            patch.applied_patch_artifact_count,
        );
        render_patch_projection_counts(ui, render_cache, &patch.patch_projection);

        for (record_key, record) in run_records.index.iter().take(2) {
            ui.separator();
            cached_kv_path(ui, render_cache, "record", record_key.as_str());
            cached_kv_id(ui, render_cache, "manifest", record.manifest_id.as_str());
            cached_kv_id(
                ui,
                render_cache,
                "instance",
                record.metadata.benchmark.instance_id.as_str(),
            );
            if let Some(model) = record.metadata.agent.model_id.as_deref() {
                cached_kv_id(ui, render_cache, "record model", model);
            }
            if let Some(provider) = record.metadata.agent.provider.as_deref() {
                cached_kv_id(ui, render_cache, "record provider", provider);
            }
            if let Some(timing) = record.timing.as_ref() {
                cached_kv_text(
                    ui,
                    render_cache,
                    "wall clock",
                    format!("{:.3}s", timing.total_wall_clock_secs).as_str(),
                );
                cached_kv_optional_f64(
                    ui,
                    render_cache,
                    "agent clock",
                    timing.agent_wall_clock_secs,
                );
            }
            if let Some(packaging) = record.phases.packaging.as_ref() {
                cached_kv_text(
                    ui,
                    render_cache,
                    "submission",
                    submission_artifact_state_label(packaging.submission_artifact_state),
                );
                cached_kv_text(
                    ui,
                    render_cache,
                    "projection",
                    patch_projection_check_state_label(packaging.patch_projection_check_state),
                );
            }
        }
        if run_records.index.len() > 2 {
            cached_kv_usize(
                ui,
                render_cache,
                "more records",
                run_records.index.len() - 2,
            );
        }
    }

    if dashboard.protocol_artifacts().is_some() {
        ui.separator();
        ui.label(egui::RichText::new("Protocol").strong());
        cached_kv_optional_usize(
            ui,
            render_cache,
            "artifact files",
            dashboard.protocol_artifacts_file_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "artifacts",
            dashboard.protocol_artifacts_parsed_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "intent segments",
            dashboard.protocol_artifacts_intent_segmentation_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "call reviews",
            dashboard.protocol_artifacts_review_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "segment reviews",
            dashboard.protocol_artifacts_segment_review_count(),
        );

        let stats = dashboard.protocol_aggregate_counts();
        cached_kv_usize(
            ui,
            render_cache,
            "issue detections",
            stats.issue_detection_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "issue cases",
            stats.issue_detection_case_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "interventions",
            stats.intervention_candidate_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "intervention applies",
            stats.intervention_apply_count,
        );
        render_eval_protocol_visual_summary(ui, render_cache, dashboard.visual_summary());
        render_protocol_artifact_drilldowns(ui, render_cache, &dashboard);
    }
}

pub(crate) fn render_eval_protocol_pane(
    ui: &mut egui::Ui,
    graph: &Graph,
    render_cache: &mut InspectorRenderCache,
    mode: EvalProtocolRenderMode,
) {
    match mode {
        EvalProtocolRenderMode::Full => render_eval_protocol_for_graph(ui, graph, render_cache),
        EvalProtocolRenderMode::CallReviewScanOnly => {
            render_eval_protocol_call_review_scan_for_graph(ui, graph, render_cache);
        }
    }
}

fn render_eval_protocol_call_review_scan_for_graph(
    ui: &mut egui::Ui,
    graph: &Graph,
    render_cache: &mut InspectorRenderCache,
) {
    let dashboard = EvalProtocolDashboard::from_graph(graph);
    let Some(protocol_artifacts) = dashboard.protocol_artifacts() else {
        cached_kv_text(ui, render_cache, "call review scan", "not_available");
        return;
    };
    render_call_review_scan(ui, render_cache, protocol_artifacts);
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::EVAL_PROTOCOL_VISUAL_SUMMARY)]
fn render_eval_protocol_visual_summary(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    summary: EvalProtocolVisualSummary,
) {
    if !summary.has_any_visual_data() {
        return;
    }

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Analyst Snapshot").default_open(true),
        |ui| {
            let volume = [
                ("records", summary.run_records as f32),
                ("turns", summary.turns as f32),
                ("tool calls", summary.tool_calls as f32),
                ("failed tools", summary.failed_tool_calls as f32),
            ];
            crate::ui::charts::horizontal_bar_chart(ui, "Run Effort", &volume);

            ui.separator();
            let coverage = [
                ("reviewed calls", summary.reviewed_calls as f32),
                (
                    "missing call reviews",
                    summary.missing_call_reviews() as f32,
                ),
                ("usable segments", summary.usable_segments as f32),
                ("mismatched segments", summary.mismatched_segments as f32),
                ("missing segments", summary.missing_segments as f32),
            ];
            crate::ui::charts::horizontal_bar_chart(ui, "Protocol Coverage", &coverage);

            let outcomes = summary
                .call_review_outcomes
                .combined(summary.segment_review_outcomes);
            if outcomes.total() > 0 {
                ui.separator();
                let data = [
                    ("focused progress", outcomes.focused_progress as f32),
                    ("useful exploration", outcomes.useful_exploration as f32),
                    ("recoverable detour", outcomes.recoverable_detour as f32),
                    ("redundant thrash", outcomes.redundant_thrash as f32),
                    ("mixed", outcomes.mixed as f32),
                    ("unclear", outcomes.unclear as f32),
                ];
                crate::ui::charts::horizontal_bar_chart(ui, "Review Outcome Mix", &data);
            }

            if summary.patch.edit_proposal_count > 0
                || summary.patch.create_proposal_count > 0
                || summary.patch.expected_file_change_count > 0
                || summary.patch.applied_patch_artifact_count > 0
            {
                ui.separator();
                let patch = [
                    ("edit proposals", summary.patch.edit_proposal_count as f32),
                    (
                        "create proposals",
                        summary.patch.create_proposal_count as f32,
                    ),
                    (
                        "expected file changes",
                        summary.patch.expected_file_change_count as f32,
                    ),
                    (
                        "applied artifacts",
                        summary.patch.applied_patch_artifact_count as f32,
                    ),
                ];
                crate::ui::charts::horizontal_bar_chart(ui, "Patch Production", &patch);
            }

            ui.separator();
            cached_kv_usize(ui, render_cache, "total calls", summary.total_calls);
            cached_kv_usize(ui, render_cache, "total segments", summary.total_segments);
        },
    );
}

fn render_protocol_artifact_drilldowns(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    dashboard: &EvalProtocolDashboard<'_>,
) {
    let Some(protocol_artifacts) = dashboard.protocol_artifacts() else {
        return;
    };

    let review_count = protocol_artifacts
        .index
        .values()
        .filter(|artifact| matches!(&artifact.body, ArtifactBody::ToolCallReview(_)))
        .count();
    if review_count > 0 {
        render_call_review_scan(ui, render_cache, protocol_artifacts);
        show_inspector_collapsing(
            ui,
            egui::CollapsingHeader::new("Reviewed Calls").default_open(false),
            |ui| {
                cached_kv_usize(ui, render_cache, "reviewed calls", review_count);
                for (review_index, (artifact_key, artifact)) in protocol_artifacts
                    .index
                    .iter()
                    .filter(|(_, artifact)| {
                        matches!(&artifact.body, ArtifactBody::ToolCallReview(_))
                    })
                    .enumerate()
                {
                    let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
                        continue;
                    };
                    render_tool_call_review_artifact(
                        ui,
                        render_cache,
                        review_index,
                        artifact_key,
                        artifact,
                        payload,
                    );
                }
            },
        );
    }

    let segment_count = protocol_artifacts
        .index
        .values()
        .filter(|artifact| matches!(&artifact.body, ArtifactBody::ToolCallSegmentReview(_)))
        .count();
    if segment_count > 0 {
        show_inspector_collapsing(
            ui,
            egui::CollapsingHeader::new("Usable Segment Reviews").default_open(false),
            |ui| {
                cached_kv_usize(ui, render_cache, "segment reviews", segment_count);
                for (review_index, (artifact_key, artifact)) in protocol_artifacts
                    .index
                    .iter()
                    .filter(|(_, artifact)| {
                        matches!(&artifact.body, ArtifactBody::ToolCallSegmentReview(_))
                    })
                    .enumerate()
                {
                    let ArtifactBody::ToolCallSegmentReview(payload) = &artifact.body else {
                        continue;
                    };
                    render_tool_call_segment_review_artifact(
                        ui,
                        render_cache,
                        review_index,
                        artifact_key,
                        artifact,
                        payload,
                    );
                }
            },
        );
    }

    let segmentation_count = protocol_artifacts
        .index
        .values()
        .filter(|artifact| matches!(&artifact.body, ArtifactBody::ToolCallIntentSegmentation(_)))
        .count();
    if segmentation_count > 0 {
        show_inspector_collapsing(
            ui,
            egui::CollapsingHeader::new("Intent Segmentation").default_open(false),
            |ui| {
                cached_kv_usize(ui, render_cache, "segmentations", segmentation_count);
                for (artifact_index, (artifact_key, artifact)) in protocol_artifacts
                    .index
                    .iter()
                    .filter(|(_, artifact)| {
                        matches!(&artifact.body, ArtifactBody::ToolCallIntentSegmentation(_))
                    })
                    .enumerate()
                {
                    let ArtifactBody::ToolCallIntentSegmentation(payload) = &artifact.body else {
                        continue;
                    };
                    render_intent_segmentation_artifact(
                        ui,
                        render_cache,
                        artifact_index,
                        artifact_key,
                        artifact,
                        payload,
                    );
                }
            },
        );
    }
}

fn selected_eval_protocol_call_review_id() -> egui::Id {
    egui::Id::new("ploke-egui.eval-protocol.selected-call-review")
}

fn selected_eval_protocol_call_review_key(ui: &egui::Ui) -> Option<Arc<str>> {
    ui.data(|data| data.get_temp::<Arc<str>>(selected_eval_protocol_call_review_id()))
}

fn set_selected_eval_protocol_call_review(ui: &mut egui::Ui, artifact_key: &str) {
    ui.data_mut(|data| {
        data.insert_temp(
            selected_eval_protocol_call_review_id(),
            Arc::<str>::from(artifact_key),
        );
    });
}

fn clear_selected_eval_protocol_call_review(ui: &mut egui::Ui) {
    ui.data_mut(|data| {
        let _ = data.remove_temp::<Arc<str>>(selected_eval_protocol_call_review_id());
    });
}

fn render_selected_eval_protocol_call_review(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    artifact_key: &str,
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
) {
    let Some(artifact) = protocol_artifacts.index.get(artifact_key) else {
        cached_kv_id(ui, render_cache, "call review", "not_available");
        cached_kv_artifact_file(ui, render_cache, "artifact", artifact_key);
        return;
    };

    let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
        cached_kv_id(ui, render_cache, "call review", "not_applicable");
        cached_kv_artifact_file(ui, render_cache, "artifact", artifact_key);
        return;
    };

    render_tool_call_review_detail(ui, render_cache, artifact_key, artifact, payload, true);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallReviewFilter {
    All,
    FailedScope,
    Mixed,
    Recoverable,
    Redundant,
    LowConfidence,
}

impl CallReviewFilter {
    const ALL: [Self; 6] = [
        Self::All,
        Self::FailedScope,
        Self::Mixed,
        Self::Recoverable,
        Self::Redundant,
        Self::LowConfidence,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::FailedScope => "scope failures",
            Self::Mixed => "mixed",
            Self::Recoverable => "recoverable",
            Self::Redundant => "redundant",
            Self::LowConfidence => "low confidence",
        }
    }

    fn hover_text(self) -> &'static str {
        match self {
            Self::All => "Show every reviewed tool call.",
            Self::FailedScope => "Show reviews whose local analysis scope includes failed calls.",
            Self::Mixed => "Show reviews whose overall verdict is Mixed.",
            Self::Recoverable => "Show reviews whose overall verdict is RecoverableDetour.",
            Self::Redundant => "Show reviews whose overall verdict is RedundantThrash.",
            Self::LowConfidence => "Show reviews whose overall confidence is Low.",
        }
    }

    fn count(self, counts: CallReviewScanCounts) -> usize {
        match self {
            Self::All => counts.total,
            Self::FailedScope => counts.failed_scope,
            Self::Mixed => counts.mixed,
            Self::Recoverable => counts.recoverable,
            Self::Redundant => counts.redundant,
            Self::LowConfidence => counts.low_confidence,
        }
    }

    fn matches(self, payload: &ploke_records::protocol::ToolCallReviewPayload) -> bool {
        match self {
            Self::All => true,
            Self::FailedScope => payload.output.signals.failed_calls_in_scope > 0,
            Self::Mixed => payload.output.overall == ploke_protocol::OverallVerdict::Mixed,
            Self::Recoverable => {
                payload.output.overall == ploke_protocol::OverallVerdict::RecoverableDetour
            }
            Self::Redundant => {
                payload.output.overall == ploke_protocol::OverallVerdict::RedundantThrash
            }
            Self::LowConfidence => {
                payload.output.overall_confidence == ploke_protocol::Confidence::Low
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CallReviewSort {
    column: CallReviewSortColumn,
    direction: SortDirection,
}

impl CallReviewSort {
    fn toggle_column(&mut self, column: CallReviewSortColumn) {
        if self.column == column {
            self.direction = self.direction.toggled();
        } else {
            self.column = column;
            self.direction = SortDirection::Asc;
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum CallReviewSortColumn {
    #[default]
    Call,
    Tool,
    Outcome,
    Confidence,
    Failed,
    LatencyMs,
    Scope,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl SortDirection {
    fn toggled(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }

    fn indicator(self) -> &'static str {
        match self {
            Self::Asc => "^",
            Self::Desc => "v",
        }
    }

    fn apply(self, ordering: Ordering) -> Ordering {
        match self {
            Self::Asc => ordering,
            Self::Desc => ordering.reverse(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CallReviewScanCounts {
    total: usize,
    failed_scope: usize,
    mixed: usize,
    recoverable: usize,
    redundant: usize,
    low_confidence: usize,
}

#[derive(Debug, Default)]
struct CallReviewScanOrderCache {
    key: Option<CallReviewScanOrderKey>,
    rows: Arc<[String]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CallReviewScanOrderKey {
    filter: CallReviewFilter,
    sort: CallReviewSort,
    artifact_count: usize,
    artifact_rows_hash: u64,
}

impl CallReviewScanOrderKey {
    fn new(
        protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
        filter: CallReviewFilter,
        sort: CallReviewSort,
    ) -> Self {
        Self {
            filter,
            sort,
            artifact_count: protocol_artifacts.index.len(),
            artifact_rows_hash: protocol_call_review_rows_hash(protocol_artifacts),
        }
    }
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::EVAL_PROTOCOL_CALL_REVIEW_SCAN)]
fn render_call_review_scan(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
) {
    let counts = call_review_scan_counts(protocol_artifacts);
    let filter_id = ui.make_persistent_id("eval-protocol-call-review-filter");
    let mut filter = ui
        .data(|data| data.get_temp::<CallReviewFilter>(filter_id))
        .unwrap_or(CallReviewFilter::All);
    let sort_id = ui.make_persistent_id("eval-protocol-call-review-sort");
    let mut sort = ui
        .data(|data| data.get_temp::<CallReviewSort>(sort_id))
        .unwrap_or_default();

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Call Review Scan").default_open(true),
        |ui| {
            cached_kv_usize(ui, render_cache, "call reviews", counts.total);
            ui.horizontal_wrapped(|ui| {
                cached_label(ui, render_cache, "slice");
                for candidate in CallReviewFilter::ALL {
                    let label = candidate.label();
                    let count = candidate.count(counts);
                    let response = ui
                        .selectable_label(filter == candidate, label)
                        .on_hover_text(candidate.hover_text());
                    if response.clicked() {
                        filter = candidate;
                        ui.data_mut(|data| data.insert_temp(filter_id, filter));
                    }
                    let mut buffer = itoa::Buffer::new();
                    cached_monospace_label(ui, render_cache, buffer.format(count));
                }
            });

            render_call_review_reasoning_spotlight(ui, render_cache, protocol_artifacts);

            let matching = filter.count(counts);
            if matching == 0 {
                cached_kv_text(ui, render_cache, "index", "no matching call reviews");
                return;
            }

            ui.separator();
            egui::Grid::new("eval-protocol-call-review-scan-grid")
                .num_columns(7)
                .striped(true)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    let header_background = ui.painter().add(egui::Shape::Noop);
                    let mut header_rect = egui::Rect::NOTHING;
                    let mut sort_changed = false;
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "call",
                            "Focal tool-call index in the run.",
                            CallReviewSortColumn::Call,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "tool",
                            "Focal tool name from the protocol review.",
                            CallReviewSortColumn::Tool,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "outcome",
                            "Overall protocol review verdict for the focal call.",
                            CallReviewSortColumn::Outcome,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "confidence",
                            "Overall confidence assigned by the protocol review.",
                            CallReviewSortColumn::Confidence,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "failed",
                            "Whether the focal call failed, or another call in scope failed.",
                            CallReviewSortColumn::Failed,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "latency ms",
                            "Protocol NeighborhoodCall.latency_ms: focal tool-call latency in milliseconds. This is not split into model wait versus local/tool time yet.",
                            CallReviewSortColumn::LatencyMs,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    include_response_rect(
                        &mut header_rect,
                        &call_review_scan_header_cell(
                            ui,
                            "scope",
                            "Number of calls in the local analysis scope.",
                            CallReviewSortColumn::Scope,
                            &mut sort,
                            &mut sort_changed,
                        ),
                    );
                    ui.end_row();
                    if sort_changed {
                        ui.data_mut(|data| data.insert_temp(sort_id, sort));
                    }
                    paint_call_review_scan_header_background(
                        ui,
                        header_background,
                        header_rect.expand2(egui::vec2(4.0, 2.0)),
                    );

                    let selected_call_review = selected_eval_protocol_call_review_key(ui);
                    let row_order = render_cache.call_review_scan_order(
                        protocol_artifacts,
                        filter,
                        sort,
                    );
                    for artifact_key in row_order.iter() {
                        let Some(artifact) = protocol_artifacts.index.get(artifact_key.as_str())
                        else {
                            continue;
                        };
                        let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
                            continue;
                        };
                        let selected =
                            selected_call_review.as_deref() == Some(artifact_key.as_str());
                        render_call_review_scan_row(
                            ui,
                            render_cache,
                            artifact_key.as_str(),
                            payload,
                            selected,
                        );
                    }
                });
        },
    );
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::EVAL_PROTOCOL_CALL_REVIEW_SPOTLIGHT)]
fn render_call_review_reasoning_spotlight(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
) {
    let selected_artifact_key = selected_eval_protocol_call_review_key(ui);
    ui.add_space(4.0);
    egui::Frame::group(ui.style())
        .fill(ui.visuals().widgets.active.weak_bg_fill)
        .stroke(egui::Stroke::new(1.0, ui.visuals().selection.bg_fill))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                cached_label(ui, render_cache, "LLM Reasoning Spotlight");
                if let Some(artifact_key) = selected_artifact_key.as_deref() {
                    cached_artifact_file_value(
                        ui,
                        render_cache,
                        ("call-review-spotlight-artifact", artifact_key),
                        artifact_key,
                    );
                }
            });

            let Some(artifact_key) = selected_artifact_key.as_deref() else {
                cached_label(
                    ui,
                    render_cache,
                    "Select a call row to pin its LLM reasoning here.",
                );
                return;
            };
            let Some(artifact) = protocol_artifacts.index.get(artifact_key) else {
                cached_kv_artifact_file(ui, render_cache, "selected", artifact_key);
                cached_kv_id(ui, render_cache, "reasoning", "not_available");
                return;
            };
            let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
                cached_kv_artifact_file(ui, render_cache, "selected", artifact_key);
                cached_kv_id(ui, render_cache, "reasoning", "not_applicable");
                return;
            };

            let focal = &payload.input.focal;
            ui.horizontal_wrapped(|ui| {
                cached_label(ui, render_cache, "call");
                let mut index_buffer = itoa::Buffer::new();
                cached_monospace_label(ui, render_cache, index_buffer.format(focal.index));
                cached_monospace_label(ui, render_cache, focal.tool_name.as_str());
                scan_value_label(
                    ui,
                    render_cache,
                    overall_verdict_label(payload.output.overall),
                    overall_verdict_emphasis(payload.output.overall),
                );
                scan_value_label(
                    ui,
                    render_cache,
                    confidence_label(payload.output.overall_confidence),
                    confidence_emphasis(payload.output.overall_confidence),
                );
            });

            ui.horizontal(|ui| {
                cached_label(ui, render_cache, "synthesis");
                let copy_value = CopyableText::new(payload.output.synthesis_rationale.as_str());
                id_display::copy_button(ui, &copy_value);
            });
            let copy_value = CopyableText::new(payload.output.synthesis_rationale.as_str());
            let response = cached_wrapped_monospace_label(
                ui,
                render_cache,
                payload.output.synthesis_rationale.as_str(),
            )
            .on_hover_text(copy_value.hover_text(false, false));
            id_display::attach_copy_context_menu(&response, &copy_value);
        });
}

fn call_review_scan_counts(
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
) -> CallReviewScanCounts {
    let mut counts = CallReviewScanCounts::default();
    for artifact in protocol_artifacts.index.values() {
        let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
            continue;
        };
        counts.total += 1;
        if CallReviewFilter::FailedScope.matches(payload) {
            counts.failed_scope += 1;
        }
        if CallReviewFilter::Mixed.matches(payload) {
            counts.mixed += 1;
        }
        if CallReviewFilter::Recoverable.matches(payload) {
            counts.recoverable += 1;
        }
        if CallReviewFilter::Redundant.matches(payload) {
            counts.redundant += 1;
        }
        if CallReviewFilter::LowConfidence.matches(payload) {
            counts.low_confidence += 1;
        }
    }
    counts
}

fn build_call_review_scan_order(
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
    filter: CallReviewFilter,
    sort: CallReviewSort,
) -> Arc<[String]> {
    let mut rows = Vec::new();
    for (artifact_key, artifact) in &protocol_artifacts.index {
        let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
            continue;
        };
        if filter.matches(payload) {
            rows.push(artifact_key.clone());
        }
    }

    rows.sort_by(|left_key, right_key| {
        compare_call_review_artifact_keys(protocol_artifacts, left_key, right_key, sort)
    });
    Arc::from(rows)
}

fn compare_call_review_artifact_keys(
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
    left_key: &str,
    right_key: &str,
    sort: CallReviewSort,
) -> Ordering {
    let Some(left) = protocol_artifacts.index.get(left_key) else {
        return Ordering::Greater;
    };
    let Some(right) = protocol_artifacts.index.get(right_key) else {
        return Ordering::Less;
    };
    let ArtifactBody::ToolCallReview(left) = &left.body else {
        return Ordering::Greater;
    };
    let ArtifactBody::ToolCallReview(right) = &right.body else {
        return Ordering::Less;
    };

    sort.direction
        .apply(compare_call_review_payloads(left, right, sort.column))
        .then_with(|| left.input.focal.index.cmp(&right.input.focal.index))
        .then_with(|| left.input.focal.tool_name.cmp(&right.input.focal.tool_name))
        .then_with(|| left_key.cmp(right_key))
}

fn compare_call_review_payloads(
    left: &ploke_records::protocol::ToolCallReviewPayload,
    right: &ploke_records::protocol::ToolCallReviewPayload,
    column: CallReviewSortColumn,
) -> Ordering {
    match column {
        CallReviewSortColumn::Call => left.input.focal.index.cmp(&right.input.focal.index),
        CallReviewSortColumn::Tool => left.input.focal.tool_name.cmp(&right.input.focal.tool_name),
        CallReviewSortColumn::Outcome => overall_verdict_rank(left.output.overall)
            .cmp(&overall_verdict_rank(right.output.overall)),
        CallReviewSortColumn::Confidence => confidence_rank(left.output.overall_confidence)
            .cmp(&confidence_rank(right.output.overall_confidence)),
        CallReviewSortColumn::Failed => failure_rank(left).cmp(&failure_rank(right)),
        CallReviewSortColumn::LatencyMs => left
            .input
            .focal
            .latency_ms
            .cmp(&right.input.focal.latency_ms),
        CallReviewSortColumn::Scope => left
            .output
            .packet
            .total_calls_in_scope
            .cmp(&right.output.packet.total_calls_in_scope),
    }
}

fn protocol_call_review_rows_hash(
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    for (key, artifact) in &protocol_artifacts.index {
        key.hash(&mut hasher);
        if let ArtifactBody::ToolCallReview(payload) = &artifact.body {
            1u8.hash(&mut hasher);
            payload.input.focal.index.hash(&mut hasher);
            payload.input.focal.tool_name.hash(&mut hasher);
            overall_verdict_rank(payload.output.overall).hash(&mut hasher);
            confidence_rank(payload.output.overall_confidence).hash(&mut hasher);
            failure_rank(payload).hash(&mut hasher);
            payload.input.focal.latency_ms.hash(&mut hasher);
            payload.output.packet.total_calls_in_scope.hash(&mut hasher);
        } else {
            0u8.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn call_review_scan_header_cell(
    ui: &mut egui::Ui,
    label: &'static str,
    hover_text: &'static str,
    column: CallReviewSortColumn,
    sort: &mut CallReviewSort,
    sort_changed: &mut bool,
) -> egui::Response {
    let active = sort.column == column;
    ui.horizontal(|ui| {
        let response = ui
            .add(egui::Button::new(text_style::inspector_table_header(ui, label)).frame(false))
            .on_hover_ui(|ui| {
                ui.label(hover_text);
                ui.label(call_review_sort_hover_text(active, sort.direction));
            });
        if response.clicked() {
            sort.toggle_column(column);
            *sort_changed = true;
        }
        if sort.column == column {
            ui.label(text_style::inspector_table_sort_indicator(
                ui,
                sort.direction.indicator(),
            ));
        }
    })
    .response
}

fn call_review_sort_hover_text(active: bool, direction: SortDirection) -> &'static str {
    if !active {
        "Click to sort by this column."
    } else {
        match direction {
            SortDirection::Asc => "Sorted ascending. Click to sort descending.",
            SortDirection::Desc => "Sorted descending. Click to sort ascending.",
        }
    }
}

fn include_response_rect(rect: &mut egui::Rect, response: &egui::Response) {
    *rect = rect.union(response.rect);
}

fn paint_call_review_scan_header_background(
    ui: &egui::Ui,
    shape: egui::layers::ShapeIdx,
    rect: egui::Rect,
) {
    ui.painter().set(
        shape,
        egui::Shape::rect_filled(rect, 0.0, text_style::inspector_table_header_background(ui)),
    );
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::EVAL_PROTOCOL_CALL_REVIEW_ROW)]
fn render_call_review_scan_row(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    artifact_key: &str,
    payload: &ploke_records::protocol::ToolCallReviewPayload,
    selected: bool,
) {
    let background = ui.painter().add(egui::Shape::Noop);
    let rail = ui.painter().add(egui::Shape::Noop);
    let mut row_rect = egui::Rect::NOTHING;
    let mut row_hovered = false;
    let mut row_clicked = false;
    let focal = &payload.input.focal;
    let mut index_buffer = itoa::Buffer::new();
    let response = cached_monospace_cell(ui, render_cache, index_buffer.format(focal.index));
    let response = call_review_scan_reason_hover(
        response,
        render_cache,
        "focal call summary",
        focal.summary.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    let response = cached_monospace_cell(ui, render_cache, focal.tool_name.as_str());
    let response = call_review_scan_reason_hover(
        response,
        render_cache,
        "LLM synthesis",
        payload.output.synthesis_rationale.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    let response = scan_value_cell(
        ui,
        render_cache,
        overall_verdict_label(payload.output.overall),
        overall_verdict_emphasis(payload.output.overall),
    );
    let response = call_review_scan_reason_hover(
        response,
        render_cache,
        "LLM synthesis",
        payload.output.synthesis_rationale.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    let response = scan_value_cell(
        ui,
        render_cache,
        confidence_label(payload.output.overall_confidence),
        confidence_emphasis(payload.output.overall_confidence),
    );
    let response = call_review_scan_reason_hover(
        response,
        render_cache,
        "LLM synthesis",
        payload.output.synthesis_rationale.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    let response = scan_value_cell(
        ui,
        render_cache,
        call_review_failure_label(payload),
        failure_emphasis(payload),
    );
    let response = call_review_scan_reason_hover(
        response,
        render_cache,
        "LLM recoverability rationale",
        payload.output.recoverability.rationale.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    let mut latency_buffer = itoa::Buffer::new();
    let response = cached_monospace_cell(ui, render_cache, latency_buffer.format(focal.latency_ms));
    let response = call_review_scan_two_part_hover(
        response,
        render_cache,
        "focal call summary",
        focal.summary.as_str(),
        "LLM synthesis",
        payload.output.synthesis_rationale.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    let mut scope_buffer = itoa::Buffer::new();
    let response = cached_monospace_cell(
        ui,
        render_cache,
        scope_buffer.format(payload.output.packet.total_calls_in_scope),
    );
    let response = call_review_scan_two_part_hover(
        response,
        render_cache,
        "scope summary",
        payload.output.packet.scope_summary.as_str(),
        "LLM synthesis",
        payload.output.synthesis_rationale.as_str(),
    );
    include_call_review_scan_cell(&mut row_rect, &mut row_hovered, &mut row_clicked, response);
    ui.end_row();

    let row_hovered = row_hovered || call_review_scan_row_contains_pointer(ui, row_rect);
    if row_clicked {
        set_selected_eval_protocol_call_review(ui, artifact_key);
    }

    let fill = if selected {
        Some(text_style::inspector_table_selected_row_background(ui))
    } else if row_hovered {
        Some(text_style::inspector_table_hovered_row_background(ui))
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().set(
            background,
            egui::Shape::rect_filled(row_rect.expand2(egui::vec2(4.0, 1.0)), 0.0, fill),
        );
    }
    if selected {
        let rail_rect = egui::Rect::from_min_max(
            egui::pos2(row_rect.left() - 5.0, row_rect.top() - 1.0),
            egui::pos2(row_rect.left() - 2.0, row_rect.bottom() + 1.0),
        );
        ui.painter().set(
            rail,
            egui::Shape::rect_filled(
                rail_rect,
                0.0,
                text_style::inspector_table_selected_row_rail(ui),
            ),
        );
    }
}

fn call_review_scan_row_contains_pointer(ui: &egui::Ui, row_rect: egui::Rect) -> bool {
    if !row_rect.is_positive() {
        return false;
    }
    let hit_rect = row_rect.expand2(egui::vec2(4.0, 2.0));
    ui.ctx()
        .pointer_hover_pos()
        .is_some_and(|pos| hit_rect.contains(pos))
}

fn include_call_review_scan_cell(
    row_rect: &mut egui::Rect,
    row_hovered: &mut bool,
    row_clicked: &mut bool,
    response: egui::Response,
) {
    include_response_rect(row_rect, &response);
    *row_hovered |= response.hovered();
    *row_clicked |= response.clicked();
}

fn cached_monospace_cell(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, CachedTextKind::Monospace);
    ui.add(egui::Label::new(galley).sense(egui::Sense::click()))
}

fn scan_value_cell(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
    emphasis: ScanValueEmphasis,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, scan_value_cached_text_kind(emphasis));
    ui.add(egui::Label::new(galley).sense(egui::Sense::click()))
}

fn call_review_scan_reason_hover(
    response: egui::Response,
    render_cache: &mut InspectorRenderCache,
    title: &'static str,
    reasoning: &str,
) -> egui::Response {
    response.on_hover_ui(|ui| {
        cached_label(ui, render_cache, title);
        cached_hover_monospace_preview(ui, render_cache, reasoning);
        cached_label(
            ui,
            render_cache,
            "Click to show the full reasoning in the right inspector.",
        );
    })
}

fn call_review_scan_two_part_hover(
    response: egui::Response,
    render_cache: &mut InspectorRenderCache,
    first_title: &'static str,
    first_body: &str,
    second_title: &'static str,
    second_body: &str,
) -> egui::Response {
    response.on_hover_ui(|ui| {
        cached_label(ui, render_cache, first_title);
        cached_hover_monospace_preview(ui, render_cache, first_body);
        ui.separator();
        cached_label(ui, render_cache, second_title);
        cached_hover_monospace_preview(ui, render_cache, second_body);
        cached_label(
            ui,
            render_cache,
            "Click to show the full reasoning in the right inspector.",
        );
    })
}

fn cached_hover_monospace_block(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, CachedTextKind::MonospaceHover);
    ui.add(egui::Label::new(galley))
}

fn cached_hover_monospace_preview(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let (preview, truncated) = bounded_utf8_prefix(text, CALL_REVIEW_SCAN_HOVER_PREVIEW_BYTES);
    let response = cached_hover_monospace_block(ui, render_cache, preview);
    if truncated {
        cached_label(
            ui,
            render_cache,
            "Preview truncated to keep table hover responsive.",
        );
    }
    response
}

fn bounded_utf8_prefix(text: &str, max_bytes: usize) -> (&str, bool) {
    if text.len() <= max_bytes {
        return (text, false);
    }

    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanValueEmphasis {
    Normal,
    Warn,
    Error,
}

fn scan_value_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
    emphasis: ScanValueEmphasis,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, scan_value_cached_text_kind(emphasis));
    ui.add(egui::Label::new(galley))
}

fn scan_value_cached_text_kind(emphasis: ScanValueEmphasis) -> CachedTextKind {
    match emphasis {
        ScanValueEmphasis::Normal => CachedTextKind::Monospace,
        ScanValueEmphasis::Warn => CachedTextKind::MonospaceWarn,
        ScanValueEmphasis::Error => CachedTextKind::MonospaceError,
    }
}

fn call_review_failure_label(
    payload: &ploke_records::protocol::ToolCallReviewPayload,
) -> &'static str {
    if payload.input.focal.failed {
        "focal"
    } else if payload.output.signals.failed_calls_in_scope > 0 {
        "scope"
    } else {
        "ok"
    }
}

fn failure_rank(payload: &ploke_records::protocol::ToolCallReviewPayload) -> u8 {
    if payload.input.focal.failed {
        2
    } else if payload.output.signals.failed_calls_in_scope > 0 {
        1
    } else {
        0
    }
}

fn failure_emphasis(payload: &ploke_records::protocol::ToolCallReviewPayload) -> ScanValueEmphasis {
    if payload.input.focal.failed {
        ScanValueEmphasis::Error
    } else if payload.output.signals.failed_calls_in_scope > 0 {
        ScanValueEmphasis::Warn
    } else {
        ScanValueEmphasis::Normal
    }
}

fn overall_verdict_label(verdict: ploke_protocol::OverallVerdict) -> &'static str {
    match verdict {
        ploke_protocol::OverallVerdict::FocusedProgress => "focused",
        ploke_protocol::OverallVerdict::UsefulExploration => "useful",
        ploke_protocol::OverallVerdict::RecoverableDetour => "recoverable",
        ploke_protocol::OverallVerdict::RedundantThrash => "redundant",
        ploke_protocol::OverallVerdict::Mixed => "mixed",
        ploke_protocol::OverallVerdict::Unclear => "unclear",
    }
}

fn overall_verdict_rank(verdict: ploke_protocol::OverallVerdict) -> u8 {
    match verdict {
        ploke_protocol::OverallVerdict::FocusedProgress => 0,
        ploke_protocol::OverallVerdict::UsefulExploration => 1,
        ploke_protocol::OverallVerdict::RecoverableDetour => 2,
        ploke_protocol::OverallVerdict::RedundantThrash => 3,
        ploke_protocol::OverallVerdict::Mixed => 4,
        ploke_protocol::OverallVerdict::Unclear => 5,
    }
}

fn overall_verdict_emphasis(verdict: ploke_protocol::OverallVerdict) -> ScanValueEmphasis {
    match verdict {
        ploke_protocol::OverallVerdict::FocusedProgress
        | ploke_protocol::OverallVerdict::UsefulExploration => ScanValueEmphasis::Normal,
        ploke_protocol::OverallVerdict::RecoverableDetour
        | ploke_protocol::OverallVerdict::Mixed
        | ploke_protocol::OverallVerdict::Unclear => ScanValueEmphasis::Warn,
        ploke_protocol::OverallVerdict::RedundantThrash => ScanValueEmphasis::Error,
    }
}

fn confidence_label(confidence: ploke_protocol::Confidence) -> &'static str {
    match confidence {
        ploke_protocol::Confidence::Low => "low",
        ploke_protocol::Confidence::Medium => "medium",
        ploke_protocol::Confidence::High => "high",
    }
}

fn confidence_rank(confidence: ploke_protocol::Confidence) -> u8 {
    match confidence {
        ploke_protocol::Confidence::Low => 0,
        ploke_protocol::Confidence::Medium => 1,
        ploke_protocol::Confidence::High => 2,
    }
}

fn confidence_emphasis(confidence: ploke_protocol::Confidence) -> ScanValueEmphasis {
    match confidence {
        ploke_protocol::Confidence::Low => ScanValueEmphasis::Warn,
        ploke_protocol::Confidence::Medium | ploke_protocol::Confidence::High => {
            ScanValueEmphasis::Normal
        }
    }
}

fn render_protocol_artifact_coordinate(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    artifact_key: &str,
    artifact: &ploke_records::protocol::Artifact,
) {
    cached_kv_artifact_file(ui, render_cache, "artifact", artifact_key);
    cached_kv_id(
        ui,
        render_cache,
        "procedure",
        artifact.procedure_name.as_str(),
    );
    cached_kv_id(ui, render_cache, "subject", artifact.subject_id.as_str());
    cached_kv_run_name(ui, render_cache, "run", artifact.run_id.as_str());
    cached_kv_u64(ui, render_cache, "created at ms", artifact.created_at_ms);
    if let Some(model) = artifact.model_id.as_deref() {
        cached_kv_id(ui, render_cache, "model", model);
    }
    if let Some(provider) = artifact.provider_slug.as_deref() {
        cached_kv_id(ui, render_cache, "provider", provider);
    }
}

fn render_tool_call_review_artifact(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    review_index: usize,
    artifact_key: &str,
    artifact: &ploke_records::protocol::Artifact,
    payload: &ploke_records::protocol::ToolCallReviewPayload,
) {
    let focal = &payload.input.focal;
    let title = format!(
        "call {} {} {:?}",
        focal.index, focal.tool_name, payload.output.overall
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt(("tool-call-review", artifact_key, review_index))
            .default_open(false),
        |ui| {
            render_tool_call_review_detail(
                ui,
                render_cache,
                artifact_key,
                artifact,
                payload,
                false,
            );
        },
    );
}

fn render_tool_call_review_detail_header(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    payload: &ploke_records::protocol::ToolCallReviewPayload,
    clearable: bool,
) {
    let focal = &payload.input.focal;
    ui.horizontal_wrapped(|ui| {
        cached_label(ui, render_cache, "selected call");
        let mut index_buffer = itoa::Buffer::new();
        cached_monospace_label(ui, render_cache, index_buffer.format(focal.index))
            .on_hover_text("Focal tool-call index in the reviewed neighborhood.");
        cached_monospace_label(ui, render_cache, focal.tool_name.as_str())
            .on_hover_text("Focal tool name from the protocol review.");
        scan_value_label(
            ui,
            render_cache,
            overall_verdict_label(payload.output.overall),
            overall_verdict_emphasis(payload.output.overall),
        )
        .on_hover_ui(|ui| {
            cached_label(ui, render_cache, "overall");
            cached_label(
                ui,
                render_cache,
                "Overall protocol verdict synthesized from usefulness, redundancy, and recoverability.",
            );
            ui.separator();
            cached_label(ui, render_cache, "LLM synthesis");
            cached_hover_monospace_block(
                ui,
                render_cache,
                payload.output.synthesis_rationale.as_str(),
            );
        });
        scan_value_label(
            ui,
            render_cache,
            confidence_label(payload.output.overall_confidence),
            confidence_emphasis(payload.output.overall_confidence),
        )
        .on_hover_text("Overall confidence assigned by the protocol review.");
        scan_value_label(
            ui,
            render_cache,
            call_review_failure_label(payload),
            failure_emphasis(payload),
        )
        .on_hover_text("Failure state for the focal call or calls in its review scope.");
        let mut latency_buffer = itoa::Buffer::new();
        cached_monospace_label(ui, render_cache, latency_buffer.format(focal.latency_ms))
            .on_hover_text("Protocol NeighborhoodCall.latency_ms for the focal tool call.");
        cached_monospace_label(ui, render_cache, "ms");
        cached_label(ui, render_cache, "scope");
        let mut scope_buffer = itoa::Buffer::new();
        cached_monospace_label(
            ui,
            render_cache,
            scope_buffer.format(payload.output.packet.total_calls_in_scope),
        )
        .on_hover_text("Number of calls included in the local analysis scope.");
        if clearable
            && ui
                .small_button("Clear")
                .on_hover_text("Clear the selected Eval & Protocol item.")
                .clicked()
        {
            clear_selected_eval_protocol_call_review(ui);
        }
    });
}

fn render_tool_call_review_detail(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    artifact_key: &str,
    artifact: &ploke_records::protocol::Artifact,
    payload: &ploke_records::protocol::ToolCallReviewPayload,
    clearable: bool,
) {
    render_tool_call_review_detail_header(ui, render_cache, payload, clearable);
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("artifact provenance")
            .id_salt(("tool-call-review-provenance", artifact_key))
            .default_open(false),
        |ui| {
            render_protocol_artifact_coordinate(ui, render_cache, artifact_key, artifact);
        },
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("packet")
            .id_salt(("tool-call-review-packet", artifact_key))
            .default_open(false),
        |ui| {
            render_local_analysis_packet(ui, render_cache, &payload.output.packet);
        },
    );
    render_local_analysis_signals(ui, render_cache, &payload.output.signals);
    render_local_analysis_assessment(ui, render_cache, &payload.output);
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("reviewed neighborhood")
            .id_salt(("tool-call-review-neighborhood", artifact_key))
            .default_open(false),
        |ui| {
            for call in &payload.output.packet.calls {
                render_protocol_call_row(
                    ui,
                    render_cache,
                    ("tool-call-review-call", artifact_key),
                    call.index,
                    call.turn,
                    call.tool_name.as_str(),
                    call.tool_kind,
                    call.failed,
                    call.latency_ms,
                    call.summary.as_str(),
                    call.args_preview.as_str(),
                    call.result_preview.as_str(),
                    call.search_term.as_deref(),
                    call.path_hint.as_deref(),
                );
            }
        },
    );
}

fn render_tool_call_segment_review_artifact(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    review_index: usize,
    artifact_key: &str,
    artifact: &ploke_records::protocol::Artifact,
    payload: &ploke_records::protocol::ToolCallSegmentReviewPayload,
) {
    let segment = &payload.input.segment;
    let title = format!(
        "segment {} {:?} {:?}",
        segment.segment_index, segment.label, payload.output.overall
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt(("tool-call-segment-review", artifact_key, review_index))
            .default_open(false),
        |ui| {
            render_protocol_artifact_coordinate(ui, render_cache, artifact_key, artifact);
            ui.separator();
            cached_kv_usize(ui, render_cache, "segment", segment.segment_index);
            cached_kv_usize(ui, render_cache, "start call", segment.start_index);
            cached_kv_usize(ui, render_cache, "end call", segment.end_index);
            cached_kv_debug(ui, render_cache, "status", segment.status);
            cached_kv_debug(ui, render_cache, "label", segment.label);
            cached_kv_debug(ui, render_cache, "confidence", segment.confidence);
            render_copyable_text_preview(
                ui,
                render_cache,
                ("segment-review-rationale", artifact_key, review_index),
                "segment rationale",
                segment.rationale.as_str(),
            );
            render_segmentation_coverage(ui, render_cache, &payload.input.coverage);
            render_local_analysis_packet(ui, render_cache, &payload.output.packet);
            render_local_analysis_signals(ui, render_cache, &payload.output.signals);
            render_local_analysis_assessment(ui, render_cache, &payload.output);
            show_inspector_collapsing(
                ui,
                egui::CollapsingHeader::new("segment calls").default_open(false),
                |ui| {
                    for call in &segment.calls {
                        render_protocol_call_row(
                            ui,
                            render_cache,
                            ("tool-call-segment-review-call", artifact_key),
                            call.index,
                            call.turn,
                            call.tool_name.as_str(),
                            call.tool_kind,
                            call.failed,
                            call.latency_ms,
                            call.summary.as_str(),
                            call.args_preview.as_str(),
                            call.result_preview.as_str(),
                            call.search_term.as_deref(),
                            call.path_hint.as_deref(),
                        );
                    }
                },
            );
        },
    );
}

fn render_intent_segmentation_artifact(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    artifact_index: usize,
    artifact_key: &str,
    artifact: &ploke_records::protocol::Artifact,
    payload: &ploke_records::protocol::IntentSegmentationPayload,
) {
    let title = format!(
        "segmentation {} calls -> {} segments",
        payload.output.coverage.total_calls,
        payload.output.segments.len()
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((
                "tool-call-intent-segmentation",
                artifact_key,
                artifact_index,
            ))
            .default_open(false),
        |ui| {
            render_protocol_artifact_coordinate(ui, render_cache, artifact_key, artifact);
            render_segmentation_coverage(ui, render_cache, &payload.output.coverage);
            render_copyable_text_preview(
                ui,
                render_cache,
                ("intent-overall-rationale", artifact_key, artifact_index),
                "overall rationale",
                payload.output.overall_rationale.as_str(),
            );
            for segment in &payload.output.segments {
                let title = format!(
                    "segment {} {:?} calls {}..{}",
                    segment.segment_index, segment.label, segment.start_index, segment.end_index
                );
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new(title)
                        .id_salt(("intent-segment", artifact_key, segment.segment_index))
                        .default_open(false),
                    |ui| {
                        cached_kv_debug(ui, render_cache, "status", segment.status);
                        cached_kv_debug(ui, render_cache, "confidence", segment.confidence);
                        render_protocol_turn_span(ui, render_cache, segment.turns.as_slice());
                        render_copyable_text_preview(
                            ui,
                            render_cache,
                            (
                                "intent-segment-rationale",
                                artifact_key,
                                segment.segment_index,
                            ),
                            "rationale",
                            segment.rationale.as_str(),
                        );
                        for call in &segment.calls {
                            render_protocol_call_row(
                                ui,
                                render_cache,
                                ("intent-segment-call", artifact_key, segment.segment_index),
                                call.index,
                                call.turn,
                                call.tool_name.as_str(),
                                call.tool_kind,
                                call.failed,
                                call.latency_ms,
                                call.summary.as_str(),
                                call.args_preview.as_str(),
                                call.result_preview.as_str(),
                                call.search_term.as_deref(),
                                call.path_hint.as_deref(),
                            );
                        }
                    },
                );
            }
        },
    );
}

fn render_local_analysis_packet(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    packet: &ploke_protocol::LocalAnalysisPacket,
) {
    cached_kv_id(ui, render_cache, "target", packet.target_id.as_str());
    cached_kv_debug(ui, render_cache, "target kind", packet.target_kind);
    cached_kv_usize(ui, render_cache, "scope calls", packet.total_calls_in_scope);
    cached_kv_usize(ui, render_cache, "run calls", packet.total_calls_in_run);
    if let Some(index) = packet.focal_call_index {
        cached_kv_usize(ui, render_cache, "focal call", index);
    }
    if let Some(index) = packet.segment_index {
        cached_kv_usize(ui, render_cache, "segment", index);
    }
    if let Some(status) = packet.segment_status {
        cached_kv_debug(ui, render_cache, "segment status", status);
    }
    if let Some(label) = packet.segment_label {
        cached_kv_debug(ui, render_cache, "segment label", label);
    }
    render_protocol_turn_span(ui, render_cache, packet.turn_span.as_slice());
    render_copyable_text_preview(
        ui,
        render_cache,
        ("local-analysis-scope", packet.target_id.as_str()),
        "scope",
        packet.scope_summary.as_str(),
    );
}

fn render_local_analysis_signals(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    signals: &ploke_protocol::LocalAnalysisSignals,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("signals").default_open(false),
        |ui| {
            cached_kv_usize(ui, render_cache, "turns", signals.scope_turn_count);
            cached_kv_usize(
                ui,
                render_cache,
                "distinct tools",
                signals.distinct_tool_count,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "repeated tools",
                signals.repeated_tool_name_count,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "search calls",
                signals.search_calls_in_scope,
            );
            cached_kv_usize(ui, render_cache, "read calls", signals.read_calls_in_scope);
            cached_kv_usize(
                ui,
                render_cache,
                "browse calls",
                signals.browse_calls_in_scope,
            );
            cached_kv_usize(ui, render_cache, "edit calls", signals.edit_calls_in_scope);
            cached_kv_usize(
                ui,
                render_cache,
                "execute calls",
                signals.execute_calls_in_scope,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "failed calls",
                signals.failed_calls_in_scope,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "similar searches",
                signals.similar_search_neighbors,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "directory pivots",
                signals.directory_pivots,
            );
            cached_kv_optional_usize(
                ui,
                render_cache,
                "labeled segments",
                signals.labeled_segments_in_source,
            );
            cached_kv_optional_usize(
                ui,
                render_cache,
                "ambiguous segments",
                signals.ambiguous_segments_in_source,
            );
            cached_kv_optional_usize(
                ui,
                render_cache,
                "uncovered calls",
                signals.uncovered_calls_in_source,
            );
            if !signals.candidate_concerns.is_empty() {
                cached_label(ui, render_cache, "concerns");
                for concern in &signals.candidate_concerns {
                    cached_monospace_label(ui, render_cache, format!("{concern:?}").as_str());
                }
            }
        },
    );
}

fn render_local_analysis_assessment(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    assessment: &ploke_protocol::LocalAnalysisAssessment,
) {
    cached_kv_debug(ui, render_cache, "overall", assessment.overall);
    cached_kv_debug(
        ui,
        render_cache,
        "overall confidence",
        assessment.overall_confidence,
    );
    ui.separator();
    cached_kv_debug(
        ui,
        render_cache,
        "usefulness",
        assessment.usefulness.verdict,
    );
    cached_kv_debug(
        ui,
        render_cache,
        "redundancy",
        assessment.redundancy.verdict,
    );
    cached_kv_debug(
        ui,
        render_cache,
        "recoverability",
        assessment.recoverability.verdict,
    );
    render_copyable_text_preview(
        ui,
        render_cache,
        ("assessment-synthesis", assessment.packet.target_id.as_str()),
        "synthesis raw",
        assessment.synthesis_rationale.as_str(),
    );
    render_protocol_judgment(
        ui,
        render_cache,
        "usefulness",
        assessment.usefulness.verdict,
        assessment.usefulness.confidence,
        assessment.usefulness.rationale.as_str(),
    );
    render_protocol_judgment(
        ui,
        render_cache,
        "redundancy",
        assessment.redundancy.verdict,
        assessment.redundancy.confidence,
        assessment.redundancy.rationale.as_str(),
    );
    render_protocol_judgment(
        ui,
        render_cache,
        "recoverability",
        assessment.recoverability.verdict,
        assessment.recoverability.confidence,
        assessment.recoverability.rationale.as_str(),
    );
}

fn render_protocol_judgment(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    label: &str,
    verdict: impl std::fmt::Debug,
    confidence: impl std::fmt::Debug,
    rationale: &str,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(label).default_open(false),
        |ui| {
            cached_kv_debug(ui, render_cache, "verdict", verdict);
            cached_kv_debug(ui, render_cache, "confidence", confidence);
            render_copyable_text_preview(
                ui,
                render_cache,
                ("judgment-rationale", label, rationale),
                "rationale",
                rationale,
            );
        },
    );
}

fn render_segmentation_coverage(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    coverage: &ploke_protocol::SegmentationCoverage,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("coverage").default_open(false),
        |ui| {
            cached_kv_usize(ui, render_cache, "total calls", coverage.total_calls);
            cached_kv_usize(
                ui,
                render_cache,
                "labeled segments",
                coverage.labeled_segments,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "ambiguous segments",
                coverage.ambiguous_segments,
            );
            cached_kv_usize(ui, render_cache, "labeled calls", coverage.labeled_calls);
            cached_kv_usize(
                ui,
                render_cache,
                "ambiguous calls",
                coverage.ambiguous_calls,
            );
            cached_kv_usize(
                ui,
                render_cache,
                "uncovered calls",
                coverage.uncovered_calls,
            );
        },
    );
}

fn render_protocol_turn_span(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turns: &[u32],
) {
    if turns.is_empty() {
        cached_kv_id(ui, render_cache, "turns", "none");
        return;
    }

    ui.horizontal(|ui| {
        cached_label(ui, render_cache, "turns");
        let mut buffer = itoa::Buffer::new();
        for turn in turns {
            cached_monospace_label(ui, render_cache, buffer.format(*turn));
        }
    });
}

fn render_protocol_call_row(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_salt: impl std::hash::Hash,
    index: usize,
    turn: u32,
    tool_name: &str,
    tool_kind: impl std::fmt::Debug,
    failed: bool,
    latency_ms: u64,
    summary: &str,
    args_preview: &str,
    result_preview: &str,
    search_term: Option<&str>,
    path_hint: Option<&str>,
) {
    let status = if failed { "failed" } else { "ok" };
    let title = format!("[{index}] {tool_name} {status}");
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt(("protocol-call-row", id_salt, index))
            .default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "tool", tool_name);
            cached_kv_u32(ui, render_cache, "turn", turn);
            cached_kv_debug(ui, render_cache, "kind", tool_kind);
            cached_kv_bool(ui, render_cache, "failed", failed);
            cached_kv_u64(ui, render_cache, "latency ms", latency_ms);
            if let Some(search_term) = search_term {
                cached_kv_text(ui, render_cache, "search term", search_term);
            }
            if let Some(path_hint) = path_hint {
                cached_kv_path(ui, render_cache, "path hint", path_hint);
            }
            render_copyable_text_preview(
                ui,
                render_cache,
                ("protocol-call-summary", tool_name, index, summary),
                "summary",
                summary,
            );
            render_protocol_preview_payload(
                ui,
                render_cache,
                ("protocol-call-args-preview", tool_name, index),
                "args preview",
                ProtocolPreviewKind::Arguments,
                tool_name,
                args_preview,
            );
            render_protocol_preview_payload(
                ui,
                render_cache,
                ("protocol-call-result-preview", tool_name, index),
                "result preview",
                ProtocolPreviewKind::Result,
                tool_name,
                result_preview,
            );
        },
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtocolPreviewKind {
    Arguments,
    Result,
}

fn render_protocol_preview_payload(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    label: &'static str,
    kind: ProtocolPreviewKind,
    tool_name: &str,
    preview: &str,
) {
    let preview_id = egui::Id::new(("protocol-preview-payload", label, id_source, preview));
    let can_show_fields = protocol_preview_has_typed_fields(render_cache, kind, tool_name, preview);
    let mode_id = ui.make_persistent_id(("protocol-preview-mode", preview_id));
    let mut show_fields = ui.data(|data| data.get_temp::<bool>(mode_id).unwrap_or(can_show_fields));
    if !can_show_fields {
        show_fields = false;
    }

    let copy_value = CopyableText::new(preview);
    ui.horizontal_wrapped(|ui| {
        cached_label(ui, render_cache, label);
        if ui
            .selectable_label(!show_fields, "Raw")
            .on_hover_text("Show raw preview text.")
            .clicked()
        {
            show_fields = false;
            ui.data_mut(|data| data.insert_temp(mode_id, show_fields));
        }
        if can_show_fields
            && ui
                .selectable_label(show_fields, "Fields")
                .on_hover_text("Show deserialized fields.")
                .clicked()
        {
            show_fields = true;
            ui.data_mut(|data| data.insert_temp(mode_id, show_fields));
        }
        id_display::copy_button(ui, &copy_value);
    });

    ui.indent(("protocol-preview-body", preview_id), |ui| {
        if show_fields {
            render_protocol_preview_typed_fields(ui, render_cache, kind, tool_name, preview);
        } else {
            render_copyable_multiline_body(ui, render_cache, preview, preview);
            if kind == ProtocolPreviewKind::Result && !can_show_fields {
                render_result_preview_fields_note(ui, render_cache);
            }
        }
    });
}

fn render_result_preview_fields_note(ui: &mut egui::Ui, render_cache: &mut InspectorRenderCache) {
    ui.horizontal_wrapped(|ui| {
        cached_label(ui, render_cache, "fields");
        cached_wrapped_monospace_label(
            ui,
            render_cache,
            "unavailable: this protocol artifact stores a truncated result_preview, not the full tool result. The typed Fields view needs the full run-record result or a primary-branch protocol data fix.",
        );
    });
}

fn protocol_preview_has_typed_fields(
    render_cache: &mut InspectorRenderCache,
    kind: ProtocolPreviewKind,
    tool_name: &str,
    preview: &str,
) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        match kind {
            ProtocolPreviewKind::Arguments => render_cache
                .tool_arguments("protocol-preview", tool_name, preview)
                .decoded()
                .is_some(),
            ProtocolPreviewKind::Result => render_cache
                .tool_result("protocol-preview", tool_name, preview)
                .decoded()
                .is_some(),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (render_cache, kind, tool_name, preview);
        false
    }
}

fn render_protocol_preview_typed_fields(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    kind: ProtocolPreviewKind,
    tool_name: &str,
    preview: &str,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        match kind {
            ProtocolPreviewKind::Arguments => {
                let decoded = render_cache.tool_arguments("protocol-preview", tool_name, preview);
                render_decoded_tool_arguments(ui, render_cache, decoded.as_ref());
            }
            ProtocolPreviewKind::Result => {
                let decoded = render_cache.tool_result("protocol-preview", tool_name, preview);
                render_decoded_tool_result(ui, render_cache, decoded.as_ref());
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (kind, tool_name, preview);
        cached_kv_id(ui, render_cache, "decode", "native_only");
    }
}

fn render_copyable_multiline_body(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    copy_text: &str,
    display_text: &str,
) {
    let copy_value = CopyableText::new(copy_text);
    let response = ui
        .add(
            egui::Label::new(render_cache.wrapped_monospace_galley(ui, display_text))
                .sense(egui::Sense::click()),
        )
        .on_hover_text(copy_value.hover_text(false, false));
    id_display::attach_copy_context_menu(&response, &copy_value);
}

fn render_run_level_llm_trace_for_graph(
    ui: &mut egui::Ui,
    graph: &Graph,
    render_cache: &mut InspectorRenderCache,
) {
    let dashboard = EvalProtocolDashboard::from_graph(graph);
    let Some(run_records) = dashboard.run_records() else {
        cached_kv_id(ui, render_cache, "run llm trace", "not_available");
        return;
    };
    if run_records.index.is_empty() {
        cached_kv_id(ui, render_cache, "run llm trace", "none");
        return;
    }

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Run LLM Trace").default_open(true),
        |ui| {
            cached_kv_usize(ui, render_cache, "records", run_records.index.len());
            cached_kv_optional_usize(
                ui,
                render_cache,
                "turns",
                dashboard.run_records_total_turn_count(),
            );
            for (record_index, (record_key, record)) in run_records.index.iter().enumerate() {
                render_run_record_llm_trace_for_record(
                    ui,
                    render_cache,
                    "agent-trace",
                    record_index,
                    record_key,
                    record,
                );
            }
        },
    );
}

fn render_run_record_llm_trace_for_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_index: usize,
    record_key: &str,
    record: &ploke_records::run_record::RunRecord,
) {
    let title = format!(
        "{} {}",
        record.metadata.benchmark.instance_id, record.manifest_id
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((scope, "run-record-llm", record_key, record_index))
            .default_open(record_index == 0),
        |ui| {
            cached_kv_path(ui, render_cache, "record", record_key);
            cached_kv_id(ui, render_cache, "manifest", record.manifest_id.as_str());
            cached_kv_id(
                ui,
                render_cache,
                "instance",
                record.metadata.benchmark.instance_id.as_str(),
            );
            if let Some(model) = record.metadata.agent.model_id.as_deref() {
                cached_kv_id(ui, render_cache, "model", model);
            }
            if let Some(provider) = record.metadata.agent.provider.as_deref() {
                cached_kv_id(ui, render_cache, "provider", provider);
            }
            cached_kv_usize(ui, render_cache, "turns", record.phases.agent_turns.len());
            for (turn_index, turn) in record.phases.agent_turns.iter().enumerate() {
                render_run_record_turn_llm_trace(
                    ui,
                    render_cache,
                    scope,
                    record_key,
                    turn_index,
                    record,
                    turn,
                    true,
                );
            }
        },
    );
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::INSPECTOR_AGENT_TRACE_LLM_TRACE)]
fn render_run_record_turn_llm_trace(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    record: &ploke_records::run_record::RunRecord,
    turn: &ploke_records::run_record::TurnRecord,
    include_tool_steps: bool,
) {
    let title = format!("turn {} {:?}", turn.turn_number, turn.outcome);
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((scope, "run-record-turn-llm", record_key, turn_index))
            .default_open(turn_index == 0),
        |ui| {
            cached_kv_u32(ui, render_cache, "turn", turn.turn_number);
            cached_kv_text(ui, render_cache, "started", turn.started_at.as_str());
            cached_kv_text(ui, render_cache, "ended", turn.ended_at.as_str());
            cached_kv_i64(ui, render_cache, "db micros", turn.db_timestamp_micros);
            cached_kv_id(
                ui,
                render_cache,
                "outcome",
                turn_outcome_label(&turn.outcome),
            );
            if let Some(count) = turn_outcome_tool_count(&turn.outcome) {
                cached_kv_usize(ui, render_cache, "outcome tools", count);
            }
            if let Some(message) = turn_outcome_error(&turn.outcome) {
                cached_kv_text(ui, render_cache, "outcome error", message);
            }
            if let Some(elapsed) = turn_outcome_elapsed_secs(&turn.outcome) {
                cached_kv_u64(ui, render_cache, "elapsed secs", elapsed);
            }
            if let Some(request) = turn.llm_request.as_ref() {
                render_llm_request_record(ui, render_cache, scope, record_key, turn_index, request);
            } else {
                cached_kv_id(ui, render_cache, "llm request", "missing");
            }
            if let Some(response) = turn.llm_response.as_ref() {
                render_llm_response_record(
                    ui,
                    render_cache,
                    scope,
                    "turn-response",
                    record_key,
                    turn_index,
                    response,
                );
            } else {
                cached_kv_id(ui, render_cache, "llm response", "missing");
            }
            render_agent_turn_artifact_trace(ui, render_cache, scope, record_key, turn_index, turn);
            if include_tool_steps {
                render_run_record_tool_steps(ui, render_cache, turn.tool_calls.as_slice());
            }
            if let Some(model) = record.metadata.agent.model_id.as_deref() {
                cached_kv_id(ui, render_cache, "record model", model);
            }
        },
    );
}

fn render_llm_request_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    request: &ploke_records::run_record::ChatRequestRecord,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("llm request").default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "model", request.model.as_str());
            cached_kv_usize(ui, render_cache, "messages", request.messages.len());
            for (message_index, message) in request.messages.iter().enumerate() {
                render_request_message_record(
                    ui,
                    render_cache,
                    scope,
                    record_key,
                    turn_index,
                    message_index,
                    message,
                );
            }
        },
    );
}

fn render_request_message_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    message_index: usize,
    message: &ploke_records::agent_turn::RequestMessageRecord,
) {
    let title = format!(
        "message {} {}",
        message_index + 1,
        request_role_label(message.role)
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((scope, "llm-message", record_key, turn_index, message_index))
            .default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "role", request_role_label(message.role));
            if let Some(tool_call_id) = message.tool_call_id.as_deref() {
                cached_kv_id(ui, render_cache, "tool call id", tool_call_id);
            }
            cached_label(ui, render_cache, "content");
            render_cached_code_block(
                ui,
                render_cache,
                (
                    scope,
                    "llm-message-content",
                    record_key,
                    turn_index,
                    message_index,
                ),
                message.content.as_str(),
            );
            if let Some(tool_calls) = message.tool_calls.as_ref() {
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new("provider tool calls").default_open(false),
                    |ui| {
                        cached_kv_usize(ui, render_cache, "tool calls", tool_calls.len());
                        for (tool_index, tool_call) in tool_calls.iter().enumerate() {
                            render_provider_tool_call_record(
                                ui,
                                render_cache,
                                scope,
                                record_key,
                                turn_index,
                                message_index,
                                tool_index,
                                tool_call,
                            );
                        }
                    },
                );
            }
        },
    );
}

fn render_provider_tool_call_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    message_index: usize,
    tool_index: usize,
    tool_call: &ploke_records::agent_turn::ProviderToolCallRecord,
) {
    let title = format!(
        "tool call {} {}",
        tool_index + 1,
        tool_call.function.name.as_str()
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((
                scope,
                "provider-tool-call",
                record_key,
                turn_index,
                message_index,
                tool_index,
            ))
            .default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "call id", tool_call.call_id.as_str());
            cached_kv_id(ui, render_cache, "tool", tool_call.function.name.as_str());
            cached_label(ui, render_cache, "arguments");
            render_cached_code_block(
                ui,
                render_cache,
                (
                    scope,
                    "provider-tool-call-arguments",
                    record_key,
                    turn_index,
                    message_index,
                    tool_index,
                ),
                tool_call.function.arguments.as_str(),
            );
        },
    );
}

fn render_llm_response_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    id_kind: &'static str,
    record_key: &str,
    turn_index: usize,
    response: &ploke_records::agent_turn::LlmResponseRecord,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("llm response")
            .id_salt((scope, id_kind, record_key, turn_index))
            .default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "model", response.model.as_str());
            if let Some(reason) = response.finish_reason.as_ref() {
                cached_kv_id(
                    ui,
                    render_cache,
                    "finish",
                    response_finish_reason_label(reason),
                );
            }
            if let Some(usage) = response.usage {
                render_token_usage(ui, render_cache, usage);
            }
            if let Some(metadata) = response.metadata.as_ref() {
                cached_kv_text(ui, render_cache, "cost", format_f64(metadata.cost).as_str());
                cached_kv_text(
                    ui,
                    render_cache,
                    "tokens/sec",
                    format!("{:.3}", metadata.performance.tokens_per_second).as_str(),
                );
            }
            cached_label(ui, render_cache, "content");
            render_cached_code_block(
                ui,
                render_cache,
                (scope, id_kind, "content", record_key, turn_index),
                response.content.as_str(),
            );
        },
    );
}

fn render_agent_turn_artifact_trace(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    turn: &ploke_records::run_record::TurnRecord,
) {
    let Some(artifact) = turn.agent_turn_artifact.as_ref() else {
        cached_kv_id(ui, render_cache, "agent-turn artifact", "not_recorded");
        return;
    };

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("agent-turn artifact")
            .id_salt((scope, "agent-turn-artifact", record_key, turn_index))
            .default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "task", artifact.task_id.as_str());
            cached_kv_id(
                ui,
                render_cache,
                "selected model",
                artifact.selected_model.as_str(),
            );
            cached_kv_id(
                ui,
                render_cache,
                "user message",
                artifact.user_message_id.as_str(),
            );
            cached_kv_usize(ui, render_cache, "events", artifact.events.len());
            if let Some(prompt_debug) = artifact.prompt_debug.as_deref() {
                cached_label(ui, render_cache, "prompt debug");
                render_cached_code_block(
                    ui,
                    render_cache,
                    (scope, "agent-turn-prompt-debug", record_key, turn_index),
                    prompt_debug,
                );
            }
            if !artifact.llm_prompt.is_empty() {
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new("artifact llm prompt").default_open(false),
                    |ui| {
                        cached_kv_usize(ui, render_cache, "messages", artifact.llm_prompt.len());
                        for (message_index, message) in artifact.llm_prompt.iter().enumerate() {
                            render_request_message_record(
                                ui,
                                render_cache,
                                scope,
                                record_key,
                                turn_index,
                                message_index,
                                message,
                            );
                        }
                    },
                );
            }
            if let Some(response) = artifact.llm_response.as_deref() {
                cached_label(ui, render_cache, "artifact llm response");
                render_cached_code_block(
                    ui,
                    render_cache,
                    (scope, "agent-turn-llm-response", record_key, turn_index),
                    response,
                );
            }
            if let Some(final_message) = artifact.final_assistant_message.as_ref() {
                render_message_snapshot(ui, render_cache, "final assistant", final_message);
            }
            if let Some(terminal) = artifact.terminal_record.as_ref() {
                render_turn_finished_record(ui, render_cache, "terminal", terminal);
            }
            render_patch_artifact_details(
                ui,
                render_cache,
                scope,
                "agent-turn",
                record_key,
                turn_index,
                &artifact.patch_artifact,
            );
            render_observed_turn_events(
                ui,
                render_cache,
                scope,
                record_key,
                turn_index,
                artifact.events.as_slice(),
            );
        },
    );
}

fn render_observed_turn_events(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    events: &[ploke_records::agent_turn::ObservedTurnEventRecord],
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("observed events").default_open(false),
        |ui| {
            cached_kv_usize(ui, render_cache, "events", events.len());
            for (event_index, event) in events.iter().enumerate() {
                render_observed_turn_event(
                    ui,
                    render_cache,
                    scope,
                    record_key,
                    turn_index,
                    event_index,
                    event,
                );
            }
        },
    );
}

fn render_observed_turn_event(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    record_key: &str,
    turn_index: usize,
    event_index: usize,
    event: &ploke_records::agent_turn::ObservedTurnEventRecord,
) {
    use ploke_records::agent_turn::ObservedTurnEventRecord;

    let title = format!(
        "event {} {}",
        event_index + 1,
        observed_turn_event_label(event)
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((
                scope,
                "observed-turn-event",
                record_key,
                turn_index,
                event_index,
            ))
            .default_open(false),
        |ui| match event {
            ObservedTurnEventRecord::DebugCommand(command) => {
                render_cached_code_block(
                    ui,
                    render_cache,
                    (scope, "debug-command", record_key, turn_index, event_index),
                    command,
                );
            }
            ObservedTurnEventRecord::LlmEvent(event) => {
                render_cached_code_block(
                    ui,
                    render_cache,
                    (scope, "llm-event", record_key, turn_index, event_index),
                    event,
                );
            }
            ObservedTurnEventRecord::LlmResponse(response) => {
                render_llm_response_record(
                    ui,
                    render_cache,
                    scope,
                    "event-llm-response",
                    record_key,
                    turn_index,
                    response,
                );
            }
            ObservedTurnEventRecord::ToolRequested(request) => {
                cached_kv_id(ui, render_cache, "request", request.request_id.as_str());
                cached_kv_id(ui, render_cache, "parent", request.parent_id.as_str());
                cached_kv_id(ui, render_cache, "call", request.call_id.as_str());
                cached_kv_id(ui, render_cache, "tool", request.tool.as_str());
                cached_label(ui, render_cache, "arguments");
                render_cached_code_block(
                    ui,
                    render_cache,
                    (
                        scope,
                        "event-tool-request",
                        record_key,
                        turn_index,
                        event_index,
                    ),
                    request.arguments.as_str(),
                );
            }
            ObservedTurnEventRecord::ToolCompleted(completed) => {
                cached_kv_id(ui, render_cache, "request", completed.request_id.as_str());
                cached_kv_id(ui, render_cache, "parent", completed.parent_id.as_str());
                cached_kv_id(ui, render_cache, "call", completed.call_id.as_str());
                cached_kv_id(ui, render_cache, "tool", completed.tool.as_str());
                cached_kv_u64(ui, render_cache, "latency ms", completed.latency_ms);
                cached_label(ui, render_cache, "content");
                render_cached_code_block(
                    ui,
                    render_cache,
                    (
                        scope,
                        "event-tool-completed",
                        record_key,
                        turn_index,
                        event_index,
                    ),
                    completed.content.as_str(),
                );
            }
            ObservedTurnEventRecord::ToolFailed(failed) => {
                cached_kv_id(ui, render_cache, "request", failed.request_id.as_str());
                cached_kv_id(ui, render_cache, "parent", failed.parent_id.as_str());
                cached_kv_id(ui, render_cache, "call", failed.call_id.as_str());
                if let Some(tool) = failed.tool.as_deref() {
                    cached_kv_id(ui, render_cache, "tool", tool);
                }
                cached_kv_u64(ui, render_cache, "latency ms", failed.latency_ms);
                cached_label(ui, render_cache, "error");
                render_cached_code_block(
                    ui,
                    render_cache,
                    (
                        scope,
                        "event-tool-failed",
                        record_key,
                        turn_index,
                        event_index,
                    ),
                    failed.error.as_str(),
                );
            }
            ObservedTurnEventRecord::MessageUpdated(message) => {
                render_message_snapshot(ui, render_cache, "message", message);
            }
            ObservedTurnEventRecord::TurnFinished(finished) => {
                render_turn_finished_record(ui, render_cache, "finished", finished);
            }
        },
    );
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::INSPECTOR_PATCH_GENERATION)]
fn render_run_level_patch_generation_for_graph(
    ui: &mut egui::Ui,
    graph: &Graph,
    render_cache: &mut InspectorRenderCache,
) {
    let dashboard = EvalProtocolDashboard::from_graph(graph);
    let Some(run_records) = dashboard.run_records() else {
        cached_kv_id(ui, render_cache, "patch generation", "not_available");
        return;
    };
    if run_records.index.is_empty() {
        cached_kv_id(ui, render_cache, "patch generation", "none");
        return;
    }

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Run Patch Generation").default_open(true),
        |ui| {
            cached_kv_usize(ui, render_cache, "records", run_records.index.len());
            for (record_index, (record_key, record)) in run_records.index.iter().enumerate() {
                render_patch_generation_record(ui, render_cache, record_index, record_key, record);
            }
        },
    );
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::INSPECTOR_PATCH_GENERATION_RECORD)]
fn render_patch_generation_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    record_index: usize,
    record_key: &str,
    record: &ploke_records::run_record::RunRecord,
) {
    let title = format!(
        "{} {} patch trace",
        record.metadata.benchmark.instance_id, record.manifest_id
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt(("patch-generation-record", record_key, record_index))
            .default_open(record_index == 0),
        |ui| {
            cached_kv_path(ui, render_cache, "record", record_key);
            if let Some(patch_phase) = record.phases.patch.as_ref() {
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new("patch phase").default_open(true),
                    |ui| {
                        cached_kv_text(
                            ui,
                            render_cache,
                            "started",
                            patch_phase.started_at.as_str(),
                        );
                        cached_kv_text(ui, render_cache, "ended", patch_phase.ended_at.as_str());
                        render_patch_artifact_details(
                            ui,
                            render_cache,
                            "patch-generation",
                            "patch-phase",
                            record_key,
                            0,
                            &patch_phase.patch_artifact,
                        );
                        if let Some(diff) = patch_phase.diff.as_deref() {
                            cached_label(ui, render_cache, "diff");
                            render_cached_code_block(
                                ui,
                                render_cache,
                                ("patch-generation-diff", record_key),
                                diff,
                            );
                        }
                    },
                );
            } else {
                cached_kv_id(ui, render_cache, "patch phase", "not_recorded");
            }

            let mut patch_turns = 0usize;
            for (turn_index, turn) in record.phases.agent_turns.iter().enumerate() {
                if turn.agent_turn_artifact.is_none() {
                    continue;
                }
                patch_turns += 1;
                render_run_record_turn_llm_trace(
                    ui,
                    render_cache,
                    "patch-generation",
                    record_key,
                    turn_index,
                    record,
                    turn,
                    true,
                );
            }
            if patch_turns == 0 {
                cached_kv_id(ui, render_cache, "agent patch artifacts", "not_recorded");
            }
        },
    );
}

fn render_patch_artifact_details(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    id_kind: &'static str,
    record_key: &str,
    turn_index: usize,
    artifact: &ploke_records::agent_turn::PatchArtifactRecord,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("patch artifact")
            .id_salt((scope, id_kind, "patch-artifact", record_key, turn_index))
            .default_open(false),
        |ui| {
            cached_kv_bool(ui, render_cache, "applied", artifact.applied);
            cached_kv_bool(
                ui,
                render_cache,
                "all proposals applied",
                artifact.all_proposals_applied,
            );
            cached_kv_bool(
                ui,
                render_cache,
                "any expected changed",
                artifact.any_expected_file_changed,
            );
            cached_kv_bool(
                ui,
                render_cache,
                "all expected changed",
                artifact.all_expected_files_changed,
            );
            render_patch_proposals(
                ui,
                render_cache,
                scope,
                id_kind,
                record_key,
                turn_index,
                "edit proposals",
                artifact.edit_proposals.as_slice(),
            );
            render_patch_proposals(
                ui,
                render_cache,
                scope,
                id_kind,
                record_key,
                turn_index,
                "create proposals",
                artifact.create_proposals.as_slice(),
            );
            render_expected_file_changes(
                ui,
                render_cache,
                scope,
                id_kind,
                record_key,
                turn_index,
                artifact.expected_file_changes.as_slice(),
            );
        },
    );
}

fn render_patch_proposals(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    id_kind: &'static str,
    record_key: &str,
    turn_index: usize,
    title: &'static str,
    proposals: &[ploke_records::agent_turn::ProposalSnapshotRecord],
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((scope, id_kind, title, record_key, turn_index))
            .default_open(false),
        |ui| {
            cached_kv_usize(ui, render_cache, "count", proposals.len());
            for (proposal_index, proposal) in proposals.iter().enumerate() {
                let proposal_title = format!("proposal {} {}", proposal_index + 1, proposal.status);
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new(proposal_title)
                        .id_salt((
                            scope,
                            id_kind,
                            title,
                            "proposal",
                            record_key,
                            turn_index,
                            proposal_index,
                        ))
                        .default_open(false),
                    |ui| {
                        cached_kv_id(ui, render_cache, "request", proposal.request_id.as_str());
                        cached_kv_id(ui, render_cache, "call", proposal.call_id.as_str());
                        cached_kv_id(ui, render_cache, "status", proposal.status.as_str());
                        cached_kv_id(ui, render_cache, "preview", proposal.preview_mode.as_str());
                        for file in &proposal.files {
                            cached_kv_path(ui, render_cache, "file", file.as_str());
                        }
                    },
                );
            }
        },
    );
}

fn render_expected_file_changes(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    id_kind: &'static str,
    record_key: &str,
    turn_index: usize,
    changes: &[ploke_records::agent_turn::ExpectedFileChangeRecord],
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("expected file changes")
            .id_salt((
                scope,
                id_kind,
                "expected-file-changes",
                record_key,
                turn_index,
            ))
            .default_open(false),
        |ui| {
            cached_kv_usize(ui, render_cache, "count", changes.len());
            for (change_index, change) in changes.iter().enumerate() {
                let title = format!("file {} {}", change_index + 1, bool_label(change.changed));
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new(title)
                        .id_salt((
                            scope,
                            id_kind,
                            "expected-file-change",
                            record_key,
                            turn_index,
                            change_index,
                        ))
                        .default_open(false),
                    |ui| {
                        cached_kv_path(ui, render_cache, "path", change.path.as_str());
                        cached_kv_bool(ui, render_cache, "existed before", change.existed_before);
                        cached_kv_bool(ui, render_cache, "exists after", change.exists_after);
                        cached_kv_bool(ui, render_cache, "changed", change.changed);
                        if let Some(sha) = change.before_sha256.as_deref() {
                            cached_kv_id(ui, render_cache, "before sha", sha);
                        }
                        if let Some(sha) = change.after_sha256.as_deref() {
                            cached_kv_id(ui, render_cache, "after sha", sha);
                        }
                    },
                );
            }
        },
    );
}

fn request_role_label(role: ploke_records::agent_turn::RequestRoleRecord) -> &'static str {
    match role {
        ploke_records::agent_turn::RequestRoleRecord::User => "user",
        ploke_records::agent_turn::RequestRoleRecord::Assistant => "assistant",
        ploke_records::agent_turn::RequestRoleRecord::System => "system",
        ploke_records::agent_turn::RequestRoleRecord::Tool => "tool",
    }
}

fn observed_turn_event_label(
    event: &ploke_records::agent_turn::ObservedTurnEventRecord,
) -> &'static str {
    match event {
        ploke_records::agent_turn::ObservedTurnEventRecord::DebugCommand(_) => "debug_command",
        ploke_records::agent_turn::ObservedTurnEventRecord::LlmEvent(_) => "llm_event",
        ploke_records::agent_turn::ObservedTurnEventRecord::LlmResponse(_) => "llm_response",
        ploke_records::agent_turn::ObservedTurnEventRecord::ToolRequested(_) => "tool_requested",
        ploke_records::agent_turn::ObservedTurnEventRecord::ToolCompleted(_) => "tool_completed",
        ploke_records::agent_turn::ObservedTurnEventRecord::ToolFailed(_) => "tool_failed",
        ploke_records::agent_turn::ObservedTurnEventRecord::MessageUpdated(_) => "message_updated",
        ploke_records::agent_turn::ObservedTurnEventRecord::TurnFinished(_) => "turn_finished",
    }
}

fn render_message_snapshot(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    label: &str,
    message: &ploke_records::agent_turn::MessageSnapshotRecord,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(label).default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "id", message.id.as_str());
            cached_kv_id(ui, render_cache, "kind", message.kind.as_str());
            cached_kv_id(ui, render_cache, "status", message.status.as_str());
            if let Some(tool_call_id) = message.tool_call_id.as_deref() {
                cached_kv_id(ui, render_cache, "tool call", tool_call_id);
            }
            cached_kv_usize(ui, render_cache, "content len", message.content_len);
            cached_label(ui, render_cache, "preview");
            cached_wrapped_monospace_label(ui, render_cache, message.content_preview.as_str());
        },
    );
}

fn render_turn_finished_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    label: &str,
    finished: &ploke_records::agent_turn::TurnFinishedRecord,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(label).default_open(false),
        |ui| {
            cached_kv_id(ui, render_cache, "session", finished.session_id.as_str());
            cached_kv_id(ui, render_cache, "request", finished.request_id.as_str());
            cached_kv_id(ui, render_cache, "parent", finished.parent_id.as_str());
            cached_kv_id(
                ui,
                render_cache,
                "assistant message",
                finished.assistant_message_id.as_str(),
            );
            cached_kv_id(ui, render_cache, "outcome", finished.outcome.as_str());
            if let Some(error_id) = finished.error_id.as_deref() {
                cached_kv_id(ui, render_cache, "error", error_id);
            }
            cached_kv_u32(ui, render_cache, "attempts", finished.attempts);
            cached_label(ui, render_cache, "summary");
            cached_wrapped_monospace_label(ui, render_cache, finished.summary.as_str());
        },
    );
}

fn render_patch_projection_counts(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    counts: &PatchProjectionCounts,
) {
    cached_kv_usize(
        ui,
        render_cache,
        "patch projection.not_recorded",
        counts.not_recorded,
    );
    cached_kv_usize(
        ui,
        render_cache,
        "patch projection.not_applicable",
        counts.not_applicable,
    );
    cached_kv_usize(ui, render_cache, "patch projection.passed", counts.passed);
    cached_kv_usize(ui, render_cache, "patch projection.failed", counts.failed);
    cached_kv_usize(ui, render_cache, "patch projection.not_run", counts.not_run);
}

fn evidence_state_label(state: EvidenceState) -> &'static str {
    match state {
        EvidenceState::Available => "available",
        EvidenceState::Missing => "missing",
        EvidenceState::NotApplicable => "not_applicable",
    }
}

fn closure_status_label<T>(value: &T) -> String
where
    T: serde::Serialize + std::fmt::Debug,
{
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{value:?}"))
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

pub(crate) fn render_parent_create_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
    open_state: InspectorOpenState,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    match sections.parent_create() {
        Some(slot) => render_parent_create(
            ui,
            graph,
            slot.resolve(graph),
            sections.run_records(),
            render_cache,
            open_state,
        ),
        None => kv(ui, "attempt", "not_available"),
    }
}

/// archaeology:run-record-branch-output
/// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_run_records")
)]
pub(crate) fn render_run_records_for_inspector(
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
        sections.run_records().iter().filter_map(|slot| {
            let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_RESOLVE_SLOT).entered();
            slot.resolve(graph)
        }),
    );
}

fn render_run_records<'a>(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    records: impl IntoIterator<Item = RunRecordInspection<'a>>,
) {
    let mut rendered = false;
    for record in records {
        let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_ROW).entered();
        rendered = true;
        ui.separator();
        run_record_kv_id(
            ui,
            render_cache,
            "arm",
            compared_run_arm_label(record.record_ref.arm),
        );
        run_record_kv_id(
            ui,
            render_cache,
            "instance",
            record.record_ref.instance_id.as_str(),
        );
        run_record_kv_path(
            ui,
            render_cache,
            "record",
            record
                .record_ref
                .record_path
                .to_str()
                .unwrap_or("non_utf8_path"),
        );
        run_record_kv_id(
            ui,
            render_cache,
            "manifest",
            record.record.manifest_id.as_str(),
        );
        if let Some(model) = record.record.metadata.agent.model_id.as_deref() {
            run_record_kv_id(ui, render_cache, "model", model);
        }
        if let Some(provider) = record.record.metadata.agent.provider.as_deref() {
            run_record_kv_id(ui, render_cache, "provider", provider);
        }
        run_record_kv_path(
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
        run_record_kv_usize(ui, render_cache, "turns", record.stats.turn_count);
        run_record_kv_usize(ui, render_cache, "tool calls", record.stats.tool_call_count);
        run_record_kv_usize(
            ui,
            render_cache,
            "failed tool calls",
            record.stats.failed_tool_call_count,
        );
        if let Some(packaging) = record.record.phases.packaging.as_ref() {
            run_record_kv_id(
                ui,
                render_cache,
                "submission",
                submission_artifact_state_label(packaging.submission_artifact_state),
            );
            run_record_kv_id(
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

fn run_record_kv_id(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_WIDGET_ROW).entered();
    ui.horizontal(|ui| {
        run_record_label(ui, render_cache, key);
        run_record_expandable_id(ui, render_cache, ("kv", key, value), value);
    });
}

fn run_record_kv_path(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_WIDGET_ROW).entered();
    ui.horizontal(|ui| {
        run_record_label(ui, render_cache, key);
        run_record_path_value(ui, render_cache, ("path", key, value), value);
    });
}

fn run_record_kv_usize(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: usize,
) {
    let mut buffer = itoa::Buffer::new();
    run_record_kv_id(ui, render_cache, key, buffer.format(value));
}

fn run_record_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = {
        let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_TEXT_GALLEY).entered();
        render_cache.run_record_text_galley(ui, text, CachedTextKind::Plain)
    };
    let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_LABEL_WIDGET).entered();
    ui.add(egui::Label::new(galley))
}

fn run_record_expandable_id(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    full: &str,
) -> egui::Response {
    let Some(_) = ShortId::new(full) else {
        return run_record_monospace_label(ui, render_cache, full);
    };

    let value = CopyableId::new(full);
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        true,
        |cache, ui, expanded| {
            let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_ID_GALLEY).entered();
            cache.run_record_id_galley(ui, full, expanded)
        },
    )
}

fn run_record_path_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    full: &str,
) -> egui::Response {
    let value = CopyablePath::new(full);
    let expandable = value.is_expandable();
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        expandable,
        |cache, ui, expanded| {
            let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_ID_GALLEY).entered();
            let label = if expanded { full } else { value.tail() };
            cache.run_record_text_galley(ui, label, CachedTextKind::Monospace)
        },
    )
}

fn run_record_monospace_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = {
        let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_TEXT_GALLEY).entered();
        render_cache.run_record_text_galley(ui, text, CachedTextKind::Monospace)
    };
    let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_LABEL_WIDGET).entered();
    ui.add(egui::Label::new(galley))
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
pub(crate) fn render_graph_edges_for_inspector(
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
pub(crate) fn render_artifact_edges_for_inspector(
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
pub(crate) fn render_patches_for_inspector(
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

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_candidate_comparison")
)]
pub(crate) fn render_candidate_comparison_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    let Some(slot) = sections.candidate_comparison() else {
        kv(ui, "candidate comparison", "not_available");
        return;
    };

    cached_kv_id(
        ui,
        render_cache,
        "parent node",
        slot.parent_node_id.as_str(),
    );
    cached_kv_usize(ui, render_cache, "planned children", slot.children().len());
    if let Some(metric_set) = slot.metric_set(graph) {
        cached_kv_id(
            ui,
            render_cache,
            "metric set",
            metric_set.metric_set_id.0.as_str(),
        );
        cached_kv_text(
            ui,
            render_cache,
            "score profile",
            score_profile_label(metric_set.policy.score_profile),
        );
        cached_kv_usize(
            ui,
            render_cache,
            "imp@k budget",
            metric_set.policy.imp_at_k.budget_k,
        );
        cached_kv_i64(
            ui,
            render_cache,
            "imp@k score weight",
            metric_set.policy.imp_at_k.score_points_per_imp_point,
        );
        cached_kv_text(
            ui,
            render_cache,
            "imp@k required",
            bool_label(metric_set.policy.imp_at_k.require_for_score),
        );
    }

    render_candidate_comparison_formula_summary(ui, render_cache, slot.selection_formula(graph));

    egui::ScrollArea::horizontal().show(ui, |ui| {
        egui::Grid::new(("candidate-comparison", slot.parent_node_id.as_str()))
            .striped(true)
            .num_columns(32)
            .show(ui, |ui| {
                cached_label(ui, render_cache, "selected");
                cached_label(ui, render_cache, "child");
                cached_label(ui, render_cache, "candidate");
                cached_label(ui, render_cache, "payload");
                cached_label(ui, render_cache, "outcome");
                cached_label(ui, render_cache, "outcome pts");
                cached_label(ui, render_cache, "operational pts");
                cached_label(ui, render_cache, "protocol pts");
                cached_label(ui, render_cache, "imp@k delta");
                cached_label(ui, render_cache, "performance");
                cached_label(ui, render_cache, "oracle rate");
                cached_label(ui, render_cache, "alpha");
                cached_label(ui, render_cache, "alpha mid");
                cached_label(ui, render_cache, "exploitation");
                cached_label(ui, render_cache, "exploration");
                cached_label(ui, render_cache, "weight");
                cached_label(ui, render_cache, "cumulative");
                cached_label(ui, render_cache, "sample hit");
                cached_label(ui, render_cache, "child count");
                cached_label(ui, render_cache, "selectable");
                cached_label(ui, render_cache, "exclusion");
                cached_label(ui, render_cache, "improvement");
                cached_label(ui, render_cache, "baseline");
                cached_label(ui, render_cache, "best descendant");
                cached_label(ui, render_cache, "scored / descendants");
                cached_label(ui, render_cache, "tool failures delta");
                cached_label(ui, render_cache, "patch failures delta");
                cached_label(ui, render_cache, "valid patch");
                cached_label(ui, render_cache, "converged");
                cached_label(ui, render_cache, "oracle eligible");
                cached_label(ui, render_cache, "protocol reviewed delta");
                cached_label(ui, render_cache, "protocol missing delta");
                ui.end_row();

                for child in slot.children() {
                    if let Some(candidate) = slot.resolve_child(graph, child) {
                        render_candidate_comparison_candidate(ui, render_cache, candidate);
                        ui.end_row();
                    }
                }
            });
    });
}

/// archaeology:score-child-prop-ui
/// proof:docs/active/archaeology/ploke-tree-graph/score-child-prop-ui-spec.md
fn render_candidate_comparison_formula_summary(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    formula: Option<&ploke_tree::graph::SelectionFormulaNode>,
) {
    let Some(formula) = formula else {
        cached_kv_text(ui, render_cache, "selector formula", "not_recorded");
        return;
    };
    match &formula.formula {
        ploke_tree::graph::SelectionFormulaKind::ScoreChildProp(score) => {
            cached_kv_text(ui, render_cache, "selector formula", "score_child_prop");
            cached_kv_id(
                ui,
                render_cache,
                "formula selection entry",
                formula.selection_entry_id.0.as_str(),
            );
            cached_kv_id(
                ui,
                render_cache,
                "formula metric set",
                formula.metric_set_id.0.as_str(),
            );
            cached_kv_u64(ui, render_cache, "seed", score.record.seed);
            cached_kv_usize(ui, render_cache, "top_m", score.record.top_m);
            cached_kv_u32(
                ui,
                render_cache,
                "lambda_millis",
                score.record.lambda_millis,
            );
            cached_kv_f64(ui, render_cache, "lambda", score.record.lambda);
            cached_kv_text(
                ui,
                render_cache,
                "metric inputs",
                score.record.metric_inputs.as_str(),
            );
            cached_kv_text(
                ui,
                render_cache,
                "oracle mode",
                score.record.oracle_mode.as_str(),
            );
            cached_kv_text(
                ui,
                render_cache,
                "oracle required",
                bool_label(score.record.oracle_require_evidence),
            );
            cached_kv_text(
                ui,
                render_cache,
                "alpha source",
                if score.record.oracle_used_for_alpha {
                    "oracle"
                } else {
                    "performance"
                },
            );
            cached_kv_f64(ui, render_cache, "alpha_mid", score.record.alpha_mid);
            cached_kv_f64(ui, render_cache, "total_weight", score.record.total_weight);
            cached_kv_optional_f64(ui, render_cache, "sample", score.record.sample);
            cached_kv_optional_f64(
                ui,
                render_cache,
                "sample threshold",
                score.record.sample_threshold,
            );
            cached_kv_optional_usize(
                ui,
                render_cache,
                "uniform fallback slot",
                score.record.uniform_fallback_slot,
            );
            cached_kv_optional_usize(
                ui,
                render_cache,
                "selected index",
                score.record.selected_index,
            );
            cached_kv_text(
                ui,
                render_cache,
                "selected replay candidate",
                score
                    .record
                    .selected_candidate
                    .as_deref()
                    .unwrap_or("not_recorded"),
            );
        }
    }
}

/// archaeology:score-child-prop-ui
/// proof:docs/active/archaeology/ploke-tree-graph/score-child-prop-ui-spec.md
fn render_candidate_comparison_candidate(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    candidate: crate::ui::inspector::CandidateComparisonCandidate<'_>,
) {
    let row = candidate.selector.and_then(|selector| selector.row);
    cached_label(
        ui,
        render_cache,
        if candidate.selected { "yes" } else { "no" },
    );
    cached_expandable_id(
        ui,
        render_cache,
        (
            "candidate-comparison-child",
            candidate.child.node.node_id.as_str(),
        ),
        candidate.child.node.node_id.as_str(),
    );
    cached_expandable_id(
        ui,
        render_cache,
        (
            "candidate-comparison-candidate",
            candidate.child.node.node_id.as_str(),
        ),
        candidate
            .candidate
            .map(|candidate| candidate.subject.value.as_str())
            .unwrap_or(candidate.child.resolved.branch.candidate_id.as_str()),
    );
    render_optional_usize_value(ui, render_cache, row.map(|row| row.payload_index));
    render_optional_str_value(
        ui,
        render_cache,
        row.map(|row| outcome_label(row.base_outcome)),
    );
    render_optional_i64(ui, render_cache, row.map(|row| row.outcome_points));
    render_optional_i64(ui, render_cache, row.map(|row| row.operational_points));
    render_optional_i64(ui, render_cache, row.map(|row| row.protocol_points));
    render_optional_i64(ui, render_cache, row.and_then(|row| row.imp_at_k_delta));
    render_optional_i64(ui, render_cache, row.and_then(|row| row.performance));
    render_optional_f64_value(ui, render_cache, row.and_then(|row| row.oracle_rate));
    render_optional_f64_value(ui, render_cache, row.and_then(|row| row.alpha));
    render_optional_f64_value(ui, render_cache, row.and_then(|row| row.alpha_mid));
    render_optional_f64_value(ui, render_cache, row.and_then(|row| row.exploitation));
    render_optional_f64_value(ui, render_cache, row.and_then(|row| row.exploration));
    render_optional_f64_value(ui, render_cache, row.and_then(|row| row.weight));
    render_optional_f64_range(
        ui,
        render_cache,
        row.and_then(|row| row.cumulative_lower.zip(row.cumulative_upper)),
    );
    render_optional_bool(ui, render_cache, row.map(|row| row.sample_hit));
    render_optional_usize_value(ui, render_cache, row.and_then(|row| row.child_count));
    render_optional_bool(ui, render_cache, row.map(|row| row.selectable));
    render_optional_str_value(
        ui,
        render_cache,
        row.and_then(|row| row.exclusion_reason.as_deref())
            .or(Some("none")),
    );
    render_optional_i64(
        ui,
        render_cache,
        candidate
            .metric
            .and_then(|metric| metric.imp_at_k.as_ref())
            .and_then(|imp| imp.improvement),
    );
    render_optional_i64(
        ui,
        render_cache,
        candidate
            .metric
            .and_then(|metric| metric.imp_at_k.as_ref())
            .and_then(|imp| imp.baseline_score),
    );
    render_optional_i64(
        ui,
        render_cache,
        candidate
            .metric
            .and_then(|metric| metric.imp_at_k.as_ref())
            .and_then(|imp| imp.best_descendant_score),
    );
    render_imp_at_k_counts(ui, render_cache, candidate.metric);
    render_optional_i64(
        ui,
        render_cache,
        candidate.metric.and_then(metric_tool_failures_delta),
    );
    render_optional_i64(
        ui,
        render_cache,
        candidate.metric.and_then(metric_patch_failures_delta),
    );
    render_optional_bool(
        ui,
        render_cache,
        candidate.metric.and_then(metric_treatment_valid_patch),
    );
    render_optional_bool(
        ui,
        render_cache,
        candidate.metric.and_then(metric_treatment_converged),
    );
    render_optional_bool(
        ui,
        render_cache,
        candidate.metric.and_then(metric_treatment_oracle_eligible),
    );
    render_optional_i64(
        ui,
        render_cache,
        candidate.metric.and_then(metric_protocol_reviewed_delta),
    );
    render_optional_i64(
        ui,
        render_cache,
        candidate.metric.and_then(metric_protocol_missing_delta),
    );
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn score_profile_label(profile: ploke_records::selection::ScoreProfile) -> &'static str {
    match profile {
        ploke_records::selection::ScoreProfile::OperationalQualityV1 => "operational_quality_v1",
    }
}

fn bool_label(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn outcome_label(outcome: ploke_records::selection::Outcome) -> &'static str {
    match outcome {
        ploke_records::selection::Outcome::Accepted => "accepted",
        ploke_records::selection::Outcome::ExploreFrom => "explore_from",
        ploke_records::selection::Outcome::Stop => "stop",
    }
}

fn format_f64(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.6}")
    } else {
        value.to_string()
    }
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn render_imp_at_k_counts(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    metric: Option<&ploke_tree::graph::MetricCandidateNode>,
) {
    let Some(imp) = metric.and_then(|metric| metric.imp_at_k.as_ref()) else {
        cached_label(ui, render_cache, "not_recorded");
        return;
    };
    let mut scored = itoa::Buffer::new();
    let mut descendants = itoa::Buffer::new();
    ui.horizontal(|ui| {
        cached_monospace_label(ui, render_cache, scored.format(imp.scored_descendant_count));
        cached_label(ui, render_cache, "/");
        cached_monospace_label(ui, render_cache, descendants.format(imp.descendant_count));
    });
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_tool_failures_delta(metric: &ploke_tree::graph::MetricCandidateNode) -> Option<i64> {
    metric.compared_runs.iter().fold(None, |acc, run| {
        sum_delta(
            acc,
            run.baseline_metrics.as_ref()?.tool_calls_failed,
            run.treatment_metrics.as_ref()?.tool_calls_failed,
        )
    })
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_patch_failures_delta(metric: &ploke_tree::graph::MetricCandidateNode) -> Option<i64> {
    metric.compared_runs.iter().fold(None, |acc, run| {
        sum_delta(
            acc,
            run.baseline_metrics.as_ref()?.partial_patch_failures,
            run.treatment_metrics.as_ref()?.partial_patch_failures,
        )
    })
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_treatment_valid_patch(metric: &ploke_tree::graph::MetricCandidateNode) -> Option<bool> {
    metric
        .compared_runs
        .iter()
        .filter_map(|run| {
            run.treatment_metrics
                .as_ref()
                .map(|metrics| metrics.nonempty_valid_patch)
        })
        .reduce(|left, right| left && right)
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_treatment_converged(metric: &ploke_tree::graph::MetricCandidateNode) -> Option<bool> {
    metric
        .compared_runs
        .iter()
        .filter_map(|run| {
            run.treatment_metrics
                .as_ref()
                .map(|metrics| metrics.convergence)
        })
        .reduce(|left, right| left && right)
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_treatment_oracle_eligible(
    metric: &ploke_tree::graph::MetricCandidateNode,
) -> Option<bool> {
    metric
        .compared_runs
        .iter()
        .filter_map(|run| {
            run.treatment_metrics
                .as_ref()
                .map(|metrics| metrics.oracle_eligible)
        })
        .reduce(|left, right| left && right)
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_protocol_reviewed_delta(metric: &ploke_tree::graph::MetricCandidateNode) -> Option<i64> {
    metric.compared_runs.iter().fold(None, |acc, run| {
        sum_delta(
            acc,
            run.baseline_protocol.as_ref()?.reviewed_call_count as u64,
            run.treatment_protocol.as_ref()?.reviewed_call_count as u64,
        )
    })
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_protocol_missing_delta(metric: &ploke_tree::graph::MetricCandidateNode) -> Option<i64> {
    metric.compared_runs.iter().fold(None, |acc, run| {
        sum_delta(
            acc,
            run.baseline_protocol.as_ref()?.missing_call_count as u64,
            run.treatment_protocol.as_ref()?.missing_call_count as u64,
        )
    })
}

fn sum_delta(acc: Option<i64>, baseline: u64, treatment: u64) -> Option<i64> {
    Some(
        acc.unwrap_or_default()
            .saturating_add((treatment as i64).saturating_sub(baseline as i64)),
    )
}

fn render_optional_i64(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    value: Option<i64>,
) {
    if let Some(value) = value {
        let mut buffer = itoa::Buffer::new();
        cached_monospace_label(ui, render_cache, buffer.format(value));
    } else {
        cached_label(ui, render_cache, "not_recorded");
    }
}

fn render_optional_usize_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    value: Option<usize>,
) {
    if let Some(value) = value {
        let mut buffer = itoa::Buffer::new();
        cached_monospace_label(ui, render_cache, buffer.format(value));
    } else {
        cached_label(ui, render_cache, "not_recorded");
    }
}

fn render_optional_f64_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    value: Option<f64>,
) {
    if let Some(value) = value {
        cached_monospace_label(ui, render_cache, format_f64(value).as_str());
    } else {
        cached_label(ui, render_cache, "not_recorded");
    }
}

fn render_optional_f64_range(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    value: Option<(f64, f64)>,
) {
    if let Some((lower, upper)) = value {
        cached_monospace_label(
            ui,
            render_cache,
            format!("{}..{}", format_f64(lower), format_f64(upper)).as_str(),
        );
    } else {
        cached_label(ui, render_cache, "not_recorded");
    }
}

fn render_optional_str_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    value: Option<&str>,
) {
    if let Some(value) = value {
        cached_monospace_label(ui, render_cache, value);
    } else {
        cached_label(ui, render_cache, "not_recorded");
    }
}

fn render_optional_bool(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    value: Option<bool>,
) {
    if let Some(value) = value {
        cached_label(ui, render_cache, bool_label(value));
    } else {
        cached_label(ui, render_cache, "not_recorded");
    }
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
                    Some(&selection),
                    Some("artifact"),
                    Some("A1"),
                    Some(sections),
                    &mut render_cache,
                    &mut diff_cache,
                    InspectorOpenState::default(),
                    None,
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
        let state = InspectorOpenState::benchmark(
            Some(crate::benchmark::BenchmarkInspectorSection::GraphEdges),
            false,
        );

        assert_eq!(state.open(InspectorPanelSection::GraphEdges), Some(true));
        assert_eq!(state.open(InspectorPanelSection::LlmCalls), None);
        assert_eq!(state.open(InspectorPanelSection::RunRecords), None);
        assert_eq!(state.open(InspectorPanelSection::PatchDebug), None);
    }

    #[test]
    fn benchmark_inspector_open_state_can_force_only_target_section() {
        let state = InspectorOpenState::benchmark(
            Some(crate::benchmark::BenchmarkInspectorSection::GraphEdges),
            true,
        );

        assert_eq!(state.open(InspectorPanelSection::GraphEdges), Some(true));
        assert_eq!(state.open(InspectorPanelSection::LlmCalls), Some(false));
        assert_eq!(state.open(InspectorPanelSection::RunRecords), Some(false));
        assert_eq!(state.open(InspectorPanelSection::PatchDebug), Some(false));
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
    graph: &Graph,
    lookup: ParentCreateLookup<'_, '_>,
    run_record_slots: &[RunRecordSlot],
    render_cache: &mut InspectorRenderCache,
    open_state: InspectorOpenState,
) {
    match lookup {
        ParentCreateLookup::Attempt(attempt) => {
            render_parent_create_attempt(
                ui,
                graph,
                attempt,
                run_record_slots,
                render_cache,
                open_state,
            );
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
    graph: &Graph,
    attempt: ParentCreateAttempt<'_>,
    run_record_slots: &[RunRecordSlot],
    render_cache: &mut InspectorRenderCache,
    open_state: InspectorOpenState,
) {
    let child = attempt.child();
    let surface = attempt.surface();
    let branch_summary = run_record_turn_summary(graph, run_record_slots);
    let summary = branch_summary.unwrap_or_else(|| agent_turn_summary(attempt));
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
    cached_kv_path(
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

    render_parent_create_llm_calls(
        ui,
        graph,
        &attempt,
        run_record_slots,
        render_cache,
        open_state,
    );

    // render_run_record_turns(ui, render_cache, graph, run_record_slots);
    // render_agent_turns(ui, render_cache, attempt.agent_turns());
    render_parent_create_source_status(ui, &attempt, render_cache);
}

pub(crate) fn render_parent_create_llm_calls(
    ui: &mut egui::Ui,
    graph: &Graph,
    attempt: &ParentCreateAttempt<'_>,
    run_record_slots: &[RunRecordSlot],
    render_cache: &mut InspectorRenderCache,
    open_state: InspectorOpenState,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("LLM calls")
            .default_open(false)
            .open(open_state.open(InspectorPanelSection::LlmCalls)),
        |ui| {
            render_parent_create_llm_calls_body(ui, graph, attempt, run_record_slots, render_cache)
        },
    );
}

pub(crate) fn render_parent_create_llm_calls_body(
    ui: &mut egui::Ui,
    graph: &Graph,
    attempt: &ParentCreateAttempt<'_>,
    run_record_slots: &[RunRecordSlot],
    render_cache: &mut InspectorRenderCache,
) {
    let _span = tracing::trace_span!(scope::INSPECTOR_PARENT_CREATE_LLM_CALLS).entered();
    if render_run_record_turns(ui, render_cache, graph, run_record_slots) {
        return;
    }
    cached_kv_id(ui, render_cache, "evidence", "agent_turn_sidecar_fallback");
    render_agent_turns(ui, render_cache, attempt.agent_turns());
}

fn render_parent_create_source_status(
    ui: &mut egui::Ui,
    attempt: &ParentCreateAttempt<'_>,
    render_cache: &mut InspectorRenderCache,
) {
    let child = attempt.child();
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Source status").default_open(false),
        |ui| {
            let _span =
                tracing::trace_span!(scope::INSPECTOR_PARENT_CREATE_SOURCE_STATUS).entered();
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
        },
    );
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

#[derive(Default, Clone, Copy)]
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

/// archaeology:run-record-branch-output
/// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
fn run_record_turn_summary(
    graph: &Graph,
    run_record_slots: &[RunRecordSlot],
) -> Option<AgentTurnSummary> {
    let mut rendered = false;
    let mut summary = AgentTurnSummary::default();
    for record in run_record_slots
        .iter()
        .filter_map(|slot| slot.resolve(graph))
    {
        if record.stats.turn_count == 0 {
            continue;
        }
        rendered = true;
        summary.tool_requested += record.stats.tool_call_count;
        summary.tool_failed += record.stats.failed_tool_call_count;
        summary.tool_completed += record
            .stats
            .tool_call_count
            .saturating_sub(record.stats.failed_tool_call_count);
        for turn in record.turns() {
            if let Some(artifact) = turn.turn.agent_turn_artifact.as_ref() {
                summary.edit_proposals += artifact.patch_artifact.edit_proposals.len();
                summary.create_proposals += artifact.patch_artifact.create_proposals.len();
                summary.expected_file_changes +=
                    artifact.patch_artifact.expected_file_changes.len();
            }
        }
    }
    rendered.then_some(summary)
}

/// archaeology:run-record-branch-output
/// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
fn render_run_record_turns(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    graph: &Graph,
    run_record_slots: &[RunRecordSlot],
) -> bool {
    let treatment = render_run_record_arm_turns(
        ui,
        render_cache,
        graph,
        run_record_slots,
        ploke_tree::ComparedRunArm::Treatment,
        true,
    );
    let baseline = render_run_record_arm_turns(
        ui,
        render_cache,
        graph,
        run_record_slots,
        ploke_tree::ComparedRunArm::Baseline,
        false,
    );
    treatment || baseline
}

fn has_run_record_arm_turns(
    graph: &Graph,
    run_record_slots: &[RunRecordSlot],
    arm: ploke_tree::ComparedRunArm,
) -> bool {
    run_record_slots
        .iter()
        .filter_map(|slot| slot.resolve(graph))
        .any(|record| record.record_ref.arm == arm && record.record.turn_count() > 0)
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_run_record_arm")
)]
fn render_run_record_arm_turns(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    graph: &Graph,
    run_record_slots: &[RunRecordSlot],
    arm: ploke_tree::ComparedRunArm,
    default_open: bool,
) -> bool {
    if !has_run_record_arm_turns(graph, run_record_slots, arm) {
        return false;
    }

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(compared_run_arm_label(arm)).default_open(default_open),
        |ui| {
            cached_kv_id(ui, render_cache, "evidence", "branch_run_record");
            for record in run_record_slots
                .iter()
                .filter_map(|slot| slot.resolve(graph))
                .filter(|record| record.record_ref.arm == arm)
            {
                for turn in record.turns() {
                    render_run_record_turn(ui, render_cache, turn);
                }
            }
        },
    );
    true
}

fn render_run_record_turn(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turn: RunRecordTurnInspection<'_>,
) {
    ui.separator();
    cached_kv_id(
        ui,
        render_cache,
        "arm",
        compared_run_arm_label(turn.record_ref.arm),
    );
    cached_kv_id(
        ui,
        render_cache,
        "instance",
        turn.record_ref.instance_id.as_str(),
    );
    if let Some(model) = turn
        .turn
        .llm_request
        .as_ref()
        .map(|request| request.model.as_str())
        .or(turn.record.metadata.agent.model_id.as_deref())
    {
        cached_kv_id(ui, render_cache, "model", model);
    }
    if let Some(provider) = turn.record.metadata.agent.provider.as_deref() {
        cached_kv_id(ui, render_cache, "provider", provider);
    }
    cached_kv_usize(ui, render_cache, "turn", turn.turn.turn_number as usize);
    cached_kv_id(
        ui,
        render_cache,
        "outcome",
        turn_outcome_label(&turn.turn.outcome),
    );
    if let Some(count) = turn_outcome_tool_count(&turn.turn.outcome) {
        cached_kv_usize(ui, render_cache, "outcome tools", count);
    }
    if let Some(message) = turn_outcome_error(&turn.turn.outcome) {
        cached_kv_text(ui, render_cache, "outcome error", message);
    }
    if let Some(elapsed) = turn_outcome_elapsed_secs(&turn.turn.outcome) {
        cached_kv_u64(ui, render_cache, "elapsed secs", elapsed);
    }
    cached_kv_usize(
        ui,
        render_cache,
        "prompt messages",
        turn.turn
            .llm_request
            .as_ref()
            .map_or(0, |request| request.messages.len()),
    );
    if let Some(response) = turn.turn.llm_response.as_ref() {
        cached_kv_id(ui, render_cache, "response", "present");
        if let Some(reason) = response.finish_reason.as_ref() {
            cached_kv_id(
                ui,
                render_cache,
                "finish reason",
                response_finish_reason_label(reason),
            );
        }
        if let Some(usage) = response.usage {
            render_token_usage(ui, render_cache, usage);
        }
    } else {
        cached_kv_id(ui, render_cache, "response", "missing");
    }
    render_run_record_agent_turn_artifact(ui, render_cache, turn);
    render_run_record_tool_steps(ui, render_cache, turn.turn.tool_calls.as_slice());
}

fn render_token_usage(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    usage: ploke_records::agent_turn::TokenUsageRecord,
) {
    ui.horizontal(|ui| {
        let mut prompt = itoa::Buffer::new();
        let mut completion = itoa::Buffer::new();
        let mut total = itoa::Buffer::new();
        cached_label(ui, render_cache, "usage");
        cached_monospace_label(ui, render_cache, "prompt=");
        cached_monospace_label(ui, render_cache, prompt.format(usage.prompt_tokens));
        cached_monospace_label(ui, render_cache, " completion=");
        cached_monospace_label(ui, render_cache, completion.format(usage.completion_tokens));
        cached_monospace_label(ui, render_cache, " total=");
        cached_monospace_label(ui, render_cache, total.format(usage.total_tokens));
    });
}

fn render_run_record_agent_turn_artifact(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turn: RunRecordTurnInspection<'_>,
) {
    let Some(artifact) = turn.turn.agent_turn_artifact.as_ref() else {
        cached_kv_id(ui, render_cache, "agent-turn artifact", "not_recorded");
        return;
    };

    cached_kv_usize(ui, render_cache, "artifact events", artifact.events.len());
    if let Some(terminal) = artifact.terminal_record.as_ref() {
        cached_kv_id(
            ui,
            render_cache,
            "terminal outcome",
            terminal.outcome.as_str(),
        );
        cached_kv_u32(ui, render_cache, "terminal attempts", terminal.attempts);
        cached_kv_text(
            ui,
            render_cache,
            "terminal summary",
            terminal.summary.as_str(),
        );
    } else {
        cached_kv_id(ui, render_cache, "terminal", "not_recorded");
    }
    ui.horizontal(|ui| {
        let mut edits = itoa::Buffer::new();
        let mut creates = itoa::Buffer::new();
        let mut expected = itoa::Buffer::new();
        cached_label(ui, render_cache, "patch proposals");
        cached_monospace_label(ui, render_cache, "edits=");
        cached_monospace_label(
            ui,
            render_cache,
            edits.format(artifact.patch_artifact.edit_proposals.len()),
        );
        cached_monospace_label(ui, render_cache, " creates=");
        cached_monospace_label(
            ui,
            render_cache,
            creates.format(artifact.patch_artifact.create_proposals.len()),
        );
        cached_monospace_label(ui, render_cache, " expected_files=");
        cached_monospace_label(
            ui,
            render_cache,
            expected.format(artifact.patch_artifact.expected_file_changes.len()),
        );
    });
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::INSPECTOR_RUN_RECORD_TOOL_STEPS)]
fn render_run_record_tool_steps(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    tools: &[ploke_records::run_record::ToolExecutionRecord],
) {
    if tools.is_empty() {
        cached_kv_id(ui, render_cache, "tool steps", "none");
        return;
    }

    cached_kv_usize(ui, render_cache, "tool steps", tools.len());
    for (index, tool) in tools.iter().enumerate() {
        render_run_record_tool_step(ui, render_cache, index, tool);
    }
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_run_record_tool_step")
)]
fn render_run_record_tool_step(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    index: usize,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    let call_id = tool.request.call_id.as_str();
    let header_id = ui.make_persistent_id(("run-record-tool-step", index, call_id));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), header_id, false)
        .show_header(ui, |ui| {
            render_run_record_tool_step_header(ui, render_cache, index, tool);
        })
        .body(|ui| {
            render_run_record_tool_step_details(ui, render_cache, index, tool);
        });
}

fn render_run_record_tool_step_header(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    index: usize,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    ui.horizontal(|ui| {
        let mut step = itoa::Buffer::new();
        cached_monospace_label(ui, render_cache, step.format(index + 1));
        cached_monospace_label(ui, render_cache, tool_execution_name(tool));
        render_tool_execution_status_badge(ui, tool);
    });
}

fn render_run_record_tool_step_details(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    index: usize,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    let call_id = tool.request.call_id.as_str();
    let mut latency = itoa::Buffer::new();

    ui.horizontal(|ui| {
        cached_label(ui, render_cache, "call id");
        cached_expandable_id(
            ui,
            render_cache,
            ("run-record-tool-call-id", index, call_id),
            call_id,
        );
    });
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, "latency");
        cached_monospace_label(ui, render_cache, latency.format(tool.latency_ms));
        cached_monospace_label(ui, render_cache, "ms");
    });
    cached_label(ui, render_cache, "summary");
    cached_wrapped_monospace_label(ui, render_cache, tool_execution_summary(tool));
    render_tool_arguments_section(ui, render_cache, index, tool);
    if let Some(payload) = tool_execution_ui_payload(tool) {
        render_tool_ui_payload(ui, render_cache, payload);
    }
    render_tool_result_section(ui, render_cache, index, tool);
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_arguments")
)]
fn render_tool_arguments_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    index: usize,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    let call_id = tool.request.call_id.as_str();
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("tool call arguments")
            .id_salt(("run-record-tool-arguments", index, call_id))
            .default_open(true),
        |ui| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let decoded = render_cache.tool_arguments(
                    call_id,
                    tool.request.tool.as_str(),
                    tool.request.arguments.as_str(),
                );
                render_decoded_tool_arguments(ui, render_cache, decoded.as_ref());
            }
            #[cfg(target_arch = "wasm32")]
            tool_kv_text(ui, render_cache, "decode", "native_only");

            show_inspector_collapsing(
                ui,
                egui::CollapsingHeader::new("raw arguments")
                    .id_salt(("run-record-tool-raw-arguments", index, call_id))
                    .default_open(false),
                |ui| {
                    render_tool_raw_arguments_section(ui, render_cache, index, call_id, tool);
                },
            );
        },
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_raw_arguments")
)]
fn render_tool_raw_arguments_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    index: usize,
    call_id: &str,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    render_cached_code_block(
        ui,
        render_cache,
        ("run-record-tool-arguments-block", index, call_id),
        tool.request.arguments.as_str(),
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_result")
)]
fn render_tool_result_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    index: usize,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    let call_id = tool.request.call_id.as_str();
    let raw_content = tool_execution_content(tool);
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("tool call content")
            .id_salt(("run-record-tool-content", index, call_id))
            .default_open(false),
        |ui| match &tool.result {
            ploke_records::run_record::ToolResult::Completed(_) => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let decoded =
                        render_cache.tool_result(call_id, tool_execution_name(tool), raw_content);
                    render_decoded_tool_result(ui, render_cache, decoded.as_ref());
                }
                #[cfg(target_arch = "wasm32")]
                tool_kv_text(ui, render_cache, "decode", "native_only");

                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new("raw content")
                        .id_salt(("run-record-tool-content-raw", index, call_id))
                        .default_open(false),
                    |ui| {
                        render_tool_raw_result_section(
                            ui,
                            render_cache,
                            ("run-record-tool-content-block", index, call_id),
                            raw_content,
                        );
                    },
                );
            }
            ploke_records::run_record::ToolResult::Failed(_) => {
                render_tool_failure_content(ui, render_cache, tool);
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new("raw error")
                        .id_salt(("run-record-tool-error-raw", index, call_id))
                        .default_open(false),
                    |ui| {
                        render_tool_raw_result_section(
                            ui,
                            render_cache,
                            ("run-record-tool-error-block", index, call_id),
                            raw_content,
                        );
                    },
                );
            }
        },
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_raw_result")
)]
fn render_tool_raw_result_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_salt: impl std::hash::Hash,
    raw_content: &str,
) {
    render_cached_code_block(ui, render_cache, id_salt, raw_content);
}

fn render_tool_execution_status_badge(
    ui: &mut egui::Ui,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    let label = tool_execution_status_label(tool);
    let (fill, text_color) = match &tool.result {
        ploke_records::run_record::ToolResult::Completed(_) => {
            (egui::Color32::from_rgb(33, 164, 106), egui::Color32::WHITE)
        }
        ploke_records::run_record::ToolResult::Failed(_) => {
            (egui::Color32::from_rgb(178, 72, 72), egui::Color32::WHITE)
        }
    };
    ui.label(
        egui::RichText::new(label)
            .background_color(fill)
            .color(text_color)
            .monospace(),
    );
}

fn tool_execution_content(tool: &ploke_records::run_record::ToolExecutionRecord) -> &str {
    match &tool.result {
        ploke_records::run_record::ToolResult::Completed(result) => result.content.as_str(),
        ploke_records::run_record::ToolResult::Failed(result) => result.error.as_str(),
    }
}

fn tool_execution_ui_payload(
    tool: &ploke_records::run_record::ToolExecutionRecord,
) -> Option<&ploke_records::agent_turn::ToolUiPayloadRecord> {
    match &tool.result {
        ploke_records::run_record::ToolResult::Completed(result) => result.ui_payload.as_ref(),
        ploke_records::run_record::ToolResult::Failed(result) => result.ui_payload.as_ref(),
    }
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_ui_payload")
)]
fn render_tool_ui_payload(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    payload: &ploke_records::agent_turn::ToolUiPayloadRecord,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("tool ui payload").default_open(true),
        |ui| {
            tool_kv_text(ui, render_cache, "tool", payload.tool.as_str());
            tool_kv_text(ui, render_cache, "call id", payload.call_id.as_str());
            if let Some(request_id) = payload.request_id.as_deref() {
                tool_kv_text(ui, render_cache, "request id", request_id);
            }
            if let Some(proposal_id) = payload.proposal_id.as_deref() {
                tool_kv_text(ui, render_cache, "proposal id", proposal_id);
            }
            tool_kv_text(ui, render_cache, "summary", payload.summary.as_str());
            tool_kv_debug(ui, render_cache, "verbosity", payload.verbosity);
            for field in &payload.fields {
                if is_pathish_key(field.name.as_str()) {
                    tool_kv_path(ui, render_cache, field.name.as_str(), field.value.as_str());
                } else {
                    tool_kv_text(ui, render_cache, field.name.as_str(), field.value.as_str());
                }
            }
            if let Some(details) = payload.details.as_deref() {
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new("details").default_open(false),
                    |ui| {
                        render_tool_ui_payload_details(ui, render_cache, details);
                    },
                );
            }
            if let Some(error) = payload.error.as_ref() {
                render_tool_error_wire(ui, render_cache, error);
            }
        },
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_ui_details")
)]
fn render_tool_ui_payload_details(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    details: &str,
) {
    cached_wrapped_monospace_label(ui, render_cache, details);
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_error")
)]
fn render_tool_error_wire(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    error: &ploke_records::agent_turn::ToolErrorWireRecord,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("typed error").default_open(true),
        |ui| {
            tool_kv_text(ui, render_cache, "user", error.user.as_str());
            tool_kv_text(ui, render_cache, "system", error.system.as_str());
            tool_kv_bool(ui, render_cache, "ok", error.llm.ok);
            tool_kv_text(ui, render_cache, "tool", error.llm.tool.as_str());
            tool_kv_debug(ui, render_cache, "code", error.llm.code);
            if let Some(field) = error.llm.field.as_deref() {
                tool_kv_text(ui, render_cache, "field", field);
            }
            if let Some(expected) = error.llm.expected.as_deref() {
                tool_kv_text(ui, render_cache, "expected", expected);
            }
            if let Some(received) = error.llm.received.as_deref() {
                tool_kv_text(ui, render_cache, "received", received);
            }
            tool_kv_text(ui, render_cache, "message", error.llm.message.as_str());
            if let Some(hint) = error.llm.retry_hint.as_deref() {
                tool_kv_text(ui, render_cache, "retry hint", hint);
            }
        },
    );
}

fn render_tool_failure_content(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    if let ploke_records::run_record::ToolResult::Failed(result) = &tool.result {
        tool_kv_text(ui, render_cache, "status", "failed");
        if let Some(tool_name) = result.tool.as_deref() {
            tool_kv_text(ui, render_cache, "tool", tool_name);
        }
        tool_kv_text(ui, render_cache, "error", result.error.as_str());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_decoded_tool_arguments(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    decoded: &PersistedToolCallArguments,
) {
    match decoded {
        PersistedToolCallArguments::Decoded(arguments) => {
            tool_kv_text(ui, render_cache, "decode", "ok");
            render_tool_call_arguments(ui, render_cache, arguments);
        }
        PersistedToolCallArguments::ParseFailure(failure) => {
            tool_kv_text(ui, render_cache, "decode", "failed");
            tool_kv_text(ui, render_cache, "tool", failure.tool.as_str());
            tool_kv_debug(ui, render_cache, "error", &failure.error);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_tool_call_arguments(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    arguments: &ToolCallArguments,
) {
    match arguments {
        ToolCallArguments::RequestCodeContext(args) => {
            render_optional_u32(
                ui,
                render_cache,
                "token budget per result",
                args.token_budget_per_result,
            );
            render_optional_u32(
                ui,
                render_cache,
                "token budget total",
                args.token_budget_total,
            );
            render_optional_str(ui, render_cache, "search term", args.search_term.as_deref());
        }
        ToolCallArguments::ApplyCodeEdit(args) => {
            render_optional_f32(ui, render_cache, "confidence", args.confidence);
            tool_kv_usize(ui, render_cache, "edits", args.edits.len());
            for (index, edit) in args.edits.iter().enumerate() {
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new(format!("edit {}", index + 1))
                        .default_open(index == 0),
                    |ui| {
                        let _span =
                            tracing::trace_span!(scope::INSPECTOR_TOOL_ARGUMENT_EDIT).entered();
                        tool_kv_path(ui, render_cache, "file", edit.file.as_str());
                        tool_kv_text(ui, render_cache, "canon", edit.canon.as_str());
                        tool_kv_debug(ui, render_cache, "node type", edit.node_type);
                        tool_kv_text_size_summary(ui, render_cache, "code", edit.code.as_str());
                    },
                );
            }
        }
        ToolCallArguments::InsertRustItem(args) => {
            tool_kv_path(ui, render_cache, "file", args.file.as_str());
            tool_kv_debug(ui, render_cache, "container kind", args.container_kind);
            render_optional_str(
                ui,
                render_cache,
                "container canon",
                args.container_canon.as_deref(),
            );
            tool_kv_debug(ui, render_cache, "item kind", args.item_kind);
            render_optional_f32(ui, render_cache, "confidence", args.confidence);
            tool_kv_text_size_summary(ui, render_cache, "code", args.code.as_str());
        }
        ToolCallArguments::CreateFile(args) => {
            tool_kv_path(ui, render_cache, "file path", args.file_path.as_str());
            render_optional_str(ui, render_cache, "on exists", args.on_exists.as_deref());
            tool_kv_bool(ui, render_cache, "create parents", args.create_parents);
            tool_kv_text_size_summary(ui, render_cache, "content", args.content.as_str());
        }
        ToolCallArguments::NsPatch(args) => {
            render_optional_f32(ui, render_cache, "confidence", args.confidence);
            tool_kv_usize(ui, render_cache, "patches", args.patches.len());
            for (index, patch) in args.patches.iter().enumerate() {
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new(format!("patch {}", index + 1))
                        .default_open(index == 0),
                    |ui| {
                        let _span =
                            tracing::trace_span!(scope::INSPECTOR_TOOL_ARGUMENT_PATCH).entered();
                        tool_kv_path(ui, render_cache, "file", patch.file.as_str());
                        tool_kv_text(ui, render_cache, "reasoning", patch.reasoning.as_str());
                        tool_kv_text_size_summary(ui, render_cache, "diff", patch.diff.as_str());
                    },
                );
            }
        }
        ToolCallArguments::NsRead(args) => {
            tool_kv_path(ui, render_cache, "file", args.file.as_str());
            render_optional_u32(ui, render_cache, "start line", args.start_line);
            render_optional_u32(ui, render_cache, "end line", args.end_line);
            render_optional_u32(ui, render_cache, "max bytes", args.max_bytes);
        }
        ToolCallArguments::CodeItemLookup(args) => {
            render_code_item_query(
                ui,
                render_cache,
                args.item_name.as_str(),
                args.file_path.as_str(),
                args.node_kind.as_str(),
                args.module_path.as_str(),
            );
        }
        ToolCallArguments::CodeItemEdges(args) => {
            render_code_item_query(
                ui,
                render_cache,
                args.item_name.as_str(),
                args.file_path.as_str(),
                args.node_kind.as_str(),
                args.module_path.as_str(),
            );
        }
        ToolCallArguments::Cargo(args) => {
            tool_kv_debug(ui, render_cache, "command", args.command);
            tool_kv_debug(ui, render_cache, "scope", args.scope);
            render_optional_str(ui, render_cache, "package", args.package.as_deref());
            render_optional_string_list(ui, render_cache, "features", args.features.as_deref());
            tool_kv_bool(ui, render_cache, "all features", args.all_features);
            tool_kv_bool(
                ui,
                render_cache,
                "no default features",
                args.no_default_features,
            );
            render_optional_str(ui, render_cache, "target", args.target.as_deref());
            render_optional_str(ui, render_cache, "profile", args.profile.as_deref());
            tool_kv_bool(ui, render_cache, "release", args.release);
            tool_kv_bool(ui, render_cache, "lib", args.lib);
            tool_kv_bool(ui, render_cache, "tests", args.tests);
            tool_kv_bool(ui, render_cache, "bins", args.bins);
            tool_kv_bool(ui, render_cache, "examples", args.examples);
            tool_kv_bool(ui, render_cache, "benches", args.benches);
            render_optional_string_list(ui, render_cache, "test args", args.test_args.as_deref());
        }
        ToolCallArguments::ListDir(args) => {
            tool_kv_path(ui, render_cache, "dir", args.dir.as_str());
            tool_kv_bool(ui, render_cache, "include hidden", args.include_hidden);
            render_optional_str(ui, render_cache, "sort", args.sort.as_deref());
            render_optional_u32(ui, render_cache, "max entries", args.max_entries);
        }
        ToolCallArguments::SearchCode(args)
        | ToolCallArguments::SearchSymbols(args)
        | ToolCallArguments::QueryCodebase(args) => {
            render_optional_str(ui, render_cache, "search term", args.search_term.as_deref());
            render_optional_str(ui, render_cache, "query", args.query.as_deref());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_decoded_tool_result(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    decoded: &PersistedToolResultContent,
) {
    match decoded {
        PersistedToolResultContent::Decoded(result) => {
            tool_kv_text(ui, render_cache, "decode", "ok");
            render_tool_result_content(ui, render_cache, result);
        }
        PersistedToolResultContent::ParseFailure(failure) => {
            tool_kv_text(ui, render_cache, "decode", "failed");
            tool_kv_text(ui, render_cache, "tool", failure.tool.as_str());
            tool_kv_debug(ui, render_cache, "error", &failure.error);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_tool_result_content(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    result: &ToolResultContent,
) {
    match result {
        ToolResultContent::RequestCodeContext(result) => {
            tool_kv_bool(ui, render_cache, "ok", result.ok);
            tool_kv_text(ui, render_cache, "search term", result.search_term.as_str());
            tool_kv_usize(ui, render_cache, "top k", result.top_k);
            tool_kv_debug(ui, render_cache, "kind", result.kind);
            render_optional_str(ui, render_cache, "note", result.note.as_deref());
            render_string_list(ui, render_cache, "next steps", &result.next_steps);
            tool_kv_usize(ui, render_cache, "context items", result.context.len());
            for (index, item) in result.context.iter().take(3).enumerate() {
                render_concise_context(ui, render_cache, index, item);
            }
        }
        ToolResultContent::ApplyCodeEdit(result) | ToolResultContent::InsertRustItem(result) => {
            render_patch_like_result(
                ui,
                render_cache,
                result.ok,
                result.staged,
                result.applied,
                &result.files,
                result.preview_mode.as_str(),
                result.auto_confirmed,
            );
        }
        ToolResultContent::CreateFile(result) => {
            render_patch_like_result(
                ui,
                render_cache,
                result.ok,
                result.staged,
                result.applied,
                &result.files,
                result.preview_mode.as_str(),
                result.auto_confirmed,
            );
        }
        ToolResultContent::NsPatch(result) => {
            render_patch_like_result(
                ui,
                render_cache,
                result.ok,
                result.staged,
                result.applied,
                &result.files,
                result.preview_mode.as_str(),
                result.auto_confirmed,
            );
        }
        ToolResultContent::NsRead(result) => {
            tool_kv_bool(ui, render_cache, "ok", result.ok);
            tool_kv_path(ui, render_cache, "file path", result.file_path.as_str());
            tool_kv_bool(ui, render_cache, "exists", result.exists);
            render_optional_u64(ui, render_cache, "byte len", result.byte_len);
            render_optional_u32(ui, render_cache, "start line", result.start_line);
            render_optional_u32(ui, render_cache, "end line", result.end_line);
            tool_kv_bool(ui, render_cache, "truncated", result.truncated);
            if let Some(hash) = result.file_hash.as_ref() {
                tool_kv_debug(ui, render_cache, "file hash", hash);
            }
            if let Some(content) = result.content.as_deref() {
                tool_kv_text_size_summary(ui, render_cache, "content", content);
            }
        }
        ToolResultContent::CodeItemLookup(result) => {
            render_concise_context(ui, render_cache, 0, result);
        }
        ToolResultContent::Cargo(result) => {
            tool_kv_bool(ui, render_cache, "ok", result.ok);
            tool_kv_debug(ui, render_cache, "status", result.status_reason);
            tool_kv_debug(ui, render_cache, "command", result.command);
            tool_kv_debug(ui, render_cache, "scope", result.scope);
            tool_kv_path(ui, render_cache, "manifest", result.manifest_path.as_str());
            render_optional_i32(ui, render_cache, "exit code", result.exit_code);
            tool_kv_u64(ui, render_cache, "duration ms", result.duration_ms);
            tool_kv_u32(ui, render_cache, "errors", result.summary.errors);
            tool_kv_u32(ui, render_cache, "warnings", result.summary.warnings);
            tool_kv_u32(ui, render_cache, "notes", result.summary.notes);
            tool_kv_usize(ui, render_cache, "diagnostics", result.diagnostics.len());
            tool_kv_bool(ui, render_cache, "truncated", result.raw_messages_truncated);
        }
        ToolResultContent::ListDir(result) => {
            tool_kv_usize(ui, render_cache, "entries", result.entries.len());
            show_inspector_collapsing(ui, egui::CollapsingHeader::new("details"), |ui| {
                let _span =
                    tracing::trace_span!(scope::INSPECTOR_TOOL_RESULT_LIST_DIR_DETAILS).entered();
                tool_kv_bool(ui, render_cache, "ok", result.ok);
                tool_kv_path(ui, render_cache, "dir", result.dir.as_str());
                tool_kv_bool(ui, render_cache, "exists", result.exists);
                tool_kv_bool(ui, render_cache, "truncated", result.truncated);
            });
            for (index, entry) in result.entries.iter().take(8).enumerate() {
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new(entry.name.as_str()).default_open(index == 0),
                    |ui| {
                        let _span =
                            tracing::trace_span!(scope::INSPECTOR_TOOL_RESULT_LIST_DIR_ENTRY)
                                .entered();
                        tool_kv_path(ui, render_cache, "path", entry.path.as_str());
                        tool_kv_text(ui, render_cache, "kind", entry.kind.as_str());
                        render_optional_u64(ui, render_cache, "size bytes", entry.size_bytes);
                    },
                );
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_patch_like_result(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    ok: bool,
    staged: usize,
    applied: usize,
    files: &[String],
    preview_mode: &str,
    auto_confirmed: bool,
) {
    tool_kv_bool(ui, render_cache, "ok", ok);
    tool_kv_usize(ui, render_cache, "staged", staged);
    tool_kv_usize(ui, render_cache, "applied", applied);
    tool_kv_text(ui, render_cache, "preview mode", preview_mode);
    tool_kv_bool(ui, render_cache, "auto confirmed", auto_confirmed);
    render_path_list(ui, render_cache, "files", files);
}

#[cfg(not(target_arch = "wasm32"))]
fn render_code_item_query(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    item_name: &str,
    file_path: &str,
    node_kind: &str,
    module_path: &str,
) {
    tool_kv_text(ui, render_cache, "item name", item_name);
    tool_kv_path(ui, render_cache, "file path", file_path);
    tool_kv_text(ui, render_cache, "node kind", node_kind);
    tool_kv_text(ui, render_cache, "module path", module_path);
}

#[cfg(not(target_arch = "wasm32"))]
fn render_concise_context(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    index: usize,
    context: &ploke_records::tool_contracts::ConciseContext,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(format!("context {}", index + 1)).default_open(index == 0),
        |ui| {
            let _span = tracing::trace_span!(scope::INSPECTOR_CONTEXT).entered();
            tool_kv_path(ui, render_cache, "file", context.file_path.as_ref());
            tool_kv_text(ui, render_cache, "canon", context.canon_path.as_ref());
            tool_kv_text_size_summary(ui, render_cache, "snippet", context.snippet.as_str());
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn render_optional_str(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        tool_kv_text(ui, render_cache, key, value);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_optional_string_list(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<&[String]>,
) {
    if let Some(value) = value {
        render_string_list(ui, render_cache, key, value);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_string_list(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    values: &[String],
) {
    tool_kv_usize(ui, render_cache, key, values.len());
    for (index, value) in values.iter().take(8).enumerate() {
        tool_kv_text(ui, render_cache, list_item_key(index), value.as_str());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_path_list(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    values: &[String],
) {
    tool_kv_usize(ui, render_cache, key, values.len());
    for (index, value) in values.iter().take(8).enumerate() {
        tool_kv_path(ui, render_cache, list_item_key(index), value.as_str());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_optional_u32(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<u32>,
) {
    if let Some(value) = value {
        tool_kv_u32(ui, render_cache, key, value);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_optional_u64(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<u64>,
) {
    if let Some(value) = value {
        tool_kv_u64(ui, render_cache, key, value);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_optional_i32(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<i32>,
) {
    if let Some(value) = value {
        tool_kv_owned(ui, render_cache, key, value.to_string());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_optional_f32(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<f32>,
) {
    if let Some(value) = value {
        tool_kv_owned(ui, render_cache, key, format!("{value:.2}"));
    }
}

fn tool_kv_text(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal_wrapped(|ui| {
        cached_label(ui, render_cache, key);
        cached_monospace_label(ui, render_cache, value);
    });
}

fn tool_kv_path(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal_wrapped(|ui| {
        cached_label(ui, render_cache, key);
        cached_path_value(ui, render_cache, ("tool-path", key, value), value);
    });
}

fn is_pathish_key(key: &str) -> bool {
    matches!(
        key,
        "dir" | "file" | "file path" | "manifest" | "path" | "record" | "repo root"
    ) || key.ends_with(" path")
}

fn tool_kv_owned(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: String,
) {
    tool_kv_text(ui, render_cache, key, value.as_str());
}

fn tool_kv_bool(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: bool,
) {
    tool_kv_text(ui, render_cache, key, if value { "true" } else { "false" });
}

#[cfg(not(target_arch = "wasm32"))]
fn tool_kv_u32(ui: &mut egui::Ui, render_cache: &mut InspectorRenderCache, key: &str, value: u32) {
    let mut buffer = itoa::Buffer::new();
    tool_kv_text(ui, render_cache, key, buffer.format(value));
}

#[cfg(not(target_arch = "wasm32"))]
fn tool_kv_u64(ui: &mut egui::Ui, render_cache: &mut InspectorRenderCache, key: &str, value: u64) {
    let mut buffer = itoa::Buffer::new();
    tool_kv_text(ui, render_cache, key, buffer.format(value));
}

#[cfg(not(target_arch = "wasm32"))]
fn tool_kv_usize(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: usize,
) {
    let mut buffer = itoa::Buffer::new();
    tool_kv_text(ui, render_cache, key, buffer.format(value));
}

fn tool_kv_debug(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: impl std::fmt::Debug,
) {
    tool_kv_owned(ui, render_cache, key, format!("{value:?}"));
}

#[cfg(not(target_arch = "wasm32"))]
fn tool_kv_text_size_summary(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    text: &str,
) {
    let value = render_cache.text_size_summary(text);
    tool_kv_text(ui, render_cache, key, value.as_ref());
}

fn cached_wrapped_monospace_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = render_cache.wrapped_monospace_galley(ui, text);
    ui.add(egui::Label::new(galley))
}

fn list_item_key(index: usize) -> &'static str {
    match index {
        0 => "1",
        1 => "2",
        2 => "3",
        3 => "4",
        4 => "5",
        5 => "6",
        6 => "7",
        _ => "8",
    }
}

fn render_cached_code_block(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_salt: impl std::hash::Hash,
    text: &str,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, CachedTextKind::MonospaceBlock);
    render_diff_galley(ui, id_salt, galley)
}

fn render_agent_turns<'a>(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turns: impl IntoIterator<Item = &'a AgentTurnArtifactMetadata>,
) {
    let mut rendered = false;
    for (index, turn) in turns.into_iter().enumerate() {
        rendered = true;
        show_inspector_collapsing(
            ui,
            egui::CollapsingHeader::new(format!("turn {}", index + 1)).default_open(index == 0),
            |ui| {
                let _span = tracing::trace_span!(scope::INSPECTOR_AGENT_TURN).entered();
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
            },
        );
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
    let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_PATCH).entered();
    {
        let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_HEADER).entered();
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
    }
    render_diff(ui, patch, diff_cache);

    {
        let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_DETAILS_HEADER).entered();
        egui::CollapsingHeader::new("Details")
            .id_salt(("patch-details", patch.patch_id()))
            .default_open(false)
            .show(ui, |ui| {
                let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DETAILS).entered();
                render_patch_details(ui, render_cache, patch);
            });
    }
}

fn render_patch_details(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    patch: PatchInspection<'_>,
) {
    cached_kv_path(ui, render_cache, "target", patch.target_relpath());
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
        let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_TOUCH).entered();
        touched = true;
        render_patch_touch_label(ui, render_cache, touch);
        cached_wrapped_monospace_label(ui, render_cache, touch.replacement);
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
) -> egui::Response {
    let galley = {
        let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_DIFF_CACHE).entered();
        diff_cache.highlighted_patch_galley(ui, patch)
    };
    render_diff_galley(
        ui,
        (
            "ploke_egui.patch_debug.diff",
            patch.child.node.node_id.as_str(),
            patch.patch_id(),
        ),
        galley,
    )
}

fn render_diff_galley(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    galley: Arc<egui::Galley>,
) -> egui::Response {
    let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_DIFF_WIDGET).entered();
    let width = ui.available_width().max(240.0);
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt(id_salt)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.set_min_width(width);
                    ui.add(egui::Label::new(galley).selectable(true));
                });
        })
        .response
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
