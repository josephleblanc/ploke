mod app;
mod client;
mod model;

use app::WalkUiApp;
use eframe::egui::ViewportBuilder;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([1280.0, 820.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ploke-walk-ui",
        options,
        Box::new(|_cc| Ok(Box::new(WalkUiApp::new()))),
    )
}
