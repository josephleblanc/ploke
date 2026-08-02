use super::super::*;
use super::common::*;
use ploke_db::CallNodeKind;

#[derive(Clone, Copy)]
struct ProcMacroPathCase {
    label: &'static str,
    macro_name: &'static str,
    path: &'static [&'static str],
    arg_count: u32,
}

#[test]
fn axum_proc_macro_body_calls_reach_expand_with() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = function_id_by_name_in_module(&db, &["crate"], "expand_with")?;

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   proc-macro body call rows.
    //
    // Source oracle:
    //   axum-macros/src/lib.rs:377
    //     `derive_from_request` calls `expand_with(item, ...)`.
    //   axum-macros/src/lib.rs:426
    //     `derive_from_request_parts` calls `expand_with(item, ...)`.
    //   axum-macros/src/lib.rs:665
    //     `derive_typed_path` calls `expand_with(input, ...)`.
    //   axum-macros/src/lib.rs:715
    //     `derive_from_ref` calls `expand_with(item, from_ref::expand)`.
    // Expected traversal: each public proc-macro owner reaches the root
    // `expand_with` helper by one resolved path-call edge.
    let cases = [
        ProcMacroPathCase {
            label: "axum-macros/src/lib.rs:377 derive_from_request -> expand_with",
            macro_name: "derive_from_request",
            path: &["expand_with"],
            arg_count: 2,
        },
        ProcMacroPathCase {
            label: "axum-macros/src/lib.rs:426 derive_from_request_parts -> expand_with",
            macro_name: "derive_from_request_parts",
            path: &["expand_with"],
            arg_count: 2,
        },
        ProcMacroPathCase {
            label: "axum-macros/src/lib.rs:665 derive_typed_path -> expand_with",
            macro_name: "derive_typed_path",
            path: &["expand_with"],
            arg_count: 2,
        },
        ProcMacroPathCase {
            label: "axum-macros/src/lib.rs:715 derive_from_ref -> expand_with",
            macro_name: "derive_from_ref",
            path: &["expand_with"],
            arg_count: 2,
        },
    ];

    assert_proc_macro_callers(&db, target, &cases)?;
    assert_sites_match_callers(
        &db,
        target,
        &db.callers_for_target(target)?,
        "proc-macro expand_with callers",
    )?;

    Ok(())
}

#[test]
fn axum_proc_macro_attribute_helpers_reach_expand_attr_with() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = function_id_by_name_in_module(&db, &["crate"], "expand_attr_with")?;

    // Matrix: proc-macro callback argument rows.
    //
    // Source oracle:
    //   axum-macros/src/lib.rs:581
    //     `debug_handler` calls
    //     `expand_attr_with(_attr, input, |attrs, item_fn| ...)`.
    //   axum-macros/src/lib.rs:637
    //     `debug_middleware` calls the same helper.
    //   axum-macros/src/lib.rs:655 is behind `#[cfg(feature = "__private")]`
    //     in this fixture profile, so no `__private_axum_test` row is active.
    // Expected traversal: the active attribute proc macros reach
    // `expand_attr_with` in one edge. Function items and calls inside closure
    // arguments are not fabricated as separate ordinary call edges.
    let cases = [
        ProcMacroPathCase {
            label: "axum-macros/src/lib.rs:581 debug_handler -> expand_attr_with",
            macro_name: "debug_handler",
            path: &["expand_attr_with"],
            arg_count: 3,
        },
        ProcMacroPathCase {
            label: "axum-macros/src/lib.rs:637 debug_middleware -> expand_attr_with",
            macro_name: "debug_middleware",
            path: &["expand_attr_with"],
            arg_count: 3,
        },
    ];

    assert_proc_macro_callers(&db, target, &cases)?;
    assert_sites_match_callers(
        &db,
        target,
        &db.callers_for_target(target)?,
        "proc-macro expand_attr_with callers",
    )?;

    // The callback body itself now owns two closure callsites:
    //   axum-macros/src/lib.rs:581
    //   axum-macros/src/lib.rs:637
    // Both closures call `debug_handler::expand(attrs, &item_fn, FunctionKind::...)`
    // and should traverse to the helper binding in one edge.
    let callback_target =
        function_id_by_name_in_module(&db, &["crate", "debug_handler"], "expand")?;
    let callback_callers = db.callers_for_target(callback_target)?;
    assert_eq!(
        callback_callers.len(),
        2,
        "debug_handler::expand should expose the two closure callback callers: {callback_callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        callback_target,
        &callback_callers,
        "debug_handler::expand callback callers",
    )?;
    for (idx, caller) in callback_callers.iter().enumerate() {
        assert_eq!(
            owner_kind_for_call_body_owner(&db, caller.site.owner_id)?,
            "Closure",
            "debug_handler::expand callback caller should be closure-owned"
        );
        assert_eq!(caller.site.kind, CallSiteKind::Path);
        assert_eq!(
            caller.site.path.as_ref(),
            Some(&path(&["debug_handler", "expand"]))
        );
        assert_eq!(caller.site.arg_count, Some(3));
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(caller.target.relation, CallRelationKind::Function);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Function);
        assert_one_edge_traversal(
            &db,
            TraversalExpectation {
                label: if idx == 0 {
                    "axum-macros/src/lib.rs:581 closure -> debug_handler::expand"
                } else {
                    "axum-macros/src/lib.rs:637 closure -> debug_handler::expand"
                },
                owner: caller.site.owner_id,
                target: callback_target,
                site_id: caller.site.id,
                expected_edge_count: 1,
            },
        )?;
    }

    assert_no_path_rows(&db, &["axum_test", "expand"])?;
    assert_no_path_rows(&db, &["from_ref", "expand"])
}

fn assert_proc_macro_callers(
    db: &Database,
    target: uuid::Uuid,
    cases: &[ProcMacroPathCase],
) -> Result<(), DbError> {
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        cases.len(),
        "target should expose exactly the inspected proc-macro callers: {callers:#?}"
    );

    for case in cases {
        let owner = macro_id_by_name(db, case.macro_name)?;
        let owner_info = db
            .call_node_info(owner)?
            .expect("proc-macro owner should expose call-node metadata");
        assert_eq!(owner_info.kind, CallNodeKind::Macro);
        assert_eq!(owner_info.name, case.macro_name);
        assert!(
            owner_info.file_path.ends_with("axum-macros/src/lib.rs"),
            "{} should be sourced from axum-macros/src/lib.rs: {owner_info:#?}",
            case.label
        );

        let context = db.call_context_for_owner(owner)?;
        let row = row_by_path(&context, case.path);
        assert_eq!(row.site.arg_count, Some(case.arg_count));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
        assert_one_edge_traversal(
            db,
            TraversalExpectation {
                label: case.label,
                owner,
                target,
                site_id: row.site.id,
                expected_edge_count: 1,
            },
        )?;

        let caller = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, case.path);
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
    }

    Ok(())
}
