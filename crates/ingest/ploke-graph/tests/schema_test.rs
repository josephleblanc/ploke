use cozo::{Db, MemStorage, ScriptMutability};
use ploke_graph::schema::{create_schema, insert_sample_data, verify_schema};
use std::collections::BTreeMap;

#[test]
fn test_schema_creation() {
    // Create an in-memory database
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");

    // Create the schema
    create_schema(&db).expect("Failed to create schema");

    // Verify the schema was created by listing relations
    let result = db
        .run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)
        .expect("Failed to list relations");

    #[cfg(feature = "debug")]
    println!("Relations: {:?}", result);

    // Check that we have the expected relations including visibility
    let relation_names: Vec<_> = result.rows.iter().map(|r| r[0].get_str().unwrap()).collect();
    assert!(relation_names.contains(&"visibility"), "Missing visibility relation");
    assert!(relation_names.len() >= 13, "Expected at least 13 relations");

    // Verify visibility index exists
    let indices = db
        .run_script(
            "::indices visibility",
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to list indices");
    assert!(!indices.rows.is_empty(), "Expected visibility index");

    // Test visibility insertion and querying
    insert_visibility(&db, 100, "public", None).expect("Failed to insert public visibility");
    insert_visibility(&db, 101, "restricted", Some(vec!["super", "module"]))
        .expect("Failed to insert restricted visibility");

    assert!(verify_visibility(&db, 100, "public", None), "Public visibility check failed");
    assert!(
        verify_visibility(&db, 101, "restricted", Some(vec!["super", "module"])),
        "Restricted visibility check failed"
    );

    // Verify we can insert and query sample data
    insert_sample_data(&db).expect("Failed to insert sample data");
    verify_schema(&db).expect("Failed to verify schema");

    // Test a query joining functions and visibility
    let result = db
        .run_script(
            r#"
            ?[fn_name, vis_kind] := 
                *functions[fn_id, fn_name, _, _, _, _],
                *visibility[fn_id, vis_kind, _]
            "#,
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to query functions with visibility");

    assert!(!result.rows.is_empty(), "Expected functions with visibility");
}

#[test]
fn test_indices() {
    // Create an in-memory database
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");

    // Create the schema (which includes indices)
    create_schema(&db).expect("Failed to create schema");

    // Verify indices were created
    let result = db
        .run_script(
            "::indices functions",
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to list indices");

    #[cfg(feature = "debug")]
    println!("Indices for functions: {:?}", result);
    assert!(
        !result.rows.is_empty(),
        "Expected at least one index for functions"
    );

    // Check relations indices
    let result = db
        .run_script(
            "::indices relations",
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to list indices");

    #[cfg(feature = "debug")]
    println!("Indices for relations: {:?}", result);
    assert!(
        !result.rows.is_empty(),
        "Expected at least one index for relations"
    );
}
