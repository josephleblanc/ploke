use crate::error::DbError;
use crate::QueryBuilder;
use crate::QueryResult;
use cozo::Db;
use cozo::MemStorage;

/// Main database connection and query interface
#[derive(Debug)]
pub struct Database {
    db: Db<MemStorage>,
}

impl std::ops::Deref for Database {
    type Target = Db<MemStorage>;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

impl Database {
    /// Create new database connection
    pub fn new(db: Db<MemStorage>) -> Self {
        Self { db }
    }

    /// Execute a raw CozoScript query
    pub fn raw_query(&self, script: &str) -> Result<QueryResult, DbError> {
        let result = self
            .db
            .run_script(
                script,
                std::collections::BTreeMap::new(),
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;
        Ok(QueryResult::from(result))
    }

    /// Create a new query builder
    pub fn query(&self) -> QueryBuilder {
        QueryBuilder::new(&self.db)
    }
}
