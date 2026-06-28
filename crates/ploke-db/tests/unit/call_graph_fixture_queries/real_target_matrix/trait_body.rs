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
    //   axum/src/extract/state.rs:309 and
    //   middleware/from_extractor.rs:328 call `InnerState::from_ref(state)`.
    // Current model gap: the bounded associated path rows are visible but
    // unsupported and targetless.
    assert_targetless_path_rows(
        &db,
        &["InnerState", "from_ref"],
        CallStatusKind::Unsupported,
        2,
    )
}

#[test]
fn axum_real_target_self_accept_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `Self::accept(self).await` listener row.
    // Source chain:
    //   axum/src/serve/listener.rs:41 and :61 use `Self::accept`.
    // Current model gap: the visible `Self::accept` path row is unsupported and
    // targetless; it should not be treated as recursive trait dispatch.
    assert_targetless_path_rows(&db, &["Self", "accept"], CallStatusKind::Unsupported, 1)
}

#[test]
fn axum_real_target_const_initializer_external_paths_are_targetless() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: const initializer call-owner rows.
    // Source chain:
    //   axum/src/extract/ws.rs:382,384 and routing/route.rs:202 include
    //   `HeaderValue::from_static(...)` in const/static-like initializers.
    // Current DB contract: these external path rows stay targetless and do not
    // become local traversal edges.
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
