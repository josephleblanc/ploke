use std::collections::BTreeMap;

use cozo::DataValue;
use ploke_db::{Database, DbError};
use uuid::Uuid;

use super::values::{list, span, uuid};

pub(in crate::unit) fn insert_owner_source(
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
