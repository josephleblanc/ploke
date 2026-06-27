use super::super::super::*;
use super::super::helpers::assert_resolved_call;

struct Case {
    label: &'static str,
    target: Uuid,
    owners: Vec<Owner>,
}

struct Owner {
    id: Uuid,
    label: &'static str,
}

#[tokio::test]
async fn proof_context_collection_preserves_target_centered_method_family_rows() -> Result<(), Error>
{
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let cases = vec![
        method_case(&db)?,
        associated_function_case(&db)?,
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

    let mut cfg = crate::RagConfig::default();
    cfg.proof_context.max_rows_per_part = 64;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.proof_context_degraded(),
        "target-centered method-family proof facts should enable RAG proof context"
    );

    for case in &cases {
        let proof_context = rag.collect_proof_context(&[(case.target, 1.0)])?;
        let rows = proof_context
            .get(&case.target)
            .unwrap_or_else(|| panic!("{} target seed should receive proof rows", case.label));
        for owner in &case.owners {
            assert_owner_rows(rows, owner, case);
        }
    }

    Ok(())
}

fn method_case(db: &Database) -> Result<Case, Error> {
    Ok(Case {
        label: "method target",
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

fn associated_function_case(db: &Database) -> Result<Case, Error> {
    Ok(Case {
        label: "associated-function target",
        target: one_uuid(db, &method_by_impl_self_query("LocalAssoc", "make"))?,
        owners: vec![
            owner(
                db,
                "local associated function",
                &["crate"],
                "call_local_assoc_make",
            )?,
            Owner {
                id: one_uuid(
                    db,
                    &method_by_impl_self_query("LocalAssoc", "call_self_make"),
                )?,
                label: "Self associated function",
            },
            owner(
                db,
                "qualified associated function",
                &["crate"],
                "call_qualified_local_assoc_make",
            )?,
        ],
    })
}

fn trait_dispatch_case(db: &Database) -> Result<Case, Error> {
    Ok(Case {
        label: "trait-dispatch target",
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

fn imported_trait_case(db: &Database) -> Result<Case, Error> {
    Ok(Case {
        label: "imported trait associated-function target",
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

fn owner(db: &Database, label: &'static str, module: &[&str], name: &str) -> Result<Owner, Error> {
    Ok(Owner {
        id: one_uuid(db, &function_in_module_query(module, name))?,
        label,
    })
}

fn assert_owner_rows(rows: &[ProofContextInfo], owner: &Owner, case: &Case) {
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
