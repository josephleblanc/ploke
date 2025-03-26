//! Formats query results for different output types

use crate::result::CodeSnippet;
use crate::span::CodeLocation;

/// Formats query results for different use cases
pub struct ResultFormatter;

impl ResultFormatter {
    /// Format as plain text with locations
    pub fn plain_text(snippets: &[CodeSnippet]) -> String {
        snippets.iter()
            .map(|s| format!("{}:{}:{}\n{}\n---\n", 
                s.file_path.display(),
                s.span.0,
                s.span.1,
                s.text))
            .collect()
    }

    /// Format as JSON with full metadata
    pub fn json(snippets: &[CodeSnippet]) -> String {
        serde_json::to_string(snippets).unwrap_or_default()
    }

    /// Format as markdown for documentation
    pub fn markdown(snippets: &[CodeSnippet]) -> String {
        snippets.iter()
            .map(|s| format!(
                "### `{}`\n\n```rust\n{}\n```\n\n*Location*: `{}` ({}-{})\n",
                s.metadata.get("name").unwrap_or(&"unnamed".to_string()),
                s.text,
                s.file_path.display(),
                s.span.0,
                s.span.1
            ))
            .collect()
    }
}
