use eframe::egui;

use crate::ui::theme::tokens_from_ui;

/// A simple horizontal bar chart for categorical data.
pub(super) fn horizontal_bar_chart(ui: &mut egui::Ui, title: &str, data: &[(&str, f32)]) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(title).strong());
        ui.add_space(4.0);

        let max_val = data.iter().map(|(_, v)| *v).fold(0.0, f32::max);
        let max_val = max_val.max(1.0);
        let tokens = tokens_from_ui(ui);

        egui::Grid::new(title)
            .num_columns(3)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                for (label, value) in data {
                    ui.label(*label);

                    let progress = value / max_val;
                    let (rect, _response) = ui.allocate_at_least(
                        egui::vec2(ui.available_width().max(120.0), 14.0),
                        egui::Sense::hover(),
                    );

                    ui.painter()
                        .rect_filled(rect, 2.0, ui.visuals().faint_bg_color);

                    let mut bar_rect = rect;
                    bar_rect.set_width((rect.width() * progress).max(2.0));
                    ui.painter().rect_filled(bar_rect, 2.0, tokens.accent);

                    ui.monospace(format_chart_value(*value));
                    ui.end_row();
                }
            });
    });
}

fn format_chart_value(value: f32) -> String {
    if (value.fract()).abs() < f32::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}
