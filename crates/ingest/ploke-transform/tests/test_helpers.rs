//! Common test helpers for graph tests
#![allow(unused_imports, dead_code)]

use cozo::NamedRows;
use cozo::{DataValue, Db, MemStorage};
// use ploke_transform::schema::create_schema;
use std::collections::BTreeMap;
use std::path::Path;
use syn_parser::CodeGraph;

/// Creates a new in-memory database with the schema initialized
#[allow(dead_code)]
pub fn setup_test_db() -> Db<MemStorage> {
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");

    // Create the schema
    // create_schema(&db).expect("Failed to create schema");

    db
}

/// Helper to insert a visibility record
#[allow(dead_code)]
pub fn insert_visibility(
    db: &Db<MemStorage>,
    node_id: i64,
    kind: &str,
    path: Option<Vec<&str>>,
) -> Result<NamedRows, cozo::Error> {
    let path_value = path.map(|p| DataValue::List(p.iter().map(|s| DataValue::from(*s)).collect()));

    let params = BTreeMap::from([
        ("node_id".to_string(), DataValue::from(node_id)),
        ("kind".to_string(), DataValue::from(kind)),
        ("path".to_string(), path_value.unwrap_or(DataValue::Null)),
    ]);

    db.run_script(
        "?[node_id, kind, path] <- [[$node_id, $kind, $path]] :put visibility",
        params,
        cozo::ScriptMutability::Mutable,
    )
}

/// Helper to verify a visibility record exists
#[allow(dead_code)]
pub fn verify_visibility(
    db: &Db<MemStorage>,
    node_id: i64,
    expected_kind: &str,
    expected_path: Option<Vec<&str>>,
) -> bool {
    let query = "?[kind, path] := *visibility[node_id, kind, path]";
    let params = BTreeMap::from([("node_id".to_string(), DataValue::from(node_id))]);

    let result = db
        .run_script(query, params, cozo::ScriptMutability::Immutable)
        .expect("Query failed");

    if result.rows.is_empty() {
        return false;
    }

    let row = &result.rows[0];
    let kind = row[0].get_str().unwrap();
    let path = match &row[1] {
        DataValue::List(p) => Some(p.iter().map(|v| v.get_str().unwrap()).collect::<Vec<_>>()),
        _ => None,
    };

    kind == expected_kind && path == expected_path.map(|p| p.to_vec())
}

#[cfg(feature = "debug")]
#[allow(dead_code)]
pub fn print_debug(message: &str, result: &cozo::NamedRows) {
    println!("\n{:-<50}", "");
    println!("DEBUG: {}", message);
    for row in result.clone().into_iter() {
        println!("{:?}", row);
    }
    println!("{:-<50}\n", "");
}

// //  Parse a fixture file and return the resulting CodeGraph
// #[allow(dead_code)]
// pub fn parse_fixture(fixture_name: &str) -> CodeGraph {
//     #[cfg(feature = "debug")]
//     println!("parsing fixture: fixture_name");
//     let path = Path::new("../syn_parser/tests/fixtures").join(fixture_name);
//     #[cfg(feature = "debug")]
//     println!(
//         "parsing file path: {}",
//         path.to_str().expect("invalid file path")
//     );
//     #[cfg(feature = "debug")]
//     println!(
//         "parsing target file path exists: {}",
//         path.try_exists()
//             .expect("file path existance cannot be confirmed as true or false")
//     );
//     analyze_code(&path).expect("Failed to parse fixture")
// }
