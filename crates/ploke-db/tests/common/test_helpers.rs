//! Test helpers for ploke-db

use ploke_db::Database;

/// Creates a new database with HNSW index initialized
#[allow(dead_code)]
pub fn setup_test_db_with_index() -> Database {
    Database::init_with_schema().expect("Failed to initialize database")
}
