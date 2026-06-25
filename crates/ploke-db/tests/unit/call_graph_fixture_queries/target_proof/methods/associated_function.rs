use super::*;

#[test]
fn fixture_projection_stores_real_target_centered_associated_function_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let local_owner = function_id_by_name(&db, "call_local_assoc_make")?;
    let self_owner = method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?;
    let qualified_owner = function_id_by_name(&db, "call_qualified_local_assoc_make")?;

    let local_context = db.call_context_for_owner(local_owner)?;
    let local_site = row_by_path(&local_context, &["LocalAssoc", "make"]).site.id;

    let self_context = db.call_context_for_owner(self_owner)?;
    let self_site = row_by_path(&self_context, &["Self", "make"]).site.id;

    let qualified_context = db.call_context_for_owner(qualified_owner)?;
    let qualified_site = row_by_path(&qualified_context, &["LocalAssoc", "make"])
        .site
        .id;

    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 3, "target-centered associated-function")?;

    for (owner, expected_path) in [
        (local_owner, &["LocalAssoc", "make"][..]),
        (self_owner, &["Self", "make"][..]),
        (qualified_owner, &["LocalAssoc", "make"][..]),
    ] {
        let caller = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, expected_path);
        assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Method);
    }

    assert_target_proof_projection(
        &db,
        "target-centered associated-function",
        "bd:fixture-call-graph",
        target,
        &callers,
        &[
            TargetProofSite {
                owner: local_owner,
                site: local_site,
            },
            TargetProofSite {
                owner: self_owner,
                site: self_site,
            },
            TargetProofSite {
                owner: qualified_owner,
                site: qualified_site,
            },
        ],
        "src/lib.rs",
        "type_resolution_missing",
    )?;

    Ok(())
}
