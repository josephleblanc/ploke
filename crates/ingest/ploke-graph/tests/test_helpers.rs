//! Common test helpers for graph tests

use cozo::{Db, MemStorage};
use ploke_graph::schema::create_schema;
use std::path::Path;
use syn_parser::analyze_code;
use syn_parser::CodeGraph;

// Allow this as it is used in tests and we get warnings otherwise.
// Is there a better way to indicate that it is actually being used?
#[allow(dead_code)]
/// Creates a new in-memory database with the schema initialized
pub fn setup_test_db() -> Db<MemStorage> {
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");

    // Create the schema
    create_schema(&db).expect("Failed to create schema");

    db
}

#[cfg(feature = "debug")]
pub fn print_debug(message: &str, result: &cozo::NamedRows) {
    println!("\n{:-<50}", "");
    println!("DEBUG: {}", message);
    println!("{:?}", result);
    println!("{:-<50}\n", "");
}

// Allow this as it is used in tests and we get warnings otherwise.
// Is there a better way to indicate that it is actually being used?
#[allow(dead_code)]
/// Parse a fixture file and return the resulting CodeGraph
pub fn parse_fixture(fixture_name: &str) -> CodeGraph {
    let path = Path::new("../syn_parser/tests/fixtures").join(fixture_name);
    analyze_code(&path).expect("Failed to parse fixture")
}
