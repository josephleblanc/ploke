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

    let assoc_case = |label: &'static str,
                      owner: Uuid,
                      expected_path: &'static [&'static str],
                      target|
     -> ResolvedProofCase<'static> {
        ResolvedProofCase {
            label,
            owner,
            rows: 1,
            calls: vec![ResolvedProofCall::path(
                expected_path,
                target,
                CallRelationKind::AssociatedFunction,
                CallTargetKind::Method,
            )],
        }
    };
    let mut cases = Vec::new();
    cases.push(assoc_case(
        "call_local_assoc_make",
        function_id_by_name(&db, "call_local_assoc_make")?,
        &["LocalAssoc", "make"][..],
        local_make,
    ));
    cases.push(assoc_case(
        "call_self_make",
        method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?,
        &["Self", "make"],
        local_make,
    ));
    cases.push(assoc_case(
        "call_qualified_local_assoc_make",
        function_id_by_name(&db, "call_qualified_local_assoc_make")?,
        &["LocalAssoc", "make"],
        local_make,
    ));
    cases.push(assoc_case(
        "call_imported_type_assoc_make",
        function_id_by_name(&db, "call_imported_type_assoc_make")?,
        &["ImportedAssocAlias", "make"],
        imported_make,
    ));
    cases.push(assoc_case(
        "call_glob_imported_type_assoc_make",
        function_id_by_name(&db, "call_glob_imported_type_assoc_make")?,
        &["ImportedAssoc", "make"],
        imported_make,
    ));
    cases.push(assoc_case(
        "call_reexported_type_assoc_make",
        function_id_by_name(&db, "call_reexported_type_assoc_make")?,
        &["ReexportedAssoc", "make"],
        imported_make,
    ));
    cases.push(assoc_case(
        "call_type_alias_assoc_make",
        function_id_by_name(&db, "call_type_alias_assoc_make")?,
        &["LocalAssocTypeAlias", "make"],
        local_make,
    ));
    cases.push(assoc_case(
        "call_type_alias_chain_assoc_make",
        function_id_by_name(&db, "call_type_alias_chain_assoc_make")?,
        &["LocalAssocAliasChain", "make"],
        local_make,
    ));
    cases.push(assoc_case(
        "call_imported_type_alias_assoc_make",
        function_id_by_name(&db, "call_imported_type_alias_assoc_make")?,
        &["ImportedLocalAssocAlias", "make"],
        local_make,
    ));
    cases.push(assoc_case(
        "call_method_as_associated_function",
        function_id_by_name(&db, "call_method_as_associated_function")?,
        &["LocalAssoc", "instance_value"],
        instance_value,
    ));
    cases.push(assoc_case(
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
        cases.push(assoc_case(
            owner_name,
            function_id_by_name_in_module(&db, module_path, owner_name)?,
            expected_path,
            imported_trait_make,
        ));
    }

    assert_fixture_resolved_proofs(&db, "associated-function", &cases)?;

    Ok(())
}
