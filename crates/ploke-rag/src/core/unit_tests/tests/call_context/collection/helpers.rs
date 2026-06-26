use super::super::super::*;

#[cfg(feature = "call_graph")]
pub(super) struct CallSeed<'a> {
    pub(super) id: Uuid,
    pub(super) owner: Uuid,
    pub(super) kind: &'a str,
    pub(super) span: (i64, i64),
    pub(super) path: Option<Vec<&'a str>>,
    pub(super) method: Option<&'a str>,
    pub(super) macro_name: Option<&'a str>,
    pub(super) receiver: Option<(&'a str, Vec<&'a str>)>,
    pub(super) arg_count: Option<i64>,
    pub(super) generic_arg_count: Option<i64>,
}

#[cfg(feature = "call_graph")]
pub(super) fn insert_call_site(db: &Database, seed: CallSeed<'_>) -> Result<(), Error> {
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(seed.id));
    params.insert("owner_id".to_string(), uuid(seed.owner));
    params.insert("call_kind".to_string(), DataValue::from(seed.kind));
    params.insert("span".to_string(), span(seed.span));
    params.insert("cfgs".to_string(), list(&[]));
    params.insert("path".to_string(), option_list(seed.path));
    params.insert("method_name".to_string(), option_str(seed.method));
    params.insert("macro_name".to_string(), option_str(seed.macro_name));
    let (kind, path) = seed
        .receiver
        .map(|(kind, path)| (DataValue::from(kind), list(&path)))
        .unwrap_or((DataValue::Null, DataValue::Null));
    params.insert("receiver_kind".to_string(), kind);
    params.insert("receiver_path".to_string(), path);
    params.insert("arg_count".to_string(), option_int(seed.arg_count));
    params.insert(
        "generic_arg_count".to_string(),
        option_int(seed.generic_arg_count),
    );

    db.raw_query_mut_params(
            r#"?[id, at, owner_id, call_kind, span, cfgs, path, method_name, macro_name, receiver_kind, receiver_path, arg_count, generic_arg_count] :=
                id = $id,
                owner_id = $owner_id,
                call_kind = $call_kind,
                span = $span,
                cfgs = $cfgs,
                path = $path,
                method_name = $method_name,
                macro_name = $macro_name,
                receiver_kind = $receiver_kind,
                receiver_path = $receiver_path,
                arg_count = $arg_count,
                generic_arg_count = $generic_arg_count,
                at = 'ASSERT'
            :put call_site { id, at => owner_id, call_kind, span, cfgs, path, method_name, macro_name, receiver_kind, receiver_path, arg_count, generic_arg_count }"#,
            params,
        )
        .map_err(Error::from)?;
    Ok(())
}

#[cfg(feature = "call_graph")]
pub(super) fn insert_call_edge(
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

#[cfg(feature = "call_graph")]
fn ensure_function_owner(db: &Database, owner: Uuid) -> Result<(), Error> {
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

#[cfg(feature = "call_graph")]
pub(super) fn insert_call_target(
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

#[cfg(feature = "call_graph")]
fn ensure_call_target(db: &Database, target: Uuid, kind: &str) -> Result<(), Error> {
    match kind {
        "Function" => ensure_function_owner(db, target),
        "Method" => ensure_method_target(db, target),
        "Struct" => ensure_struct_target(db, target),
        "Variant" => ensure_variant_target(db, target),
        other => panic!("unexpected synthetic call target kind {other}"),
    }
}

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
pub(super) fn insert_call_status(
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

#[cfg(feature = "call_graph")]
fn uuid(value: Uuid) -> DataValue {
    DataValue::Uuid(UuidWrapper(value))
}

#[cfg(feature = "call_graph")]
fn span((start, end): (i64, i64)) -> DataValue {
    DataValue::List(vec![DataValue::from(start), DataValue::from(end)])
}

#[cfg(feature = "call_graph")]
fn list(items: &[&str]) -> DataValue {
    DataValue::List(items.iter().map(|item| DataValue::from(*item)).collect())
}

#[cfg(feature = "call_graph")]
fn option_list(items: Option<Vec<&str>>) -> DataValue {
    items.map(|items| list(&items)).unwrap_or(DataValue::Null)
}

#[cfg(feature = "call_graph")]
fn option_str(value: Option<&str>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}

#[cfg(feature = "call_graph")]
fn option_int(value: Option<i64>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}
