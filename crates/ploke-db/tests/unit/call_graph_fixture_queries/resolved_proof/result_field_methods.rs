use super::super::*;

#[test]
fn fixture_projection_stores_real_result_and_field_receiver_method_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let clone_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "clone_assoc")?;
    let make_target = function_id_by_name(&db, "make_local_assoc")?;
    let ready_target = function_id_by_name(&db, "make_ready_local_assoc")?;
    let tuple_target = struct_id_by_name(&db, "TupleFieldMethodReceiver")?;
    let mut expected_edges = Vec::new();

    let owner = function_id_by_name(&db, "call_path_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "path-result context rows: {context:#?}");
    let row = row_by_path(&context, &["make_local_assoc"]);
    assert_resolved_target(
        row,
        make_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: make_target,
    });

    let receiver = CallReceiver::PathCallResult {
        path: path(&["make_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    let owner = function_id_by_name(&db, "call_method_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "method-result context rows: {context:#?}");
    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let row = row_by_method_receiver(&context, "clone_assoc", &receiver);
    assert_resolved_target(
        row,
        clone_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: clone_target,
    });

    let receiver = CallReceiver::MethodCallResult {
        method_name: "clone_assoc".to_string(),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    let owner = function_id_by_name(&db, "call_await_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "await-result context rows: {context:#?}");
    let row = row_by_path(&context, &["make_ready_local_assoc"]);
    assert_resolved_target(
        row,
        ready_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: ready_target,
    });

    let receiver = CallReceiver::AwaitPathCallResult {
        path: path(&["make_ready_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    let owner = function_id_by_name(&db, "call_tuple_field_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "field-method context rows: {context:#?}");
    let row = row_by_path(&context, &["TupleFieldMethodReceiver"]);
    assert_resolved_target(
        row,
        tuple_target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallTargetKind::Struct,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: tuple_target,
    });

    let receiver = CallReceiver::FieldInitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["TupleFieldMethodReceiver"]),
        field_path: path(&["0"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    assert_owner_proof_edges(
        &db,
        "result/field receiver method",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}
