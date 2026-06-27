use super::*;

#[test]
fn fixture_context_reads_projected_ambiguous_dynamic_candidates() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;

    for owner_name in AMBIGUOUS_DYNAMIC_OWNERS {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        assert_dynamic_candidates(&context[0], owner, &expected, owner_name);
    }

    let target = function_id_by_name(&db, "other_target")?;
    let context = db.call_context_for_target(target)?;
    assert_eq!(
        context.len(),
        AMBIGUOUS_DYNAMIC_OWNERS.len(),
        "other_target should expose every ambiguous dynamic caller: {context:#?}"
    );

    for owner_name in AMBIGUOUS_DYNAMIC_OWNERS {
        let owner = function_id_by_name(&db, owner_name)?;
        let row = context
            .iter()
            .find(|row| row.site.owner_id == owner)
            .unwrap_or_else(|| {
                panic!("target-centered context missing {owner_name}: {context:#?}")
            });
        assert_dynamic_candidates(row, owner, &expected, owner_name);
    }

    Ok(())
}
