use super::*;

pub(in crate::unit) struct TryResultContext {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) context: Vec<CallContextRow>,
    pub(in crate::unit) sites: TryResultSites,
    pub(in crate::unit) targets: TryResultTargets,
}

pub(in crate::unit) struct TryResultSites {
    pub(in crate::unit) ok: Uuid,
    pub(in crate::unit) try_call: Uuid,
    pub(in crate::unit) method: Uuid,
}

pub(in crate::unit) struct TryResultTargets {
    pub(in crate::unit) try_fn: Uuid,
    pub(in crate::unit) method: Uuid,
}

pub(in crate::unit) fn try_result_context(db: &Database) -> Result<TryResultContext, DbError> {
    let owner = function_id_by_name(db, "call_try_result_instance_method")?;
    let targets = TryResultTargets {
        try_fn: function_id_by_name(db, "try_local_assoc")?,
        method: method_id_by_impl_self_type_name(db, "LocalAssoc", "instance_value")?,
    };
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "try-result context rows: {context:#?}");

    let ok = row_by_path(&context, &["Ok"]).site.id;
    let try_call = row_by_path(&context, &["try_local_assoc"]).site.id;
    let receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let method = row_by_method_receiver(&context, "instance_value", &receiver)
        .site
        .id;

    Ok(TryResultContext {
        owner,
        context,
        sites: TryResultSites {
            ok,
            try_call,
            method,
        },
        targets,
    })
}
