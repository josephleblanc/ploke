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

crate_test_transform!(test_transform_syn, "ingest/syn_parser");
crate_test_transform!(test_transform_self, "ingest/ploke-transform");
crate_test_transform!(test_transform_embed, "ingest/ploke-embed");
crate_test_transform!(test_transform_core, "ploke-core");
crate_test_transform!(test_transform_db, "ploke-db");
crate_test_transform!(test_transform_error, "ploke-error");
crate_test_transform!(test_transform_io, "ploke-io");
crate_test_transform!(test_transform_rag, "ploke-rag");
crate_test_transform!(test_transform_tui, "ploke-tui");
crate_test_transform!(test_transform_ty_mcp, "ploke-ty-mcp");
crate_test_transform!(test_transform_, "test-utils");
// crate_test_transform!(test_transform_, "ploke-");

// NOTE: Keeping this commented out as refernce for the macro above for sanity checks should
// something fail.
//
// #[test]
// fn test_transform_syn() -> Result<(), ploke_error::Error> {
//     // let _ = init_tracing_v2();
//     // initialize db
//     let db = Db::new(MemStorage::default()).expect("Failed to create database");
//     db.initialize().expect("Failed to initialize database");
//     // create and insert schema for all nodes
//     create_schema_all(&db)?;
//     let (merged, tree) = parse_and_build_tree("ingest/syn_parser")?; // Use workspace root for context
//     transform_parsed_graph(&db, merged, &tree)?;
//
//     Ok(())
// }

