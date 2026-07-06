use super::*;
use ploke_db::CallPathOptions;

#[test]
fn fixture_context_reads_projected_path_resolution_forms() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let nested_target =
        function_id_by_name_in_module(&db, &["crate", "local_mod"], "nested_target")?;
    let imported_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "imported_target")?;
    let globbed_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "globbed_target")?;
    let cases: [(&[&str], &str, Vec<String>, Uuid); 11] = [
        (
            &["crate"],
            "call_unqualified_local_target",
            path(&["local_target"]),
            local_target,
        ),
        (
            &["crate", "local_mod"],
            "call_self_nested_target",
            path(&["self", "nested_target"]),
            nested_target,
        ),
        (
            &["crate"],
            "call_crate_module_nested_target",
            path(&["crate", "local_mod", "nested_target"]),
            nested_target,
        ),
        (
            &["crate"],
            "call_self_module_nested_target",
            path(&["self", "local_mod", "nested_target"]),
            nested_target,
        ),
        (
            &["crate", "super_path_scope"],
            "call_super_local_target",
            path(&["super", "local_target"]),
            local_target,
        ),
        (
            &["crate"],
            "call_imported_alias_target",
            path(&["imported_alias"]),
            imported_target,
        ),
        (
            &["crate"],
            "call_glob_imported_target",
            path(&["globbed_target"]),
            globbed_target,
        ),
        (
            &["crate"],
            "call_reexported_target",
            path(&["reexported_target"]),
            imported_target,
        ),
        (
            &["crate"],
            "call_imported_module_target",
            path(&["targets_alias", "globbed_target"]),
            globbed_target,
        ),
        (
            &["crate", "grouped_function_import_scope"],
            "call_grouped_imported_alias_target",
            path(&["grouped_alias"]),
            imported_target,
        ),
        (
            &["crate", "grouped_function_import_scope"],
            "call_grouped_imported_globbed_target",
            path(&["grouped_globbed_alias"]),
            globbed_target,
        ),
    ];

    for (module_path, owner_name, expected_path, target) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_resolves_single_caller_function_pointer_parameter() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_single_function_pointer_param")?;
    let caller = function_id_by_name(&db, "call_single_function_pointer_param_with_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;
    let helper = function_id_by_name(&db, "call_single_function_pointer_param")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1503-1505:
    // `call_single_function_pointer_param(f: fn() -> i32) { f() }` has one
    // local caller in this fixture. The caller passes `local_target`, so the
    // parameter call is exact instead of a generic function-pointer blocker.
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "parameter owner context rows: {context:#?}"
    );
    let row = row_by_path(&context, &["f"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let callers = db.callers_for_target(target)?;
    let caller_row = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, &["f"]);
    assert_eq!(caller_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller_row.target.target_id, target);
    assert_eq!(caller_row.target.relation, CallRelationKind::Function);

    let paths = db.call_paths_from_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == owner && path.end_id == target && path.depth == 1)
        .expect("parameter owner should have a one-hop path to local_target");
    assert_eq!(path.edges[0].caller_id, owner);
    assert_eq!(path.edges[0].callee_id, target);
    assert_eq!(path.edges[0].relation, CallRelationKind::Function);

    // The source oracle's caller remains a normal direct path call to the
    // helper function; argument proof is additional resolver evidence, not a
    // replacement for the caller's own edge.
    let caller_context = db.call_context_for_owner(caller)?;
    let helper_call = row_by_path(&caller_context, &["call_single_function_pointer_param"]);
    assert_eq!(helper_call.site.arg_count, Some(1));
    assert_resolved_target(
        helper_call,
        helper,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    Ok(())
}

#[test]
fn fixture_context_resolves_single_caller_parenthesized_function_pointer_parameter()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_single_parenthesized_function_pointer_param")?;
    let caller = function_id_by_name(
        &db,
        "call_single_parenthesized_function_pointer_param_with_local_target",
    )?;
    let target = function_id_by_name(&db, "local_target")?;
    let helper = function_id_by_name(&db, "call_single_parenthesized_function_pointer_param")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1521-1527:
    // `call_single_parenthesized_function_pointer_param(f: fn() -> i32)
    // { (f)() }` has the same private, single-caller proof as the bare `f()`
    // case, but the callsite is dynamic because the callee is parenthesized.
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "parenthesized parameter owner context rows: {context:#?}"
    );
    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["f"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, None);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let callers = db.callers_for_target(target)?;
    let caller_row = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Dynamic, &["f"]);
    assert_eq!(caller_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller_row.target.target_id, target);
    assert_eq!(
        caller_row.target.relation,
        CallRelationKind::DynamicFunction
    );

    let paths = db.call_paths_from_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == owner && path.end_id == target && path.depth == 1)
        .expect("parenthesized parameter owner should have a one-hop path to local_target");
    assert_eq!(path.edges[0].caller_id, owner);
    assert_eq!(path.edges[0].callee_id, target);
    assert_eq!(path.edges[0].relation, CallRelationKind::DynamicFunction);

    let caller_context = db.call_context_for_owner(caller)?;
    let helper_call = row_by_path(
        &caller_context,
        &["call_single_parenthesized_function_pointer_param"],
    );
    assert_eq!(helper_call.site.arg_count, Some(1));
    assert_resolved_target(
        helper_call,
        helper,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    Ok(())
}

#[test]
fn fixture_context_resolves_single_caller_indexed_field_function_parameters() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;

    struct Case {
        owner: &'static str,
        caller: &'static str,
        path: &'static [&'static str],
    }

    let cases = [
        Case {
            owner: "call_single_indexed_field_function_param",
            caller: "call_single_indexed_field_function_param_with_local_target",
            path: &["holder", "callbacks", "0"],
        },
        Case {
            owner: "call_single_indexed_tuple_field_function_param",
            caller: "call_single_indexed_tuple_field_function_param_with_local_target",
            path: &["holder", "0", "0"],
        },
    ];

    for case in cases {
        let owner = function_id_by_name(&db, case.owner)?;
        let caller = function_id_by_name(&db, case.caller)?;
        let helper = function_id_by_name(&db, case.owner)?;

        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1532-1547:
        // These private helpers call through `holder.callbacks[0]()` and
        // `holder.0[0]()`. Each helper has one local caller that constructs the
        // holder with `local_target`, so the indexed field parameter call is an
        // exact DynamicFunction edge instead of an opaque dynamic blocker.
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "{} owner context rows: {context:#?}",
            case.owner
        );
        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, case.path);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );

        let callers = db.callers_for_target(target)?;
        let caller_row =
            caller_by_owner_kind_path(&callers, owner, CallSiteKind::Dynamic, case.path);
        assert_eq!(caller_row.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller_row.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(caller_row.target.target_id, target);
        assert_eq!(
            caller_row.target.relation,
            CallRelationKind::DynamicFunction
        );

        let paths = db.call_paths_from_owner(
            owner,
            CallPathOptions {
                max_depth: 1,
                max_paths: 8,
            },
        )?;
        let path = paths
            .iter()
            .find(|path| path.start_id == owner && path.end_id == target && path.depth == 1)
            .unwrap_or_else(|| panic!("{} should have a one-hop path to local_target", case.owner));
        assert_eq!(path.edges[0].caller_id, owner);
        assert_eq!(path.edges[0].callee_id, target);
        assert_eq!(path.edges[0].relation, CallRelationKind::DynamicFunction);

        let caller_context = db.call_context_for_owner(caller)?;
        let helper_call = row_by_path(&caller_context, &[case.owner]);
        assert_eq!(helper_call.site.arg_count, Some(1));
        assert_resolved_target(
            helper_call,
            helper,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_keeps_generic_fn_once_parameter_targetless() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_single_generic_fn_once_param")?;
    let caller = function_id_by_name(&db, "call_single_generic_fn_once_param_with_local_target")?;
    let helper = function_id_by_name(&db, "call_single_generic_fn_once_param")?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1519:
    // the private single-caller proof is intentionally limited to bare
    // `fn(...)` parameter types. `F: FnOnce` is callable-trait dispatch, so the
    // `generic_f()` call stays targetless until binding/type proof can model it.
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "generic FnOnce parameter context rows: {context:#?}"
    );
    let row = row_by_path(&context, &["generic_f"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "generic FnOnce parameter call must not fabricate targets: {row:#?}"
    );

    let paths = db.call_paths_from_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    assert!(
        paths.iter().all(|path| path.end_id != target),
        "generic FnOnce parameter owner should not traverse through callable-trait dispatch: {paths:#?}"
    );

    // The caller still has a normal direct call edge to the private helper; the
    // unsupported frontier is inside the helper body.
    let caller_context = db.call_context_for_owner(caller)?;
    let helper_call = row_by_path(&caller_context, &["call_single_generic_fn_once_param"]);
    assert_eq!(helper_call.site.arg_count, Some(1));
    assert_resolved_target(
        helper_call,
        helper,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    Ok(())
}
