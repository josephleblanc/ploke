use super::*;

#[cfg(feature = "call_graph")]
pub(super) fn ensure_function_owner(db: &Database, owner: Uuid) -> Result<(), Error> {
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(owner));
    let rows = db
        .raw_query_params(
            r#"?[id] := *function { id @ 'NOW' }, id = $id"#,
            params.clone(),
        )
        .map_err(Error::from)?;
    if !rows.rows.is_empty() {
        return Ok(());
    }

    let module = Uuid::from_u128(0xfeed_0000_0000_0000_0000_0000_0000_0002);
    params.insert("name".to_string(), DataValue::from("caller"));
    params.insert("docstring".to_string(), DataValue::Null);
    params.insert("vis_kind".to_string(), DataValue::from("Public"));
    params.insert("vis_path".to_string(), DataValue::Null);
    params.insert("span".to_string(), span((0, 100)));
    params.insert("tracking_hash".to_string(), uuid(Uuid::from_u128(97)));
    params.insert("cfgs".to_string(), list(&[]));
    params.insert("return_type_id".to_string(), DataValue::Null);
    params.insert("body".to_string(), DataValue::Null);
    params.insert("module_id".to_string(), uuid(module));

    db.raw_query_mut_params(
            r#"?[id, at, name, docstring, vis_kind, vis_path, span, tracking_hash, cfgs, return_type_id, body, module_id] :=
                id = $id,
                name = $name,
                docstring = $docstring,
                vis_kind = $vis_kind,
                vis_path = $vis_path,
                span = $span,
                tracking_hash = $tracking_hash,
                cfgs = $cfgs,
                return_type_id = $return_type_id,
                body = $body,
                module_id = $module_id,
                at = 'ASSERT'
            :put function { id, at => name, docstring, vis_kind, vis_path, span, tracking_hash, cfgs, return_type_id, body, module_id }"#,
            params,
        )
        .map_err(Error::from)?;
    Ok(())
}
