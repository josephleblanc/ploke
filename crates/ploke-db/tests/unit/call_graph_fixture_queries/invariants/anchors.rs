use super::*;

#[test]
fn fixture_projected_call_relations_and_statuses_anchor_to_call_sites() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let sites = db.raw_query(
        r#"?[id, call_kind] :=
            *call_site { id, call_kind @ 'NOW' }"#,
    )?;
    assert!(
        !sites.rows.is_empty(),
        "fixture should project call_site rows"
    );

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
        assert_eq!(
            data_str(&statuses.rows[0][0], "call_resolution_status.source_kind"),
            call_kind,
            "call_resolution_status source_kind should match call_site.call_kind for {site_id}"
        );
    }

    let relations = db.raw_query(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            *call_relation { source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW' }"#,
    )?;
    assert!(
        !relations.rows.is_empty(),
        "fixture should project resolved call_relation rows"
    );

    for row in &relations.rows {
        let site_id = to_uuid(&row[0])?;
        let target = to_uuid(&row[1])?;
        let relation = data_str(&row[2], "call_relation.relation_kind");
        let source = data_str(&row[3], "call_relation.source_kind");
        let target_kind = data_str(&row[4], "call_relation.target_kind");
        let site_kind = call_site_kind_for_site(&db, site_id)?;

        assert_eq!(
            source, site_kind,
            "call_relation source_kind should match call_site.call_kind for {site_id}"
        );
        assert!(
            valid_call_target_family(relation, source, target_kind),
            "unexpected call_relation endpoint family for {site_id} -> {target}: {relation}/{source}/{target_kind}"
        );
        assert!(
            call_target_exists(&db, target, target_kind)?,
            "call_relation target {target} should exist in {target_kind} endpoint relation"
        );
    }

    Ok(())
}

#[test]
fn fixture_projected_call_edge_and_status_rows_have_existing_anchors() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let edges = db.raw_query(
        r#"?[source_id, target_id, source_kind, target_kind] :=
            *call_site_edge {
                source_id,
                target_id,
                relation_kind: "BodyContainsCall",
                source_kind,
                target_kind @ 'NOW'
            }"#,
    )?;
    assert!(
        !edges.rows.is_empty(),
        "fixture should project BodyContainsCall rows"
    );

    for edge in &edges.rows {
        let owner = to_uuid(&edge[0])?;
        let site_id = to_uuid(&edge[1])?;
        let source = data_str(&edge[2], "call_site_edge.source_kind");
        let target = data_str(&edge[3], "call_site_edge.target_kind");
        let owner_kind = owner_kind_for_call_body_owner(&db, owner)?;
        let (site_owner, call_kind) = call_site_owner_and_kind(&db, site_id)?;

        assert_eq!(
            site_owner, owner,
            "BodyContainsCall target {site_id} should be owned by source {owner}"
        );
        assert_eq!(
            source, owner_kind,
            "BodyContainsCall source_kind should match owner family for edge {owner} -> {site_id}"
        );
        assert_eq!(
            target, call_kind,
            "BodyContainsCall target_kind should match call_site.call_kind for {site_id}"
        );
    }

    let statuses = db.raw_query(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            *call_resolution_status {
                source_id,
                source_kind,
                status_kind,
                resolution_kind @ 'NOW'
            }"#,
    )?;
    assert!(
        !statuses.rows.is_empty(),
        "fixture should project call_resolution_status rows"
    );

    for status in &statuses.rows {
        let site_id = to_uuid(&status[0])?;
        let source = data_str(&status[1], "call_resolution_status.source_kind");
        let status_kind = data_str(&status[2], "call_resolution_status.status_kind");
        let resolution = optional_data_str(&status[3], "call_resolution_status.resolution_kind");
        let (_owner, call_kind) = call_site_owner_and_kind(&db, site_id)?;

        assert_eq!(
            source, call_kind,
            "call_resolution_status source_kind should match call_site.call_kind for {site_id}"
        );
        assert_valid_status_shape(site_id, status_kind, resolution);
    }

    Ok(())
}
