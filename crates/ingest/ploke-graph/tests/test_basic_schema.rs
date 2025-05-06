//! Tests for basic schema functionality

use cozo::{DataValue, ScriptMutability};
use ploke_graph::schema::{insert_sample_data, verify_schema};
use std::collections::BTreeMap;
use test_helpers::setup_test_db;

mod test_helpers;

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_schema_creation() {
    let db = setup_test_db();

    // Verify the schema was created by listing relations
    let result = db
        .run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)
        .expect("Failed to list relations");

    #[cfg(feature = "debug")]
    test_helpers::print_debug("Relations", &result);

    // Check that we have the expected number of relations
    assert!(result.rows.len() >= 12, "Expected at least 12 relations");

    // Verify we can insert and query data
    insert_sample_data(&db).expect("Failed to insert sample data");
    verify_schema(&db).expect("Failed to verify schema");

    // Test a specific query to ensure data was inserted correctly
    let result = db
        .run_script(
            "?[name] := *functions[_, name, _, _, _]",
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
#[cfg(not(feature = "type_bearing_ids"))]
fn test_indices() {
    let db = setup_test_db();

    // Verify indices were created
    let result = db
        .run_script(
            "::indices functions",
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to list indices");

    #[cfg(feature = "debug")]
    test_helpers::print_debug("Indices for functions", &result);

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
    test_helpers::print_debug("Indices for relations", &result);

    assert!(
        !result.rows.is_empty(),
        "Expected at least one index for relations"
    );
}

fn insert_sample_type_alias(
    db: &cozo::Db<cozo::MemStorage>,
) -> Result<cozo::NamedRows, cozo::Error> {
    let params = BTreeMap::from([
        ("id".to_string(), DataValue::from(10)),
        ("name".to_string(), DataValue::from("StringVec")),
        ("visibility".to_string(), DataValue::from("Public")),
        ("type_id".to_string(), DataValue::from(1)),
        (
            "docstring".to_string(),
            DataValue::from("Type alias for Vec<String>"),
        ),
    ]);

    db.run_script(
        "?[id, name, visibility, type_id, docstring] <- [[$id, $name, $visibility, $type_id, $docstring]] :put type_aliases",
        params,
        ScriptMutability::Mutable,
    )
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_type_alias_insertion() {
    let db = setup_test_db();

    // Insert a sample type alias
    insert_sample_type_alias(&db).expect("Failed to insert type alias");

    // Query to verify insertion
    let result = db
        .run_script(
            "?[name, docstring] := *type_aliases[_, name, _, _, docstring]",
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to query type aliases");

    assert_eq!(result.rows.len(), 1, "Expected 1 type alias");
    assert_eq!(
        result.rows[0][0].get_str(),
        Some("StringVec"),
        "Expected type alias name to be 'StringVec'"
    );
}
