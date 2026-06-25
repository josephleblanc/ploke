use super::*;

#[test]
fn fixture_projection_stores_real_target_centered_trait_dispatch_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "LocalDispatchTrait",
        "TraitDispatchTarget",
        "trait_value",
    )?;
    let initialized_owner = function_id_by_name(&db, "call_initialized_local_trait_method")?;
    let chained_owner =
        function_id_by_name(&db, "call_reference_chain_trait_object_binding_method")?;
    let receiver = CallReceiver::InitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["TraitDispatchTarget"]),
    };

    let initialized_context = db.call_context_for_owner(initialized_owner)?;
    let initialized_site = row_by_method_receiver(&initialized_context, "trait_value", &receiver)
        .site
        .id;

    let chained_context = db.call_context_for_owner(chained_owner)?;
    let chained_site = row_by_method_receiver(&chained_context, "trait_value", &receiver)
        .site
        .id;

    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 3, "target-centered trait dispatch")?;

    let initialized =
        caller_by_owner_method_receiver(&callers, initialized_owner, "trait_value", &receiver);
    assert_eq!(initialized.target.relation, CallRelationKind::Method);
    let chained =
        caller_by_owner_method_receiver(&callers, chained_owner, "trait_value", &receiver);
    assert_eq!(chained.target.relation, CallRelationKind::Method);

    assert_target_proof_projection(
        &db,
        "target-centered trait dispatch",
        "bd:fixture-call-graph",
        target,
        &callers,
        &[
            TargetProofSite {
                owner: initialized_owner,
                site: initialized_site,
            },
            TargetProofSite {
                owner: chained_owner,
                site: chained_site,
            },
        ],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
    )?;

    Ok(())
}
