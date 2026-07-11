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
    let deep_target = function_id_by_name_in_module(
        &db,
        &["crate", "deep_path_root", "branch", "leaf"],
        "deep_target",
    )?;
    let file_module_target =
        function_id_by_name_in_module(&db, &["crate", "file_mod"], "file_module_target")?;
    let cases: [(&[&str], &str, Vec<String>, Uuid); 15] = [
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
        (
            &["crate", "deep_path_root"],
            "call_self_deep_path_target",
            path(&["self", "branch", "leaf", "deep_target"]),
            deep_target,
        ),
        (
            &["crate"],
            "call_crate_deep_path_target",
            path(&["crate", "deep_path_root", "branch", "leaf", "deep_target"]),
            deep_target,
        ),
        (
            &["crate"],
            "call_self_deep_path_target",
            path(&["self", "deep_path_root", "branch", "leaf", "deep_target"]),
            deep_target,
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
fn fixture_context_resolves_same_target_multi_caller_function_pointer_parameter()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_multi_function_pointer_param")?;
    let target = function_id_by_name(&db, "local_target")?;
    let helper = function_id_by_name(&db, "call_multi_function_pointer_param")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // `call_multi_function_pointer_param(f) { f() }` has two local callers in
    // this fixture. Both pass `local_target`, so complete private caller proof
    // still resolves to one exact target.
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "multi-caller parameter owner context rows: {context:#?}"
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
        .expect("multi-caller parameter owner should have a one-hop path to local_target");
    assert_eq!(path.edges[0].caller_id, owner);
    assert_eq!(path.edges[0].callee_id, target);
    assert_eq!(path.edges[0].relation, CallRelationKind::Function);

    for caller_name in [
        "call_multi_function_pointer_param_with_local_target_a",
        "call_multi_function_pointer_param_with_local_target_b",
    ] {
        let caller = function_id_by_name(&db, caller_name)?;
        let caller_context = db.call_context_for_owner(caller)?;
        let helper_call = row_by_path(&caller_context, &["call_multi_function_pointer_param"]);
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
fn fixture_context_resolves_one_hop_forwarded_function_pointer_parameter() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let leaf = function_id_by_name(&db, "call_forwarded_function_pointer_leaf")?;
    let wrapper = function_id_by_name(&db, "call_forwarded_function_pointer_wrapper")?;
    let caller = function_id_by_name(
        &db,
        "call_forwarded_function_pointer_param_with_local_target",
    )?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1942-1951:
    // `call_forwarded_function_pointer_leaf(f) { f() }` is called only by a
    // private wrapper that forwards its own `f` parameter. The wrapper's
    // complete local caller set passes `local_target`, so this one-hop value
    // forwarding proof is exact.
    let context = db.call_context_for_owner(leaf)?;
    assert_eq!(
        context.len(),
        1,
        "forwarded leaf context rows: {context:#?}"
    );
    let row = row_by_path(&context, &["f"]);
    assert_eq!(row.site.owner_id, leaf);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let wrapper_context = db.call_context_for_owner(wrapper)?;
    let leaf_call = row_by_path(&wrapper_context, &["call_forwarded_function_pointer_leaf"]);
    assert_eq!(leaf_call.site.arg_count, Some(1));
    assert_resolved_target(
        leaf_call,
        leaf,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let caller_context = db.call_context_for_owner(caller)?;
    let wrapper_call = row_by_path(
        &caller_context,
        &["call_forwarded_function_pointer_wrapper"],
    );
    assert_eq!(wrapper_call.site.arg_count, Some(1));
    assert_resolved_target(
        wrapper_call,
        wrapper,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let paths = db.call_paths_between(
        caller,
        target,
        CallPathOptions {
            max_depth: 3,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == caller && path.end_id == target && path.depth == 3)
        .unwrap_or_else(|| {
            panic!(
                "forwarded caller should traverse caller -> wrapper -> leaf -> target: {paths:#?}"
            )
        });
    assert_eq!(path.edges[0].caller_id, caller);
    assert_eq!(path.edges[0].callee_id, wrapper);
    assert_eq!(path.edges[1].caller_id, wrapper);
    assert_eq!(path.edges[1].callee_id, leaf);
    assert_eq!(path.edges[2].caller_id, leaf);
    assert_eq!(path.edges[2].callee_id, target);
    assert!(
        path.edges
            .iter()
            .all(|edge| edge.relation == CallRelationKind::Function),
        "forwarded path should be ordinary function traversal edges: {path:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_resolves_two_hop_forwarded_function_pointer_parameter() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let leaf = function_id_by_name(&db, "call_two_hop_forwarded_function_pointer_leaf")?;
    let middle = function_id_by_name(&db, "call_two_hop_forwarded_function_pointer_middle")?;
    let wrapper = function_id_by_name(&db, "call_two_hop_forwarded_function_pointer_wrapper")?;
    let caller = function_id_by_name(
        &db,
        "call_two_hop_forwarded_function_pointer_param_with_local_target",
    )?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2004-2018:
    // `leaf(f) { f() }` receives `f` through two private forwarding helpers.
    // The complete local caller set still supplies `local_target`, so the
    // bounded two-hop value-forwarding proof can emit a real traversal path.
    let context = db.call_context_for_owner(leaf)?;
    assert_eq!(
        context.len(),
        1,
        "two-hop forwarded leaf context rows: {context:#?}"
    );
    let row = row_by_path(&context, &["f"]);
    assert_eq!(row.site.owner_id, leaf);
    assert_eq!(row.site.arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let middle_context = db.call_context_for_owner(middle)?;
    let leaf_call = row_by_path(
        &middle_context,
        &["call_two_hop_forwarded_function_pointer_leaf"],
    );
    assert_resolved_target(
        leaf_call,
        leaf,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let wrapper_context = db.call_context_for_owner(wrapper)?;
    let middle_call = row_by_path(
        &wrapper_context,
        &["call_two_hop_forwarded_function_pointer_middle"],
    );
    assert_resolved_target(
        middle_call,
        middle,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let caller_context = db.call_context_for_owner(caller)?;
    let wrapper_call = row_by_path(
        &caller_context,
        &["call_two_hop_forwarded_function_pointer_wrapper"],
    );
    assert_resolved_target(
        wrapper_call,
        wrapper,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let paths = db.call_paths_between(
        caller,
        target,
        CallPathOptions {
            max_depth: 4,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == caller && path.end_id == target && path.depth == 4)
        .unwrap_or_else(|| {
            panic!(
                "two-hop forwarded caller should traverse caller -> wrapper -> middle -> leaf -> target: {paths:#?}"
            )
        });
    assert_eq!(path.edges[0].caller_id, caller);
    assert_eq!(path.edges[0].callee_id, wrapper);
    assert_eq!(path.edges[1].caller_id, wrapper);
    assert_eq!(path.edges[1].callee_id, middle);
    assert_eq!(path.edges[2].caller_id, middle);
    assert_eq!(path.edges[2].callee_id, leaf);
    assert_eq!(path.edges[3].caller_id, leaf);
    assert_eq!(path.edges[3].callee_id, target);
    assert!(
        path.edges
            .iter()
            .all(|edge| edge.relation == CallRelationKind::Function),
        "two-hop forwarded path should be ordinary function traversal edges: {path:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_resolves_forwarded_callable_trait_object_parameters() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;

    struct Case<'a> {
        label: &'a str,
        leaf: &'a str,
        intermediates: &'a [&'a str],
        caller: &'a str,
        expected_depth: u32,
    }

    // tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    // each private leaf receives `f` through a bounded forwarding chain. The
    // complete local caller set ends in either `&local_target` or
    // `Box::new(local_target)`, so the callable trait-object parameter proof can
    // reuse the same forwarding contract as bare function-pointer parameters.
    for case in [
        Case {
            label: "one-hop forwarded referenced dyn Fn",
            leaf: "call_forwarded_referenced_dyn_fn_leaf",
            intermediates: &["call_forwarded_referenced_dyn_fn_wrapper"],
            caller: "call_forwarded_referenced_dyn_fn_with_local_target",
            expected_depth: 3,
        },
        Case {
            label: "two-hop forwarded referenced dyn Fn",
            leaf: "call_two_hop_forwarded_referenced_dyn_fn_leaf",
            intermediates: &[
                "call_two_hop_forwarded_referenced_dyn_fn_middle",
                "call_two_hop_forwarded_referenced_dyn_fn_wrapper",
            ],
            caller: "call_two_hop_forwarded_referenced_dyn_fn_with_local_target",
            expected_depth: 4,
        },
        Case {
            label: "one-hop forwarded boxed dyn Fn",
            leaf: "call_forwarded_boxed_dyn_fn_leaf",
            intermediates: &["call_forwarded_boxed_dyn_fn_wrapper"],
            caller: "call_forwarded_boxed_dyn_fn_with_local_target",
            expected_depth: 3,
        },
        Case {
            label: "two-hop forwarded boxed dyn Fn",
            leaf: "call_two_hop_forwarded_boxed_dyn_fn_leaf",
            intermediates: &[
                "call_two_hop_forwarded_boxed_dyn_fn_middle",
                "call_two_hop_forwarded_boxed_dyn_fn_wrapper",
            ],
            caller: "call_two_hop_forwarded_boxed_dyn_fn_with_local_target",
            expected_depth: 4,
        },
    ] {
        let leaf = function_id_by_name(&db, case.leaf)?;
        let caller = function_id_by_name(&db, case.caller)?;
        let mut chain = vec![caller];
        for name in case.intermediates.iter().rev() {
            chain.push(function_id_by_name(&db, name)?);
        }
        chain.push(leaf);
        chain.push(target);

        let context = db.call_context_for_owner(leaf)?;
        assert_eq!(
            context.len(),
            1,
            "{} leaf context rows: {context:#?}",
            case.label
        );
        let row = row_by_path(&context, &["f"]);
        assert_eq!(row.site.owner_id, leaf);
        assert_eq!(row.site.arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );

        let paths = db.call_paths_between(
            caller,
            target,
            CallPathOptions {
                max_depth: case.expected_depth,
                max_paths: 8,
            },
        )?;
        let path = paths
            .iter()
            .find(|path| {
                path.start_id == caller
                    && path.end_id == target
                    && path.depth == case.expected_depth
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} should traverse caller through forwarded callable trait-object helpers: {paths:#?}",
                    case.label
                )
            });
        assert_eq!(
            path.edges.len(),
            chain.len() - 1,
            "{} path edge count: {path:#?}",
            case.label
        );
        for (edge, pair) in path.edges.iter().zip(chain.windows(2)) {
            assert_eq!(edge.caller_id, pair[0]);
            assert_eq!(edge.callee_id, pair[1]);
            assert_eq!(edge.relation, CallRelationKind::Function);
        }
    }

    Ok(())
}

#[test]
fn fixture_context_preserves_forwarded_conflicting_boxed_dyn_fn_candidates() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;
    let leaf = function_id_by_name(&db, "call_forwarded_conflicting_boxed_dyn_fn_leaf")?;
    let wrapper = function_id_by_name(&db, "call_forwarded_conflicting_boxed_dyn_fn_wrapper")?;
    let local_caller = function_id_by_name(
        &db,
        "call_forwarded_conflicting_boxed_dyn_fn_with_local_target",
    )?;
    let other_caller = function_id_by_name(
        &db,
        "call_forwarded_conflicting_boxed_dyn_fn_with_other_target",
    )?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    // the private wrapper forwards `f: Box<dyn Fn()>`, but two complete callers
    // construct conflicting boxed function items. The leaf should retain
    // candidate provenance without fabricating a resolved edge.
    let context = db.call_context_for_owner(leaf)?;
    assert_eq!(
        context.len(),
        1,
        "forwarded conflicting boxed dyn Fn leaf context rows: {context:#?}"
    );
    assert_path_function_candidates(
        &context[0],
        leaf,
        &["f"],
        &expected,
        "call_forwarded_conflicting_boxed_dyn_fn_leaf",
    );

    let wrapper_context = db.call_context_for_owner(wrapper)?;
    let leaf_call = row_by_path(
        &wrapper_context,
        &["call_forwarded_conflicting_boxed_dyn_fn_leaf"],
    );
    assert_resolved_target(
        leaf_call,
        leaf,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    for caller in [local_caller, other_caller] {
        let caller_context = db.call_context_for_owner(caller)?;
        let wrapper_call = row_by_path(
            &caller_context,
            &["call_forwarded_conflicting_boxed_dyn_fn_wrapper"],
        );
        assert_resolved_target(
            wrapper_call,
            wrapper,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_resolves_one_hop_forwarded_named_field_parameter() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let leaf = function_id_by_name(&db, "call_forwarded_named_field_leaf")?;
    let wrapper = function_id_by_name(&db, "call_forwarded_named_field_wrapper")?;
    let caller = function_id_by_name(&db, "call_forwarded_named_field_param_with_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    // `call_forwarded_named_field_leaf(holder) { (holder.callback)() }` is
    // called only by a private wrapper that forwards its own `holder`
    // parameter. The wrapper's complete caller set constructs
    // `CallbackHolder { callback: local_target }`, so the holder-field proof is
    // exact after one forwarding hop.
    let context = db.call_context_for_owner(leaf)?;
    assert_eq!(
        context.len(),
        1,
        "forwarded field leaf context rows: {context:#?}"
    );
    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["holder", "callback"]);
    assert_eq!(row.site.owner_id, leaf);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, None);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let wrapper_context = db.call_context_for_owner(wrapper)?;
    let leaf_call = row_by_path(&wrapper_context, &["call_forwarded_named_field_leaf"]);
    assert_eq!(leaf_call.site.arg_count, Some(1));
    assert_resolved_target(
        leaf_call,
        leaf,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let caller_context = db.call_context_for_owner(caller)?;
    let wrapper_call = row_by_path(&caller_context, &["call_forwarded_named_field_wrapper"]);
    assert_eq!(wrapper_call.site.arg_count, Some(1));
    assert_resolved_target(
        wrapper_call,
        wrapper,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let paths = db.call_paths_between(
        caller,
        target,
        CallPathOptions {
            max_depth: 3,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == caller && path.end_id == target && path.depth == 3)
        .unwrap_or_else(|| {
            panic!(
                "forwarded field caller should traverse caller -> wrapper -> leaf -> target: {paths:#?}"
            )
        });
    assert_eq!(path.edges[0].caller_id, caller);
    assert_eq!(path.edges[0].callee_id, wrapper);
    assert_eq!(path.edges[0].relation, CallRelationKind::Function);
    assert_eq!(path.edges[1].caller_id, wrapper);
    assert_eq!(path.edges[1].callee_id, leaf);
    assert_eq!(path.edges[1].relation, CallRelationKind::Function);
    assert_eq!(path.edges[2].caller_id, leaf);
    assert_eq!(path.edges[2].callee_id, target);
    assert_eq!(path.edges[2].relation, CallRelationKind::DynamicFunction);

    Ok(())
}

#[test]
fn fixture_context_resolves_two_hop_forwarded_named_field_parameter() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let leaf = function_id_by_name(&db, "call_two_hop_forwarded_named_field_leaf")?;
    let middle = function_id_by_name(&db, "call_two_hop_forwarded_named_field_middle")?;
    let wrapper = function_id_by_name(&db, "call_two_hop_forwarded_named_field_wrapper")?;
    let caller = function_id_by_name(
        &db,
        "call_two_hop_forwarded_named_field_param_with_local_target",
    )?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    // `call_two_hop_forwarded_named_field_leaf(holder) { (holder.callback)() }`
    // receives `holder` through two private forwarding helpers. The complete
    // caller set still constructs `CallbackHolder { callback: local_target }`,
    // so the bounded field-forwarding proof can emit one dynamic edge at the
    // leaf and ordinary function edges through the helper chain.
    let context = db.call_context_for_owner(leaf)?;
    assert_eq!(
        context.len(),
        1,
        "two-hop forwarded field leaf context rows: {context:#?}"
    );
    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["holder", "callback"]);
    assert_eq!(row.site.owner_id, leaf);
    assert_eq!(row.site.arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let middle_context = db.call_context_for_owner(middle)?;
    let leaf_call = row_by_path(
        &middle_context,
        &["call_two_hop_forwarded_named_field_leaf"],
    );
    assert_resolved_target(
        leaf_call,
        leaf,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let wrapper_context = db.call_context_for_owner(wrapper)?;
    let middle_call = row_by_path(
        &wrapper_context,
        &["call_two_hop_forwarded_named_field_middle"],
    );
    assert_resolved_target(
        middle_call,
        middle,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let caller_context = db.call_context_for_owner(caller)?;
    let wrapper_call = row_by_path(
        &caller_context,
        &["call_two_hop_forwarded_named_field_wrapper"],
    );
    assert_resolved_target(
        wrapper_call,
        wrapper,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let paths = db.call_paths_between(
        caller,
        target,
        CallPathOptions {
            max_depth: 4,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == caller && path.end_id == target && path.depth == 4)
        .unwrap_or_else(|| {
            panic!(
                "two-hop forwarded field caller should traverse caller -> wrapper -> middle -> leaf -> target: {paths:#?}"
            )
        });
    assert_eq!(path.edges[0].caller_id, caller);
    assert_eq!(path.edges[0].callee_id, wrapper);
    assert_eq!(path.edges[0].relation, CallRelationKind::Function);
    assert_eq!(path.edges[1].caller_id, wrapper);
    assert_eq!(path.edges[1].callee_id, middle);
    assert_eq!(path.edges[1].relation, CallRelationKind::Function);
    assert_eq!(path.edges[2].caller_id, middle);
    assert_eq!(path.edges[2].callee_id, leaf);
    assert_eq!(path.edges[2].relation, CallRelationKind::Function);
    assert_eq!(path.edges[3].caller_id, leaf);
    assert_eq!(path.edges[3].callee_id, target);
    assert_eq!(path.edges[3].relation, CallRelationKind::DynamicFunction);

    Ok(())
}

#[test]
fn fixture_context_preserves_forwarded_conflicting_function_pointer_candidates()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;
    let leaf = function_id_by_name(&db, "call_forwarded_conflicting_function_pointer_leaf")?;
    let wrapper = function_id_by_name(&db, "call_forwarded_conflicting_function_pointer_wrapper")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1955-1968:
    // the leaf receives `f` from a private wrapper, but the wrapper's complete
    // caller set passes both `local_target` and `other_target`. The leaf keeps
    // both candidate functions and remains ambiguous.
    let context = db.call_context_for_owner(leaf)?;
    assert_eq!(
        context.len(),
        1,
        "forwarded conflicting leaf context rows: {context:#?}"
    );
    assert_path_function_candidates(
        &context[0],
        leaf,
        &["f"],
        &expected,
        "forwarded conflicting function-pointer leaf",
    );

    let wrapper_context = db.call_context_for_owner(wrapper)?;
    let leaf_call = row_by_path(
        &wrapper_context,
        &["call_forwarded_conflicting_function_pointer_leaf"],
    );
    assert_eq!(leaf_call.site.arg_count, Some(1));
    assert_resolved_target(
        leaf_call,
        leaf,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    Ok(())
}

#[test]
fn fixture_context_preserves_two_hop_forwarded_conflicting_function_pointer_candidates()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;
    let leaf = function_id_by_name(
        &db,
        "call_two_hop_forwarded_conflicting_function_pointer_leaf",
    )?;
    let middle = function_id_by_name(
        &db,
        "call_two_hop_forwarded_conflicting_function_pointer_middle",
    )?;
    let wrapper = function_id_by_name(
        &db,
        "call_two_hop_forwarded_conflicting_function_pointer_wrapper",
    )?;
    let caller = function_id_by_name(
        &db,
        "call_two_hop_forwarded_conflicting_function_pointer_param_with_local_target",
    )?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2020-2038:
    // the leaf receives `f` through two private forwarding helpers, but the
    // wrapper's complete local caller set supplies both `local_target` and
    // `other_target`. The leaf keeps candidate proof and does not become a
    // resolved traversal edge.
    let context = db.call_context_for_owner(leaf)?;
    assert_eq!(
        context.len(),
        1,
        "two-hop forwarded conflicting leaf context rows: {context:#?}"
    );
    assert_path_function_candidates(
        &context[0],
        leaf,
        &["f"],
        &expected,
        "two-hop forwarded conflicting function-pointer leaf",
    );

    let middle_context = db.call_context_for_owner(middle)?;
    let leaf_call = row_by_path(
        &middle_context,
        &["call_two_hop_forwarded_conflicting_function_pointer_leaf"],
    );
    assert_resolved_target(
        leaf_call,
        leaf,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let wrapper_context = db.call_context_for_owner(wrapper)?;
    let middle_call = row_by_path(
        &wrapper_context,
        &["call_two_hop_forwarded_conflicting_function_pointer_middle"],
    );
    assert_resolved_target(
        middle_call,
        middle,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let paths = db.call_paths_between(
        caller,
        target,
        CallPathOptions {
            max_depth: 4,
            max_paths: 8,
        },
    )?;
    assert!(
        paths.is_empty(),
        "ambiguous two-hop forwarded candidates must not become a resolved caller -> target path: {paths:#?}"
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
fn fixture_context_resolves_single_caller_aliased_function_pointer_parameters()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;

    struct Case {
        owner: &'static str,
        caller: &'static str,
        source: &'static str,
        site_kind: CallSiteKind,
        relation: CallRelationKind,
        target_kind: CallTargetKind,
    }

    let cases = [
        Case {
            owner: "call_single_aliased_function_pointer_param",
            caller: "call_single_aliased_function_pointer_param_with_local_target",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1673-1675 `let g = f; g()`",
            site_kind: CallSiteKind::Path,
            relation: CallRelationKind::Function,
            target_kind: CallTargetKind::Function,
        },
        Case {
            owner: "call_single_parenthesized_aliased_function_pointer_param",
            caller: "call_single_parenthesized_aliased_function_pointer_param_with_local_target",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1682-1684 `let g = f; (g)()`",
            site_kind: CallSiteKind::Dynamic,
            relation: CallRelationKind::DynamicFunction,
            target_kind: CallTargetKind::Function,
        },
    ];

    for case in cases {
        let owner = function_id_by_name(&db, case.owner)?;
        let caller = function_id_by_name(&db, case.caller)?;
        let helper = function_id_by_name(&db, case.owner)?;

        // These private helpers alias the callable parameter with `let g = f`
        // before calling through `g`. The persisted call row keeps the
        // observed callee path as `g`, while resolver proof follows the alias
        // back to the single caller's `local_target` argument.
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "{} owner context rows from {}: {context:#?}",
            case.owner,
            case.source
        );
        let row = row_by_kind_path(&context, case.site_kind, &["g"]);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(
            row.site.generic_arg_count,
            match case.site_kind {
                CallSiteKind::Path => Some(0),
                CallSiteKind::Dynamic => None,
                _ => unreachable!("aliased callable case should be path or dynamic"),
            }
        );
        assert_resolved_target(row, target, case.relation, case.site_kind, case.target_kind);

        let callers = db.callers_for_target(target)?;
        let caller_row = caller_by_owner_kind_path(&callers, owner, case.site_kind, &["g"]);
        assert_eq!(caller_row.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller_row.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(caller_row.target.target_id, target);
        assert_eq!(caller_row.target.relation, case.relation);

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
        assert_eq!(path.edges[0].relation, case.relation);

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
fn fixture_context_preserves_forwarded_conflicting_named_field_candidates() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;
    let leaf = function_id_by_name(&db, "call_forwarded_conflicting_named_field_leaf")?;
    let wrapper = function_id_by_name(&db, "call_forwarded_conflicting_named_field_wrapper")?;
    let local_caller = function_id_by_name(
        &db,
        "call_forwarded_conflicting_named_field_param_with_local_target",
    )?;
    let other_caller = function_id_by_name(
        &db,
        "call_forwarded_conflicting_named_field_param_with_other_target",
    )?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    // the private wrapper forwards `holder`, but two complete callers construct
    // conflicting callback functions. The leaf should retain candidate
    // provenance without fabricating a resolved dynamic edge.
    let context = db.call_context_for_owner(leaf)?;
    assert_eq!(
        context.len(),
        1,
        "forwarded conflicting field leaf context rows: {context:#?}"
    );
    assert_dynamic_path_function_candidates(
        &context[0],
        leaf,
        &["holder", "callback"],
        &expected,
        "call_forwarded_conflicting_named_field_leaf",
    );

    let wrapper_context = db.call_context_for_owner(wrapper)?;
    let leaf_call = row_by_path(
        &wrapper_context,
        &["call_forwarded_conflicting_named_field_leaf"],
    );
    assert_resolved_target(
        leaf_call,
        leaf,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    for caller in [local_caller, other_caller] {
        let caller_context = db.call_context_for_owner(caller)?;
        let wrapper_call = row_by_path(
            &caller_context,
            &["call_forwarded_conflicting_named_field_wrapper"],
        );
        assert_resolved_target(
            wrapper_call,
            wrapper,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_preserves_two_hop_forwarded_conflicting_named_field_candidates()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;
    let leaf = function_id_by_name(&db, "call_two_hop_forwarded_conflicting_named_field_leaf")?;
    let middle = function_id_by_name(&db, "call_two_hop_forwarded_conflicting_named_field_middle")?;
    let wrapper = function_id_by_name(
        &db,
        "call_two_hop_forwarded_conflicting_named_field_wrapper",
    )?;
    let local_caller = function_id_by_name(
        &db,
        "call_two_hop_forwarded_conflicting_named_field_param_with_local_target",
    )?;
    let other_caller = function_id_by_name(
        &db,
        "call_two_hop_forwarded_conflicting_named_field_param_with_other_target",
    )?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    // the leaf receives `holder` through two private forwarding helpers, but
    // the wrapper's complete caller set constructs both `local_target` and
    // `other_target`. The leaf should keep both dynamic candidates and emit no
    // resolved dynamic edge.
    let context = db.call_context_for_owner(leaf)?;
    assert_eq!(
        context.len(),
        1,
        "two-hop forwarded conflicting field leaf context rows: {context:#?}"
    );
    assert_dynamic_path_function_candidates(
        &context[0],
        leaf,
        &["holder", "callback"],
        &expected,
        "call_two_hop_forwarded_conflicting_named_field_leaf",
    );

    let middle_context = db.call_context_for_owner(middle)?;
    let leaf_call = row_by_path(
        &middle_context,
        &["call_two_hop_forwarded_conflicting_named_field_leaf"],
    );
    assert_resolved_target(
        leaf_call,
        leaf,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let wrapper_context = db.call_context_for_owner(wrapper)?;
    let middle_call = row_by_path(
        &wrapper_context,
        &["call_two_hop_forwarded_conflicting_named_field_middle"],
    );
    assert_resolved_target(
        middle_call,
        middle,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    for caller in [local_caller, other_caller] {
        let caller_context = db.call_context_for_owner(caller)?;
        let wrapper_call = row_by_path(
            &caller_context,
            &["call_two_hop_forwarded_conflicting_named_field_wrapper"],
        );
        assert_resolved_target(
            wrapper_call,
            wrapper,
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
        callers: &'static [&'static str],
        path: &'static [&'static str],
    }

    let cases = [
        Case {
            owner: "call_single_named_field_function_param",
            callers: &["call_single_named_field_function_param_with_local_target"],
            path: &["holder", "callback"],
        },
        Case {
            owner: "call_multi_named_field_function_param",
            callers: &[
                "call_multi_named_field_function_param_with_local_target_a",
                "call_multi_named_field_function_param_with_local_target_b",
            ],
            path: &["holder", "callback"],
        },
        Case {
            owner: "call_single_indexed_function_pointer_param",
            callers: &["call_single_indexed_function_pointer_param_with_local_target"],
            path: &["funcs", "0"],
        },
        Case {
            owner: "call_single_indexed_field_function_param",
            callers: &["call_single_indexed_field_function_param_with_local_target"],
            path: &["holder", "callbacks", "0"],
        },
        Case {
            owner: "call_single_indexed_tuple_field_function_param",
            callers: &["call_single_indexed_tuple_field_function_param_with_local_target"],
            path: &["holder", "0", "0"],
        },
    ];

    for case in cases {
        let owner = function_id_by_name(&db, case.owner)?;
        let helper = function_id_by_name(&db, case.owner)?;

        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1555,
        // 1598-1606, 1847-1877, and the final direct array-parameter helper:
        // These private helpers call through `(holder.callback)()`,
        // `holder.callbacks[0]()`, `holder.0[0]()`, and `funcs[0]()`. Each
        // helper has a complete local caller set that supplies `local_target` in the
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

        for caller_name in case.callers {
            let caller = function_id_by_name(&db, caller_name)?;
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
    }

    Ok(())
}

#[test]
fn fixture_context_resolves_same_target_multi_caller_generic_fn_once_parameter()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_multi_generic_fn_once_param")?;
    let target = function_id_by_name(&db, "local_target")?;
    let helper = function_id_by_name(&db, "call_multi_generic_fn_once_param")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // `call_multi_generic_fn_once_param(generic_f) { generic_f() }` has two
    // local callers. Both pass `local_target`, so the complete private caller
    // proof still resolves to one exact target without broad FnOnce dispatch.
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "multi-caller generic FnOnce context rows: {context:#?}"
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
        .expect("multi-caller generic FnOnce owner should have a one-hop path to local_target");
    assert_eq!(path.edges[0].caller_id, owner);
    assert_eq!(path.edges[0].callee_id, target);
    assert_eq!(path.edges[0].relation, CallRelationKind::Function);

    for caller_name in [
        "call_multi_generic_fn_once_param_with_local_target_a",
        "call_multi_generic_fn_once_param_with_local_target_b",
    ] {
        let caller = function_id_by_name(&db, caller_name)?;
        let caller_context = db.call_context_for_owner(caller)?;
        let helper_call = row_by_path(&caller_context, &["call_multi_generic_fn_once_param"]);
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
    let target = function_id_by_name(&db, "local_target")?;

    struct Case {
        owner: &'static str,
        caller: &'static str,
        source: &'static str,
        site_kind: CallSiteKind,
        relation: CallRelationKind,
        target_kind: CallTargetKind,
    }

    let cases = [
        Case {
            owner: "call_single_generic_fn_once_param",
            caller: "call_single_generic_fn_once_param_with_local_target",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1519-1524 `generic_f()`",
            site_kind: CallSiteKind::Path,
            relation: CallRelationKind::Function,
            target_kind: CallTargetKind::Function,
        },
        Case {
            owner: "call_single_parenthesized_generic_fn_once_param",
            caller: "call_single_parenthesized_generic_fn_once_param_with_local_target",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1729-1734 `(generic_f)()`",
            site_kind: CallSiteKind::Dynamic,
            relation: CallRelationKind::DynamicFunction,
            target_kind: CallTargetKind::Function,
        },
    ];

    for case in cases {
        let owner = function_id_by_name(&db, case.owner)?;
        let caller = function_id_by_name(&db, case.caller)?;
        let helper = function_id_by_name(&db, case.owner)?;

        // These private generic helpers have one local caller in this fixture.
        // Each caller passes `local_target`, so the parameter call is admitted
        // as exact value-flow proof, while public or multi-target callable-trait
        // dispatch stays targetless.
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "{} generic FnOnce parameter context rows from {}: {context:#?}",
            case.owner,
            case.source
        );
        let row = row_by_kind_path(&context, case.site_kind, &["generic_f"]);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(
            row.site.generic_arg_count,
            match case.site_kind {
                CallSiteKind::Path => Some(0),
                CallSiteKind::Dynamic => None,
                _ => unreachable!("generic callable case should be path or dynamic"),
            }
        );
        assert_resolved_target(row, target, case.relation, case.site_kind, case.target_kind);

        let callers = db.callers_for_target(target)?;
        let caller_row = caller_by_owner_kind_path(&callers, owner, case.site_kind, &["generic_f"]);
        assert_eq!(caller_row.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller_row.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(caller_row.target.target_id, target);
        assert_eq!(caller_row.target.relation, case.relation);

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
        assert_eq!(path.edges[0].relation, case.relation);

        // The caller still has a normal direct call edge to the private helper;
        // single-caller argument proof is additional resolver evidence, not a
        // replacement for the caller's own edge.
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
fn fixture_context_resolves_single_caller_referenced_callable_trait_object_parameter()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;

    let cases = [
        CallableParamCase {
            owner: "call_single_referenced_dyn_fn_param",
            caller: "call_single_referenced_dyn_fn_param_with_local_target",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:2148-2153 `f()` with `&dyn Fn` parameter",
            callee_path: &["f"],
            site_kind: CallSiteKind::Path,
            relation: CallRelationKind::Function,
            target_kind: CallTargetKind::Function,
            proof_note: "private `&dyn Fn` helper whose only local caller passes `&local_target`",
        },
        CallableParamCase {
            owner: "call_single_parenthesized_referenced_dyn_fn_param",
            caller: "call_single_parenthesized_referenced_dyn_fn_param_with_local_target",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:2156-2161 `(f)()` with `&dyn Fn` parameter",
            callee_path: &["f"],
            site_kind: CallSiteKind::Dynamic,
            relation: CallRelationKind::DynamicFunction,
            target_kind: CallTargetKind::Function,
            proof_note: "private `&dyn Fn` helper whose only local caller passes `&local_target`",
        },
    ];

    for case in cases {
        assert_callable_case(&db, target, case)?;
    }

    Ok(())
}

#[test]
fn fixture_context_resolves_single_caller_boxed_callable_trait_object_parameter()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;

    let cases = [
        CallableParamCase {
            owner: "call_single_boxed_dyn_fn_param",
            caller: "call_single_boxed_dyn_fn_param_with_local_target",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs EOF `f()` with `Box<dyn Fn>` parameter",
            callee_path: &["f"],
            site_kind: CallSiteKind::Path,
            relation: CallRelationKind::Function,
            target_kind: CallTargetKind::Function,
            proof_note: "private `Box<dyn Fn>` helper whose only local caller passes `Box::new(local_target)`",
        },
        CallableParamCase {
            owner: "call_single_parenthesized_boxed_dyn_fn_param",
            caller: "call_single_parenthesized_boxed_dyn_fn_param_with_local_target",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs EOF `(f)()` with `Box<dyn Fn>` parameter",
            callee_path: &["f"],
            site_kind: CallSiteKind::Dynamic,
            relation: CallRelationKind::DynamicFunction,
            target_kind: CallTargetKind::Function,
            proof_note: "private `Box<dyn Fn>` helper whose only local caller passes `Box::new(local_target)`",
        },
    ];

    for case in cases {
        assert_callable_case(&db, target, case)?;
    }

    Ok(())
}

struct CallableParamCase {
    owner: &'static str,
    caller: &'static str,
    source: &'static str,
    callee_path: &'static [&'static str],
    site_kind: CallSiteKind,
    relation: CallRelationKind,
    target_kind: CallTargetKind,
    proof_note: &'static str,
}

fn assert_callable_case(
    db: &Database,
    target: Uuid,
    case: CallableParamCase,
) -> Result<(), DbError> {
    let owner = function_id_by_name(db, case.owner)?;
    let caller = function_id_by_name(db, case.caller)?;
    let helper = function_id_by_name(db, case.owner)?;

    // This is exact complete-private-caller proof. It must not generalize to
    // public callable parameters or unbounded runtime trait-object dispatch.
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "{} callable parameter context rows from {} ({note}): {context:#?}",
        case.owner,
        case.source,
        note = case.proof_note
    );
    let row = row_by_kind_path(&context, case.site_kind, case.callee_path);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(
        row.site.generic_arg_count,
        match case.site_kind {
            CallSiteKind::Path => Some(0),
            CallSiteKind::Dynamic => None,
            _ => unreachable!("callable parameter case should be path or dynamic"),
        }
    );
    assert_resolved_target(row, target, case.relation, case.site_kind, case.target_kind);

    let callers = db.callers_for_target(target)?;
    let caller_row = caller_by_owner_kind_path(&callers, owner, case.site_kind, case.callee_path);
    assert_eq!(caller_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller_row.target.target_id, target);
    assert_eq!(caller_row.target.relation, case.relation);

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
    assert_eq!(path.edges[0].relation, case.relation);

    // The caller remains an ordinary direct call to the private helper; argument
    // proof only affects the helper body's callable-parameter invocation.
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

    Ok(())
}
