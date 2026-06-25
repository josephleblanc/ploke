use std::collections::BTreeMap;

use cozo::{DataValue, Db, MemStorage, UuidWrapper};
use ploke_db::{
    CallCallerRow, CallContextCandidate, CallContextRelation, CallContextRow, CallReceiver,
    CallRelationKind, CallResolutionKind, CallSiteKind, CallStatusKind, CallTargetKind, Database,
    DbError, ProofGraphContextRow, ProofGraphStore, QueryResult, to_uuid,
};
use ploke_transform::{schema::create_schema_all, transform::transform_parsed_graph};
use uuid::Uuid;

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

pub(super) const AMBIGUOUS_DYNAMIC_OWNERS: [&str; 2] = [
    "call_if_ambiguous_function_item",
    "call_match_ambiguous_function_item",
];

pub(super) fn dynamic_candidates(db: &Database) -> Result<Vec<Uuid>, DbError> {
    let mut expected = vec![
        function_id_by_name(db, "local_target")?,
        function_id_by_name(db, "other_target")?,
    ];
    expected.sort_unstable();
    Ok(expected)
}

pub(super) fn candidate_strings(candidates: &[Uuid]) -> Vec<String> {
    let mut expected = candidates
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    expected.sort();
    expected
}

pub(super) fn assert_dynamic_candidates(
    row: &CallContextRow,
    owner: Uuid,
    expected: &[Uuid],
    label: &str,
) {
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Dynamic);
    assert_eq!(row.site.path, None);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(row.status.status, CallStatusKind::Ambiguous);
    assert_eq!(row.status.resolution, None);
    assert_eq!(
        row.targets.len(),
        expected.len(),
        "{label} targets: {row:#?}"
    );
    assert!(row.targets.iter().all(|target| {
        target.relation == CallRelationKind::DynamicFunction
            && target.source_kind == CallSiteKind::Dynamic
            && target.target_kind == CallTargetKind::Function
    }));

    let mut actual = row
        .targets
        .iter()
        .map(|target| target.target_id)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(
        actual, expected,
        "{label} should expose proven ambiguous dynamic candidates"
    );
}

pub(super) fn assert_candidate_proof(
    facts: &[serde_json::Value],
    site: &str,
    expected: &[String],
    label: &str,
) {
    assert_eq!(facts.len(), 2, "{label} proof facts: {facts:#?}");

    let resolution = facts
        .iter()
        .find(|fact| {
            fact.get("fact_kind").and_then(serde_json::Value::as_str) == Some("call_resolution")
                && fact.get("call_site_id").and_then(serde_json::Value::as_str) == Some(site)
        })
        .unwrap_or_else(|| panic!("{label} should project call_resolution proof fact"));
    assert_eq!(
        resolution
            .get("blocking_reason")
            .and_then(serde_json::Value::as_str),
        Some("type_resolution_missing")
    );
    let mut actual = resolution
        .get("candidate_def_ids")
        .and_then(serde_json::Value::as_array)
        .expect("ambiguous dynamic resolution should carry candidate_def_ids")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("candidate_def_ids should contain string IDs")
                .to_string()
        })
        .collect::<Vec<_>>();
    actual.sort();
    assert_eq!(actual, expected, "{label} candidate_def_ids");
}

pub(super) fn assert_candidate_blocker(
    db: &Database,
    site: &str,
    label: &str,
) -> Result<(), DbError> {
    let rows = db.proof_graphrag_context("type_resolution_missing")?;
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site)
                && proof.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "{label} projected ambiguous blocker proof rows: {rows:#?}"
    );
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct OwnerProofEdge {
    pub(super) owner: Uuid,
    pub(super) site: Uuid,
    pub(super) span: (u32, u32),
    pub(super) target: Uuid,
}

#[derive(Clone, Copy)]
pub(super) enum ProofEdgeCount {
    Exact,
    AtLeast,
}

#[derive(Clone, Copy)]
pub(super) struct TargetProofSite {
    pub(super) owner: Uuid,
    pub(super) site: Uuid,
}

#[derive(Clone, Copy)]
pub(super) struct BlockerProofSite {
    pub(super) site: Uuid,
    pub(super) span: (u32, u32),
    pub(super) blocker_reason: &'static str,
}

pub(super) fn assert_owner_proof_edges(
    db: &Database,
    label: &str,
    expected: &[OwnerProofEdge],
    source_suffix: &str,
    blocker_reason: &str,
    count: ProofEdgeCount,
) -> Result<(), DbError> {
    let edges = db.proof_checker_edges()?;
    match count {
        ProofEdgeCount::Exact => assert_eq!(
            edges.len(),
            expected.len(),
            "{label} proof checker edges: {edges:#?}"
        ),
        ProofEdgeCount::AtLeast => assert!(
            edges.len() >= expected.len(),
            "{label} proof checker edges should include at least the expected edges: {edges:#?}"
        ),
    }

    for expected in expected {
        let owner = expected.owner.to_string();
        let site = expected.site.to_string();
        let target = expected.target.to_string();
        assert!(
            edges.iter().any(|edge| {
                edge.call_site_id == site
                    && edge.caller_def_id == owner
                    && edge.callee_def_id.as_deref() == Some(target.as_str())
                    && edge.resolution_state == "resolved"
                    && edge.blocker_reason.is_none()
            }),
            "{label} proof edge missing for {site}: {edges:#?}"
        );

        let provenance = db
            .proof_source_provenance(&site)?
            .unwrap_or_else(|| panic!("projected fixture {label} call-site source provenance"));
        assert!(
            provenance.source_file.ends_with(source_suffix),
            "source provenance for {site}: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, expected.span.0);
        assert_eq!(provenance.end_byte, expected.span.1);
    }

    assert!(
        db.proof_graphrag_context(blocker_reason)?.is_empty(),
        "resolved {label} proofs should not produce {blocker_reason} blockers"
    );

    Ok(())
}

pub(super) fn assert_targetless_blocker_proofs(
    db: &Database,
    label: &str,
    expected: &[BlockerProofSite],
    source_suffix: &str,
) -> Result<(), DbError> {
    assert!(
        db.proof_checker_edges()?.is_empty(),
        "{label} targetless blockers must not fabricate proof edges"
    );

    assert_blocker_proofs(db, label, expected, source_suffix)
}

pub(super) fn assert_blocker_proofs(
    db: &Database,
    label: &str,
    expected: &[BlockerProofSite],
    source_suffix: &str,
) -> Result<(), DbError> {
    for expected in expected {
        let site = expected.site.to_string();
        let rows = db.proof_graphrag_context(expected.blocker_reason)?;
        assert!(
            rows.iter().any(|row| {
                row.kind == "call_resolution"
                    && row.call_site_id.as_deref() == Some(site.as_str())
                    && row.blocker_reason.as_deref() == Some(expected.blocker_reason)
            }),
            "{label} proof rows missing {site}: {rows:#?}"
        );

        let provenance = db
            .proof_source_provenance(&site)?
            .unwrap_or_else(|| panic!("projected fixture {label} call-site source provenance"));
        assert!(
            provenance.source_file.ends_with(source_suffix),
            "source provenance for {site}: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, expected.span.0);
        assert_eq!(provenance.end_byte, expected.span.1);
    }

    Ok(())
}

pub(super) fn assert_resolved_target_callers(
    callers: &[CallCallerRow],
    target: Uuid,
    min_count: usize,
    label: &str,
) -> Result<(), DbError> {
    assert!(
        callers.len() >= min_count,
        "{label} should have at least {min_count} incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "{label} setup returned mismatched target rows: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.status.status == CallStatusKind::Resolved
                && caller.status.resolution == Some(CallResolutionKind::LocalExact)),
        "{label} setup should only include resolved local callers: {callers:#?}"
    );
    Ok(())
}

pub(super) fn expected_target_proof_count(callers: &[CallCallerRow]) -> usize {
    callers
        .iter()
        .map(|caller| {
            if caller.status.status == CallStatusKind::Resolved {
                3
            } else {
                2
            }
        })
        .sum()
}

pub(super) fn assert_target_proof_projection(
    db: &Database,
    label: &str,
    domain: &str,
    target: Uuid,
    callers: &[CallCallerRow],
    expected: &[TargetProofSite],
    source_suffix: &str,
    blocker_reason: &str,
) -> Result<(), DbError> {
    let count = db.project_call_proof_facts_for_target(target, domain)?;
    let resolved_count = callers
        .iter()
        .filter(|caller| caller.status.status == CallStatusKind::Resolved)
        .count();
    let expected_count = expected_target_proof_count(callers);
    assert_eq!(count, expected_count, "{label} target-centered proof count");

    let target_str = target.to_string();
    let edges = db.proof_checker_edges()?;
    assert_eq!(
        edges.len(),
        resolved_count,
        "{label} target-centered proof edges: {edges:#?}"
    );
    assert!(
        edges.iter().all(
            |edge| edge.callee_def_id.as_deref() == Some(target_str.as_str())
                && edge.resolution_state == "resolved"
                && edge.blocker_reason.is_none()
        ),
        "{label} target-centered proof edges should all resolve to the seed target: {edges:#?}"
    );

    for site in expected {
        let owner = site.owner.to_string();
        let call_site = site.site.to_string();
        assert!(
            edges
                .iter()
                .any(|edge| { edge.call_site_id == call_site && edge.caller_def_id == owner }),
            "{label} incoming proof edge missing for {call_site}: {edges:#?}"
        );
    }

    assert!(
        db.proof_graphrag_context(blocker_reason)?.is_empty(),
        "{label} projection should not include unrelated {blocker_reason} blockers"
    );

    for site in expected {
        let provenance = db
            .proof_source_provenance(&site.site.to_string())?
            .unwrap_or_else(|| panic!("projected {label} source provenance"));
        assert!(
            provenance.source_file.ends_with(source_suffix),
            "source provenance: {provenance:#?}"
        );
        assert!(provenance.start_byte < provenance.end_byte);
    }

    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct ConstructorCase {
    pub(super) label: &'static str,
    pub(super) fixture: &'static str,
    pub(super) domain: &'static str,
    pub(super) owner_module: &'static [&'static str],
    pub(super) owner: &'static str,
    pub(super) path: &'static [&'static str],
    pub(super) target: ConstructorTarget,
    pub(super) relation: CallRelationKind,
    pub(super) endpoint: CallTargetKind,
    pub(super) source_suffix: &'static str,
}

#[derive(Clone, Copy)]
pub(super) enum ConstructorTarget {
    Struct {
        name: &'static str,
    },
    Variant {
        enum_name: &'static str,
        variant_name: &'static str,
    },
}

pub(super) struct ResolvedConstructor {
    pub(super) owner: Uuid,
    pub(super) target: Uuid,
    pub(super) site: Uuid,
    pub(super) span: (u32, u32),
    pub(super) proof_count: usize,
    pub(super) owner_str: String,
    pub(super) target_str: String,
    pub(super) site_str: String,
}

const CONSTRUCTOR_CASES: &[ConstructorCase] = &[
    ConstructorCase {
        label: "tuple-struct constructor",
        fixture: "fixture_call_graph",
        domain: "bd:fixture-call-graph",
        owner_module: &["crate"],
        owner: "call_new_type_constructor",
        path: &["NewType"],
        target: ConstructorTarget::Struct { name: "NewType" },
        relation: CallRelationKind::TupleStructConstructor,
        endpoint: CallTargetKind::Struct,
        source_suffix: "fixture_call_graph/src/lib.rs",
    },
    ConstructorCase {
        label: "enum-variant constructor",
        fixture: "fixture_nodes",
        domain: "bd:fixture-nodes",
        owner_module: &["crate", "imports"],
        owner: "use_imported_items",
        path: &["EnumWithData", "Variant1"],
        target: ConstructorTarget::Variant {
            enum_name: "EnumWithData",
            variant_name: "Variant1",
        },
        relation: CallRelationKind::EnumVariantConstructor,
        endpoint: CallTargetKind::Variant,
        source_suffix: "fixture_nodes/src/imports.rs",
    },
];

pub(super) fn constructor_cases() -> &'static [ConstructorCase] {
    CONSTRUCTOR_CASES
}

impl ConstructorTarget {
    fn id(self, db: &Database) -> Result<Uuid, DbError> {
        match self {
            Self::Struct { name } => struct_id_by_name(db, name),
            Self::Variant {
                enum_name,
                variant_name,
            } => variant_id_by_enum_and_variant_names(db, enum_name, variant_name),
        }
    }
}

pub(super) fn assert_constructor_context(
    db: &Database,
    case: &ConstructorCase,
) -> Result<ResolvedConstructor, DbError> {
    let owner = function_id_by_name_in_module(db, case.owner_module, case.owner)?;
    let target = case.target.id(db)?;
    let context = db.call_context_for_owner(owner)?;
    let proof_count = context
        .iter()
        .map(|row| 2 + row.targets.len())
        .sum::<usize>();
    let row = row_by_path(&context, case.path);
    assert_resolved_target(
        row,
        target,
        case.relation,
        CallSiteKind::Path,
        case.endpoint,
    );

    Ok(ResolvedConstructor {
        owner,
        target,
        site: row.site.id,
        span: row.site.span,
        proof_count,
        owner_str: owner.to_string(),
        target_str: target.to_string(),
        site_str: row.site.id.to_string(),
    })
}

pub(super) fn assert_constructor_callers(
    db: &Database,
    case: &ConstructorCase,
    resolved: &ResolvedConstructor,
) -> Result<Vec<CallCallerRow>, DbError> {
    let callers = db.callers_for_target(resolved.target)?;
    let caller = caller_by_owner_kind_path(&callers, resolved.owner, CallSiteKind::Path, case.path);
    assert_eq!(caller.site.id, resolved.site);
    assert_eq!(
        caller.target.relation, case.relation,
        "{} caller relation",
        case.label
    );
    assert_eq!(caller.target.source_kind, CallSiteKind::Path);
    assert_eq!(
        caller.target.target_kind, case.endpoint,
        "{} caller endpoint kind",
        case.label
    );
    Ok(callers)
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
