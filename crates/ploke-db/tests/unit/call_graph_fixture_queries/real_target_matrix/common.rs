use std::collections::BTreeMap;

use cozo::DataValue;
use ploke_test_utils::{CORPUS_AXUM_CALL_GRAPH, fresh_backup_fixture_db};
use uuid::Uuid;

use super::super::*;

#[derive(Clone, Copy)]
pub(super) struct ExpectedCaller {
    pub(super) owner_name: &'static str,
    pub(super) path: &'static [&'static str],
}

#[derive(Clone, Copy)]
pub(super) struct TraversalExpectation {
    pub(super) label: &'static str,
    pub(super) owner: Uuid,
    pub(super) target: Uuid,
    pub(super) site_id: Uuid,
}

pub(super) fn setup_axum_call_graph_db() -> Result<Database, DbError> {
    let db = fresh_backup_fixture_db(&CORPUS_AXUM_CALL_GRAPH)
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;
    assert!(
        db.has_call_graph_relations()?,
        "corpus_axum_call_graph must be regenerated with populated call graph relations"
    );
    Ok(db)
}

pub(super) fn assert_one_edge_traversal(
    db: &Database,
    expected: TraversalExpectation,
) -> Result<(), DbError> {
    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(expected.owner),
        CallContextOptions {
            include_incoming_callers: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert_outgoing_candidate(&outgoing, expected.target, expected.site_id, expected.label);

    let incoming = db.expand_call_context(
        CallContextSeed::Target(expected.target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert_incoming_candidate(
        &incoming,
        expected.owner,
        expected.site_id,
        expected.target,
        expected.label,
    );

    Ok(())
}

pub(super) fn assert_external_targetless(row: &ploke_db::CallContextRow) {
    assert_targetless_status(row, CallStatusKind::External);
}

pub(super) fn assert_targetless_status(row: &ploke_db::CallContextRow, status: CallStatusKind) {
    assert_eq!(row.status.status, status);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "{status:?} row should not expose local traversal targets: {row:#?}"
    );
}

pub(super) fn method_id_by_name_and_body_substring(
    db: &Database,
    name: &str,
    body_marker: &str,
) -> Result<Uuid, DbError> {
    let matching = method_ids_by_name_and_body_substring(db, name, body_marker)?;
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method named {name:?} whose body contains {body_marker:?}"
    );
    Ok(matching[0])
}

pub(super) fn macro_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let rows = db.raw_query_params(
        r#"?[id] :=
            *macro { id, name: $name @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one macro node named {name:?}; rows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][0])
}

pub(super) fn method_ids_by_name_and_body_substring(
    db: &Database,
    name: &str,
    body_marker: &str,
) -> Result<Vec<Uuid>, DbError> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let rows = db.raw_query_params(
        r#"?[id, body] :=
            *method { id, name: $name, body @ 'NOW' }"#,
        params,
    )?;
    let normalized_marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter_map(|row| {
            let body = match &row[1] {
                DataValue::Str(body) => body.as_str(),
                _ => return None,
            };
            body_key(body)
                .contains(&normalized_marker)
                .then(|| row[0].clone())
        })
        .collect::<Vec<_>>();

    matching.iter().map(to_uuid).collect()
}

fn body_key(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}

pub(super) fn assert_targetless_path_rows(
    db: &Database,
    path_parts: &[&str],
    status: CallStatusKind,
    expected_count: usize,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("path".to_string(), path_value(path_parts));
    params.insert("status".to_string(), DataValue::from(format!("{status:?}")));

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
        assert_eq!(row[2], DataValue::Null);
        let site_id = to_uuid(&row[0])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "{path_parts:?} row should not have call_relation targets"
        );
    }

    Ok(())
}

pub(super) fn assert_no_path_rows(db: &Database, path_parts: &[&str]) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("path".to_string(), path_value(path_parts));

    let rows = db.raw_query_params(
        r#"?[site_id] :=
            *call_site {
                id: site_id,
                call_kind: "Path",
                path: $path @ 'NOW'
            }"#,
        params,
    )?;
    assert!(
        rows.rows.is_empty(),
        "expected no path rows for {path_parts:?}: {:#?}",
        rows.rows
    );

    Ok(())
}

pub(super) fn assert_targetless_method_rows(
    db: &Database,
    method: &str,
    receiver_kind: &str,
    receiver_path: Option<&[&str]>,
    status: CallStatusKind,
    expected_count: usize,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), DataValue::from(method));
    params.insert("receiver_kind".to_string(), DataValue::from(receiver_kind));
    params.insert("status".to_string(), DataValue::from(format!("{status:?}")));
    params.insert(
        "receiver_path".to_string(),
        receiver_path.map_or(DataValue::Null, path_value),
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
        assert_eq!(row[2], DataValue::Null);
        let site_id = to_uuid(&row[0])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "{method}.{receiver_path:?} row should not have call_relation targets"
        );
    }

    Ok(())
}

fn path_value(path_parts: &[&str]) -> DataValue {
    DataValue::List(
        path_parts
            .iter()
            .map(|part| DataValue::from(*part))
            .collect(),
    )
}
