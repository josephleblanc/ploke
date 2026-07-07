use super::*;
use ploke_db::CallPathOptions;

#[test]
fn fixture_call_paths_include_direct_recursive_edges_without_expanding_cycles()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "recursive_fixture_call")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // `recursive_fixture_call(depth - 1)` is a resolved direct self-call. Path
    // traversal should report the real edge once, then stop expanding the
    // cycle instead of dropping the edge or looping.
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "recursive owner context: {context:#?}");
    let row = row_by_path(&context, &["recursive_fixture_call"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.arg_count, Some(1));
    assert_resolved_target(
        row,
        owner,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let options = CallPathOptions {
        max_depth: 4,
        max_paths: 16,
    };
    let outgoing = db.call_paths_from_owner(owner, options)?;
    assert_eq!(
        outgoing.len(),
        1,
        "recursive outgoing traversal should record one self-edge and stop: {outgoing:#?}"
    );
    assert_eq!(outgoing[0].start_id, owner);
    assert_eq!(outgoing[0].end_id, owner);
    assert_eq!(outgoing[0].depth, 1);
    assert_eq!(outgoing[0].edges.len(), 1);
    assert_eq!(outgoing[0].edges[0].caller_id, owner);
    assert_eq!(outgoing[0].edges[0].callee_id, owner);
    assert_eq!(outgoing[0].edges[0].call_site_id, row.site.id);

    let incoming = db.call_paths_to_target(owner, options)?;
    assert_eq!(
        incoming, outgoing,
        "target-centered recursion traversal should preserve the same one-edge path"
    );

    let between = db.call_paths_between(owner, owner, options)?;
    assert_eq!(
        between, outgoing,
        "direct reachability from a recursive function to itself should expose the real self-edge"
    );

    let cycles = db.call_cycles_from_owner(owner, options)?;
    assert_eq!(
        cycles, outgoing,
        "cycle query should return only the resolved path that starts and ends at the owner"
    );

    let reach = db.call_reach_for_owner(owner, options)?;
    assert_eq!(
        reach.paths, outgoing,
        "owner reach summaries should include direct recursive edges without expanding the cycle"
    );
    let impact = db.call_impact_for_target(owner, options)?;
    assert_eq!(
        impact.paths, incoming,
        "target impact summaries should include direct recursive callers without expanding the cycle"
    );

    Ok(())
}

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
    let file_module_target =
        function_id_by_name_in_module(&db, &["crate", "file_mod"], "file_module_target")?;
    let cases: [(&[&str], &str, Vec<String>, Uuid); 12] = [
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
        (
            &["crate"],
            "call_crate_file_module_target",
            path(&["crate", "file_mod", "file_module_target"]),
            file_module_target,
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
fn fixture_context_resolves_single_caller_dynamic_function_pointer_parameter_forms()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;

    struct Case {
        owner: &'static str,
        caller: &'static str,
        caller_arg_count: u32,
        source: &'static str,
    }

    let cases = [
        Case {
            owner: "call_single_parenthesized_function_pointer_param",
            caller: "call_single_parenthesized_function_pointer_param_with_local_target",
            caller_arg_count: 1,
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1522-1527 `(f)()`",
        },
        Case {
            owner: "call_single_if_function_pointer_param_branch",
            caller: "call_single_if_function_pointer_param_branch_with_local_target",
            caller_arg_count: 2,
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1654-1659 `(if flag { f } else { f })()`",
        },
        Case {
            owner: "call_single_match_function_pointer_param_arm",
            caller: "call_single_match_function_pointer_param_arm_with_local_target",
            caller_arg_count: 2,
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1662-1670 `match flag { true => f, false => f }`",
        },
        Case {
            owner: "call_single_function_pointer_param_cast",
            caller: "call_single_function_pointer_param_cast_with_local_target",
            caller_arg_count: 1,
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1564-1569 `(f as fn() -> i32)()`",
        },
    ];

    for case in cases {
        let owner = function_id_by_name(&db, case.owner)?;
        let caller = function_id_by_name(&db, case.caller)?;
        let helper = function_id_by_name(&db, case.owner)?;

        // These private helpers have one local caller in this fixture, and
        // that caller passes `local_target`. The callee syntax is dynamic, but
        // the complete private caller proof is still exact.
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "{} owner context rows from {}: {context:#?}",
            case.owner,
            case.source
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
            .unwrap_or_else(|| panic!("{} should have a one-hop path to local_target", case.owner));
        assert_eq!(path.edges[0].caller_id, owner);
        assert_eq!(path.edges[0].callee_id, target);
        assert_eq!(path.edges[0].relation, CallRelationKind::DynamicFunction);

        let caller_context = db.call_context_for_owner(caller)?;
        let helper_call = row_by_path(&caller_context, &[case.owner]);
        assert_eq!(helper_call.site.arg_count, Some(case.caller_arg_count));
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
            owner: "call_single_named_field_function_param",
            caller: "call_single_named_field_function_param_with_local_target",
            path: &["holder", "callback"],
        },
        Case {
            owner: "call_single_indexed_function_pointer_param",
            caller: "call_single_indexed_function_pointer_param_with_local_target",
            path: &["funcs", "0"],
        },
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

        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1555,
        // 1598-1606, and the final direct array-parameter helper:
        // These private helpers call through `(holder.callback)()`,
        // `holder.callbacks[0]()`, `holder.0[0]()`, and `funcs[0]()`. Each
        // helper has one local caller that supplies `local_target` in the
        // relevant holder field or array slot, so the parameter call is an
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
fn fixture_context_resolves_single_caller_generic_fn_once_parameter() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_single_generic_fn_once_param")?;
    let caller = function_id_by_name(&db, "call_single_generic_fn_once_param_with_local_target")?;
    let helper = function_id_by_name(&db, "call_single_generic_fn_once_param")?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1519:
    // `call_single_generic_fn_once_param<F>(generic_f: F) where F: FnOnce()`
    // has one local caller in this fixture. The caller passes `local_target`,
    // so the parameter call is admitted as exact value-flow proof, while
    // broader public or multi-target callable-trait dispatch stays targetless.
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
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let callers = db.callers_for_target(target)?;
    let caller_row = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, &["generic_f"]);
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
        .expect("generic FnOnce parameter owner should have a one-hop path to local_target");
    assert_eq!(path.edges[0].caller_id, owner);
    assert_eq!(path.edges[0].callee_id, target);
    assert_eq!(path.edges[0].relation, CallRelationKind::Function);

    // The caller still has a normal direct call edge to the private helper; the
    // single-caller argument proof is additional resolver evidence, not a
    // replacement for the caller's own edge.
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
