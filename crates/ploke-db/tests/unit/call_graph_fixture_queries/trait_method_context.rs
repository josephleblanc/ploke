use super::*;

#[test]
fn fixture_context_reads_projected_trait_dispatch_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "LocalDispatchTrait",
        "TraitDispatchTarget",
        "trait_value",
    )?;
    let cases = [
        (
            "call_param_trait_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_typed_local_trait_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_initialized_local_trait_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_parenthesized_initialized_local_trait_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_concrete_trait_object_binding_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_aliased_concrete_trait_object_binding_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_reference_alias_trait_object_binding_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_reference_chain_trait_object_binding_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("trait_value"));
        assert_eq!(row.site.receiver, Some(receiver));
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert_eq!(row.targets[0].target_id, target);
        assert_eq!(row.targets[0].relation, CallRelationKind::Method);
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
        assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_constrained_generic_self_trait_method() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_constrained_generic_self_trait_method")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ConstrainedGenericSelfTrait",
        "GenericWrapper",
        "constrained_generic_self_value",
    )?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "constrained generic self trait method context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(
        row.site.method.as_deref(),
        Some("constrained_generic_self_value")
    );
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::LocalBinding {
            name: "value".to_string(),
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
fn fixture_context_reads_projected_generic_bound_and_trait_object_methods() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_trait_name(&db, "GenericBoundTrait", "bound_value")?;
    let cases = [
        (
            "call_inline_generic_bound_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_where_generic_bound_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_impl_trait_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_trait_object_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_local_trait_object_binding_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["GenericBoundTrait"]),
            },
        ),
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("bound_value"));
        assert_eq!(row.site.receiver, Some(receiver));
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
fn fixture_context_reads_projected_imported_trait_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ScopedTrait",
        "ScopedTraitTarget",
        "scoped_value",
    )?;
    let cases = [
        (
            &["crate", "trait_scope", "with_direct_import"][..],
            "call_direct_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_alias_import"][..],
            "call_alias_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_glob_import"][..],
            "call_glob_imported_trait_method",
        ),
        (
            &["crate", "trait_reexport_scope"][..],
            "call_reexported_trait_method",
        ),
        (
            &["crate", "grouped_trait_import_scope"][..],
            "call_grouped_imported_trait_method",
        ),
    ];

    for (module_path, owner_name) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("scoped_value"));
        assert_eq!(
            row.site.receiver,
            Some(CallReceiver::LocalBinding {
                name: "value".to_string(),
            })
        );
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
fn fixture_context_reads_projected_blanket_trait_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        (
            "call_blanket_trait_method",
            "BlanketDispatchTrait",
            "blanket_value",
        ),
        (
            "call_inline_bound_blanket_trait_method",
            "InlineBoundBlanketTrait",
            "inline_bound_value",
        ),
        (
            "call_where_bound_blanket_trait_method",
            "WhereBoundBlanketTrait",
            "where_bound_value",
        ),
        (
            "call_transitive_bound_blanket_trait_method",
            "TransitiveBoundBlanketTrait",
            "transitive_bound_value",
        ),
    ];

    for (owner_name, trait_name, method_name) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let target = method_id_by_impl_trait_name(&db, trait_name, method_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some(method_name));
        assert_eq!(
            row.site.receiver,
            Some(CallReceiver::LocalBinding {
                name: "value".to_string(),
            })
        );
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
fn fixture_context_reads_projected_borrowed_and_dereferenced_method_receivers()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let cases = [
        (
            "call_borrowed_typed_local_instance_method",
            CallReceiver::BorrowedTypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_dereferenced_local_instance_method",
            CallReceiver::DereferencedInitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_dereferenced_param_instance_method",
            CallReceiver::DereferencedLocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_borrowed_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
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
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("instance_value"));
        assert_eq!(row.site.receiver, Some(receiver));
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert_eq!(row.targets[0].target_id, target);
        assert_eq!(row.targets[0].relation, CallRelationKind::Method);
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
        assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
    }

    Ok(())
}
