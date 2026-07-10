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
        AMBIGUOUS_DYNAMIC_OWNERS.len() + 5,
        "other_target should expose every ambiguous candidate caller: {context:#?}"
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

    let owner = function_id_by_name(&db, AMBIGUOUS_PATH_OWNER)?;
    let row = context
        .iter()
        .find(|row| row.site.owner_id == owner)
        .unwrap_or_else(|| {
            panic!("target-centered context missing {AMBIGUOUS_PATH_OWNER}: {context:#?}")
        });
    assert_path_function_candidates(row, owner, &["f"], &expected, AMBIGUOUS_PATH_OWNER);

    for (owner_name, expected_path) in [
        ("call_multi_conflicting_function_pointer_param", &["f"][..]),
        (
            "call_forwarded_conflicting_function_pointer_leaf",
            &["f"][..],
        ),
        (
            "call_multi_conflicting_generic_fn_once_param",
            &["generic_f"][..],
        ),
    ] {
        let owner = function_id_by_name(&db, owner_name)?;
        let row = context
            .iter()
            .find(|row| row.site.owner_id == owner)
            .unwrap_or_else(|| {
                panic!("target-centered context missing {owner_name}: {context:#?}")
            });
        assert_path_function_candidates(row, owner, expected_path, &expected, owner_name);
    }

    let owner_name = "call_multi_conflicting_named_field_function_param";
    let owner = function_id_by_name(&db, owner_name)?;
    let row = context
        .iter()
        .find(|row| row.site.owner_id == owner)
        .unwrap_or_else(|| panic!("target-centered context missing {owner_name}: {context:#?}"));
    assert_dynamic_path_function_candidates(
        row,
        owner,
        &["holder", "callback"],
        &expected,
        owner_name,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_mixed_branch_dynamic_candidates() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let function = function_id_by_name(&db, "local_target")?;

    for owner_name in MIXED_DYNAMIC_OWNERS {
        let owner = function_id_by_name(&db, owner_name)?;
        let closure = closure_owner_for_parent(&db, owner)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        assert_mixed_dynamic_candidates(&context[0], owner, function, closure, owner_name);

        for target in [function, closure] {
            let context = db.call_context_for_target(target)?;
            let row = context
                .iter()
                .find(|row| row.site.owner_id == owner)
                .unwrap_or_else(|| {
                    panic!("target-centered context missing {owner_name}: {context:#?}")
                });
            assert_mixed_dynamic_candidates(row, owner, function, closure, owner_name);
        }
    }

    Ok(())
}
