use super::*;

#[test]
fn has_call_graph_relations_requires_populated_projection() -> Result<(), DbError> {
    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let empty = Database::new(raw);
    assert!(
        !empty.has_call_graph_relations()?,
        "empty database should not report call-graph relations"
    );

    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    assert!(
        !db.has_call_graph_relations()?,
        "schema-only database should not report populated call-graph projection"
    );

    let owner = Uuid::from_u128(0x5ca1);
    let site = Uuid::from_u128(0x5ca2);
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 20),
            path: Some(vec!["crate", "missing"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_status(&db, site, "Path", "Unresolved", None)?;
    assert!(
        db.has_call_graph_relations()?,
        "coherent call_site, BodyContainsCall edge, and status rows should report populated call-graph projection"
    );

    Ok(())
}
