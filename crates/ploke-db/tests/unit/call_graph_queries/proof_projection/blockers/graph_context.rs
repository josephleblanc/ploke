use super::*;

#[derive(Clone, Copy)]
struct GraphSeed {
    owner: Uuid,
    module: Uuid,
    site: Uuid,
    target: Uuid,
}

#[test]
fn proof_graphrag_context_links_generated_call_facts_by_call_site() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let seed = GraphSeed {
        owner: Uuid::from_u128(0x181),
        module: Uuid::from_u128(0x182),
        site: Uuid::from_u128(0x183),
        target: Uuid::from_u128(0x184),
    };

    insert_resolved_graph(&db, seed)?;
    assert_eq!(
        db.project_call_proof_facts_for_owner(seed.owner, "bd:test")?,
        3
    );

    let rows = db.proof_graphrag_context(&seed.target.to_string())?;
    assert_eq!(
        rows.len(),
        3,
        "callee id query should return the matching edge plus linked call_site and call_resolution rows: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.call_site_id.as_deref() == Some(seed.site.to_string().as_str())
                && row.caller_def_id.as_deref() == Some(seed.owner.to_string().as_str())
        }),
        "linked call_site row missing: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some(seed.site.to_string().as_str())
                && row.callee_def_id.as_deref() == Some(seed.target.to_string().as_str())
        }),
        "matching call_edge row missing: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(seed.site.to_string().as_str())
                && row.detail.is_none()
                && row.blocker_reason.is_none()
        }),
        "linked resolved call_resolution row missing: {rows:#?}"
    );

    Ok(())
}

#[test]
fn proof_domain_context_links_generated_call_facts_by_build_domain() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let primary = GraphSeed {
        owner: Uuid::from_u128(0x191),
        module: Uuid::from_u128(0x192),
        site: Uuid::from_u128(0x193),
        target: Uuid::from_u128(0x194),
    };
    let other = GraphSeed {
        owner: Uuid::from_u128(0x195),
        module: Uuid::from_u128(0x196),
        site: Uuid::from_u128(0x197),
        target: Uuid::from_u128(0x198),
    };

    insert_resolved_graph(&db, primary)?;
    insert_resolved_graph(&db, other)?;
    assert_eq!(
        db.project_call_proof_facts_for_owner(primary.owner, "bd:test")?,
        3
    );
    assert_eq!(
        db.project_call_proof_facts_for_owner(other.owner, "bd:other")?,
        3
    );

    let rows = db.proof_domain_context("bd:test")?;
    assert_eq!(
        rows.len(),
        3,
        "build-domain query should return direct call_site facts plus linked edge and resolution rows: {rows:#?}"
    );
    for kind in ["call_site", "call_edge", "call_resolution"] {
        assert!(
            rows.iter().any(|row| {
                row.kind == kind
                    && row.call_site_id.as_deref() == Some(primary.site.to_string().as_str())
            }),
            "{kind} row missing from build-domain context: {rows:#?}"
        );
    }
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.call_site_id.as_deref() == Some(primary.site.to_string().as_str())
                && row.build_domain_id.as_deref() == Some("bd:test")
        }),
        "matched call_site row should expose its build domain: {rows:#?}"
    );
    assert!(
        rows.iter()
            .all(|row| row.call_site_id.as_deref() != Some(other.site.to_string().as_str())),
        "unrelated build-domain rows leaked into proof domain context: {rows:#?}"
    );

    let error = db
        .proof_domain_context("")
        .expect_err("proof domain lookup requires a concrete build domain");
    assert!(
        error
            .to_string()
            .contains("requires non-empty build_domain_id"),
        "unexpected empty-domain error: {error}"
    );

    Ok(())
}

fn insert_resolved_graph(db: &Database, seed: GraphSeed) -> Result<(), DbError> {
    insert_owner_source(db, seed.owner, seed.module, "src/lib.rs")?;
    insert_call_site(
        db,
        SiteSeed {
            id: seed.site,
            owner: seed.owner,
            kind: "Path",
            span: (10, 24),
            path: Some(vec!["crate", "helper"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(db, seed.owner, seed.site, "Path")?;
    insert_relation(db, seed.site, seed.target, "Function", "Path", "Function")?;
    insert_status(db, seed.site, "Path", "Resolved", Some("LocalExact"))?;
    Ok(())
}
