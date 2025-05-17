//! Query result handling and formatting

mod formatter;
mod snippet;

pub use formatter::ResultFormatter;
pub use snippet::CodeSnippet;

use crate::error::Error;
use cozo::NamedRows;

/// Result of a database query
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub rows: Vec<Vec<cozo::DataValue>>,
    pub headers: Vec<String>,
}

impl QueryResult {
    /// Convert query results into code snippets
    pub fn into_snippets(self) -> Result<Vec<CodeSnippet>, Error> {
        self.rows
            .iter()
            .map(|row| CodeSnippet::from_db_row(row))
            .collect()
    }
}

impl From<NamedRows> for QueryResult {
    fn from(named_rows: NamedRows) -> Self {
        Self {
            rows: named_rows.rows,
            headers: named_rows.headers,
        }
    }
}
