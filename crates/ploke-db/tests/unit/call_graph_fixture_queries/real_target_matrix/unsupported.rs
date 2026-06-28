use super::super::*;
use super::common::*;

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

    Ok(())
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
        assert_eq!(
            dynamic_rows[0].site.arg_count,
            Some(case.expected_args),
            "dynamic callable row at matrix source line {} should preserve argument count",
            case.source_line
        );
        assert_eq!(dynamic_rows[0].status.status, CallStatusKind::Unsupported);
        assert!(
            dynamic_rows[0].targets.is_empty(),
            "unsupported dynamic callable field call should remain targetless: {dynamic_rows:#?}"
        );
    }

    Ok(())
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
    // Current model contract: the `syn::parse` path and `and_then(f)` receiver
    // are projected in `expand_with`; `expand_attr_with` projects the IIFE
    // dynamic call, but the inner closure-body `f(attr, input)` is absent until
    // nested closure owners are modeled.
    let expand_with = function_id_by_name_in_module(&db, &["crate"], "expand_with")?;
    let expand_with_context = db.call_context_for_owner(expand_with)?;

    let parse_row = row_by_path(&expand_with_context, &["syn", "parse"]);
    assert_external_targetless(parse_row);

    let and_then = row_by_method_receiver(
        &expand_with_context,
        "and_then",
        &CallReceiver::PathCallResult {
            path: path(&["syn", "parse"]),
        },
    );
    assert_targetless_status(and_then, CallStatusKind::Unsupported);

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
    assert!(
        expand_attr_context
            .iter()
            .all(|row| row.site.kind != CallSiteKind::Dynamic || row.site.arg_count != Some(2)),
        "inner closure-body f(attr, input) should remain absent until closure ownership is modeled: {expand_attr_context:#?}"
    );

    Ok(())
}
