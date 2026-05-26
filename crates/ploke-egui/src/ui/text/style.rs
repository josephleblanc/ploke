//! Shared egui text and compact table styling.

pub(crate) fn body_font_id(style: &egui::Style) -> egui::FontId {
    egui::TextStyle::Body.resolve(style)
}

pub(crate) fn monospace_font_id(style: &egui::Style) -> egui::FontId {
    egui::TextStyle::Monospace.resolve(style)
}

pub(crate) fn inspector_table_header(ui: &egui::Ui, label: &'static str) -> egui::RichText {
    egui::RichText::new(label)
        .strong()
        .color(ui.visuals().text_color())
        .size(body_font_id(ui.style()).size + 1.0)
        .monospace()
}

pub(crate) fn inspector_table_sort_indicator(ui: &egui::Ui, label: &'static str) -> egui::RichText {
    egui::RichText::new(label)
        .color(ui.visuals().weak_text_color())
        .monospace()
}

pub(crate) fn inspector_table_header_background(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().widgets.noninteractive.weak_bg_fill
}

pub(crate) fn inspector_table_hovered_row_background(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().widgets.hovered.weak_bg_fill
}

pub(crate) fn inspector_table_selected_row_background(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().widgets.active.weak_bg_fill
}

pub(crate) fn inspector_table_selected_row_rail(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().selection.bg_fill
}

pub(crate) fn inspector_warn_text_color() -> egui::Color32 {
    egui::Color32::from_rgb(196, 145, 58)
}

pub(crate) fn inspector_error_text_color() -> egui::Color32 {
    egui::Color32::from_rgb(178, 72, 72)
}
