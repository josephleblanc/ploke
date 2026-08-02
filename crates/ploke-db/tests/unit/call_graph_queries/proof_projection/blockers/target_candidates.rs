use super::*;

#[test]
fn ambiguous_target_centered_projection_stores_candidate_def_ids() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x1a1);
    let module = Uuid::from_u128(0x1a2);
    let site = Uuid::from_u128(0x1a3);
    let target = Uuid::from_u128(0x1a4);
    let sibling = Uuid::from_u128(0x1a5);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Dynamic",
            span: (10, 24),
            path: Some(vec!["selected"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: None,
        },
    )?;
    insert_edge(&db, owner, site, "Dynamic")?;
    insert_relation(&db, site, target, "DynamicFunction", "Dynamic", "Function")?;
    insert_relation(&db, site, sibling, "DynamicFunction", "Dynamic", "Function")?;
    insert_status(&db, site, "Dynamic", "Ambiguous", None)?;

    let facts = db.call_proof_facts_for_target(target, "bd:test")?;
    assert_eq!(
        facts.len(),
        2,
        "ambiguous target-centered facts should emit call_site and call_resolution only: {facts:#?}"
    );

    let resolution = facts
        .iter()
        .find(|fact| {
            fact.get("fact_kind").and_then(serde_json::Value::as_str) == Some("call_resolution")
        })
        .expect("target-centered projection should emit call_resolution");
    assert_eq!(
        resolution
            .get("resolution_state")
            .and_then(serde_json::Value::as_str),
        Some("ambiguous")
    );
    assert_eq!(
        resolution
            .get("blocking_reason")
            .and_then(serde_json::Value::as_str),
        Some("type_resolution_missing")
    );
    assert_candidate_ids(
        resolution
            .get("candidate_def_ids")
            .and_then(serde_json::Value::as_array)
            .expect("target-centered ambiguous projection should preserve candidate_def_ids")
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("candidate_def_ids should contain string IDs")
                    .to_string()
            }),
        &[target, sibling],
    );

    assert_eq!(
        db.project_call_proof_facts_for_target(target, "bd:test")?,
        2
    );
    assert!(db.proof_checker_edges()?.is_empty());

    let rows = db.proof_graphrag_context("")?;
    assert_eq!(
        rows.len(),
        2,
        "stored ambiguous proof should contain call_site and call_resolution only: {rows:#?}"
    );
    let site_id = site.to_string();
    let stored = rows
        .iter()
        .find(|row| {
            row.kind == "call_resolution" && row.call_site_id.as_deref() == Some(site_id.as_str())
        })
        .expect("stored target-centered proof should include call_resolution");
    assert_eq!(stored.resolution_state.as_deref(), Some("ambiguous"));
    assert_eq!(
        stored.blocker_reason.as_deref(),
        Some("type_resolution_missing")
    );
    assert_candidate_ids(stored.candidate_def_ids.iter().cloned(), &[target, sibling]);

    Ok(())
}

fn assert_candidate_ids(actual: impl IntoIterator<Item = String>, expected: &[Uuid]) {
    let mut actual = actual.into_iter().collect::<Vec<_>>();
    actual.sort();
    let mut expected = expected.iter().map(ToString::to_string).collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected, "ambiguous candidate_def_ids");
}
