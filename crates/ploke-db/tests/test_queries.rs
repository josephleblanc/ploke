//! Tests for database queries

use ploke_db::{Database, QueryBuilder};
use ploke_db::Error;
use ploke_graph::schema::{create_schema, insert_sample_data};

mod test_helpers;

#[test]
fn test_basic_function_query() -> Result<(), Error> {
    let db = test_helpers::setup_test_db();
    insert_sample_data(&db).expect("Failed to insert sample data");
    let ploke_db = Database::new(db);

    // Find sample_function by name
    let result = QueryBuilder::new(ploke_db.db.clone())
        .functions()
        .with_name("sample_function")
        .execute()?;

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.headers, vec!["id", "name", "visibility", "docstring"]);
    assert_eq!(result.rows[0][1].get_str(), Some("sample_function"));

    Ok(())
}

#[test]
fn test_basic_struct_query() -> Result<(), Error> {
    let db = test_helpers::setup_test_db();
    insert_sample_data(&db).expect("Failed to insert sample data");
    let ploke_db = Database::new(db);

    // Find SampleStruct by name
    let result = QueryBuilder::new(ploke_db.db.clone())
        .structs()
        .with_name("SampleStruct")
        .execute()?;

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.headers, vec!["id", "name", "visibility", "docstring"]);
    assert_eq!(result.rows[0][1].get_str(), Some("SampleStruct"));

    Ok(())
}
