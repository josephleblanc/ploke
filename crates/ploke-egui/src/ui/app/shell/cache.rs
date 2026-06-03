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

#[derive(Debug, Default)]
pub(crate) struct InspectorRenderCache {
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
    #[cfg(all(
        not(target_arch = "wasm32"),
        feature = "dev",
        feature = "native-benchmark"
    ))]
    benchmark_tool_decode_ns: u64,
}

impl InspectorRenderCache {
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
        let style_key = CachedTextStyleKey::from_ui(ui);
        let wrap_width_points = cached_wrap_width_points(ui.available_width());
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

fn cached_wrap_width_points(width: f32) -> u32 {
    if width.is_finite() && width > 1.0 {
        width.round() as u32
    } else {
        INSPECTOR_HOVER_WRAP_WIDTH.round() as u32
    }
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
        ui.visuals().text_color(),
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
    let text_color = ui.visuals().text_color();
    let font_id = match kind {
        CachedTextKind::Plain => text_style::body_font_id(style),
        CachedTextKind::Monospace
        | CachedTextKind::MonospaceWarn
        | CachedTextKind::MonospaceError
        | CachedTextKind::MonospaceBlock
        | CachedTextKind::MonospaceWrapped
        | CachedTextKind::MonospaceHover => text_style::monospace_font_id(style),
    };
    let _span = tracing::trace_span!(scope::EGUI_TEXT_FONT_LAYOUT).entered();
    match kind {
        CachedTextKind::Plain | CachedTextKind::Monospace => {
            ui.fonts_mut(|fonts| fonts.layout_no_wrap(text, font_id, text_color))
        }
        CachedTextKind::MonospaceWarn => ui.fonts_mut(|fonts| {
            fonts.layout_no_wrap(text, font_id, text_style::inspector_warn_text_color(ui))
        }),
        CachedTextKind::MonospaceError => ui.fonts_mut(|fonts| {
            fonts.layout_no_wrap(text, font_id, text_style::inspector_error_text_color(ui))
        }),
        CachedTextKind::MonospaceBlock => {
            let job = egui::text::LayoutJob::simple(
                text,
                font_id,
                ui.visuals().text_color(),
                f32::INFINITY,
            );
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        }
        CachedTextKind::MonospaceWrapped => {
            layout_owned_wrapped_monospace_text(ui, text, INSPECTOR_HOVER_WRAP_WIDTH.round() as u32)
        }
        CachedTextKind::MonospaceHover => {
            let job = egui::text::LayoutJob::simple(
                text,
                font_id,
                ui.visuals().text_color(),
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
