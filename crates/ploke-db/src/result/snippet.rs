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
        // TODO: Implement actual conversion
        Ok(Self {
            text: String::new(),
            file_path: PathBuf::new(),
            span: (0, 0),
            context: String::new(),
            metadata: Vec::new(),
        })
    }
}
