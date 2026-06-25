use super::super::*;

#[test]
fn fixture_projection_stores_real_associated_function_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_make = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let imported_make = method_id_by_impl_self_type_name(&db, "ImportedAssoc", "make")?;
    let instance_value = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let local_trait_make = method_id_by_trait_name(&db, "LocalAssocFunctionTrait", "trait_make")?;
    let imported_trait_make =
        method_id_by_trait_name(&db, "ImportedAssocFunctionTrait", "imported_trait_make")?;

    let mut cases = Vec::new();
    cases.push((
        "call_local_assoc_make",
        function_id_by_name(&db, "call_local_assoc_make")?,
        &["LocalAssoc", "make"][..],
        local_make,
    ));
    cases.push((
        "call_self_make",
        method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?,
        &["Self", "make"],
        local_make,
    ));
    cases.push((
        "call_qualified_local_assoc_make",
        function_id_by_name(&db, "call_qualified_local_assoc_make")?,
        &["LocalAssoc", "make"],
        local_make,
    ));
    cases.push((
        "call_imported_type_assoc_make",
        function_id_by_name(&db, "call_imported_type_assoc_make")?,
        &["ImportedAssocAlias", "make"],
        imported_make,
    ));
    cases.push((
        "call_glob_imported_type_assoc_make",
        function_id_by_name(&db, "call_glob_imported_type_assoc_make")?,
        &["ImportedAssoc", "make"],
        imported_make,
    ));
    cases.push((
        "call_reexported_type_assoc_make",
        function_id_by_name(&db, "call_reexported_type_assoc_make")?,
        &["ReexportedAssoc", "make"],
        imported_make,
    ));
    cases.push((
        "call_type_alias_assoc_make",
        function_id_by_name(&db, "call_type_alias_assoc_make")?,
        &["LocalAssocTypeAlias", "make"],
        local_make,
    ));
    cases.push((
        "call_type_alias_chain_assoc_make",
        function_id_by_name(&db, "call_type_alias_chain_assoc_make")?,
        &["LocalAssocAliasChain", "make"],
        local_make,
    ));
    cases.push((
        "call_imported_type_alias_assoc_make",
        function_id_by_name(&db, "call_imported_type_alias_assoc_make")?,
        &["ImportedLocalAssocAlias", "make"],
        local_make,
    ));
    cases.push((
        "call_method_as_associated_function",
        function_id_by_name(&db, "call_method_as_associated_function")?,
        &["LocalAssoc", "instance_value"],
        instance_value,
    ));
    cases.push((
        "call_trait_associated_function",
        function_id_by_name(&db, "call_trait_associated_function")?,
        &["LocalAssocFunctionTrait", "trait_make"],
        local_trait_make,
    ));

    for (module_path, owner_name, expected_path) in [
        (
            &["crate", "trait_assoc_function_scope", "with_direct_import"][..],
            "call_direct_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_alias_import"],
            "call_alias_imported_trait_associated_function",
            &["VisibleAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_glob_import"],
            "call_glob_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "trait_assoc_reexport_scope"],
            "call_reexported_trait_associated_function",
            &["ReexportedAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "grouped_trait_assoc_function_scope"],
            "call_grouped_imported_trait_associated_function",
            &["GroupedAssocFunctionTrait", "imported_trait_make"],
        ),
    ] {
        cases.push((
            owner_name,
            function_id_by_name_in_module(&db, module_path, owner_name)?,
            expected_path,
            imported_trait_make,
        ));
    }

    let mut expected_edges = Vec::new();

    for (owner_name, owner, expected_path, target) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_path(&context, expected_path);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
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
        "associated-function",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}
