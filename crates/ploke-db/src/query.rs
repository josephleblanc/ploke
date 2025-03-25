//! Query building and execution

use crate::Error;
use cozo::NamedRows;
use std::collections::BTreeMap;

/// Result of a database query
#[derive(Debug)]
pub struct QueryResult {
    pub rows: Vec<Vec<cozo::DataValue>>,
    pub headers: Vec<String>,
}

impl From<NamedRows> for QueryResult {
    fn from(named_rows: NamedRows) -> Self {
        Self {
            rows: named_rows.rows,
            headers: named_rows.headers,
        }
    }
}

/// Builder for constructing complex queries
pub struct QueryBuilder {
    base_query: String,
    filters: Vec<String>,
    limits: Option<usize>,
}

impl QueryBuilder {
    /// Create new query builder
    pub fn new(base_query: impl Into<String>) -> Self {
        Self {
            base_query: base_query.into(),
            filters: Vec::new(),
            limits: None,
        }
    }

    /// Add filter condition
    pub fn filter(mut self, condition: impl Into<String>) -> Self {
        self.filters.push(condition.into());
        self
    }

    /// Set maximum results
    pub fn limit(mut self, limit: usize) -> Self {
        self.limits = Some(limit);
        self
    }

    /// Build final query string
    pub fn build(self) -> String {
        let mut query = self.base_query;
        
        if !self.filters.is_empty() {
            query.push_str(" ");
            query.push_str(&self.filters.join(" "));
        }

        if let Some(limit) = self.limits {
            query.push_str(&format!(" :limit {}", limit));
        }

        query
    }
}
