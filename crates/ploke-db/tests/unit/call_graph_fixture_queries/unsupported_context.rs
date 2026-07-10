use super::*;

#[test]
fn fixture_context_reads_projected_external_path_status_without_targets() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        (
            "call_prelude_string_new",
            TargetlessRowCase::path(
                &["String", "new"],
                0,
                CallStatusKind::External,
                "String::new external",
            ),
        ),
        (
            "call_external_default_bound_assoc",
            TargetlessRowCase::path(
                &["T", "default"],
                0,
                CallStatusKind::External,
                "T::default external Default-bound assoc",
            ),
        ),
    ];

    for (owner_name, expected) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        assert_targetless_row(&context, owner, expected);
    }

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
fn fixture_context_reads_generic_self_field_receiver_targetless_statuses() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_nodes")?;
    let receiver = CallReceiver::SelfField {
        path: path(&["value"]),
    };
    let cases = [
        (
            "generic str self-field len",
            method_id_by_impl_self_type_name(&db, "GenericStruct", "get_str_len")?,
            "len",
            CallStatusKind::External,
        ),
        (
            "generic SimpleTrait self-field into",
            method_id_by_impl_trait_and_self_type_names(
                &db,
                "SimpleTrait",
                "GenericStruct",
                "trait_method",
            )?,
            "into",
            CallStatusKind::External,
        ),
    ];

    for (label, owner, method, status) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{label} context rows: {context:#?}");

        // tests/fixture_crates/fixture_nodes/src/impls.rs:77 and :103:
        // `self.value.len()` is promoted through the concrete
        // `GenericStruct<&str>` impl argument; `self.value.into()` is promoted
        // through the source-visible external `Into<i32>` trait bound.
        assert_targetless_method_row(
            &context,
            owner,
            TargetlessMethodCase::method(method, &receiver, status, label),
        );
    }

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
        (
            &["crate"][..],
            "call_crate_scoped_macro",
            "crate::crate_scoped_macro",
        ),
        (&["crate"][..], "call_vec_macro", "vec"),
        (
            &["crate"][..],
            "call_imported_macro_alias",
            "imported_macro_alias",
        ),
        (
            &["crate"][..],
            "call_item_macro_inside_body",
            "call_graph_item_macro",
        ),
        (
            &["crate", "call_graph_tests"][..],
            "assert_eq_macro_call",
            "assert_eq",
        ),
    ];

    for (module_path, owner_name, macro_name) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
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
