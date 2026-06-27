use super::super::assertions::trait_method_query;
use super::super::*;
use super::helpers::assert_resolved_call;

struct Case {
    label: &'static str,
    search: &'static str,
    top_k: usize,
    call_id: &'static str,
    target: Uuid,
    owners: Vec<Owner>,
}

struct Owner {
    id: Uuid,
    label: &'static str,
}

#[tokio::test]
async fn request_code_context_returns_target_centered_method_proof_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let cases = vec![
        method_case(&db)?,
        trait_dispatch_case(&db)?,
        imported_trait_case(&db)?,
    ];

    for case in &cases {
        let count = db.project_call_proof_facts_for_target(case.target, "bd:fixture-call-graph")?;
        assert!(
            count >= case.owners.len() * 3,
            "{} should project at least one resolved proof row set for every selected owner",
            case.label
        );
    }

    for case in &cases {
        let tool_result =
            execute_fixture_tool_request(&db, case.search, case.top_k, case.call_id).await?;
        let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
        assert_result_ok(&result, case.search, case.top_k, "fixture_call_graph");
        assert!(
            result
                .note
                .as_deref()
                .is_none_or(|note| { !note.contains("Proof-context expansion is unavailable") }),
            "projected {} proof facts should avoid degraded proof-context note: {result:#?}",
            case.label
        );

        let target_part = result
            .context
            .iter()
            .find(|part| part.id == case.target)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} proof target",
                    case.label
                )
            });
        for owner in &case.owners {
            assert_owner_rows(&target_part.proof_context, owner, case);
        }
    }

    Ok(())
}

fn method_case(db: &Database) -> color_eyre::Result<Case> {
    Ok(Case {
        label: "method target",
        search: "instance_value",
        top_k: 1,
        call_id: "target_method_proof_context",
        target: one_uuid(
            db,
            &method_by_impl_self_query("LocalAssoc", "instance_value"),
        )?,
        owners: vec![
            owner(
                db,
                "typed local method",
                &["crate"],
                "call_typed_local_instance_method",
            )?,
            owner(
                db,
                "nested-reference method",
                &["crate"],
                "call_typed_double_reference_local_instance_method",
            )?,
            Owner {
                id: one_uuid(
                    db,
                    &method_by_impl_self_query(
                        "SelfFieldAssocOwner",
                        "call_self_field_instance_method",
                    ),
                )?,
                label: "self-field method",
            },
            owner(
                db,
                "method as associated function",
                &["crate"],
                "call_method_as_associated_function",
            )?,
        ],
    })
}

fn trait_dispatch_case(db: &Database) -> color_eyre::Result<Case> {
    Ok(Case {
        label: "trait-dispatch target",
        search: "144 trait_value",
        top_k: 5,
        call_id: "target_trait_dispatch_proof_context",
        target: one_uuid(
            db,
            &method_by_impl_trait_self_query(
                "LocalDispatchTrait",
                "TraitDispatchTarget",
                "trait_value",
            ),
        )?,
        owners: vec![
            owner(
                db,
                "initialized trait-dispatch caller",
                &["crate"],
                "call_initialized_local_trait_method",
            )?,
            owner(
                db,
                "reference-chain trait-object caller",
                &["crate"],
                "call_reference_chain_trait_object_binding_method",
            )?,
        ],
    })
}

fn imported_trait_case(db: &Database) -> color_eyre::Result<Case> {
    Ok(Case {
        label: "imported trait associated-function target",
        search: "987 imported_trait_make",
        top_k: 1,
        call_id: "target_imported_trait_assoc_proof_context",
        target: one_uuid(
            db,
            &trait_method_query("ImportedAssocFunctionTrait", "imported_trait_make"),
        )?,
        owners: vec![
            owner(
                db,
                "direct imported trait associated function",
                &["crate", "trait_assoc_function_scope", "with_direct_import"],
                "call_direct_imported_trait_associated_function",
            )?,
            owner(
                db,
                "alias imported trait associated function",
                &["crate", "trait_assoc_function_scope", "with_alias_import"],
                "call_alias_imported_trait_associated_function",
            )?,
            owner(
                db,
                "glob imported trait associated function",
                &["crate", "trait_assoc_function_scope", "with_glob_import"],
                "call_glob_imported_trait_associated_function",
            )?,
            owner(
                db,
                "re-exported trait associated function",
                &["crate", "trait_assoc_reexport_scope"],
                "call_reexported_trait_associated_function",
            )?,
            owner(
                db,
                "grouped imported trait associated function",
                &["crate", "grouped_trait_assoc_function_scope"],
                "call_grouped_imported_trait_associated_function",
            )?,
        ],
    })
}

fn owner(
    db: &Database,
    label: &'static str,
    module: &[&str],
    name: &str,
) -> color_eyre::Result<Owner> {
    Ok(Owner {
        id: one_uuid(db, &function_in_module_query(module, name))?,
        label,
    })
}

fn assert_owner_rows(rows: &[ploke_core::rag_types::ProofContextInfo], owner: &Owner, case: &Case) {
    let owner_id = owner.id.to_string();
    assert!(
        rows.iter()
            .any(|row| row.caller_def_id.as_deref() == Some(owner_id.as_str())),
        "{} proof rows should include {}: {rows:#?}",
        case.label,
        owner.label
    );
    assert_resolved_call(rows, owner.id, case.target);
}
