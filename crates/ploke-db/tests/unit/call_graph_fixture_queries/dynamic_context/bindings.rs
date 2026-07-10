use super::*;
use ploke_db::{CallContextRow, CallPathOptions};

#[test]
fn fixture_context_reads_projected_function_item_binding_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let imported_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "imported_target")?;
    let cases = [
        (
            "call_local_function_item_binding",
            path(&["f"]),
            local_target,
        ),
        (
            "call_aliased_function_item_binding",
            path(&["g"]),
            local_target,
        ),
        (
            "call_typed_function_pointer_binding",
            path(&["f"]),
            local_target,
        ),
        (
            "call_typed_function_pointer_alias_binding",
            path(&["g"]),
            local_target,
        ),
        (
            "call_imported_function_item_binding",
            path(&["f"]),
            imported_target,
        ),
        (
            "call_block_initialized_function_item_binding",
            path(&["f"]),
            local_target,
        ),
        (
            "call_if_initialized_function_item_binding",
            path(&["f"]),
            local_target,
        ),
    ];

    for (owner_name, expected_path, target) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
    }

    let owner = function_id_by_name(&db, "call_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "boxed dyn Fn binding context rows: {context:#?}"
    );
    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(
            &["Box", "new"],
            1,
            CallStatusKind::External,
            "boxed dyn Fn Box::new setup call",
        ),
    );
    let row = row_by_path(&context, &["boxed_fn"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["boxed_fn"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_resolved_target(
        row,
        local_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let owner = function_id_by_name(&db, "call_shadowed_local_target_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "shadowed binding context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["local_target"]);
    assert_ne!(
        row.targets[0].target_id, local_target,
        "shadowed closure binding must not fake-resolve to the module function"
    );
    assert_resolved_target(
        row,
        row.targets[0].target_id,
        CallRelationKind::Closure,
        CallSiteKind::Path,
        CallTargetKind::Closure,
    );

    let owner = function_id_by_name(&db, "call_if_ambiguous_initialized_function_item_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "ambiguous branch-initialized binding context rows: {context:#?}"
    );
    let expected = dynamic_candidates(&db)?;
    assert_path_function_candidates(
        &context[0],
        owner,
        &["f"],
        &expected,
        "ambiguous branch-initialized function pointer binding",
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_dynamic_function_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    assert_resolved_dynamic_context_cases(
        &db,
        target,
        &[ResolvedDynamicContextCase {
            owner: "call_parenthesized_local_target",
            path: &["local_target"],
            expected_rows: 1,
        }],
    )
}

#[test]
fn fixture_context_reads_projected_dynamic_closure_binding_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "dynamic_calls")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "dynamic_calls context rows: {context:#?}");

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:5:
    // `(closure)()` calls a named local closure binding and should target the
    // closure executable owner.
    assert_dynamic_closure_call(&db, &context, owner, &["closure"], "dynamic_calls")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:6:
    // `(|| 11)()` has no callee path, but it is a non-async closure literal
    // whose body owner can be targeted exactly.
    let literal_row = context
        .iter()
        .find(|row| {
            row.site.kind == CallSiteKind::Dynamic
                && row.site.path.is_none()
                && row
                    .targets
                    .iter()
                    .any(|target| target.relation == CallRelationKind::DynamicClosure)
        })
        .expect("dynamic closure literal should target its closure owner");
    assert_eq!(literal_row.site.owner_id, owner);
    assert_eq!(literal_row.site.arg_count, Some(0));
    assert_eq!(literal_row.site.generic_arg_count, None);
    assert_eq!(literal_row.targets.len(), 1);
    let literal_closure = literal_row.targets[0].target_id;
    assert_resolved_target(
        literal_row,
        literal_closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_closure_binding_fn_pointer_cast_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_closure_binding_cast")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "closure binding cast context rows: {context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:687-689:
    // `let closure = || 21; (closure as fn() -> i32)()` should preserve the
    // binding proof and target the closure executable owner.
    assert_dynamic_closure_call(
        &db,
        &context,
        owner,
        &["closure"],
        "call_closure_binding_cast",
    )?;

    Ok(())
}

#[test]
fn fixture_context_reads_projected_dereferenced_closure_binding_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_dereferenced_closure_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "dereferenced closure binding context rows: {context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:692-694:
    // `let closure = || 34; (*closure)()` should preserve the binding proof
    // and target the closure executable owner.
    assert_dynamic_closure_call(
        &db,
        &context,
        owner,
        &["closure"],
        "call_dereferenced_closure_binding",
    )?;

    Ok(())
}

#[test]
fn fixture_context_reads_projected_parenthesized_binding_dynamic_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let cases = [
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_function_item_binding",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_aliased_function_item_binding",
            path: &["g"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_typed_function_pointer_alias_binding",
            path: &["g"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_block_initialized_function_item_binding",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_match_initialized_function_item_binding",
            path: &["f"],
            expected_rows: 1,
        },
        ResolvedDynamicContextCase {
            owner: "call_parenthesized_boxed_dyn_fn_value_binding",
            path: &["boxed_fn"],
            expected_rows: 2,
        },
        ResolvedDynamicContextCase {
            owner: "call_dereferenced_boxed_dyn_fn_value_binding",
            path: &["boxed_fn"],
            expected_rows: 2,
        },
    ];

    assert_resolved_dynamic_context_cases(&db, target, &cases)?;

    for (owner_name, label) in [
        (
            "call_parenthesized_boxed_dyn_fn_value_binding",
            "parenthesized boxed dyn Fn Box::new setup call",
        ),
        (
            "call_dereferenced_boxed_dyn_fn_value_binding",
            "dereferenced boxed dyn Fn Box::new setup call",
        ),
    ] {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_targetless_row(
            &context,
            owner,
            TargetlessRowCase::path(&["Box", "new"], 1, CallStatusKind::External, label),
        );
    }

    Ok(())
}

fn assert_dynamic_closure_call(
    db: &Database,
    context: &[CallContextRow],
    owner: Uuid,
    path: &[&str],
    label: &str,
) -> Result<(), DbError> {
    let row = row_by_kind_path(context, CallSiteKind::Dynamic, path);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(
        row.targets.len(),
        1,
        "{label} dynamic closure row: {row:#?}"
    );
    let closure = row.targets[0].target_id;
    assert_resolved_target(
        row,
        closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );

    let callers = db.callers_for_target(closure)?;
    let caller = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Dynamic, path);
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
        .unwrap_or_else(|| panic!("{label} should have a one-hop path to the closure owner"));
    assert_eq!(path.edges[0].caller_id, owner);
    assert_eq!(path.edges[0].callee_id, closure);
    assert_eq!(path.edges[0].relation, CallRelationKind::DynamicClosure);
    assert_eq!(path.edges[0].target_kind, CallTargetKind::Closure);

    Ok(())
}
