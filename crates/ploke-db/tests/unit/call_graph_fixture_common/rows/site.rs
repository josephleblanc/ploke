use super::*;

pub(in crate::unit) fn body_edges_for_site(
    db: &Database,
    site_id: Uuid,
) -> Result<QueryResult, DbError> {
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

pub(in crate::unit) fn statuses_for_site(
    db: &Database,
    site_id: Uuid,
) -> Result<QueryResult, DbError> {
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

pub(in crate::unit) fn relations_for_site(
    db: &Database,
    site_id: Uuid,
) -> Result<QueryResult, DbError> {
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

pub(in crate::unit) fn call_site_owner_and_kind(
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

pub(in crate::unit) fn call_site_kind_for_site(
    db: &Database,
    site_id: Uuid,
) -> Result<String, DbError> {
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

pub(in crate::unit) fn owner_kind_for_call_body_owner(
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

pub(in crate::unit) fn call_target_exists(
    db: &Database,
    target: Uuid,
    kind: &str,
) -> Result<bool, DbError> {
    let relation = call_target_endpoint_relation(kind)
        .unwrap_or_else(|| panic!("unexpected call relation target kind {kind}"));
    let rows = db.raw_query(&format!(
        r#"?[id] :=
            id = to_uuid("{target}"),
            *{relation} {{ id @ 'NOW' }}"#
    ))?;
    Ok(rows.rows.len() == 1)
}
