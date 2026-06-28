use super::super::*;
use super::common::*;
use super::source_lines::{
    SourceLineFanout, assert_targetless_dynamic_line_fanout_by_method_arg_count,
};
use ploke_test_utils::CORPUS_AXUM_CALL_GRAPH;

#[test]
fn axum_proc_macro_body_calls_are_documented_unsupported_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = function_id_by_name_in_module(&db, &["crate"], "expand_with")?;

    // Ground truth:
    //   rg -n "\bexpand_with\(" axum-macros/src/lib.rs
    // direct calls at lines 377, 426, 665, and 715.
    //
    // Current model gap: proc-macro item functions are recorded as functions,
    // but their bodies are not visited for structural call-site extraction.
    for macro_name in [
        "derive_from_request",
        "derive_from_request_parts",
        "derive_typed_path",
        "derive_from_ref",
    ] {
        macro_id_by_name(&db, macro_name)?;
    }

    let callers = db.callers_for_target(target)?;
    assert!(
        callers.is_empty(),
        "expand_with should have no resolved callers until proc-macro bodies are visited: {callers:#?}"
    );

    let sites = db.call_sites_for_target(target)?;
    assert!(
        sites.is_empty(),
        "call_sites_for_target should mirror targetless proc-macro-body gap: {sites:#?}"
    );
    assert_no_incoming_traversal_to_target(
        &db,
        target,
        "axum-macros/src/lib.rs:377,426,665,715 expand_with proc-macro body calls",
    )?;

    Ok(())
}

#[test]
fn axum_proc_macro_callback_argument_calls_are_documented_unsupported_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: proc-macro callback argument rows.
    // Source chains:
    //   axum-macros/src/lib.rs:581 calls
    //   `expand_attr_with(_attr, input, |attrs, item_fn| ...)` from
    //   `debug_handler`.
    //   axum-macros/src/lib.rs:637 calls the same helper from
    //   `debug_middleware`.
    //   axum-macros/src/lib.rs:655 passes function item
    //   `axum_test::expand` to `expand_attr_with`.
    //   axum-macros/src/lib.rs:715 passes function item `from_ref::expand` to
    //   `expand_with`.
    // Current model gap: proc-macro bodies are not visited for call-site
    // extraction, and function items passed as callback arguments must not be
    // fabricated as ordinary call edges.
    let expand_attr_with = function_id_by_name_in_module(&db, &["crate"], "expand_attr_with")?;
    let callers = db.callers_for_target(expand_attr_with)?;
    assert!(
        callers.is_empty(),
        "expand_attr_with should have no resolved proc-macro-body callers until those bodies are visited: {callers:#?}"
    );
    let sites = db.call_sites_for_target(expand_attr_with)?;
    assert!(
        sites.is_empty(),
        "call_sites_for_target should mirror targetless expand_attr_with proc-macro-body gap: {sites:#?}"
    );
    assert_no_incoming_traversal_to_target(
        &db,
        expand_attr_with,
        "axum-macros/src/lib.rs:581,637,655 expand_attr_with proc-macro body calls",
    )?;

    assert_no_path_rows(&db, &["expand_attr_with"])?;
    assert_no_path_rows(&db, &["debug_handler", "expand"])?;
    assert_no_path_rows(&db, &["axum_test", "expand"])?;
    assert_no_path_rows(&db, &["from_ref", "expand"])
}

#[test]
fn axum_closure_body_call_is_documented_unsupported_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Ground truth:
    //   axum-macros/src/from_ref.rs:23
    //   .map(|(idx, field)| expand_field(&item.ident, idx, field))
    //
    // Current model gap: closure bodies do not yet get independent call-body
    // ownership, so the `expand_field(...)` call is not projected.
    let owner = function_id_by_name_in_module(&db, &["crate", "from_ref"], "expand")?;
    let target = function_id_by_name_in_module(&db, &["crate", "from_ref"], "expand_field")?;

    let context = db.call_context_for_owner(owner)?;
    let unsupported_path = path(&["expand_field"]);
    assert!(
        context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&unsupported_path)),
        "closure-body expand_field call should remain absent until closure ownership is modeled: {context:#?}"
    );

    let sites = db.call_sites_for_target(target)?;
    assert!(
        sites.is_empty(),
        "expand_field should have no resolved call sites until closure body calls are projected: {sites:#?}"
    );
    assert_no_incoming_traversal_to_target(
        &db,
        target,
        "axum-macros/src/from_ref.rs:23 closure-body expand_field call",
    )?;

    Ok(())
}

#[derive(Clone, Copy)]
struct DynamicGap {
    method_name: &'static str,
    body_marker: &'static str,
    source_line: u32,
    expected_args: u32,
}

#[test]
fn axum_dynamic_callable_fields_are_visible_unsupported_blockers() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Ground truth:
    //   axum/src/boxed.rs:85  (self.into_route)(self.handler, state)
    //   axum/src/boxed.rs:120 (self.into_route)(self.router, state)
    //   axum/src/boxed.rs:159 (self.layer)(self.inner.into_route(state))
    //   axum/src/serve/listener.rs:236 (self.tap_fn)(&mut io)
    let cases = [
        DynamicGap {
            method_name: "into_route",
            body_marker: "(self.into_route)(self.handler, state)",
            source_line: 85,
            expected_args: 2,
        },
        DynamicGap {
            method_name: "into_route",
            body_marker: "(self.into_route)(self.router, state)",
            source_line: 120,
            expected_args: 2,
        },
        DynamicGap {
            method_name: "into_route",
            body_marker: "(self.layer)(self.inner.into_route(state))",
            source_line: 159,
            expected_args: 1,
        },
        DynamicGap {
            method_name: "accept",
            body_marker: "(self.tap_fn)(&mut io)",
            source_line: 236,
            expected_args: 1,
        },
    ];

    for case in cases {
        let owner = method_id_by_name_and_body_substring(&db, case.method_name, case.body_marker)?;
        let context = db.call_context_for_owner(owner)?;
        let dynamic_rows = context
            .iter()
            .filter(|row| row.site.kind == CallSiteKind::Dynamic)
            .collect::<Vec<_>>();

        assert_eq!(
            dynamic_rows.len(),
            1,
            "matrix source line {} should project one dynamic callable field call: {context:#?}",
            case.source_line
        );
        let row = dynamic_rows[0];
        assert_eq!(
            row.site.arg_count,
            Some(case.expected_args),
            "dynamic callable row at matrix source line {} should preserve argument count",
            case.source_line
        );
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert!(
            row.targets.is_empty(),
            "unsupported dynamic callable field call should remain targetless: {dynamic_rows:#?}"
        );
        assert!(
            relations_for_site(&db, row.site.id)?.rows.is_empty(),
            "dynamic callable field row at matrix source line {} should have zero persisted call edges",
            case.source_line
        );
        let label = format!("axum dynamic callable matrix line {}", case.source_line);
        assert_no_traversal_candidates_for_site(&db, owner, row.site.id, &label)?;
    }

    assert_targetless_dynamic_line_fanout_by_method_arg_count(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "into_route",
        2,
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "axum/src/boxed.rs",
            lines: &[85, 120],
        }],
        "(self.into_route)",
    )?;
    assert_targetless_dynamic_line_fanout_by_method_arg_count(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "into_route",
        1,
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "axum/src/boxed.rs",
            lines: &[159],
        }],
        "(self.layer)",
    )?;
    assert_targetless_dynamic_line_fanout_by_method_arg_count(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "accept",
        1,
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "axum/src/serve/listener.rs",
            lines: &[236],
        }],
        "(self.tap_fn)",
    )
}

#[test]
fn axum_macro_callback_rows_are_visible_or_explicitly_absent() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: macro callback rows.
    // Source chain:
    //   axum-macros/src/lib.rs:724 calls
    //   `expand(syn::parse(input).and_then(f))`.
    //   lib.rs:734-738 immediately invokes an IIFE closure whose body calls
    //   `f(attr, input)`.
    //   axum-macros/src/from_request/mod.rs:200-203 immediately invokes an
    //   IIFE closure while deriving enum state.
    // Current model contract: the `syn::parse` path and `and_then(f)` receiver
    // are projected in `expand_with`; `expand_attr_with` projects the IIFE
    // dynamic call, and `from_request::expand` projects its enum-state IIFE.
    // The inner closure-body `f(attr, input)` is absent until nested closure
    // owners are modeled.
    let expand_with = function_id_by_name_in_module(&db, &["crate"], "expand_with")?;
    let expand_with_context = db.call_context_for_owner(expand_with)?;

    let parse_row = row_by_path(&expand_with_context, &["syn", "parse"]);
    assert_external_targetless(parse_row);
    assert_no_traversal_candidates_for_site(
        &db,
        expand_with,
        parse_row.site.id,
        "axum-macros/src/lib.rs:724 syn::parse external callback setup",
    )?;

    let and_then = row_by_method_receiver(
        &expand_with_context,
        "and_then",
        &CallReceiver::PathCallResult {
            path: path(&["syn", "parse"]),
        },
    );
    assert_targetless_status(and_then, CallStatusKind::Unsupported);
    assert!(
        relations_for_site(&db, and_then.site.id)?.rows.is_empty(),
        "axum-macros/src/lib.rs:724 and_then(f) should have zero persisted call edges"
    );
    assert_no_traversal_candidates_for_site(
        &db,
        expand_with,
        and_then.site.id,
        "axum-macros/src/lib.rs:724 and_then(f)",
    )?;

    let expand_attr_with = function_id_by_name_in_module(&db, &["crate"], "expand_attr_with")?;
    let expand_attr_context = db.call_context_for_owner(expand_attr_with)?;
    let iife = assert_targetless_row(
        &expand_attr_context,
        expand_attr_with,
        TargetlessRowCase::dynamic_args(None, 0, CallStatusKind::Unsupported, "expand_attr IIFE"),
    );
    assert!(
        relations_for_site(&db, iife.site.id)?.rows.is_empty(),
        "IIFE dynamic row should not fabricate targets"
    );
    assert_no_traversal_candidates_for_site(
        &db,
        expand_attr_with,
        iife.site.id,
        "axum-macros/src/lib.rs:734-738 IIFE dynamic call",
    )?;
    assert!(
        expand_attr_context
            .iter()
            .all(|row| row.site.kind != CallSiteKind::Dynamic || row.site.arg_count != Some(2)),
        "inner closure-body f(attr, input) should remain absent until closure ownership is modeled: {expand_attr_context:#?}"
    );

    let from_request_expand =
        function_id_by_name_in_module(&db, &["crate", "from_request"], "expand")?;
    let from_request_context = db.call_context_for_owner(from_request_expand)?;
    let enum_state_iife = assert_targetless_row(
        &from_request_context,
        from_request_expand,
        TargetlessRowCase::dynamic_args(
            None,
            0,
            CallStatusKind::Unsupported,
            "from_request enum-state IIFE",
        ),
    );
    assert!(
        relations_for_site(&db, enum_state_iife.site.id)?
            .rows
            .is_empty(),
        "from_request enum-state IIFE dynamic row should not fabricate targets"
    );
    assert_no_traversal_candidates_for_site(
        &db,
        from_request_expand,
        enum_state_iife.site.id,
        "axum-macros/src/from_request/mod.rs:200-203 IIFE dynamic call",
    )?;

    Ok(())
}
