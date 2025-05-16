//! Test helpers for ploke-db

use cozo::Db;
use cozo::MemStorage;
use ploke_transform::schema::create_schema_all;

/// Creates a new in-memory database with the schema initialized
pub fn setup_test_db() -> Db<MemStorage> {
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");

    // Create the schema
    create_schema_all(&db).expect("Failed to create schema");

    db
}
