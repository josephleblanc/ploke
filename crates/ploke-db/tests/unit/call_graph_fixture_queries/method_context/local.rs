use super::*;

const METHOD_TUPLE_RETURN_PATTERN_LOCAL_INIT_CALL_SPAN: (usize, usize) = (40981, 40999);

#[test]
fn fixture_context_reads_projected_typed_local_method_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_typed_local_instance_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("instance_value"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["LocalAssoc"]),
        })
    );
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].relation, CallRelationKind::Method);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_local_and_alias_instance_method_receivers() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let cases = [
        (
            "call_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_initialized_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_initialized_local_alias_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_tuple_pattern_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_parenthesized_typed_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_type_alias_chain_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssocAliasChain"]),
            },
        ),
        (
            "call_imported_type_alias_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["ImportedLocalAssocAlias"]),
            },
        ),
        (
            "call_if_expression_receiver_method",
            CallReceiver::IfBranchPaths {
                paths: vec![path(&["LocalAssoc"]), path(&["LocalAssoc"])],
            },
        ),
        (
            "call_match_expression_receiver_method",
            CallReceiver::IfBranchPaths {
                paths: vec![path(&["LocalAssoc"]), path(&["LocalAssoc"])],
            },
        ),
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = row_by_method_receiver(&context, "instance_value", &receiver);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_typed_tuple_pattern_receiver() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_typed_tuple_pattern_local_instance_method")?;
    let pair_target = function_id_by_name(&db, "make_local_assoc_pair")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "typed tuple-pattern owner should expose the tuple initializer and receiver rows: {context:#?}"
    );

    let path_row = row_by_path(&context, &["make_local_assoc_pair"]);
    assert_eq!(path_row.site.owner_id, owner);
    assert_eq!(path_row.site.arg_count, Some(0));
    assert_resolved_target(
        path_row,
        pair_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let method_row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_eq!(method_row.site.owner_id, owner);
    assert_eq!(method_row.site.arg_count, Some(0));
    assert_resolved_target(
        method_row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_tuple_return_pattern_receiver() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_tuple_return_pattern_local_instance_method")?;
    let pair_target = function_id_by_name(&db, "make_local_assoc_pair")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "tuple-return owner should expose the tuple initializer and receiver rows: {context:#?}"
    );

    let path_row = row_by_path(&context, &["make_local_assoc_pair"]);
    assert_eq!(path_row.site.owner_id, owner);
    assert_eq!(path_row.site.arg_count, Some(0));
    assert_resolved_target(
        path_row,
        pair_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // `let (value, _) = make_local_assoc_pair(); value.instance_value()`
    // should use the local function's tuple return type as receiver proof.
    let receiver = CallReceiver::TupleReturnBinding {
        name: "value".to_string(),
        path: path(&["make_local_assoc_pair"]),
        index: 0,
    };
    let method_row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_eq!(method_row.site.owner_id, owner);
    assert_eq!(method_row.site.arg_count, Some(0));
    assert_resolved_target(
        method_row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_method_tuple_return_pattern_receiver() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(
        &db,
        "call_method_tuple_return_pattern_local_instance_method",
    )?;
    let pair_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "tuple_pair")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "method tuple-return owner should expose the initializer and receiver rows: {context:#?}"
    );

    let init_receiver = CallReceiver::InitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["LocalAssoc"]),
    };
    let init_row = row_by_method_receiver(&context, "tuple_pair", &init_receiver);
    assert_eq!(init_row.site.owner_id, owner);
    assert_eq!(init_row.site.arg_count, Some(0));
    assert_resolved_target(
        init_row,
        pair_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // `let (next, _) = value.tuple_pair(); next.instance_value()`
    // should use the initializer method's tuple return type as receiver proof.
    let receiver = CallReceiver::TupleMethodReturn {
        name: "next".to_string(),
        method_name: "tuple_pair".to_string(),
        method_span: METHOD_TUPLE_RETURN_PATTERN_LOCAL_INIT_CALL_SPAN,
        index: 0,
    };
    let method_row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_eq!(method_row.site.owner_id, owner);
    assert_eq!(method_row.site.arg_count, Some(0));
    assert_resolved_target(
        method_row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_reference_instance_method_receivers() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let cases = [
        (
            "call_referenced_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_typed_reference_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_typed_double_reference_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = row_by_method_receiver(&context, "instance_value", &receiver);
        assert_eq!(row.site.owner_id, owner);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_explicit_drop_method_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_explicit_drop_method")?;
    let target = method_id_by_impl_self_type_name(&db, "ExplicitDropTarget", "drop")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "explicit drop context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("drop"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::InitializedLocalBinding {
            name: "value".to_string(),
            init_path: path(&["ExplicitDropTarget"]),
        })
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}
