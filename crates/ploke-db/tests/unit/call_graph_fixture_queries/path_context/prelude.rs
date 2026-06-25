use super::*;

#[test]
fn fixture_context_reads_projected_prelude_drop_shadowing() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_prelude_drop_value")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "prelude drop context rows: {context:#?}");

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(&["drop"], 1, CallStatusKind::External, "prelude drop"),
    );

    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "prelude_shadow_scope"],
        "call_local_drop_shadow",
    )?;
    let target = function_id_by_name_in_module(&db, &["crate", "prelude_shadow_scope"], "drop")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "local drop shadow context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["drop"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    Ok(())
}
