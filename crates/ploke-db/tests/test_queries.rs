//! Tests for database queries
#![cfg(feature = "type_bearing_ids")]
use ploke_db::CodeSnippet;
use ploke_db::Database;
use ploke_db::Error;
// use ploke_db::query::QueryBuilder;
// use ploke_db::result::CodeSnippet;
// use ploke_db::error::Error;
use ploke_transform::schema::{create_schema, insert_sample_data};

mod test_helpers;

#[test]
fn test_basic_function_query() -> Result<(), Error> {
    let db = test_helpers::setup_test_db();
    insert_sample_data(&db).expect("Failed to insert sample data");
    let ploke_db = Database::new(db);

    // Find sample_function by name
    let result = ploke_db.raw_query(
        r#"?[id, name, visibility, return_type_id, docstring, body] := 
            *functions[id, name, visibility, return_type_id, docstring, body],
            name = 'sample_function'"#,
    )?;

    let snippets: Vec<CodeSnippet> = result.into_snippets()?;
    assert_eq!(snippets.len(), 1);
    assert_eq!(snippets[0].text, "sample_function");

    Ok(())
}

#[test]
fn test_basic_struct_query() -> Result<(), Error> {
    let db = test_helpers::setup_test_db();
    insert_sample_data(&db).expect("Failed to insert sample data");
    let ploke_db = Database::new(db);

    // Find SampleStruct by name
    let result = ploke_db
        .query()
        .structs()
        .with_name("SampleStruct")
        .execute()?;

    let snippets: Vec<CodeSnippet> = result.into_snippets()?;
    assert_eq!(snippets.len(), 1);
    assert_eq!(snippets[0].text, "SampleStruct");

    Ok(())
}
