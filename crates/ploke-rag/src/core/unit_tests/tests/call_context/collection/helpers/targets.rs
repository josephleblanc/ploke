use super::*;
pub(in super::super) fn insert_call_target(
    db: &Database,
    site: Uuid,
    target: Uuid,
    relation: &str,
    source_kind: &str,
    target_kind: &str,
) -> Result<(), Error> {
    ensure_call_target(db, target, target_kind)?;

    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), uuid(site));
    params.insert("target_id".to_string(), uuid(target));
    params.insert("relation_kind".to_string(), DataValue::from(relation));
    params.insert("source_kind".to_string(), DataValue::from(source_kind));
    params.insert("target_kind".to_string(), DataValue::from(target_kind));

    db.raw_query_mut_params(
            r#"?[source_id, target_id, at, relation_kind, source_kind, target_kind] :=
                source_id = $site_id,
                target_id = $target_id,
                relation_kind = $relation_kind,
                source_kind = $source_kind,
                target_kind = $target_kind,
                at = 'ASSERT'
            :put call_relation { source_id, target_id, at => relation_kind, source_kind, target_kind }"#,
            params,
        )
        .map_err(Error::from)?;
    Ok(())
}
fn ensure_call_target(db: &Database, target: Uuid, kind: &str) -> Result<(), Error> {
    match kind {
        "Function" => ensure_function_owner(db, target),
        "Method" => ensure_method_target(db, target),
        "Struct" => ensure_struct_target(db, target),
        "Variant" => ensure_variant_target(db, target),
        other => panic!("unexpected synthetic call target kind {other}"),
    }
}
fn ensure_method_target(db: &Database, target: Uuid) -> Result<(), Error> {
    if relation_has_id(db, "method", target)? {
        return Ok(());
    }

    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(target));
    params.insert("name".to_string(), DataValue::from("target_method"));
    params.insert("span".to_string(), span((0, 100)));
    params.insert("vis_kind".to_string(), DataValue::from("Public"));
    params.insert("vis_path".to_string(), DataValue::Null);
    params.insert("docstring".to_string(), DataValue::Null);
    params.insert("body".to_string(), DataValue::Null);
    params.insert("tracking_hash".to_string(), uuid(Uuid::from_u128(0x101)));
    params.insert("cfgs".to_string(), list(&[]));
    params.insert("owner_id".to_string(), uuid(Uuid::from_u128(0x102)));

    db.raw_query_mut_params(
            r#"?[id, at, name, span, vis_kind, vis_path, docstring, body, tracking_hash, cfgs, owner_id] :=
                id = $id,
                name = $name,
                span = $span,
                vis_kind = $vis_kind,
                vis_path = $vis_path,
                docstring = $docstring,
                body = $body,
                tracking_hash = $tracking_hash,
                cfgs = $cfgs,
                owner_id = $owner_id,
                at = 'ASSERT'
            :put method { id, at => name, span, vis_kind, vis_path, docstring, body, tracking_hash, cfgs, owner_id }"#,
            params,
        )
        .map_err(Error::from)?;
    Ok(())
}
fn ensure_struct_target(db: &Database, target: Uuid) -> Result<(), Error> {
    if relation_has_id(db, "struct", target)? {
        return Ok(());
    }

    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(target));
    params.insert("name".to_string(), DataValue::from("TargetStruct"));
    params.insert("span".to_string(), span((0, 100)));
    params.insert("vis_kind".to_string(), DataValue::from("Public"));
    params.insert("vis_path".to_string(), DataValue::Null);
    params.insert("docstring".to_string(), DataValue::Null);
    params.insert("tracking_hash".to_string(), uuid(Uuid::from_u128(0x103)));
    params.insert("cfgs".to_string(), list(&[]));

    db.raw_query_mut_params(
            r#"?[id, at, name, span, vis_kind, vis_path, docstring, tracking_hash, cfgs] :=
                id = $id,
                name = $name,
                span = $span,
                vis_kind = $vis_kind,
                vis_path = $vis_path,
                docstring = $docstring,
                tracking_hash = $tracking_hash,
                cfgs = $cfgs,
                at = 'ASSERT'
            :put struct { id, at => name, span, vis_kind, vis_path, docstring, tracking_hash, cfgs }"#,
            params,
        )
        .map_err(Error::from)?;
    Ok(())
}
fn ensure_variant_target(db: &Database, target: Uuid) -> Result<(), Error> {
    if relation_has_id(db, "variant", target)? {
        return Ok(());
    }

    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(target));
    params.insert("name".to_string(), DataValue::from("TargetVariant"));
    params.insert("owner_id".to_string(), uuid(Uuid::from_u128(0x104)));
    params.insert("index".to_string(), DataValue::from(0));
    params.insert("discriminant".to_string(), DataValue::Null);
    params.insert("cfgs".to_string(), list(&[]));

    db.raw_query_mut_params(
        r#"?[id, at, name, owner_id, index, discriminant, cfgs] :=
                id = $id,
                name = $name,
                owner_id = $owner_id,
                index = $index,
                discriminant = $discriminant,
                cfgs = $cfgs,
                at = 'ASSERT'
            :put variant { id, at => name, owner_id, index, discriminant, cfgs }"#,
        params,
    )
    .map_err(Error::from)?;
    Ok(())
}
fn relation_has_id(db: &Database, relation: &str, id: Uuid) -> Result<bool, Error> {
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(id));
    let rows = db
        .raw_query_params(
            &format!(r#"?[id] := *{relation} {{ id @ 'NOW' }}, id = $id"#),
            params,
        )
        .map_err(Error::from)?;
    Ok(!rows.rows.is_empty())
}
