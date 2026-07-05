use super::super::*;
use super::common::*;
use super::source_lines::{
    SourceLineFanout, assert_targetless_dynamic_line_fanout_by_method_arg_count,
};
use ploke_test_utils::CORPUS_AXUM_CALL_GRAPH;

#[test]
fn axum_closure_body_call_is_documented_unsupported_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Ground truth:
    //   axum-macros/src/from_ref.rs:23
    //   .map(|(idx, field)| expand_field(&item.ident, idx, field))
    //
    // Contract after regenerating the axum fixture with closure owners:
    // `expand_field(...)` is projected on the nested closure executable owner,
    // not flattened into the parent `from_ref::expand` function.
    let owner = function_id_by_name_in_module(&db, &["crate", "from_ref"], "expand")?;
    let target = function_id_by_name_in_module(&db, &["crate", "from_ref"], "expand_field")?;

    let context = db.call_context_for_owner(owner)?;
    assert!(
        context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&path(&["expand_field"]))),
        "closure-body expand_field call should remain owned by the nested closure, not by from_ref::expand: {context:#?}"
    );

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        1,
        "expand_field should expose the one closure-body caller from axum-macros/src/from_ref.rs:23: {callers:#?}"
    );
    let caller = &callers[0];
    assert_eq!(
        owner_kind_for_call_body_owner(&db, caller.site.owner_id)?,
        "Closure",
        "expand_field call should be owned by the nested closure executable owner"
    );
    assert_eq!(caller.site.path.as_ref(), Some(&path(&["expand_field"])));
    assert_eq!(caller.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller.target.relation, CallRelationKind::Function);
    assert_eq!(caller.target.source_kind, CallSiteKind::Path);
    assert_eq!(caller.target.target_kind, CallTargetKind::Function);
    assert_sites_match_callers(
        &db,
        target,
        &callers,
        "axum-macros/src/from_ref.rs:23 closure-body expand_field call",
    )?;
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-macros/src/from_ref.rs:23 closure-body expand_field",
            owner: caller.site.owner_id,
            target,
            site_id: caller.site.id,
            expected_edge_count: 1,
        },
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
    // are projected in `expand_with`; `expand_attr_with` projects its IIFE
    // dynamic call as a closure target, and `from_request::expand` projects
    // its enum-state IIFE as a closure target. The callable-parameter body
    // call `f(attr, input)` remains absent because its callee is opaque.
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
    let iife = resolved_dynamic_iife_row(&expand_attr_context, "expand_attr IIFE");
    let iife_target = iife.targets[0].target_id;
    assert_resolved_target(
        iife,
        iife_target,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-macros/src/lib.rs:734-738 IIFE dynamic call",
            owner: expand_attr_with,
            target: iife_target,
            site_id: iife.site.id,
            expected_edge_count: 1,
        },
    )?;
    assert!(
        expand_attr_context
            .iter()
            .all(|row| row.site.kind != CallSiteKind::Dynamic || row.site.arg_count != Some(2)),
        "inner closure-body f(attr, input) should remain absent because the callable parameter remains opaque: {expand_attr_context:#?}"
    );

    let from_request_expand =
        function_id_by_name_in_module(&db, &["crate", "from_request"], "expand")?;
    let from_request_context = db.call_context_for_owner(from_request_expand)?;
    let enum_state_iife =
        resolved_dynamic_iife_row(&from_request_context, "from_request enum-state IIFE");
    let enum_state_target = enum_state_iife.targets[0].target_id;
    assert_resolved_target(
        enum_state_iife,
        enum_state_target,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-macros/src/from_request/mod.rs:200-203 IIFE dynamic call",
            owner: from_request_expand,
            target: enum_state_target,
            site_id: enum_state_iife.site.id,
            expected_edge_count: 1,
        },
    )?;

    Ok(())
}

fn resolved_dynamic_iife_row<'a>(
    context: &'a [ploke_db::CallContextRow],
    label: &str,
) -> &'a ploke_db::CallContextRow {
    let rows = context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Dynamic && row.site.arg_count == Some(0))
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        1,
        "{label} should expose one zero-argument dynamic IIFE row: {context:#?}"
    );
    rows[0]
}
