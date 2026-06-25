use super::*;

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
