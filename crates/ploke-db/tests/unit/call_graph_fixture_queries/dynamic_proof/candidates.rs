use super::*;

#[test]
fn fixture_projection_marks_real_branch_and_match_dynamic_ambiguity_with_candidates()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;
    let expected_names = candidate_strings(&expected);

    for owner_name in AMBIGUOUS_DYNAMIC_OWNERS {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = &context[0];
        assert_dynamic_candidates(row, owner, &expected, owner_name);

        let site = row.site.id.to_string();
        let facts = db.call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_candidate_proof(&facts, &site, &expected_names, owner_name);

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 2, "{owner_name} projected proof fact count");
        assert_candidate_blocker(&db, &site, owner_name)?;

        let target_facts = db
            .call_proof_facts_for_target(expected[0], "bd:fixture-call-graph")?
            .into_iter()
            .filter(|fact| {
                fact.get("call_site_id").and_then(serde_json::Value::as_str) == Some(site.as_str())
            })
            .collect::<Vec<_>>();
        assert_candidate_proof(
            &target_facts,
            &site,
            &expected_names,
            &format!("{owner_name} target-centered"),
        );
    }

    Ok(())
}
