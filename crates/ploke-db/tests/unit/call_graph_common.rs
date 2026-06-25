use std::collections::BTreeMap;

use cozo::DataValue;
use ploke_db::{Database, DbError};
use uuid::Uuid;

mod facts;
mod site;
mod values;

pub(super) use facts::{fact_count, fact_for_call_site};
pub(super) use site::{SiteSeed, insert_call_site, insert_call_site_raw_receiver};
pub(super) use values::list;

use values::{option_str, span, uuid};

pub(super) fn insert_edge(
    db: &Database,
    owner: Uuid,
    site: Uuid,
    kind: &str,
) -> Result<(), DbError> {
    insert_edge_with_source_kind(db, owner, site, "Function", kind)
}

pub(super) fn insert_edge_with_source_kind(
    db: &Database,
    owner: Uuid,
    site: Uuid,
    source_kind: &str,
    target_kind: &str,
) -> Result<(), DbError> {
    ensure_function_owner(db, owner)?;

    let mut params = BTreeMap::new();
    params.insert("owner_id".to_string(), uuid(owner));
    params.insert("site_id".to_string(), uuid(site));
    params.insert("source_kind".to_string(), DataValue::from(source_kind));
    params.insert("target_kind".to_string(), DataValue::from(target_kind));

    db.raw_query_mut_params(
        r#"?[source_id, target_id, at, relation_kind, source_kind, target_kind] :=
            source_id = $owner_id,
            target_id = $site_id,
            relation_kind = "BodyContainsCall",
            source_kind = $source_kind,
            target_kind = $target_kind,
            at = 'ASSERT'
        :put call_site_edge { source_id, target_id, at => relation_kind, source_kind, target_kind }"#,
        params,
    )?;
    Ok(())
}

pub(super) fn insert_relation(
    db: &Database,
    site: Uuid,
    target: Uuid,
    relation: &str,
    source_kind: &str,
    target_kind: &str,
) -> Result<(), DbError> {
    ensure_call_target(db, target, target_kind)?;
    insert_relation_raw(db, site, target, relation, source_kind, target_kind)
}

pub(super) fn insert_relation_raw(
    db: &Database,
    site: Uuid,
    target: Uuid,
    relation: &str,
    source_kind: &str,
    target_kind: &str,
) -> Result<(), DbError> {
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
    )?;
    Ok(())
}

fn ensure_call_target(db: &Database, target: Uuid, kind: &str) -> Result<(), DbError> {
    match kind {
        "Function" => ensure_function_owner(db, target),
        "Method" => ensure_method_target(db, target),
        "Struct" => ensure_struct_target(db, target),
        "Variant" => ensure_variant_target(db, target),
        other => panic!("unexpected synthetic call target kind {other}"),
    }
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

pub(super) fn insert_status(
    db: &Database,
    site: Uuid,
    kind: &str,
    status: &str,
    resolution: Option<&str>,
) -> Result<(), DbError> {
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
    )?;
    Ok(())
}

fn ensure_function_owner(db: &Database, owner: Uuid) -> Result<(), DbError> {
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

pub(super) fn insert_owner_source(
    db: &Database,
    owner: Uuid,
    module: Uuid,
    file: &str,
) -> Result<(), DbError> {
    let namespace = Uuid::from_u128(99);
    insert_module(db, module)?;
    insert_function(db, owner, module)?;
    insert_contains(db, module, owner)?;

    let mut params = BTreeMap::new();
    params.insert("owner_id".to_string(), uuid(module));
    params.insert("file_path".to_string(), DataValue::from(file));
    params.insert("file_docs".to_string(), DataValue::Null);
    params.insert("items".to_string(), DataValue::List(vec![uuid(owner)]));
    params.insert("namespace".to_string(), uuid(namespace));

    db.raw_query_mut_params(
        r#"?[owner_id, at, file_path, file_docs, items, namespace] :=
            owner_id = $owner_id,
            file_path = $file_path,
            file_docs = $file_docs,
            items = $items,
            namespace = $namespace,
            at = 'ASSERT'
        :put file_mod { owner_id, at => file_path, file_docs, items, namespace }"#,
        params,
    )?;
    Ok(())
}

fn insert_module(db: &Database, module: Uuid) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(module));
    params.insert("name".to_string(), DataValue::from("crate"));
    params.insert("path".to_string(), list(&["crate"]));
    params.insert("vis_kind".to_string(), DataValue::from("Public"));
    params.insert("vis_path".to_string(), DataValue::Null);
    params.insert("docstring".to_string(), DataValue::Null);
    params.insert("span".to_string(), span((0, 100)));
    params.insert("tracking_hash".to_string(), uuid(Uuid::from_u128(98)));
    params.insert("module_kind".to_string(), DataValue::from("FileBased"));
    params.insert("cfgs".to_string(), list(&[]));

    db.raw_query_mut_params(
        r#"?[id, at, name, path, vis_kind, vis_path, docstring, span, tracking_hash, module_kind, cfgs] :=
            id = $id,
            name = $name,
            path = $path,
            vis_kind = $vis_kind,
            vis_path = $vis_path,
            docstring = $docstring,
            span = $span,
            tracking_hash = $tracking_hash,
            module_kind = $module_kind,
            cfgs = $cfgs,
            at = 'ASSERT'
        :put module { id, at => name, path, vis_kind, vis_path, docstring, span, tracking_hash, module_kind, cfgs }"#,
        params,
    )?;
    Ok(())
}

fn insert_function(db: &Database, owner: Uuid, module: Uuid) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(owner));
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

fn insert_contains(db: &Database, module: Uuid, owner: Uuid) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("module_id".to_string(), uuid(module));
    params.insert("owner_id".to_string(), uuid(owner));

    db.raw_query_mut_params(
        r#"?[source_id, target_id, at, relation_kind, source_kind, target_kind] :=
            source_id = $module_id,
            target_id = $owner_id,
            relation_kind = "Contains",
            source_kind = "Module",
            target_kind = "Function",
            at = 'ASSERT'
        :put syntax_edge { source_id, target_id, at => relation_kind, source_kind, target_kind }"#,
        params,
    )?;
    Ok(())
}
