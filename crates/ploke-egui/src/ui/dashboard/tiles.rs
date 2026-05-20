use eframe::egui;
use egui_tiles::{Behavior, TileId, UiResponse};
use ploke_tree::Graph;
use serde::{Deserialize, Serialize};

use crate::ui::{app::shell, view::GraphSelectionRef};

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize, Default)]
pub enum Pane {
    #[default]
    Graph,
    Inspector,
    PinnedInspector(GraphSelectionRef),
    InspectorSection(GraphSelectionRef, shell::InspectorPanelSection),
    ArtifactDistribution,
    RecentActivity,
    StatsSummary,
    Diagnostics,
}

impl Pane {
    pub fn title(&self) -> String {
        match self {
            Self::Graph => "🌐 Graph".to_owned(),
            Self::Inspector => "🔍 Inspector".to_owned(),
            Self::PinnedInspector(_) => "📌 Inspector".to_owned(),
            Self::InspectorSection(_, section) => format!("📌 {}", section.title()),
            Self::ArtifactDistribution => "📊 Artifact Distribution".to_owned(),
            Self::RecentActivity => "📋 Recent Activity".to_owned(),
            Self::StatsSummary => "🔢 Stats Summary".to_owned(),
            Self::Diagnostics => "🩺 Diagnostics".to_owned(),
        }
    }
}

pub(crate) enum TreeAction {
    Pin(GraphSelectionRef),
    PinSection(GraphSelectionRef, shell::InspectorPanelSection),
    Remove(TileId),
}

pub(crate) struct TreeBehavior<'a> {
    pub(crate) graph: &'a Graph,
    pub(crate) view: &'a mut crate::ui::view::GraphView,
    pub(crate) inspector_cache: &'a mut crate::ui::inspector::InspectorCache,
    pub(crate) inspector_render_cache: &'a mut shell::InspectorRenderCache,
    pub(crate) patch_diff_cache: &'a mut crate::ui::diff::PatchDiffCache,
    pub(crate) graph_revision: crate::ui::inspector::GraphRevision,
    pub(crate) actions: &'a mut Vec<TreeAction>,
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    pub(crate) benchmark_inspector_section: Option<crate::benchmark::BenchmarkInspectorSection>,
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    pub(crate) benchmark_inspector_exclusive: bool,
}

impl<'a> Behavior<Pane> for TreeBehavior<'a> {
    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile_id: TileId, pane: &mut Pane) -> UiResponse {
        let response = UiResponse::None;

        ui.vertical(|ui| match pane {
            Pane::Graph => {
                self.view.show(ui, self.graph);
            }
            Pane::Inspector => {
                ui.horizontal(|ui| {
                    if let Some(detail) = self.view.selected_node_detail(self.graph) {
                        if ui
                            .button("📌 Pin")
                            .on_hover_text("Pin this inspector as a new pane")
                            .clicked()
                        {
                            self.actions.push(TreeAction::Pin(detail.reference));
                        }
                    }
                });
                ui.separator();

                let selected_node = self.view.selected_node(self.graph);
                let selected_reference = selected_node.map(|(reference, _, _)| reference);
                let selected_kind = selected_node.map(|(_, _, kind)| kind);
                let selected_label = selected_node.map(|(_, label, _)| label);
                let selected_sections = self.inspector_cache.sections(
                    self.graph,
                    self.graph_revision,
                    selected_reference,
                );

                #[cfg(all(
                    not(target_arch = "wasm32"),
                    feature = "dev",
                    feature = "native-benchmark"
                ))]
                let inspector_open_state = shell::InspectorOpenState::benchmark(
                    self.benchmark_inspector_section,
                    self.benchmark_inspector_exclusive,
                );
                #[cfg(not(all(
                    not(target_arch = "wasm32"),
                    feature = "dev",
                    feature = "native-benchmark"
                )))]
                let inspector_open_state = shell::InspectorOpenState::default();

                shell::render_right_inspector(
                    ui,
                    self.graph,
                    selected_reference,
                    selected_kind,
                    selected_label,
                    selected_sections,
                    self.inspector_render_cache,
                    self.patch_diff_cache,
                    inspector_open_state,
                    Some(self.actions),
                );
            }
            Pane::PinnedInspector(reference_mut) => {
                let reference: &GraphSelectionRef = reference_mut;
                let sections =
                    self.inspector_cache
                        .sections(self.graph, self.graph_revision, Some(reference));

                let inspector =
                    crate::ui::inspector::SelectionInspector::from_reference(self.graph, reference);
                let (kind, label) = match &inspector {
                    crate::ui::inspector::SelectionInspector::RunForestNode(node) => {
                        (Some("run-forest-node"), Some(node.node.key.as_str()))
                    }
                    crate::ui::inspector::SelectionInspector::Artifact(_) => match reference {
                        GraphSelectionRef::Artifact { key } => {
                            (Some("artifact"), Some(key.as_str()))
                        }
                        _ => (Some("artifact"), None),
                    },
                    crate::ui::inspector::SelectionInspector::Unresolved(_) => (None, None),
                };

                shell::render_right_inspector(
                    ui,
                    self.graph,
                    Some(reference),
                    kind,
                    label,
                    sections,
                    self.inspector_render_cache,
                    self.patch_diff_cache,
                    shell::InspectorOpenState::default(),
                    None,
                );
            }
            Pane::InspectorSection(reference_mut, section) => {
                let reference: &GraphSelectionRef = reference_mut;
                let sections =
                    self.inspector_cache
                        .sections(self.graph, self.graph_revision, Some(reference));

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .inner_margin(egui::Margin {
                                left: 8,
                                right: 0,
                                top: 0,
                                bottom: 0,
                            })
                            .show(ui, |ui| {
                                if let Some(sections) = sections {
                                    match section {
                                        shell::InspectorPanelSection::Identity => {
                                            shell::render_identity(
                                                ui,
                                                self.graph,
                                                sections,
                                                self.inspector_render_cache,
                                            );
                                        }
                                        shell::InspectorPanelSection::Roles => {
                                            shell::render_roles_and_metrics(
                                                ui,
                                                self.graph,
                                                sections,
                                                self.inspector_render_cache,
                                            );
                                        }
                                        shell::InspectorPanelSection::PatchGeneration => {
                                            shell::render_parent_create_for_inspector(
                                                ui,
                                                self.graph,
                                                sections,
                                                self.inspector_render_cache,
                                                shell::InspectorOpenState::default(),
                                            );
                                        }
                                        shell::InspectorPanelSection::RunRecords => {
                                            shell::render_run_records_for_inspector(
                                                ui,
                                                self.graph,
                                                sections,
                                                self.inspector_render_cache,
                                            );
                                        }
                                        shell::InspectorPanelSection::GraphEdges => {
                                            shell::render_graph_edges_for_inspector(
                                                ui,
                                                sections,
                                                self.inspector_render_cache,
                                            );
                                        }
                                        shell::InspectorPanelSection::ArtifactEdges => {
                                            shell::render_artifact_edges_for_inspector(
                                                ui,
                                                sections,
                                                self.inspector_render_cache,
                                            );
                                        }
                                        shell::InspectorPanelSection::Patches => {
                                            shell::render_patches_for_inspector(
                                                ui,
                                                self.graph,
                                                sections,
                                                self.inspector_render_cache,
                                                self.patch_diff_cache,
                                            );
                                        }
                                        shell::InspectorPanelSection::PatchDebug => {
                                            shell::render_patches_for_inspector(
                                                ui,
                                                self.graph,
                                                sections,
                                                self.inspector_render_cache,
                                                self.patch_diff_cache,
                                            );
                                        }
                                        shell::InspectorPanelSection::LlmCalls => {
                                            if let Some(parent_create) = sections.parent_create() {
                                                if let ploke_tree::graph::ParentCreateLookup::Attempt(
                                                    attempt,
                                                ) = parent_create.resolve(self.graph)
                                                {
                                                    shell::render_parent_create_llm_calls_body(
                                                        ui,
                                                        self.graph,
                                                        &attempt,
                                                        sections.run_records(),
                                                        self.inspector_render_cache,
                                                    );
                                                } else {
                                                    ui.label("Attempt not available.");
                                                }
                                            } else {
                                                ui.label("Attempt not available.");
                                            }
                                        }
                                        shell::InspectorPanelSection::SourceRefs => {
                                            shell::render_source_refs_for_inspector(
                                                ui,
                                                self.graph,
                                                sections,
                                                self.inspector_render_cache,
                                            );
                                        }
                                        shell::InspectorPanelSection::ArtifactIds => {
                                            shell::render_artifact_ids_for_inspector(
                                                ui,
                                                self.graph,
                                                sections,
                                                self.inspector_render_cache,
                                            );
                                        }
                                        shell::InspectorPanelSection::Technical => {
                                            ui.label(egui::RichText::new("Run Records").strong());
                                            shell::render_run_records_for_inspector(
                                                ui,
                                                self.graph,
                                                sections,
                                                self.inspector_render_cache,
                                            );

                                            ui.separator();
                                            ui.label(egui::RichText::new("Graph edges").strong());
                                            shell::render_graph_edges_for_inspector(
                                                ui,
                                                sections,
                                                self.inspector_render_cache,
                                            );

                                            ui.separator();
                                            ui.label(
                                                egui::RichText::new("Artifact edges").strong(),
                                            );
                                            shell::render_artifact_edges_for_inspector(
                                                ui,
                                                sections,
                                                self.inspector_render_cache,
                                            );

                                            ui.separator();
                                            ui.label(egui::RichText::new("Source refs").strong());
                                            shell::render_source_refs_for_inspector(
                                                ui,
                                                self.graph,
                                                sections,
                                                self.inspector_render_cache,
                                            );
                                        }
                                        shell::InspectorPanelSection::CandidateComparison => {
                                            shell::render_candidate_comparison_for_inspector(
                                                ui,
                                                self.graph,
                                                sections,
                                                self.inspector_render_cache,
                                            );
                                        }
                                        shell::InspectorPanelSection::LineageAuthority => {
                                            shell::render_lineage_authority_for_inspector(
                                                ui,
                                                self.graph,
                                                sections,
                                                self.inspector_render_cache,
                                            );
                                        }
                                    }
                                } else {
                                    ui.label("No data available.");
                                }
                            });
                    });
            }
            Pane::ArtifactDistribution => {
                crate::ui::dashboard::charts::horizontal_bar_chart(
                    ui,
                    "Artifacts by Kind",
                    &[
                        ("History", self.graph.history.blocks.len() as f32),
                        ("Artifacts", self.graph.artifacts.artifacts.len() as f32),
                        ("Operations", self.graph.operations.operations.len() as f32),
                    ],
                );
            }
            Pane::RecentActivity => {
                ui.label("Recent activity log would go here...");
            }
            Pane::StatsSummary => {
                ui.label(format!(
                    "Total Artifacts: {}",
                    self.graph.artifacts.artifacts.len()
                ));
                ui.label(format!(
                    "Total Operations: {}",
                    self.graph.operations.operations.len()
                ));
            }
            Pane::Diagnostics => {
                if let Some(diagnostics) = self.view.diagnostics() {
                    crate::ui::app::render_diagnostics(ui, &diagnostics);
                } else {
                    ui.label("No diagnostics available for current view.");
                }
            }
        });
        response
    }

    fn top_bar_right_ui(
        &mut self,
        _tiles: &egui_tiles::Tiles<Pane>,
        ui: &mut egui::Ui,
        _tile_id: TileId,
        tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        if let Some(active_pane_id) = tabs.active {
            if ui.button("❌").on_hover_text("Close active pane").clicked() {
                self.actions.push(TreeAction::Remove(active_pane_id));
            }
        }
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        match pane {
            Pane::Inspector => {
                if let Some((_, label, _)) = self.view.selected_node(self.graph) {
                    format!("🔍 Inspector ({})", label).into()
                } else {
                    pane.title().into()
                }
            }
            Pane::PinnedInspector(reference) => {
                let inspector =
                    crate::ui::inspector::SelectionInspector::from_reference(self.graph, reference);
                match inspector {
                    crate::ui::inspector::SelectionInspector::RunForestNode(node) => {
                        format!("📌 {}", node.node.key.as_str()).into()
                    }
                    crate::ui::inspector::SelectionInspector::Artifact(_) => match reference {
                        GraphSelectionRef::Artifact { key } => format!("📌 {}", key).into(),
                        _ => pane.title().into(),
                    },
                    _ => pane.title().into(),
                }
            }
            Pane::InspectorSection(reference, section) => {
                let inspector =
                    crate::ui::inspector::SelectionInspector::from_reference(self.graph, reference);
                match inspector {
                    crate::ui::inspector::SelectionInspector::RunForestNode(node) => {
                        format!("📌 {} ({})", section.title(), node.node.key.as_str()).into()
                    }
                    crate::ui::inspector::SelectionInspector::Artifact(_) => match reference {
                        GraphSelectionRef::Artifact { key } => {
                            format!("📌 {} ({})", section.title(), key).into()
                        }
                        _ => pane.title().into(),
                    },
                    _ => pane.title().into(),
                }
            }
            _ => pane.title().into(),
        }
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            prune_empty_tabs: true,
            prune_single_child_tabs: false,
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }
}

pub fn create_default_tree() -> egui_tiles::Tree<Pane> {
    let mut tiles = egui_tiles::Tiles::default();

    let graph_pane = tiles.insert_pane(Pane::Graph);
    let inspector_pane = tiles.insert_pane(Pane::Inspector);

    let graph_tab = tiles.insert_tab_tile(vec![graph_pane]);
    let inspector_tab = tiles.insert_tab_tile(vec![inspector_pane]);

    // Create a horizontal split between graph and inspector
    let root = tiles.insert_horizontal_tile(vec![graph_tab, inspector_tab]);

    egui_tiles::Tree::new("main_tree", root, tiles)
}
