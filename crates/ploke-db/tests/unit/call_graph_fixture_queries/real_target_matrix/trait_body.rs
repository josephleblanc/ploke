use super::super::*;
use super::common::*;

#[test]
fn axum_real_target_trait_associated_paths_are_documented_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: trait-associated extraction calls.
    // Source chain:
    //   axum-core/src/ext_traits/request.rs:279 calls `E::from_request`.
    //   axum-core/src/ext_traits/request.rs:305 and
    //   ext_traits/request_parts.rs:133 call `E::from_request_parts`.
    // Current model gap: these type-parameter trait calls are structurally
    // visible but remain unsupported and targetless in the axum fixture.
    let cases = [
        (
            "axum-core/src/ext_traits/request.rs:279",
            method_id_by_name_body_and_file_suffix(
                &db,
                "extract_with_state",
                "E::from_request(self, state)",
                "axum-core/src/ext_traits/request.rs",
            )?,
            &["E", "from_request"][..],
        ),
        (
            "axum-core/src/ext_traits/request.rs:305",
            method_id_by_name_body_and_file_suffix(
                &db,
                "extract_parts_with_state",
                "E::from_request_parts(&mut parts, state).await",
                "axum-core/src/ext_traits/request.rs",
            )?,
            &["E", "from_request_parts"][..],
        ),
        (
            "axum-core/src/ext_traits/request_parts.rs:133",
            method_id_by_name_body_and_file_suffix(
                &db,
                "extract_with_state",
                "E::from_request_parts(self, state)",
                "axum-core/src/ext_traits/request_parts.rs",
            )?,
            &["E", "from_request_parts"][..],
        ),
    ];
    for (label, owner, path) in cases {
        assert_owner_path_targetless(&db, owner, path, CallStatusKind::Unsupported, label)?;
    }

    assert_targetless_path_rows(&db, &["E", "from_request"], CallStatusKind::Unsupported, 1)?;
    assert_targetless_path_rows(
        &db,
        &["E", "from_request_parts"],
        CallStatusKind::Unsupported,
        2,
    )?;

    Ok(())
}

#[test]
fn axum_real_target_from_ref_paths_are_documented_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `FromRef::from_ref` bounded calls.
    // Source chain:
    //   axum-core/src/ext_traits/mod.rs:25 calls
    //   `InnerState::from_ref(state)`.
    //   axum-core/src/ext_traits/mod.rs:45 calls `String::from_ref(state)`.
    //   axum/src/extract/state.rs:309 calls `InnerState::from_ref(state)`.
    //   axum/src/middleware/from_extractor.rs:328 calls
    //   `Secret::from_ref(state)` from a test function.
    // Current model gap: these bounded associated path rows are visible but
    // unsupported, targetless, and must not traverse to a guessed blanket impl.
    let method_cases = [
        (
            "axum-core/src/ext_traits/mod.rs:25",
            method_id_by_name_body_and_file_suffix(
                &db,
                "from_request_parts",
                "InnerState::from_ref(state)",
                "axum-core/src/ext_traits/mod.rs",
            )?,
            &["InnerState", "from_ref"][..],
        ),
        (
            "axum-core/src/ext_traits/mod.rs:45",
            method_id_by_name_body_and_file_suffix(
                &db,
                "from_request_parts",
                "String::from_ref(state)",
                "axum-core/src/ext_traits/mod.rs",
            )?,
            &["String", "from_ref"][..],
        ),
        (
            "axum/src/extract/state.rs:309",
            method_id_by_name_body_and_file_suffix(
                &db,
                "from_request_parts",
                "InnerState::from_ref(state)",
                "axum/src/extract/state.rs",
            )?,
            &["InnerState", "from_ref"][..],
        ),
    ];
    for (label, owner, path) in method_cases {
        assert_owner_path_targetless(&db, owner, path, CallStatusKind::Unsupported, label)?;
    }

    let test_owner = function_id_by_name(&db, "test_from_extractor")?;
    assert_owner_path_targetless(
        &db,
        test_owner,
        &["Secret", "from_ref"],
        CallStatusKind::Unsupported,
        "axum/src/middleware/from_extractor.rs:328",
    )?;

    assert_targetless_path_rows(
        &db,
        &["InnerState", "from_ref"],
        CallStatusKind::Unsupported,
        2,
    )?;
    assert_targetless_path_rows(&db, &["String", "from_ref"], CallStatusKind::Unsupported, 1)?;
    assert_targetless_path_rows(&db, &["Secret", "from_ref"], CallStatusKind::Unsupported, 1)
}

#[test]
fn axum_real_target_self_accept_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `Self::accept(self).await` listener row.
    // Source chain:
    //   axum/src/serve/listener.rs:41 and :61 use `Self::accept`.
    // Current model gap: the visible `Self::accept` path row is unsupported and
    // targetless; it should not be treated as recursive trait dispatch.
    // The current fixture projects the line-41 owner only; line 61 remains part
    // of the same body-owner completeness gap.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "accept",
        "Self::accept(self).await",
        "axum/src/serve/listener.rs",
    )?;
    assert_owner_path_targetless(
        &db,
        owner,
        &["Self", "accept"],
        CallStatusKind::Unsupported,
        "axum/src/serve/listener.rs:41",
    )?;
    assert_targetless_path_rows(&db, &["Self", "accept"], CallStatusKind::Unsupported, 1)
}

#[test]
fn axum_real_target_header_value_from_static_external_paths_are_targetless() -> Result<(), DbError>
{
    let db = setup_axum_call_graph_db()?;

    // Matrix: const initializer and external `HeaderValue::from_static` rows.
    // Source chain:
    //   axum/src/routing/route.rs:202 includes
    //   `HeaderValue::from_static("0")` in a local const initializer.
    //   axum-core/src/response/into_response.rs:196,207,232,320,
    //   axum/src/json.rs:208,217, and axum/src/response/mod.rs:47 call the
    //   same external associated function from response conversion bodies.
    // Current DB contract: all projected rows stay external and targetless.
    // The local const initializer is still flattened under `set_content_length`
    // rather than owned by a `CallBodyOwnerId::Const`.
    let route_owner =
        function_id_by_name_in_module(&db, &["crate", "routing", "route"], "set_content_length")?;
    assert_owner_path_targetless(
        &db,
        route_owner,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        "axum/src/routing/route.rs:202",
    )?;

    let const_rows = db.raw_query(
        r#"?[site_id] :=
            *const { id: owner_id @ 'NOW' },
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Path",
                path: ["HeaderValue", "from_static"] @ 'NOW'
            }"#,
    )?;
    assert!(
        const_rows.rows.is_empty(),
        "HeaderValue::from_static should not be modeled as const-owned until const body ownership lands: {const_rows:#?}"
    );

    assert_targetless_path_rows(
        &db,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        8,
    )
}

#[test]
fn axum_real_target_trait_object_dispatch_rows_are_documented_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: trait-object dispatch rows.
    // Source chains:
    //   axum/src/error_handling/mod.rs:251 calls
    //   `self.project().future.poll(cx)` on
    //   `Pin<Box<dyn Future<...>>>` from error_handling/mod.rs:240.
    //   axum-core/src/body.rs:32 calls
    //   `<dyn std::any::Any>::downcast_mut::<Option<T>>(&mut k)`.
    // Current model gap: dyn Future dispatch is visible through the
    // method-call-result `poll` row and stays targetless; the qualified
    // `<dyn Any>::downcast_mut` syntax is not projected as a path row yet.
    assert_targetless_method_rows(
        &db,
        "poll",
        "MethodCallResult",
        Some(&["as_mut"]),
        CallStatusKind::Unsupported,
        4,
    )?;
    assert_no_path_rows(&db, &["dyn", "Any", "downcast_mut"])
}

#[test]
fn axum_real_target_blanket_via_parts_self_path_is_absent_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `FromRequest` ViaParts blanket inner call.
    // Source chain:
    //   axum-core/src/extract/mod.rs:103 calls
    //   `Self::from_request_parts(parts, state).await`.
    // Current model gap: this async blanket-impl body does not project a
    // `Self::from_request_parts` path row in the axum fixture yet.
    assert_no_path_rows(&db, &["Self", "from_request_parts"])
}

#[test]
fn axum_real_target_handler_macro_extraction_paths_are_absent_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: generated `Handler::call` extraction rows.
    // Source chain:
    //   axum/src/handler/mod.rs:242 calls
    //   `$ty::from_request_parts(&mut parts, &state).await`.
    //   handler/mod.rs:250 calls `$last::from_request(req, &state).await`.
    // Current model gap: these macro-template associated paths are not
    // projected as stable call_site rows in the axum fixture yet.
    assert_no_path_rows(&db, &["ty", "from_request_parts"])?;
    assert_no_path_rows(&db, &["last", "from_request"])
}
