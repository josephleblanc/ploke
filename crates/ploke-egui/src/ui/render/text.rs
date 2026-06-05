use eframe::egui;
use std::sync::Arc;

/// Paint a theme-keyed inspector galley through the standard label widget path.
pub(crate) fn add_cached_theme_galley(
    ui: &mut egui::Ui,
    galley: Arc<egui::Galley>,
    sense: egui::Sense,
) -> egui::Response {
    ui.add(egui::Label::new(galley).selectable(true).sense(sense))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CachedTextKind {
    Plain,
    Monospace,
    MonospaceWarn,
    MonospaceError,
    MonospaceBlock,
    MonospaceWrapped,
    MonospaceHover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextSizeSummaryKey {
    pub(crate) bytes: usize,
    pub(crate) lines: usize,
}

#[derive(Debug)]
pub(crate) struct TextSizeSummaryEntry {
    pub(crate) key: TextSizeSummaryKey,
    pub(crate) summary: Arc<str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CachedTextStyleKey {
    theme_key: u8,
    pixels_per_point: u32,
}

impl From<&str> for TextSizeSummaryKey {
    fn from(v: &str) -> Self {
        Self {
            bytes: v.len(),
            lines: v.lines().count(),
        }
    }
}

impl CachedTextStyleKey {
    pub(crate) fn from_ui(ui: &egui::Ui) -> Self {
        Self {
            theme_key: crate::ui::theme::tokens_from_ui(ui).cache_theme_key(),
            pixels_per_point: ui.ctx().pixels_per_point().to_bits(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CachedTextGalley {
    pub(crate) kind: CachedTextKind,
    pub(crate) style_key: CachedTextStyleKey,
    pub(crate) wrap_width_points: Option<u32>,
    pub(crate) text: Box<str>,
    pub(crate) galley: Arc<egui::Galley>,
}
