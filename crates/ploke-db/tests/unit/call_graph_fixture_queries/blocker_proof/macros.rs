use super::*;

#[test]
fn fixture_projection_marks_real_macro_call_without_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        ("call_crate_scoped_macro", "crate::crate_scoped_macro"),
        ("call_vec_macro", "vec"),
        ("call_imported_macro_alias", "imported_macro_alias"),
        ("call_item_macro_inside_body", "call_graph_item_macro"),
    ];
    let mut expected = Vec::new();

    for (owner_name, macro_name) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = assert_targetless_macro_row(
            &context,
            owner,
            TargetlessMacroCase::macro_call(macro_name, owner_name),
        );
        let site = row.site.id;
        let span = row.site.span;

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 2);
        expected.push(BlockerProofSite {
            site,
            span,
            blocker_reason: "macro_expansion_not_available",
        });
    }

    assert_targetless_blocker_proofs(&db, "macro", &expected, "fixture_call_graph/src/lib.rs")?;

    Ok(())
}
