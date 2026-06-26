use super::*;

#[cfg(feature = "call_graph")]
pub(in super::super) fn insert_call_status(
    db: &Database,
    site: Uuid,
    kind: &str,
    status: &str,
    resolution: Option<&str>,
) -> Result<(), Error> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), uuid(site));
    params.insert("source_kind".to_string(), DataValue::from(kind));
    params.insert("status_kind".to_string(), DataValue::from(status));
    params.insert("resolution_kind".to_string(), option_str(resolution));

    db.raw_query_mut_params(
            r#"?[source_id, at, source_kind, status_kind, resolution_kind] :=
                source_id = $site_id,
                source_kind = $source_kind,
                status_kind = $status_kind,
                resolution_kind = $resolution_kind,
                at = 'ASSERT'
            :put call_resolution_status { source_id, at => source_kind, status_kind, resolution_kind }"#,
            params,
        )
        .map_err(Error::from)?;
    Ok(())
}
