use crate::allocation::scope;
use crate::ui::app::layout::INSPECTOR_MARGIN_INNER;
use crate::ui::eval_protocol::EvalProtocolDashboard;
use crate::ui::inspector::InspectorSections;
use eframe::egui;
use ploke_tree::Graph;

use super::eval_protocol::{
    render_selected_eval_protocol_call_review, selected_eval_protocol_call_review_key,
};
use super::fields::{cached_kv_artifact_file, cached_kv_id, kv};
use super::{
    InspectorRenderCache, add_inspector_scroll_end_padding, render_artifact_edges_for_inspector,
    render_artifact_ids_for_inspector, render_candidate_comparison_for_inspector,
    render_eval_protocol_for_graph, render_graph_edges_for_inspector, render_identity,
    render_lineage_authority_for_inspector, render_parent_create_for_inspector,
    render_patches_for_inspector, render_roles_and_metrics, render_run_level_llm_trace_for_graph,
    render_run_level_patch_generation_for_graph, render_run_records_for_inspector,
    render_selection_drilldown_for_inspector, render_source_refs_for_inspector,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum EvalProtocolRenderMode {
    #[default]
    Full,
    CallReviewScanOnly,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct InspectorOpenState {
    force_open: Option<InspectorPanelSection>,
    force_exclusive: bool,
}

impl InspectorOpenState {
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "native-benchmark",
        any(test, feature = "dev")
    ))]
    pub(crate) fn benchmark(
        section: Option<crate::benchmark::BenchmarkInspectorSection>,
        exclusive: bool,
    ) -> Self {
        Self {
            force_open: section.map(InspectorPanelSection::from_benchmark),
            force_exclusive: exclusive,
        }
    }

    pub(crate) fn open(self, section: InspectorPanelSection) -> Option<bool> {
        if self.force_exclusive {
            Some(self.force_open == Some(section))
        } else {
            (self.force_open == Some(section)).then_some(true)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum InspectorPanelSection {
    Identity,
    #[serde(alias = "RunReview")]
    EvalProtocol,
    Roles,
    PatchGeneration,
    LlmCalls,
    Patches,
    RunRecords,
    GraphEdges,
    ArtifactEdges,
    SourceRefs,
    ArtifactIds,
    Technical,
    PatchDebug,
    CandidateComparison,
    SelectionStory,
    LineageAuthority,
}

impl InspectorPanelSection {
    pub(crate) fn title(&self) -> &'static str {
        match self {
            Self::Identity => "Identity",
            Self::EvalProtocol => "Eval & Protocol",
            Self::Roles => "Roles",
            Self::PatchGeneration => "Patch Generation",
            Self::LlmCalls => "LLM Calls",
            Self::RunRecords => "Run Records",
            Self::GraphEdges => "Graph Edges",
            Self::ArtifactEdges => "Artifact Edges",
            Self::Patches => "Patches",
            Self::SourceRefs => "Source Refs",
            Self::ArtifactIds => "Artifact IDs",
            Self::Technical => "Technical",
            Self::PatchDebug => "Patch Debug",
            Self::CandidateComparison => "Candidate Comparison",
            Self::SelectionStory => "Selection Story",
            Self::LineageAuthority => "Lineage Authority",
        }
    }
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "native-benchmark",
        any(test, feature = "dev")
    ))]
    fn from_benchmark(section: crate::benchmark::BenchmarkInspectorSection) -> Self {
        match section {
            crate::benchmark::BenchmarkInspectorSection::LlmCalls => Self::LlmCalls,
            crate::benchmark::BenchmarkInspectorSection::RunRecords => Self::RunRecords,
            crate::benchmark::BenchmarkInspectorSection::GraphEdges => Self::GraphEdges,
            crate::benchmark::BenchmarkInspectorSection::ArtifactEdges => Self::ArtifactEdges,
            crate::benchmark::BenchmarkInspectorSection::PatchDebug => Self::PatchDebug,
            crate::benchmark::BenchmarkInspectorSection::SourceRefs => Self::SourceRefs,
            crate::benchmark::BenchmarkInspectorSection::ArtifactIds => Self::ArtifactIds,
            crate::benchmark::BenchmarkInspectorSection::ToolDecode => Self::RunRecords,
        }
    }
}

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

                        if let Some(crate::ui::view::GraphSelectionRef::Selection { entry_id }) =
                            selection_ref
                        {
                            show_inspector_section_collapsing(
                                ui,
                                "Selection Story",
                                InspectorPanelSection::SelectionStory,
                                open_state
                                    .open(InspectorPanelSection::SelectionStory)
                                    .or(Some(true)),
                                selection_ref,
                                &mut actions,
                                |ui| {
                                    render_selection_drilldown_for_inspector(
                                        ui,
                                        graph,
                                        entry_id,
                                        render_cache,
                                    );
                                },
                            );
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
