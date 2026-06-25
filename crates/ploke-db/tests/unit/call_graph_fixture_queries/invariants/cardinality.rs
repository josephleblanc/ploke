use super::*;

#[test]
fn fixture_projected_status_rows_match_relation_cardinality() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let sites = db.raw_query(
        r#"?[id, call_kind] :=
            *call_site { id, call_kind @ 'NOW' }"#,
    )?;
    assert!(
        !sites.rows.is_empty(),
        "fixture should project call_site rows"
    );

    let mut resolved_count = 0;
    let mut targetless_count = 0;
    let mut candidate_count = 0;

    for site in &sites.rows {
        let site_id = to_uuid(&site[0])?;
        let call_kind = data_str(&site[1], "call_site.call_kind");
        let statuses = statuses_for_site(&db, site_id)?;
        assert_eq!(
            statuses.rows.len(),
            1,
            "call_site {site_id} should have exactly one call_resolution_status row; rows: {:#?}",
            statuses.rows
        );

        let status = &statuses.rows[0];
        let source = data_str(&status[0], "call_resolution_status.source_kind");
        let status_kind = data_str(&status[1], "call_resolution_status.status_kind");
        let resolution = optional_data_str(&status[2], "call_resolution_status.resolution_kind");
        let relations = relations_for_site(&db, site_id)?;

        assert_eq!(
            source, call_kind,
            "call_resolution_status source_kind should match call_site.call_kind for {site_id}"
        );
        assert_valid_status_shape(site_id, status_kind, resolution);

        match status_kind {
            "Resolved" => {
                resolved_count += 1;
                assert_eq!(
                    relations.rows.len(),
                    1,
                    "resolved call_site {site_id} should have exactly one call_relation row; rows: {:#?}",
                    relations.rows
                );
            }
            "Ambiguous" if !relations.rows.is_empty() => {
                candidate_count += 1;
                assert_eq!(
                    call_kind, "Dynamic",
                    "candidate-bearing ambiguous call_site {site_id} should be dynamic"
                );
                for relation in &relations.rows {
                    let target = to_uuid(&relation[0])?;
                    let relation_kind = data_str(&relation[1], "call_relation.relation_kind");
                    let source_kind = data_str(&relation[2], "call_relation.source_kind");
                    let target_kind = data_str(&relation[3], "call_relation.target_kind");
                    assert_eq!(relation_kind, "DynamicFunction");
                    assert_eq!(source_kind, "Dynamic");
                    assert_eq!(target_kind, "Function");
                    assert!(
                        call_target_exists(&db, target, target_kind)?,
                        "ambiguous dynamic candidate target {target} should exist in {target_kind} endpoint relation"
                    );
                }
            }
            "Unresolved" | "Ambiguous" | "External" | "Unsupported" => {
                targetless_count += 1;
                assert!(
                    relations.rows.is_empty(),
                    "non-resolved call_site {site_id} must not have call_relation rows; rows: {:#?}",
                    relations.rows
                );
            }
            other => panic!("unexpected call status kind {other} for {site_id}"),
        }
    }

    assert!(
        resolved_count > 0,
        "fixture should include resolved call sites"
    );
    assert!(
        targetless_count > 0,
        "fixture should include non-resolved call sites"
    );
    assert!(
        candidate_count > 0,
        "fixture should include candidate-bearing ambiguous call sites"
    );

    Ok(())
}
