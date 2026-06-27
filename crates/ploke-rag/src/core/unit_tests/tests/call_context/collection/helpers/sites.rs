use super::*;
pub(in super::super) struct CallSeed<'a> {
    pub(in super::super) id: Uuid,
    pub(in super::super) owner: Uuid,
    pub(in super::super) kind: &'a str,
    pub(in super::super) span: (i64, i64),
    pub(in super::super) path: Option<Vec<&'a str>>,
    pub(in super::super) method: Option<&'a str>,
    pub(in super::super) macro_name: Option<&'a str>,
    pub(in super::super) receiver: Option<(&'a str, Vec<&'a str>)>,
    pub(in super::super) arg_count: Option<i64>,
    pub(in super::super) generic_arg_count: Option<i64>,
}
pub(in super::super) fn insert_call_site(db: &Database, seed: CallSeed<'_>) -> Result<(), Error> {
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
