use std::collections::BTreeMap;

use cozo::DataValue;
use ploke_db::{Database, DbError};
use uuid::Uuid;

use super::values::{list, option_int, option_list, option_str, span, uuid};

pub(in crate::unit) struct SiteSeed<'a> {
    pub id: Uuid,
    pub owner: Uuid,
    pub kind: &'a str,
    pub span: (i64, i64),
    pub path: Option<Vec<&'a str>>,
    pub method: Option<&'a str>,
    pub macro_name: Option<&'a str>,
    pub receiver: Option<(&'a str, Vec<&'a str>)>,
    pub arg_count: Option<i64>,
    pub generic_arg_count: Option<i64>,
}

pub(in crate::unit) fn insert_call_site(db: &Database, seed: SiteSeed<'_>) -> Result<(), DbError> {
    let (kind, path) = seed
        .receiver
        .as_ref()
        .map(|(kind, path)| (DataValue::from(*kind), list(path.as_slice())))
        .unwrap_or((DataValue::Null, DataValue::Null));
    insert_call_site_raw_receiver(db, seed, kind, path)
}

pub(in crate::unit) fn insert_call_site_raw_receiver(
    db: &Database,
    seed: SiteSeed<'_>,
    receiver_kind: DataValue,
    receiver_path: DataValue,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), uuid(seed.id));
    params.insert("owner_id".to_string(), uuid(seed.owner));
    params.insert("call_kind".to_string(), DataValue::from(seed.kind));
    params.insert("span".to_string(), span(seed.span));
    params.insert("cfgs".to_string(), list(&[]));
    params.insert("path".to_string(), option_list(seed.path));
    params.insert("method_name".to_string(), option_str(seed.method));
    params.insert("macro_name".to_string(), option_str(seed.macro_name));
    params.insert("receiver_kind".to_string(), receiver_kind);
    params.insert("receiver_path".to_string(), receiver_path);
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
    )?;
    Ok(())
}
