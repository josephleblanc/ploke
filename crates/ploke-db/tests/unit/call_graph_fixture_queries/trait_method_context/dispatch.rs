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
