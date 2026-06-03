use std::sync::Arc;

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
    dark_mode: bool,
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
            dark_mode: ui.visuals().dark_mode,
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
