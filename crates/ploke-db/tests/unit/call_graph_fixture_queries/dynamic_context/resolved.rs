use ploke_db::CallPathOptions;

use super::*;

#[test]
fn fixture_context_reads_projected_returned_function_nested_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_returned_function")?;
    let maker = function_id_by_name(&db, "make_fn")?;
    let returned = function_id_by_name(&db, "local_target")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned function context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["make_fn"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic = row_by_kind_path(&context, CallSiteKind::Dynamic, &["make_fn"]);
    assert_eq!(dynamic.site.owner_id, owner);
    assert_eq!(dynamic.site.arg_count, Some(0));
    assert_resolved_target(
        dynamic,
        returned,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_returned_parameter_function_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(
        &db,
        "call_returned_forwarded_function_pointer_param_with_local_target",
    )?;
    let helper = function_id_by_name(&db, "return_forwarded_function_pointer")?;
    let returned = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2103-2109:
    // the helper returns its private `f` parameter directly. Its complete
    // caller set contains this call with `local_target`, so the outer
    // returned callable invocation can become a resolved dynamic edge.
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned parameter function context rows: {context:#?}"
    );

    let helper_row = row_by_path(&context, &["return_forwarded_function_pointer"]);
    assert_eq!(helper_row.site.arg_count, Some(1));
    assert_resolved_target(
        helper_row,
        helper,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic_row = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["return_forwarded_function_pointer"],
    );
    assert_eq!(dynamic_row.site.arg_count, Some(0));
    assert_resolved_target(
        dynamic_row,
        returned,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let paths = db.call_paths_between(
        owner,
        returned,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == owner && path.end_id == returned && path.depth == 1)
        .unwrap_or_else(|| {
            panic!("returned parameter caller should traverse directly to local_target: {paths:#?}")
        });
    assert_eq!(path.edges[0].caller_id, owner);
    assert_eq!(path.edges[0].callee_id, returned);
    assert_eq!(path.edges[0].relation, CallRelationKind::DynamicFunction);

    Ok(())
}

#[test]
fn fixture_context_resolves_returned_boxed_dyn_fn_value_flow() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_returned_boxed_dyn_fn")?;
    let maker = function_id_by_name(&db, "make_boxed_dyn_fn")?;
    let returned = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2438-2444:
    // `make_boxed_dyn_fn` returns the exact `Box::new(local_target)` value,
    // and `call_returned_boxed_dyn_fn` immediately invokes that returned boxed
    // callable. This should admit only the exact returned function edge.
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned boxed dyn Fn context rows: {context:#?}"
    );

    let maker_row = row_by_path(&context, &["make_boxed_dyn_fn"]);
    assert_eq!(maker_row.site.arg_count, Some(0));
    assert_resolved_target(
        maker_row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic_row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["make_boxed_dyn_fn"]);
    assert_eq!(dynamic_row.site.arg_count, Some(0));
    assert_resolved_target(
        dynamic_row,
        returned,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let paths = db.call_paths_between(
        owner,
        returned,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    assert!(
        paths.iter().any(|path| {
            path.start_id == owner
                && path.end_id == returned
                && path.depth == 1
                && path.edges[0].relation == CallRelationKind::DynamicFunction
        }),
        "returned boxed dyn Fn should traverse one exact dynamic edge to local_target: {paths:#?}"
    );

    let flows = db.returned_call_binding_flows_for_owner(owner)?;
    assert_eq!(
        flows.len(),
        1,
        "returned boxed dyn Fn should expose one producer return-binding proof flow: {flows:#?}"
    );
    let flow = &flows[0];
    assert_eq!(flow.caller_id, owner);
    assert_eq!(flow.dynamic.id, dynamic_row.site.id);
    assert_eq!(flow.dynamic.path, path(&["make_boxed_dyn_fn"]));
    assert_eq!(flow.dynamic.target_id, returned);
    assert_eq!(flow.dynamic.relation, CallRelationKind::DynamicFunction);
    assert_eq!(flow.dynamic.target_kind, CallTargetKind::Function);
    assert_eq!(flow.producer.id, maker);
    assert_eq!(flow.producer.site_id, maker_row.site.id);
    assert_eq!(
        flow.binding.source.relation,
        LocalBindingRelationKind::BindingSourceCallResult
    );
    assert_eq!(flow.binding.source.kind, "Path");

    Ok(())
}

#[test]
fn fixture_context_reads_projected_returned_closure_nested_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1431:
        // `make_closure()()` has an inner maker path call and an outer dynamic
        // call to the closure returned directly at lines 1426-1428.
        ReturnedClosureCase {
            owner: "call_returned_closure",
            maker: "make_closure",
            path: &["make_closure"],
        },
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1453:
        // `make_bound_closure()()` resolves through the maker's returned local
        // closure binding from lines 1447-1449.
        ReturnedClosureCase {
            owner: "call_returned_bound_closure",
            maker: "make_bound_closure",
            path: &["make_bound_closure"],
        },
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1500:
        // `make_alias_bound_closure()()` resolves through the maker's returned
        // local alias of a closure binding from lines 1493-1496.
        ReturnedClosureCase {
            owner: "call_returned_alias_bound_closure",
            maker: "make_alias_bound_closure",
            path: &["make_alias_bound_closure"],
        },
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1520:
        // `make_forwarded_returned_closure()()` resolves through a bounded
        // sync returned-callable forwarding proof from the producer to
        // `make_target_closure`'s returned closure owner.
        ReturnedClosureCase {
            owner: "call_forwarded_returned_closure",
            maker: "make_forwarded_returned_closure",
            path: &["make_forwarded_returned_closure"],
        },
    ];

    for case in cases {
        assert_returned_closure_context(&db, case)?;
    }

    Ok(())
}

#[test]
fn fixture_context_resolves_awaited_returned_async_closure_only_when_polled() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let maker = function_id_by_name(&db, "make_returned_async_closure")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2355-2365:
    // the un-awaited caller invokes the returned async closure but never polls
    // the future, while the awaited and stored callers poll it with `.await`.
    let no_await_owner = function_id_by_name(&db, "call_returned_async_closure_without_await")?;
    let mut closure = None;
    for (owner_name, label) in [
        (
            "call_awaited_returned_async_closure",
            "awaited returned async closure",
        ),
        (
            "call_stored_returned_async_closure",
            "stored returned async closure future",
        ),
    ] {
        let target = assert_polled_returned_async_context(&db, owner_name, label, maker)?;
        if let Some(existing) = closure {
            assert_eq!(
                target, existing,
                "{label} should target the same returned async-closure owner"
            );
        } else {
            closure = Some(target);
        }
    }
    let closure = closure.expect("polled returned async closure cases should establish a target");

    let no_await_context = db.call_context_for_owner(no_await_owner)?;
    assert_eq!(
        no_await_context.len(),
        2,
        "un-awaited returned async closure context rows: {no_await_context:#?}"
    );
    assert_returned_async_maker_path(&no_await_context, no_await_owner, maker);

    let no_await_dynamic = row_by_kind_path(
        &no_await_context,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(no_await_dynamic.site.owner_id, no_await_owner);
    assert_eq!(no_await_dynamic.status.status, CallStatusKind::Unsupported);
    assert_eq!(no_await_dynamic.status.resolution, None);
    assert!(
        no_await_dynamic.targets.is_empty(),
        "un-awaited returned async closure must not fabricate a closure edge: {no_await_dynamic:#?}"
    );

    let no_await_paths = db.call_paths_from_owner(
        no_await_owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    assert!(
        no_await_paths
            .iter()
            .all(|path| path.end_id != closure || path.depth != 1),
        "un-awaited returned async closure must not traverse to the returned closure owner: {no_await_paths:#?}"
    );

    Ok(())
}

struct ReturnedClosureCase {
    owner: &'static str,
    maker: &'static str,
    path: &'static [&'static str],
}

fn assert_returned_async_maker_path(context: &[CallContextRow], owner: Uuid, maker: Uuid) {
    let row = row_by_path(context, &["make_returned_async_closure"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
}

fn assert_polled_returned_async_context(
    db: &ploke_db::Database,
    owner_name: &str,
    label: &str,
    maker: Uuid,
) -> Result<Uuid, DbError> {
    let owner = function_id_by_name(db, owner_name)?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "{label} context rows: {context:#?}");
    assert_returned_async_maker_path(&context, owner, maker);

    let dynamic = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(dynamic.site.owner_id, owner);
    assert_eq!(dynamic.site.arg_count, Some(0));
    assert_eq!(dynamic.site.generic_arg_count, None);
    assert_eq!(
        dynamic.targets.len(),
        1,
        "{label} dynamic row: {dynamic:#?}"
    );
    let closure = dynamic.targets[0].target_id;
    assert_resolved_target(
        dynamic,
        closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );

    let callers = db.callers_for_target(closure)?;
    let caller = caller_by_owner_kind_path(
        &callers,
        owner,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(caller.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller.target.relation, CallRelationKind::DynamicClosure);

    let paths = db.call_paths_from_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    assert!(
        paths.iter().any(|path| {
            path.start_id == owner
                && path.end_id == closure
                && path.depth == 1
                && path.edges[0].relation == CallRelationKind::DynamicClosure
        }),
        "{label} should traverse one hop to the closure owner: {paths:#?}"
    );

    Ok(closure)
}

fn assert_returned_closure_context(
    db: &ploke_db::Database,
    case: ReturnedClosureCase,
) -> Result<(), DbError> {
    let owner = function_id_by_name(db, case.owner)?;
    let maker = function_id_by_name(db, case.maker)?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "{} returned closure context rows: {context:#?}",
        case.owner
    );

    let maker_row = row_by_path(&context, case.path);
    assert_eq!(maker_row.site.owner_id, owner);
    assert_eq!(maker_row.site.arg_count, Some(0));
    assert_eq!(maker_row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        maker_row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic = row_by_kind_path(&context, CallSiteKind::Dynamic, case.path);
    assert_eq!(dynamic.site.owner_id, owner);
    assert_eq!(dynamic.site.arg_count, Some(0));
    assert_eq!(dynamic.site.generic_arg_count, None);
    assert_eq!(dynamic.site.receiver, None);
    assert_eq!(
        dynamic.targets.len(),
        1,
        "{} returned closure row: {dynamic:#?}",
        case.owner
    );
    let closure = dynamic.targets[0].target_id;
    assert_resolved_target(
        dynamic,
        closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );

    let callers = db.callers_for_target(closure)?;
    let caller = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Dynamic, case.path);
    assert_eq!(caller.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller.target.relation, CallRelationKind::DynamicClosure);
    assert_eq!(caller.target.target_kind, CallTargetKind::Closure);

    let paths = db.call_paths_from_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == owner && path.end_id == closure && path.depth == 1)
        .unwrap_or_else(|| {
            panic!(
                "{} should have a one-hop path to the returned closure owner",
                case.owner
            )
        });
    assert_eq!(path.edges[0].caller_id, owner);
    assert_eq!(path.edges[0].callee_id, closure);
    assert_eq!(path.edges[0].relation, CallRelationKind::DynamicClosure);
    assert_eq!(path.edges[0].target_kind, CallTargetKind::Closure);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_resolved_dynamic_function_shapes() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let cases = [
        ResolvedDynamicContextCase {
            owner: "call_function_pointer_cast_path",
            path: &["local_target"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_function_pointer_cast_binding",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_dereferenced_function_pointer_binding",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_dereferenced_boxed_dyn_fn_value_binding",
            path: &["boxed_fn"],
            expected_rows: 2,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_referenced_dyn_fn_value_binding",
            path: &["referenced_fn"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_mut_referenced_dyn_fnmut_value_binding",
            path: &["referenced_fn"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_block_function_item",
            path: &["local_target"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_if_same_function_item",
            path: &["local_target"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_match_same_function_item",
            path: &["local_target"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_match_guarded_function_item",
            path: &["local_target"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_single_if_function_pointer_param_branch",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_single_match_function_pointer_param_arm",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_match_initialized_function_item_binding",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_if_nested_branch_expression",
            path: &["local_target"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_match_nested_arm_expression",
            path: &["local_target"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_named_field_function_binding",
            path: &["holder", "callback"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_aliased_named_field_function_binding",
            path: &["alias", "callback"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_indexed_named_field_function_binding",
            path: &["holder", "callbacks", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_indexed_named_field_array_alias_binding",
            path: &["holder", "callbacks", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_aliased_indexed_named_field_function_binding",
            path: &["alias", "callbacks", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_indexed_tuple_field_function_binding",
            path: &["holder", "0", "0"],
            expected_rows: 2,
        },
        ResolvedDynamicContextCase {
            owner: "call_indexed_tuple_field_array_alias_binding",
            path: &["holder", "0", "0"],
            expected_rows: 2,
        },
        ResolvedDynamicContextCase {
            owner: "call_aliased_indexed_tuple_field_function_binding",
            path: &["alias", "0", "0"],
            expected_rows: 2,
        },
        ResolvedDynamicContextCase {
            owner: "call_indexed_initialized_function_array",
            path: &["funcs", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_typed_indexed_initialized_function_array",
            path: &["funcs", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_aliased_indexed_initialized_function_array",
            path: &["alias", "0"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_single_named_field_function_param",
            path: &["holder", "callback"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_multi_named_field_function_param",
            path: &["holder", "callback"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_forwarded_named_field_leaf",
            path: &["holder", "callback"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_single_indexed_function_pointer_param",
            path: &["funcs", "0"],
            expected_rows: 1,
        },
    ];

    assert_resolved_dynamic_context_cases(&db, target, &cases)
}
