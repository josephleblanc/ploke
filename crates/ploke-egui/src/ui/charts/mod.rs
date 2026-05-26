use eframe::egui;

mod bar;

pub(crate) fn horizontal_bar_chart(ui: &mut egui::Ui, title: &str, data: &[(&str, f32)]) {
    bar::horizontal_bar_chart(ui, title, data);
}
