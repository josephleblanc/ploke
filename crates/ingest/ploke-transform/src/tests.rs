#![cfg(test)]

use cozo::{Db, MemStorage};
use ploke_test_utils::{init_tracing_v2, parse_and_build_tree};

use crate::{schema::create_schema_all, transform::transform_parsed_graph};

#[test]
fn test_transform_syn() -> Result<(), ploke_error::Error> {
    let _ = init_tracing_v2();
    // initialize db
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    // create and insert schema for all nodes
    create_schema_all(&db)?;
    let (merged, tree) = parse_and_build_tree("ingest/syn_parser")?; // Use workspace root for context
    transform_parsed_graph(&db, merged, &tree)?;

    Ok(())
}
