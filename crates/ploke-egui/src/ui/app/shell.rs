//! Stable frame rendering for the operator graph UI.

use eframe::egui;
use std::collections::BTreeMap;

use crate::ui::app::layout::INSPECTOR_MARGIN_INNER;
use crate::ui::eval_protocol::{EvalProtocolDashboard, EvidenceState, PatchProjectionCounts};
use crate::ui::id_display;
use crate::ui::id_display::{InteractiveId, ShortId, TraceId};
use crate::ui::inspector::{
    ArtifactSourceSlot, IdentitySlot, InspectorSections, MetricsSlot, PatchInspection,
    RoleBadgeSet, RunRecordInspection, RunRecordSlot, RunRecordTurnInspection, SelectionEdge,
    SourceRef, UnavailableReason, find_run_forest_node, phase_label, response_finish_reason_label,
    result_class_label, run_forest_node_identity, surface_apply_status_label,
    surface_check_status_label, tool_execution_name, tool_execution_status_label,
    tool_execution_summary, turn_outcome_elapsed_secs, turn_outcome_error, turn_outcome_label,
    turn_outcome_tool_count,
};
use crate::ui::view::{GraphViewDiagnostics, GraphViewMode};
use ploke_records::tool_contracts::{
    PersistedToolCallArguments, PersistedToolResultContent, ToolCallArguments, ToolResultContent,
};
use ploke_tree::Graph;
use ploke_tree::graph::{AgentTurnArtifactMetadata, ParentCreateAttempt, ParentCreateLookup};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct InspectorOpenState {
    force_open: Option<InspectorPanelSection>,
    force_exclusive: bool,
    force_tool_decode_expanded: bool,
}

#[derive(Debug, Default)]
pub(crate) struct InspectorRenderCache {
    parent_create_rows: BTreeMap<ParentCreateRowsKey, ParentCreateRows>,
    parent_create_row_rebuilds: usize,
    text_galleys: Vec<CachedTextGalley>,
    id_galleys: Vec<CachedIdGalley>,
    tool_argument_decodes: Vec<ToolArgumentDecodeEntry>,
    tool_result_decodes: Vec<ToolResultDecodeEntry>,
    text_size_summaries: Vec<TextSizeSummaryEntry>,
    text_galley_rebuilds: usize,
    id_galley_rebuilds: usize,
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    benchmark_tool_decode_expanded: bool,
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    benchmark_tool_decode_ns: u64,
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

    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    pub(crate) fn take_benchmark_tool_decode_ns(&mut self) -> u64 {
        std::mem::take(&mut self.benchmark_tool_decode_ns)
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

    fn run_record_text_galley(
        &mut self,
        ui: &egui::Ui,
        text: &str,
        kind: CachedTextKind,
    ) -> Arc<egui::Galley> {
        let style_key = CachedTextStyleKey::from_ui(ui);
        {
            let _span = tracing::trace_span!("inspector_run_records_text_cache_lookup").entered();
            if let Some(entry) = self.text_galleys.iter().find(|entry| {
                entry.kind == kind && entry.style_key == style_key && entry.text.as_ref() == text
            }) {
                let _span = tracing::trace_span!("inspector_run_records_text_cache_hit").entered();
                return entry.galley.clone();
            }
        }

        let layout_text = {
            let _span =
                tracing::trace_span!("inspector_run_records_text_layout_owned_string").entered();
            text.to_owned()
        };
        let galley = {
            let _span = tracing::trace_span!("inspector_run_records_text_egui_layout").entered();
            layout_owned_cached_text(ui, layout_text, kind)
        };
        {
            let _span = tracing::trace_span!("inspector_run_records_text_cache_store").entered();
            self.text_galleys.push(CachedTextGalley {
                kind,
                style_key,
                text: text.into(),
                galley: galley.clone(),
            });
            self.text_galley_rebuilds += 1;
        }
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

    fn run_record_id_galley(
        &mut self,
        ui: &egui::Ui,
        full: &str,
        expanded: bool,
    ) -> Arc<egui::Galley> {
        let style_key = CachedTextStyleKey::from_ui(ui);
        {
            let _span = tracing::trace_span!("inspector_run_records_id_cache_lookup").entered();
            if let Some(entry) = self.id_galleys.iter().find(|entry| {
                entry.expanded == expanded
                    && entry.style_key == style_key
                    && entry.full.as_ref() == full
            }) {
                let _span = tracing::trace_span!("inspector_run_records_id_cache_hit").entered();
                return entry.galley.clone();
            }
        }

        let label = {
            let _span = tracing::trace_span!("inspector_run_records_id_label_prep").entered();
            if expanded {
                full.to_owned()
            } else {
                ShortId::new(full)
                    .map(|short| short.to_string())
                    .unwrap_or_else(|| full.to_owned())
            }
        };
        let galley = {
            let _span = tracing::trace_span!("inspector_run_records_id_egui_layout").entered();
            layout_owned_cached_text(ui, label, CachedTextKind::Monospace)
        };
        {
            let _span = tracing::trace_span!("inspector_run_records_id_cache_store").entered();
            self.id_galleys.push(CachedIdGalley {
                expanded,
                style_key,
                full: full.into(),
                galley: galley.clone(),
            });
            self.id_galley_rebuilds += 1;
        }
        galley
    }

    fn tool_arguments(
        &mut self,
        call_id: &str,
        tool: &str,
        raw_arguments: &str,
    ) -> Arc<PersistedToolCallArguments> {
        if let Some(index) = self.tool_argument_decodes.iter().position(|entry| {
            entry.call_id.as_ref() == call_id
                && entry.tool.as_ref() == tool
                && entry.raw_arguments.as_ref() == raw_arguments
        }) {
            return self.tool_argument_decodes[index].decoded.clone();
        }

        let decoded = ploke_records::tool_contracts::decode_tool_arguments(tool, raw_arguments);
        self.tool_argument_decodes.push(ToolArgumentDecodeEntry {
            call_id: call_id.into(),
            tool: tool.into(),
            raw_arguments: raw_arguments.into(),
            decoded: Arc::new(decoded),
        });
        let index = self.tool_argument_decodes.len() - 1;
        self.tool_argument_decodes[index].decoded.clone()
    }

    fn tool_result(
        &mut self,
        call_id: &str,
        tool: &str,
        raw_content: &str,
    ) -> Arc<PersistedToolResultContent> {
        if let Some(index) = self.tool_result_decodes.iter().position(|entry| {
            entry.call_id.as_ref() == call_id
                && entry.tool.as_ref() == tool
                && entry.raw_content.as_ref() == raw_content
        }) {
            return self.tool_result_decodes[index].decoded.clone();
        }

        let decoded = ploke_records::tool_contracts::decode_tool_result_content(tool, raw_content);
        self.tool_result_decodes.push(ToolResultDecodeEntry {
            call_id: call_id.into(),
            tool: tool.into(),
            raw_content: raw_content.into(),
            decoded: Arc::new(decoded),
        });
        let index = self.tool_result_decodes.len() - 1;
        self.tool_result_decodes[index].decoded.clone()
    }

    fn text_size_summary(&mut self, text: &str) -> Arc<str> {
        let key = TextSizeSummaryKey {
            bytes: text.len(),
            lines: text.lines().count(),
        };
        if let Some(index) = self
            .text_size_summaries
            .iter()
            .position(|entry| entry.key == key)
        {
            return self.text_size_summaries[index].summary.clone();
        }

        self.text_size_summaries.push(TextSizeSummaryEntry {
            key,
            summary: Arc::<str>::from(format!("{} bytes / {} lines", key.bytes, key.lines)),
        });
        let index = self.text_size_summaries.len() - 1;
        self.text_size_summaries[index].summary.clone()
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

#[derive(Debug)]
struct ToolArgumentDecodeEntry {
    call_id: Arc<str>,
    tool: Arc<str>,
    raw_arguments: Arc<str>,
    decoded: Arc<PersistedToolCallArguments>,
}

#[derive(Debug)]
struct ToolResultDecodeEntry {
    call_id: Arc<str>,
    tool: Arc<str>,
    raw_content: Arc<str>,
    decoded: Arc<PersistedToolResultContent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TextSizeSummaryKey {
    bytes: usize,
    lines: usize,
}

#[derive(Debug)]
struct TextSizeSummaryEntry {
    key: TextSizeSummaryKey,
    summary: Arc<str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CachedTextKind {
    Plain,
    Monospace,
    MonospaceBlock,
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
    layout_owned_cached_text(ui, text.to_owned(), kind)
}

fn layout_owned_cached_text(
    ui: &egui::Ui,
    text: String,
    kind: CachedTextKind,
) -> Arc<egui::Galley> {
    let style = ui.style();
    let text_style = match kind {
        CachedTextKind::Plain => egui::TextStyle::Body,
        CachedTextKind::Monospace | CachedTextKind::MonospaceBlock => egui::TextStyle::Monospace,
    };
    let font_id = text_style.resolve(style);
    let _span = tracing::trace_span!("egui_text_font_layout").entered();
    match kind {
        CachedTextKind::Plain | CachedTextKind::Monospace => {
            ui.fonts_mut(|fonts| fonts.layout_no_wrap(text, font_id, egui::Color32::PLACEHOLDER))
        }
        CachedTextKind::MonospaceBlock => {
            let job = egui::text::LayoutJob::simple(
                text,
                font_id,
                ui.visuals().text_color(),
                f32::INFINITY,
            );
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        }
    }
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
    pub(crate) fn benchmark(
        section: Option<crate::benchmark::BenchmarkInspectorSection>,
        exclusive: bool,
    ) -> Self {
        let force_tool_decode_expanded = matches!(
            section,
            Some(crate::benchmark::BenchmarkInspectorSection::ToolDecode)
        );
        Self {
            force_open: section.map(InspectorPanelSection::from_benchmark),
            force_exclusive: exclusive,
            force_tool_decode_expanded,
        }
    }

    fn tool_decode_expanded(self) -> bool {
        self.force_tool_decode_expanded
    }

    fn open(self, section: InspectorPanelSection) -> Option<bool> {
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
            Self::LineageAuthority => "Lineage Authority",
        }
    }
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
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
    let _span = tracing::trace_span!("inspector_collapsing_header_layout").entered();
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
    let _span = tracing::trace_span!("inspector_collapsing_header_layout").entered();
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
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    {
        render_cache.benchmark_tool_decode_expanded = open_state.tool_decode_expanded();
        render_cache.benchmark_tool_decode_ns = 0;
    }
    {
        let _span = tracing::trace_span!("inspector_scroll_area_layout").entered();
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
                        } else {
                            kv(ui, "selection", "not_applicable");
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
                                } else {
                                    kv(ui, "agent trace", "not_applicable");
                                }
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
                                } else {
                                    kv(ui, "patch", "not_applicable");
                                }
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
                    });
            });
    }
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
        cached_kv_id(
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
            cached_kv_id(ui, render_cache, "record", record_key.as_str());
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

        let review_stats = dashboard.protocol_review_stats();
        render_count_map(ui, render_cache, "overall", &review_stats.overall);
        render_count_map(ui, render_cache, "redundancy", &review_stats.redundancy);
        render_count_map(
            ui,
            render_cache,
            "recoverability",
            &review_stats.recoverability,
        );
    }
}

fn render_count_map(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    prefix: &str,
    counts: &BTreeMap<String, usize>,
) {
    if counts.is_empty() {
        cached_kv_text(ui, render_cache, prefix, "none");
        return;
    }
    for (label, count) in counts {
        let row_label = format!("{prefix}.{label}");
        cached_kv_usize(ui, render_cache, row_label.as_str(), *count);
    }
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
            let _span = tracing::trace_span!("inspector_run_records_resolve_slot").entered();
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
        let _span = tracing::trace_span!("inspector_run_records_row").entered();
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
        run_record_kv_id(
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
        run_record_kv_id(
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
    let _span = tracing::trace_span!("inspector_run_records_widget_row").entered();
    ui.horizontal(|ui| {
        run_record_label(ui, render_cache, key);
        run_record_expandable_id(ui, render_cache, ("kv", key, value), value);
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
        let _span = tracing::trace_span!("inspector_run_records_text_galley").entered();
        render_cache.run_record_text_galley(ui, text, CachedTextKind::Plain)
    };
    let _span = tracing::trace_span!("inspector_run_records_label_widget").entered();
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

    let id = ui.make_persistent_id(("ploke-egui.short-id", id_source));
    let mut expanded = ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
    let galley = {
        let _span = tracing::trace_span!("inspector_run_records_id_galley").entered();
        render_cache.run_record_id_galley(ui, full, expanded)
    };
    let response = {
        let _span = tracing::trace_span!("inspector_run_records_id_widget").entered();
        ui.add(egui::Label::new(galley).sense(egui::Sense::click()))
            .on_hover_text("Click to expand. Right click to copy the full id.")
    };

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

fn run_record_monospace_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = {
        let _span = tracing::trace_span!("inspector_run_records_text_galley").entered();
        render_cache.run_record_text_galley(ui, text, CachedTextKind::Monospace)
    };
    let _span = tracing::trace_span!("inspector_run_records_label_widget").entered();
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn tool_decoded_payload_cache_reuses_stable_records() {
        let mut cache = InspectorRenderCache::default();
        let args_raw = r#"{"token_budget":2048,"search_term":"ToolRequestRecord"}"#;
        let result_raw = r#"{
            "ok":true,
            "file_path":"crates/ploke-records/src/tool_contracts.rs",
            "exists":true,
            "byte_len":128,
            "start_line":1,
            "end_line":4,
            "truncated":false,
            "content":"pub mod tool_contracts;",
            "file_hash":null
        }"#;

        let first_args = cache.tool_arguments("call-1", "request_code_context", args_raw);
        let second_args = cache.tool_arguments("call-1", "request_code_context", args_raw);
        assert!(Arc::ptr_eq(&first_args, &second_args));

        let changed_args = cache.tool_arguments("call-2", "request_code_context", args_raw);
        assert!(!Arc::ptr_eq(&first_args, &changed_args));

        let first_result = cache.tool_result("call-1", "read_file", result_raw);
        let second_result = cache.tool_result("call-1", "read_file", result_raw);
        assert!(Arc::ptr_eq(&first_result, &second_result));
    }

    #[test]
    fn text_size_summary_cache_reuses_stable_size_labels() {
        let mut cache = InspectorRenderCache::default();

        let first = cache.text_size_summary("alpha\nbeta");
        let second = cache.text_size_summary("gamma\nzeta");
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first.as_ref(), "10 bytes / 2 lines");

        let changed = cache.text_size_summary("alpha");
        assert!(!Arc::ptr_eq(&first, &changed));
        assert_eq!(changed.as_ref(), "5 bytes / 1 lines");
    }

    #[test]
    fn tool_ui_payload_renderer_keeps_cached_payload_labels_visible() {
        use ploke_records::agent_turn::{
            ToolUiFieldRecord, ToolUiPayloadRecord, ToolVerbosityRecord,
        };
        use ploke_records::tool_contracts::ToolName;

        let payload = ToolUiPayloadRecord {
            tool: ToolName::ApplyCodeEdit,
            call_id: "call-1".to_owned(),
            request_id: Some("request-1".to_owned()),
            proposal_id: Some("proposal-1".to_owned()),
            summary: "edit staged".to_owned(),
            fields: vec![ToolUiFieldRecord {
                name: "status".to_owned(),
                value: "staged".to_owned(),
            }],
            details: Some("Ready to apply".to_owned()),
            verbosity: ToolVerbosityRecord::Normal,
            error: None,
            error_code: None,
        };
        let mut cache = InspectorRenderCache::default();
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::empty());

        let output = ctx.run_ui(Default::default(), |ui| {
            render_tool_ui_payload(ui, &mut cache, &payload);
        });
        let texts = clipped_shape_texts(&output.shapes);

        assert!(texts.iter().any(|text| text.contains("tool ui payload")));
        assert!(texts.iter().any(|text| text.contains("tool")));
        assert!(texts.iter().any(|text| text.contains("apply_code_edit")));
        assert!(texts.iter().any(|text| text.contains("call id")));
        assert!(texts.iter().any(|text| text.contains("call-1")));
        assert!(texts.iter().any(|text| text.contains("summary")));
        assert!(texts.iter().any(|text| text.contains("edit staged")));
        assert!(texts.iter().any(|text| text.contains("status")));
        assert!(texts.iter().any(|text| text.contains("staged")));
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

    #[test]
    fn patch_diff_galleys_keep_natural_height_when_repeated() {
        use std::fmt::Write as _;

        egui::__run_test_ui(|ui| {
            let mut diff = String::new();
            for line in 0..48 {
                writeln!(&mut diff, "+let value_{line} = {line};").unwrap();
            }
            let job = egui::text::LayoutJob::simple(
                diff,
                egui::FontId::monospace(12.0),
                ui.visuals().text_color(),
                f32::INFINITY,
            );
            let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
            let content_height = galley.size().y;

            let first = render_diff_galley(ui, ("patch-diff-height", 1), galley.clone());
            let second = render_diff_galley(ui, ("patch-diff-height", 2), galley);

            assert!(
                first.rect.height() >= content_height,
                "first diff frame clipped content height: frame={} content={content_height}",
                first.rect.height()
            );
            assert!(
                second.rect.height() >= content_height,
                "second diff frame clipped content height: frame={} content={content_height}",
                second.rect.height()
            );
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
    let _span = tracing::trace_span!("inspector_parent_create_llm_calls").entered();
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
            let _span = tracing::trace_span!("inspector_parent_create_source_status").entered();
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
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    let tool_sections_open = render_cache.benchmark_tool_decode_expanded;
    #[cfg(not(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    )))]
    let tool_sections_open = true;
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("tool call arguments")
            .id_salt(("run-record-tool-arguments", index, call_id))
            .default_open(tool_sections_open),
        |ui| {
            {
                #[cfg(all(
                    not(target_arch = "wasm32"),
                    feature = "dev",
                    feature = "native-benchmark"
                ))]
                let decode_start = std::time::Instant::now();
                let decoded = render_cache.tool_arguments(
                    call_id,
                    tool.request.tool.as_str(),
                    tool.request.arguments.as_str(),
                );
                #[cfg(all(
                    not(target_arch = "wasm32"),
                    feature = "dev",
                    feature = "native-benchmark"
                ))]
                {
                    render_cache.benchmark_tool_decode_ns +=
                        decode_start.elapsed().as_nanos() as u64;
                }
                render_decoded_tool_arguments(ui, render_cache, decoded.as_ref());
            }

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
            .default_open({
                #[cfg(all(
                    not(target_arch = "wasm32"),
                    feature = "dev",
                    feature = "native-benchmark"
                ))]
                {
                    render_cache.benchmark_tool_decode_expanded
                }
                #[cfg(not(all(
                    not(target_arch = "wasm32"),
                    feature = "dev",
                    feature = "native-benchmark"
                )))]
                {
                    false
                }
            }),
        |ui| match &tool.result {
            ploke_records::run_record::ToolResult::Completed(_) => {
                {
                    #[cfg(all(
                        not(target_arch = "wasm32"),
                        feature = "dev",
                        feature = "native-benchmark"
                    ))]
                    let decode_start = std::time::Instant::now();
                    let decoded =
                        render_cache.tool_result(call_id, tool_execution_name(tool), raw_content);
                    #[cfg(all(
                        not(target_arch = "wasm32"),
                        feature = "dev",
                        feature = "native-benchmark"
                    ))]
                    {
                        render_cache.benchmark_tool_decode_ns +=
                            decode_start.elapsed().as_nanos() as u64;
                    }
                    render_decoded_tool_result(ui, render_cache, decoded.as_ref());
                }

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
                tool_kv_text(ui, render_cache, field.name.as_str(), field.value.as_str());
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
                        let _span = tracing::trace_span!("inspector_tool_argument_edit").entered();
                        tool_kv_text(ui, render_cache, "file", edit.file.as_str());
                        tool_kv_text(ui, render_cache, "canon", edit.canon.as_str());
                        tool_kv_debug(ui, render_cache, "node type", edit.node_type);
                        tool_kv_text_size_summary(ui, render_cache, "code", edit.code.as_str());
                    },
                );
            }
        }
        ToolCallArguments::InsertRustItem(args) => {
            tool_kv_text(ui, render_cache, "file", args.file.as_str());
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
            tool_kv_text(ui, render_cache, "file path", args.file_path.as_str());
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
                        let _span = tracing::trace_span!("inspector_tool_argument_patch").entered();
                        tool_kv_text(ui, render_cache, "file", patch.file.as_str());
                        tool_kv_text(ui, render_cache, "reasoning", patch.reasoning.as_str());
                        tool_kv_text_size_summary(ui, render_cache, "diff", patch.diff.as_str());
                    },
                );
            }
        }
        ToolCallArguments::NsRead(args) => {
            tool_kv_text(ui, render_cache, "file", args.file.as_str());
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
            tool_kv_text(ui, render_cache, "dir", args.dir.as_str());
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
            tool_kv_text(ui, render_cache, "file path", result.file_path.as_str());
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
            tool_kv_text(ui, render_cache, "manifest", result.manifest_path.as_str());
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
                    tracing::trace_span!("inspector_tool_result_list_dir_details").entered();
                tool_kv_bool(ui, render_cache, "ok", result.ok);
                tool_kv_text(ui, render_cache, "dir", result.dir.as_str());
                tool_kv_bool(ui, render_cache, "exists", result.exists);
                tool_kv_bool(ui, render_cache, "truncated", result.truncated);
            });
            for (index, entry) in result.entries.iter().take(8).enumerate() {
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new(entry.name.as_str()).default_open(index == 0),
                    |ui| {
                        let _span =
                            tracing::trace_span!("inspector_tool_result_list_dir_entry").entered();
                        tool_kv_text(ui, render_cache, "path", entry.path.as_str());
                        tool_kv_text(ui, render_cache, "kind", entry.kind.as_str());
                        render_optional_u64(ui, render_cache, "size bytes", entry.size_bytes);
                    },
                );
            }
        }
    }
}

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
    render_string_list(ui, render_cache, "files", files);
}

fn render_code_item_query(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    item_name: &str,
    file_path: &str,
    node_kind: &str,
    module_path: &str,
) {
    tool_kv_text(ui, render_cache, "item name", item_name);
    tool_kv_text(ui, render_cache, "file path", file_path);
    tool_kv_text(ui, render_cache, "node kind", node_kind);
    tool_kv_text(ui, render_cache, "module path", module_path);
}

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
            let _span = tracing::trace_span!("inspector_context").entered();
            tool_kv_text(ui, render_cache, "file", context.file_path.as_ref());
            tool_kv_text(ui, render_cache, "canon", context.canon_path.as_ref());
            tool_kv_text_size_summary(ui, render_cache, "snippet", context.snippet.as_str());
        },
    );
}

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

fn tool_kv_u32(ui: &mut egui::Ui, render_cache: &mut InspectorRenderCache, key: &str, value: u32) {
    let mut buffer = itoa::Buffer::new();
    tool_kv_text(ui, render_cache, key, buffer.format(value));
}

fn tool_kv_u64(ui: &mut egui::Ui, render_cache: &mut InspectorRenderCache, key: &str, value: u64) {
    let mut buffer = itoa::Buffer::new();
    tool_kv_text(ui, render_cache, key, buffer.format(value));
}

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
    let galley = render_cache.text_galley(ui, text, CachedTextKind::Monospace);
    ui.add(egui::Label::new(galley).wrap())
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
                let _span = tracing::trace_span!("inspector_agent_turn").entered();
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
    let _span = tracing::trace_span!("inspector_source_refs_iter").entered();
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
    let _span = tracing::trace_span!("inspector_source_refs_row").entered();
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
    let _span = tracing::trace_span!("inspector_patch_debug_patch").entered();
    {
        let _span = tracing::trace_span!("inspector_patch_debug_header").entered();
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
        let _span = tracing::trace_span!("inspector_patch_debug_details_header").entered();
        egui::CollapsingHeader::new("Details")
            .id_salt(("patch-details", patch.patch_id()))
            .default_open(false)
            .show(ui, |ui| {
                let _span = tracing::trace_span!("inspector_patch_details").entered();
                render_patch_details(ui, render_cache, patch);
            });
    }
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
        let _span = tracing::trace_span!("inspector_patch_debug_touch").entered();
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
) -> egui::Response {
    let galley = {
        let _span = tracing::trace_span!("inspector_patch_debug_diff_cache").entered();
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
    let _span = tracing::trace_span!("inspector_patch_debug_diff_widget").entered();
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
