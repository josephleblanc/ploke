use super::*;

#[test]
fn fixture_context_reads_projected_external_path_status_without_targets() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_prelude_string_new")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(
            &["String", "new"],
            0,
            CallStatusKind::External,
            "String::new external",
        ),
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_ambiguous_method_status_without_targets() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_ambiguous_trait_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let receiver = CallReceiver::LocalBinding {
        name: "value".to_string(),
    };
    assert_targetless_method_row(
        &context,
        owner,
        TargetlessMethodCase::method(
            "overlap",
            &receiver,
            CallStatusKind::Ambiguous,
            "ambiguous overlap",
        ),
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_unsupported_method_status_without_targets() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_unimported_trait_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let receiver = CallReceiver::LocalBinding {
        name: "value".to_string(),
    };
    assert_targetless_method_row(
        &context,
        owner,
        TargetlessMethodCase::method(
            "scoped_value",
            &receiver,
            CallStatusKind::Unsupported,
            "unimported scoped_value",
        ),
    );

    Ok(())
}

#[test]
fn fixture_context_reads_function_pointer_param_cast_path_without_target() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_function_pointer_param_cast")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "function-pointer param cast context rows: {context:#?}"
    );

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::dynamic(
            Some(&["f"]),
            CallStatusKind::Unsupported,
            "opaque function-pointer param cast",
        ),
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_macro_statuses_without_targets() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        ("call_crate_scoped_macro", "crate::crate_scoped_macro"),
        ("call_vec_macro", "vec"),
        ("call_imported_macro_alias", "imported_macro_alias"),
        ("call_item_macro_inside_body", "call_graph_item_macro"),
    ];

    for (owner_name, macro_name) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        assert_targetless_macro_row(
            &context,
            owner,
            TargetlessMacroCase::macro_call(macro_name, owner_name),
        );
    }

    Ok(())
}
