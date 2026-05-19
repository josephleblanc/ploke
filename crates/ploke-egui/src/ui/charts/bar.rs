use eframe::egui;

/// A simple horizontal bar chart for categorical data.
pub(super) fn horizontal_bar_chart(ui: &mut egui::Ui, title: &str, data: &[(&str, f32)]) {
    ui.vertical(|ui| {
        ui.heading(title);
        ui.add_space(8.0);

        let max_val = data.iter().map(|(_, v)| *v).fold(0.0, f32::max);

        egui::Grid::new(title)
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                for (label, value) in data {
                    ui.label(*label);

                    let progress = value / max_val;
                    let (rect, _response) = ui.allocate_at_least(
                        egui::vec2(ui.available_width() - 20.0, 16.0),
                        egui::Sense::hover(),
                    );

                    // Draw background
                    ui.painter()
                        .rect_filled(rect, 2.0, ui.visuals().faint_bg_color);

                    // Draw bar
                    let mut bar_rect = rect;
                    bar_rect.set_width(rect.width() * progress);
                    ui.painter()
                        .rect_filled(bar_rect, 2.0, ui.visuals().selection.bg_fill);

                    ui.end_row();
                }
            });
    });
}
