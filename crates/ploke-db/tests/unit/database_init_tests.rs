use crate::common::test_helpers::setup_test_db; // Uses the function
use ploke_db::Database;

#[test]
fn test_schema_creation_success() {
    // Setup database using test helper
    let db = setup_test_db();

    // Query to check existence of system tables
    let script = "::relations";
    let result = db.run_script(
        script,
        Default::default(),
        cozo::ScriptMutability::Immutable,
    );

    assert!(result.is_ok(), "Schema creation failed");
}

#[test]
#[ignore = "outdated test needs update"]
// NOTE: This test is helpful but needs to be updated
fn test_hnsw_initialization() {
    let db = Database::init_with_schema().expect("Failed to initialize database with schema");

    // Test if HNSW index was created
    let script = "?[type, name, access] ::index";
    let result = db.run_script(
        script,
        Default::default(),
        cozo::ScriptMutability::Immutable,
    );

    assert!(result.is_ok(), "HNSW index query failed");
    let result = result.unwrap();

    // Look for our embedding index
    let found_embedding_index = result.rows.iter().any(|row| {
        row.len() >= 3 &&
        // NOTE: Problem is the embedding_nodes area here
        row[1].get_str() == Some("embedding_nodes:embedding_idx")
    });

    assert!(found_embedding_index, "Embedding index not created");
}

#[test]
fn test_reinitialization_handling() {
    // Try to re-initialize the database
    let db = Database::init_with_schema();

    assert!(db.is_ok(), "Re-initialization should not cause errors");
}

#[test]
fn test_function_schema_creation_script() {
    use cozo::{Db, MemStorage, ScriptMutability};
    use ploke_transform::schema::primary_nodes::FunctionNodeSchema;
    use std::collections::BTreeMap;

    let db = Db::new(MemStorage::default()).unwrap();
    db.initialize().unwrap();

    let schema = FunctionNodeSchema::SCHEMA;
    let script = schema.script_create();
    println!("Generated FunctionNodeSchema script:\n{}", script);

    let result = db.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable);
    assert!(
        result.is_ok(),
        "Failed to create FunctionNodeSchema: {:?}",
        result.err()
    );
}
