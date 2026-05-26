use super::*;

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
fn tool_ui_payload_renderer_keeps_cached_payload_labels_visible() {
    use ploke_records::agent_turn::{ToolUiFieldRecord, ToolUiPayloadRecord, ToolVerbosityRecord};
    use ploke_records::tool_contracts::ToolName;

    let payload = ToolUiPayloadRecord {
        tool: ToolName::ApplyCodeEdit,
        call_id: "call-1".to_owned(),
        request_id: Some("request-1".to_owned()),
        proposal_id: Some("proposal-1".to_owned()),
        summary: "edit staged".to_owned(),
        fields: vec![ToolUiFieldRecord {
            name: "status".to_owned(),
            value: "staged".to_owned(),
        }],
        details: Some("Ready to apply".to_owned()),
        verbosity: ToolVerbosityRecord::Normal,
        error: None,
        error_code: None,
    };
    let mut cache = InspectorRenderCache::default();
    let ctx = egui::Context::default();
    ctx.set_fonts(egui::FontDefinitions::empty());

    let output = ctx.run_ui(Default::default(), |ui| {
        render_tool_ui_payload(ui, &mut cache, &payload);
    });
    let texts = clipped_shape_texts(&output.shapes);

    assert!(texts.iter().any(|text| text.contains("tool ui payload")));
    assert!(texts.iter().any(|text| text.contains("tool")));
    assert!(texts.iter().any(|text| text.contains("apply_code_edit")));
    assert!(texts.iter().any(|text| text.contains("call id")));
    assert!(texts.iter().any(|text| text.contains("call-1")));
    assert!(texts.iter().any(|text| text.contains("summary")));
    assert!(texts.iter().any(|text| text.contains("edit staged")));
    assert!(texts.iter().any(|text| text.contains("status")));
    assert!(texts.iter().any(|text| text.contains("staged")));
}

fn clipped_shape_texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    let mut texts = Vec::new();
    for shape in shapes {
        collect_shape_texts(&shape.shape, &mut texts);
    }
    texts
}

fn collect_shape_texts(shape: &egui::epaint::Shape, texts: &mut Vec<String>) {
    match shape {
        egui::epaint::Shape::Text(text) => texts.push(text.galley.text().to_owned()),
        egui::epaint::Shape::Vec(shapes) => {
            for shape in shapes {
                collect_shape_texts(shape, texts);
            }
        }
        _ => {}
    }
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

        let first = render_diff_galley(ui, ("patch-diff-height", 1), galley.clone());
        let second = render_diff_galley(ui, ("patch-diff-height", 2), galley);

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
