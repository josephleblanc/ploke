use eframe::egui;
use ploke_tree::Graph;

mod bar;

pub(crate) fn render_dashboard_content(ui: &mut egui::Ui, graph: &Graph) {
    let data = [("first", 5.2), ("second", 2.2)];
    let title: &str = "example";
    bar::horizontal_bar_chart(ui, title, &data)
}
