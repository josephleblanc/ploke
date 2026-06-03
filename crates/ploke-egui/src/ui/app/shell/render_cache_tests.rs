use super::{cache::ParentCreateRowsKey, *};
use crate::ui::theme::{AppTheme, NamedScheme};

#[test]
fn parent_create_render_rows_reuse_allocated_text_for_stable_key() {
    let key = ParentCreateRowsKey {
        surface_touches: Some(2),
        check_status: Some("passed"),
        apply_status: Some("applied"),
        tool_requested: 3,
        tool_completed: 2,
        tool_failed: 1,
        edit_proposals: 4,
        create_proposals: 1,
        expected_file_changes: 5,
        candidate_evaluations: 6,
    };
    let mut cache = InspectorRenderCache::default();

    let first_tools = Arc::as_ptr(&cache.parent_create_rows(key).tools);
    assert_eq!(cache.parent_create_row_rebuilds(), 1);

    let second_tools = Arc::as_ptr(&cache.parent_create_rows(key).tools);
    assert_eq!(cache.parent_create_row_rebuilds(), 1);
    assert_eq!(first_tools, second_tools);

    let mut changed = key;
    changed.tool_completed += 1;
    let third_tools = Arc::as_ptr(&cache.parent_create_rows(changed).tools);
    assert_eq!(cache.parent_create_row_rebuilds(), 2);
    assert_ne!(first_tools, third_tools);
}

#[test]
fn inspector_text_galley_cache_invalidates_on_theme_layout_key_change() {
    let mut cache = InspectorRenderCache::default();

    egui::__run_test_ui(|ui| {
        for scheme in [
            NamedScheme::TokyoNight,
            NamedScheme::GruvboxLight,
            NamedScheme::Dracula,
            NamedScheme::OneDark,
        ] {
            AppTheme { scheme }.apply_to_context(ui.ctx());
            let galley = cache.text_galley(ui, "turn", CachedTextKind::Monospace);
            let color = galley
                .job
                .sections
                .first()
                .map(|section| section.format.color)
                .expect("galley section");
            assert_eq!(color, scheme.tokens().text, "galley color for {scheme:?}");
        }
        assert_eq!(cache.text_galley_rebuilds(), 4);
    });
}

#[test]
fn inspector_theme_layout_key_tracks_active_palette() {
    use super::fields::inspector_theme_layout_key;

    egui::__run_test_ui(|ui| {
        AppTheme {
            scheme: NamedScheme::TokyoNight,
        }
        .apply_to_context(ui.ctx());
        let dark_key = inspector_theme_layout_key(ui);

        AppTheme {
            scheme: NamedScheme::GruvboxLight,
        }
        .apply_to_context(ui.ctx());
        let light_key = inspector_theme_layout_key(ui);

        assert_ne!(dark_key, light_key);
    });
}

#[test]
fn edge_label_palette_text_tracks_active_theme() {
    use crate::ui::theme::PaletteTokens;

    let dark = PaletteTokens::for_scheme(NamedScheme::TokyoNight);
    let light = PaletteTokens::for_scheme(NamedScheme::GruvboxLight);
    assert_ne!(dark.edge_label_text(), light.edge_label_text());
    assert_ne!(dark.edge_label_background(), light.edge_label_background());
}

#[test]
fn inspector_text_galley_uses_active_theme_text_color() {
    let mut cache = InspectorRenderCache::default();

    egui::__run_test_ui(|ui| {
        AppTheme {
            scheme: NamedScheme::TokyoNight,
        }
        .apply_to_context(ui.ctx());

        let dark = cache.text_galley(ui, "turn", CachedTextKind::Monospace);
        let dark_color = dark
            .job
            .sections
            .first()
            .map(|section| section.format.color)
            .expect("galley section");
        assert_eq!(dark_color, NamedScheme::TokyoNight.tokens().text);

        AppTheme {
            scheme: NamedScheme::GruvboxLight,
        }
        .apply_to_context(ui.ctx());

        let light = cache.text_galley(ui, "turn", CachedTextKind::Monospace);
        let light_color = light
            .job
            .sections
            .first()
            .map(|section| section.format.color)
            .expect("galley section");
        assert_eq!(light_color, NamedScheme::GruvboxLight.tokens().text);
        assert_ne!(dark_color, light_color);
    });
}

#[test]
fn inspector_text_galley_cache_reuses_stable_labels() {
    let mut cache = InspectorRenderCache::default();

    egui::__run_test_ui(|ui| {
        let first = cache.text_galley(ui, "artifact", CachedTextKind::Monospace);
        assert_eq!(cache.text_galley_rebuilds(), 1);

        let second = cache.text_galley(ui, "artifact", CachedTextKind::Monospace);
        assert_eq!(cache.text_galley_rebuilds(), 1);
        assert!(Arc::ptr_eq(&first, &second));

        let plain = cache.text_galley(ui, "artifact", CachedTextKind::Plain);
        assert_eq!(cache.text_galley_rebuilds(), 2);
        assert!(!Arc::ptr_eq(&first, &plain));
    });
}

#[test]
fn inspector_id_galley_cache_reuses_short_id_labels() {
    let mut cache = InspectorRenderCache::default();
    let id = "artifact:git-commit:deadbeefcafebabe";

    egui::__run_test_ui(|ui| {
        let first = cache.id_galley(ui, id, false);
        assert_eq!(cache.id_galley_rebuilds(), 1);

        let second = cache.id_galley(ui, id, false);
        assert_eq!(cache.id_galley_rebuilds(), 1);
        assert!(Arc::ptr_eq(&first, &second));

        let expanded = cache.id_galley(ui, id, true);
        assert_eq!(cache.id_galley_rebuilds(), 2);
        assert!(!Arc::ptr_eq(&first, &expanded));
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn tool_decoded_payload_cache_reuses_stable_records() {
    let mut cache = InspectorRenderCache::default();
    let args_raw = r#"{"token_budget":2048,"search_term":"ToolRequestRecord"}"#;
    let result_raw = r#"{
        "ok":true,
        "file_path":"crates/ploke-records/src/tool_contracts.rs",
        "exists":true,
        "byte_len":128,
        "start_line":1,
        "end_line":4,
        "truncated":false,
        "content":"pub mod tool_contracts;",
        "file_hash":null
    }"#;

    let first_args = cache.tool_arguments("call-1", "request_code_context", args_raw);
    let second_args = cache.tool_arguments("call-1", "request_code_context", args_raw);
    assert!(Arc::ptr_eq(&first_args, &second_args));

    let changed_args = cache.tool_arguments("call-2", "request_code_context", args_raw);
    assert!(!Arc::ptr_eq(&first_args, &changed_args));

    let first_result = cache.tool_result("call-1", "read_file", result_raw);
    let second_result = cache.tool_result("call-1", "read_file", result_raw);
    assert!(Arc::ptr_eq(&first_result, &second_result));
}

#[test]
fn text_size_summary_cache_reuses_stable_size_labels() {
    let mut cache = InspectorRenderCache::default();

    let first = cache.text_size_summary("alpha\nbeta");
    let second = cache.text_size_summary("gamma\nzeta");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.as_ref(), "10 bytes / 2 lines");

    let changed = cache.text_size_summary("alpha");
    assert!(!Arc::ptr_eq(&first, &changed));
    assert_eq!(changed.as_ref(), "5 bytes / 1 lines");
}

#[test]
fn run_llm_trace_header_labels_use_theme_override_paint() {
    use super::fields::{cached_label, cached_monospace_label};

    let mut cache = InspectorRenderCache::default();
    let mut responses: Vec<egui::Rect> = Vec::new();

    egui::__run_test_ui(|ui| {
        AppTheme {
            scheme: NamedScheme::TokyoNight,
        }
        .apply_to_context(ui.ctx());

        let mut push = |response: egui::Response| {
            if response.rect.width() > 1.0 && response.rect.height() > 1.0 {
                responses.push(response.rect);
            }
        };

        ui.horizontal(|ui| {
            push(cached_label(ui, &mut cache, "records"));
            push(cached_monospace_label(ui, &mut cache, "19"));
        });
        ui.horizontal(|ui| {
            push(cached_label(ui, &mut cache, "turns"));
            push(cached_monospace_label(ui, &mut cache, "4"));
        });

        ui.horizontal(|ui| {
            push(cached_label(ui, &mut cache, "manifest"));
            push(cached_monospace_label(ui, &mut cache, "manifest:bench:alpha"));
        });
        ui.horizontal(|ui| {
            push(cached_label(ui, &mut cache, "turn"));
            push(cached_monospace_label(ui, &mut cache, "1"));
        });

        assert!(cache.text_galley_rebuilds() > 0);
        assert!(cache.id_galley_rebuilds() == 0);

        for (left_index, left) in responses.iter().enumerate() {
            for right in responses.iter().skip(left_index + 1) {
                let overlap = left.intersect(*right);
                assert!(
                    overlap.width() < 2.0 || overlap.height() < 2.0,
                    "run llm trace header widgets overlap: left={left:?} right={right:?} overlap={overlap:?}"
                );
            }
        }
    });
}

#[test]
fn patch_diff_galleys_keep_natural_height_when_repeated() {
    use std::fmt::Write as _;

    egui::__run_test_ui(|ui| {
        let mut diff = String::new();
        for line in 0..48 {
            writeln!(&mut diff, "+let value_{line} = {line};").unwrap();
        }
        let job = egui::text::LayoutJob::simple(
            diff,
            egui::FontId::monospace(12.0),
            ui.visuals().text_color(),
            f32::INFINITY,
        );
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        let content_height = galley.size().y;

        let first = render_diff_galley(ui, ("patch-diff-height", 1), galley.clone(), false);
        let second = render_diff_galley(ui, ("patch-diff-height", 2), galley, false);

        assert!(
            first.rect.height() >= content_height,
            "first diff frame clipped content height: frame={} content={content_height}",
            first.rect.height()
        );
        assert!(
            second.rect.height() >= content_height,
            "second diff frame clipped content height: frame={} content={content_height}",
            second.rect.height()
        );
    });
}
