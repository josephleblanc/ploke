//! High-performance text retrieval from code graph database

mod database;
mod error;

pub use database::Database;
pub use error::Error;

/// Core query interface
pub mod query {
    pub use super::query::*;
}

/// Result handling and formatting  
pub mod result {
    pub use super::result::*;
}

/// Source location tracking
pub mod span {
    pub use super::span::*;
}

#[derive(Debug)]
pub struct Database {
    db: cozo::Db<cozo::MemStorage>,
}

impl Database {
    /// Create new database connection
    pub fn new(db: cozo::Db<cozo::MemStorage>) -> Self {
        Self { db }
    }

    /// Execute a raw CozoScript query
    pub fn raw_query(&self, script: &str) -> Result<QueryResult, Error> {
        let result = self
            .db
            .run_script(
                script,
                std::collections::BTreeMap::new(),
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| Error::Cozo(e.to_string()))?;
        Ok(QueryResult::from(result))
    }

    /// Create a new query builder
    pub fn query(&self) -> QueryBuilder {
        QueryBuilder::new(self.db.clone())
    }
}
