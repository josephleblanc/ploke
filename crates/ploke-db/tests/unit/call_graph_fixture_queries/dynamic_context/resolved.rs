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
    ];

    for case in cases {
        assert_returned_closure_context(&db, case)?;
    }

    Ok(())
}

struct ReturnedClosureCase {
    owner: &'static str,
    maker: &'static str,
    path: &'static [&'static str],
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
    ];

    assert_resolved_dynamic_context_cases(&db, target, &cases)
}
