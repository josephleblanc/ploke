//! Tests for database queries

use ploke_db::Database;
use ploke_db::Error;
use ploke_graph::schema::{create_schema, insert_sample_data};

mod test_helpers;

#[test]
fn test_find_type_usages() -> Result<(), Error> {
    // Setup test database
    let db = test_helpers::setup_test_db();
    insert_sample_data(&db).expect("Failed to insert sample data");

    // Create our database wrapper
    let ploke_db = Database::new(db);

    // Execute the query
    let query = r#"
        ?[fn_name, type_str] := 
            *functions[fn_id, fn_name, _, _, _, _],
            *function_params[fn_id, _, _, type_id, _, _],
            *types[type_id, _, type_str]
    "#;

    let result = ploke_db.raw_query(query)?;

    // We should have at least one function using a type
    assert!(!result.rows.is_empty(), "Expected at least one type usage");

    Ok(())
}
