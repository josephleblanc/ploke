use cozo::{Db, MemStorage, ScriptMutability};
use graph::schema::{create_schema, insert_sample_data, verify_schema};
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

    println!("Relations: {:?}", result);

    // Check that we have the expected number of relations
    assert!(result.rows.len() >= 12, "Expected at least 12 relations");

    // Verify we can insert and query data
    insert_sample_data(&db).expect("Failed to insert sample data");
    verify_schema(&db).expect("Failed to verify schema");

    // Test a specific query to ensure data was inserted correctly
    let result = db
        .run_script(
            "?[name] := *functions[_, name, _]",
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to query functions");

    assert_eq!(result.rows.len(), 1, "Expected 1 function");
    assert!(
        result.rows[0][0].to_string().contains("sample_function"),
        "Expected function name to be 'sample_function'"
    );
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

    println!("Indices for functions: {:?}", result);
    assert!(!result.rows.is_empty(), "Expected at least one index for functions");

    // Check relations indices
    let result = db
        .run_script(
            "::indices relations",
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to list indices");

    println!("Indices for relations: {:?}", result);
    assert!(!result.rows.is_empty(), "Expected at least one index for relations");
}
