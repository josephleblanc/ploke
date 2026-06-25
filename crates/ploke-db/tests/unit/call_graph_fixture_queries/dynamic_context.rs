use super::*;

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

    let owner = function_id_by_name(&db, "call_shadowed_local_target_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "shadowed binding context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["local_target"])));
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "shadowed closure binding must not fabricate local function edges: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_dynamic_function_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_parenthesized_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Dynamic);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["local_target"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(row.targets[0].relation, CallRelationKind::DynamicFunction);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Dynamic);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_parenthesized_binding_dynamic_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let cases = [
        ("call_parenthesized_function_item_binding", &["f"][..]),
        (
            "call_parenthesized_aliased_function_item_binding",
            &["g"][..],
        ),
        (
            "call_parenthesized_typed_function_pointer_alias_binding",
            &["g"][..],
        ),
    ];

    for (owner_name, expected_path) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, expected_path);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_eq!(row.site.receiver, None);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_returned_function_nested_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_returned_function")?;
    let target = function_id_by_name(&db, "make_fn")?;

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
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic_rows = context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Dynamic)
        .collect::<Vec<_>>();
    assert_eq!(
        dynamic_rows.len(),
        1,
        "expected one outer returned-function dynamic call row: {context:#?}"
    );
    let row = dynamic_rows[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "returned-function dynamic call must remain unsupported without fake targets: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_resolved_dynamic_function_shapes() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let cases = [
        (
            "call_function_pointer_cast_path",
            path(&["local_target"]),
            1,
        ),
        ("call_function_pointer_cast_binding", path(&["f"]), 1),
        (
            "call_dereferenced_function_pointer_binding",
            path(&["f"]),
            1,
        ),
        ("call_block_function_item", path(&["local_target"]), 1),
        ("call_if_same_function_item", path(&["local_target"]), 1),
        ("call_match_same_function_item", path(&["local_target"]), 1),
        (
            "call_named_field_function_binding",
            path(&["holder", "callback"]),
            1,
        ),
        (
            "call_aliased_named_field_function_binding",
            path(&["alias", "callback"]),
            1,
        ),
        (
            "call_indexed_named_field_function_binding",
            path(&["holder", "callbacks", "0"]),
            1,
        ),
        (
            "call_indexed_named_field_array_alias_binding",
            path(&["holder", "callbacks", "0"]),
            1,
        ),
        (
            "call_aliased_indexed_named_field_function_binding",
            path(&["alias", "callbacks", "0"]),
            1,
        ),
        (
            "call_indexed_tuple_field_function_binding",
            path(&["holder", "0", "0"]),
            2,
        ),
        (
            "call_indexed_tuple_field_array_alias_binding",
            path(&["holder", "0", "0"]),
            2,
        ),
        (
            "call_aliased_indexed_tuple_field_function_binding",
            path(&["alias", "0", "0"]),
            2,
        ),
        (
            "call_indexed_initialized_function_array",
            path(&["funcs", "0"]),
            1,
        ),
        (
            "call_typed_indexed_initialized_function_array",
            path(&["funcs", "0"]),
            1,
        ),
        (
            "call_aliased_indexed_initialized_function_array",
            path(&["alias", "0"]),
            1,
        ),
    ];

    for (owner_name, expected_path, expected_rows) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let refs = expected_path.iter().map(String::as_str).collect::<Vec<_>>();
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            expected_rows,
            "{owner_name} context rows: {context:#?}"
        );

        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &refs);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_eq!(row.site.receiver, None);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_targetless_dynamic_failures() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        (
            "call_match_guarded_function_item",
            None,
            CallStatusKind::Unsupported,
        ),
        ("call_if_closure_branch", None, CallStatusKind::Unsupported),
        ("call_match_closure_arm", None, CallStatusKind::Unsupported),
        (
            "call_parenthesized_function_pointer_param",
            Some(path(&["f"])),
            CallStatusKind::Unsupported,
        ),
        (
            "call_if_function_pointer_param_branch",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_match_function_pointer_param_arm",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_if_nested_branch_expression",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_match_nested_arm_expression",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_closure_binding_cast",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_dereferenced_closure_binding",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_field_function_param",
            Some(path(&["holder", "callback"])),
            CallStatusKind::Unsupported,
        ),
        (
            "call_indexed_function_pointer",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_indexed_field_function_param",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_indexed_tuple_field_function_param",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_move_closure_literal_with_body_call",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_async_closure_literal_with_body_call",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_parenthesized_generic_fn_once_value_binding",
            Some(path(&["generic_f"])),
            CallStatusKind::Unsupported,
        ),
    ];

    for (owner_name, expected_path, expected_status) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Dynamic);
        assert_eq!(row.site.path, expected_path);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_eq!(row.status.status, expected_status);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "{owner_name} dynamic failure must not fabricate targets: {row:#?}"
        );
    }

    let owner = function_id_by_name(&db, "call_parenthesized_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "boxed dyn Fn context rows: {context:#?}");

    let row = row_by_path(&context, &["Box", "new"]);
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "Box::new must not fabricate local target edges: {row:#?}"
    );

    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["boxed_fn"]);
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "boxed dyn Fn dynamic call must remain unsupported without fake targets: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_does_not_project_closure_or_async_body_calls_to_outer_owner()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let forbidden_path = path(&["local_target"]);
    let owners = [
        "closure_body_call_is_not_outer_call_site",
        "async_block_call_is_not_outer_call_site",
        "call_move_closure_literal_with_body_call",
        "call_async_closure_literal_with_body_call",
    ];

    for owner_name in owners {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert!(
            context.iter().all(|row| row.site.kind != CallSiteKind::Path
                || row.site.path.as_ref() != Some(&forbidden_path)),
            "{owner_name} leaked a closure/async body local_target() path row into the outer owner: {context:#?}"
        );
        assert!(
            context
                .iter()
                .flat_map(|row| row.targets.iter())
                .all(|target| target.target_id != local_target),
            "{owner_name} leaked a closure/async body edge to local_target into the outer owner: {context:#?}"
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_callable_value_path_failures_and_vec_external()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let path_failures = [
        ("call_function_pointer_param", &["f"][..]),
        ("call_generic_fn_once_value_binding", &["generic_f"][..]),
    ];

    for (owner_name, expected_path) in path_failures {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = row_by_path(&context, expected_path);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "callable value path row must not fabricate local targets: {row:#?}"
        );
    }

    let owner = function_id_by_name(&db, "call_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "boxed dyn Fn path context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["Box", "new"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "Box::new setup call must not fabricate local targets: {row:#?}"
    );

    let row = row_by_path(&context, &["boxed_fn"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "boxed dyn Fn path call must not fabricate local targets: {row:#?}"
    );

    let owner = function_id_by_name(&db, "call_prelude_vec_new")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "Vec::new context rows: {context:#?}");
    let row = row_by_path(&context, &["Vec", "new"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "Vec::new external row must not fabricate local targets: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_ambiguous_dynamic_candidates() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;

    for owner_name in AMBIGUOUS_DYNAMIC_OWNERS {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        assert_dynamic_candidates(&context[0], owner, &expected, owner_name);
    }

    Ok(())
}
