use super::super::*;
use super::common::*;

#[test]
fn axum_core_extract_self_methods_reach_same_impl_methods() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Ground truth:
    //   axum-core/src/ext_traits/request.rs:268 self.extract_with_state(&())
    //   axum-core/src/ext_traits/request_parts.rs:122 self.extract_with_state(&())
    let owners =
        method_ids_by_name_and_body_substring(&db, "extract", "self.extract_with_state(&())")?;
    assert_eq!(
        owners.len(),
        2,
        "axum should expose both RequestExt and RequestPartsExt extract methods"
    );

    let request_target = method_id_by_name_and_body_substring(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
    )?;
    let parts_target = method_id_by_name_and_body_substring(
        &db,
        "extract_with_state",
        "E::from_request_parts(self, state)",
    )?;
    let expected_targets = [request_target, parts_target];

    for owner in owners {
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_method_receiver(&context, "extract_with_state", &CallReceiver::SelfValue);
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert!(
            expected_targets.contains(&row.targets[0].target_id),
            "self.extract_with_state should resolve to one of the same-impl extract_with_state methods: {row:#?}"
        );
        assert_eq!(row.targets[0].relation, CallRelationKind::Method);
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
        assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
        assert_one_edge_traversal(
            &db,
            TraversalExpectation {
                label: "extract -> extract_with_state",
                owner,
                target: row.targets[0].target_id,
                site_id: row.site.id,
            },
        )?;
    }

    for target in expected_targets {
        let callers = db.callers_for_target(target)?;
        assert_eq!(
            callers.len(),
            1,
            "each extract_with_state impl should have exactly one inspected self-method caller"
        );
        assert_eq!(callers[0].status.status, CallStatusKind::Resolved);
        assert_eq!(callers[0].target.target_id, target);
    }

    Ok(())
}

#[test]
fn axum_real_target_request_extensions_mut_receivers_are_documented_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: local and parameter `req.extensions_mut` receiver rows.
    // Source chain:
    //   axum-core/src/ext_traits/request.rs:302 calls `req.extensions_mut()`.
    //   axum/src/extension.rs:184 calls `req.extensions_mut()`.
    // Current model gap: both are projected as method callsites on a local
    // binding named `req`, but they remain unresolved external receiver calls.
    assert_targetless_method_rows(
        &db,
        "extensions_mut",
        "LocalBinding",
        &["req"],
        CallStatusKind::Unresolved,
        6,
    )?;

    Ok(())
}

#[test]
fn axum_real_target_self_field_size_hint_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `self.0.size_hint` receiver row.
    // Source chain:
    //   axum-core/src/body.rs:127 calls `self.0.size_hint()`.
    // Current model gap: the tuple-field receiver shape is visible but remains
    // unsupported and targetless.
    let owner = method_id_by_name_and_body_substring(&db, "size_hint", "self.0.size_hint()")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_method_receiver(
        &context,
        "size_hint",
        &CallReceiver::SelfField {
            path: vec!["0".to_string()],
        },
    );
    assert_targetless_status(row, CallStatusKind::Unsupported);

    Ok(())
}

#[test]
fn axum_real_target_turbofish_local_receiver_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: turbofish method call receiver row.
    // Source chain:
    //   axum-core/src/ext_traits/request_parts.rs:164 calls
    //   `parts.extract_with_state::<String, _>(&state)`.
    // Current model gap: the local receiver row is projected, but it does not
    // resolve back to the `RequestPartsExt::extract_with_state` impl yet.
    let _target_owner = method_id_by_name_and_body_substring(
        &db,
        "extract_with_state",
        "E::from_request_parts(self, state)",
    )?;
    assert_targetless_method_rows(
        &db,
        "extract_with_state",
        "LocalBinding",
        &["parts"],
        CallStatusKind::Unresolved,
        1,
    )?;

    Ok(())
}

fn assert_targetless_method_rows(
    db: &Database,
    method: &str,
    receiver_kind: &str,
    receiver_path: &[&str],
    status: CallStatusKind,
    expected_count: usize,
) -> Result<(), DbError> {
    let mut params = std::collections::BTreeMap::new();
    params.insert("method".to_string(), cozo::DataValue::from(method));
    params.insert(
        "receiver_kind".to_string(),
        cozo::DataValue::from(receiver_kind),
    );
    params.insert(
        "status".to_string(),
        cozo::DataValue::from(format!("{status:?}")),
    );
    params.insert(
        "receiver_path".to_string(),
        cozo::DataValue::List(
            receiver_path
                .iter()
                .map(|part| cozo::DataValue::from(*part))
                .collect(),
        ),
    );

    let rows = db.raw_query_params(
        r#"?[site_id, owner_id, resolution_kind] :=
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Method",
                method_name: $method,
                receiver_kind: $receiver_kind,
                receiver_path: $receiver_path @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Method",
                status_kind: $status,
                resolution_kind @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        expected_count,
        "expected {expected_count} {status:?} targetless method rows for {method}.{receiver_path:?}: {:#?}",
        rows.rows
    );
    for row in &rows.rows {
        assert_eq!(row[2], cozo::DataValue::Null);
        let site_id = to_uuid(&row[0])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "{method}.{receiver_path:?} row should not have call_relation targets"
        );
    }

    Ok(())
}
