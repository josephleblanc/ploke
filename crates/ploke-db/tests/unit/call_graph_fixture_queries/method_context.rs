use super::*;

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
fn fixture_context_reads_projected_external_and_shadowed_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_literal_str_to_string")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "literal to_string context rows: {context:#?}"
    );

    let receiver = CallReceiver::Literal;
    assert_targetless_method_row(
        &context,
        owner,
        TargetlessMethodCase::method(
            "to_string",
            &receiver,
            CallStatusKind::External,
            "literal to_string",
        ),
    );

    let owner = function_id_by_name(&db, "call_typed_vec_len_external")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "typed Vec context rows: {context:#?}");

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(&["Vec", "new"], 0, CallStatusKind::External, "Vec::new"),
    );

    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["Vec"]),
    };
    assert_targetless_method_row(
        &context,
        owner,
        TargetlessMethodCase::method(
            "len",
            &receiver,
            CallStatusKind::External,
            "unshadowed Vec::len",
        ),
    );

    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "local_prelude_shadow"],
        "call_shadowed_typed_vec_len",
    )?;
    let target = method_id_by_impl_self_type_name(&db, "Vec", "len")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "shadowed Vec::len context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("len"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["Vec"]),
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

#[test]
fn fixture_context_reads_projected_inherent_method_precedence() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_inherent_over_trait_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "inherent precedence rows: {context:#?}");

    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["InherentPrecedenceTarget"]),
    };
    let row = row_by_method_receiver(&context, "priority", &receiver);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].relation, CallRelationKind::Method);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
    assert!(
        method_owner_is_inherent_impl(&db, row.targets[0].target_id, "InherentPrecedenceTarget")?,
        "inherent method call must project a target owned by the inherent impl: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_preserves_path_result_owner_order_and_targets() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_path_result_instance_method")?;
    let make_target = function_id_by_name(&db, "make_local_assoc")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "context rows: {context:#?}");

    let path_row = &context[0];
    assert_eq!(path_row.site.kind, CallSiteKind::Path);
    assert_eq!(
        path_row.site.path.as_ref(),
        Some(&path(&["make_local_assoc"]))
    );
    assert_eq!(path_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        path_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(path_row.targets.len(), 1);
    assert_eq!(path_row.targets[0].target_id, make_target);
    assert_eq!(path_row.targets[0].relation, CallRelationKind::Function);

    let method_row = &context[1];
    assert_eq!(method_row.site.kind, CallSiteKind::Method);
    assert_eq!(method_row.site.method.as_deref(), Some("instance_value"));
    assert_eq!(
        method_row.site.receiver,
        Some(CallReceiver::PathCallResult {
            path: path(&["make_local_assoc"]),
        })
    );
    assert_eq!(method_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        method_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(method_row.targets.len(), 1);
    assert_eq!(method_row.targets[0].relation, CallRelationKind::Method);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_result_receiver_method_chains() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let owner = function_id_by_name(&db, "call_method_result_instance_method")?;
    let clone_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "clone_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "method-result context rows: {context:#?}");

    let clone_receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let row = row_by_method_receiver(&context, "clone_assoc", &clone_receiver);
    assert_resolved_target(
        row,
        clone_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let result_receiver = CallReceiver::MethodCallResult {
        method_name: "clone_assoc".to_string(),
    };
    let row = row_by_method_receiver(&context, "instance_value", &result_receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = function_id_by_name(&db, "call_await_result_instance_method")?;
    let ready_target = function_id_by_name(&db, "make_ready_local_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "await-result context rows: {context:#?}");

    let row = row_by_path(&context, &["make_ready_local_assoc"]);
    assert_resolved_target(
        row,
        ready_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let await_receiver = CallReceiver::AwaitPathCallResult {
        path: path(&["make_ready_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &await_receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let try_target = function_id_by_name(&db, "try_local_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "try-result context rows: {context:#?}");

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(&["Ok"], 1, CallStatusKind::Unsupported, "try-result Ok"),
    );

    let row = row_by_path(&context, &["try_local_assoc"]);
    assert_resolved_target(
        row,
        try_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let try_receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &try_receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_field_receiver_and_dynamic_field_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let owner = function_id_by_name(&db, "call_tuple_field_instance_method")?;
    let struct_target = struct_id_by_name(&db, "TupleFieldMethodReceiver")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "field-method context rows: {context:#?}");

    let row = row_by_path(&context, &["TupleFieldMethodReceiver"]);
    assert_resolved_target(
        row,
        struct_target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallTargetKind::Struct,
    );

    let receiver = CallReceiver::FieldInitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["TupleFieldMethodReceiver"]),
        field_path: path(&["0"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = function_id_by_name(&db, "call_tuple_field_function")?;
    let struct_target = struct_id_by_name(&db, "TupleFieldFunction")?;
    let function_target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "field-dynamic context rows: {context:#?}");

    let row = row_by_path(&context, &["TupleFieldFunction"]);
    assert_resolved_target(
        row,
        struct_target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallTargetKind::Struct,
    );

    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["value", "0"]);
    assert_eq!(row.site.receiver, None);
    assert_resolved_target(
        row,
        function_target,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    Ok(())
}
