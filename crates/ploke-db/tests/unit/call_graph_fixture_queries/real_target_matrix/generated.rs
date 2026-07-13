use std::collections::BTreeMap;

use cozo::{DataValue, UuidWrapper};
use uuid::Uuid;

use super::super::*;
use super::common::*;

#[test]
fn axum_real_target_define_rejection_generated_methods_resolve() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: bounded generated `define_rejection!` impl methods.
    // Source chain:
    //   axum-core/src/macros.rs:30-115 defines `__define_rejection!`.
    //   axum/src/extract/rejection.rs:42-48 invokes it for
    //   `MissingExtension(Error)`.
    //
    // Expected traversal:
    //   the generated `IntoResponse::into_response` owner calls
    //   `self.status()` and `self.body_text()`. Those rows resolve in one
    //   edge to generated inherent methods on the same `MissingExtension`
    //   self type. This proves the cross-impl `Self` fallback without
    //   modeling arbitrary macro expansion or composite rejection enum fanout.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "into_response",
        "rejection_type=MissingExtension",
        "axum/src/extract/rejection.rs",
    )?;
    let context = db.call_context_for_owner(owner)?;
    let status = row_by_method_receiver(&context, "status", &CallReceiver::SelfValue);
    let body = row_by_method_receiver(&context, "body_text", &CallReceiver::SelfValue);

    assert_generated_rejection_method(
        &db,
        owner,
        status,
        "status",
        "MissingExtension",
        "axum MissingExtension generated self.status()",
    )?;
    assert_generated_rejection_method(
        &db,
        owner,
        body,
        "body_text",
        "MissingExtension",
        "axum MissingExtension generated self.body_text()",
    )?;

    Ok(())
}

fn assert_generated_rejection_method(
    db: &Database,
    owner: Uuid,
    row: &ploke_db::CallContextRow,
    method: &str,
    ty: &str,
    label: &'static str,
) -> Result<(), DbError> {
    assert_eq!(
        row.targets.len(),
        1,
        "{label} should resolve to exactly one generated method target: {row:#?}"
    );
    let target = row.targets[0].target_id;
    assert_method_owner_type(db, target, method, ty, label)?;
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        db,
        TraversalExpectation {
            label,
            owner,
            target,
            site_id: row.site.id,
            expected_edge_count: 1,
        },
    )
}

fn assert_method_owner_type(
    db: &Database,
    method_id: Uuid,
    method: &str,
    ty: &str,
    label: &str,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert(
        "method_id".to_string(),
        DataValue::Uuid(UuidWrapper(method_id)),
    );
    params.insert("method".to_string(), DataValue::from(method));
    params.insert(
        "owner_path".to_string(),
        DataValue::List(vec![DataValue::from(ty)]),
    );

    let rows = db.raw_query_params(
        r#"?[id] :=
            id = $method_id,
            *method { id: $method_id, name: $method, owner_id: impl_id @ 'NOW' },
            *impl { id: impl_id, self_type: self_type_id @ 'NOW' },
            *named_type { type_id: self_type_id, path: $owner_path @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "{label} should target generated {ty}::{method}; rows: {:#?}",
        rows.rows
    );

    Ok(())
}
