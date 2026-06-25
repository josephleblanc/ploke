use super::*;

#[test]
fn fixture_context_reads_projected_external_and_shadowed_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_literal_str_to_string")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "literal to_string context rows: {context:#?}"
    );

    let receiver = CallReceiver::Literal;
    assert_targetless_method_row(
        &context,
        owner,
        TargetlessMethodCase::method(
            "to_string",
            &receiver,
            CallStatusKind::External,
            "literal to_string",
        ),
    );

    let owner = function_id_by_name(&db, "call_typed_vec_len_external")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "typed Vec context rows: {context:#?}");

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(&["Vec", "new"], 0, CallStatusKind::External, "Vec::new"),
    );

    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["Vec"]),
    };
    assert_targetless_method_row(
        &context,
        owner,
        TargetlessMethodCase::method(
            "len",
            &receiver,
            CallStatusKind::External,
            "unshadowed Vec::len",
        ),
    );

    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "local_prelude_shadow"],
        "call_shadowed_typed_vec_len",
    )?;
    let target = method_id_by_impl_self_type_name(&db, "Vec", "len")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "shadowed Vec::len context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("len"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["Vec"]),
        })
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}
