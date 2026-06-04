use crate::allocation::scope;
use crate::ui::id_display::ShortId;
use crate::ui::render::text::*;
use crate::ui::text::style as text_style;
use eframe::egui;
use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use ploke_records::tool_contracts::{PersistedToolCallArguments, PersistedToolResultContent};

use super::call_review::{
    CallReviewFilter, CallReviewScanOrderCache, CallReviewScanOrderKey, CallReviewSort,
    build_call_review_scan_order,
};

const INSPECTOR_HOVER_WRAP_WIDTH: f32 = 520.0;

/// Per run-record turn: bulk collapse/expand for the tool-steps ladder (session-local).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct ToolStepsDisclosureScope {
    pub(super) record_key: String,
    pub(super) turn_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ToolStepsDisclosureMode {
    #[default]
    Default,
    Collapsed,
    Expanded,
}

#[derive(Debug, Clone, Copy, Default)]
struct ToolStepsDisclosureEntry {
    generation: u32,
    mode: ToolStepsDisclosureMode,
}

#[derive(Debug, Default)]
pub(crate) struct InspectorRenderCache {
    pub(super) trajectory: super::trajectory::TrajectoryRenderCache,
    tool_steps_disclosure: BTreeMap<ToolStepsDisclosureScope, ToolStepsDisclosureEntry>,
    /// Set for the duration of [`super::run_records::render_run_record_tool_steps`].
    active_tool_steps_disclosure: Option<ToolStepsDisclosureScope>,
    parent_create_rows: BTreeMap<ParentCreateRowsKey, ParentCreateRows>,
    parent_create_row_rebuilds: usize,
    call_review_scan_order: CallReviewScanOrderCache,
    text_galleys: Vec<CachedTextGalley>,
    id_galleys: Vec<CachedIdGalley>,
    #[cfg(not(target_arch = "wasm32"))]
    tool_argument_decodes: Vec<ToolArgumentDecodeEntry>,
    #[cfg(not(target_arch = "wasm32"))]
    tool_result_decodes: Vec<ToolResultDecodeEntry>,
    #[cfg(not(target_arch = "wasm32"))]
    text_size_summaries: Vec<TextSizeSummaryEntry>,
    text_galley_rebuilds: usize,
    id_galley_rebuilds: usize,
    pub(super) run_evidence_source_label: Option<String>,
    pub(super) run_evidence_catalog_error: Option<String>,
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    benchmark_tool_decode_ns: u64,
}

impl InspectorRenderCache {
    pub(super) fn tool_steps_disclosure_scope(
        record_key: &str,
        turn_index: usize,
    ) -> ToolStepsDisclosureScope {
        ToolStepsDisclosureScope {
            record_key: record_key.to_owned(),
            turn_index,
        }
    }

    pub(super) fn with_active_tool_steps_disclosure<R>(
        &mut self,
        scope: Option<ToolStepsDisclosureScope>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous = std::mem::replace(&mut self.active_tool_steps_disclosure, scope);
        let result = f(self);
        self.active_tool_steps_disclosure = previous;
        result
    }

    pub(super) fn active_tool_steps_disclosure_generation(&self) -> u32 {
        self.active_tool_steps_disclosure
            .as_ref()
            .and_then(|scope| self.tool_steps_disclosure.get(scope))
            .map(|entry| entry.generation)
            .unwrap_or(0)
    }

    pub(super) fn tool_steps_default_open(&self, default: bool) -> bool {
        match self
            .active_tool_steps_disclosure
            .as_ref()
            .and_then(|scope| self.tool_steps_disclosure.get(scope))
            .map(|entry| entry.mode)
        {
            Some(ToolStepsDisclosureMode::Collapsed) => false,
            Some(ToolStepsDisclosureMode::Expanded) => true,
            Some(ToolStepsDisclosureMode::Default) | None => default,
        }
    }

    pub(super) fn tool_steps_collapse_all(&mut self, scope: ToolStepsDisclosureScope) {
        let entry = self
            .tool_steps_disclosure
            .entry(scope)
            .or_insert(ToolStepsDisclosureEntry::default());
        entry.generation = entry.generation.wrapping_add(1);
        entry.mode = ToolStepsDisclosureMode::Collapsed;
    }

    pub(super) fn tool_steps_expand_all(&mut self, scope: ToolStepsDisclosureScope) {
        let entry = self
            .tool_steps_disclosure
            .entry(scope)
            .or_insert(ToolStepsDisclosureEntry::default());
        entry.generation = entry.generation.wrapping_add(1);
        entry.mode = ToolStepsDisclosureMode::Expanded;
    }

    pub(crate) fn set_run_evidence_context(
        &mut self,
        source_label: Option<String>,
        catalog_error: Option<String>,
    ) {
        self.run_evidence_source_label = source_label;
        self.run_evidence_catalog_error = catalog_error;
    }

    pub(super) fn run_evidence_source_label(&self) -> Option<&str> {
        self.run_evidence_source_label.as_deref()
    }

    pub(super) fn run_evidence_catalog_error(&self) -> Option<&str> {
        self.run_evidence_catalog_error.as_deref()
    }

    pub(super) fn parent_create_rows(&mut self, key: ParentCreateRowsKey) -> ParentCreateRows {
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

    pub(super) fn call_review_scan_order(
        &mut self,
        protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
        filter: CallReviewFilter,
        sort: CallReviewSort,
    ) -> Arc<[String]> {
        let key = CallReviewScanOrderKey::new(protocol_artifacts, filter, sort);
        if self.call_review_scan_order.key != Some(key) {
            self.call_review_scan_order.rows =
                build_call_review_scan_order(protocol_artifacts, filter, sort);
            self.call_review_scan_order.key = Some(key);
        }
        Arc::clone(&self.call_review_scan_order.rows)
    }

    #[cfg(test)]
    pub(super) fn parent_create_row_rebuilds(&self) -> usize {
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

    pub(super) fn text_galley(
        &mut self,
        ui: &egui::Ui,
        text: &str,
        kind: CachedTextKind,
    ) -> Arc<egui::Galley> {
        let style_key = CachedTextStyleKey::from_ui(ui);
        if let Some(entry) = self.text_galleys.iter().find(|entry| {
            entry.kind == kind
                && entry.style_key == style_key
                && entry.wrap_width_points.is_none()
                && entry.text.as_ref() == text
        }) {
            return entry.galley.clone();
        }

        let galley = layout_cached_text(ui, text, kind);
        self.text_galleys.push(CachedTextGalley {
            kind,
            style_key,
            wrap_width_points: None,
            text: text.into(),
            galley: galley.clone(),
        });
        self.text_galley_rebuilds += 1;
        galley
    }

    pub(super) fn wrapped_monospace_galley(
        &mut self,
        ui: &egui::Ui,
        text: &str,
    ) -> Arc<egui::Galley> {
        let wrap_width_points = cached_wrap_width_points(effective_inspector_content_width(ui));
        self.wrapped_monospace_galley_with_wrap_width(ui, text, wrap_width_points)
    }

    pub(super) fn wrapped_monospace_galley_with_wrap_width(
        &mut self,
        ui: &egui::Ui,
        text: &str,
        wrap_width_points: u32,
    ) -> Arc<egui::Galley> {
        let style_key = CachedTextStyleKey::from_ui(ui);
        if let Some(entry) = self.text_galleys.iter().find(|entry| {
            entry.kind == CachedTextKind::MonospaceWrapped
                && entry.style_key == style_key
                && entry.wrap_width_points == Some(wrap_width_points)
                && entry.text.as_ref() == text
        }) {
            return entry.galley.clone();
        }

        let galley = layout_owned_wrapped_monospace_text(ui, text.to_owned(), wrap_width_points);
        self.text_galleys.push(CachedTextGalley {
            kind: CachedTextKind::MonospaceWrapped,
            style_key,
            wrap_width_points: Some(wrap_width_points),
            text: text.into(),
            galley: galley.clone(),
        });
        self.text_galley_rebuilds += 1;
        galley
    }

    pub(super) fn run_record_text_galley(
        &mut self,
        ui: &egui::Ui,
        text: &str,
        kind: CachedTextKind,
    ) -> Arc<egui::Galley> {
        let style_key = CachedTextStyleKey::from_ui(ui);
        {
            let _span =
                tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_TEXT_CACHE_LOOKUP).entered();
            if let Some(entry) = self.text_galleys.iter().find(|entry| {
                entry.kind == kind
                    && entry.style_key == style_key
                    && entry.wrap_width_points.is_none()
                    && entry.text.as_ref() == text
            }) {
                let _span =
                    tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_TEXT_CACHE_HIT).entered();
                return entry.galley.clone();
            }
        }

        let layout_text = {
            let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_TEXT_LAYOUT_OWNED_STRING)
                .entered();
            text.to_owned()
        };
        let galley = {
            let _span =
                tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_TEXT_EGUI_LAYOUT).entered();
            layout_owned_cached_text(ui, layout_text, kind)
        };
        {
            let _span =
                tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_TEXT_CACHE_STORE).entered();
            self.text_galleys.push(CachedTextGalley {
                kind,
                style_key,
                wrap_width_points: None,
                text: text.into(),
                galley: galley.clone(),
            });
            self.text_galley_rebuilds += 1;
        }
        galley
    }

    pub(super) fn id_galley(
        &mut self,
        ui: &egui::Ui,
        full: &str,
        expanded: bool,
    ) -> Arc<egui::Galley> {
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

    pub(super) fn run_record_id_galley(
        &mut self,
        ui: &egui::Ui,
        full: &str,
        expanded: bool,
    ) -> Arc<egui::Galley> {
        let style_key = CachedTextStyleKey::from_ui(ui);
        {
            let _span =
                tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_ID_CACHE_LOOKUP).entered();
            if let Some(entry) = self.id_galleys.iter().find(|entry| {
                entry.expanded == expanded
                    && entry.style_key == style_key
                    && entry.full.as_ref() == full
            }) {
                let _span =
                    tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_ID_CACHE_HIT).entered();
                return entry.galley.clone();
            }
        }

        let label = {
            let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_ID_LABEL_PREP).entered();
            if expanded {
                full.to_owned()
            } else {
                ShortId::new(full)
                    .map(|short| short.to_string())
                    .unwrap_or_else(|| full.to_owned())
            }
        };
        let galley = {
            let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_ID_EGUI_LAYOUT).entered();
            layout_owned_cached_text(ui, label, CachedTextKind::Monospace)
        };
        {
            let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_ID_CACHE_STORE).entered();
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

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn tool_arguments(
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

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn tool_result(
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

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn text_size_summary(&mut self, text: &str) -> Arc<str> {
        let key = TextSizeSummaryKey::from(text);
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
    pub(super) fn text_galley_rebuilds(&self) -> usize {
        self.text_galley_rebuilds
    }

    #[cfg(test)]
    pub(super) fn id_galley_rebuilds(&self) -> usize {
        self.id_galley_rebuilds
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct ToolArgumentDecodeEntry {
    call_id: Arc<str>,
    tool: Arc<str>,
    raw_arguments: Arc<str>,
    decoded: Arc<PersistedToolCallArguments>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct ToolResultDecodeEntry {
    call_id: Arc<str>,
    tool: Arc<str>,
    raw_content: Arc<str>,
    decoded: Arc<PersistedToolResultContent>,
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

/// Inset from the visible clip edge so labels and chips are not cut mid-glyph.
pub(super) const PANE_CONTENT_EDGE_INSET: f32 = 2.0;

fn pane_content_width_cap(raw: f32) -> f32 {
    (raw - PANE_CONTENT_EDGE_INSET).max(1.0)
}

fn visible_pane_column_width(ui: &egui::Ui) -> f32 {
    let clip_width = ui.clip_rect().width().max(1.0);
    let available = ui.available_width();
    if available.is_finite() && available > 1.0 {
        available.min(clip_width)
    } else {
        clip_width
    }
}

/// Width for inspector prose inside scroll areas and nested drilldowns.
///
/// Horizontal scroll content often reports a very large [`egui::Ui::available_width`]; cap by the
/// visible clip width so wrapped galleys do not extend past the panel.
pub(super) fn effective_inspector_content_width(ui: &egui::Ui) -> f32 {
    pane_content_width_cap(visible_pane_column_width(ui))
}

/// Visible inspector column width from clip only (ignores scroll child min-width).
pub(super) fn inspector_clip_content_width(ui: &egui::Ui) -> f32 {
    ui.clip_rect().width().max(1.0)
}

const EVAL_PANE_COLUMN_WIDTH_TEMP_ID: &str = "ploke_eval_protocol_tile_column_width";

fn eval_pane_column_width_from_ui(ui: &egui::Ui) -> f32 {
    pane_content_width_cap(visible_pane_column_width(ui))
}

/// Pin the Eval & Protocol tile column width for this frame (call on the tile `ui` before scroll).
///
/// Wide call-review grids inflate scroll-content [`egui::Ui::clip_rect`] width; descendants should
/// read the pinned width via [`effective_eval_pane_content_width`] instead of re-measuring inside scroll.
pub(crate) fn refresh_eval_pane_column_width(ui: &egui::Ui) -> f32 {
    let width = eval_pane_column_width_from_ui(ui);
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::new(EVAL_PANE_COLUMN_WIDTH_TEMP_ID), width));
    width
}

/// Content width for Eval & Protocol pane tiles and call-review detail sections.
///
/// `min(finite available_width, clip_rect.width())` so horizontal scroll children do not lay out
/// prose or chips past the visible eval column. Uses the tile pin from [`refresh_eval_pane_column_width`]
/// when present.
pub(super) fn effective_eval_pane_content_width(ui: &egui::Ui) -> f32 {
    ui.ctx()
        .data(|data| data.get_temp(egui::Id::new(EVAL_PANE_COLUMN_WIDTH_TEMP_ID)))
        .unwrap_or_else(|| eval_pane_column_width_from_ui(ui))
}

/// Lay out eval-pane / call-review body at [`effective_eval_pane_content_width`].
pub(super) fn scope_eval_pane_content_width<R>(
    ui: &mut egui::Ui,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let width = effective_eval_pane_content_width(ui);
    ui.scope(|ui| {
        ui.set_max_width(width);
        ui.set_width(width);
        body(ui)
    })
    .inner
}

/// Visible Eval & Protocol tile column width at scroll content roots.
///
/// Alias for [`effective_eval_pane_content_width`]; on the Eval tile call [`refresh_eval_pane_column_width`]
/// on the tile `ui` before [`egui::ScrollArea::show`].
pub(crate) fn eval_pane_content_width(ui: &egui::Ui) -> f32 {
    effective_eval_pane_content_width(ui)
}

pub(super) fn inspector_wrap_width_points(width: f32) -> u32 {
    cached_wrap_width_points(width)
}

fn cached_wrap_width_points(width: f32) -> u32 {
    if width.is_finite() && width > 1.0 {
        width.round() as u32
    } else {
        INSPECTOR_HOVER_WRAP_WIDTH.round() as u32
    }
}

fn inspector_cached_text_color(ui: &egui::Ui) -> egui::Color32 {
    crate::ui::theme::tokens_from_ui(ui).text
}

fn layout_owned_wrapped_monospace_text(
    ui: &egui::Ui,
    text: String,
    wrap_width_points: u32,
) -> Arc<egui::Galley> {
    let font_id = text_style::monospace_font_id(ui.style());
    let job = egui::text::LayoutJob::simple(
        text,
        font_id,
        inspector_cached_text_color(ui),
        wrap_width_points as f32,
    );
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

fn layout_owned_cached_text(
    ui: &egui::Ui,
    text: String,
    kind: CachedTextKind,
) -> Arc<egui::Galley> {
    let style = ui.style();
    let font_id = match kind {
        CachedTextKind::Plain => text_style::body_font_id(style),
        CachedTextKind::Monospace
        | CachedTextKind::MonospaceWarn
        | CachedTextKind::MonospaceError
        | CachedTextKind::MonospaceBlock
        | CachedTextKind::MonospaceWrapped
        | CachedTextKind::MonospaceHover => text_style::monospace_font_id(style),
    };
    let text_color = inspector_cached_text_color(ui);
    let warn_color = text_style::inspector_warn_text_color(ui);
    let error_color = text_style::inspector_error_text_color(ui);
    let _span = tracing::trace_span!(scope::EGUI_TEXT_FONT_LAYOUT).entered();
    match kind {
        CachedTextKind::Plain | CachedTextKind::Monospace => {
            ui.fonts_mut(|fonts| fonts.layout_no_wrap(text, font_id, text_color))
        }
        CachedTextKind::MonospaceWarn => {
            ui.fonts_mut(|fonts| fonts.layout_no_wrap(text, font_id, warn_color))
        }
        CachedTextKind::MonospaceError => {
            ui.fonts_mut(|fonts| fonts.layout_no_wrap(text, font_id, error_color))
        }
        CachedTextKind::MonospaceBlock => {
            let job = egui::text::LayoutJob::simple(text, font_id, text_color, f32::INFINITY);
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        }
        CachedTextKind::MonospaceWrapped => {
            layout_owned_wrapped_monospace_text(ui, text, INSPECTOR_HOVER_WRAP_WIDTH.round() as u32)
        }
        CachedTextKind::MonospaceHover => {
            let job = egui::text::LayoutJob::simple(
                text,
                font_id,
                text_color,
                INSPECTOR_HOVER_WRAP_WIDTH,
            );
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ParentCreateRowsKey {
    pub(super) surface_touches: Option<usize>,
    pub(super) check_status: Option<&'static str>,
    pub(super) apply_status: Option<&'static str>,
    pub(super) tool_requested: usize,
    pub(super) tool_completed: usize,
    pub(super) tool_failed: usize,
    pub(super) edit_proposals: usize,
    pub(super) create_proposals: usize,
    pub(super) expected_file_changes: usize,
    pub(super) candidate_evaluations: usize,
}

#[derive(Debug, Clone)]
pub(super) struct ParentCreateRows {
    pub(super) surface_touches: Option<Arc<str>>,
    pub(super) check_apply: Option<Arc<str>>,
    pub(super) tools: Arc<str>,
    pub(super) llm_proposal: Arc<str>,
    pub(super) child_eval: Arc<str>,
}

#[cfg(test)]
mod tool_steps_disclosure_tests {
    use super::*;

    #[test]
    fn collapse_all_forces_closed_defaults_and_bumps_generation() {
        let mut cache = InspectorRenderCache::default();
        let scope = InspectorRenderCache::tool_steps_disclosure_scope("record-a", 2);
        cache.tool_steps_collapse_all(scope.clone());
        cache.with_active_tool_steps_disclosure(Some(scope.clone()), |cache| {
            assert!(!cache.tool_steps_default_open(true));
        });
        assert_eq!(cache.tool_steps_disclosure[&scope].generation, 1);
        cache.tool_steps_collapse_all(scope.clone());
        assert_eq!(cache.tool_steps_disclosure[&scope].generation, 2);
    }

    #[test]
    fn expand_all_forces_open_defaults() {
        let mut cache = InspectorRenderCache::default();
        let scope = InspectorRenderCache::tool_steps_disclosure_scope("record-b", 0);
        cache.tool_steps_expand_all(scope.clone());
        cache.with_active_tool_steps_disclosure(Some(scope), |cache| {
            assert!(cache.tool_steps_default_open(false));
        });
    }

    #[test]
    fn active_scope_exposes_generation_for_id_salt() {
        let mut cache = InspectorRenderCache::default();
        let scope = InspectorRenderCache::tool_steps_disclosure_scope("r", 1);
        cache.tool_steps_collapse_all(scope.clone());
        cache.with_active_tool_steps_disclosure(Some(scope), |cache| {
            assert_eq!(cache.active_tool_steps_disclosure_generation(), 1);
        });
        assert_eq!(cache.active_tool_steps_disclosure_generation(), 0);
    }
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
