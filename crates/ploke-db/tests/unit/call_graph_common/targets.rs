use std::collections::BTreeMap;

use cozo::DataValue;
use ploke_db::{Database, DbError};
use uuid::Uuid;

use super::values::{list, span, uuid};

pub(super) fn ensure_call_target(db: &Database, target: Uuid, kind: &str) -> Result<(), DbError> {
    match kind {
        "Function" => ensure_function_owner(db, target),
        "Method" => ensure_method_target(db, target),
        "Struct" => ensure_struct_target(db, target),
        "Variant" => ensure_variant_target(db, target),
        other => panic!("unexpected synthetic call target kind {other}"),
    }
}

pub(super) fn ensure_function_owner(db: &Database, owner: Uuid) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(owner));
    let rows = db.raw_query_params(
        r#"?[id] := *function { id @ 'NOW' }, id = $id"#,
        params.clone(),
    )?;
    if !rows.rows.is_empty() {
        return Ok(());
    }

    let module = Uuid::from_u128(0xfeed_0000_0000_0000_0000_0000_0000_0001);
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
    )?;
    Ok(())
}

fn ensure_method_target(db: &Database, target: Uuid) -> Result<(), DbError> {
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
    )?;
    Ok(())
}

fn ensure_struct_target(db: &Database, target: Uuid) -> Result<(), DbError> {
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
    )?;
    Ok(())
}

fn ensure_variant_target(db: &Database, target: Uuid) -> Result<(), DbError> {
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
    )?;
    Ok(())
}

fn relation_has_id(db: &Database, relation: &str, id: Uuid) -> Result<bool, DbError> {
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(id));
    let rows = db.raw_query_params(
        &format!(r#"?[id] := *{relation} {{ id @ 'NOW' }}, id = $id"#),
        params,
    )?;
    Ok(!rows.rows.is_empty())
}
