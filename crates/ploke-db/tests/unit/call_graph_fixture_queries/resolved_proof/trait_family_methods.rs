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
    let method_case = |label: &'static str,
                       method: &'static str,
                       receiver,
                       target|
     -> Result<ResolvedProofCase<'static>, DbError> {
        Ok(ResolvedProofCase {
            label,
            owner: function_id_by_name(&db, label)?,
            rows: 1,
            calls: vec![ResolvedProofCall::method(method, receiver, target)],
        })
    };
    let scoped_case = |module_path: &[&str],
                       label: &'static str,
                       method: &'static str,
                       receiver,
                       target|
     -> Result<ResolvedProofCase<'static>, DbError> {
        Ok(ResolvedProofCase {
            label,
            owner: function_id_by_name_in_module(&db, module_path, label)?,
            rows: 1,
            calls: vec![ResolvedProofCall::method(method, receiver, target)],
        })
    };
    let mut cases = Vec::new();

    for owner_name in [
        "call_inline_generic_bound_method",
        "call_where_generic_bound_method",
        "call_impl_trait_method",
        "call_trait_object_method",
    ] {
        cases.push(method_case(
            owner_name,
            "bound_value",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            generic_target,
        )?);
    }

    cases.push(method_case(
        "call_local_trait_object_binding_method",
        "bound_value",
        CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["GenericBoundTrait"]),
        },
        generic_target,
    )?);

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
        cases.push(scoped_case(
            module_path,
            owner_name,
            "scoped_value",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            imported_target,
        )?);
    }

    cases.push(method_case(
        "call_constrained_generic_self_trait_method",
        "constrained_generic_self_value",
        CallReceiver::LocalBinding {
            name: "value".to_string(),
        },
        constrained_target,
    )?);

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
        cases.push(method_case(
            owner_name,
            method_name,
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            method_id_by_impl_trait_name(&db, trait_name, method_name)?,
        )?);
    }

    assert_fixture_resolved_proofs(&db, "trait family method", &cases)?;

    Ok(())
}
