use crate::ui::id_display::{self, CopyableExpandable, CopyableRunName};
use crate::ui::theme::AppTheme;
use crate::ui::view::GraphViewMode;
use eframe::egui;

const INSPECTOR_SCROLL_END_FALLBACK_PADDING: f32 = 240.0;

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "top_strip")
)]
pub(crate) fn render_top_strip(
    ui: &mut egui::Ui,
    theme: &mut AppTheme,
    mode: GraphViewMode,
    run_name: Option<&str>,
    graph_has_content: bool,
) -> bool {
    let mut theme_changed = false;
    ui.horizontal(|ui| {
        ui.label("ploke-egui");
        ui.separator();
        if theme.selector_ui(ui) {
            theme_changed = true;
        }
        ui.separator();
        ui.label(format!("Mode: {}", mode.as_str()));
        if let Some(run_name) = run_name {
            ui.separator();
            ui.label("Run:");
            let value = CopyableRunName::new(run_name);
            let response = ui
                .monospace(run_name)
                .on_hover_text(value.hover_text(false, false));
            id_display::attach_copy_context_menu(&response, &value);
            id_display::copy_button(ui, &value);
        }
        if !graph_has_content {
            ui.separator();
            ui.label("No run loaded");
        }
    });
    theme_changed
}

pub(crate) fn add_inspector_scroll_end_padding(ui: &mut egui::Ui, viewport_height: f32) {
    let anchor_height = ui.spacing().interact_size.y;
    ui.add_space(inspector_scroll_end_padding(viewport_height, anchor_height));
}

fn inspector_scroll_end_padding(viewport_height: f32, anchor_height: f32) -> f32 {
    if viewport_height.is_finite() && anchor_height.is_finite() {
        (viewport_height - anchor_height.max(0.0)).max(0.0)
    } else {
        INSPECTOR_SCROLL_END_FALLBACK_PADDING
    }
}
