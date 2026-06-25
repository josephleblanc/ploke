use super::*;

#[test]
fn fixture_projected_call_sites_have_one_matching_body_edge() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let sites = db.raw_query(
        r#"?[id, owner_id, call_kind] :=
            *call_site { id, owner_id, call_kind @ 'NOW' }"#,
    )?;
    assert!(
        !sites.rows.is_empty(),
        "fixture should project call_site rows"
    );

    for site in &sites.rows {
        let site_id = to_uuid(&site[0])?;
        let owner = to_uuid(&site[1])?;
        let call_kind = data_str(&site[2], "call_site.call_kind");
        let owner_kind = owner_kind_for_call_body_owner(&db, owner)?;
        let edges = body_edges_for_site(&db, site_id)?;

        assert_eq!(
            edges.rows.len(),
            1,
            "call_site {site_id} should have exactly one BodyContainsCall edge; rows: {:#?}",
            edges.rows
        );
        let edge = &edges.rows[0];
        assert_eq!(
            to_uuid(&edge[0])?,
            owner,
            "BodyContainsCall source should match call_site.owner_id for {site_id}"
        );
        assert_eq!(
            to_uuid(&edge[1])?,
            site_id,
            "BodyContainsCall target should match call_site.id for {site_id}"
        );
        assert_eq!(
            data_str(&edge[2], "call_site_edge.source_kind"),
            owner_kind,
            "BodyContainsCall source_kind should match owner family for call_site {site_id}"
        );
        assert_eq!(
            data_str(&edge[3], "call_site_edge.target_kind"),
            call_kind,
            "BodyContainsCall target_kind should match call_site.call_kind for {site_id}"
        );
    }

    Ok(())
}
