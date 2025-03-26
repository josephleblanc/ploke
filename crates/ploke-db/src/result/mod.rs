//! Query result handling and formatting

mod snippet;
mod formatter;

pub use snippet::CodeSnippet;
pub use formatter::ResultFormatter;

/// Result of a database query
#[derive(Debug)]
pub struct QueryResult {
    pub rows: Vec<Vec<cozo::DataValue>>,
    pub headers: Vec<String>,
}

impl From<cozo::NamedRows> for QueryResult {
    fn from(named_rows: cozo::NamedRows) -> Self {
        Self {
            rows: named_rows.rows,
            headers: named_rows.headers,
        }
    }
}
