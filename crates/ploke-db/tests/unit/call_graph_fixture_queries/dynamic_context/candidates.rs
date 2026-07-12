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
        OTHER_TARGET_AMBIGUOUS_CANDIDATE_COUNT,
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

    for (owner_name, expected_path) in AMBIGUOUS_PATH_FUNCTION_CANDIDATES {
        let owner = function_id_by_name(&db, owner_name)?;
        let row = context
            .iter()
            .find(|row| row.site.owner_id == owner)
            .unwrap_or_else(|| {
                panic!("target-centered context missing {owner_name}: {context:#?}")
            });
        assert_path_function_candidates(row, owner, expected_path, &expected, owner_name);
    }

    for (owner_name, expected_path) in AMBIGUOUS_DYNAMIC_FUNCTION_CANDIDATES {
        let owner = function_id_by_name(&db, owner_name)?;
        let row = context
            .iter()
            .find(|row| row.site.owner_id == owner)
            .unwrap_or_else(|| {
                panic!("target-centered context missing {owner_name}: {context:#?}")
            });
        assert_dynamic_path_function_candidates(row, owner, expected_path, &expected, owner_name);
    }

    for owner_name in RETURNED_CONFLICTING_FUNCTION_POINTER_OWNERS {
        let owner = function_id_by_name(&db, owner_name)?;
        let owner_context = db.call_context_for_owner(owner)?;
        assert_eq!(
            owner_context.len(),
            2,
            "{owner_name} owner context rows: {owner_context:#?}"
        );
        let helper = row_by_path(
            &owner_context,
            &["return_conflicting_forwarded_function_pointer"],
        );
        assert_eq!(helper.site.arg_count, Some(1));

        let row = row_by_owner_kind_path(
            &context,
            owner,
            CallSiteKind::Dynamic,
            &["return_conflicting_forwarded_function_pointer"],
        );
        assert_dynamic_path_function_candidates(
            row,
            owner,
            &["return_conflicting_forwarded_function_pointer"],
            &expected,
            owner_name,
        );
    }

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

#[test]
fn fixture_context_reads_direct_self_field_function_candidates() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = method_id_by_impl_self_type_exact_name(&db, "DirectSelfFieldDispatcher", "invoke")?;
    let mut expected = vec![
        function_id_by_name(&db, "direct_self_field_local")?,
        function_id_by_name(&db, "direct_self_field_other")?,
    ];
    expected.sort_unstable();

    // tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    // `DirectSelfFieldDispatcher::invoke` calls `(self.call)(self)`.
    // The only explicit struct initializers in the fixture assign
    // `direct_self_field_local` and `direct_self_field_other` directly to the
    // `call` field, so DB context should preserve those finite candidates
    // without admitting a traversal edge.
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "DirectSelfFieldDispatcher::invoke context rows: {context:#?}"
    );
    assert_dynamic_path_function_candidates_with_args(
        &context[0],
        owner,
        &["self", "call"],
        1,
        &expected,
        "DirectSelfFieldDispatcher::invoke",
    );
    let site_id = context[0].site.id;
    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(owner),
        CallContextOptions {
            include_incoming_callers: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        outgoing
            .iter()
            .all(|candidate| candidate.call_site_id != site_id),
        "DirectSelfFieldDispatcher::invoke should preserve ambiguous candidates without admitted traversal edges: {outgoing:#?}"
    );

    for target in &expected {
        let target_context = db.call_context_for_target(*target)?;
        let row = target_context
            .iter()
            .find(|row| row.site.owner_id == owner)
            .unwrap_or_else(|| {
                panic!(
                    "target-centered context missing DirectSelfFieldDispatcher::invoke: {target_context:#?}"
                )
            });
        assert_dynamic_path_function_candidates_with_args(
            row,
            owner,
            &["self", "call"],
            1,
            &expected,
            "DirectSelfFieldDispatcher::invoke target context",
        );
    }

    Ok(())
}
