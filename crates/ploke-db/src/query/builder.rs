//! Query builder implementation

use crate::error::Error;
use crate::QueryResult;
use std::collections::BTreeMap;

/// Main query builder struct
pub struct QueryBuilder {
    db: cozo::Db<cozo::MemStorage>,
    selected_node: Option<NodeType>,
    filters: Vec<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone)]
enum NodeType {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
}

impl QueryBuilder {
    /// Create a new query builder
    pub fn new(db: cozo::Db<cozo::MemStorage>) -> Self {
        Self {
            db,
            selected_node: None,
            filters: Vec::new(),
            limit: None,
        }
    }

    /// Select functions to query
    pub fn functions(mut self) -> Self {
        self.selected_node = Some(NodeType::Function);
        self
    }

    /// Select structs to query
    pub fn structs(mut self) -> Self {
        self.selected_node = Some(NodeType::Struct);
        self
    }

    /// Filter by name (exact match)
    pub fn with_name(mut self, name: &str) -> Self {
        self.filters.push(format!("name = '{}'", name));
        self
    }

    /// Set maximum number of results
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Execute the constructed query
    pub fn execute(self) -> Result<QueryResult, Error> {
        let relation = match self.selected_node {
            Some(NodeType::Function) => "functions",
            Some(NodeType::Struct) => "structs",
            _ => return Err(Error::QueryConstruction("No node type selected".into())),
        };

        let mut query = format!(
            "?[id, name, visibility, docstring] := *{}[id, name, visibility, docstring]",
            relation
        );

        if !self.filters.is_empty() {
            query.push_str(", ");
            query.push_str(&self.filters.join(", "));
        }

        if let Some(limit) = self.limit {
            query.push_str(&format!(" :limit {}", limit));
        }

        self.db.run_script(
            &query,
            BTreeMap::new(),
            cozo::ScriptMutability::Immutable,
        )
        .map(QueryResult::from)
        .map_err(|e| Error::Cozo(e.to_string()))
    }
}
