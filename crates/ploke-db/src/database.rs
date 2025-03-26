use crate::error::Error;
use crate::result::QueryResult;
use cozo::Db;
use cozo::MemStorage;

/// Main database connection and query interface
#[derive(Debug)]
pub struct Database {
    db: Db<MemStorage>,
}

impl Database {
    /// Create new database connection
    pub fn new(db: Db<MemStorage>) -> Self {
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
