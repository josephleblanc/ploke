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

#[test]
fn fixture_projected_call_site_ids_do_not_overlap_node_or_type_ids() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let overlaps = db.raw_query(
        r#"?[call_site_id, relation] :=
            *call_site { id: call_site_id @ 'NOW' },
            stored_non_call_id[call_site_id, relation]

        stored_non_call_id[id, relation] := *function { id @ 'NOW' }, relation = "function"
        stored_non_call_id[id, relation] := *method { id @ 'NOW' }, relation = "method"
        stored_non_call_id[id, relation] := *const { id @ 'NOW' }, relation = "const"
        stored_non_call_id[id, relation] := *static { id @ 'NOW' }, relation = "static"
        stored_non_call_id[id, relation] := *struct { id @ 'NOW' }, relation = "struct"
        stored_non_call_id[id, relation] := *enum { id @ 'NOW' }, relation = "enum"
        stored_non_call_id[id, relation] := *variant { id @ 'NOW' }, relation = "variant"
        stored_non_call_id[id, relation] := *trait { id @ 'NOW' }, relation = "trait"
        stored_non_call_id[id, relation] := *impl { id @ 'NOW' }, relation = "impl"
        stored_non_call_id[id, relation] := *type_alias { id @ 'NOW' }, relation = "type_alias"
        stored_non_call_id[id, relation] := *union { id @ 'NOW' }, relation = "union"
        stored_non_call_id[id, relation] := *module { id @ 'NOW' }, relation = "module"
        stored_non_call_id[id, relation] := *import { id @ 'NOW' }, relation = "import"
        stored_non_call_id[id, relation] := *macro { id @ 'NOW' }, relation = "macro"
        stored_non_call_id[id, relation] := *field { id @ 'NOW' }, relation = "field"
        stored_non_call_id[id, relation] := *generic_type { id @ 'NOW' }, relation = "generic_type"
        stored_non_call_id[id, relation] := *generic_lifetime { id @ 'NOW' }, relation = "generic_lifetime"
        stored_non_call_id[id, relation] := *generic_const { id @ 'NOW' }, relation = "generic_const"
        stored_non_call_id[id, relation] := *file_mod { owner_id: id @ 'NOW' }, relation = "file_mod"
        stored_non_call_id[id, relation] := *type_use { id @ 'NOW' }, relation = "type_use"
        stored_non_call_id[id, relation] := *named_type { type_id: id @ 'NOW' }, relation = "named_type"
        stored_non_call_id[id, relation] := *reference_type { type_id: id @ 'NOW' }, relation = "reference_type"
        stored_non_call_id[id, relation] := *slice_type { type_id: id @ 'NOW' }, relation = "slice_type"
        stored_non_call_id[id, relation] := *array_type { type_id: id @ 'NOW' }, relation = "array_type"
        stored_non_call_id[id, relation] := *tuple_type { type_id: id @ 'NOW' }, relation = "tuple_type"
        stored_non_call_id[id, relation] := *function_type { type_id: id @ 'NOW' }, relation = "function_type"
        stored_non_call_id[id, relation] := *never_type { type_id: id @ 'NOW' }, relation = "never_type"
        stored_non_call_id[id, relation] := *inferred_type { type_id: id @ 'NOW' }, relation = "inferred_type"
        stored_non_call_id[id, relation] := *raw_pointer_type { type_id: id @ 'NOW' }, relation = "raw_pointer_type"
        stored_non_call_id[id, relation] := *trait_object_type { type_id: id @ 'NOW' }, relation = "trait_object_type"
        stored_non_call_id[id, relation] := *impl_trait_type { type_id: id @ 'NOW' }, relation = "impl_trait_type"
        stored_non_call_id[id, relation] := *trait_bound_type { type_id: id @ 'NOW' }, relation = "trait_bound_type"
        stored_non_call_id[id, relation] := *paren_type { type_id: id @ 'NOW' }, relation = "paren_type"
        stored_non_call_id[id, relation] := *macro_type { type_id: id @ 'NOW' }, relation = "macro_type"
        stored_non_call_id[id, relation] := *unknown_type { type_id: id @ 'NOW' }, relation = "unknown_type""#,
    )?;
    assert!(
        overlaps.rows.is_empty(),
        "call_site ids must remain in the CallId universe; overlaps: {:#?}",
        overlaps.rows
    );

    Ok(())
}

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
            is_valid_call_relation_family(relation, source, target_kind),
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
