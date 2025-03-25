/// High-level query interface for ploke database
mod error;
mod query;

pub use error::Error;
pub use query::builder::QueryBuilder;
pub use query::QueryResult;

/// Re-export common types for convenience
// Re-export common types from syn_parser
pub use syn_parser::parser::nodes::{EnumNode, FunctionNode, StructNode, TraitNode};
pub use syn_parser::parser::relations::RelationKind;
pub use syn_parser::parser::types::TypeNode;

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
}
