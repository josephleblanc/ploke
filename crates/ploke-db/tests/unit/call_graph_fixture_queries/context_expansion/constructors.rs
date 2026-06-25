use super::*;

#[test]
fn fixture_expand_call_context_target_seed_preserves_constructor_callers() -> Result<(), DbError> {
    for case in constructor_cases() {
        let db = setup_call_graph_fixture_db(case.fixture)?;
        let resolved = assert_constructor_context(&db, case)?;
        assert_constructor_callers(&db, case, &resolved)?;

        let candidates = db.expand_call_context(
            CallContextSeed::Target(resolved.target),
            CallContextOptions {
                include_outgoing_targets: false,
                max_candidates: 128,
                ..CallContextOptions::default()
            },
        )?;
        assert_call_candidate(
            &candidates,
            resolved.owner,
            CallContextRelation::IncomingCaller,
            resolved.site,
            resolved.target,
            &format!("{} incoming candidate missing", case.label),
        );
    }

    Ok(())
}
