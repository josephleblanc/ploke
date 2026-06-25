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
mod proof;

pub(super) use constructor::*;
pub(super) use dynamic::*;
pub(super) use proof::*;

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

pub(super) fn function_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *function {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn function_id_by_exact_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    exactly_one_uuid_params(
        db,
        r#"?[id] :=
            *function { id, name: $name @ 'NOW' }"#,
        params,
        0,
    )
}

pub(super) fn method_id_by_impl_self_type_name(
    db: &Database,
    self_type_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *type_relation {{
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }},
                *struct {{ id: self_target_id, name: "{self_type_name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn method_owner_is_inherent_impl(
    db: &Database,
    method_id: Uuid,
    self_type_name: &str,
) -> Result<bool, DbError> {
    let mut params = BTreeMap::new();
    params.insert(
        "method_id".to_string(),
        DataValue::Uuid(UuidWrapper(method_id)),
    );
    params.insert(
        "self_type_name".to_string(),
        DataValue::from(self_type_name),
    );

    let self_rows = db.raw_query_params(
        r#"?[impl_id] :=
            *method { id: $method_id, owner_id: impl_id @ 'NOW' },
            *impl { id: impl_id @ 'NOW' },
            *type_use {
                owner_id: impl_id,
                root_type_id: self_type_id,
                role: "ImplSelf" @ 'NOW'
            },
            *type_relation {
                source_id: self_type_id,
                target_id: self_target_id,
                relation_kind: "Ordinary" @ 'NOW'
            },
            *struct { id: self_target_id, name: $self_type_name @ 'NOW' }"#,
        params.clone(),
    )?;
    let trait_rows = db.raw_query_params(
        r#"?[trait_type_id] :=
            *method { id: $method_id, owner_id: impl_id @ 'NOW' },
            *type_use {
                owner_id: impl_id,
                root_type_id: trait_type_id,
                role: "ImplTrait" @ 'NOW'
            }"#,
        params,
    )?;

    Ok(self_rows.rows.len() == 1 && trait_rows.rows.is_empty())
}

pub(super) fn method_id_by_impl_self_type_exact_name(
    db: &Database,
    self_type_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert(
        "self_type_name".to_string(),
        DataValue::from(self_type_name),
    );
    params.insert("method_name".to_string(), DataValue::from(method_name));
    exactly_one_uuid_params(
        db,
        r#"?[method_id] :=
            *method { id: method_id, name: $method_name, owner_id: impl_id @ 'NOW' },
            *impl { id: impl_id @ 'NOW' },
            *type_use {
                owner_id: impl_id,
                root_type_id: self_type_id,
                role: "ImplSelf" @ 'NOW'
            },
            *type_relation {
                source_id: self_type_id,
                target_id: self_target_id,
                relation_kind: "Ordinary" @ 'NOW'
            },
            *struct { id: self_target_id, name: $self_type_name @ 'NOW' }"#,
        params,
        0,
    )
}

pub(super) fn method_id_by_impl_trait_and_self_type_names(
    db: &Database,
    trait_name: &str,
    self_type_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *type_relation {{
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }},
                *struct {{ id: self_target_id, name: "{self_type_name}" @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: trait_type_id,
                    role: "ImplTrait" @ 'NOW'
                }},
                *type_relation {{
                    source_id: trait_type_id,
                    target_id: trait_target_id,
                    relation_kind: "Trait" @ 'NOW'
                }},
                *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn method_id_by_impl_trait_name(
    db: &Database,
    trait_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: trait_type_id,
                    role: "ImplTrait" @ 'NOW'
                }},
                *type_relation {{
                    source_id: trait_type_id,
                    target_id: trait_target_id,
                    relation_kind: "Trait" @ 'NOW'
                }},
                *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn method_id_by_trait_name(
    db: &Database,
    trait_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: trait_id @ 'NOW' }},
                *trait {{ id: trait_id, name: "{trait_name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn function_id_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
) -> Result<Uuid, DbError> {
    let module_path = cozo_path_literal(module_path);
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *function {{ id, name: "{name}", module_id @ 'NOW' }},
                *module {{ id: module_id, path: {module_path} @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn struct_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *struct {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn variant_id_by_enum_and_variant_names(
    db: &Database,
    enum_name: &str,
    variant_name: &str,
) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert("enum_name".to_string(), DataValue::from(enum_name));
    params.insert("variant_name".to_string(), DataValue::from(variant_name));
    exactly_one_uuid_params(
        db,
        r#"?[id] :=
            *enum { id: enum_id, name: $enum_name @ 'NOW' },
            *variant { id, name: $variant_name, owner_id: enum_id @ 'NOW' }"#,
        params,
        0,
    )
}

pub(super) fn const_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *const {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn static_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *static {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

fn exactly_one_uuid(db: &Database, script: &str, column: usize) -> Result<Uuid, DbError> {
    let rows = db.raw_query(script)?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one row for query:\n{script}\nrows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][column])
}

fn exactly_one_uuid_params(
    db: &Database,
    script: &str,
    params: BTreeMap<String, DataValue>,
    column: usize,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query_params(script, params)?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one row for query:\n{script}\nrows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][column])
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

pub(super) fn row_by_path<'a>(
    context: &'a [CallContextRow],
    expected: &[&str],
) -> &'a CallContextRow {
    let expected = path(expected);
    let matches = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Path && row.site.path.as_ref() == Some(&expected)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one path row {expected:?}; context rows: {context:#?}"
    );
    matches[0]
}

pub(super) fn row_by_method_receiver<'a>(
    context: &'a [CallContextRow],
    method: &str,
    receiver: &CallReceiver,
) -> &'a CallContextRow {
    let matches = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Method
                && row.site.method.as_deref() == Some(method)
                && row.site.receiver.as_ref() == Some(receiver)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one method row {method} with receiver {receiver:?}; context rows: {context:#?}"
    );
    matches[0]
}

pub(super) fn row_by_kind_path<'a>(
    context: &'a [CallContextRow],
    kind: CallSiteKind,
    expected: &[&str],
) -> &'a CallContextRow {
    let expected = path(expected);
    let matches = context
        .iter()
        .filter(|row| row.site.kind == kind && row.site.path.as_ref() == Some(&expected))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one {kind:?} row {expected:?}; context rows: {context:#?}"
    );
    matches[0]
}

pub(super) fn caller_by_owner_kind_path<'a>(
    callers: &'a [CallCallerRow],
    owner: Uuid,
    kind: CallSiteKind,
    expected: &[&str],
) -> &'a CallCallerRow {
    let expected = path(expected);
    let matches = callers
        .iter()
        .filter(|row| {
            row.site.owner_id == owner
                && row.site.kind == kind
                && row.site.path.as_ref() == Some(&expected)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one incoming {kind:?} caller row {expected:?}; caller rows: {callers:#?}"
    );
    matches[0]
}

pub(super) fn caller_by_owner_method_receiver<'a>(
    callers: &'a [CallCallerRow],
    owner: Uuid,
    method: &str,
    receiver: &CallReceiver,
) -> &'a CallCallerRow {
    let matches = callers
        .iter()
        .filter(|row| {
            row.site.owner_id == owner
                && row.site.kind == CallSiteKind::Method
                && row.site.method.as_deref() == Some(method)
                && row.site.receiver.as_ref() == Some(receiver)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one incoming method caller {method} with receiver {receiver:?}; caller rows: {callers:#?}"
    );
    matches[0]
}

pub(super) fn assert_resolved_target(
    row: &CallContextRow,
    target: Uuid,
    relation: CallRelationKind,
    source: CallSiteKind,
    target_kind: CallTargetKind,
) {
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(row.targets[0].relation, relation);
    assert_eq!(row.targets[0].source_kind, source);
    assert_eq!(row.targets[0].target_kind, target_kind);
}

pub(super) fn assert_call_candidate(
    candidates: &[CallContextCandidate],
    node_id: Uuid,
    relation: CallContextRelation,
    call_site_id: Uuid,
    target_id: Uuid,
    message: &str,
) {
    assert!(
        candidates.iter().any(|candidate| {
            candidate.node_id == node_id
                && candidate.relation == relation
                && candidate.call_site_id == call_site_id
                && candidate.target_id == target_id
                && candidate.distance == 1
        }),
        "{message}; candidates: {candidates:#?}"
    );
}

pub(super) fn path(segments: &[&str]) -> Vec<String> {
    segments
        .iter()
        .map(|segment| (*segment).to_string())
        .collect()
}

fn cozo_path_literal(segments: &[&str]) -> String {
    let joined = segments
        .iter()
        .map(|segment| format!(r#""{segment}""#))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{joined}]")
}
