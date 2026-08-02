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

pub(in crate::unit) struct InitializerCase<'a> {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) label: &'a str,
    pub(in crate::unit) path: &'a [&'a str],
    pub(in crate::unit) target: Uuid,
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

pub(in crate::unit) fn const_static_cases(
    db: &Database,
) -> Result<Vec<InitializerCase<'static>>, DbError> {
    let target = function_id_by_name_in_module(db, &["crate", "const_static"], "five")?;

    Ok(vec![
        InitializerCase {
            owner: const_id_by_name(db, "FN_CALL_CONST")?,
            label: "const initializer",
            path: &["five"],
            target,
        },
        InitializerCase {
            owner: static_id_by_name(db, "STATIC_FN_CALL")?,
            label: "static initializer",
            path: &["five"],
            target,
        },
    ])
}

pub(in crate::unit) fn assoc_const_cases(
    db: &Database,
) -> Result<Vec<InitializerCase<'static>>, DbError> {
    let target = function_id_by_name(db, "assoc_const_value")?;

    Ok(vec![
        InitializerCase {
            owner: const_id_by_name(db, "IMPL_ASSOC_VALUE")?,
            label: "impl associated const initializer",
            path: &["assoc_const_value"],
            target,
        },
        InitializerCase {
            owner: const_id_by_name(db, "TRAIT_ASSOC_VALUE")?,
            label: "trait associated const initializer",
            path: &["assoc_const_value"],
            target,
        },
    ])
}

pub(in crate::unit) fn assert_initializer_contexts(
    db: &Database,
    cases: &[InitializerCase<'_>],
) -> Result<Vec<OwnerProofEdge>, DbError> {
    let mut expected = Vec::new();

    for case in cases {
        let context = db.call_context_for_owner(case.owner)?;
        assert_eq!(
            context.len(),
            1,
            "{} context rows: {context:#?}",
            case.label
        );

        let row = row_by_path(&context, case.path);
        assert_eq!(row.site.owner_id, case.owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_resolved_target(
            row,
            case.target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
        expected.push(OwnerProofEdge {
            owner: case.owner,
            site: row.site.id,
            span: row.site.span,
            target: case.target,
        });
    }

    Ok(expected)
}

pub(in crate::unit) fn assert_initializer_proofs(
    db: &Database,
    label: &str,
    domain: &str,
    cases: &[InitializerCase<'_>],
    source_suffix: &str,
) -> Result<(), DbError> {
    let expected = assert_initializer_contexts(db, cases)?;

    for case in cases {
        let count = db.project_call_proof_facts_for_owner(case.owner, domain)?;
        assert_eq!(count, 3, "{} proof fact count", case.label);
    }

    assert_owner_proof_edges(
        db,
        label,
        &expected,
        source_suffix,
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )
}
