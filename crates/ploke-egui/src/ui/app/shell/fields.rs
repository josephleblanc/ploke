use crate::ui::id_display;
use crate::ui::id_display::{
    CopyableArtifactFile, CopyableExpandable, CopyableId, CopyablePath, CopyableRunName,
    CopyableText, InteractiveId, ShortId,
};
use crate::ui::render::text::*;
use eframe::egui;
use std::sync::Arc;

use super::cache::{effective_inspector_content_width, inspector_wrap_width_points};
use super::{InspectorRenderCache, bool_label, format_f64};

pub(super) fn kv(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(key);
        id_display::expandable_id(ui, ("kv", key, value), value);
    });
}

pub(super) fn inspector_theme_layout_key(ui: &egui::Ui) -> u8 {
    crate::ui::theme::tokens_from_ui(ui).cache_theme_key()
}

pub(super) fn fresh_label(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.label(text)
}

pub(super) fn fresh_monospace_label(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.label(egui::RichText::new(text).monospace())
}

pub(super) fn cached_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, CachedTextKind::Plain);
    add_cached_theme_galley(ui, galley, egui::Sense::hover())
}

pub(super) fn cached_monospace_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, CachedTextKind::Monospace);
    add_cached_theme_galley(ui, galley, egui::Sense::hover())
}

pub(super) fn cached_expandable_id(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    full: &str,
) -> egui::Response {
    let Some(_) = ShortId::new(full) else {
        return cached_monospace_label(ui, render_cache, full);
    };

    let value = CopyableId::new(full);
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        true,
        |cache, ui, expanded| cache.id_galley(ui, full, expanded),
    )
}

pub(super) fn cached_path_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    full: &str,
) -> egui::Response {
    let value = CopyablePath::new(full);
    let expandable = value.is_expandable();
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        expandable,
        |cache, ui, expanded| {
            let label = if expanded { full } else { value.tail() };
            cache.text_galley(ui, label, CachedTextKind::Monospace)
        },
    )
}

pub(super) fn cached_artifact_file_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    full: &str,
) -> egui::Response {
    let value = CopyableArtifactFile::new(full);
    let expandable = value.is_expandable();
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        expandable,
        |cache, ui, expanded| {
            let label = if expanded { full } else { value.display_tail() };
            cache.text_galley(ui, label, CachedTextKind::Monospace)
        },
    )
}

pub(super) fn cached_run_name_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    full: &str,
) -> egui::Response {
    let value = CopyableRunName::new(full);
    let expandable = value.is_compact();
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        expandable,
        |cache, ui, expanded| {
            let label = if expanded {
                std::borrow::Cow::Borrowed(full)
            } else {
                value.compact_label()
            };
            cache.text_galley(ui, label.as_ref(), CachedTextKind::Monospace)
        },
    )
}

pub(super) fn cached_copyable_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    value: &impl CopyableExpandable,
    expandable: bool,
    label_galley: impl FnOnce(&mut InspectorRenderCache, &egui::Ui, bool) -> Arc<egui::Galley>,
) -> egui::Response {
    let id = ui.make_persistent_id((
        "ploke-egui.copyable-value",
        value.copy_menu_label(),
        id_source,
    ));
    let mut expanded = ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
    if !expandable {
        expanded = false;
    }
    let galley = label_galley(render_cache, ui, expanded);
    let response = add_cached_theme_galley(ui, galley, egui::Sense::click()).on_hover_ui(|ui| {
        ui.monospace(value.full_text());
        ui.label(value.hover_text(expanded, expandable));
    });

    if response.clicked() && expandable {
        expanded = !expanded;
        ui.data_mut(|data| data.insert_temp(id, expanded));
    }

    id_display::attach_copy_context_menu(&response, value);
    id_display::copy_button(ui, value);

    response
}

pub(super) fn cached_compact_id(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    id: &impl InteractiveId,
) -> egui::Response {
    let full = id.full_id();
    let (compact, expandable) = id.compact_label();
    let _span = tracing::trace_span!(
        "ploke_egui.id_display.show_compact",
        full = full,
        compact = compact,
        expandable = expandable
    )
    .entered();
    let value = CopyableId::new(full);
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        expandable,
        |cache, ui, expanded| {
            let label = if expanded { full } else { compact };
            cache.text_galley(ui, label, CachedTextKind::Monospace)
        },
    )
}

pub(super) fn cached_kv_id(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, key);
        cached_expandable_id(ui, render_cache, ("kv", key, value), value);
    });
}

pub(super) fn cached_kv_path(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, key);
        cached_path_value(ui, render_cache, ("path", key, value), value);
    });
}

pub(super) fn cached_kv_artifact_file(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, key);
        cached_artifact_file_value(ui, render_cache, ("artifact-file", key, value), value);
    });
}

pub(super) fn cached_kv_usize(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: usize,
) {
    let mut buffer = itoa::Buffer::new();
    cached_kv_id(ui, render_cache, key, buffer.format(value));
}

pub(super) fn cached_kv_u32(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: u32,
) {
    let mut buffer = itoa::Buffer::new();
    cached_kv_id(ui, render_cache, key, buffer.format(value));
}

pub(super) fn cached_kv_u64(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: u64,
) {
    let mut buffer = itoa::Buffer::new();
    cached_kv_id(ui, render_cache, key, buffer.format(value));
}

pub(super) fn cached_kv_i64(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: i64,
) {
    let mut buffer = itoa::Buffer::new();
    cached_kv_id(ui, render_cache, key, buffer.format(value));
}

pub(super) fn cached_kv_bool(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: bool,
) {
    cached_kv_id(ui, render_cache, key, bool_label(value));
}

pub(super) fn cached_kv_debug(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: impl std::fmt::Debug,
) {
    cached_kv_id(ui, render_cache, key, format!("{value:?}").as_str());
}

pub(super) fn cached_kv_f64(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: f64,
) {
    cached_kv_text(ui, render_cache, key, format_f64(value).as_str());
}

pub(super) fn cached_kv_optional_f64(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<f64>,
) {
    if let Some(value) = value {
        cached_kv_f64(ui, render_cache, key, value);
    } else {
        cached_kv_text(ui, render_cache, key, "not_recorded");
    }
}

pub(super) fn cached_kv_optional_usize(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<usize>,
) {
    if let Some(value) = value {
        cached_kv_usize(ui, render_cache, key, value);
    } else {
        cached_kv_text(ui, render_cache, key, "not_recorded");
    }
}

pub(super) fn cached_kv_text(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, key);
        cached_monospace_label(ui, render_cache, value);
    });
}

pub(super) fn render_copyable_text_preview(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    label: &str,
    text: &str,
) {
    let id = ui.make_persistent_id(("ploke-egui.copyable-text-preview", label, id_source));
    let mut state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    let value = CopyableText::new(text);
    let content_width = effective_inspector_content_width(ui);
    ui.set_max_width(content_width);

    let header_response = ui.horizontal(|ui| {
        ui.set_max_width(content_width);
        let prev_item_spacing = ui.spacing_mut().item_spacing;
        ui.spacing_mut().item_spacing.x = 0.0;
        state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
        ui.spacing_mut().item_spacing = prev_item_spacing;

        let title_response = ui
            .add(egui::Label::new(label).wrap().sense(egui::Sense::click()))
            .on_hover_text(value.hover_text(state.is_open(), true));
        if title_response.clicked() {
            state.toggle(ui);
        }
        id_display::attach_copy_context_menu(&title_response, &value);
        id_display::copy_button(ui, &value);
    });

    state.show_body_indented(&header_response.response, ui, |ui| {
        render_inspector_wrapped_prose(ui, render_cache, text);
    });
}

/// Wrapped monospace prose constrained to the visible inspector column.
pub(super) fn render_inspector_wrapped_prose(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    render_inspector_wrapped_prose_with_sense(ui, render_cache, text, egui::Sense::click())
}

pub(super) fn render_inspector_wrapped_prose_with_sense(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
    sense: egui::Sense,
) -> egui::Response {
    render_inspector_wrapped_prose_in_width(
        ui,
        render_cache,
        text,
        effective_inspector_content_width(ui),
        sense,
    )
}

/// Wrapped monospace prose at an explicit content width (e.g. after a bullet or tree indent).
pub(super) fn render_inspector_wrapped_prose_in_width(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
    content_width: f32,
    sense: egui::Sense,
) -> egui::Response {
    let content_width = content_width.max(1.0);
    ui.set_max_width(content_width);
    ui.set_width(content_width);
    let galley = render_cache.wrapped_monospace_galley_with_wrap_width(
        ui,
        text,
        inspector_wrap_width_points(content_width),
    );
    add_cached_theme_galley(ui, galley, sense)
}

pub(super) fn cached_wrapped_monospace_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    render_inspector_wrapped_prose(ui, render_cache, text)
}
