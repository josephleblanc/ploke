use eframe::egui;
use std::sync::Arc;

/// Paint a cached inspector galley with explicit theme text color (see edge labels).
pub(crate) fn add_cached_theme_galley(
    ui: &mut egui::Ui,
    galley: Arc<egui::Galley>,
    sense: egui::Sense,
) -> egui::Response {
    let text_color = ui.visuals().text_color();
    let (galley_pos, galley, response) = egui::Label::new(galley).sense(sense).layout_in_ui(ui);
    if ui.is_rect_visible(response.rect) {
        ui.painter().add(
            egui::epaint::TextShape::new(galley_pos, galley, text_color)
                .with_override_text_color(text_color),
        );
    }
    response
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

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextSizeSummaryKey {
    pub(crate) bytes: usize,
    pub(crate) lines: usize,
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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
