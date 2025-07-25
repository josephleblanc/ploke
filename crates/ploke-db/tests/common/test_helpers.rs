//! Test helpers for ploke-db

use cozo::Db;
use cozo::MemStorage;
use ploke_db::Database;
use ploke_transform::schema::create_schema_all;

/// Creates a new in-memory database with the schema initialized
pub fn setup_test_db() -> Db<MemStorage> {
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");

    // Create the schema
    create_schema_all(&db).expect("Failed to create schema");

    db
}

/// Creates a new database with HNSW index initialized
#[allow(dead_code)]
pub fn setup_test_db_with_index() -> Database {
    Database::init_with_schema().expect("Failed to initialize database")
}
