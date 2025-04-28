//! Source location handling

use std::path::PathBuf;

/// Represents a precise code location
#[derive(Debug, Clone, PartialEq)]
pub struct CodeLocation {
    pub file: PathBuf,
    pub span: (usize, usize),
    pub text: String,
}

impl CodeLocation {
    /// Create from database values
    pub fn from_db(file: &str, start: usize, end: usize, text: &str) -> Self {
        Self {
            file: PathBuf::from(file),
            span: (start, end),
            text: text.to_string(),
        }
    }
}
