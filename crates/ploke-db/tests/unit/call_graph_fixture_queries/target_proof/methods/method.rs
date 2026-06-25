use super::*;

#[test]
fn fixture_projection_stores_real_target_centered_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let method_owner = function_id_by_name(&db, "call_typed_local_instance_method")?;
    let assoc_owner = function_id_by_name(&db, "call_method_as_associated_function")?;

    let method_context = db.call_context_for_owner(method_owner)?;
    let method_receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let method_site = row_by_method_receiver(&method_context, "instance_value", &method_receiver)
        .site
        .id;

    let assoc_context = db.call_context_for_owner(assoc_owner)?;
    let assoc_site = row_by_path(&assoc_context, &["LocalAssoc", "instance_value"])
        .site
        .id;

    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 2, "target-centered method")?;
    assert_target_proof_projection(
        &db,
        "target-centered method",
        "bd:fixture-call-graph",
        target,
        &callers,
        &[
            TargetProofSite {
                owner: method_owner,
                site: method_site,
            },
            TargetProofSite {
                owner: assoc_owner,
                site: assoc_site,
            },
        ],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
    )?;

    Ok(())
}
