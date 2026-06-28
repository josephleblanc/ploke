use std::collections::BTreeMap;

use cozo::DataValue;
use ploke_db::multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE};
use ploke_test_utils::{CORPUS_AXUM_CALL_GRAPH, FixtureDb, fresh_backup_fixture_db};
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
    setup_call_graph_db(&CORPUS_AXUM_CALL_GRAPH)
}

pub(super) fn setup_call_graph_db(fixture: &'static FixtureDb) -> Result<Database, DbError> {
    let db =
        fresh_backup_fixture_db(fixture).map_err(|err| DbError::QueryExecution(err.to_string()))?;
    assert!(
        db.has_call_graph_relations()?,
        "{} must be regenerated with populated call graph relations",
        fixture.id
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

pub(super) fn assert_owner_path_targetless(
    db: &Database,
    owner: Uuid,
    path_parts: &[&str],
    status: CallStatusKind,
    label: &str,
) -> Result<Uuid, DbError> {
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_kind_path(&context, CallSiteKind::Path, path_parts);
    assert_targetless_status(row, status);
    assert!(
        relations_for_site(db, row.site.id)?.rows.is_empty(),
        "{label} should not have raw call_relation targets"
    );
    Ok(row.site.id)
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

pub(super) fn method_id_by_name_body_and_file_suffix(
    db: &Database,
    name: &str,
    body_marker: &str,
    file_suffix: &str,
) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, body, file_path] :=
    *method {{ id, name: $name, body @ 'NOW' }},
    ancestor[id, mod_id],
    *module{{ id: mod_id @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db.raw_query_params(&script, params)?;
    let normalized_marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter_map(|row| {
            let body = match &row[1] {
                DataValue::Str(body) => body.as_str(),
                _ => return None,
            };
            let file_path = data_str(&row[2], "file_path");
            (body_key(body).contains(&normalized_marker) && file_path.ends_with(file_suffix))
                .then(|| row[0].clone())
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method named {name:?} in {file_suffix:?} whose body contains {body_marker:?}; rows: {:#?}",
        rows.rows
    );

    to_uuid(matching.iter().next().expect("one matching method"))
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

pub(super) fn assert_path_module_fanout(
    db: &Database,
    path_parts: &[&str],
    status: CallStatusKind,
    expected: &[(&[&str], usize)],
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("path".to_string(), path_value(path_parts));
    params.insert("status".to_string(), DataValue::from(format!("{status:?}")));

    let rows = db.raw_query_params(
        r#"?[module_path, count(site_id)] :=
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Path",
                path: $path @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Path",
                status_kind: $status @ 'NOW'
            },
            *function { id: owner_id, module_id @ 'NOW' },
            *module { id: module_id, path: module_path @ 'NOW' }
        :sort module_path"#,
        params,
    )?;

    let actual = rows
        .rows
        .iter()
        .map(|row| {
            let DataValue::List(parts) = &row[0] else {
                panic!("module path should be a list: {row:#?}");
            };
            let module_path = parts
                .iter()
                .map(|part| data_str(part, "module_path").to_string())
                .collect::<Vec<_>>();
            let DataValue::Num(cozo::Num::Int(count)) = &row[1] else {
                panic!("fanout count should be an integer: {row:#?}");
            };
            (module_path, *count as usize)
        })
        .collect::<BTreeMap<_, _>>();

    let expected = expected
        .iter()
        .map(|(module_path, count)| {
            (
                module_path
                    .iter()
                    .map(|part| (*part).to_string())
                    .collect::<Vec<_>>(),
                *count,
            )
        })
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        actual, expected,
        "unexpected {status:?} module fanout for {path_parts:?}"
    );

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

pub(super) fn assert_targetless_dynamic_rows_by_method_name(
    db: &Database,
    method: &str,
    expected_arg_counts: &[u32],
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), DataValue::from(method));

    let rows = db.raw_query_params(
        r#"?[site_id, arg_count, status_kind, resolution_kind] :=
            *method { id: owner_id, name: $method @ 'NOW' },
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Dynamic",
                arg_count @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Dynamic",
                status_kind,
                resolution_kind @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        expected_arg_counts.len(),
        "expected {} targetless dynamic rows owned by methods named {method:?}: {:#?}",
        expected_arg_counts.len(),
        rows.rows
    );

    let mut actual_arg_counts = Vec::new();
    for row in &rows.rows {
        assert_eq!(row[2], DataValue::from("Unsupported"));
        assert_eq!(row[3], DataValue::Null);
        let site_id = to_uuid(&row[0])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "dynamic row owned by {method:?} should not have call_relation targets"
        );
        let DataValue::Num(cozo::Num::Int(arg_count)) = &row[1] else {
            panic!("dynamic row arg_count should be numeric: {row:#?}");
        };
        actual_arg_counts.push(*arg_count as u32);
    }
    actual_arg_counts.sort_unstable();

    let mut expected = expected_arg_counts.to_vec();
    expected.sort_unstable();
    assert_eq!(
        actual_arg_counts, expected,
        "unexpected dynamic arg counts for methods named {method:?}"
    );

    Ok(())
}

pub(super) fn assert_no_dynamic_rows_by_method_name(
    db: &Database,
    method: &str,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), DataValue::from(method));

    let rows = db.raw_query_params(
        r#"?[site_id] :=
            *method { id: owner_id, name: $method @ 'NOW' },
            *call_site { id: site_id, owner_id, call_kind: "Dynamic" }"#,
        params,
    )?;
    assert!(
        rows.rows.is_empty(),
        "expected no dynamic rows owned by methods named {method:?}: {:#?}",
        rows.rows
    );

    Ok(())
}

pub(super) fn assert_no_dynamic_rows_by_function_names(
    db: &Database,
    function_names: &[&str],
) -> Result<(), DbError> {
    for function_name in function_names {
        let mut params = BTreeMap::new();
        params.insert("function".to_string(), DataValue::from(*function_name));

        let rows = db.raw_query_params(
            r#"?[site_id] :=
                *function { id: owner_id, name: $function @ 'NOW' },
                *call_site { id: site_id, owner_id, call_kind: "Dynamic" }"#,
            params,
        )?;
        assert!(
            rows.rows.is_empty(),
            "expected no dynamic rows owned by functions named {function_name:?}: {:#?}",
            rows.rows
        );
    }

    Ok(())
}

pub(super) fn assert_no_method_rows(db: &Database, method: &str) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), DataValue::from(method));

    let rows = db.raw_query_params(
        r#"?[site_id] :=
            *call_site {
                id: site_id,
                call_kind: "Method",
                method_name: $method @ 'NOW'
            }"#,
        params,
    )?;
    assert!(
        rows.rows.is_empty(),
        "expected no method rows for {method:?}: {:#?}",
        rows.rows
    );

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
