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

fn assert_targetless_path_rows(
    db: &Database,
    path_parts: &[&str],
    status: CallStatusKind,
    expected_count: usize,
) -> Result<(), DbError> {
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "path".to_string(),
        cozo::DataValue::List(
            path_parts
                .iter()
                .map(|part| cozo::DataValue::from(*part))
                .collect(),
        ),
    );
    params.insert(
        "status".to_string(),
        cozo::DataValue::from(format!("{status:?}")),
    );

    let rows = db.raw_query_params(
        r#"?[site_id, owner_id, resolution_kind] :=
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Path",
                path: $path @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Path",
                status_kind: $status,
                resolution_kind @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        expected_count,
        "expected {expected_count} {status:?} targetless path rows for {path_parts:?}: {:#?}",
        rows.rows
    );
    for row in &rows.rows {
        assert_eq!(row[2], cozo::DataValue::Null);
        let site_id = to_uuid(&row[0])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "{path_parts:?} row should not have call_relation targets"
        );
    }

    Ok(())
}
