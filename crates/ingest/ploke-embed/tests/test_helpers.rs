//! Common test helpers for graph tests

 use cozo::{Db, MemStorage};
 use ploke_transform::schema::create_schema_all;

 /// Creates a new in-memory database with the schema initialized
 pub fn setup_test_db() -> Db<MemStorage> {
     let db = Db::new(MemStorage::default()).expect("Failed to create database");
     db.initialize().expect("Failed to initialize database");

     // Create the schema
     create_schema_all(&db).expect("Failed to create schema");

     db
 }

 #[cfg(feature = "debug")]
 pub fn print_debug(message: &str, result: &cozo::NamedRows) {
     println!("\n{:-<50}", "");
     println!("DEBUG: {}", message);
     println!("{:?}", result);
     println!("{:-<50}\n", "");
 }
