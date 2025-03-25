pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
/// High-level query interface for ploke database

mod error;
mod query;

pub use error::Error;
pub use query::{QueryBuilder, QueryResult};

/// Re-export common types for convenience
// Re-export common types from syn_parser
pub use syn_parser::parser::nodes::{
    FunctionNode, StructNode, EnumNode, TraitNode
};
pub use syn_parser::parser::types::TypeNode;
pub use syn_parser::parser::relations::RelationKind;

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
        let result = self.db.run_script(
            script,
            std::collections::BTreeMap::new(),
            cozo::ScriptMutability::Immutable,
        ).map_err(|e| Error::Cozo(e.to_string()))?;
        Ok(QueryResult::from(result))
    }
}
