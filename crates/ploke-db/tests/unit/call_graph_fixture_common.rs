use std::collections::BTreeMap;

use cozo::{DataValue, Db, MemStorage, UuidWrapper};
use ploke_db::{
    CallCallerRow, CallContextCandidate, CallContextRelation, CallContextRow, CallReceiver,
    CallRelationKind, CallResolutionKind, CallSiteKind, CallStatusKind, CallTargetKind, Database,
    DbError, ProofGraphContextRow, ProofGraphStore, QueryResult, to_uuid,
};
use ploke_transform::{schema::create_schema_all, transform::transform_parsed_graph};
use uuid::Uuid;

mod constructor;
mod dynamic;
mod lookup;
mod proof;
mod selectors;

pub(super) use constructor::*;
pub(super) use dynamic::*;
pub(super) use lookup::*;
pub(super) use proof::*;
pub(super) use selectors::*;

pub(super) fn setup_call_graph_fixture_db(fixture: &'static str) -> Result<Database, DbError> {
    let db = Db::new(MemStorage::default()).expect("in-memory cozo db");
    db.initialize().expect("initialize cozo db");
    create_schema_all(&db).map_err(|err| DbError::QueryExecution(err.to_string()))?;

    let mut merged = syn_parser::parser::ParsedCodeGraph::merge_new(
        ploke_test_utils::test_run_phases_and_collect(fixture),
    )
    .map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let tree = merged
        .build_tree_and_prune()
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;
    transform_parsed_graph(&db, merged, &tree)
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;

    Ok(Database::new(db))
}

pub(super) fn data_str<'a>(value: &'a DataValue, label: &str) -> &'a str {
    value
        .get_str()
        .unwrap_or_else(|| panic!("{label} should be a string, got {value:?}"))
}

pub(super) fn optional_data_str<'a>(value: &'a DataValue, label: &str) -> Option<&'a str> {
    match value {
        DataValue::Null => None,
        other => Some(data_str(other, label)),
    }
}

pub(super) fn body_edges_for_site(db: &Database, site_id: Uuid) -> Result<QueryResult, DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));
    db.raw_query_params(
        r#"?[source_id, target_id, source_kind, target_kind] :=
            site_id = $site_id,
            *call_site_edge {
                source_id,
                target_id,
                relation_kind: "BodyContainsCall",
                source_kind,
                target_kind @ 'NOW'
            },
            target_id = site_id"#,
        params,
    )
}

pub(super) fn statuses_for_site(db: &Database, site_id: Uuid) -> Result<QueryResult, DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));
    db.raw_query_params(
        r#"?[source_kind, status_kind, resolution_kind] :=
            site_id = $site_id,
            *call_resolution_status {
                source_id,
                source_kind,
                status_kind,
                resolution_kind @ 'NOW'
            },
            source_id = site_id"#,
        params,
    )
}

pub(super) fn relations_for_site(db: &Database, site_id: Uuid) -> Result<QueryResult, DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));
    db.raw_query_params(
        r#"?[target_id, relation_kind, source_kind, target_kind] :=
            site_id = $site_id,
            *call_relation {
                source_id,
                target_id,
                relation_kind,
                source_kind,
                target_kind @ 'NOW'
            },
            source_id = site_id"#,
        params,
    )
}

pub(super) fn call_site_owner_and_kind(
    db: &Database,
    site_id: Uuid,
) -> Result<(Uuid, String), DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));
    let rows = db.raw_query_params(
        r#"?[owner_id, call_kind] :=
            site_id = $site_id,
            *call_site { id, owner_id, call_kind @ 'NOW' },
            id = site_id"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one call_site row for {site_id}; rows: {:#?}",
        rows.rows
    );
    Ok((
        to_uuid(&rows.rows[0][0])?,
        data_str(&rows.rows[0][1], "call_site.call_kind").to_string(),
    ))
}

pub(super) fn call_site_kind_for_site(db: &Database, site_id: Uuid) -> Result<String, DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));
    let rows = db.raw_query_params(
        r#"?[call_kind] :=
            site_id = $site_id,
            *call_site { id, call_kind @ 'NOW' },
            id = site_id"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "call_relation source should point to exactly one call_site {site_id}; rows: {:#?}",
        rows.rows
    );
    Ok(data_str(&rows.rows[0][0], "call_site.call_kind").to_string())
}

pub(super) fn owner_kind_for_call_body_owner(
    db: &Database,
    owner: Uuid,
) -> Result<String, DbError> {
    let mut params = BTreeMap::new();
    params.insert("owner".to_string(), DataValue::Uuid(UuidWrapper(owner)));
    let rows = db.raw_query_params(
        r#"?[kind] :=
            owner = $owner,
            (
                *function { id: owner @ 'NOW' },
                kind = "Function"
            ) or (
                *method { id: owner @ 'NOW' },
                kind = "Method"
            ) or (
                *const { id: owner @ 'NOW' },
                kind = "Const"
            ) or (
                *static { id: owner @ 'NOW' },
                kind = "Static"
            )"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "call-site owner should be exactly one call body owner {owner}; rows: {:#?}",
        rows.rows
    );
    Ok(data_str(&rows.rows[0][0], "call body owner kind").to_string())
}

pub(super) fn is_valid_call_relation_family(relation: &str, source: &str, target: &str) -> bool {
    matches!(
        (relation, source, target),
        ("Function", "Path", "Function")
            | ("DynamicFunction", "Dynamic", "Function")
            | ("Method", "Method", "Method")
            | ("AssociatedFunction", "Path", "Method")
            | ("TupleStructConstructor", "Path", "Struct")
            | ("EnumVariantConstructor", "Path", "Variant")
    )
}

pub(super) fn call_target_exists(db: &Database, target: Uuid, kind: &str) -> Result<bool, DbError> {
    let relation = match kind {
        "Function" => "function",
        "Method" => "method",
        "Struct" => "struct",
        "Variant" => "variant",
        other => panic!("unexpected call relation target kind {other}"),
    };
    let rows = db.raw_query(&format!(
        r#"?[id] :=
            id = to_uuid("{target}"),
            *{relation} {{ id @ 'NOW' }}"#
    ))?;
    Ok(rows.rows.len() == 1)
}

pub(super) fn assert_valid_status_shape(site_id: Uuid, status: &str, resolution: Option<&str>) {
    let valid = match status {
        "Resolved" => resolution == Some("LocalExact"),
        "Unresolved" | "Ambiguous" | "External" | "Unsupported" => resolution.is_none(),
        other => panic!("unexpected call status kind {other} for {site_id}"),
    };
    assert!(
        valid,
        "call_resolution_status resolution_kind {resolution:?} is invalid for {status} call site {site_id}"
    );
}

pub(super) fn proof_kind_count(rows: &[&ProofGraphContextRow], kind: &str) -> usize {
    rows.iter().filter(|row| row.kind == kind).count()
}

pub(super) fn proof_fact_for_kind<'a>(
    rows: &'a [&ProofGraphContextRow],
    kind: &str,
) -> &'a ProofGraphContextRow {
    let matches = rows
        .iter()
        .copied()
        .filter(|row| row.kind == kind)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one {kind} proof fact; rows: {rows:#?}"
    );
    matches[0]
}
