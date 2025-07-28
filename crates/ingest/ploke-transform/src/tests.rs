#![cfg(test)]

use cozo::{Db, MemStorage};
use ploke_test_utils::parse_and_build_tree;

use crate::{schema::create_schema_all, transform::transform_parsed_graph};

macro_rules! crate_test_transform {
    ($test_name:ident, $crate_name:expr) => {
        #[test]
        pub fn $test_name() -> Result<(), ploke_error::Error> {
            // initialize db
            let db = Db::new(MemStorage::default()).expect("Failed to create database");
            db.initialize().expect("Failed to initialize database");

            // create and insert schema for all nodes
            create_schema_all(&db)?;
            let (merged, tree) = parse_and_build_tree($crate_name)?; // Use workspace root for context
            transform_parsed_graph(&db, merged, &tree)?;

            Ok(())
        }
    }
}

crate_test_transform!(test_transform_syn_new, "ingest/syn_parser");

#[test]
fn test_transform_syn() -> Result<(), ploke_error::Error> {
    // let _ = init_tracing_v2();
    // initialize db
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    // create and insert schema for all nodes
    create_schema_all(&db)?;
    let (merged, tree) = parse_and_build_tree("ingest/syn_parser")?; // Use workspace root for context
    transform_parsed_graph(&db, merged, &tree)?;

    Ok(())
}

crate_test_transform!(test_transform_self_new, "ingest/ploke-transform");
#[test]
fn test_transform_self() -> Result<(), ploke_error::Error> {
    // let _ = init_tracing_v2();
    // initialize db
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    // create and insert schema for all nodes
    create_schema_all(&db)?;
    let (merged, tree) = parse_and_build_tree("ingest/ploke-transform")?; // Use workspace root for context
    transform_parsed_graph(&db, merged, &tree)?;

    Ok(())
}

crate_test_transform!(test_transform_embed_new, "ingest/ploke-embed");
#[test]
fn test_transform_embed() -> Result<(), ploke_error::Error> {
    // let _ = init_tracing_v2();
    // initialize db
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    // create and insert schema for all nodes
    create_schema_all(&db)?;
    let (merged, tree) = parse_and_build_tree("ingest/ploke-embed")?; // Use workspace root for context
    transform_parsed_graph(&db, merged, &tree)?;

    Ok(())
}

crate_test_transform!(test_transform_core_new, "ploke-core");
#[test]
fn test_transform_core() -> Result<(), ploke_error::Error> {
    // let _ = init_tracing_v2();
    // initialize db
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    // create and insert schema for all nodes
    create_schema_all(&db)?;
    let (merged, tree) = parse_and_build_tree("ploke-core")?; // Use workspace root for context
    transform_parsed_graph(&db, merged, &tree)?;

    Ok(())
}
