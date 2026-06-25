use super::super::*;

#[test]
fn fixture_projection_stores_real_trait_family_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let generic_target = method_id_by_trait_name(&db, "GenericBoundTrait", "bound_value")?;
    let imported_target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ScopedTrait",
        "ScopedTraitTarget",
        "scoped_value",
    )?;
    let constrained_target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ConstrainedGenericSelfTrait",
        "GenericWrapper",
        "constrained_generic_self_value",
    )?;
    let mut cases = Vec::new();

    for owner_name in [
        "call_inline_generic_bound_method",
        "call_where_generic_bound_method",
        "call_impl_trait_method",
        "call_trait_object_method",
    ] {
        cases.push((
            owner_name,
            function_id_by_name(&db, owner_name)?,
            "bound_value",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            generic_target,
        ));
    }

    cases.push((
        "call_local_trait_object_binding_method",
        function_id_by_name(&db, "call_local_trait_object_binding_method")?,
        "bound_value",
        CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["GenericBoundTrait"]),
        },
        generic_target,
    ));

    for (module_path, owner_name) in [
        (
            &["crate", "trait_scope", "with_direct_import"][..],
            "call_direct_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_alias_import"],
            "call_alias_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_glob_import"],
            "call_glob_imported_trait_method",
        ),
        (
            &["crate", "trait_reexport_scope"],
            "call_reexported_trait_method",
        ),
        (
            &["crate", "grouped_trait_import_scope"],
            "call_grouped_imported_trait_method",
        ),
    ] {
        cases.push((
            owner_name,
            function_id_by_name_in_module(&db, module_path, owner_name)?,
            "scoped_value",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            imported_target,
        ));
    }

    cases.push((
        "call_constrained_generic_self_trait_method",
        function_id_by_name(&db, "call_constrained_generic_self_trait_method")?,
        "constrained_generic_self_value",
        CallReceiver::LocalBinding {
            name: "value".to_string(),
        },
        constrained_target,
    ));

    for (owner_name, trait_name, method_name) in [
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
    ] {
        cases.push((
            owner_name,
            function_id_by_name(&db, owner_name)?,
            method_name,
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            method_id_by_impl_trait_name(&db, trait_name, method_name)?,
        ));
    }

    let mut expected_edges = Vec::new();

    for (owner_name, owner, method_name, receiver, target) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_method_receiver(&context, method_name, &receiver);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "trait family method",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}
