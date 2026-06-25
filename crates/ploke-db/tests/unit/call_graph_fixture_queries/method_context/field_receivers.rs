use super::*;

#[test]
fn fixture_context_reads_projected_field_receiver_and_dynamic_field_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let owner = function_id_by_name(&db, "call_tuple_field_instance_method")?;
    let struct_target = struct_id_by_name(&db, "TupleFieldMethodReceiver")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "field-method context rows: {context:#?}");

    let row = row_by_path(&context, &["TupleFieldMethodReceiver"]);
    assert_resolved_target(
        row,
        struct_target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallTargetKind::Struct,
    );

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

    let owner = function_id_by_name(&db, "call_tuple_field_function")?;
    let struct_target = struct_id_by_name(&db, "TupleFieldFunction")?;
    let function_target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "field-dynamic context rows: {context:#?}");

    let row = row_by_path(&context, &["TupleFieldFunction"]);
    assert_resolved_target(
        row,
        struct_target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallTargetKind::Struct,
    );

    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["value", "0"]);
    assert_eq!(row.site.receiver, None);
    assert_resolved_target(
        row,
        function_target,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    Ok(())
}
