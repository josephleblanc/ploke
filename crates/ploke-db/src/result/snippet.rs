//! Code snippet result type

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::Error;

/// A retrieved code snippet with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSnippet {
    /// The actual code text
    pub text: String,
    /// Source file location
    pub file_path: PathBuf,
    /// Byte offsets (start, end)
    pub span: (usize, usize),
    /// Surrounding context lines
    pub context: String,
    /// Additional metadata
    pub metadata: Vec<(String, String)>,
}

impl CodeSnippet {
    /// Create new snippet from database row
    pub fn from_db_row(row: &[cozo::DataValue]) -> Result<Self, Error> {
        // Expected row format: [id, name, visibility, docstring]
        let name = row.get(1)
            .and_then(|v| v.get_str())
            .ok_or(Error::QueryExecution("Missing name in row".into()))?;
            
        Ok(Self {
            text: name.to_string(),
            file_path: PathBuf::new(), // TODO: Add file path tracking
            span: (0, 0), // TODO: Add span tracking
            context: String::new(), // TODO: Add context extraction
            metadata: vec![
                ("name".into(), name.into()),
                ("visibility".into(), 
                    row.get(2)
                        .and_then(|v| v.get_str())
                        .unwrap_or("unknown")
                        .into()
                ),
            ],
        })
    }
}
