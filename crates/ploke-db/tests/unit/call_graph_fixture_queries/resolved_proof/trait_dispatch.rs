use super::super::*;

#[test]
fn fixture_projection_stores_real_trait_dispatch_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "LocalDispatchTrait",
        "TraitDispatchTarget",
        "trait_value",
    )?;
    let cases = [
        "call_initialized_local_trait_method",
        "call_concrete_trait_object_binding_method",
        "call_reference_chain_trait_object_binding_method",
    ];
    let mut expected_edges = Vec::new();

    for owner_name in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let receiver = CallReceiver::InitializedLocalBinding {
            name: "value".to_string(),
            init_path: path(&["TraitDispatchTarget"]),
        };
        let row = row_by_method_receiver(&context, "trait_value", &receiver);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3);
        expected_edges.push(OwnerProofEdge {
            owner,
            site: row.site.id,
            span: row.site.span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "trait dispatch",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}
