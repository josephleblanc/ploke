use super::*;
pub(in super::super) fn insert_call_edge(
    db: &Database,
    owner: Uuid,
    site: Uuid,
    kind: &str,
) -> Result<(), Error> {
    ensure_function_owner(db, owner)?;

    let mut params = BTreeMap::new();
    params.insert("owner_id".to_string(), uuid(owner));
    params.insert("site_id".to_string(), uuid(site));
    params.insert("target_kind".to_string(), DataValue::from(kind));

    db.raw_query_mut_params(
            r#"?[source_id, target_id, at, relation_kind, source_kind, target_kind] :=
                source_id = $owner_id,
                target_id = $site_id,
                relation_kind = "BodyContainsCall",
                source_kind = "Function",
                target_kind = $target_kind,
                at = 'ASSERT'
            :put call_site_edge { source_id, target_id, at => relation_kind, source_kind, target_kind }"#,
            params,
        )
        .map_err(Error::from)?;
    Ok(())
}
