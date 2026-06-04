use eframe::egui;

use crate::ui::bar_profiles::{metric_fill_width, paint_metric_fill_bar};
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

                    let (rect, _response) = ui.allocate_at_least(
                        egui::vec2(ui.available_width().max(120.0), 14.0),
                        egui::Sense::hover(),
                    );

                    let fill_width = metric_fill_width(*value, max_val, rect.width());
                    paint_metric_fill_bar(ui, rect, fill_width, tokens.accent);

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
