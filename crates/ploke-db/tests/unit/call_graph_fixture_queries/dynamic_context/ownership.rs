use super::*;

#[test]
fn fixture_context_does_not_project_closure_or_async_body_calls_to_outer_owner()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let forbidden_path = path(&["local_target"]);
    let owners = [
        "closure_body_call_is_not_outer_call_site",
        "async_block_call_is_not_outer_call_site",
        "call_move_closure_literal_with_body_call",
        "call_async_closure_literal_with_body_call",
    ];

    for owner_name in owners {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert!(
            context.iter().all(|row| row.site.kind != CallSiteKind::Path
                || row.site.path.as_ref() != Some(&forbidden_path)),
            "{owner_name} leaked a closure/async body local_target() path row into the outer owner: {context:#?}"
        );
        assert!(
            context
                .iter()
                .flat_map(|row| row.targets.iter())
                .all(|target| target.target_id != local_target),
            "{owner_name} leaked a closure/async body edge to local_target into the outer owner: {context:#?}"
        );
    }

    Ok(())
}

#[test]
fn fixture_context_does_not_project_local_const_initializer_calls_to_outer_owner()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "local_const_initializer_call_is_not_outer_call_site")?;
    let target = function_id_by_name(&db, "assoc_const_value")?;
    let forbidden_path = path(&["assoc_const_value"]);

    let context = db.call_context_for_owner(owner)?;
    assert!(
        context.iter().all(|row| row.site.kind != CallSiteKind::Path
            || row.site.path.as_ref() != Some(&forbidden_path)),
        "local const initializer assoc_const_value() path row leaked into the outer owner: {context:#?}"
    );
    assert!(
        context
            .iter()
            .flat_map(|row| row.targets.iter())
            .all(|edge| edge.target_id != target),
        "local const initializer edge to assoc_const_value leaked into the outer owner: {context:#?}"
    );

    Ok(())
}
