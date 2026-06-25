use super::*;

#[test]
fn proof_projection_rejects_non_resolved_local_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(51);
    let module = Uuid::from_u128(52);
    let site = Uuid::from_u128(53);
    let target = Uuid::from_u128(54);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (70, 90),
            path: Some(vec!["unknown"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Unresolved", None)?;

    let error = db
        .project_call_proof_facts_for_owner(owner, "bd:test")
        .expect_err("unresolved call site with local targets must reject");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(db.proof_graphrag_context("")?.is_empty());

    Ok(())
}

#[test]
fn proof_projection_rejects_ambiguous_local_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x151);
    let module = Uuid::from_u128(0x152);
    let site = Uuid::from_u128(0x153);
    let target = Uuid::from_u128(0x154);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Method",
            span: (100, 130),
            path: None,
            method: Some("overlap"),
            macro_name: None,
            receiver: Some(("LocalBinding", vec!["value"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Method")?;
    insert_relation(&db, site, target, "Method", "Method", "Method")?;
    insert_status(&db, site, "Method", "Ambiguous", None)?;

    let error = db
        .project_call_proof_facts_for_owner(owner, "bd:test")
        .expect_err("ambiguous call site with local targets must reject");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(db.proof_graphrag_context("")?.is_empty());

    Ok(())
}

#[test]
fn target_centered_proof_projection_rejects_non_resolved_local_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let resolved_owner = Uuid::from_u128(61);
    let resolved_module = Uuid::from_u128(62);
    let unresolved_owner = Uuid::from_u128(63);
    let unresolved_module = Uuid::from_u128(64);
    let target = Uuid::from_u128(65);
    let resolved_site = Uuid::from_u128(66);
    let unresolved_site = Uuid::from_u128(67);

    insert_owner_source(&db, resolved_owner, resolved_module, "src/resolved.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: resolved_site,
            owner: resolved_owner,
            kind: "Path",
            span: (10, 30),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, resolved_owner, resolved_site, "Path")?;
    insert_relation(&db, resolved_site, target, "Function", "Path", "Function")?;
    insert_status(&db, resolved_site, "Path", "Resolved", Some("LocalExact"))?;

    insert_owner_source(
        &db,
        unresolved_owner,
        unresolved_module,
        "src/unresolved.rs",
    )?;
    insert_call_site(
        &db,
        SiteSeed {
            id: unresolved_site,
            owner: unresolved_owner,
            kind: "Path",
            span: (40, 60),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, unresolved_owner, unresolved_site, "Path")?;
    insert_relation(&db, unresolved_site, target, "Function", "Path", "Function")?;
    insert_status(&db, unresolved_site, "Path", "Unresolved", None)?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), 2, "target callers: {callers:#?}");

    let error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection must reject inconsistent incoming rows");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "target-centered projection must not store partial proof facts after rejection"
    );

    Ok(())
}

#[test]
fn target_centered_proof_projection_rejects_ambiguous_local_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x161);
    let module = Uuid::from_u128(0x162);
    let target = Uuid::from_u128(0x163);
    let site = Uuid::from_u128(0x164);

    insert_owner_source(&db, owner, module, "src/ambiguous.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Method",
            span: (140, 170),
            path: None,
            method: Some("overlap"),
            macro_name: None,
            receiver: Some(("LocalBinding", vec!["value"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Method")?;
    insert_relation(&db, site, target, "Method", "Method", "Method")?;
    insert_status(&db, site, "Method", "Ambiguous", None)?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), 1, "target callers: {callers:#?}");

    let error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection must reject ambiguous incoming rows");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "target-centered projection must not store partial proof facts after rejection"
    );

    Ok(())
}
